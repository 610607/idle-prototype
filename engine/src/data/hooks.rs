//! 钩子：机制层按名字认的内容 id。
//!
//! 大多数词条只靠数据描述（效果种类 + 数字），机制层不需要知道它们叫什么。这里列的是例外：
//! 某个 id 背后有专门的一段代码。机制层只通过这些常量引用内容 id（`engine/tests/layers.rs`
//! 拦写死的 id），内容校验（[`check`]）保证它们在内容里都存在，改名或删掉时会报出来。

use super::Content;

/// Profession whose workers are a district's militia (the defenders of its border tiles).
pub const MILITIA: &str = "militia";
/// Facility whose district retakes more wardenship with a won counter.
pub const CLINIC: &str = "clinic";
/// Facility shown as the source of the grounding (shock → wardenship) log lines.
pub const GROUNDING_GRID: &str = "grounding_grid";
/// Trait: a garrison with it raises its district's trust.
pub const SILVER_TONGUE: &str = "silver_tongue";
/// Doctrine every research pool starts from (the reaction hub).
pub const HUB_DOCTRINE: &str = "react";
/// Class used when a profession names none.
pub const FALLBACK_CLASS: &str = "vanguard";
/// The crew every cycle starts with.
pub const START_CREW: [&str; 3] = ["vanguard", "marksman", "medic"];
/// Defenders (`defenders.json`) the surge code puts in: the HQ guard at the HQ, a district's
/// militia unit elsewhere (same id as the profession whose workers back it).
pub const HQ_GUARD: &str = "hq_guard";
pub const MILITIA_UNIT: &str = "militia";
/// The fixed map a new player starts on (`maps.json`).
pub const TUTORIAL_MAP: &str = "tutorial";
/// Automation rules (规章): each is a piece of code in `mech::sim`.
pub const RULE_AUTO_CLAIM: &str = "auto_claim";
pub const RULE_AUTO_RECRUIT: &str = "auto_recruit";
pub const RULE_AUTO_COUNTER: &str = "auto_counter";

/// Hooks missing from the content: each one is code that would silently stop working.
pub fn check(c: &Content) -> Vec<String> {
    let mut errs = Vec::new();
    let mut need = |ok: bool, kind: &str, id: &str| {
        if !ok {
            errs.push(format!("hook: {kind} {id} is named by code but missing"));
        }
    };
    need(c.profession(MILITIA).is_some(), "profession", MILITIA);
    need(c.facility(CLINIC).is_some(), "facility", CLINIC);
    need(
        c.facility(GROUNDING_GRID).is_some(),
        "facility",
        GROUNDING_GRID,
    );
    need(c.traits.contains_key(SILVER_TONGUE), "trait", SILVER_TONGUE);
    need(c.doctrine(HUB_DOCTRINE).is_some(), "doctrine", HUB_DOCTRINE);
    need(
        c.classes.contains_key(FALLBACK_CLASS),
        "class",
        FALLBACK_CLASS,
    );
    for id in START_CREW {
        need(c.classes.contains_key(id), "class", id);
    }
    for id in [HQ_GUARD, MILITIA_UNIT] {
        need(c.defenders.contains_key(id), "defender", id);
    }
    for id in [RULE_AUTO_CLAIM, RULE_AUTO_RECRUIT, RULE_AUTO_COUNTER] {
        need(c.rule(id).is_some(), "rule", id);
    }
    need(c.map(TUTORIAL_MAP).is_some(), "map", TUTORIAL_MAP);
    errs
}

/// Every content id named here, for the test that keeps other ids out of the mechanics layer.
pub fn all() -> Vec<&'static str> {
    let mut v = vec![
        MILITIA,
        CLINIC,
        GROUNDING_GRID,
        SILVER_TONGUE,
        HUB_DOCTRINE,
        FALLBACK_CLASS,
        HQ_GUARD,
        MILITIA_UNIT,
        RULE_AUTO_CLAIM,
        RULE_AUTO_RECRUIT,
        RULE_AUTO_COUNTER,
        TUTORIAL_MAP,
    ];
    v.extend(START_CREW);
    v
}
