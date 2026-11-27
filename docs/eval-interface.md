# 评估接口：试玩协议和结果格式

评估要能反哺设计，前提是三件事：谁来玩都走同一个接口，结果都落成同一种格式，游戏改了以后工具不悄悄失效。这份文档是这三件事的约定。数值指标和合格区间见 `docs/balance-eval.md`，数学模型见 `docs/balance-model.md`，LLM 试玩的历次结果见 `docs/llm-playtest.md`。

## 目标

- **玩家可替换**：脚本机器人、Claude 子代理、Gemini 或其他模型，甚至真人，都通过同一个“会话”玩，产出同一种会话目录，用同一套指标评估。接一个新模型只需要让它所在的 harness 调用会话命令，不改游戏、不改评估。
- **贵的东西只做一次**：LLM 试玩只记录“什么时刻发了什么命令”和玩家的反馈。指标在事后重放计算，评估工具改进后，旧的试玩可以重新算分。
- **游戏改了能看出来**：原型阶段规则常改。会话记录游戏版本和终局存档的指纹，重放结果对不上就判为过期，只保留玩家反馈，不拿它算指标。工具本身的失效要在 `cargo test` 里暴露，而不是等到试玩时才发现。

## 结构

```
玩家（机器人 / Claude 子代理 / 其他模型的 harness / 真人）
   │ 观察 JSON ↓   ↑ 行动 JSON
会话（textui::session，agent_cli session …）：按日程推进、给观察、收行动、阶段末收打分、写会话目录
   │ 会话目录
评估（eval session DIR…）：重放命令日志 → 和 eval run 同名的指标 + llm.* 反馈指标 → RunFile → check / compare
```

两种驱动方式，底下是同一个会话实现：

- **命令行**（玩家自己调命令，适合 Claude 子代理、Gemini CLI 这类会用终端的代理）：`agent_cli session new / obs / act`。
- **标准输入输出**（外部程序驱动）：`agent_cli session serve DIR` 每次输出一行观察 JSON，读入一行行动 JSON，直到结束。

模型怎么接由各自的 harness 负责（提示词、调用、重试），本仓库不放模型适配器；仓库只保证这两种入口和下面的格式稳定。

## 日程

回访时刻由会话决定，玩家不能自己选等多久。这样不同模型的结果可比，也避免上一次 Haiku 试玩那样自己改了节奏。

- `week-decay`（默认，168 小时）：第 1 天每 3 小时（0、3、…、21），第 2～3 天每 4 小时（24、28、…、68），第 4～7 天每 8 小时（72、80、…、160），第 168 小时结束。模拟越来越少上线的玩家。
- `every:N`：每 N 小时一次。
- `--hours H` 改总时长。

阶段：`d1` = [0, 24)，`d2_3` = [24, 72)，`d4_7` = [72, 168]。每个阶段结束后的第一次回访要先打分（第 24、72 小时的回访和终局）；总时长不到 168 小时的，终局给最后一个阶段打分。

## 会话目录

```
DIR/
  manifest.json   会话设定，开局写一次
  save.json       当前存档（引擎的存档格式）
  journal.jsonl   每条被接受的命令：{"t": 秒, "cmd": {…}}，按顺序；包括开局设定和自动领取的见习任务
  visits.jsonl    每次回访一行（行动执行之后写）
  final.json      终局：{"t": 秒, "summary": {…}, "save_sha": "…"}
```

`t` 是从本轮开始算起的秒数（整数）。命令日志来自引擎的测量钩子 `Trace::Command`，所以不管命令来自菜单、JSON 还是自动领取，都会记下来。引擎是确定性的，而且一次推进多久不影响结果（一次推进 48 小时和每 5 分钟推进一次，存档逐字节相同），所以按 (t, 命令) 重放能得到同一局。

manifest.json：

```json
{
  "protocol": 1,
  "build": "3f9c0a12",
  "seed": 0,
  "player": "haiku",
  "schedule": "week-decay",
  "hours": 168,
  "lang": "zh-CN",
  "research": true
}
```

