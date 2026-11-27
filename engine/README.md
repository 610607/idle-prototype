# engine：v1 规则引擎

Rust 写的确定性规则引擎。客户端（网页、以后的 iOS）只做两件事：发 JSON 命令进来，拿视图 JSON 出去。

## 约定

- **整数**：百分比一律用万分比（bp），没有浮点。
- **确定性**：随机数只来自存档里的 SplitMix64 种子；只用 `BTreeMap`/`Vec`，不依赖哈希顺序。战斗本身不掷骰子，同样的输入一定打出同样的结果，所以战报可以随时重放。
- **内容编进二进制**：`content/*.json` 在编译时用 `include_str!` 嵌入，启动时校验一次（引用、标签原则）。
- **离线 = 在线**：`sim::advance` 是离散事件循环（整点经济、来潮、小队往返、规章），离线多久都走同一条路径；测试保证在线逐秒推进和离线一次跳过的结果逐字节相同。

## 模块

| 模块 | 内容 |
| --- | --- |
| `geom`, `rng`, `content` | 15×15 棋盘几何、种子随机、内容定义与校验 |
| `mapgen` | 生成地图：天险、裂隙、特殊地点、街区、每格的敌人波次；轮初三选一预览 |
| `combat` | 前后排、AP 主动（战术条件→技能→目标）、PP 反应；六种标签；因果链日志（每条带公式和全场快照） |
| `operative` | 队员生成、九项属性、特质、经历词条成长 |
| `economy` | 住房与岗位、人口增长与无家可归者离开、电力、产出、信任因素、人才积累与候选、志愿民兵 |
| `world`, `defense` | 边界、来潮来源、关注度；驻守与民兵、来潮结算、告急 |
| `routes` | 路线校验、出发时预先打完整趟、回来时结算 |
| `doctrine` | 研究（情报买三选一的条令）、换一批、条令进战斗、反应图鉴、知识库 |
| `tasks` | 见习任务：每个系统一到三条「做一次」，按链解锁，小奖励 |
| `actions`, `commands` | 所有玩家操作；`Command` 是它们的 JSON 形式 |
| `sim` | 新游戏、选图、时间推进、规章、特殊人物、潮汐预报、一轮结算与收束 |
| `view` | 视图模型：客户端要画的一切，包括「待你决定」 |
| `save` | 存档信封：格式、版本、修订号、设备、校验和、逐版迁移 |
| `api` | `Engine`：命令、预演（dry-run，给出红→绿对比和前后 12 小时潮汐表）、驻守逐人预演、路线预演、战斗重放 |
| `wasm` | `wasm` feature 下的 wasm-bindgen 绑定 |

## 命令

```bash
cargo test                                   # 44 个测试
cargo run --example battle                   # 打一场示例战斗，打印因果链
cargo run --release --example bot 3 72       # 自动玩家跑 3 个种子各 72 小时，看节奏（会研究；BOT_NORESEARCH=1 关掉）
cargo run --release --example militia        # 各圈层：民兵 / 单人驻守 / 三人小队的胜率
cargo run --release --example opening        # 开局总部第一次来潮
cargo run --example dump_view                # 打印一份视图 JSON
cargo run --release --example synergy        # 220 支三人队爬难度梯子：加法模型、两两交互（加 equip 看装备，loop 开闭环）
cargo run --release --example doctrines      # 规则改写发给全队：单独、两两、成套（stack）、汇率（rate）
cargo run --release --example fixtures       # 守城：驻守职业 × 设施固定物组合
cargo run --release --example population     # 人口、岗位、闲置、人才和候选从哪来（docs/ideas.md）
cargo run --release --example customize      # 定制新队员值多少：指定词条 / 稀有度保底（加 loop 开闭环）
cargo run --release --example custcost       # 定制一单的代价：每个能下单的街区，下单和不下单各跑 24 小时比较
cargo run --release --example midsave 1 60 $(date +%s)  # 自动玩家玩 60 小时，打印一份结束在此刻的存档（网页「从文本读档」）
cargo run --release --example agent_cli -- new <存档> <种子>   # 文字版（src/textui.rs）局面 + 编号菜单，给 LLM 玩家用（docs/llm-playtest.md）；不带参数看用法
cargo run --release --example agent_cli -- do <存档> 1 3 --wait 4 --note "理由"
cargo run --release --example agent_cli -- autoplay <存档> <种子> 72 4   # 同节奏的脚本玩家对照（BOT_NORESEARCH=1 关研究）
```

`population` 和 `custcost` 可以多给一个 JSON 参数临时改数值（`content::patch_balance`，只在内容第一次加载前有效），比如复现人口改稀缺之前的旧数值：

```bash
cargo run --release --example custcost 6 '{"housing_capacity":40,"growth_bp":0,"homeless_leave_bp":0,"natives_base":12,"natives_per_ring":4,"natives_rand":10,"volunteer_div":1000,"score_pop_div":20}'
```

后六个是玩法探索用的：synergy、doctrines、fixtures 的结果和读法见 `docs/gameplay-exploration.md`，population、customize、custcost 见 `docs/ideas.md`。

## 数值现状

数值是临时的（设计文档里标了「待定」）。目前用上面几个例子调到「能玩」：

- 来潮只带出源格 70% 的强度，深入路线抬高关注度后逐步回到满强度。
- 民兵按人数分成最多 3 个单位；住得下、又信任你的居民会出来当志愿民兵。
- 守线战两波之间回 15% 生命；[侵蚀] 每次结算后退一层。
- 第 1–2 圈单人驻守基本守得住，第 3 圈要驻守加民兵，第 4 圈起要好队员、装备和设施配合；三人小队走路线到第 5 圈仍有九成胜率。
