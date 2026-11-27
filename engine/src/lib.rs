//! Idle prototype rules engine (design v1).
//!
//! Everything that decides an outcome lives here; the web and iOS clients only render view models
//! and send commands. All maths is integer (basis points for percentages) and every random roll
//! comes from the seeded [`kernel::rng::Rng`], so the same save and the same commands give the same
//! result on every platform.
//!
//! # Layers
//!
//! Each layer may use only the layers above it in this list (`tests/layers.rs` checks it):
//!
//! | layer | module | what lives there | changes when |
//! |---|---|---|---|
//! | 本地化 | [`l10n`] (+ `locales/`) | text catalogs, `t!` / `txt!`, stored text | wording or a new language |
//! | 引擎 | [`kernel`] | board geometry, seeded random, the discrete-event clock, save envelope, hashing | almost never: it knows no game rule |
//! | 数据 | [`data`] (+ `content/`) | content definitions and their effect vocabulary, the balance table, loading and validation | a new entry (JSON only), or a new effect kind (schema + the mechanic that reads it) |
//! | 机制 | [`mech`] | game state and every rule: combat, economy, defense, routes, research, the cycle | a new mechanic |
//! | 客户端 | [`client`] | the `Engine` facade (commands, previews, views), the text client, wasm bindings | a new thing to show or preview |
//!
//! See `docs/architecture.md` for how to add an entry and how to add a mechanic.

pub mod l10n;

pub mod kernel;

pub mod data;

pub mod mech;

pub mod client;
