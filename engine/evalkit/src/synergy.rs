//! Combat lab shared by the synergy probe (`examples/synergy.rs`) and the eval tool: the ten
//! classes, the fixed encounter ladder of real map encounters (rings 1..=7 plus over-rungs), team
//! members with or without traits, battles, and least-squares fits of team scores.
use idle_engine::data::{content, Stat, Tag};
use idle_engine::kernel::geom::ring;
use idle_engine::mech::combat::{
    self, enemy_spec, BattleInput, BattleKind, BattleReport, UnitSpec,
};
use idle_engine::mech::state::{Operative, Terrain};
use idle_engine::{kernel::rng::Rng, mech::mapgen, mech::operative};

pub const CLASSES: [&str; 10] = [
    "vanguard",
    "marksman",
    "medic",
    "striker",
    "occultist",
    "technician",
    "combat_sapper",
    "mist_walker",
    "echo_body",
    "glass_gardener",
];
pub const SHORT: [&str; 10] = [
    "先锋", "射手", "医护", "突击", "灵媒", "技师", "工兵", "雾行", "回声", "玻璃",
];
const RINGS: i32 = 7;
const PER_RING: usize = 8;
/// Past ring 7 the ladder keeps going with ring-7 encounters at higher strength (like attention).
const OVER: [i32; 6] = [12_500, 15_000, 17_500, 20_000, 25_000, 30_000];
pub const RUNGS: usize = RINGS as usize + OVER.len();

pub struct Enc {
    pub waves: Vec<Vec<UnitSpec>>,
    pub ally_tags: Vec<(Tag, i32)>,
    pub enemy_tags: Vec<(Tag, i32)>,
}

/// Route encounters by rung: rungs 1..=7 are rings, later rungs are ring 7 at higher strength.
pub fn encounters() -> Vec<Vec<Enc>> {
    let c = content();
    let mut raw: Vec<Vec<(Vec<Vec<String>>, &str)>> = (0..=RINGS).map(|_| Vec::new()).collect();
    for seed in 0..40u64 {
        for modifier in ["static_turbulence", "dense_fog"] {
            let m = mapgen::generate(seed, modifier);
            for (i, t) in m.tiles() {
                let r = ring(i);
                if t.waves.is_empty() || t.terrain == Terrain::Hq || r < 1 || r > RINGS {
                    continue;
                }
                let have = raw[r as usize].len();
                // Spread picks across seeds: at most one tile per ring per map.
                if have >= PER_RING || have as u64 > seed * 2 + (modifier == "dense_fog") as u64 {
                    continue;
                }
                raw[r as usize].push((t.waves.clone(), modifier));
            }
        }
    }
    let build = |r: i32, mult: i32, (waves, modifier): &(Vec<Vec<String>>, &str)| {
        let md = c.modifier(modifier).unwrap();
        Enc {
            waves: waves
                .iter()
                .map(|w| w.iter().map(|id| enemy_spec(id, r, mult)).collect())
                .collect(),
            ally_tags: md.ally_tags.clone(),
            enemy_tags: md.enemy_tags.clone(),
        }
    };
    let mut pool: Vec<Vec<Enc>> = vec![Vec::new()];
    for r in 1..=RINGS {
        pool.push(
            raw[r as usize]
                .iter()
                .map(|x| build(r, 10_000, x))
                .collect(),
        );
    }
    for m in OVER {
        pool.push(
            raw[RINGS as usize]
                .iter()
                .map(|x| build(RINGS, m, x))
                .collect(),
        );
    }
    pool
}

#[derive(Clone, Copy, PartialEq)]
pub enum Traits {
    Bare,
    Random,
}

/// One member: attributes fixed at the class mean (rarity R), traits either none or two drawn from
/// the class pool, so scores compare compositions rather than dice.
pub fn member(
    class: &str,
    sample: u64,
    slot: u32,
    traits: Traits,
    equip: Option<&str>,
) -> Operative {
    let cl = &content().classes[class];
    let mut rng = Rng::new(sample * 7919 + slot as u64 * 104_729 + class.len() as u64 * 31);
    let origin = operative::Origin {
        label: idle_engine::txt!("origin.hq"),
        district: None,
        profession: None,
        flavor_trait: None,
        policy_trait: None,
        rarity_bonus_bp: 0,
        min_rarity: 0,
        lock_trait: None,
    };
    let mut op = operative::generate(&mut rng, slot, class, &origin);
    op.rarity = 1;
    for s in Stat::ALL {
        op.stats[s.idx()] = 47 + 4 + cl.bias.get(&s).copied().unwrap_or(0);
    }
    match traits {
        Traits::Bare => op.traits.clear(),
        Traits::Random => {
            let mut pool = cl.trait_pool.clone();
            rng.shuffle(&mut pool);
            op.traits = pool.into_iter().take(2).collect();
        }
    }
    operative::with_item(&op, equip)
}

pub fn fight(allies: &[UnitSpec], e: &Enc, record: bool) -> BattleReport {
    combat::simulate(&BattleInput {
        kind: BattleKind::Route,
        allies: allies.to_vec(),
        waves: e.waves.clone(),
        ally_tags: e.ally_tags.clone(),
        enemy_tags: e.enemy_tags.clone(),
        grounding: 0,
        defense: false,
        record,
    })
}

#[derive(Default, Clone)]
pub struct Score {
    pub frontier: f64,
    pub applied: [f64; 6],
    pub consumed: [f64; 6],
    pub reactions: f64,
    pub battles: f64,
    /// Losses where someone was still standing after the round limit.
    pub timeouts: f64,
    pub losses: f64,
    /// Tag reactions fired (prototype), by id.
    pub reacts: std::collections::BTreeMap<String, f64>,
}

