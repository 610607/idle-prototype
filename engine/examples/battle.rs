//! Print one battle log: `cargo run --example battle`.
use idle_engine::combat::*;
use idle_engine::content::content;

fn ally(class: &str, key: &str) -> UnitSpec {
    let cl = &content().classes[class];
    UnitSpec {
        key: key.into(),
        name: cl.name.clone(),
        icon: cl.icon.clone(),
        row: cl.row,
        max_hp: cl.base.hp,
        atk: cl.base.atk,
        def: cl.base.def,
        spd: cl.base.spd,
        ap: cl.base.ap,
        pp: cl.base.pp,
        stats: [55; 9],
        skills: cl.skills.clone(),
        gambits: cl.gambits.clone(),
        effects: vec![],
        start_tags: vec![],
    }
}

fn main() {
    let input = BattleInput {
        kind: BattleKind::Route,
        allies: vec![
            ally("vanguard", "op:1"),
            ally("marksman", "op:2"),
            ally("technician", "op:3"),
        ],
        waves: vec![vec![
            enemy_spec("rust_drone", 3, 10_000),
            enemy_spec("signal_ghost", 3, 10_000),
        ]],
        ally_tags: vec![],
        enemy_tags: vec![],
        grounding: 1,
        defense: true,
        record: true,
    };
    let r = simulate(&input);
    for e in &r.log {
        let pad = "  ".repeat(e.depth as usize);
        println!("{:>3} {}{}", e.id, pad, e.text);
        if let Some(f) = &e.formula {
            println!("    {}  = {}", pad, f);
        }
    }
    println!(
        "{:?} waves {}/{} rounds {} warden+{}",
        r.outcome, r.waves_cleared, r.total_waves, r.rounds, r.warden_gain
    );
}
