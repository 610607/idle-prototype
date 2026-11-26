# 开源挂机游戏实现调研

Nov 26, 2026 · @biobot

结论：成熟的开源挂机游戏几乎都是浏览器 JS/TS 游戏，实现上已经有一套公认做法，可以直接照抄而不用自己发明：大数用 break\_eternity.js，主循环用固定步长加 Web Worker 计时，离线收益用「封顶的有限精度模拟」或「时间银行加速」，存档是 JSON 压缩成 base64 字符串、带版本号逐版迁移。最值得精读的两份代码是 Antimatter Dimensions（结构清晰）和 Synergism（TypeScript + zod 存档校验），两者都是 MIT 许可。

本文所有结论来自 2026-11-26 对各仓库默认分支的浅克隆和阅读，文件路径与行号对应当日版本。标「推断」的是我的判断，没在代码或文档里直接看到。

## 如果我们自己做：推荐复用什么

1. **技术栈用 TypeScript + 浏览器。** 本次看的 8 个游戏全部是 Web 技术栈，所有大数库、格式化库也是 JS 生态。Synergism 用 Capacitor 打包 iOS/Android、用 Electron 上 Steam，证明这条路走得到手机和桌面。UI 框架不关键：AD 用 Vue 2，wrtsc 用 React + Redux Toolkit。
2. **大数直接用 break\_eternity.js（MIT，v2.2.2）。** 它和 break\_infinity.js 接口一致，上限从 1e9e15 扩到 10↑↑1e308，README 称速度在 break\_infinity 的 0.5 到 2 倍之间。如果数值设计确定永远不超过 1e9e15，break\_infinity.js（MIT，v2.2.0）更简单、AD 和 Synergism 都在用。显示格式用 @antimatter-dimensions/notations（MIT）。
3. **主循环用固定步长 + 时间累加器，计时放到 Worker 里。** 浏览器会节流后台标签页的 setInterval，Kittens、Evolve 自己起 Web Worker 发 tick，Synergism 用 worker-timers 库。逻辑和界面分开：游戏状态推进只是一个纯函数 `tick(dt)`，这样离线模拟、单元测试、「预测器」都复用同一套逻辑（wrtsc 的 `src/engine/` 就是这么做的）。
4. **离线收益先选「时间银行加速」或「封顶模拟」，别一开始就做解析快进。** 三种做法的取舍见下面「离线收益」一节。时间银行实现最简单、不会出现离线和在线结果不一致；解析快进（Kittens）每个系统都要单写一份快进公式，维护成本最高。这本质上也是设计决定，要和另一条商业游戏调研对照着定。
5. **存档照 AD + Synergism 的组合做。** JSON → lz-string `compressToBase64`（或 pako deflate + base64）→ 加前缀魔字符串和版本号；读档时用 zod schema 校验，并把字符串转回 Decimal；迁移写成按版本号编号的函数表，读到旧档就依次执行。导入、导出、本地存档共用同一个编码函数，并保留几个自动备份槽。
6. **数值和内容写成数据对象，公式写成函数字段。** AD 的 `src/core/secret-formula/` 和 The Modding Tree 的 `addLayer({...})` 都是这样：每个升级是一个对象，cost、effect、description 是字段或函数。早期直接写 TS 对象就好，有类型检查；等内容量大了再考虑外置表格（推断）。
7. **可选：先用 The Modding Tree 做玩法原型。** 它是 MIT 的配置式模板，写几个 `addLayer` 就能跑起来，适合在正式立项前验证核心循环好不好玩。这是我的建议，不是哪个项目的做法。

## 横向对比

| 项目 | 技术栈 | 主循环 | 大数 | 离线收益 | 存档 | 许可证 | 最近提交 |
| --- | --- | --- | --- | --- | --- | --- | --- |
| Antimatter Dimensions | JS + Vue 2，vue-cli | setInterval，间隔可调，超 60 秒视为休眠转离线 | break\_infinity.js | 封顶模拟：50ms 基准，最多 1e6 tick，异步跑带进度条 | JSON → pako → base64，魔字符串 + 版本，编号迁移 | MIT | 2026-07 |
| Synergism | TypeScript，Capacitor，Electron | 固定步长累加器，桌面 200 tick/s、手机 40 | break\_infinity.js | 封顶 3 天起，切成 200 步模拟，部分资源解析计算 | zod schema + lz-string，迁移文件 | MIT | 2026-11 |
| Kittens Game | JS + Dojo | Web Worker，5 tick/s | 原生 double（推断：无大数依赖） | 解析快进「redshift」，封顶 10 或 40 游戏年 | JSON → lz-string，saveVersion 15，逐版迁移 | WET PAWS（禁止商用和衍生） | 2026-11 |
| Evolve | 原生 JS 模块，esbuild，jQuery | Web Worker 低漂移计时，250ms 快循环 + 1s 中循环 + 5s 长循环 | 原生 double（推断） | 时间银行：离线时间的 2/3 换成 2 倍速，封顶约 8 小时 | JSON → lz-string UTF16，约 90 段版本判断 | MPL-2.0 | 2026-06 |
| Trimps | 原生 JS，单文件 2 万行 | setTimeout 100ms，带漂移补偿追帧 | decimal.js（少量） | 「Time Warp」真模拟最多 24 小时，超出部分解析近似 | JSON → lz-string base64 | GPL | 2025-03 |
| The Modding Tree | 原生 JS + Vue 2.7 | setInterval 50ms | break\_eternity.js | 时间银行，默认封顶 1 小时，按剩余量的 1/10 加速消化 | JSON → base64，缺字段用默认值补 | MIT | 2024-10 |
| Idle Loops（Lloyd 版） | 原生 JS | setInterval | 原生 double | 时间银行，开关式 5 倍速 | JSON 存 localStorage，字段级版本判断 | 无许可证文件 | 2022-08 |
| wrtsc | TypeScript + React + Redux Toolkit | 引擎与 UI 分离 | 原生 number | 时间银行（README 所述） | 自写 JSON 存档 | MIT | 2022-10 |

