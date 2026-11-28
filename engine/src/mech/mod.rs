//! 机制层：游戏状态和全部规则。每个文件是一块机制；它们读数据层的定义（`crate::data`），按
//! 引擎层的时钟推进（`crate::kernel::clock`，接入点在 [`sim`]），玩家操作从 [`commands`] 进来。

pub mod actions;
pub mod armory;
pub mod combat;
pub mod commands;
pub mod cost;
pub mod defense;
pub mod doctrine;
pub mod economy;
pub mod mapgen;
pub mod medic;
pub mod operative;
pub mod pace;
pub mod pressure;
pub mod query;
pub mod routes;
pub mod score;
pub mod sim;
pub mod state;
pub mod tasks;
pub mod unlocks;
pub mod world;

/// Game version fingerprint (docs/eval-interface.md, manifest `build`): content, balance patch and
/// the state's save version.
pub fn fingerprint() -> String {
    crate::data::fingerprint(state::SAVE_VERSION)
}
