# 评估工具现状和缺口

2026-11-27 盘点：评估工具能评估什么、还缺什么、代码要不要整理。游戏本体的问题记在 `docs/game-issues.md`，这里只记评估工具自己的事。

## 结论

- **一轮（一周）之内基本成体系**：节奏、战斗、经济、防守、爽点时间轴、玩家感受都有确定性的指标和设计目标，`eval review` 能一次给出结论、自检和覆盖情况，LLM 试玩能接进同一套指标。
- **最大的缺口是轮回**：评估只跑第一轮，组织等级、跨轮保留、知识库这一层完全没测过，而它是设计里的长期成长线。其次是选择的意义没有直接量（第二节 3）。
- **打法**：六个人格（`greedy`、`explorer`、`builder`、`turtle`、`scholar`、`tactician`）各有一份基线，`eval review` 并排给出打法离散度；局内只剩封存、锁定没人用。人格都是手写规则，找不到规则之外的打法。
- **代码已整理**（第三节，2026-11-27）：评估工具成了独立的库，机器人、测量、评估者都拆开了，补轮回和更多人格时改动面小。每一步都用“整理前的全部输出逐字节相同”验证行为没变。

## 一、现在能评估什么

| 维度 | 工具（`engine/evalkit/`） | 设计目标（`eval/goals.json`） | 可信度 |
|---|---|---|---|
| 一周节奏：进环时间 | `run`、`model`（节奏模型） | `week_cycle`（门槛） | 高：8 个种子，模型对得上 |
| 战斗卡扩张 | `strength`（模型强度、模拟标尺、墙）、`calibrate`、`price` | `combat_gate`（门槛） | 中：模型只看攻防血，效果类词条靠模拟标尺 |
| 局内成长 | `strength`（研究幅度）、`payoff` | `growth_in_cycle` | 中 |
| 爽点节奏 | `payoff`（时间轴、新鲜度、回访命中、峰值） | `hook`、`every_visit`、`peaks_rise`、`novelty_paced` | 中：分级已被 LLM 试玩粗校准（`llm.delight_lift` 0.29～0.43） |
| 防守 | `run`（来潮失守率） | `defense`（门槛） | 中：参考机器人会防守了（失守 0.67 → 0.47），但同一种子 Sonnet 打到 0.22、机器人 0.43，机器人只是下限 |
| 经济流转 | `run`（库存、无事可花） | `economy_flow`（门槛） | 高 |
| 三项成果 | `run` | `three_lines`（门槛）、`score_curve` | 中：`score_curve` 区间还按 72 小时一轮 |
| 玩家感受 | `session`（LLM 会话重放，`llm.*`） | `llm_feel` | 低：每种性格 1 局 |
| 工具自身 | 注册表（含缺值读法）、事件归类、会话过期、模型比值、标尺封顶、指标消失 | `model_agrees`、`payoff_calibrated`、`interface_usable` 和自动发现 | 高：都有测试 |

## 二、缺口

按重要程度排。

1. **轮回没有评估**。
   - 机器人从不收束（`collapse`）、不进知识库（`archive`），也不封存、锁定（`seal`、`lock`）。
   - 所以下面这些都没量过：组织经验和等级（2 级解锁 `chain`、`overclock`，3 级解锁 `spotter`）、特别队员跨轮保留的成长、知识库带进下一轮的条令、第二轮比第一轮快多少。
   - 要补：
     - 多轮运行：机器人第 168 小时收束、接着玩下一轮；
     - 每轮的指标：组织经验、到 2 级和 3 级用了几轮、第 k 轮的进环时间、带过去的强度；
     - 设计目标：“后一轮比前一轮快、有新东西”。
2. **人格还只是规则机器人**。
   - 2026-11-27 加了 `builder`、`turtle`、`scholar`、`tactician` 四个人格（`docs/balance-eval.md`“机器人人格”），`eval review` 加了第五节“打法离散度”。局内命令里只剩 `seal`、`lock` 没人用（加上轮回的 `archive`、`collapse`）。
   - 每个人格都是“参考打法改一处”的手写规则，找不到规则之外的打法，也找不到漏洞（如第 17 条“放弃再夺回重复计成果”是读代码发现的，没有人格去刷）。
   - 要补：随机合法策略或进化搜索，专门找成果最高的打法和漏洞。
3. **选择的意义没有直接量**。
   - 研究三选一、建什么、派谁驻守，选不同的结果差多少，现在只有战斗词条有测价（`price`）。
   - 要补：在选择点把局面分叉，各选一个玩下去比结果（`eval choice`）。
4. **LLM 试玩样本太小**。
   - 每种性格 1 局，报告里的判断都是单样本。
   - Sonnet 只扮了精算规划，模型和性格分不开。
   - 感受文字靠分析员模板（`docs/prompts/analyst.md`）人工归纳。
   - 要补：每种性格至少 3 个种子；同一性格换模型跑一次作对照。
5. **设计目标还是草案**。`goals.json` 是按文档起草的，没有逐条确认；`score_curve` 的区间按 72 小时一轮定，要重定。
6. **跑一局的时间**。
   - 补防守规则后，参考机器人一个种子一周约 2 分钟（种子 0 实测 137 秒）；种子按核分摊，8 个种子在 4 核上每核两个，约 5 分钟。
   - 更准的防守做法（每小时把建造也逐个推演）要慢三倍，没采用（`docs/balance-eval.md`“机器人人格”）。
7. **网页没有评估**。评估只走文字版；网页和文字版的信息是否一致，只靠“文字版跟着功能一起改”的约定和文字快照。

## 三、代码结构