- `protocol`：本协议的版本。观察、行动或目录结构有不兼容的改动时加一，harness 看到不认识的版本应当报错退出。
- `build`：游戏版本指纹，即 `SAVE_VERSION` 和全部内容数据（含 `balance.json`）的哈希。只作参考：引擎代码改了它不一定变，真正的判据是重放后的存档指纹。
- `player`：谁在玩，评估结果里的人格名是 `llm:<player>`（机器人写的会话用机器人人格名）。
- `schedule`：回访日程（见“日程”）。机器人写的会话记为 `bot`：机器人按自己的人格节奏行动，没有回访，visits.jsonl 为空；save.json 是终局存档。这种目录只给 `eval session` 重放，不能用 `agent_cli session` 接着玩。

visits.jsonl 的一行：

```json
{
  "visit": 4, "t": 32400, "stage": "d1",
  "actions": [{"id": "a1b2c3", "cmd": {"cmd": "set_route", "squad": 4, "route": [97]}, "ok": true, "err": null}],
  "feel": {"label": "delight", "text": "废材终于有了产出"},
  "scores": null,
  "summary": {"owned": 3, "ring": 1, "score": 12}
}
```

## 观察

`agent_cli session obs DIR --json` 和 `serve` 输出的对象：

```json
{
  "protocol": 1,
  "visit": 4, "t": 32400, "hour": 9, "hours": 168, "stage": "d1", "next_hour": 12,
  "need": "act",
  "text": "== 第 9 小时 / 共 168 小时 … ==\n（和 agent_cli show 相同的局面文字，菜单项前面是 id）",
  "menu": [{"id": "a1b2c3", "label": "甲队 改走路线 H7 …", "cmd": {"cmd": "set_route", "squad": 4, "route": [97]}}],
  "results": ["上一次行动每一项的结果"],
  "feel_labels": [{"id": "delight", "text": "惊喜"}, {"id": "flat", "text": "平淡"}, {"id": "letdown", "text": "失望"}],
  "score_dims": []
}
```

- `need`：`act` 普通回访；`scores` 这次要先给上一个阶段打分（`score_dims` 列出维度）；`done` 已结束，不再接受行动。
- `menu[].id`：由命令内容算出的短哈希，同一局面下稳定。不用序号，因为序号会随局面变化，旧序号可能执行到别的命令；旧 id 只会被拒绝。
- `text` 用会话的语言（`lang`）渲染，玩家看到的文字全部来自 `locales/`。结构化字段（`menu[].cmd`、`summary`）给程序用。

## 行动

```json
{
  "pick": ["a1b2c3"],
  "commands": [{"cmd": "garrison", "op": 2, "tile": 97}],
  "feel": {"label": "delight", "text": "上次离开以后注意到的变化"},
  "scores": {"growth": {"score": 3, "why": "依据"}}
}
```

- 先按顺序执行 `pick`（必须是这次观察里的 id），再执行 `commands`（任意引擎命令，格式见 `agent_cli info` 的命令参考）。失败的项不中断，结果写进下一次观察的 `results` 和 visits.jsonl。
- `feel`：这次回访的感受，`label` 是 `delight` / `flat` / `letdown` 之一，`text` 一句话说注意到了什么。可以省略，记为空。
- `scores`：`need` 为 `scores` 时必须给齐全部维度，每项 1～5 的整数加一句依据；缺了就拒绝这次行动，局面不前进。
- 行动执行完，会话推进到下一个回访时刻，自动领取见习任务，再给出新的观察。

命令行写法：

```bash
agent_cli session new DIR --seed 0 [--schedule week-decay|every:N] [--hours 168] [--player NAME] [--no-research]
agent_cli session obs DIR [--json]
agent_cli session act DIR [--pick ID,ID] [--cmd '<命令 JSON>']… [--feel delight|flat|letdown] [--why '一句话'] [--score growth=3:'依据']… 
agent_cli session act DIR --action '<行动 JSON>'
agent_cli session serve DIR
```

## 打分维度

1～5 分，越高越好。id 固定，说明文字在 `locales/<语言>/cli.json`。

