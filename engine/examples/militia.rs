//! Defence curve: how often base militia (0/4/8 workers) and one starter hold a surge, by source ring.
use idle_engine::combat::{self, enemy_spec, BattleInput, BattleKind};
use idle_engine::defense::militia_units;
use idle_engine::geom::ring;
use idle_engine::state::Terrain;
use idle_engine::{mapgen, operative, rng::Rng};

fn main() {
    let mut rows = vec![[0i32; 8]; 8];
    let mut n = vec![0i32; 8];
    for seed in 0..40u64 {
        let m = mapgen::generate(seed, "static_turbulence");
        let mut rng = Rng::new(seed);
        let origin = operative::Origin {
            label: "x".into(),
            district: None,
            profession: None,
            flavor_trait: None,
            policy_trait: None,
            rarity_bonus_bp: 0,
        };
        let starters = ["vanguard", "marksman", "medic"]
            .map(|c| operative::spec(&operative::generate(&mut rng, 1, c, &origin)));
        for (i, t) in m.tiles.iter().enumerate() {
            if t.waves.is_empty() || t.terrain == Terrain::Hq {
                continue;
            }
            let r = ring(i as u16).max(1) as usize;
            if r > 7 {
                continue;
            }
            let sb = idle_engine::content::balance().surge_strength_bp;
            let full: Vec<_> = t
                .waves
                .iter()
                .map(|w| {
                    w.iter()
                        .map(|id| enemy_spec(id, r as i32, 10_000))
                        .collect::<Vec<_>>()
                })
                .collect();
            let waves: Vec<_> = t
                .waves
                .iter()
                .map(|w| {
                    w.iter()
                        .map(|id| enemy_spec(id, r as i32, sb))
                        .collect::<Vec<_>>()
                })
                .collect();
            let fight = |allies: Vec<combat::UnitSpec>| {
                combat::simulate(&BattleInput {
                    kind: BattleKind::Surge,
                    allies,
                    waves: waves.clone(),
                    ally_tags: vec![],
                    enemy_tags: vec![],
                    grounding: 0,
                    defense: true,
                    record: false,
                })
                .won()
            };
            n[r] += 1;
            for (k, mm) in [0, 4, 8].iter().enumerate() {
                if fight(militia_units(*mm)) {
                    rows[r][k] += 1;
                }
            }
            for (k, s) in starters.iter().enumerate() {
                if fight([vec![s.clone()], militia_units(0)].concat()) {
                    rows[r][3 + k] += 1;
                }
            }
            if fight([vec![starters[0].clone()], militia_units(8)].concat()) {
                rows[r][6] += 1;
            }
            let sq = combat::simulate(&BattleInput {
                kind: BattleKind::Route,
                allies: starters.to_vec(),
                waves: full.clone(),
                ally_tags: vec![],
                enemy_tags: vec![],
                grounding: 0,
                defense: false,
                record: false,
            });
            if sq.won() {
                rows[r][7] += 1;
            }
        }
    }
    println!("ring  n    m0   m4   m8   vang+m0 mark+m0 medic+m0 vang+m8  squad3 (hold %)");
    for r in 1..8 {
        if n[r] == 0 {
            continue;
        }
        let p = |k: usize| rows[r][k] * 100 / n[r];
        println!(
            "{r:>4} {:>4}  {:>3}  {:>3}  {:>3}   {:>6}  {:>6}  {:>6}  {:>6}  {:>6}",
            n[r],
            p(0),
            p(1),
            p(2),
            p(3),
            p(4),
            p(5),
            p(6),
            p(7)
        );
    }
}
