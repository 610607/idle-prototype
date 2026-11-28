# 解锁登记表

[阶段 3](design-v2-stage3.md) 的 S3。游戏里的每个系统挂在一个解锁 id 上：没解锁时它的命令被拒绝，局面数据里没有它的块，文字版菜单和网页都不显示它。随机地图的局开局全部解锁，所以正式局的行为、局面数据和存档都和没有这套机制时逐字节相同。

## 文件

`engine/content/unlocks.json`，数据层的类型在 `src/data/unlocks.rs`，名字和说明在 `locales/<语言>/content.json`（`unlock.<id>.name`、`unlock.<id>.hint`，各一句话，不写数字）。

```json
{
  "id": "hire",
  "gates": ["hire", "squad"],
  "facilities": [],
  "rules": [],
  "when": { "own_former": { "former": "housing", "n": 1 } },
  "after": "build"
}
```

| 字段 | 意思 |
|---|---|
| `id` | 解锁 id。也是内容 id：机制层不许写死，要按名字认就登记到 `data/hooks.rs` |
| `gates` | 它开放的命令组和局面数据块（`data::unlocks::System`，封闭的枚举）。每个命令组正好由一个解锁管，校验会拦 |
| `facilities` | 它开放的建筑：设施 id，或 `housing`（住房）。建一座没开放的建筑被拒绝，局面数据的建筑清单里也没有它。每种建筑最多在一个解锁里；不归任何解锁的写进文件顶层的 `ungated_facilities` |
| `rules` | 它开放的规章 id。每条规章正好在一个解锁里 |
| `when` | 开放条件，见下 |
| `after` | 排在哪个解锁之后：那个开了才检查这一条。必须是表里更靠前的一条 |

### 条件（封闭的清单）

| 写法 | 什么时候算满足 | 进度 |
|---|---|---|
| `"always"` | 随时（只等 `after`） | 1/1 |
| `"never"` | 只能被授予（`mech::unlocks::grant`：以后的部门、正式局的壳） | 不显示 |
| `{"own_tile": {"tile": 112}}` | 这一格（棋盘下标）是自己的 | 0/1 |
| `{"own_tiles": {"n": 3}}` | 自己的格子不少于 n（总部不算） | 有几格/n |
| `{"built": {"facility": "clinic", "n": 1}}` | 自己的地上这种建筑不少于 n 座（`housing` 是住房） | 有几座/n |
| `{"own_former": {"former": "housing", "n": 1}}` | 自己的格子里“曾经的用途”（`Tile::former`，固定地图才有）是它的不少于 n 格（设施 id，或 `housing` 民居）；`n` 不写是 1 | 有几格/n |
| `{"done": {"action": "dispatch", "n": 5}}` | 这一轮做过这件事不少于 n 次 | 几次/n |
| `{"won_tile": {"tile": 112}}` | 在这一格打赢过一场路线战斗 | 0/1 |
| `{"department": {"id": "…"}}` | 这个部门成立了（S5 加部门；现在没有东西会成立部门） | 0/1 |

`done` 能数的事（`data::unlocks::Action`）：`dispatch`（亲手派遣；S5 之前是玩家设的一格路线）、`route`（玩家设的多格路线）、`claim`、`build`、`hire`（玩家的命令，规章代做的不算）、`research`、`surge_held`、`route_win`（后三样读这一轮已有的统计）。

加一种条件：数据层的 `Condition` 加一个变体，`mech/unlocks.rs` 的 `progress` 里算进度，`client/view.rs` 的 `cond_text` 和 `locales/` 里的 `unlock.cond.*` 写它的说法。

### 开局档案

文件顶层的 `profiles` 决定一局开局开哪些：

| 档案 | 内容 |
|---|---|
| `default` | `"all": true`：全部开放。随机地图的局（`Engine::new`、`sim::new_game`）用它，这时不存任何解锁状态 |
| `tutorial` | 只开 `dispatch`、`claim`（第一章 0:00 只能派人打隔壁一格、占领），其余按条件开。教学关地图用它 |

固定地图在 `maps.json` 里用 `"unlocks": "<档案 id>"` 写开局用哪个档案，不写是 `default`；`sim::start_map`（`sim::new_map_game`、`Engine::new_map`、文字版 `--map`、网页 `?map=`）按它设解锁状态。校验要求档案存在。

档案还可以有 `when`：`{解锁 id: 条件}`，这个档案里换掉登记表的条件（给 S7 正式局的壳用：那里第二章以后的系统写 `never`）。

## 第一版的登记表

条件按教学关第一章写；第一章以后的还没编排，都是 `never`，S7 编排时再填。

