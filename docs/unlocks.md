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
| `{"low_hp": {"pct": 50}}` | 花名册上有人倒下了，或者血量低于生命上限的 pct% | 0/1 |
| `{"stopped_at": {"tile": "H7"}}` | 某支小队上一趟停在这一格前面（那一仗不是稳赢） | 0/1 |

写格子的地方（`own_tile`、`won_tile`、`stopped_at`，以及编排里的 `tile`）可以写棋盘下标，也可以写格子的标签（`"H7"`：列字母、行号），读进来都是下标。

`done` 能数的事（`data::unlocks::Action`）：`dispatch`（亲手派遣，`Command::Dispatch`）、`route`（玩家设的路线，`set_route` 不空）、`claim`、`build`、`hire`、`heal`（玩家的命令，规章代做的不算，用药线治的也不算）、`research`、`surge_held`、`route_win`（后三样读这一轮已有的统计）。前六样的计数在 `Cycle::done`，每一轮清零；有没有解锁状态都记（部门的条件要读）。`won_tile` 只在有解锁状态的局里记。

加一种条件：数据层的 `Condition` 加一个变体，`mech/unlocks.rs` 的 `progress` 里算进度，`client/view.rs` 的 `cond_text` 和 `locales/` 里的 `unlock.cond.*` 写它的说法。

### 开局档案

文件顶层的 `profiles` 决定一局开局开哪些：

| 档案 | 内容 |
|---|---|
| `default` | `"all": true`：全部开放。随机地图的局（`Engine::new`、`sim::new_game`）用它，这时不存任何解锁状态 |
| `tutorial` | 只开 `dispatch`、`claim`（第一章 0:00 只能派人打隔壁一格、占领），其余按条件开；`when` 换掉两条：治疗是“占下曾是诊所的地”，用药线是“亲手治疗 3 次”。教学关地图用它 |

固定地图在 `maps.json` 里用 `"unlocks": "<档案 id>"` 写开局用哪个档案，不写是 `default`；`sim::start_map`（`sim::new_map_game`、`Engine::new_map`、文字版 `--map`、网页 `?map=`）按它设解锁状态。校验要求档案存在。

档案还可以有 `when`：`{解锁 id: 条件}`，这个档案里换掉登记表的条件（给 S7 正式局的壳用：那里第二章以后的系统写 `never`）。

## 第一版的登记表

条件按教学关第一章写；第一章以后的还没编排，都是 `never`，S7 编排时再填。

| 解锁 | 管的命令组 | 建筑、规章 | 条件（教学关） |
|---|---|---|---|
| `dispatch` | 亲手派遣（`dispatch`：打一格一次就回） | | 开局就开 |
| `claim` | 占领 | | 开局就开 |
| `build` | 建造 | 废材站、诊所、住房 | 占下 3 格 |
| `hire` | 招募和辞退、编队（`set_squad`） | | 占下 7 格，在 `build` 之后（和编排里送巷战突击手的条件一样，S7） |
| `healing` | 治疗（`heal`，命令组 `medicine`）；队员卡上的治疗预览 | | 建 1 座诊所；教学关档案里是“占下曾是诊所的地”（那块地占下就送一座诊所） |
| `med_line` | 用药线（`set_med_line`，命令组 `med_line`；`set_rule auto_medicate` 也归这组） | 规章“用药线” | 建 1 座诊所，在 `healing` 之后（医务部门也授予它）；教学关档案里是“亲手治疗 3 次” |
| `routes` | 路线（`set_route`，不论几格）、硬闯 | | 成立调度部门（调度部门也直接授予它） |
| `rules` | 规章 | 三条规章 | never |
| `research` | 研究、换一批、选学说、知识库 | 档案室 | never |
| `equipment` | 装备 | | never |
| `gambits` | 战术规则 | | never |
| `industry` | 局面块“电力”（`power`） | 工坊、发电站、市场、训练场 | never |
| `policy` | 政策 | | never |
| `customize` | 定制招募 | | never |
| `priority` | 岗位顺序 | | never |
| `garrison` | 驻扎 | 哨所、路障、瞭望塔、路灯、接地网 | never |
| `counter` | 反击、放弃 | | never |
| `dial` | 异界调节 | | never |
| `arsenal` | 军械 | | never |
| `tasks` | 见习任务 | | never |
| `retire` | 退役、锁定、封存 | | never |
| `threat` | 局面块“威胁”和来潮预报（`threat`：局面数据的 `threat`、`tide`） | | never（第三章） |
| `collapse` | 收束；局面块“本轮成果”（`score`）、“组织等级”（`org_level`：局面数据的 `org.perks`，文字版的“组织：Lv.”一行） | | never |

