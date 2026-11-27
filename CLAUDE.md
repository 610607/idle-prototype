# 项目约定

## 文字和图像资源（docs/i18n.md）

- 玩家看得到的文字只放在 `locales/<语言>/*.json`，代码里用 `t!("key", 参数 = 值)` 取；内容数据（`engine/content/*.json`）只放 id 和数字；图标只按 id 引用（`icons/pixel/<id>.txt`），图里不画字。
- 存档不存渲染好的句子：要存的文字用 `txt!` 存 key 和参数，key 放 `events.json`；生成的名字存名字池下标。
- 快速迭代期不写存档迁移：存档结构或 `events.json` 的 key 名变了，就把 `SAVE_VERSION` 加一，旧存档直接作废。
- 挪文字前后各跑一次 `cargo run --release --example text_snapshot -- <文件>`，两次输出必须逐字一致。
- 新代码不新增写死的文字。`cargo test` 里的 `i18n` 测试是棘轮：文件里的中文串变多会失败；挪走以后用 `I18N_BLESS=1 cargo test --test i18n` 记下新基线。

## 文字版（textui）跟着功能一起改

- 文字版是和网页并列的客户端，给 LLM 试玩用（docs/llm-playtest.md）：逻辑在 `engine/src/textui.rs`，文字在 `locales/<语言>/cli.json`，`engine/examples/agent_cli.rs` 只是命令行外壳。加了新命令、新的玩家可见信息或预演，同一个改动里也要让文字版看得到、做得到；网页上有、文字版没有的信息会让试玩结论失真。
- 新命令不改文字版编译不过：`textui::reach` 没有兜底分支，要决定它进菜单、只走 `--json`、自动做还是轮回之间；`textui::samples` 和 `cli.cmd.<命令>` 的说明由测试检查有没有漏。
- 改完跑一遍：`cargo run --release --example agent_cli -- new /tmp/t.json 0`，再 `do /tmp/t.json 1 --wait 4`，确认新东西出现在局面或菜单里。

## 子代理

- 同时运行的子代理最多 3 个，后台的也算，workflow 里的也算。要跑更多（比如多局 LLM 试玩），分批：一批全部结束再开下一批。

## 检查

- 引擎：`cargo fmt`、`cargo test`（在 `engine/`）。
- 改了引擎要重编 WebAssembly：`npm run wasm`（在 `web/`，编好的 `pkg/` 要提交）。
- 网页：`npx tsc --noEmit -p .`（在 `web/`）。
- 文档用中文写。
