//! 引擎层：不认识任何游戏规则的基础设施。棋盘几何、种子随机、离散事件时钟、存档信封、哈希和实体表。
//! Nothing here names a resource, a tile kind or a mechanic; the mechanics layer plugs into the
//! clock through [`clock::Timeline`], into the save envelope through [`save::Saved`] and keeps its
//! entities in [`entity::Store`] with a state type of its own.

pub mod clock;
pub mod entity;
pub mod geom;
pub mod hash;
pub mod rng;
pub mod save;
