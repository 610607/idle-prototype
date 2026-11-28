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
| `{"department": {"id": "…"}}` | 这个部门成立了（见下面“部门”） | 0/1 |

`done` 能数的事（`data::unlocks::Action`）：`dispatch`（亲手派遣，`Command::Dispatch`）、`route`（玩家设的路线，`set_route` 不空）、`claim`、`build`、`hire`（玩家的命令，规章代做的不算）、`research`、`surge_held`、`route_win`（后三样读这一轮已有的统计）。前五样的计数在 `Cycle::done`，每一轮清零；有没有解锁状态都记（部门的条件要读）。`won_tile` 只在有解锁状态的局里记。

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
| `dispatch` | 亲手派遣（`dispatch`：打一格一次就回） | | 开局就开 |
| `claim` | 占领 | | 开局就开 |
| `build` | 建造 | 废材站、诊所、住房 | 占下 3 格 |
| `hire` | 招募和辞退、编队（`set_squad`） | | 占下 1 块曾是民居的地，在 `build` 之后（教学关里占下的城区格都自动是住房，所以不用“建住房”当条件） |
| `routes` | 路线（`set_route`，不论几格）、硬闯 | | 成立调度部门（调度部门也直接授予它） |
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

选地图（`choose_map`）和成立部门（`found_department`）不归任何命令组，永远能用；成立部门由每个部门自己的 `shown_after` 管。

## 部门

`engine/content/departments.json`，数据层的类型是 `data::unlocks::DepartmentDef`，名字和说明在 `locales/<语言>/content.json`（`department.<id>.name`、`department.<id>.desc`，不写数字）。

```json
{
  "id": "dispatch_office",
  "when": { "done": { "action": "dispatch", "n": 5 } },
  "cost": { "scrap": 15 },
  "grants": ["routes"],
  "wired": true
}
```

| 字段 | 意思 |
|---|---|
| `id` | 部门 id（内容 id，机制层不写死） |
| `when` | 成立的条件，和解锁同一张条件清单（不能是 `never`） |
| `cost` | 成立花多少（资源，不写是不花钱） |
| `grants` | 成立以后开放的解锁 id；之后每一轮开局都直接开着 |
| `shown_after` | 这个解锁开了才列出这个部门（不写是一直列出）；教学关第一章只列调度部门 |
| `wired` | `false`：它管的自动化还没做，可以成立但 `grants` 必须为空（校验会拦） |

首批六个（`docs/design-v2-content.md` 的草案）：

| 部门 | 条件 | 花费 | 开放 | 列出的时机 |
|---|---|---|---|---|
| 调度 `dispatch_office` | 亲手派遣 5 次 | 废材 15 | `routes` | 一直 |
| 医务 `medical_office` | 有 1 座诊所 | 不花钱 | 还没接上 | `rules` 开了以后 |
| 研究 `research_office` | 研究 1 次 | 废材 30 | 还没接上 | `research` 开了以后 |
| 规划 `planning_office` | 建造 6 次 | 废材 40 | 还没接上 | `industry` 开了以后 |
| 人事 `personnel_office` | 招募 4 次 | 废材 40 | 还没接上 | `rules` 开了以后 |
| 守备 `defense_office` | 守住 3 次来潮 | 废材 40 | 还没接上 | `garrison` 开了以后 |

“有 6 座建筑”先用“建造 6 次”代替（条件清单里没有“建筑总数”）。

- **状态**：成立过的部门在 `Org::departments`（组织上，收束不清）。命令 `found_department` 检查：列出了、没成立过、条件满足、付得起；然后扣钱、记下、写事件 `event.department_founded`（`EventKind::Unlock`，爽点归 `unlock`），再逐个授予 `grants`（`mech::unlocks::grant`，有解锁状态时各写一条 `event.unlocked`）。
- **开局**：`sim::start_map`、`sim::choose_map` 设好解锁状态以后调 `mech::unlocks::seat_departments`：每个成立过的部门的 `grants` 直接加进已开的集合，不写事件，不花钱。
- **全部开放的局**（随机地图）：部门照样列出、照样能成立（条件读 `Cycle::done` 和这一轮的统计），但没有解锁状态，授予什么都不改变，所以机器人能做的事不变。
- **局面数据**：`departments`（`view::DepartmentView`：名字、说明、成立没有、条件够没有、现在能不能成立、条件的说法、进度、花费、接上没有）。文字版在局面里打印“[部门]”，能成立的进菜单；网页在地图下面“接下来能解锁”之后一块。

## 代码

- **状态**：`Cycle::unlocks: Option<mech::unlocks::Unlocks>`。`None` 就是全部开放，不写进存档。有状态时存：档案 id、已开的 id、打赢过的格子。`done` 的计数在 `Cycle::done`，成立的部门在 `Org::departments`，两样不管有没有解锁状态都存（阶段 3 S5 起，`SAVE_VERSION` 8）。
- **开局**：`sim::new_game_with(seed, now, device, &Start)`、`Engine::new_with(…)`；`Start::default()` 是 `default` 档案，`Start::tutorial()` 是教学关的档案。固定地图开局时用地图的 `unlocks` 档案（见上）。新开一轮（收束以后）是全部开放，S7 再定。
- **命令**：`Command::system()` 是一个没有兜底分支的 `match`，新命令不归类编译不过。`commands::apply` 先查 `mech::unlocks::check`（命令组，加上 `build` 的建筑、`set_rule` 的规章），没开就拒绝，错误文字是 `err.locked`，写着差哪个解锁。
- **检查时机**：命令被接受以后（`after_command`：先记数——这一步每局都做——再查条件），时间线每个时刻处理完以后（`sim::process_instant` 的最后），按登记表的顺序查一遍，一遍就够（`after` 只指向前面的）。新开的解锁写事件 `event.unlocked`（`EventKind::Unlock`，在 `engine/eval/payoff_classes.json` 归为 `unlock`）。全部开放时这些函数直接返回，不花时间。
- **局面数据**：没开的系统的块不出现（`view::Gated`：开着时序列化和原来的值一模一样，关着时整个键不写；Rust 这边读到的是空值）。现在挂上的块：`research`、`armory`、`tasks`、`facilities`、`candidates`、`inventory`、`sealed`、`org.rules`、街区的 `policy_options` 和 `customize`；待你决定里属于没开系统的条目也去掉。另有两个新字段，全部开放时都不写：`locked`（还锁着的命令组）和 `next`（最近的一两个没开、`after` 已满足、不是 `never` 的解锁，带条件的说法、进度、目标和一句说明）。
- **文字版**：菜单只列开着的命令（`Engine::allows`），锁着的块不打印，命令参考只列开着的命令，局面最后打印“接下来能解锁”。
- **网页**：锁着的块不画（研究页签、候选人、仓库、封存、规章、军械、见习任务、建造），“接下来能解锁”固定在地图下面、待你决定上面。按钮层面的细节（驻扎、反击等按钮）留给 S9。

## 新东西怎么登记

- 新命令：在 `Command::system()` 里归到一个命令组；是新的命令组就在 `System` 加一项，并在 `unlocks.json` 里让一个解锁管它（校验要求每个命令组正好一个）。
- 新建筑：写进某个解锁的 `facilities`，或者 `ungated_facilities`（`textui` 的测试拦漏登记的）。
- 新规章：写进某个解锁的 `rules`。
- 新系统要成块出现在局面数据里：块用 `Gated` 包上，文字版和网页按 `locked` 隐藏。
