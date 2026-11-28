//! What does a customised recruit cost? (docs/ideas.md, "消耗人口定制新队员")
//!
//! The greedy bot plays to hour 48 or 72. Then, for every district that can pay for an order
//! (owned, enough residents, a profession with enough workers), the game branches: one copy
//! places the order there, the other does nothing. Both run 24 hours untouched (no bot moves, no
//! auto-claim or auto-hire; auto-counter stays on), and we compare what the order took away:
//! resources, natural candidates, lost surges, and how long the district takes to grow back.
//! Districts are split into border (some tile faces the tide) and interior.
//!
//! `cargo run --release --example custcost [seeds] [balance patch JSON]`
//! The old balance (residents nearly free) for comparison:
//! `... custcost 6 '{"housing_capacity":40,"growth_bp":0,"homeless_leave_bp":0,"natives_base":12,
//!   "natives_per_ring":4,"natives_rand":10,"volunteer_div":1000,"score_pop_div":20}'`
use evalkit::bot;
use idle_engine::client::api::Engine;
use idle_engine::data::{balance, content};
use idle_engine::mech::commands::Command;
use idle_engine::mech::state::GameState;
use idle_engine::{mech::economy, mech::world};

const T0: i64 = 1_700_000_000;
const AFTER: i64 = 24;

struct Order {
    name: &'static str,
    lock: bool,
    rarity: Option<u8>,
}

const ORDERS: [Order; 3] = [
    Order {
        name: "只指定职业",
        lock: false,
        rarity: None,
    },
    Order {
        name: "锁词条 + 保底 SR",
        lock: true,
        rarity: Some(2),
    },
    Order {
        name: "锁词条 + 保底 SSR",
        lock: true,
        rarity: Some(3),
    },
];
const GROUPS: [&str; 2] = ["边界街区", "内圈街区"];

#[derive(Default, Clone, Copy)]
struct Out {
    scrap: f64,
    parts: f64,
    candidates: f64,
    lost: f64,
    /// Hours until the district is back to the control's residents (AFTER+1 if never).
    regrow: f64,
    district_pop: f64,
    cost: f64,
    n: f64,
}

fn run(e: &mut Engine, at: i64, district: u16, mut track: impl FnMut(i64, i32)) {
    for h in 1..=AFTER {
        e.advance(T0 + (at + h) * 3600);
        track(h, e.state.cycle.district(district).population);
    }
}