还看了 A Dark Room（MPL-2.0）：它是叙事型增量游戏，没有离线收益，存档迁移在 `script/state_manager.js` 里按 1.0 → 1.3 逐版写，参考价值主要在叙事节奏，实现上不必细看。

## 离线收益的三种做法

| 做法 | 代表 | 怎么算 | 好处 | 代价 |
| --- | --- | --- | --- | --- |
| 封顶的有限精度模拟 | AD、Synergism、Trimps | 把离线时长切成有限个大 tick，用在线同一个 `tick(dt)` 跑一遍 | 只有一套逻辑，结果和在线一致 | 长离线时要异步跑、显示进度条；tick 越粗精度越差 |
| 解析快进 | Kittens | 每个系统写 `fastforward(days)`，按公式直接算结果 | 瞬间完成 | 每个系统都要单写，容易和在线逻辑不一致 |
| 时间银行加速 | Evolve、TMT、Idle Loops、wrtsc | 把离线时间存起来，回来后让游戏以 2 到 5 倍速跑 | 实现最简单，没有一致性问题 | 玩家不能立刻拿到收益，体验取决于设计 |

几个具体数字：AD 每个离线 tick 不短于 33ms、总数不超过 1e6（`src/core/storage/storage.js:86`）；Trimps 最多 864000 个 100ms tick，即 24 小时（`main.js:2700`）；Synergism 默认封顶 3 天，可被升级延长（`src/Calculate.ts:858` 一带）；Evolve 离线超过 120 秒才计入，封顶 11520 游戏日（`src/functions.js:176`）。

## 逐个看：可借鉴之处

### Antimatter Dimensions

最值得精读。主循环 `gameLoop(diff)` 在 `src/game.js:422`，由 `src/core/intervals.js:56` 的定时器按玩家设置的频率调用；单次间隔超过 60 秒就判定设备休眠，转去离线模拟。离线模拟 `simulateTime()` 在 `src/game.js:895`：先解析地加一部分资源，再用 `Async.run` 分批执行 tick，界面上有进度条和「Speed up」按钮。

存档编码在 `src/core/storage/serializer.js`：文本 → pako 压缩 → base64 → 去掉 `+ / =`，前后加 `AntimatterDimensionsSavefileFormat` 和 `EndOfSavefile`，中间带三字母版本号 `AAB`。迁移在 `src/core/storage/migrations.js`，是一个以数字为键的 `patches` 表，共 1243 行。读档时用 `src/utility/deepmerge.js` 把存档合并进默认 player 对象，顺便把字符串还原成 Decimal。数值内容全部放在 `src/core/secret-formula/`，和逻辑分离。

### Synergism

现代 TypeScript 写法的最佳参考，而且仍在活跃开发。主循环 `tick()` 在 `src/Synergism.ts:3793`：累加真实时间差，按固定 dt 反复调用 `tack(dt)`，每帧有时间预算，超时就留到下一帧。计时器统一走 `src/Timers.ts`，桌面端用 worker-timers。

存档 schema 在 `src/saves/PlayerSchema.ts`（1085 行），用 zod 描述每个字段，Decimal 字段用 `.transform()` 从字符串或数字转回来；迁移在 `src/saves/Migrations.ts`，导入导出用 lz-string（`src/ImportExport.ts`）。手机端存档走 Capacitor Preferences（`src/saves/SaveStorage.ts`）。

### Kittens Game

解析快进的代表。`js/time.js` 的 `calculateRedshift()` 把离线毫秒数除以 2000 换成游戏日，再让资源、建筑、村庄、宗教等模块各自 `fastforward(daysOffset)`。tick 由一个内联 Blob 生成的 Web Worker 发出（`game.js` 约 5260 行），存档有 `saveVersion` 和逐版 `migrateSave()`（`game.js:3304`），还有 jest 单元测试。许可证禁止衍生作品，只能看思路，不能拷代码。

### Evolve

