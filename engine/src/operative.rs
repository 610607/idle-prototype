//! Operatives: 底子 (nine fixed attributes), 本事 (skills and traits), 装备 (one item).
//! No levels or XP; experience grows new traits from what they actually do.

use crate::combat::UnitSpec;
use crate::content::{content, Stat, TraitEffect};
use crate::rng::Rng;
use crate::state::Operative;

pub const RARITY_NAMES: [&str; 4] = ["N", "R", "SR", "SSR"];

pub struct Origin<'a> {
    pub label: crate::i18n::Text,
    pub district: Option<u16>,
    pub profession: Option<&'a str>,
    pub flavor_trait: Option<&'a str>,
    pub policy_trait: Option<&'a str>,
    /// Extra weight toward higher rarity (trust, training).
    pub rarity_bonus_bp: i32,
    /// Customised recruits: rarity is at least this.
    pub min_rarity: u8,
    /// Customised recruits: this trait takes the first slot.
    pub lock_trait: Option<&'a str>,
}

pub fn trait_slots(rarity: u8) -> usize {
    [1, 2, 2, 3][rarity.min(3) as usize]
}

pub fn tactic_slots(op: &Operative) -> usize {
    (2 + (op.stats[Stat::Int.idx()] - 30).max(0) / 20) as usize
}

pub fn generate(rng: &mut Rng, id: u32, class_id: &str, origin: &Origin) -> Operative {
    let c = content();
    let class = &c.classes[class_id];
    let bonus = origin.rarity_bonus_bp.max(0);
    let rarity = rng
        .pick_weighted(&[
            6000 - bonus,
            3000 + bonus / 2,
            900 + bonus / 3,
            100 + bonus / 6,
        ])
        .unwrap_or(0) as u8;
    let rarity = rarity.max(origin.min_rarity.min(3));
    let mut stats = [0i32; 9];
    for s in Stat::ALL {
        let bias = class.bias.get(&s).copied().unwrap_or(0);
        stats[s.idx()] = (35 + rng.range(0, 25) + bias + rarity as i32 * 4).clamp(15, 90);
    }
    let mut traits: Vec<String> = class.fixed_traits.clone();
    if traits.is_empty() {
        let slots = trait_slots(rarity);
        if let Some(t) = origin.lock_trait {
            traits.push(t.to_string());
        }
        if let Some(p) = origin.policy_trait {
            if traits.len() < slots && !traits.iter().any(|t| t == p) {
                traits.push(p.to_string());
            }
        }
        if let Some(f) = origin.flavor_trait {
            if traits.len() < slots && !traits.iter().any(|t| t == f) {
                traits.push(f.to_string());
            }
        }
        let mut pool = class.trait_pool.clone();
        rng.shuffle(&mut pool);
        for t in pool {
            if traits.len() >= slots {
                break;
            }
            if !traits.contains(&t) {
                traits.push(t);
            }
        }
    }
    let name = if class.side == "otherworld" {
        rng.pick(&c.otherworld_names).cloned().unwrap_or_default()
    } else if class.side == "special" {
        class.name.clone()
    } else {
        format!(
            "{}{}",
            rng.pick(&c.family_names).cloned().unwrap_or_default(),
            rng.pick(&c.given_names).cloned().unwrap_or_default()
        )
    };
    Operative {
        id,
        name,
        class: class_id.to_string(),
        rarity: if class.side == "special" { 3 } else { rarity },
        origin: origin.label.clone(),
        origin_district: origin.district,
        origin_prof: origin.profession.map(|s| s.to_string()),
        stats,
        traits,
        exp_traits: vec![],
        gambits: class.gambits.clone(),
        equipment: None,
        counters: Default::default(),
        special: if class.side == "special" {
            Some(class_id.to_string())
        } else {
            None
        },
        locked: class.side == "special",
        assignment: Default::default(),
        rest_until: 0,
    }
}

/// All effects that apply to this operative (traits, grown traits, equipment), tagged with their source id.
pub fn effects(op: &Operative) -> Vec<(String, TraitEffect)> {
    let c = content();
    let mut out = Vec::new();
    for t in op.traits.iter().chain(op.exp_traits.iter()) {
        if let Some(def) = c.traits.get(t) {
            for e in &def.effects {
                out.push((t.clone(), e.clone()));
            }
        }
    }
    if let Some(eq) = &op.equipment {
        if let Some(def) = c.equip(eq) {
            for e in &def.effects {
                out.push((eq.clone(), e.clone()));
            }
        }
    }
    out
}

#[derive(Clone, Debug, serde::Serialize)]
pub struct Derived {
    pub hp: i32,
    pub atk: i32,
    pub def: i32,
    pub spd: i32,
    pub ap: i32,
    pub pp: i32,
    pub tactic_slots: usize,
}