pub fn ladder(team: &[Operative], pool: &[Vec<Enc>]) -> Score {
    let mut specs: Vec<UnitSpec> = team.iter().map(operative::spec).collect();
    // `regen` on the command line: the map hands every member 6% max HP per round.
    if std::env::args().any(|a| a == "regen") {
        for s in specs.iter_mut() {
            s.effects.push((
                "regen".into(),
                idle_engine::data::TraitEffect::Regen { bp: regen_bp() },
            ));
        }
    }
    // `loop` on the command line: the reaction table plus the two transmutes that close the
    // shock → mark → erosion loop (the strongest stack in the doctrine probe).
    if std::env::args().any(|a| a == "loop") {
        use idle_engine::data::{Tag, TagEvent, TraitEffect};
        for s in specs.iter_mut() {
            for e in [
                TraitEffect::TagReactions,
                TraitEffect::Transmute {
                    on: TagEvent::Consumed,
                    from: Tag::Shock,
                    to: Tag::Mark,
                    stacks: 1,
                },
                TraitEffect::Transmute {
                    on: TagEvent::Consumed,
                    from: Tag::Mark,
                    to: Tag::Erosion,
                    stacks: 2,
                },
            ] {
                s.effects.push(("loop".into(), e));
            }
        }
    }
    // `react` on the command line: the prototype tag-reaction table is on for the squad.
    if std::env::args().any(|a| a == "react") {
        for s in specs.iter_mut() {
            s.effects.push((
                "reactions".into(),
                idle_engine::data::TraitEffect::TagReactions,
            ));
        }
    }
    let mut s = Score::default();
    for r in 1..=RUNGS {
        let mut wins = 0;
        for e in &pool[r] {
            let rep = fight(&specs, e, false);
            for (id, n) in &rep.reactions {
                *s.reacts.entry(id.clone()).or_insert(0.0) += *n as f64;
            }
            if rep.won() {
                wins += 1;
            } else {
                s.losses += 1.0;
                if rep.units.iter().any(|u| u.alive) {
                    s.timeouts += 1.0;
                }
            }
            for k in 0..6 {
                s.applied[k] += rep.applied[k] as f64;
                s.consumed[k] += rep.consumed[k] as f64;
            }
            s.reactions += rep.units.iter().map(|u| u.reactions as f64).sum::<f64>();
            s.battles += 1.0;
        }
        s.frontier += wins as f64 / pool[r].len().max(1) as f64;
    }
    s
}

pub fn teams() -> Vec<[usize; 3]> {
    let n = CLASSES.len();
    let mut out = Vec::new();
    for a in 0..n {
        for b in a..n {
            for c in b..n {
                out.push([a, b, c]);
            }
        }
    }
    out
}

pub fn name(t: &[usize; 3]) -> String {
    t.iter().map(|i| SHORT[*i]).collect::<Vec<_>>().join("+")
}

/// Least squares on features of a team. Returns (coefs, predictions).
pub fn fit(
    ts: &[[usize; 3]],
    y: &[f64],
    feats: &dyn Fn(&[usize; 3]) -> Vec<f64>,
) -> (Vec<f64>, Vec<f64>) {
    let p = feats(&ts[0]).len();
    let mut a = vec![vec![0.0; p + 1]; p];
    for (t, yy) in ts.iter().zip(y) {
        let x = feats(t);
        for i in 0..p {
            for j in 0..p {
                a[i][j] += x[i] * x[j];
            }
            a[i][p] += x[i] * yy;
        }
    }
    // Class counts always sum to 3, so the system is singular: a tiny ridge pins it.
    for (i, r) in a.iter_mut().enumerate() {
        r[i] += 1e-6;
    }
    for col in 0..p {
        let piv = (col..p)
            .max_by(|x, y| a[*x][col].abs().partial_cmp(&a[*y][col].abs()).unwrap())
            .unwrap();
        a.swap(col, piv);
        let d = a[col][col];
        for j in col..=p {
            a[col][j] /= d;
        }
        for i in 0..p {
            if i != col {
                let f = a[i][col];
                for j in col..=p {
                    a[i][j] -= f * a[col][j];
                }
            }
        }
    }
    let coef: Vec<f64> = (0..p).map(|i| a[i][p]).collect();
    let pred = ts
        .iter()
        .map(|t| feats(t).iter().zip(&coef).map(|(x, b)| x * b).sum())
        .collect();
    (coef, pred)
}

/// Intercept + one count per class: "each class is worth a fixed amount".
pub fn additive(t: &[usize; 3]) -> Vec<f64> {
    let mut x = vec![0.0; CLASSES.len() + 1];
    x[0] = 1.0;
    for c in t {
        x[1 + c] += 1.0;
    }
    x
}

/// Additive plus role coverage: has a healer, has a front-liner, has two or more of one class.
pub fn roles(t: &[usize; 3]) -> Vec<f64> {
    let mut x = additive(t);
    let front = |c: usize| content().classes[CLASSES[c]].row == idle_engine::data::Row::Front;
    x.push(t.contains(&2) as i32 as f64);
    x.push(t.iter().any(|c| front(*c)) as i32 as f64);
    x.push((t[0] == t[1] || t[1] == t[2]) as i32 as f64);
    x
}

pub fn r2(y: &[f64], pred: &[f64]) -> f64 {
    let m = y.iter().sum::<f64>() / y.len() as f64;
    let tot: f64 = y.iter().map(|v| (v - m).powi(2)).sum();
    let res: f64 = y.iter().zip(pred).map(|(v, p)| (v - p).powi(2)).sum();
    1.0 - res / tot
}

/// Regen per round for the `regen` option, in basis points (REGEN_BP, default 600).
pub fn regen_bp() -> i32 {
    std::env::var("REGEN_BP")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(600)
}
