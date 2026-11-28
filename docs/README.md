# 文档

这里的文件都是**草稿**，从讨论时写的 Claude 文档导出成 markdown，内容会随讨论继续变。原文档链接附在后面，以原文档为准。

## 设计与概念

- [play-guide.md](play-guide.md)：玩法说明（初版）——网页客户端怎么玩：见习任务、每次上线做什么、守城、街区与人口、研究学说与反应图鉴、收束与知识库，附自动玩家看到的已知问题。本仓库直接写成。
- [design-v1.md](design-v1.md)：原型设计 v1——城市与异界合成一张 15×15 棋盘（路线与驻扎、来潮）、居民长成职业并产出队员、简化的队员养成、界面逻辑与风格、Rust 引擎架构、第一版测试范围。与 v0 冲突处以 v1 为准。原文档：[挂机原型设计 v1](https://claude.ai/artifact/UaLQNAuZfBtSbMBbvNvTys)
- [design-v2-outline.md](design-v2-outline.md)：设计 v2 总纲（草稿）——下一版往哪走：核心循环是“琢磨 → 挂机验收 → 瓶颈 → 带新零件再琢磨”；每道墙战斗和城市两边都能过；城市是背包式布局、自由重摆，地块有地形和曾经的用途；进攻难度看异界源，防守压力跟统计分数走、不随时间涨；刷新点、三态迷雾、特殊异界格和特殊队员；一局怎么开始和结束、轮回带走什么、评估要量什么、和现在的差距、还开着的问题。本仓库直接写成。
- [design-v2-city.md](design-v2-city.md)：设计 v2 经营系统（草稿）——总纲的经营部分展开：格子（地形、曾经的用途、障碍）、建筑是可随时重摆的物品、六个类别、三层联动、玩家圈定的街区和自动算出的多属性、信任和政策、两级资源加工链、四种招募来源、装备仿制和图纸、总部部门、守城、自动摆放；评估要量什么、还开着的问题。本仓库直接写成。
- [design-v2-combat.md](design-v2-combat.md)：设计 v2 战斗系统（草稿）——总纲的战斗部分展开：敌人按异界源分族（惯用标签、抗性弱点、行为规则三层），离源越近越要专门的队；碎片、反制和核心；新标签要拿到手里才能用；出战总名额由玩家分到各队；默认战术规则为主；研究分战斗和城市两副牌；挂机路线不打没把握的仗、停下的地方就是墙；过墙的五档手段；难度条件；评估要量什么、还开着的问题。本仓库直接写成。
- [design-v2-plan.md](design-v2-plan.md)：设计 v2 的重构、开发和测试计划（草稿）——按教学关的章节来切：准备、威胁跟分数走、引擎重构（搬到设计语法的类型上）、教学关第一章和最小的轮回闭环、第二到四章各加一两个系统（教学关约一小时，以小核心收尾）、正式局的两层深度、局外的后段；每个阶段重构什么、开发什么、评估和测试怎么做、节点的通过标准、可能推翻什么；网页什么时候补、内容清单在哪个阶段定、风险。本仓库直接写成。
- [design-v2-opening.md](design-v2-opening.md)：设计 v2 开局和节奏（草稿）——一局的三段曲线（轰炸、衰减、寻求提升）；揭示链按五个动词一个一个教；自动化分六层放、最后一层留给轮回之后；衰减靠三条成本曲线；引擎的时间刻度要变细；评估框架为什么没发现开局太慢、改成四方分工、新增的形状指标、LLM 试玩怎么用、拿设计者自己的游玩来校准；初步的目标数字。本仓库直接写成。
- [design-v2-tutorial.md](design-v2-tutorial.md)：设计 v2 教学关（草案）——第一局的编排：7×7 的固定地图，约半小时，四章（走和摆、碰、城市、墙和核心），含政策、建筑升级、自动摆放；查到的参考资料，每章按“碰到麻烦、露出解法”的顺序逐个解锁，以小高潮收尾，最后打下小核心并收束；不进教学关的内容；等设计者定的问题。本仓库直接写成。
- [design-v2-engine-model.md](design-v2-engine-model.md)：设计 v2 引擎模型——让设计语法成为引擎的类型：实体和数量、八种接口、装上去就是摆、交换、属性和状态、品质和合成、战斗是两个容器的反应、五段效果句式、行动点和被动点、容器分有位置和没位置、守备阵型、颠覆规则的内容逐步解锁；以后再定的事项。本仓库直接写成。
- [prototype-v2.md](prototype-v2.md)：原型 v2——按评估工具报的问题改出来的一版：军械（墙与突破）、按环查表的敌人强度、镇守度每胜 +34、研究池随深度扩大、成果权重进平衡表；和 design-v1 不一致的地方、改前改后的读数、还没解决的问题。本仓库直接写成。
- [design-v0.md](design-v0.md)：原型设计定稿（v0）——基于概念草案与调研收敛出的第一版可实现设计基线（核心时间模型、双线软互锁、AP/PP 战斗骨架、大渗透轮回与技术基线）。
- [gameplay-exploration.md](gameplay-exploration.md)：玩法探索——用仿真量化各系统是相加还是相乘（阵容、标签链、装备、规则改写、守城设施、资源流），结论是需要「标签反应表」这样的枢纽和闭环；附 roguelike 与开罗式的改法和引擎里的原型。本仓库直接写成，没有对应的原文档。
- [ideas.md](ideas.md)：想法记录——还没定的想法，定下来再挪进设计文档。
- [concept-draft.md](concept-draft.md)：早期挂机游戏概念草案（世界、玩家身份、玩法结构、还开着的问题）。原文档：[挂机游戏概念草案](https://claude.ai/artifact/QbW6FXVhLp1hbj821vqK98)

## 工程规范与测试

- [architecture.md](architecture.md)：引擎分层——本地化 → 引擎 → 数据 → 机制 → 客户端五层，每层放什么、什么时候改、新词条和新机制各改哪里；`tests/layers.rs` 拦往上引用。图标的格式和画法见 [../icons/README.md](../icons/README.md)。本仓库直接写成。
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
- [suggestions.md](suggestions.md)：建议汇总——各文档里还没做的建议放在一处：要设计者先定的（节奏目标按谁判、推进规章、晶体定位、高潮）、游戏本体、评估工具、仓库，每条写依据、改哪一层、怎么验证，附建议的顺序。本仓库直接写成。
- [game-issues.md](game-issues.md)：游戏本体问题记录——评估工具和试玩查出来、还没改的问题（节奏、成长、爽点分布、防守、规则漏洞），附数据来源。本仓库直接写成。

## 调研

- [research/open-source-implementations.md](research/open-source-implementations.md)：开源挂机游戏的实现做法（大数、主循环、离线收益、存档、许可证）。原文档：[开源挂机游戏实现调研](https://claude.ai/artifact/MDLAUDcQATRL8DD9FdooW4)
- [research/commercial-design.md](research/commercial-design.md)：商业挂机游戏的设计与爽点。原文档：[商业挂机游戏调研：设计与爽点](https://claude.ai/artifact/JfdF2fb3JqAx4s52vScEii)
- [research/opening-pacing.md](research/opening-pacing.md)：开局节奏调研——Evolve 为主，对照 Trimps、Kittens Game、Universal Paperclips、Melvor Idle：开局的揭示链、从手动到自动的分层、建筑倍率和科技价格两条曲线、重置怎么压缩开局；读源码得到的公式和数字，分钟级的实测时间线没查到。本仓库直接写成。
- [research/evolve-opening-timeline.md](research/evolve-opening-timeline.md)：Evolve 开局实测——浏览器脚本以每秒约 3.5 次操作玩了一遍 15 分钟，记下每样新东西出现的秒数；脚本买法不像真人，15 分钟内没走完演化阶段，头一分钟以后的时刻只能当下限参考。原始记录在 `research/data/`。本仓库直接写成。
- [research/prestige-design.md](research/prestige-design.md)：轮回（prestige）重置与保留的设计。原文档：[轮回设计调研](https://claude.ai/artifact/6HNysvA7QmhUMKHvZViEKU)

原文档导出日期：2026-11-26；“本仓库直接写成”的文件随代码一起更新。
