//! Every combat 词条 (trait, equipment piece, doctrine) has a measured price (docs/balance-eval.md
//! §词条测价). `eval/prices.json` is the blessed price card; when content gains, loses or renames
//! one, re-measure with
//! `cargo run --release --example eval -- price --bless` and review what moved.
use idle_engine::content::content;
use std::collections::BTreeMap;

#[test]
fn every_combat_entry_has_a_price() {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/eval/prices.json");
    let text = std::fs::read_to_string(path).expect("eval/prices.json");
    let card: BTreeMap<String, serde_json::Value> = serde_json::from_str(&text).unwrap();
    let c = content();
    let mut want: BTreeMap<String, &str> = BTreeMap::new();
    for id in c.traits.keys() {
        want.insert(id.clone(), "trait");
    }
    for e in &c.equipment {
        want.insert(e.id.clone(), "equipment");
    }
    for d in &c.doctrines {
        want.insert(d.id.clone(), "doctrine");
    }
    let mut problems = Vec::new();
    for (id, kind) in &want {
        match card.get(id).and_then(|p| p["kind"].as_str()) {
            None => problems.push(format!("{id} ({kind}) has no price")),
            Some(k) if k != *kind => problems.push(format!("{id} is priced as {k}, not {kind}")),
            _ => {}
        }
    }
    for id in card.keys().filter(|id| !want.contains_key(*id)) {
        problems.push(format!("{id} is priced but no longer in content"));
    }
    assert!(
        problems.is_empty(),
        "{}\nre-measure: cargo run --release --example eval -- price --bless",
        problems.join("\n")
    );
}
