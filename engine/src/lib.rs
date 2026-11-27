//! Idle prototype rules engine (design v1).
//!
//! Everything that decides an outcome lives here; the web and iOS clients only render view models
//! and send commands. All maths is integer (basis points for percentages) and every random roll
//! comes from the seeded [`rng::Rng`], so the same save and the same commands give the same result
//! on every platform.

pub mod actions;
pub mod api;
pub mod combat;
pub mod commands;
pub mod content;
pub mod defense;
pub mod doctrine;
pub mod economy;
pub mod geom;
pub mod i18n;
pub mod mapgen;
pub mod operative;
pub mod rng;
pub mod routes;
pub mod save;
pub mod sim;
pub mod state;
pub mod tasks;
pub mod textui;
pub mod view;
pub mod world;

#[cfg(feature = "wasm")]
pub mod wasm;
