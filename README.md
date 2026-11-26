# idle-prototype

一个只给自己玩的挂机游戏原型：小队冒险加基地建设，一边夺回被异界渗透的街区，一边组织探索异界。最新设计是 [v1](docs/design-v1.md)；之前的 [v0 设计定稿](docs/design-v0.md)、早期概念草案和调研都在 [docs/](docs/README.md) 里。

## 目录

- [engine/](engine/README.md)：v1 规则引擎（Rust）。所有决定结果的规则都在这里：棋盘、路线与驻守、来潮与潮汐表、人口与信任、队员与战斗、规章、轮回。整数运算、种子随机，同一存档同一串命令在任何平台结果一致。编译成 WebAssembly 给网页用，以后用 UniFFI 给 iOS 用。
- [web/](web/)：v1 网页测试客户端（React + Vite）。只渲染引擎给的视图、把玩家操作作为 JSON 命令发回去，不自己算规则。「民防指挥所 × 锈蚀工业」暗色像素风；左栏是地图、潮汐时间带和待办，右侧宽工作区放格子、队员、街区、战报等详情页（可返回），小队路线画成地铁线。
- [icons/](icons/README.md)：共用的 16×16 像素图标（文本格式，界面按 id 读取）。内容数据只存图标 id。
- `src/`：v0 的 TypeScript 原型，留作参考，不再更新。

## 运行网页客户端

需要 Node 22 和 pnpm。仓库里已经带着编好的 WebAssembly（`web/src/engine/pkg/`），不装 Rust 也能跑：

```bash
cd web && pnpm install && pnpm dev
```

改了引擎之后重新编 WebAssembly（需要 Rust、`wasm32-unknown-unknown` 目标和 0.2.130 版的 `wasm-bindgen-cli`）：

```bash
cd web && pnpm wasm
```

「总部 → 存档与调试」里可以快进 1 小时到 1 天，走的是和离线回来完全一样的模拟路径。
