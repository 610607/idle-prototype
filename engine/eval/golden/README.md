# 重构对照物

引擎重构（[docs/design-v2-refactor.md](../../../docs/design-v2-refactor.md)）期间，每一步都要和这里记下的东西逐字节相等。这个目录在第一步（A1）之前记下，记录时的主干是 `760611e`（2026-11-28），`SAVE_VERSION` 6。

下面的命令都在 `engine/` 里跑。

## 有哪些

| 对照物 | 存在哪 | 怎么查 | 怎么重记 |
|---|---|---|---|
| 战斗报告，109 场 | `tests/golden/battles.json` | `cargo test`（`tests/golden.rs`） | `GOLDEN_BLESS=1 cargo test --test golden` |
| 局面轨迹，六个参考玩家各 24 小时 | 本目录 `trace-<玩家>.json` | `cargo run --release --example eval -- golden`；`cargo test` 里查贪心的前 6 小时 | `cargo run --release --example eval -- golden --bless` |
| 读数，六个参考玩家各 3 个种子、72 小时 | `eval/baseline-pre-refactor/<玩家>.json` | `eval run` 之后 `eval compare --exact`，见下 | 重跑 `eval run` 覆盖 |
| 会话重放 | `eval/sessions/*/*/final.json` 的 `view_sha` | `eval session DIR…`，见下 | `eval session DIR… --bless` |
| 文字快照 | 本目录 `text_snapshot.txt.xz` | 见下 | 重跑后重新压缩 |

重记只在有意改动玩法之后做。重构期间任何一样变了，都先查原因，不重记。

## 阶段 3 S4 重记（血量保留和医疗，2026-11-28）

S4 有意改了正式局的玩法（血量跨战斗保留、倒下读条取代“输了全队歇 30 分钟”、新资源医疗、治疗和用药线），`SAVE_VERSION` 7 → 8。

- **战斗报告**：109 场一场没变，没有重记。这些战斗都是满血开打（`UnitSpec::damage` 为 0 时不序列化，开打血量和以前一样）。
- **局面轨迹**：六个参考玩家都重记了（`eval golden --bless`）。从第 0 小时起局面数据就不同：多了血量、医疗、用药线这些字段，小队少了 `rest_until`。
- **文字快照**：重记了，sha256 见下面“文字快照”一节。
- **会话重放**：下面“会话重放”表里那 5 个会话是在 S4 之前玩的，现在重放都是过期（存档对不上，或者有命令被拒绝：`没有这名队员`），`eval session` 把它们标成 `session.stale`，只保留 `llm.feel.*`、`llm.score.*`。录像原样留着，不重记。
- **读数**：`baseline-pre-refactor/` 是重构前的玩法，S4 以后不再逐字相等，留作对照。

## 战斗报告

`tests/golden.rs` 在一局固定的游戏（种子 20261128，选第 1 张地图）上用 `route_battle`、`surge_setup`、`operative::spec`、`militia_units` 造出 109 场战斗，每场带记录结算，把整份报告的 JSON 取摘要（`kernel::hash::fnv64`）。文件里一场一行：名字、结果、回合数、清掉几波、日志条数、摘要。对不上时失败信息里写场次名和两边的这一行。

| 组 | 场数 | 内容 |
|---|---|---|
| `enemy/<敌人>/ring<环>` | 60 | 20 种敌人 × 第 1、3、5 环，两只同种敌人对起始三人 |
| `class/<职业>` | 13 | 每个职业的起始队员（和开局造队员同一条路）加起始先锋，打一个第 2 环的格子 |
| `doctrine/<学说>`、`doctrine/none` | 21 | 每条学说单独带着打同一个第 3 环的格子，外加不带学说的一场作对照 |
| `armory/level3` | 1 | 三条军械各 3 级，同一个格子 |
| `surge/bare`、`surge/<设施>`、`surge/hq` | 7 | 占下总部旁的一格、起始先锋驻守、军械 4 级：不建设施一场；路障、瞭望塔、诊所、路灯、接地网各建在这一格并配满人手一场；总部自己的来潮一场 |
| `militia/1`、`2`、`3` | 3 | 只有民兵，分成 1、2、3 队 |
| `core` | 1 | 核心，对一支生命 6 倍、攻击 4 倍的起始三人（打 42 回合，清掉 2 波） |
| `wipe` | 1 | 起始三人打第 5 环，全灭 |
| `retreat/emergency`、`retreat/planned` | 2 | 紧急撤退（先锋生命低于六成就撤）；计划撤退（打完第一波就走） |

