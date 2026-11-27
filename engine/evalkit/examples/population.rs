//! Population probe (docs/ideas.md, "消耗人口定制新队员"): what is a resident worth, and where do
//! candidates come from? Plays the greedy bot and logs population, jobs and the candidate stream.
//!
//! `cargo run --release --example population [seeds] [hours] [balance patch JSON]`
//! e.g. `... population 6 120 '{"housing_capacity":12,"growth_bp":2500}'`
#[allow(dead_code)]
use evalkit::bot;
use idle_engine::api::Engine;
use idle_engine::commands::Command;
use idle_engine::content::content;
use idle_engine::economy;
use std::collections::BTreeMap;

const T0: i64 = 1_700_000_000;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let seeds: u64 = args.get(1).and_then(|s| s.parse().ok()).unwrap_or(6);
    let hours: i64 = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(120);
    // `old` as the 4th argument: the original bot that only builds where everyone stays housed.
    let staffing = args.get(4).map(|a| a != "old").unwrap_or(true);
    if let Some(p) = args.get(3).filter(|p| p.as_str() != "-") {
        idle_engine::content::patch_balance(p).expect("balance patch");
    }
    let marks = [12, 24, 48, 72, 96, 120, 168];
    let c = content();
    // Per mark: sums over seeds.
    let mut rows: BTreeMap<i64, [f64; 16]> = BTreeMap::new();
    let mut by_class: BTreeMap<String, i32> = BTreeMap::new();
    let mut by_source: BTreeMap<&'static str, i32> = BTreeMap::new();
    let mut by_rarity = [0i32; 4];
    let mut prof_workers: BTreeMap<i64, BTreeMap<String, f64>> = BTreeMap::new();
    for seed in 0..seeds {
        let mut e = Engine::new(seed, T0, "pop");
        e.command(&Command::ChooseMap { index: 0 }).unwrap();
        for r in ["auto_claim", "auto_recruit", "auto_counter"] {
            e.command(&Command::SetRule {
                rule: r.into(),
                on: true,
            })
            .unwrap();
        }
        let mut seen = std::collections::BTreeSet::new();
        for h in 1..=hours {
            bot::step_with(&mut e, staffing);
            e.advance(T0 + h * 3600);
            for cand in &e.state.cycle.candidates {
                if seen.insert(cand.id) {
                    *by_class.entry(cand.op.class.clone()).or_insert(0) += 1;
                    by_rarity[cand.op.rarity as usize] += 1;
                    let src = if cand.op.origin_district.is_some() {
                        "街区人才"
                    } else {
                        "裂隙/其他"
                    };
                    *by_source.entry(src).or_insert(0) += 1;
                }
            }
            if marks.contains(&h) {
                let calc = economy::calc(&e.state);
                let cy = &e.state.cycle;
                let pop: i32 = cy.districts.iter().map(|d| d.population).sum();
                let housing: i32 = calc.districts.iter().map(|d| d.housing).sum();
                let employed: i32 = calc.districts.iter().map(|d| d.employed).sum();
                let jobs: i32 = calc.districts.iter().flat_map(|d| d.jobs.values()).sum();
                let unhoused: i32 = calc.districts.iter().map(|d| d.unhoused).sum();
                let volunteers: i32 = calc.districts.iter().map(|d| d.volunteers).sum();
                let talent_h: i32 = calc
                    .districts
                    .iter()
                    .flat_map(|d| d.talent_rate.values())
                    .sum();
                let trust: f64 = {
                    let owned: Vec<_> = calc.districts.iter().filter(|d| d.owned > 0).collect();
                    owned
                        .iter()
                        .map(|d| cy.districts[d.id as usize].trust as f64)
                        .sum::<f64>()
                        / owned.len().max(1) as f64
                };
                let owned: Vec<_> = cy.tiles.iter().filter(|t| t.owned).collect();
                let facilities = owned
                    .iter()
                    .filter(|t| t.usage.facility().is_some())
                    .count();
                let alerts = owned.iter().filter(|t| t.alert()).count();
                let militia: i32 = calc.districts.iter().map(|d| d.militia).sum();
                let r = rows.entry(h).or_insert([0.0; 16]);
                r[9] += owned.len() as f64;
                r[10] += facilities as f64;
                r[11] += alerts as f64;
                r[12] += cy.stats.max_ring as f64;
                r[13] += calc.income[0] as f64 / 10_000.0;
                r[14] += calc.income[1] as f64 / 10_000.0;
                r[15] += militia as f64;
                r[0] += pop as f64;
                r[1] += housing as f64;
                r[2] += jobs as f64;
                r[3] += employed as f64;
                r[4] += unhoused as f64;
                r[5] += volunteers as f64;
                r[6] += talent_h as f64 / 100.0;
                r[7] += trust;
                r[8] += cy.operatives.len() as f64;
                let pw = prof_workers.entry(h).or_default();
                for d in &calc.districts {
                    for (p, w) in &d.workers {
                        *pw.entry(p.clone()).or_insert(0.0) += *w as f64;
                    }
                }
            }
        }
    }
    let n = seeds as f64;
    println!("## 人口与岗位（{} 个种子平均）", seeds);
    println!("  时  人口  住房  岗位  在岗  闲置  无房  志愿  人才/时  平均信任  队员");
    for (h, r) in &rows {
        println!(
            "{:4} {:5.0} {:5.0} {:5.0} {:5.0} {:5.0} {:5.0} {:5.1} {:7.1} {:8.1} {:5.1}",
            h,
            r[0] / n,
            r[1] / n,
            r[2] / n,
            r[3] / n,
            (r[0] - r[3]) / n,
            r[4] / n,
            r[5] / n,
            r[6] / n,
            r[7] / n,
            r[8] / n
        );
    }
    println!("\n## 地盘与产出");
    println!("  时  格子  设施  告急  最远圈  废材/时  零件/时  民兵");
    for (h, r) in &rows {
        println!(
            "{:4} {:5.1} {:5.1} {:5.1} {:7.1} {:8.1} {:8.1} {:5.1}",
            h,
            r[9] / n,
            r[10] / n,
            r[11] / n,
            r[12] / n,
            r[13] / n,
            r[14] / n,
            r[15] / n
        );
    }
    println!("\n## 各职业在岗人数");
    for (h, pw) in &prof_workers {
        let parts: Vec<String> = c
            .professions
            .iter()
            .map(|p| {
                format!(
                    "{} {:.1}",
                    p.name,
                    pw.get(&p.id).copied().unwrap_or(0.0) / n
                )
            })
            .collect();
        println!("{:4}  {}", h, parts.join("  "));
    }
    let total: i32 = by_class.values().sum();
    println!(
        "\n## {} 小时里出现的候选：每个种子 {:.1} 名",
        hours,
        total as f64 / n
    );
    let parts: Vec<String> = by_class
        .iter()
        .map(|(k, v)| format!("{} {}", c.classes[k].name, v))
        .collect();
    println!("  职业：{}", parts.join("，"));
    println!(
        "  稀有度 N/R/SR/SSR：{:?}；来源：{:?}",
        by_rarity, by_source
    );
}
