//! wasm-bindgen surface for the web client. Thin: every method forwards to [`crate::client::api::Engine`]
//! and speaks JSON strings. Times cross the boundary as `f64` seconds (JS numbers), not BigInt.

use crate::client::api::Engine;
use crate::mech::commands::Command;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub struct WasmEngine {
    inner: Engine,
}

#[wasm_bindgen]
impl WasmEngine {
    #[wasm_bindgen(constructor)]
    pub fn new(seed: f64, now: f64, device: &str) -> WasmEngine {
        WasmEngine {
            inner: Engine::new(seed as u64, now as i64, device),
        }
    }

    pub fn load(save: &str) -> Result<WasmEngine, JsError> {
        Engine::load(save)
            .map(|inner| WasmEngine { inner })
            .map_err(|e| JsError::new(&e))
    }

    pub fn save(&self) -> String {
        self.inner.save()
    }

    pub fn time(&self) -> f64 {
        self.inner.state.time as f64
    }

    pub fn revision(&self) -> f64 {
        self.inner.state.revision as f64
    }

    /// Catch up to `now`; returns how many new events happened.
    pub fn advance(&mut self, now: f64) -> u32 {
        self.inner.advance(now as i64) as u32
    }

    pub fn command(&mut self, json: &str) -> String {
        self.inner.command_json(json)
    }

    pub fn dry_run(&mut self, json: &str) -> String {
        self.inner.dry_run_json(json)
    }

    pub fn view(&mut self) -> String {
        self.inner.view_json()
    }

    pub fn report(&self, id: u32) -> String {
        serde_json::to_string(&self.inner.report(id)).unwrap()
    }

    pub fn trip_battle(&self, squad: u32, tile: u16) -> String {
        serde_json::to_string(&self.inner.trip_battle(squad, tile)).unwrap()
    }

    pub fn garrison_options(&self, tile: u16) -> String {
        serde_json::to_string(&self.inner.garrison_options(tile)).unwrap()
    }

    /// `op`: -1 = current setup, 0 = nobody, otherwise an operative id.
    pub fn preview_surge(&self, tile: u16, op: i32) -> String {
        let choice = match op {
            -1 => None,
            0 => Some(None),
            id => Some(Some(id as u32)),
        };
        serde_json::to_string(&self.inner.preview_surge(tile, choice)).unwrap()
    }

    pub fn preview_route(&self, squad: u32, route_json: &str) -> String {
        let route: Vec<u16> = serde_json::from_str(route_json).unwrap_or_default();
        serde_json::to_string(&self.inner.preview_route(squad, &route)).unwrap()
    }

    pub fn content() -> String {
        Engine::content_json()
    }

    pub fn balance() -> String {
        Engine::balance_json()
    }

    /// Parse-check a command without running it (lets the client validate saved macros).
    pub fn parse_command(json: &str) -> bool {
        serde_json::from_str::<Command>(json).is_ok()
    }
}