选地图（`choose_map`）和成立部门（`found_department`）不归任何命令组，永远能用；成立部门由每个部门自己的 `shown_after` 管。

### 只是局面块的命令组

S7 起 `System` 里有四项不管任何命令，只管局面数据里的一块：`threat`（威胁读数、来潮预报）、`score`（本轮成果）、`power`（电力）、`org_level`（组织等级和各级加成）。没开时这几块整个不写进局面数据（`Gated`），文字版不打印那几行，网页不画；“领地”一行在没有威胁块时只写有几格。仓库上限没有自己的解锁：有解锁状态的局里，库存到一半才显示上限那一行。

## 编排好的赠送和提示（固定地图，S7）

固定地图在 `maps.json` 里可以写 `script`：一串条目，按顺序检查，每条只触发一次。数据层的类型是 `data::maps::ScriptEntry` 和 `Gift`，机制在 `mech/script.rs`。

```json
{
  "id": "scrapyard",
  "when": { "own_tile": { "tile": "I10" } },
  "after": "first_claim",
  "give": { "tile": "I10", "facility": "scrapyard", "staff": 6 },
  "hint": true
}
```

| 字段 | 意思 |
|---|---|
| `id` | 条目 id（只在这张地图里唯一）。提示文字的 key 由它拼出来：`event.hint.<地图>.<id>`，在 `locales/<语言>/events.json`，一两句、不写数字；口气按世界观定稿（`design-v2-worldview.md`，在主干上）的多声部：街坊、诊所、废材站、招募用市井气，打仗、帮派、花费用冷嘲的台账口气，操作说明直白另起一句；自称“公所 / 我们”，对玩家说“你们” |
| `when` | 条件，和解锁同一张清单（不能是 `never`） |
| `after` | 前面某个条目触发过才检查这一条（可省） |
| `give` | 送什么，可以几样一起（都可省）：`resources`（资源，进仓库，受上限）；`tile` + `facility` + `staff`（在这一格放一座建好的建筑，不花钱，自带 `staff` 名工人：他们先占这座建筑的岗位，不算街区居民、不要住房，存在 `Tile::staff`）；`residents`（`tile` 所在街区加这么多居民）；`candidate`：`{"class": "striker", "free": true, "joins": true}`（一名这个职业的候选人进招募列表，稀有度最高 R；`free` 招募不要钱，`joins` 招下来直接编进第一支有空位的小队）；`unlock`（解锁 id 的列表，直接授予） |
| `hint` | `true`：触发时说那句提示 |

- **状态**：`Cycle::script`（`mech::script::Script`）：触发过的条目 id（按触发顺序）、现在显示的提示是哪一条、什么时候出的。随机地图没有这块，不写进存档。`SAVE_VERSION` 10。
- **检查时机**：开局（`sim::start_map` 的最后，所以 `"always"` 的条目在第 0 秒就出）、每条命令被接受以后（`commands::apply`，在解锁检查之后）、时间线每个时刻处理完以后（`sim::process_instant` 的最后）。一次检查里一条触发以后从头再查，直到没有新的；每触发一条也重查一遍解锁（送的诊所会开放治疗）。预报（`tide_log`）里不触发。
- **提示**：触发时写一条事件（`EventKind::Hint`，key 是上面那个，爽点表里归为 `hint`：送东西的那几条算小爽点，其余是涓流），并成为“现在的提示”。局面数据的 `hint`（`view::HintView`：文字和时刻）是最新的一条，一直显示到下一条出来；文字版在局面最上面打印“[提示] …”，网页在地图上面一行。提示不挡任何操作。
- **地图上的其他开关**（`MapDef`）：`talent: false` 街区不攒人才（候选人只来自编排）；`pass_claimable: true` 路线穿过“可占领但还没占”的格子不再打，亲手派遣去这种格子被拒绝（`err.dispatch_claimable`）；`offline_cap_secs` 一次追赶最多算多少秒，再长的部分当作暂停（路上的小队、倒下的读条都顺延，按整小时跳，结算的格子不错位）；`med_reserve` 开局把用药线的保留线设成这个数；节奏里的 `rest_bp`：在家（或路线这一趟没打仗）的队员每过节奏上的一小时回这么多血（万分比），兜底用。
- **加一种送的东西**：`Gift` 加字段，`MapDef::check` 里查引用，`mech::script::fire` 里送。