多速率循环的参考：Worker（`evolve/evolve.js`）用记录抖动历史的低漂移计时器发 tick，主线程按 1:4:20 的比例跑快、中、长三个循环（`src/vars.js` 的 `webWorker` 对象），追帧最多 1 分钟。离线换成「加速时间」，逻辑在 `src/functions.js:176` 的 `addATime()`。迁移是 `src/vars.js` 里约 90 个 `if (convertVersion(global['version']) < N)` 块，可读性不如 AD 的编号表。

### Trimps

`gameTimeout()`（`main.js:20000`）是很干净的漂移补偿写法：记录应走的总时间，落后多少就补跑多少个 100ms tick，再用剩余误差调整下一次 setTimeout。离线 Time Warp 真实模拟最多 24 小时，超出部分交给 `trustworthyTrimps()` 用近似公式补。代码集中在几个上万行的文件里，结构不适合照搬；GPL 许可也意味着借代码会让整个项目变成 GPL。

### The Modding Tree

不是游戏而是模板：每个声望层是一个 `addLayer(id, {...})` 配置对象（示例在 `js/layers.js`），文档在 `docs/`。主循环是 `js/game.js:402` 的 50ms setInterval，离线时间存在 `player.offTime.remain`，每次多消化剩余量的 1/10。存档 `js/utils/save.js` 用 `fixData()` 拿默认值补齐新字段，这是最省事的「迁移」，但处理不了字段改名或含义变化。

### Idle Loops 与 wrtsc

两者都是「时间循环」玩法。Idle Loops 把离线时间累计到 `totalOfflineMs`，玩家手动打开 5 倍速消耗（`driver.js:612`）；仓库没有许可证文件，代码默认不可复用。wrtsc 是受它启发的 TypeScript 重写，引擎在 `src/engine/`、与 React 界面完全分离，README 说内置的预测器和游戏用同一套逻辑；它停在 beta，但架构干净，值得看目录结构。

## 常用库

| 库 | 用途 | 许可证 | 当前版本 | 谁在用 |
| --- | --- | --- | --- | --- |
| [break\_eternity.js](https://github.com/Patashu/break_eternity.js) | 大数，上限 10↑↑1e308，支持迭代幂等超运算 | MIT | 2.2.2 | The Modding Tree |
| [break\_infinity.js](https://github.com/Patashu/break_infinity.js) | 大数，上限约 1e9e15，偏重速度 | MIT | 2.2.0 | AD、Synergism |
| [BreakInfinity.cs](https://github.com/Razenpok/BreakInfinity.cs) | break\_infinity 的 C# 单文件移植，供 Unity 用 | MIT | 最近提交 2022-12 |  |
| @antimatter-dimensions/notations | 大数显示格式（科学、工程、字母等记法） | MIT | 3.2.0 | AD |
| lz-string | 存档压缩成 base64 或 UTF16 字符串 | MIT | 1.5.0 | Synergism、Kittens、Evolve、Trimps |
| pako | zlib 压缩 | MIT 与 Zlib | 3.0.3 | AD |
| zod | 存档 schema 校验与转换 | MIT | 4.6.7 | Synergism |
| worker-timers | 不受后台节流的定时器 | MIT | 8.0.34 | Synergism |

引擎方案：本次没找到成熟、仍在维护的开源挂机游戏引擎或框架。[unity-idle-library](https://github.com/Unity-Tools-and-Libraries/unity-idle-library) 的 README 写明已停止开发。Godot 资源库里有几个大数插件，但我没读代码，不做推荐。如果日后要做原生客户端，更现实的路线是 Web 版先跑通，再像 Synergism 一样用 Capacitor 或 Electron 打包（推断）。

## 许可证：哪些代码能借

- **可以直接借代码（MIT）：** Antimatter Dimensions、Synergism、The Modding Tree、wrtsc，以及上表所有库。保留原版权声明即可。
- **能借但有条件（MPL-2.0）：** Evolve、A Dark Room。被借用的文件本身改动后要以 MPL 开源，其余代码不受影响。
- **借了会传染（GPL）：** Trimps。它的 `license.txt` 是 GPL v2 全文，源码头部写的是 GPL v3 或更新版本，两者不一致；无论哪个版本，借代码都会让整个项目必须 GPL 开源。
- **只能看思路：** Kittens Game（许可证禁止衍生作品和商用）、Idle Loops（没有许可证文件，默认保留所有权利）。

## 来源

- [Antimatter Dimensions 源码](https://github.com/IvarK/AntimatterDimensionsSourceCode)
- [Synergism](https://github.com/Pseudo-Corp/SynergismOfficial)
- [Kittens Game](https://github.com/nuclear-unicorn/kittensgame)
- [Evolve](https://github.com/pmotschmann/Evolve)
- [Trimps](https://github.com/Trimps/Trimps.github.io)
- [The Modding Tree](https://github.com/Acamaeda/The-Modding-Tree)
- [Idle Loops（Lloyd 版）](https://github.com/Lloyd-Delacroix/omsi-loops)
- [wrtsc](https://github.com/deifactor/wrtsc)
- [A Dark Room](https://github.com/doublespeakgames/adarkroom)
- 库的许可证与版本号来自 npm 注册表，2026-11-26 查询。
