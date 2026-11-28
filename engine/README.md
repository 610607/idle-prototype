# engine：v1 规则引擎

Rust 写的确定性规则引擎。客户端（网页、以后的 iOS）只做两件事：发 JSON 命令进来，拿视图 JSON 出去。

## 约定

- **整数**：百分比一律用万分比（bp），没有浮点。
- **确定性**：随机数只来自存档里的 SplitMix64 种子；只用 `BTreeMap`/`Vec`，不依赖哈希顺序。战斗本身不掷骰子，同样的输入一定打出同样的结果，所以战报可以随时重放。
- **内容编进二进制**：`content/*.json` 在编译时用 `include_str!` 嵌入，启动时校验一次（引用、标签原则）。
- **离线 = 在线**：`kernel::clock::advance` 是离散事件循环（机制层在 `mech::sim` 接入）（整点经济、来潮、小队往返、规章），离线多久都走同一条路径；测试保证在线逐秒推进和离线一次跳过的结果逐字节相同。

## 分层和模块

源码分五层，每层只用它前面的层（`tests/layers.rs` 检查）；加词条、加效果、加机制各改哪里见 `docs/architecture.md`。

| 层 | 模块 | 内容 |
| --- | --- | --- |
| 本地化 `l10n` | `l10n` | 文字目录（`locales/`）、`t!` / `txt!`、存档里的文字 |
| 引擎 `kernel` | `geom`, `rng`, `clock`, `save`, `hash`, `entity` | 15×15 棋盘几何、种子随机、离散事件时钟（`Timeline`）、存档信封（`Saved`）、FNV 哈希、实体表（`Store`） |
| 数据 `data` | `data`, `data::hooks` | 内容定义和效果词汇、数值表、加载和交叉校验；`hooks` 是代码按名字认的少数内容 id |
| 机制 `mech` | `combat` | 前后排、AP 主动（战术条件→技能→目标）、PP 反应；六种标签；因果链日志（每条带公式和全场快照） |
| | `operative` | 队员生成、九项属性、特质、经历词条成长 |
| | `economy` | 住房与岗位、人口增长与无家可归者离开、电力、产出、信任因素、人才积累与候选、志愿民兵 |
| | `world`, `defense` | 边界、来潮来源、关注度；驻守与民兵、来潮结算、告急 |
| | `routes` | 路线校验、出发时预先打完整趟、回来时结算 |
| | `doctrine` | 研究（情报买三选一的条令）、换一批、条令进战斗、反应图鉴、知识库 |
| | `tasks` | 见习任务：每个系统一到三条「做一次」，按链解锁，小奖励 |
| | `mapgen` | 生成地图：天险、裂隙、特殊地点、街区、每格的敌人波次；轮初三选一预览 |
| | `actions`, `commands` | 所有玩家操作；`Command` 是它们的 JSON 形式 |
| | `sim` | 新游戏、选图、时钟接入（每一刻发生什么）、规章、特殊人物、潮汐预报、一轮结算与收束 |
| | `state` | 游戏状态（存档的内容）和 `SAVE_VERSION` |
| 客户端 `client` | `api` | `Engine`：命令、预演（dry-run，给出红→绿对比和前后 12 小时潮汐表）、驻守逐人预演、路线预演、战斗重放 |
| | `view` | 视图模型：客户端要画的一切，包括「待你决定」 |
| | `textui` | 文字版和试玩会话（`textui::session`） |
| | `wasm` | `wasm` feature 下的 wasm-bindgen 绑定 |

## 目录和命令

`engine/` 是一个工作区：规则引擎（本目录的 `src/`）和评估工具库 `evalkit/`（机器人、测量、评估者，见 `docs/balance-eval.md`）。在 `engine/` 下 `cargo test`、`cargo fmt` 两个都管；`cargo test` 也会把所有脚本编译一遍，所以脚本不会悄悄编不过，但探索脚本的输出没有测试。

```bash
cargo test                                   # 引擎和评估工具的全部测试
```

**评估工具**（`evalkit/examples/eval.rs`，用法见 `docs/balance-eval.md`、`docs/eval-interface.md`）：