教学关第一章的编排（`maps.json` 的 `tutorial`）：

| 条目 | 条件 | 送什么 | 提示（大意） |
|---|---|---|---|
| `opening` | 一开始 | | 隔壁街口有东西想收过路费；东边那块以前是废材站 |
| `first_win` | 打赢 1 场 | | 清干净了，占下它 |
| `first_claim` | 占下 1 格 | | 每多占一格都更贵 |
| `scrapyard` | 占下 I10 | I10 一座废材站，自带 6 人 | 架子还在，老工人回来了 |
| `clinic` | 占下 G10 | G10 一座诊所，自带 4 人 | 诊所重新开门，伤能治了 |
| `low_hp` | 有人血量低于 50% | | 伤不会自己好，小伤便宜重伤贵 |
| `healed` | 亲手治疗 1 次 | | 别等快倒下才治 |
| `med_line` | 亲手治疗 3 次（在 `healed` 之后） | | 设一条用药线 |
| `residents` | 占下 F8 | F8 所在街区 12 名居民 | 人一直没走，可以再开一座废材站 |
| `striker` | 占下 7 格 | 一名巷战突击手，不要钱，招下就进甲队 | 有人找上门来 |
| `dispatch_office` | 亲手派遣 5 次 | | 该有人专门管这件事了 |
| `routes` | 调度部门成立 | | 画一条路线；往北那条通到变电所门口 |
| `gate_stop` | 路线停在 H7 前 | | 变电所里蹲着一伙帮派和看门兽；照现在这样打不过，多一个人会不一样 |
| `gate_won` | 打赢 H7 | | 窝点打服了，北、东、西的路都通了 |

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
| 医务 `medical_office` | 有 1 座诊所 | 不花钱 | `med_line`（用药线；教学关里建诊所就已经开了） | `rules` 开了以后 |
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
- **局面数据**：没开的系统的块不出现（`view::Gated`：开着时序列化和原来的值一模一样，关着时整个键不写；Rust 这边读到的是空值）。现在挂上的块：`research`、`armory`、`tasks`、`facilities`、`candidates`、`inventory`、`sealed`、`org.rules`、`med_line`、街区的 `policy_options` 和 `customize`，队员血量里的治疗预览 `health.heal`（血量本身不挂解锁，一直显示），S7 起还有 `threat`、`tide`、`results`、`power`、`org.perks`；待你决定里属于没开系统的条目也去掉。另有两个新字段，全部开放时都不写：`locked`（还锁着的命令组）和 `next`（最近的一两个没开、`after` 已满足、不是 `never` 的解锁，带条件的说法、进度、目标和一句说明）。
- **文字版**：菜单只列开着的命令（`Engine::allows`），锁着的块不打印，命令参考只列开着的命令，局面最后打印“接下来能解锁”。
- **网页**：锁着的块不画（研究页签、候选人、仓库、封存、规章、军械、见习任务、建造），“接下来能解锁”固定在地图下面、待你决定上面。按钮层面的细节（驻扎、反击等按钮）留给 S9。

## 新东西怎么登记

- 新命令：在 `Command::system()` 里归到一个命令组；是新的命令组就在 `System` 加一项，并在 `unlocks.json` 里让一个解锁管它（校验要求每个命令组正好一个）。
- 新建筑：写进某个解锁的 `facilities`，或者 `ungated_facilities`（`textui` 的测试拦漏登记的）。
- 新规章：写进某个解锁的 `rules`。
- 新系统要成块出现在局面数据里：块用 `Gated` 包上，文字版和网页按 `locked` 隐藏。