矩阵里的敌人、职业、学说、设施是从内容数据里列出来的，所以加了新词条这个测试会失败（文件里没有那一场），重记即可。测试里凡是直接改局面字段的地方都收在文件开头的几个小函数里，局面的结构变了只用改那几处，摘要不该变。

## 局面轨迹

`evalkit/src/golden.rs`。每个参考玩家（greedy、explorer、builder、turtle、scholar、tactician）种子 0、24 小时，走法和 `eval run` 一样（每小时开头机器人行动，然后每 5 分钟推进一次），所以轨迹的第 h 小时就是 `eval run` 里种子 0 的第 h 小时。每小时一行：

- `view`：局面数据（`Engine::view_json()`，网页拿到的那份）的摘要；
- `events`、`n`：这一小时里新写的事件的文字（一行一条）的摘要和条数。

第 0 行是开局设定之后、第一次行动之前。两样都不读存档，存档结构怎么变都不影响。`eval golden` 不带 `--bless` 时逐个比对，打印每个玩家第一处不同的小时和是哪一样不同，有不同就以非零状态退出。

环境变量 `BOT_NORESEARCH` 会改机器人的打法，跑对照时不要设。

### 重记过的

- 阶段 3 S5（亲手派遣、部门）：局面数据多了 `departments` 一块，六条轨迹的 `view` 摘要从第 0 小时起都变了，重记。去掉这一块再比，六条轨迹逐小时相同（`view` 和 `events` 都相同）；greedy 3 个种子 72 小时 `compare --exact` 也相同。所以 S5 没改正式局的玩法。另外修了一处存档前后不一致：路线停在同一面墙前时复用停下原因的战报编号，原来只在战报还在内存里时复用（战报不存档，读档以后会另取一个编号），现在一律复用，内存里没有就按原编号放回去。

- 阶段 3 S6（越占越贵）：改正式局玩法，六条轨迹重记。战斗对照没变（`tests/golden.rs` 的 `held_tile` 占格以后把废材补回 10000：库存算进威胁，不补回来守城战会变）。

- 阶段 3 S4 和 S5、S6 合并（2026-11-28）：`SAVE_VERSION` 9；涨价改成只在教学关（`balance.json` 的 `costs` 是中性的，随机地图占格不要钱、建筑倍数 1），所以随机地图又回到 S6 以前的玩法，再加上 S4 的血量。六条轨迹重记，战斗对照没变、没有重记。greedy 3 个种子 72 小时：占地 74/70/73（S6 以前 74/67/74），成果 966/920/918。

## 读数

```bash
cargo run -q --release --example eval -- run greedy --seeds 3 --hours 72 --out /tmp/x.json
cargo run -q --release --example eval -- compare eval/baseline-pre-refactor/greedy.json /tmp/x.json --exact
```

六个玩家都这样跑一遍。`--exact`：每个指标的每个种子、每个命令的次数都相等才算过，否则列出不同的 key 并以非零状态退出。

2026-11-28 在 `760611e` 上重跑了 greedy，和存着的 `baseline-pre-refactor/greedy.json` 逐字节相同（连跑两次也相同），所以六份基线没有重记。

## 会话重放

`final.json` 多了可选字段 `view_sha`（终局局面数据的摘要，见 [docs/eval-interface.md](../../../docs/eval-interface.md)）。重放时有这个字段就比。

