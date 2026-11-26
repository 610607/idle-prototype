//! What does a customised recruit cost? (docs/ideas.md, "消耗人口定制新队员")
//!
//! The greedy bot plays to hour 48 or 72. Then the game branches: one copy places a customised
//! order in the district with the most residents, the other does nothing. Both then run 24 hours
//! untouched (no bot moves, no auto-claim or auto-hire; auto-counter stays on), and we compare what
//! the order took away: resources, natural candidates, residents, lost surges, and how long the
//! district takes to grow back.
//!
//! `cargo run --release --example custcost [seeds] [balance patch JSON]`
//! The old balance (residents nearly free) for comparison:
//! `... custcost 6 '{"housing_capacity":40,"growth_bp":0,"homeless_leave_bp":0,"natives_base":12,
//!   "natives_per_ring":4,"natives_rand":10,"volunteer_div":1000,"score_pop_div":20}'`
use idle_engine::api::Engine;
use idle_engine::commands::Command;
use idle_engine::content::content;
use idle_engine::economy;
#[path = "bot_policy.rs"]
mod bot_policy;

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

#[derive(Default, Clone, Copy)]
struct Out {
    scrap: f64,
    parts: f64,
    intel: f64,
    candidates: f64,
    pop: f64,
    lost: f64,
    /// Hours until the district is back to the control's residents (AFTER+ if never).
    regrow: f64,
    district_pop: f64,
    cost: f64,
    n: f64,
}

fn run(e: &mut Engine, at: i64, hours: i64, district: u16, mut track: impl FnMut(i64, i32)) {
    for h in 1..=hours {
        e.advance(T0 + (at + h) * 3600);
        track(h, e.state.cycle.districts[district as usize].population);
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let seeds: u64 = args.get(1).and_then(|s| s.parse().ok()).unwrap_or(6);
    if let Some(p) = args.get(2) {
        idle_engine::content::patch_balance(p).expect("balance patch");
    }
    let c = content();
    for at in [48i64, 72] {
        let mut sums = [Out::default(); 3];
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
                bot_policy::step(&mut e);
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
            // The order: the most-populated district with a profession that can train someone.
            let calc = economy::calc(&base);
            let pick = calc
                .districts
                .iter()
                .filter(|d| d.owned > 0)
                .flat_map(|d| {
                    d.workers
                        .iter()
                        .filter(|(_, w)| **w >= idle_engine::content::balance().customize_base)
                        .map(move |(p, _)| (d.id, p.clone()))
                })
                .max_by_key(|(d, _)| base.cycle.districts[*d as usize].population);
            let Some((district, prof)) = pick else {
                continue;
            };
            let class = &c.profession(&prof).unwrap().class;
            let lock = c.classes[class].trait_pool.first().cloned();
            let mut control_pop = vec![0; AFTER as usize + 1];
            let mut ctl = Engine::from_state(base.clone());
            run(&mut ctl, at, AFTER, district, |h, p| {
                control_pop[h as usize] = p
            });
            for (k, o) in ORDERS.iter().enumerate() {
                let mut x = Engine::from_state(base.clone());
                let before = x.state.cycle.districts[district as usize].population;
                let cmd = Command::Customize {
                    district,
                    profession: prof.clone(),
                    class: None,
                    lock: if o.lock { lock.clone() } else { None },
                    rarity: o.rarity,
                };
                if x.command(&cmd).is_err() {
                    continue;
                }
                let cost = before - x.state.cycle.districts[district as usize].population;
                let mut regrow = None;
                run(&mut x, at, AFTER, district, |h, p| {
                    if regrow.is_none() && p >= control_pop[h as usize] {
                        regrow = Some(h);
                    }
                });
                let (a, b) = (&ctl.state.cycle, &x.state.cycle);
                let pop = |cy: &idle_engine::state::Cycle| -> i32 {
                    cy.districts.iter().map(|d| d.population).sum()
                };
                let s = &mut sums[k];
                s.scrap += (a.resources.scrap - b.resources.scrap) as f64;
                s.parts += (a.resources.parts - b.resources.parts) as f64;
                s.intel += (a.resources.intel - b.resources.intel) as f64;
                // The order itself is one extra candidate; count only the natural ones lost.
                s.candidates += (a.stats.candidates - (b.stats.candidates - 1)) as f64;
                s.pop += (pop(a) - pop(b)) as f64;
                s.lost += (b.stats.surges_lost - a.stats.surges_lost) as f64;
                s.regrow += regrow.unwrap_or(AFTER + 1) as f64;
                s.district_pop += before as f64;
                s.cost += cost as f64;
                s.n += 1.0;
            }
        }
        println!("## 第 {} 小时下单，之后 {} 小时（相对不下单）", at, AFTER);
        println!(
            "  订单               人数  街区原有  少得废材  少得零件  少得情报  少出候选  人口差  多丢来潮  长回来(时)"
        );
        for (k, o) in ORDERS.iter().enumerate() {
            let s = sums[k];
            if s.n == 0.0 {
                continue;
            }
            let n = s.n;
            println!(
                "  {:16} {:5.1} {:9.1} {:9.1} {:9.1} {:9.1} {:9.2} {:7.1} {:9.2} {:10.1}",
                o.name,
                s.cost / n,
                s.district_pop / n,
                s.scrap / n,
                s.parts / n,
                s.intel / n,
                s.candidates / n,
                s.pop / n,
                s.lost / n,
                s.regrow / n
            );
        }
    }
}