| id | 维度 | 说明 |
|---|---|---|
| `growth` | 成长感 | 觉得自己在变强 |
| `novelty` | 新鲜感 | 有新东西出现或可做 |
| `goal` | 目标感 | 知道下一步该追什么 |
| `return` | 回来有收获 | 每次上线都有东西在等你 |
| `decisions` | 决策趣味 | 有值得想的选择 |
| `automation` | 省心 | 重复操作有人代劳 |
| `flow` | 推进顺畅 | 没有被卡住、白等 |
| `clarity` | 看得懂 | 明白发生了什么、为什么 |

## 评估会话

```bash
cargo run --release --example eval -- session DIR… [--out RUN.json]
cargo run --release --example eval -- run greedy --seeds 8 --hours 168 --sessions OUTDIR   # 机器人也写会话目录：OUTDIR/<人格>-s<种子>/
```

`eval session` 对每个会话目录：读 manifest 和命令日志，从同一个种子开局，按时刻重放命令，期间和 `eval run` 一样每 5 分钟记一次测量，算出同名的全部指标（`owned.h*`、`reach_h.*`、`payoff.*`……）；再从 visits.jsonl 算 `llm.*` 指标。结果是一个 RunFile，人格名 `llm:<player>`（`player` 本身是机器人人格名时就用人格名，如 `greedy`），可以直接 `eval check`、`eval compare`（和同种子的 `greedy` 比），也可以和机器人的结果一起交给 `eval review`。

重放的细节（代码在 `engine/examples/eval/run.rs` 的 `measure` 和 `Replay`、`session.rs`）：

- 机器人和重放走同一个测量循环，只是行动的一方不同。两者开局相同：`Engine::new(seed, T0, "agent")`，再加 `textui::setup` 的四条设定命令（选第 1 张地图、开三条规章）。日志开头是这四条时，重放先执行它们，不算玩家第 1 小时发出的命令（和机器人一样）。
- 每 5 分钟一步。每条命令在自己的时刻执行：时刻落在一步中间，就先推进到那一刻、执行、再推进到这一步结束；同一秒的几条命令之间不推进时间（`sim::advance` 即使没有经过时间也会让空闲小队出发）。每小时交给爽点时间轴的“发出的命令”就是这一小时里日志中的命令，按名字计数。
- 测量到 final.json 的 `t` 所在的整小时为止（`hours` = `t` ÷ 3600），之后把剩下的、时刻不晚于 `t` 的命令执行完（例如最后一次回访时自动领取的见习任务），推进到 `t`，再算存档指纹。
- 机器人写出的会话重放后，存档指纹和指标都和 `eval run` 当场算的逐位相同（`cargo test` 检查）。
- RunFile 的 `commands` 是日志里各命令的条数（含开局设定），和 `eval run` 的口径一样。

一次只评估一个玩家的一组会话：

- 没有 final.json 的目录（没玩完）跳过，打印警告。
- 几个目录的 `player` 不同就报错，请按玩家分开跑；同一个玩家的同一个种子出现两次也报错。
- 时长（final.json 的 `t`）不同也报错：检查点对不齐，指标没法放进同一个 RunFile。
- manifest 的 `protocol` 不是本工具认识的版本（现在是 1），报错并写明会话的版本。

`llm.*` 指标：

- `llm.feel.<阶段>.<标签>`：该阶段回访里各种感受的占比。阶段取 visits.jsonl 每行的 `stage`；没写感受的回访也算在分母里。只输出有回访的阶段，三种标签都输出（没出现的记 0）。
- `llm.score.<维度>.<阶段>`：阶段打分，阶段取该行的 `score_stage`（缺这个字段时取上一次回访的阶段，即刚结束的那个）。同一阶段打了两次取平均。
- `llm.delight_given_felt`、`llm.delight_given_none`：回访窗口里有 / 没有可感爽点事件（爽点时间轴里 tier ≥ 1）时，玩家记“惊喜”的比例。窗口是（上一次回访的时刻，这一次回访的时刻]，第一次回访的窗口从开局算起；没写感受的回访也算在分母里。注意这次回访自己的行动当场引起的事件（比如这一刻占领一格进入新环）也落在这次回访的窗口里。两者差得越开，说明时间轴的分级越贴近玩家的感觉；这是校准幅度分级的依据。
- `llm.delight_lift`：`llm.delight_given_felt` − `llm.delight_given_none`，两者都有时才输出。越大说明爽点分级越贴近玩家的感觉。
- `llm.invalid`：行动里失败项的占比（失败项 ÷ 全部行动项；看得懂的反面信号，也能发现接口问题）。
- 没有回访记录（机器人写的会话）就不输出 `llm.*`。

