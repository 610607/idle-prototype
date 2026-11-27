//! 引擎层：不认识任何游戏规则的基础设施。棋盘几何、种子随机、离散事件时钟、存档信封和哈希。
//! Nothing here names a resource, a tile kind or a mechanic; the mechanics layer plugs into the
//! clock through [`clock::Timeline`] and into the save envelope through [`save::Saved`].

pub mod clock;
pub mod geom;
pub mod hash;
pub mod rng;
pub mod save;