```bash
cargo run -q --release --example eval -- session eval/sessions/2026-11-28-m1b/haiku-casual-s0
cargo run -q --release --example eval -- session eval/sessions/*/*/ --bless
```

`--bless` 只凭一个还对得上的指纹重记另一个：存档对得上就补 `view_sha`；`view_sha` 对得上而存档对不上（`SAVE_VERSION` 加一之后）就重记 `save_sha` 和 save.json。其余的不动并列出来。

2026-11-28 补 `view_sha` 的结果：21 个会话里 5 个存档指纹还对得上，已补上；16 个在这之前就已经过期（是在更早的版本上玩的），原样没动。

| 补上了 | |
|---|---|
| `2026-11-28-m1/` | `haiku-casual-s0`、`haiku-conqueror-s2` |
| `2026-11-28-m1b/` | `haiku-casual-s0`、`haiku-conqueror-s2`、`haiku-planner-s1` |

| 没动 | 原因 |
|---|---|
| `2026-11-27/haiku-collector-s0` | 重放出的存档和 `save_sha` 不同 |
| `2026-11-27/haiku-conqueror-s0`、`sonnet-planner-s0` | 重放时有命令被拒绝 |
| `2026-11-28-m1/haiku-planner-s1` | 重放出的存档和 `save_sha` 不同 |
| `2026-11-28/haiku-casual-s0`、`haiku-conqueror-s0`、`haiku-planner-s0` | 重放出的存档和 `save_sha` 不同 |
| `2026-11-28/` 其余 9 个 | 重放时有命令被拒绝 |

所以重构期间会话重放这一项能用的是上面 5 个。**阶段 3 S4 起这 5 个也过期了**（S4 之前录的，见上面“阶段 3 S4 重记”），重放只读体感和打分。

**这 5 个都是 S4 以前的录像**（阶段 3 S4 起血量保留，S5 起局面数据多了 `departments`，合并后 `SAVE_VERSION` 9），现在重放全部报 stale（`eval session` 只给警告、只留 `llm.feel.*` 和 `llm.score.*`，不算失败），录像原样留着，不重记。`cargo test` 里没有重放这几个目录的测试（`tests/session.rs` 只用当场生成的会话），所以不用改测试。

## 文字快照

```bash
cargo run -q --release --example text_snapshot -- /tmp/snap.txt
sha256sum /tmp/snap.txt     # 应为 36a83137956d11e21b14df75a16c23d440a892bf010dd3e956ae772739a42db9（阶段 3 S4～S6 合并后重新生成）
xz -dc eval/golden/text_snapshot.txt.xz | diff - /tmp/snap.txt | head     # 不同时看是哪一段
```

输出是确定的（连跑两次逐字节相同），但有 45 MB、13173 行，所以存的是 `xz -9` 压过的（370 KB）。阶段 3 S4～S6 合并后重新生成过：S4 加了血量、医疗、用药线，S5 加了“[部门]”和派遣菜单；S6 的涨价现在只在教学关，随机地图又能占满，所以快照回到 S4 的大小。先比 sha256，不同再解压找是哪一段（每段以 `## ` 开头，写着种子和小时）。

## 用时

记录时的机器是 4 核。重构每一步之后 `eval run greedy --seeds 3 --hours 72` 不超过基线的 1.5 倍，超了先查原因。

| 命令 | 用时 |
|---|---|
| `cargo run -q --release --example eval -- run greedy --seeds 3 --hours 72 --out /tmp/x.json` | 115 秒、121 秒（连跑两次，已编译好） |
| `cargo run -q --release --example eval -- golden` | 9 秒 |
| `cargo run -q --release --example text_snapshot -- <文件>` | 约 165 秒 |
| `cargo test`（调试构建，已编译好） | 约 25 秒，其中战斗对照 0.4 秒 |

1.5 倍的线是 180 秒（按两次里慢的那次算）。换机器要先在 `760611e` 上重新量一次基线。
