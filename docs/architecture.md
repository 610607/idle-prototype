# 引擎分层

> 设计 v2 要把引擎搬到一套新的类型上（实体、容器、效果句式），见 [design-v2-engine-model.md](design-v2-engine-model.md) 和 [design-v2-refactor.md](design-v2-refactor.md)。本文写的是**现在的**代码；重构每做完一步，对应的小节跟着改。

规则引擎（`engine/src/`）分五层。每层只能用排在它前面的层，`engine/tests/layers.rs` 会拦往上引用的代码（测试模块除外）：

```
本地化 l10n → 引擎 kernel → 数据 data → 机制 mech → 客户端 client
```

| 层 | 目录 | 放什么 | 什么时候改 |
|---|---|---|---|
| 本地化 | `src/l10n/`、`locales/<语言>/` | 文字目录，`t!` / `txt!`，存档里的文字（key + 参数） | 改措辞、加语言 |
| 引擎 | `src/kernel/` | 棋盘几何、种子随机、离散事件时钟、存档信封、哈希 | 几乎不改：它不认识任何游戏规则 |
| 数据 | `src/data/`、`content/*.json` | 内容定义和它们的效果词汇（`Effect`、`TraitEffect` 等枚举）、数值表 `balance.json`、加载和交叉校验 | 加词条（只改 JSON）；加效果种类（改枚举，再在机制层读它） |
| 机制 | `src/mech/` | 游戏状态和全部规则：战斗、经济、防守、路线、研究、一轮的结构、规章、玩家操作 | 加机制、改规则 |
| 客户端 | `src/client/` | `Engine` 门面（命令、预演、视图）、文字版、wasm 绑定 | 加了要给玩家看或预演的东西 |

网页（`web/`）和评估工具（`engine/evalkit/`）在引擎外面，只通过客户端层和公开的类型用引擎。

## 各层的边界

**引擎层**只提供机制层要接的两个口子：

- `kernel::clock::Timeline`：时钟问“下一件事什么时候发生”“这一刻有没有事”“这一刻发生什么”，循环本身（`clock::advance`）不变。离线一次跳过和在线逐秒推进走同一条路径。机制层在 `mech/sim.rs` 里为 `GameState` 实现它。
- `kernel::save::Saved`：存档信封（格式、版本、修订号、校验和）不管状态里装什么，机制层为 `GameState` 实现它，版本号是 `mech::state::SAVE_VERSION`。

引擎层可以用本地化层的 `t!`，因为存档读不了要给玩家报错。

**数据层**定义“有什么”，机制层决定“怎么算”。内容 JSON 只放 id、数字和效果描述（`{"type": "stat", "atk": 4}`），文字放 `locales/`，图标按 id 引用。效果种类是数据层里的带标签枚举，JSON 里写了没有的种类，加载时就报错。

**本地化层**最底层，只按 key 查文字，不认识游戏。内容加载时会把语言锁住（`l10n::freeze`）：内容名字按当时的语言填进去，之后不能再换。

## 加一个新词条（特质、装备、学说、敌人、设施……）

效果用现有的效果种类就能表达时，只动数据层和本地化层：

1. 在 `engine/content/<类别>.json` 里加一项：id、数字、效果、图标 id。
2. 在 `locales/<语言>/content.json` 里加它的名字和说明。
3. 图标：用已有的，或按 `icons/README.md` 新画一个。
4. 战斗词条（特质、装备、学说）测价：`cargo run --release --example eval -- price --bless`，把 `engine/eval/prices.json` 一起提交（`eval_prices` 测试会拦）。
5. `cargo test`：内容交叉校验、文字齐全、价格齐全。

不用改任何 Rust 代码。

## 加一个新效果种类

现有效果种类表达不了的词条，需要新机制：

1. 数据层：在对应枚举（`data::Effect`、`TraitEffect`、`ReactionEffect` 等）加一个变体，写清楚字段的单位（数值用整数，百分比用万分比）。
2. 机制层：在读这个枚举的地方处理它（战斗效果在 `mech/combat.rs`，经营效果在 `mech/economy.rs`……）。
3. 再按上一节加词条。

## 加一个新机制

1. 机制层新开一个文件（`mech/<机制>.rs`），状态加进 `mech/state.rs`；存档结构变了就把 `SAVE_VERSION` 加一（快速迭代期不写迁移）。
2. 有定时发生的事：在 `mech/sim.rs` 的 `Timeline` 实现里加上它的下一次时间、“这一刻有没有事”的判断和这一刻要做的事。时钟循环不用改。
3. 有玩家操作：在 `mech/commands.rs` 的 `Command` 加一项，规则写在机制层。
4. 有可调的数：放进 `content/balance.json`；有新的定义表：放进 `content/` 和数据层。
5. 客户端：视图（`client/view.rs`）、预演（`client/api.rs`）、文字版（`client/textui.rs`，新命令不接上编译不过）、网页。
6. 评估：按 `docs/balance-eval.md`“新系统怎么接入”补指标、机器人规则、合格区间和模型系数；新事件 key 在 `engine/eval/payoff_classes.json` 归类。

## 钩子：代码按名字认的内容

少数内容 id 背后有专门的代码，比如民兵这个职业决定街区的守城力量、规章 `auto_claim` 是一段代码。这类 id 集中登记在 `data::hooks`（`src/data/hooks.rs`），机制层只通过那里引用它们，`engine/tests/layers.rs` 会拦机制层里直接写死的内容 id。改名或删掉这些内容时，校验会报出哪段代码还在用它。
