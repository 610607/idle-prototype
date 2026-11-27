# 项目约定

## 分层（docs/architecture.md）

- 引擎源码分五层：本地化 `l10n` → 引擎 `kernel` → 数据 `data` → 机制 `mech` → 客户端 `client`，每层只用前面的层，`tests/layers.rs` 会拦。
- 新词条只改数据层：`content/*.json` + `locales/`，用现有效果种类表达；表达不了才加效果种类（数据层枚举 + 机制层读它）。
- 机制层不写死内容 id：代码要按名字认的内容登记在 `data/hooks.rs`，测试会拦。
- 引擎层（时钟、存档信封、随机、几何）尽量不动；新的定时机制接 `mech/sim.rs` 的 `Timeline` 实现。

## 文字和图像资源（docs/i18n.md）

- 玩家看得到的文字只放在 `locales/<语言>/*.json`，代码里用 `t!("key", 参数 = 值)` 取；内容数据（`engine/content/*.json`）只放 id 和数字；图标只按 id 引用（`icons/pixel/<id>.txt`），图里不画字。
- 文字里不写数字：数值从 `engine/content/balance.json` 或内容数据当参数传（网页用 `game.balance`），代码里的这类常数也放进 `balance.json`；`runtime_text_quotes_no_numbers` 测试会拦。
- 存档不存渲染好的句子：要存的文字用 `txt!` 存 key 和参数，key 放 `events.json`；生成的名字存名字池下标。
- 快速迭代期不写存档迁移：存档结构或 `events.json` 的 key 名变了，就把 `SAVE_VERSION` 加一，旧存档直接作废。
- 挪文字前后各跑一次 `cargo run --release --example text_snapshot -- <文件>`（网页是 `node scripts/text_dump.mjs <文件>`，在 `web/`），两次输出必须逐字一致。
- 网页用 `web/src/i18n.ts` 的 `t('key', { 参数 })` / `tn()`，文字放 `locales/<语言>/web.json`。
- 新代码不新增写死的文字。`cargo test` 里的 `i18n` 测试是棘轮：文件里的中文串变多会失败；挪走以后用 `I18N_BLESS=1 cargo test --test i18n` 记下新基线。

## 文字版（textui）跟着功能一起改

- 文字版是和网页并列的客户端，给 LLM 试玩用（docs/llm-playtest.md）：逻辑在 `engine/src/client/textui.rs`，文字在 `locales/<语言>/cli.json`，`engine/examples/agent_cli.rs` 只是命令行外壳。加了新命令、新的玩家可见信息或预演，同一个改动里也要让文字版看得到、做得到；网页上有、文字版没有的信息会让试玩结论失真。
- 新命令不改文字版编译不过：`textui::reach` 没有兜底分支，要决定它进菜单、只走 `--json`、自动做还是轮回之间；`textui::samples` 和 `cli.cmd.<命令>` 的说明由测试检查有没有漏。
- 改完跑一遍：`cargo run --release --example agent_cli -- new /tmp/t.json 0`，再 `do /tmp/t.json 1 --wait 4`，确认新东西出现在局面或菜单里。

## 数值评估（docs/balance-eval.md、docs/balance-model.md）

- 新系统、新词条都要接进评估工具 `engine/evalkit`，按 `docs/balance-eval.md`“新系统怎么接入”的清单在同一个改动里补齐：指标、机器人会用到它、合格区间、模型系数。
- 新特质、装备、学说跑 `cargo run --release --example eval -- price --bless`，把价格写进 `engine/eval/prices.json` 一起提交；`cargo test` 里的 `eval_prices` 测试会拦漏测价的词条。
- 改数值：先用 `eval pace --patch '{…}'` 和 `eval model` 算预期，再用 `eval run`（参考玩家是 `greedy`）+ `eval review`（对照 `engine/eval/goals.json` 的设计目标和基线）/ `eval compare` 验证，有意改动后 `eval review … --bless` 更新基线；模型和机器人对不上超过两成，要补模型或写明原因。
- 机器人用不到的命令会出现在 `commands.never`，等于没被评估：给 `evalkit/src/bot.rs` 加规则或加人格。
- 爽点声明：新的事件 key 要在 `engine/eval/payoff_classes.json` 里归类（爽点类别和级别，或记为涓流）；新指标 id 要登记到 `engine/eval/metrics.json`。两样 `cargo test` 都会拦。
- LLM 和其他模型试玩走会话协议（`agent_cli session`，见 `docs/eval-interface.md`），事后用 `eval session` 重放算分。观察、行动或会话目录有不兼容的改动，要把 `protocol` 加一并在文档里写明。

## 子代理

- 同时运行的子代理最多 3 个，后台的也算，workflow 里的也算。要跑更多（比如多局 LLM 试玩），分批：一批全部结束再开下一批。

## 检查

- 引擎：`cargo fmt`、`cargo test`（在 `engine/`）。
- 改了引擎要重编 WebAssembly：`npm run wasm`（在 `web/`，编好的 `pkg/` 要提交）。
- 网页：`npx tsc --noEmit -p .`（在 `web/`）。
- 文档用中文写。