pub fn derived(op: &Operative) -> Derived {
    let c = content();
    let class = &c.classes[&op.class];
    let s = |k: Stat| op.stats[k.idx()];
    let mut d = Derived {
        hp: class.base.hp + (s(Stat::Con) + s(Stat::Siz) - 100) / 3,
        atk: class.base.atk + (s(class.atk_stat) - 50) / 6,
        def: class.base.def + (s(Stat::Con) - 50) / 15,
        spd: class.base.spd + (s(Stat::Dex) - 50) / 8,
        ap: class.base.ap,
        pp: class.base.pp,
        tactic_slots: tactic_slots(op),
    };
    for (_, e) in effects(op) {
        if let TraitEffect::Stat {
            hp,
            atk,
            def,
            spd,
            pp,
        } = e
        {
            d.hp += hp;
            d.atk += atk;
            d.def += def;
            d.spd += spd;
            d.pp += pp;
        }
    }
    d.hp = d.hp.max(1);
    d.atk = d.atk.max(1);
    d
}

pub fn spec(op: &Operative) -> UnitSpec {
    let c = content();
    let class = &c.classes[&op.class];
    let d = derived(op);
    let mut gambits = op.gambits.clone();
    gambits.truncate(d.tactic_slots.max(1));
    UnitSpec {
        key: format!("op:{}", op.id),
        name: op.name.clone(),
        icon: class.icon.clone(),
        row: class.row,
        max_hp: d.hp,
        atk: d.atk,
        def: d.def,
        spd: d.spd,
        ap: d.ap,
        pp: d.pp,
        stats: op.stats,
        skills: class.skills.clone(),
        gambits,
        effects: effects(op)
            .into_iter()
            .filter(|(_, e)| !matches!(e, TraitEffect::Stat { .. }))
            .collect(),
        start_tags: vec![],
        fixture: false,
    }
}

/// Map-layer effects an operative carries (routes / garrisons).
pub fn has_map_effect(op: &Operative, f: impl Fn(&TraitEffect) -> bool) -> bool {
    effects(op).iter().any(|(_, e)| f(e))
}

pub fn reveal_bonus(op: &Operative) -> i32 {
    effects(op)
        .iter()
        .map(|(_, e)| match e {
            TraitEffect::RevealBonus { tiles } => *tiles,
            _ => 0,
        })
        .sum()
}

/// Add to an experience counter; returns traits newly grown.
pub fn bump(op: &mut Operative, counter: &str, by: i32) -> Vec<String> {
    if by <= 0 {
        return vec![];
    }
    *op.counters.entry(counter.to_string()).or_insert(0) += by;
    let have = op.counters[counter];
    let mut grown = Vec::new();
    for x in content().exp_traits.iter().filter(|x| x.counter == counter) {
        if have >= x.need
            && !op.exp_traits.contains(&x.trait_id)
            && !op.traits.contains(&x.trait_id)
            && op.exp_traits.len() < 3
        {
            op.exp_traits.push(x.trait_id.clone());
            grown.push(x.trait_id.clone());
        }
    }
    grown
}

#[cfg(test)]
mod tests {
    use super::*;

    fn origin() -> Origin<'static> {
        Origin {
            label: crate::txt!("origin.hq"),
            district: None,
            profession: Some("militia"),
            flavor_trait: Some("street_smart"),
            policy_trait: None,
            rarity_bonus_bp: 0,
            min_rarity: 0,
            lock_trait: None,
        }
    }

    #[test]
    fn generation_is_deterministic() {
        let a = generate(&mut Rng::new(3), 1, "vanguard", &origin());
        let b = generate(&mut Rng::new(3), 1, "vanguard", &origin());
        assert_eq!(
            serde_json::to_string(&a).unwrap(),
            serde_json::to_string(&b).unwrap()
        );
    }

    #[test]
    fn traits_fit_slots_and_flavor_comes_first() {
        for seed in 0..30 {
            let op = generate(&mut Rng::new(seed), 1, "marksman", &origin());
            assert!(op.traits.len() <= trait_slots(op.rarity));
            assert_eq!(op.traits[0], "street_smart");
        }
    }

    #[test]
    fn experience_grows_traits() {
        let mut op = generate(&mut Rng::new(1), 1, "vanguard", &origin());
        let need = content()
            .exp_traits
            .iter()
            .find(|x| x.counter == "surges_held")
            .unwrap()
            .need;
        assert!(bump(&mut op, "surges_held", need - 1).is_empty());
        assert_eq!(
            bump(&mut op, "surges_held", 1),
            vec!["night_watch".to_string()]
        );
        assert!(bump(&mut op, "surges_held", 1).is_empty());
    }

    #[test]
    fn specials_keep_fixed_traits() {
        let op = generate(&mut Rng::new(1), 7, "old_postman", &origin());
        assert!(op.traits.contains(&"barrier_runner".to_string()));
        assert_eq!(op.rarity, 3);
        assert!(has_map_effect(&op, |e| matches!(
            e,
            TraitEffect::CrossBarrier
        )));
    }
}
