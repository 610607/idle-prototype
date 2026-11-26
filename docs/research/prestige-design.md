# 轮回设计调研

2026-11-26 · 看了 11 款游戏在轮回（prestige / 重开一局）时重置什么、保留什么、用什么世界观理由，重点是保留下来的东西怎样让下一轮玩法更多样。数值公式（平方根货币等）已在《商业挂机游戏调研》里，这里不重复。

## 结论先说

- **玩得久的轮回，保留的多是“新选项”，不是更大的数字。** 杀戮尖塔的解锁只进入卡池、不进起始牌组；Realm Grinder 每局重选派系；Evolve 每次换星球或宇宙类型。数值加成（宝石、Favor、氦）几乎都有，但让人想再开一局的是规则变了。
- **常见做法是两层：一层每轮清空，一层永久。** 风暴之城（Against the Storm）的定居点会被风暴抹掉，但王城（Citadel）和它的升级永远在；循环英雄（Loop Hero）的远征清空，营地建筑保留。这和我们“街区与基地重置、组织保留知识”的想法基本同构。
- **世界观理由大致三种：周期性灾变、继承、主动按钮。** 风暴之城和 Evolve 用灾变或自毁，盗贼遗产（Rogue Legacy）用后代继承，很多纯挂机游戏（反物质维度、Trimps）几乎不给理由也没人在意。
- **对我们最直接的启发：** 把“异界大渗透”做成轮回的机制来源（像风暴之城的暴风），街区被吞没、地图重排，组织带着契约和经验退回总部。这是我的建议，不是哪款游戏原样有的。

## 对比表

| 游戏 | 每轮重置什么 | 保留什么 | 世界观理由 | 怎样带来变化 |
| --- | --- | --- | --- | --- |
| Evolve（挂机） | MAD：本星球文明清零；Bioseed：离开星系换星球；黑洞：整个宇宙重来 | 质粒（全局加成、买 CRISPR 永久升级）、噬菌体（买永久基因特质） | 核战自毁；造创世飞船播种；把奇异物质倒进黑洞 | 每层换星球、物种、宇宙类型（邪恶宇宙会换掉采集方式）；挑战基因换加成 |
| Realm Grinder | 退位：本局建筑、升级、金币；转生：再加宝石和挖掘 | 宝石（每颗 +3% 金币）、奖杯、传承、红宝石、神器、已解锁升级 | 退位；“死去再转生，带着前世的积累” | 每局选派系，派系各有玩法（亡灵偏离线，精灵偏主动点击）；R2 后开挑战 |
| Trimps | 传送门：几乎一切 | 氦（买天赋，可重洗）、骨头、传家宝、天赋 | 科学家超频传送门把“记忆和物品”带回来，需要氦冷却 | 挑战让下一轮规则不同；传家宝带特定词条 |
| 反物质维度 | 无限：维度、提升、星系；永恒：几乎一切含无限点数 | 无限点数/永恒点数、成就、统计 | 基本不给 | 每层加新规则（挑战），里程碑让你开局就有上一层的东西或直接自动化 |
| Idle Champions | 重置冒险：英雄等级、金币 | 装备、道具、宝石、宝箱；不论成败都给 Favor（加金币） | 来源未写（剧情前 50 关讲完后重复） | 冒险“变体”改目标和限制，如占格子、限定可用英雄 |
| 风暴之城 Against the Storm | 单局：一个定居点；周期末暴风：地图重排，定居点消失 | 王城及其升级、进度等级；本周期资源点必须在暴风前花掉 | 永恒循环，暴风每几十年抹平地图，只有王城安全 | 每周期地图生物群系和修正重洗；周期内贸易、事件效果只活一个周期 |
| 循环英雄 Loop Hero | 一次远征：路线、卡牌、装备 | 营地建筑；资源按退出方式保留：在营火格离开 100%，路上撤退 60%，死亡 30% | 来源页未写 | 撤退比例逼你判断“什么时候收手”；营地建筑解锁新卡和职业 |
| 暗黑地牢 2 | 一次远征 | 希望之烛在希望祭坛换永久强化、解锁英雄和道具；英雄“记忆”跨远征保留 | 学者台词：“想想你失去的，和你要重建的”（氛围，不是机制） | 后期可花烛火开“节点选择”、预购道具；每个英雄各自积累记忆 |
| 哈迪斯 Hades | 一次出逃：祝福、本局道具 | 黑暗之镜天赋、钥匙、宝石、已解锁武器、宫殿工程、图鉴（永不重置）、剧情进度 | 来源页未写 | 天赋有“替代版”只能二选一；武器有变体；剧情每次死后推进 |
| 盗贼遗产 Rogue Legacy | 死去的角色和本局城堡布局 | 花掉的金币换来的庄园升级、职业、铁匠和附魔师 | 后代继承：死后从 3 个随机继承人中选一个 | 继承人带随机特质和职业；力量有上限，靠新周目加难度和奖励 |
| 杀戮尖塔 | 整局牌组和遗物 | 解锁的卡牌和遗物（进卡池，不进起始牌组）、角色、进阶难度 | 来源页未写 | 解锁扩大池子而非加强起手；进阶 20 级叠加难度修正 |