2026-11-27 按盘点时的建议分五步整理完：工具变成库、机器人改成显式策略、测量拆成探针、评估者拆成判定和报告、整理脚本。整理前：评估工具是一个 example（`engine/examples/eval/`，12 个文件），机器人 `examples/bot_policy.rs` 被 8 个目标用 `#[path]` 各编一份；人格靠线程局部变量 `DEEP_FIRST`、`USED`；`run::measure` 一个函数约 180 行写下所有基础指标；`review::review` 一个函数约 320 行，判定和渲染混在一起。

现在：

- `engine/` 是工作区：规则引擎 `idle_engine` 和评估工具库 `evalkit`（`engine/evalkit/`）。数据仍在 `engine/eval/`（目标、基线、注册表、价格、爽点归类、会话）。
- `evalkit/src/` 的分工：

| 模块 | 内容 |
|---|---|
| `bot/` | 机器人：`Policy`（先打最深、按闲置居民建造、研究、侧重、照看小队）+ `Bot`（一局一个，自己记被接受的命令）。`mod.rs` 是参考打法，`districts.rs` 是各侧重的街区设置和放弃，`tactics.rs` 是照看小队（`docs/balance-eval.md`“机器人人格”）。判断空闲按分派状态，不再比较界面文字 |
| `run.rs` | 人格（`Style`、`Persona::policy`）、开局、驾驶者（机器人或日志重放）、`measure`（约 40 行，只推进时间、让驾驶者行动）、多种子运行 |
| `probes.rs` | 探针：`Pace`（占地、到环时间、检查点上的前线难度和最强小队）、`Fights`（路线战斗按深度、来潮按位置）、`Economy`（收入、库存、花钱节奏、研究时机、成果），加上 `strength::Series` 和 `payoff::Timeline` |
| `game.rs` | 几处共用的局面读数（最深环、可打的格子、库存、小队战斗单位），原来在 `run.rs` 和 `strength.rs` 各写一份 |
| `review.rs` | 目标、逐条检查、判定（`judge` 返回 `Judgment`：每条检查和基线读数、带类型的自动发现、覆盖、显著变化） |
| `report.rs` | 只负责把判定写成 `eval review` 的 markdown 和 `eval check` 的逐行输出 |
| 其余 | `registry`（指标注册表）、`stats`、`payoff`、`strength`、`model`、`calib`、`power`、`price`、`synergy`、`session` |

- 脚本：`evalkit/examples/` 放 `eval` 和用到机器人或 `synergy` 的脚本，`engine/examples/` 放文字版外壳 `agent_cli` 和不依赖机器人的脚本。`autoplay` 从 `agent_cli` 里拆出来成了独立脚本，文字版不再带机器人。
- 探索脚本没有删：文档里的数字（`docs/play-guide.md`、`docs/gameplay-exploration.md`、`docs/ideas.md`）靠它们复现。现在它们依赖库而不是按路径包含，`cargo test` 会把它们编一遍；`engine/README.md` 把命令分成评估工具、文字版和试玩、开发小工具、探索脚本四组，写明探索脚本的输出不保证和文档一致。
- 顺带的改动：网页的 `npm run wasm` 改成只编引擎（`cargo build -p idle_engine`），因为工作区默认成员包含 `evalkit`。

验证：整理前录了一份参考输出（`eval` 的 run、session、review、check、compare、metrics、numbers、pace、payoff、timeline、model、price，文字快照，网页快照存档，`autoplay`），每一步之后重跑，全部逐字节相同；`cargo test` 全过。最后用整理后的代码跑 `greedy` 8 个种子 × 168 小时，和整理前那次运行的文件逐项相同，`eval review` 对提交的基线报告显著变化 0 项。

还没动的：引擎里 `textui.rs`（约 1300 行）、`view.rs`（约 1500 行）、`combat.rs`（约 2700 行）属于游戏本体，之后再说；`payoff.rs`（约 760 行）、`session.rs`、`review.rs`（各约 600 行）还偏大，但职责单一，暂不拆。

## 四、待办

- [x] 代码整理（第三节）
- [ ] 多轮运行和轮回指标、目标（第二节 1）
- [x] 更多人格和打法离散度（第二节 2）：四个人格、`eval review` 第五节
- [ ] 找打法和漏洞的搜索型人格（第二节 2）
- [ ] 选择分叉评估（第二节 3）
- [ ] 第二批 LLM 试玩：每种性格 3 个种子，加一组换模型对照（第二节 4）
- [ ] 逐条确认 `goals.json`，重定 `score_curve`（第二节 5）
- [x] 跑数文件的浮点数逐位往返：`evalkit` 的 serde_json 开了 `float_roundtrip`，`review --bless` 写出的基线和原运行文件逐字节相同；六份基线已按原运行文件重写（只差最后一位的 289 处）。`cargo test` 里的 `baselines_round_trip_exactly` 会拦。引擎的状态和内容里没有浮点数，这个特性即使随工作区一起编进引擎也不影响规则。
- [ ] 资源按来源记账：情报有档案馆、线索格、见习任务三个来源，现在只记总数，查不出后半周靠哪个（`docs/game-issues.md` 第 3 条）

## 五、最近修掉的工具问题

- **缺值读法**：以前所有指标的缺值都按“没发生、最晚”读，比例类指标在没有样本的种子上被当成最差值。`surge_lost.inside` 因此读成 0.50（实际 0），`deep_win.h24` 读成“无法判断”（实际有值的 3 个种子都是 1）。现在注册表每条都要写 `late` 或 `skip`（`docs/balance-eval.md`“指标”）。
- **标尺封顶误报**：没有小队的种子让 `payoff.ladder.h*` 读成无穷大，被当成“到顶”。现在跳过这些种子。