```bash
cargo run --release --example eval -- run greedy --seeds 8 --hours 168 --out /tmp/g.json   # 参考机器人跑一批种子，记指标
cargo run --release --example eval -- review /tmp/g.json      # 对照设计目标和基线出报告
cargo run --release --example eval                           # 不带参数打印用法；子命令说明在 eval.rs 开头
```

**文字版和试玩**（`src/client/textui.rs`，`docs/llm-playtest.md`）：

```bash
cargo run --release --example agent_cli -- new <存档> <种子>   # 局面 + 编号菜单，给 LLM 玩家用；不带参数看用法
cargo run --release --example agent_cli -- do <存档> 1 3 --wait 4 --note "理由"
cargo run --release --example agent_cli -- session new <目录> --seed 0  # 试玩会话（src/client/textui/session.rs，docs/eval-interface.md）：固定日程、菜单 id、感受和打分；obs / act / serve
cargo run --release --example autoplay -- <存档> <种子> 72 4   # 同节奏的脚本玩家对照（BOT_NORESEARCH=1 关研究）
```

**开发小工具**：

```bash
cargo run --release --example text_snapshot -- <文件>        # 挪文字前后各跑一次，输出要逐字一致（docs/i18n.md）
cargo run --release --example web_fixtures                   # 网页文字快照用的存档（web/scripts/text_dump.mjs）
cargo run --release --example midsave 1 60 $(date +%s)       # 自动玩家玩 60 小时，打印一份结束在此刻的存档（网页「从文本读档」）
cargo run --example dump_view                                # 打印一份视图 JSON
cargo run --example battle                                   # 打一场示例战斗，打印因果链
```

**探索脚本**：一次性的玩法和数值研究，结论写在文档里。保留它们是为了能复现文档里的数字；游戏改过以后输出不保证和文档一致，要正式的读数用 `eval`。

```bash
cargo run --release --example bot 3 72       # 自动玩家跑 3 个种子各 72 小时，打印资源和节奏表（docs/play-guide.md 的数据来源；正式读数用 eval run / eval timeline）
cargo run --release --example militia        # 各圈层：民兵 / 单人驻守 / 三人小队的胜率（docs/gameplay-exploration.md）
cargo run --release --example opening        # 开局总部第一次来潮
cargo run --release --example synergy        # 220 支三人队爬难度梯子：加法模型、两两交互（加 equip 看装备，loop 开闭环）
cargo run --release --example doctrines      # 规则改写发给全队：单独、两两、成套（stack）、汇率（rate）
cargo run --release --example fixtures       # 守城：驻守职业 × 设施固定物组合
cargo run --release --example population     # 人口、岗位、闲置、人才和候选从哪来（docs/ideas.md）
cargo run --release --example customize      # 定制新队员值多少：指定词条 / 稀有度保底（加 loop 开闭环）
cargo run --release --example custcost       # 定制一单的代价：每个能下单的街区，下单和不下单各跑 24 小时比较
```

`population` 和 `custcost` 可以多给一个 JSON 参数临时改数值（`content::patch_balance`，只在内容第一次加载前有效），比如复现人口改稀缺之前的旧数值：

```bash
cargo run --release --example custcost 6 '{"housing_capacity":40,"growth_bp":0,"homeless_leave_bp":0,"natives_base":12,"natives_per_ring":4,"natives_rand":10,"volunteer_div":1000,"score_pop_div":20}'
```

synergy、doctrines、fixtures 的结果和读法见 `docs/gameplay-exploration.md`，population、customize、custcost 见 `docs/ideas.md`。用到机器人的脚本和 `eval` 共用 `evalkit/src/bot.rs`，改机器人会改它们的输出。

## 数值现状

数值是临时的（设计文档里标了「待定」）。目前用上面几个例子调到「能玩」：

- 来潮只带出源格 70% 的强度，深入路线抬高关注度后逐步回到满强度。
- 民兵按人数分成最多 3 个单位；住得下、又信任你的居民会出来当志愿民兵。
- 守线战两波之间回 15% 生命；[侵蚀] 每次结算后退一层。
- 第 1–2 圈单人驻守基本守得住，第 3 圈要驻守加民兵，第 4 圈起要好队员、装备和设施配合；三人小队走路线到第 5 圈仍有九成胜率。