## 对我们有用的五个模式

1. **保留“零件”，不保留“成品”。** 杀戮尖塔和 Realm Grinder 的经验是保留的东西进入可选池，每局重新挑。对应我们：契约、战术、设施蓝图、队员特质在轮回后进入组织的“知识库”，下一轮开局选一部分带上，组合每轮不同，正好服务“涌现 combo”。（推断：这比纯数值加成更合我们“没有硬门槛”的原则）
2. **两层空间：街区每轮清空，总部永久。** 风暴之城的王城、循环英雄的营地都是一个不会被抹掉的据点。我们可以让组织总部（一栋楼或一个地下室）永久存在，装着知识库和永久设施，街区和前线基地才是每轮重来的部分。
3. **轮回由世界给理由，并顺手改规则。** 风暴之城用暴风，Evolve 用核战和换星球、黑洞换宇宙类型。对应我们：周期性的“大渗透”吞没已夺回的街区、重排城市和异界地图，每次大渗透带一种异界形态修正，改变这一轮的规则。这也让“异界本身是什么不重要，只提供机制”落到实处。（建议，非照搬）
4. **撤退比例制造“收手”决策。** 循环英雄的 100% / 60% / 30% 很简单，但让每次远征有张力。对应我们：异界小队带回资源的比例看怎么撤（按计划返回、紧急撤退、全灭），玩家预设的战术里就可以包含撤退条件，和自动战斗天然契合。
5. **轮回后前期自动跳过。** 反物质维度的里程碑让你开局就有上一层的成果或直接自动化。对应我们：组织经验让最初几个街区自动夺回、第一批人自动招到，玩家很快回到有新选择的阶段，不用每轮重复手工前期。

人员方面另有两条可参考：暗黑地牢 2 的英雄“记忆”跨远征保留，盗贼遗产的继承人带随机特质。如果我们的特别队员（来自异界）也能跨轮回保留一部分记忆，而普通招募每轮随机特质，就有“老兵”和“新血”两种组合来源。

## 没核实到的部分

- 反物质维度只读到永恒层，后面的现实层没读到。
- 哈迪斯、杀戮尖塔、循环英雄的世界观理由不在读过的页面里，表里写了“来源页未写”。
- Idle Champions 的重置细节来自 2018 年的玩家讨论帖，可能已过时；风暴之城资源点清零的规则来自 2021 年开发日志，2023 年大更新后没有复述。
- 原计划的 Idle Loops 没找到可读的来源，就没写进来。

## 来源

- Evolve：[Evolve 新手指南](https://wooledge.org/~greg/evolve/guide.html)、[TV Tropes 条目](https://www.tvtropes.org/pmwiki/pmwiki.php/VideoGame/EvolveIdle)
- Realm Grinder：[Soft Resets 维基](https://realm-grinder.fandom.com/wiki/Soft_Resets)、[PC Gamer 新手指南](https://www.pcgamer.com/realm-grinder-guide/)
- Trimps：[Portal 维基](https://trimps.fandom.com/wiki/Portal?oldid=9659)
- 反物质维度：[Guide 维基](https://antimatter-dimensions.fandom.com/wiki/Guide)、[Eternity Milestones 维基](https://antimatter-dimensions.fandom.com/wiki/Eternity_Milestones)
- Idle Champions：[Adventure 维基](https://idlechampions.fandom.com/wiki/Adventure)、[Steam 讨论帖](https://steamcommunity.com/app/627690/discussions/0/1696049513778439976)
- 风暴之城：[Blightstorm Cycle 维基](https://against-the-storm.fandom.com/wiki/Blightstorm_Cycle)、[开发日志 4：元进度](https://eremitegames.com/devlog-4-meta-progression-new-biome-and-more/)、[Cycles Reforged 更新](https://eremitegames.com/cycles-reforged-update/)
- 循环英雄：[Gamertweak 回营地](https://gamertweak.com/how-to-return-camp-loop-hero)、[Twinfinite 回营地](https://twinfinite.net/guides/loop-hero-return-to-camp-how/)、[Gfinity 死亡会怎样](https://www.gfinityesports.com/article/loop-hero-death-what-happens-when-you-die)
- 暗黑地牢 2：[Altar of Hope 维基](https://darkestdungeon.wiki.gg/wiki/Altar_of_Hope)、[Candles of Hope 维基](https://darkestdungeon.wiki.gg/wiki/Candles_of_Hope)
- 哈迪斯：[Gamepressure 保留内容](https://www.gamepressure.com/hades/progress-what-gets-carried-forward/z9ebaa)、[Mirror of Night 维基](https://hades.wiki.fextralife.com/Mirror_of_Night)
- 盗贼遗产：[维基百科](https://en.wikipedia.org/wiki/Rogue_Legacy)
- 杀戮尖塔：[进阶指南](https://www.highgroundgaming.com/slay-the-spire-ascension-guide/)、[Steam 讨论帖](https://steamcommunity.com/app/646570/discussions/0/1742232339944759355)
