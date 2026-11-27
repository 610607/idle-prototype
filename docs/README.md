# 文档

这里的文件都是**草稿**，从讨论时写的 Claude 文档导出成 markdown，内容会随讨论继续变。原文档链接附在后面，以原文档为准。

## 设计与概念

- [play-guide.md](play-guide.md)：玩法说明（初版）——网页客户端怎么玩：见习任务、每次上线做什么、守城、街区与人口、研究条令与反应图鉴、收束与知识库，附自动玩家看到的已知问题。本仓库直接写成。
- [design-v1.md](design-v1.md)：原型设计 v1——城市与异界合成一张 15×15 棋盘（路线与驻扎、来潮）、居民长成职业并产出队员、简化的队员养成、界面逻辑与风格、Rust 引擎架构、第一版测试范围。与 v0 冲突处以 v1 为准。原文档：[挂机原型设计 v1](https://claude.ai/artifact/UaLQNAuZfBtSbMBbvNvTys)
- [design-v0.md](design-v0.md)：原型设计定稿（v0）——基于概念草案与调研收敛出的第一版可实现设计基线（核心时间模型、双线软互锁、AP/PP 战斗骨架、大渗透轮回与技术基线）。
- [gameplay-exploration.md](gameplay-exploration.md)：玩法探索——用仿真量化各系统是相加还是相乘（阵容、标签链、装备、规则改写、守城设施、资源流），结论是需要「标签反应表」这样的枢纽和闭环；附 roguelike 与开罗式的改法和引擎里的原型。本仓库直接写成，没有对应的原文档。
- [ideas.md](ideas.md)：想法记录——还没定的想法，定下来再挪进设计文档。
- [concept-draft.md](concept-draft.md)：早期挂机游戏概念草案（世界、玩家身份、玩法结构、还开着的问题）。原文档：[挂机游戏概念草案](https://claude.ai/artifact/QbW6FXVhLp1hbj821vqK98)

## 工程规范与测试

- [i18n.md](i18n.md)：文字与图像资源规范——玩家看得到的文字只放在 `locales/`，内容数据只放 id，图标按 id 引用，存档不存成品句子；棘轮测试和迁移进度。本仓库直接写成。
- [llm-playtest.md](llm-playtest.md)：LLM 试玩——文字版 `agent_cli` 让 Haiku 和脚本玩家并排玩 72 小时，研究组对照组、结果和新玩家看不懂的地方。本仓库直接写成。

## 数值

- [balance-eval.md](balance-eval.md)：数值评估框架——评估工具 `eval` 的分层、指标、区间、人格、对照与新系统接入清单，以及已定和待讨论的事项。本仓库直接写成。
- [balance-model.md](balance-model.md)：数值数学模型——战斗“两道检查”和节奏模型（每格耗时 × 格数），用机器人实测检验。本仓库直接写成。
- [balance-curves-research.md](balance-curves-research.md)：数值曲线调研——其他游戏怎么用主曲线给数值定价，拿来量本仓库。本仓库直接写成。
- [payoff-pacing-research.md](payoff-pacing-research.md)：爽点节奏调研——其他游戏怎么设计和量化爽点（里程碑倍率、新系统、自动化、墙与突破、轮回），以及爽点事件时间轴的模型和指标提议、一周骨架草案。本仓库直接写成。
- [eval-interface.md](eval-interface.md)：评估接口——试玩会话协议（观察、行动、日程、打分维度）、会话目录格式、`eval session` 重放和 `llm.*` 指标、协议版本和稳健性测试；其他模型由各自的 harness 接入。本仓库直接写成。
- [prompts/](prompts/README.md)：提示词模板——试玩玩家、读评估报告的分析员、新系统的声明起草；各模型的 harness 读模板填变量，仓库不放调用模型的代码。本仓库直接写成。
- [eval-status.md](eval-status.md)：评估工具现状和缺口——能评估什么、还缺什么（轮回、打法多样性、选择的意义、试玩样本）、代码要不要整理，以及待办。本仓库直接写成。
- [game-issues.md](game-issues.md)：游戏本体问题记录——评估工具和试玩查出来、还没改的问题（节奏、成长、爽点分布、防守、规则漏洞），附数据来源。本仓库直接写成。

## 调研

- [research/open-source-implementations.md](research/open-source-implementations.md)：开源挂机游戏的实现做法（大数、主循环、离线收益、存档、许可证）。原文档：[开源挂机游戏实现调研](https://claude.ai/artifact/MDLAUDcQATRL8DD9FdooW4)
- [research/commercial-design.md](research/commercial-design.md)：商业挂机游戏的设计与爽点。原文档：[商业挂机游戏调研：设计与爽点](https://claude.ai/artifact/JfdF2fb3JqAx4s52vScEii)
- [research/prestige-design.md](research/prestige-design.md)：轮回（prestige）重置与保留的设计。原文档：[轮回设计调研](https://claude.ai/artifact/6HNysvA7QmhUMKHvZViEKU)

导出日期：2026-11-26。