重放后的存档指纹和 final.json 不一致，或者日志里有命令被引擎拒绝（或者日志里有晚于 final.json `t` 的命令），说明游戏在会话之后改过：这个会话标为过期，只输出 `llm.feel.*`、`llm.score.*`，不输出对局指标和 `llm.delight_given_*`、`llm.delight_lift`、`llm.invalid`，并打印警告。每个重放的会话都写 `session.stale`：过期为 1，否则为 0（机器人写的会话也有），汇总多个会话时据此看出哪些过期。

## 接入其他模型

各模型用自己的 harness 驱动会话，本仓库不放适配器。harness 只需要：

- 调用上面的命令行或 `serve`，按观察里的 `need` 行事，提交符合格式的行动；
- 在 manifest 的 `player` 里写清楚是谁在玩（如 `gemini-2.5-pro`），评估时据此分组；
- 不让玩家读仓库和源代码，只看观察；
- 模型的原始提示和回答由 harness 自己留档，会话目录只存协议规定的内容。

附录的提示要点对任何模型都适用。

## 版本和稳健性

- **协议版本**：`protocol` 字段。只加可选字段不算不兼容；删字段、改含义、改目录结构要加一，并在这里写明变化。
- **会话过期**：见上一节，按重放后的存档指纹判断（重放时有命令被拒绝也算过期）。
- **菜单 id**：由命令内容算出，不随菜单顺序变化。
- **测试**（`cargo test`）：
  - 会话本身（`engine/tests/session.rs`）：模拟玩家通过库接口和 `serve` 各玩一局，命令日志重放得到同一个终局存档；被拒绝的行动不改目录。
  - 评估会话（`engine/examples/eval/session.rs`）：
    - 机器人（`greedy`，种子 0，12 小时）写出的会话重放后，存档指纹一致，指标逐位相同（重放多出的只有 `session.stale` = 0），命令计数相同，不判过期。
    - 脚本玩家通过 `textui::session` 的库接口玩一局（`every:3`，24 小时；每次选第一个菜单项，感受轮流记惊喜、平淡、不写，被问时各维度打 3 分），重放后不判过期，`llm.feel.d1.*` 之和不超过 1，`llm.score.*.d1` 都是 3，`llm.invalid` 为 0，`owned.h12` 等对局指标齐全，所有 id 都在注册表里。
    - 改掉 final.json 的 `save_sha`：判为过期，只剩 `llm.feel.*`、`llm.score.*` 和 `session.stale` = 1。
    - `protocol` 为 2 的 manifest 报错；没有 final.json 的目录跳过；不同玩家、同一种子两次都报错。
  - 事件分类棘轮：`locales/zh-CN/events.json` 里每个事件 key 都要在 `engine/eval/payoff_classes.json` 里有归类（爽点类别和级别，或明确记为“不算”）。新事件不归类，测试就失败，这就是“新系统怎么接入”清单里的爽点声明。
  - 指标注册表：`engine/eval/metrics.json` 登记每个指标 id 的模式、类别（局面探针 / 结果量 / 自选样本）和一句说明。`eval run` 产出的 id、`goals.json` 里引用的 id 都必须能在注册表里找到；改名或拼错立刻失败。`eval metrics` 打印注册表，给人和模型读结果时查。

## 附录：试玩提示要点

完整的玩家提示词模板见 `docs/prompts/player.md`，下面是要点。

- 只许用 `agent_cli session` 这几条命令，不读仓库。
- 每次：`obs` 看局面 → 决定 → `act`（带 `--feel` 和 `--why`）。`need` 为 `scores` 时先打分。
- 不要一次挑太多菜单项；id 不在当前菜单里会被拒绝，看下一次观察的 `results`。
- 同时最多 3 个子代理（`CLAUDE.md`）。