/// Districts that can train someone: (id, profession with most workers, border?).
fn eligible(state: &GameState) -> Vec<(u16, String, bool)> {
    let calc = economy::calc(state);
    let border = world::border_tiles(&state.cycle);
    calc.districts
        .iter()
        .filter(|d| d.owned > 0)
        .filter_map(|d| {
            let (p, w) = d.workers.iter().max_by_key(|(_, w)| **w)?;
            (*w >= balance().customize_base).then(|| {
                let mut tiles = state.cycle.district_tiles(d.id);
                (d.id, p.clone(), tiles.any(|t| border.contains(&t)))
            })
        })
        .collect()
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let seeds: u64 = args.get(1).and_then(|s| s.parse().ok()).unwrap_or(6);
    if let Some(p) = args.get(2) {
        idle_engine::data::patch_balance(p).expect("balance patch");
    }
    let c = content();
    // (trust, regrow hours) for the 12-resident order, to see whether trust makes orders cheaper.
    let mut trust_regrow: Vec<(f64, f64)> = Vec::new();
    for at in [48i64, 72] {
        let mut sums = [[Out::default(); 3]; 2];
        for seed in 0..seeds {
            let mut e = Engine::new(seed, T0, "cost");
            e.command(&Command::ChooseMap { index: 0 }).unwrap();
            for r in ["auto_claim", "auto_recruit", "auto_counter"] {
                e.command(&Command::SetRule {
                    rule: r.into(),
                    on: true,
                })
                .unwrap();
            }
            for h in 1..=at {
                bot::step(&mut e);
                e.advance(T0 + h * 3600);
            }
            for r in ["auto_claim", "auto_recruit"] {
                e.command(&Command::SetRule {
                    rule: r.into(),
                    on: false,
                })
                .unwrap();
            }
            e.state.cycle.stats.contiguous_districts =
                e.state.cycle.stats.contiguous_districts.max(1);
            let base = e.state.clone();
            let mut ctl = Engine::from_state(base.clone());
            let picks = eligible(&base);
            let mut control_pop = vec![vec![0; AFTER as usize + 1]; picks.len()];
            for h in 1..=AFTER {
                ctl.advance(T0 + (at + h) * 3600);
                for (k, (d, _, _)) in picks.iter().enumerate() {
                    control_pop[k][h as usize] = ctl.state.cycle.district(*d).population;
                }
            }
            for (pi, (district, prof, is_border)) in picks.iter().enumerate() {
                let class = &c.profession(prof).unwrap().class;
                let lock = c.classes[class].trait_pool.first().cloned();
                let g = if *is_border { 0 } else { 1 };
                for (k, o) in ORDERS.iter().enumerate() {
                    let mut x = Engine::from_state(base.clone());
                    let before = x.state.cycle.district(*district).population;
                    let trust = x.state.cycle.district(*district).trust;
                    let cmd = Command::Customize {
                        district: *district,
                        profession: prof.clone(),
                        class: None,
                        lock: if o.lock { lock.clone() } else { None },
                        rarity: o.rarity,
                    };
                    if x.command(&cmd).is_err() {
                        continue;
                    }
                    let cost = before - x.state.cycle.district(*district).population;
                    let mut regrow = None;
                    run(&mut x, at, *district, |h, p| {
                        if regrow.is_none() && p >= control_pop[pi][h as usize] {
                            regrow = Some(h);
                        }
                    });
                    let regrow = regrow.unwrap_or(AFTER + 1) as f64;
                    if k == 1 {
                        trust_regrow.push((trust as f64, regrow));
                    }
                    let (a, b) = (&ctl.state.cycle, &x.state.cycle);
                    let s = &mut sums[g][k];
                    s.scrap += (a.resources.scrap - b.resources.scrap) as f64;
                    s.parts += (a.resources.parts - b.resources.parts) as f64;
                    // The order itself is one extra candidate; count only the natural ones lost.
                    s.candidates += (a.stats.candidates - (b.stats.candidates - 1)) as f64;
                    s.lost += (b.stats.surges_lost - a.stats.surges_lost) as f64;
                    s.regrow += regrow;
                    s.district_pop += before as f64;
                    s.cost += cost as f64;
                    s.n += 1.0;
                }
            }
        }
        println!("## 第 {} 小时下单，之后 {} 小时（相对不下单）", at, AFTER);
        println!(
            "  街区      订单               样本  人数  街区原有  少得废材  少得零件  少出候选  多丢来潮  长回来(时)"
        );
        for (g, gname) in GROUPS.iter().enumerate() {
            for (k, o) in ORDERS.iter().enumerate() {
                let s = sums[g][k];
                if s.n == 0.0 {
                    continue;
                }
                let n = s.n;
                println!(
                    "  {:8} {:16} {:5.0} {:5.1} {:9.1} {:9.1} {:9.1} {:9.2} {:9.2} {:10.1}",
                    gname,
                    o.name,
                    n,
                    s.cost / n,
                    s.district_pop / n,
                    s.scrap / n,
                    s.parts / n,
                    s.candidates / n,
                    s.lost / n,
                    s.regrow / n
                );
            }
        }
    }
    println!("\n## 12 人订单：街区信任和长回来的时间");
    for (lo, hi) in [(0, 35), (35, 50), (50, 65), (65, 101)] {
        let v: Vec<f64> = trust_regrow
            .iter()
            .filter(|(t, _)| *t >= lo as f64 && *t < hi as f64)
            .map(|x| x.1)
            .collect();
        if v.is_empty() {
            continue;
        }
        println!(
            "  信任 {:3}–{:3}：{:3} 单，平均 {:.1} 小时",
            lo,
            hi - 1,
            v.len(),
            v.iter().sum::<f64>() / v.len() as f64
        );
    }
}
