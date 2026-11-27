//! 客户端层：对外接口。[`api::Engine`] 收命令、给预演和视图（[`view`]），[`textui`] 是给 LLM
//! 试玩的文字版，`wasm` 是网页的绑定。规则不写在这里：这里只组合机制层的结果给人看。

pub mod api;
pub mod textui;
pub mod view;

#[cfg(feature = "wasm")]
pub mod wasm;