| 解锁 | 管的命令组 | 建筑、规章 | 条件（教学关） |
|---|---|---|---|
| `dispatch` | 派一格（`set_route` 一格，S5 换成手动派遣） | | 开局就开 |
| `claim` | 占领 | | 开局就开 |
| `build` | 建造 | 废材站、诊所、住房 | 占下 3 格 |
| `hire` | 招募和辞退、编队（`set_squad`） | | 占下 1 块曾是民居的地，在 `build` 之后（教学关里占下的城区格都自动是住房，所以不用“建住房”当条件） |
| `healing` | 治疗（`heal`，命令组 `medicine`）；队员卡上的治疗预览 | | 建 1 座诊所，在 `build` 之后 |
| `med_line` | 用药线（`set_med_line`，命令组 `med_line`；`set_rule auto_medicate` 也归这组） | 规章“用药线” | 建 1 座诊所，在 `healing` 之后 |
| `routes` | 多格路线、硬闯 | | never（S5：调度部门） |
| `rules` | 规章 | 三条规章 | never |
| `research` | 研究、换一批、选学说、知识库 | 档案室 | never |
| `equipment` | 装备 | | never |
| `gambits` | 战术规则 | | never |
| `industry` | | 工坊、发电站、市场、训练场 | never |
| `policy` | 政策 | | never |
| `customize` | 定制招募 | | never |
| `priority` | 岗位顺序 | | never |
| `garrison` | 驻扎 | 哨所、路障、瞭望塔、路灯、接地网 | never |
| `counter` | 反击、放弃 | | never |
| `dial` | 异界调节 | | never |
| `arsenal` | 军械 | | never |
| `tasks` | 见习任务 | | never |
| `retire` | 退役、锁定、封存 | | never |
| `collapse` | 收束 | | never |

选地图（`choose_map`）不归任何命令组，永远能用。

## 代码

- **状态**：`Cycle::unlocks: Option<mech::unlocks::Unlocks>`。`None` 就是全部开放，不写进存档，所以随机地图的局存档和以前一样，`SAVE_VERSION` 不用动。有状态时存：档案 id、已开的 id、`done` 的计数、打赢过的格子、成立的部门。
- **开局**：`sim::new_game_with(seed, now, device, &Start)`、`Engine::new_with(…)`；`Start::default()` 是 `default` 档案，`Start::tutorial()` 是教学关的档案。固定地图开局时用地图的 `unlocks` 档案（见上）。新开一轮（收束以后）是全部开放，S7 再定。
- **命令**：`Command::system()` 是一个没有兜底分支的 `match`，新命令不归类编译不过。`commands::apply` 先查 `mech::unlocks::check`（命令组，加上 `build` 的建筑、`set_rule` 的规章），没开就拒绝，错误文字是 `err.locked`，写着差哪个解锁。
- **检查时机**：命令被接受以后（`after_command`：先记数，再查条件），时间线每个时刻处理完以后（`sim::process_instant` 的最后），按登记表的顺序查一遍，一遍就够（`after` 只指向前面的）。新开的解锁写事件 `event.unlocked`（`EventKind::Unlock`，在 `engine/eval/payoff_classes.json` 归为 `unlock`）。全部开放时这些函数直接返回，不花时间。
- **局面数据**：没开的系统的块不出现（`view::Gated`：开着时序列化和原来的值一模一样，关着时整个键不写；Rust 这边读到的是空值）。现在挂上的块：`research`、`armory`、`tasks`、`facilities`、`candidates`、`inventory`、`sealed`、`org.rules`、`med_line`、街区的 `policy_options` 和 `customize`，队员血量里的治疗预览 `health.heal`（血量本身不挂解锁，一直显示）；待你决定里属于没开系统的条目也去掉。另有两个新字段，全部开放时都不写：`locked`（还锁着的命令组）和 `next`（最近的一两个没开、`after` 已满足、不是 `never` 的解锁，带条件的说法、进度、目标和一句说明）。
- **文字版**：菜单只列开着的命令（`Engine::allows`），锁着的块不打印，命令参考只列开着的命令，局面最后打印“接下来能解锁”。
- **网页**：锁着的块不画（研究页签、候选人、仓库、封存、规章、军械、见习任务、建造），“接下来能解锁”固定在地图下面、待你决定上面。按钮层面的细节（驻扎、反击等按钮）留给 S9。

## 新东西怎么登记

- 新命令：在 `Command::system()` 里归到一个命令组；是新的命令组就在 `System` 加一项，并在 `unlocks.json` 里让一个解锁管它（校验要求每个命令组正好一个）。
- 新建筑：写进某个解锁的 `facilities`，或者 `ungated_facilities`（`textui` 的测试拦漏登记的）。
- 新规章：写进某个解锁的 `rules`。
- 新系统要成块出现在局面数据里：块用 `Gated` 包上，文字版和网页按 `locked` 隐藏。
