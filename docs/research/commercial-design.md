# 商业挂机游戏调研：设计与爽点

Nov 26, 2026 · @biobot

看了 13 款挂机游戏，让人停不下来的机制其实就四件事：成本指数涨、产出慢慢涨，中间靠里程碑倍率制造“一下跳上去”；转生把数字压回可调范围，再给一个更快重跑的理由；离线收益让每次回来都有一份礼物；随机或限时事件把人拉回来主动玩一会儿。各家的差别主要在三处：节奏快还是慢，转生叠几层，钱从哪里来（买断、加速和倍率、广告、抽卡）。

来源分两类：设计理论（Kongregate 的 Anthony Pecorella 的系列文章和 GDC 演讲、CHI 2018 的放置游戏分类论文），以及逐款资料（wiki、开发者访谈、商店页、开源代码）。标 \[推断\] 的是我的判断，没有来源直接支持。

## 一眼对照

| 游戏 | 子类型 | 成本增长 | 转生层数 | 离线 | 商业化 | 最核心的爽点 |
| --- | --- | --- | --- | --- | --- | --- |
| [Cookie Clicker](https://cookieclicker.wiki.gg/wiki/Buildings) | 点击、单资源 | 每个建筑 ×1.15 | 1 层（飞升）+ 挑战模式 | 要买升级才开，5% 起，最高 75% | 网页免费，Steam $4.99 | 金饼干叠倍率连击 |
| [Clicker Heroes](https://clickerheroes.fandom.com/wiki/Heroes) | 点击 RPG | 英雄 ×1.07 | 2 层（飞升、超越） | 无上限 | 免费，卖红宝石和跳时 | Boss 倒计时、每 25 级 ×4 |
| [AdVenture Capitalist](https://en.wikipedia.org/wiki/AdVenture_Capitalist) | 经营放置 | 各产业 1.07 到 1.15 | 1 层（天使）+ 新星球 | 经理离线时继续赚 | 免费，跳时、永久倍率、激励视频 | 经理自动化、里程碑提速 |
| [Realm Grinder](https://store.steampowered.com/app/610080/Realm_Grinder/) | 深度构筑 | 会涨价，倍率未查到 | 3 层（退位、转生、飞升） | 有，部分阵营专吃离线加成 | 免费，卖红宝石 | 阵营 × 法术 × 研究的构筑组合 |
| [Antimatter Dimensions](https://store.steampowered.com/app/1399720/Antimatter_Dimensions/) | 衍生生产链 | 每买 10 个价格跳一级 | 3 层 + 7 个天神关 | 离线按 tick 补算 | Steam 免费，卖倍率和跳时 | 突破 1.79e308 数字墙，每层自动化上一层 |
| [Kittens Game](https://store.steampowered.com/app/1097410/Kittens_Game/) | 多资源管理 | 约 ×1.15 | karma、paragon + 永久科技树 | 回来时快进，最多 10 到 40 年 | iOS $2.99，无广告无内购 | 文字版文明科技树展开、冬季生存压力 |
| [A Dark Room](https://en.wikipedia.org/wiki/A_Dark_Room) | 叙事增量 | 线性加硬上限 | 无，结局后重来 | 未找到离线补算 \[推断\] | 网页免费，iOS $0.99 | 界面慢慢长出来，世界一点点放大 |
| [Universal Paperclips](https://en.wikipedia.org/wiki/Universal_Paperclips) | 叙事增量 | 指数，到平台期要投入“突破” | 3 个阶段 + 结局后二选一加成 | 未核实 | 网页免费，iOS $1.99 | 规模从工厂跳到地球再到宇宙 |
| [Egg, Inc.](https://en.wikipedia.org/wiki/Egg,_Inc.) | 手游经营 | 未查到 | 2 层（灵魂蛋、预言蛋），官方叫“嵌套转生” | 粮仓存收益；免费 2 座半速，Pro 10 座全速 | 免费，Pro 许可证 $9.99、存钱罐 | 复利转生、限量预言蛋、多人合约 |
| [Idle Miner Tycoon](https://www.gameanalytics.com/blog/making-a-hit-idle-game-eight-lessons-from-kolibri-games) | 手游经营 | 未查到 | 每座矿单独转生，最多 6 次 | 有 | 约 60% 收入来自广告 | 一屏看清产线、激励视频短时暴增 |
| [AFK Arena](https://www.deconstructoroffun.com/blog/2019/6/6/afk-arena-puts-lilith-into-the-billionaire-club) | 放置卡牌 RPG | 关卡难度线性升，Boss 关突然变难 | 无传统转生 \[推断\] | 宝箱约存 12 小时，VIP 更长 | 抽卡、VIP、通行证、限时礼包 | 前期奖励不断、抽卡收集 |
| [Melvor Idle](https://www.jagex.com/news/jagex-announces-partnership-to-publish-melvor-idle) | 放置 RPG（RuneScape 式） | 技能等级 | 未找到 \[推断：无\] | 玩家称 24 小时，开发者未确认 | 买断 $9.99 + 资料片 | 技能练满、零付费焦虑 \[推断\] |
| [Legends of IdleOn](https://store.steampowered.com/app/1476970/Legends_of_Idleon_MMO/) | 放置 MMO | 角色栏按总等级解锁 | 未找到 | 按“挂机效率”折算在线收益 | 免费，宝石商店 | 同时养十个角色、每个世界一套新系统 \[推断\] |

## 共性一：指数成本对多项式产出

成本按指数涨、产出按线性或多项式涨，这是所有经营类挂机的骨架。Pecorella 在 [The Math of Idle Games Part I](https://gamedeveloper.com/design/the-math-of-idle-games-part-i) 里给出的公式，b 是基础价，r 是增长率，n 是已拥有数：

```latex
\text{cost}_n = b \cdot r^{n} \qquad \text{production} = p \cdot n \cdot \text{multipliers}
```

开局产出跑在成本前面，后来指数一定追上来，玩家就觉得“慢了”。这时一个倍率把收入拉回去，就是“跳”的那一刻。各家倍率的落点：

- Cookie Clicker：每个建筑 15 档升级，各 ×2，在拥有 1、5、25、50 个时解锁，之后每 50 个一档到 600。[wiki](https://cookieclicker.wiki.gg/wiki/Factory)
- Clicker Heroes：200 级起每 25 级伤害 ×4，到 8000 级前每 1000 级 ×10。[wiki](https://clickerheroes.fandom.com/wiki/Heroes)
- AdVenture Capitalist：拥有 25、50、100、200、300、400 个时生产时间减半，全部产业同时达标再减半，最多快 4096 倍。[二手来源，引自 wiki](https://medium.com/gametalk/idle-capitalism-eac089cd670)
- Antimatter Dimensions：同一维度每买 10 个产量 ×2，价格跳一级。[wiki](https://antimatter-dimensions.fandom.com/wiki/Guide)

Pecorella 的平衡建议是让“性价比最高的购买”在不同产业之间轮换，否则老产业会被永远冷落。他在 GDC 2016 的 [Idle Chatter](https://www.slideshare.net/slideshow/idle-chatter-gdc-2016-59734260/59734260) 里还说，平滑曲线很无聊，进度应该“颠簸”。

A Dark Room 是反例：陷阱和小屋是线性成本加硬上限（最多 10 个陷阱、20 间小屋），增长服务于叙事，不追求数字爆炸。[源码](https://github.com/doublespeakgames/adarkroom)

## 共性二：转生货币用开方压缩

转生做两件事：给人“爬梯子”的感觉，把数字压回能调的范围（[Part III](https://gamedeveloper.com/design/the-math-of-idle-games-part-iii)）。几乎所有公式都用开方或分数指数，所以转生货币翻倍要靠一局多赚好几倍。

| 游戏 | 转生货币公式（c 为收益） | 转生货币翻倍需要的收益 | 来源 |
| --- | --- | --- | --- |
| Cookie Clicker | ∛(c / 10¹²) | 8 倍 | Pecorella Part III |
| AdVenture Capitalist | 150 · √(c / 10¹⁵) | 3 到 4 倍 | 同上 |
| Realm Grinder | (√(1 + 8c/10¹²) − 1) / 2 | 约 4 倍 | 同上 |
| Egg, Inc. | 每局增量 (c / 10⁶)^0.14 | 128 倍 | 同上 |
| Kittens Game | karma = (√(1 + 8k/5) − 1) / 2，k 按猫数分段累计 | 未算 | 源码 game.js（社区 fork） |
| Antimatter Dimensions | IP = 10^(log₁₀(最大反物质)/308 − 0.75) | 未算 | [源码 game.js](https://github.com/IvarK/AntimatterDimensionsSourceCode) |

Pecorella 的经验法则：这次能拿的转生货币到现有的 +50% 到 +200% 时就该重置；循环越来越慢时，再加一层元转生。[Quest for Progress](https://www.slideshare.net/slideshow/quest-for-progress-gdc-europe-2016/65405507) Realm Grinder 指南的说法是可得宝石约为现有 2 倍时退位。[PC Gamer](https://www.pcgamer.com/au/realm-grinder-guide)

层数从 0 到 3 层以上都有：

- 无：A Dark Room；AFK Arena 用英雄升阶和收集代替 \[推断\]。
- 1 层：Cookie Clicker、AdVenture Capitalist、Idle Miner Tycoon（但每座矿各自转生）。
- 2 层：Clicker Heroes（英雄之魂，再到远古之魂）、Egg, Inc.（灵魂蛋，再到预言蛋，每颗复利 ×1.05）。
- 3 层以上：Realm Grinder（退位、转生、飞升）、Antimatter Dimensions（Infinity、Eternity、Reality，再到 7 个天神）。

Antimatter Dimensions 作者 Hevipelle 说多层转生是从 Realm Grinder 学来的，并且“推得比原作更远”。他的做法是每层带一套新规则：Eternity 开时间维度和挑战，Reality 开符文、黑洞和脚本自动化。[访谈](https://www.incrementaldb.com/community/interview/31)

## 共性三：揭示节奏

Alharthi 等人的论文把“功能逐步出现”列为设计要点；Tuts+ 的文章把新内容分成“已知的未知”（预告过的）和“未知的未知”（惊喜）。[论文](https://par.nsf.gov/servlets/purl/10061230)、[Tuts+](https://code.tutsplus.com/numbers-getting-bigger-the-design-and-math-of-incremental-games--cms-24023a)

- 叙事派把揭示本身当玩法。A Dark Room 作者 Townsend 说，玩家每打开新机制，游戏视野就“拉远”一点；慢节奏让玩家有时间建立自己的世界模型。[开发者评论稿](https://s3.us-east-2.amazonaws.com/amirrajan-public/design-process.pdf) Universal Paperclips 每个阶段换一种资源语法，后期成本直接用回形针计价。[IF50](https://if50.substack.com/p/2017-universal-paperclips)
- 门槛派用硬条件开新系统：Antimatter Dimensions 的 1.79e308 数字墙，Cookie Clicker 10 亿饼干开糖块，IdleOn 按全角色总等级开新角色栏（20 级开第 2 个，1500 级开第 10 个）。[IdleOn 指南](https://www.digitaltq.com/wiki/idleon/beginner-guide)
- 手游派按主线进度开：AFK Arena 新功能跟着主线关卡解锁，Deconstructor of Fun 评价“高度可预期”。
- Kittens Game 把很多机制藏起来，条件满足才出现按钮，代价是很多东西要查 wiki。[TV Tropes](https://www.tvtropes.org/pmwiki/pmwiki.php/VideoGame/KittensGame)

## 共性四：离线收益与回拉事件

Pecorella 的观点是：离线时进度照样积累，每次回来都是奖励，不需要额外的预约机制；“花光再回来”本身就是天然的体力系统。[Idle Chatter](https://www.slideshare.net/slideshow/idle-chatter-gdc-2016-59734260/59734260) AdVenture Capitalist 被认为是把离线收益带火的作品。[Game Developer](https://gamedeveloper.com/design/the-rise-of-games-you-mostly-don-t-play)

离线怎么做，各家差很多：

- 不设限：Clicker Heroes。
- 要买才有：Cookie Clicker 需要一个天堂升级才开离线，起步 5% 效率，升满到 75%。[wiki](https://cookieclicker.wiki.gg/wiki/Prestige)
- 存量上限加付费扩容：Egg, Inc. 每座粮仓存 1 小时，研究后最多 3 小时；免费玩家最多 2 座且只有一半效率，Pro 许可证 10 座全效率。[wiki](https://egg-inc.fandom.com/wiki/Grain_Silo) AFK Arena 宝箱约存 12 小时，VIP 越高越长。
- 打折：IdleOn 按“挂机效率”折算，比如 50%；Antimatter Dimensions 的离线 EP 是最佳速率的 25%。
- 快进模拟：Kittens Game 回来时一次性快进，上限 10 年，后期 40 年；Antimatter Dimensions 离线按 tick 补算。
- 做进构筑：Realm Grinder 的亡灵和邪恶阵营专吃离线加成。

为了不让游戏变成纯等待，几乎每款都有把人拉回主动玩的事件。Cookie Clicker 的金饼干每 5 到 15 分钟出现、停 13 秒，能叠出 ×777 的点击暴击；Clicker Heroes 每 5 关一个 Boss，通常 30 秒倒计时；AdVenture Capitalist 和 Egg, Inc. 有每周活动和限时合约；AFK Arena 竞技场 14 天一季、迷宫 48 小时刷新。论文把金饼干和 Boss 列为“回到主动玩”的典型。\[推断\] 离线负责“回来有礼物”，事件负责“回来后多待一会儿”，两者是一对。

## 共性五：钱从哪里来

| 模式 | 代表 | 卖什么 | 备注 |
| --- | --- | --- | --- |
| 买断 | Melvor Idle、Kittens Game、A Dark Room、Universal Paperclips、Cookie Clicker（Steam） | 游戏本体、资料片 | Paperclips 和 Kittens 明确无广告无内购 |
| 加速和永久倍率 | AdVenture Capitalist、Clicker Heroes、Realm Grinder、Antimatter Dimensions | 跳时（6 到 168 小时）、永久 ×2 倍率 | Pecorella 总结的主流免费做法 |
| 激励视频 | Idle Miner Tycoon、AdVenture Capitalist | 看广告换短时收益加成 | Idle Miner 约 60% 收入来自广告，每局约看 8 条 |
| 离线和便利付费 | Egg, Inc. | Pro 许可证、存钱罐 | 存钱罐平时自动攒金蛋，付钱才能砸开 |
| 抽卡、VIP、通行证 | AFK Arena | 英雄卡、VIP 礼包、$24.99 赛季通行证 | 终身营收超 15 亿美元（AppMagic 估算） |

Clicker Heroes 开发者说有玩家在红宝石上花了好几千美元，出于道德顾虑，续作改成了 $30 买断。[PC Gamer](https://www.pcgamer.com/clicker-heroes-2-drops-free-to-play-model-over-developers-ethical-concerns) Kolibri 联合 CEO 说他们 MVP 阶段完全不做变现，只看次日留存，做到约 75%（头部游戏通常约 40%）。[PocketGamer.biz](https://www.pocketgamer.biz/feature/70431/how-kolibri-went-from-minimal-viable-product-to-70-million-installs-for-its-idle-games/)

## 爽点清单

把“停不下来”拆成具体机制，大致按出现频率排：

1. 数字跳变：倍率在固定拥有数触发，收入一下翻好几倍（Cookie Clicker、Clicker Heroes、AdVenture Capitalist、Antimatter Dimensions）。
2. 自动化解放：雇了经理就不用再点（AdVenture Capitalist）；Antimatter Dimensions 每层都把上一层自动化掉。
3. 重置后的加速重跑：带着转生加成重来，前面要几小时的路几分钟跑完 \[推断，所有带转生的作品\]。
4. 随机暴击：金饼干的 Frenzy ×7、Click Frenzy ×777 可以叠成连击。
5. 回来有礼物：离线收益让每次打开都是一次领奖。
6. 世界放大：A Dark Room 从一间房到整片荒野，Paperclips 从工厂到宇宙。
7. 构筑组合：Realm Grinder 的阵营 × 法术 × 研究，build 能导出分享。
8. 收集和并行：AFK Arena 抽英雄，IdleOn 同时养十个角色，Idle Miner 收超级经理。
9. 稀缺和社交压力：Egg, Inc. 预言蛋总量有限，合约要和别人一起完成。

Hevipelle 把 Antimatter Dimensions 定位为“很慢节奏的放置游戏，有点脱离快节奏放置的潮流”；Paperclips 作者 Frank Lantz 说他想做真正让人着迷的东西，但要玩家情愿，“我想要你想被它钩住”。[Hevipelle 访谈](https://www.incrementaldb.com/community/interview/31)、[Lantz 访谈](https://www.ycombinator.com/blog/frank-lantz-director-of-nyus-game-center-and-creator-of-universal-paperclips)

## 各自的取舍

- 快还是慢：AdVenture Capitalist 几分钟一个里程碑；Antimatter Dimensions 和 Kittens Game 刻意慢，Kittens 自述“设计成长期、半挂机地玩”。\[推断\] 快的留住新手，慢的留住重度玩家。
- 无限还是有结局：Cookie Clicker 和 Egg, Inc. 没有尽头；A Dark Room 和 Paperclips 有结局，Paperclips 前 11 天 45 万人玩、多数打通。[Wikipedia](https://en.wikipedia.org/wiki/Universal_Paperclips)
- 转生层数：层数越多寿命越长，但新玩家越难看懂 \[推断\]。Antimatter Dimensions 用每层一套新规则来抵消重复感。
- 挂机还是主动：Alharthi 等人的定义里，挂机游戏允许离开再回来“几乎无惩罚”，但大部分作品还是保留了奖励主动玩的事件；Pecorella 认为自动连点器是“设计错误”。
- 变现与口碑：免费模式收入天花板高（AFK Arena），但 IdleOn 的差评集中在付费变强和限时礼包带来的错过焦虑 [来源](https://vaporlens.app/app/1476970/idle_on.md)；买断作品的 Steam 好评率普遍在 90% 以上（Melvor 93%，Cookie Clicker 96%，Antimatter Dimensions 91%）。

## 证据缺口

- 没找到 Realm Grinder、Egg, Inc.、Melvor Idle、IdleOn、Kittens Game 作者讲设计意图的访谈。
- Realm Grinder“每颗宝石 +3% 产出”只见于 PC Gamer 指南，wiki 没有印证。
- Egg, Inc. 离线上限：Pecorella 2017 年写的是 2 小时，现在 wiki 是每座粮仓最多 3 小时、最多 10 座，应是版本变化 \[推断\]。
- AFK Arena 营收两处数字差很多（15 亿与约 10 亿美元），都引 AppMagic，应是统计时间不同 \[推断\]。
- Paperclips 和 Melvor 的离线机制未核实；Kittens Game 公式取自社区 fork，没有对照主仓。
- Pecorella 2015 年 GDC 演讲原文读不到，只有 Game Developer 的报道和他之后的幻灯片。

## 来源

设计理论

- Pecorella, The Math of Idle Games [Part I](https://gamedeveloper.com/design/the-math-of-idle-games-part-i)、[Part II](https://www.gamedeveloper.com/game-platforms/the-math-of-idle-games-part-ii)、[Part III](https://gamedeveloper.com/design/the-math-of-idle-games-part-iii)
- Pecorella, [Quest for Progress（GDC Europe 2016）](https://www.slideshare.net/slideshow/quest-for-progress-gdc-europe-2016/65405507)、[Idle Chatter（GDC 2016）](https://www.slideshare.net/slideshow/idle-chatter-gdc-2016-59734260/59734260)
- Parkin, [The rise of games you mostly don't play](https://gamedeveloper.com/design/the-rise-of-games-you-mostly-don-t-play)（GDC 2015 演讲报道）、[GDC Vault 摘要](https://gdcvault.com/play/1022066/Idle-Games-The-Mechanics-and)
- Alharthi 等, [Playing to Wait: A Taxonomy of Idle Games（CHI 2018）](https://par.nsf.gov/servlets/purl/10061230)
- Tuts+, [Numbers Getting Bigger](https://code.tutsplus.com/numbers-getting-bigger-the-design-and-math-of-incremental-games--cms-24023a)

逐款资料

- Cookie Clicker：[Buildings](https://cookieclicker.wiki.gg/wiki/Buildings)、[Prestige](https://cookieclicker.wiki.gg/wiki/Prestige)、[金饼干效果](https://cookieclicker.wiki.gg/wiki/Dragonflight)、[Wikipedia](https://en.wikipedia.org/wiki/Cookie_Clicker)、[Steam](https://store.steampowered.com/app/1454400)
- Clicker Heroes：[Heroes wiki](https://clickerheroes.fandom.com/wiki/Heroes)、[Hero Souls 指南](https://blog.clickerheroes.com/clicker-heroes-hero-souls-complete-guide-to-power-up/)、[Wikipedia](https://en.wikipedia.org/wiki/Clicker_Heroes)、[续作改买断](https://www.pcgamer.com/clicker-heroes-2-drops-free-to-play-model-over-developers-ethical-concerns)
- AdVenture Capitalist：[Wikipedia](https://en.wikipedia.org/wiki/AdVenture_Capitalist)、[Angel Upgrades](https://adventure-capitalist.fandom.com/wiki/Angel_Upgrades)、[Rattink 访谈](https://www.pocketgamer.biz/hyper-hippos-tristan-rattink-on-why-adventure-capitalist-is-still-here-after-5-years)、[Sensor Tower](https://sensortower.com/blog/adventure-capitalist-revenue)
- Realm Grinder：[PC Gamer 指南](https://www.pcgamer.com/au/realm-grinder-guide)、[Soft Resets](https://realm-grinder.fandom.com/wiki/Soft_Resets)、[Steam](https://store.steampowered.com/app/610080/Realm_Grinder/)
- Antimatter Dimensions：[Guide](https://antimatter-dimensions.fandom.com/wiki/Guide)、[Eternity](https://antimatter-dimensions.fandom.com/wiki/Eternity)、[源码](https://github.com/IvarK/AntimatterDimensionsSourceCode)、[Hevipelle 访谈](https://www.incrementaldb.com/community/interview/31)
- Kittens Game：[Steam](https://store.steampowered.com/app/1097410/Kittens_Game/)、[App Store](https://apps.apple.com/us/app/kittens-game/id1198099725)、[TV Tropes](https://www.tvtropes.org/pmwiki/pmwiki.php/VideoGame/KittensGame)、[源码 fork](https://github.com/kitten-science/kittensgame)
- A Dark Room：[Wikipedia](https://en.wikipedia.org/wiki/A_Dark_Room)、[开发者评论稿](https://s3.us-east-2.amazonaws.com/amirrajan-public/design-process.pdf)、[Rajan 访谈](https://www.pocketgamer.biz/shedding-light-the-making-of-a-dark-room)、[源码](https://github.com/doublespeakgames/adarkroom)
- Universal Paperclips：[Wikipedia](https://en.wikipedia.org/wiki/Universal_Paperclips)、[Stages wiki](https://universalpaperclips.fandom.com/wiki/Stages)、[IF50](https://if50.substack.com/p/2017-universal-paperclips)、[Lantz 访谈（YC）](https://www.ycombinator.com/blog/frank-lantz-director-of-nyus-game-center-and-creator-of-universal-paperclips)、[Lantz 访谈（PCGI）](https://www.pcgamesinsider.biz/interviews-and-opinion/66271/interview-paperclips-developer-frank-lantz/)
- Egg, Inc.：[Wikipedia](https://en.wikipedia.org/wiki/Egg,_Inc.)、[Egg of Prophecy](https://egg-inc.fandom.com/wiki/Egg_of_Prophecy?oldid=7373)、[Grain Silo](https://egg-inc.fandom.com/wiki/Grain_Silo)、[Piggy Bank](https://egg-inc.fandom.com/wiki/Piggy_Bank)、[App Store](https://apps.apple.com/us/app/id993492744)
- Idle Miner Tycoon：[GameAnalytics](https://www.gameanalytics.com/blog/making-a-hit-idle-game-eight-lessons-from-kolibri-games)、[Kolibri MVP 访谈](https://www.pocketgamer.biz/feature/70431/how-kolibri-went-from-minimal-viable-product-to-70-million-installs-for-its-idle-games/)、[Kolibri 经济设计](https://kolibrigames.com/blog/economy-in-free-to-play-mobile-games-part-2/)、[Ubisoft 收购](https://news.ubisoft.com/en-us/article/2xWaMgJcJBXYmGEFqi8XeY/ubisoft-acquires-mobile-game-studio-kolibri-games)
- AFK Arena：[Deconstructor of Fun](https://www.deconstructoroffun.com/blog/2019/6/6/afk-arena-puts-lilith-into-the-billionaire-club)、[营收](https://www.pocketgamer.biz/afk-arena-celebrates-seventh-anniversary-and-over-15bn-in-lifetime-earnings)、[挂机奖励](https://www.appgamer.com/afk-arena/strategy-guide/afk-rewards)
- Melvor Idle：[Jagex 公告](https://www.jagex.com/news/jagex-announces-partnership-to-publish-melvor-idle)、[资料片 Steam](https://store.steampowered.com/app/2055140)、[PC Gamer](https://www.pcgamer.com/games/rpg/jagex-partnered-with-a-runescape-inspired-idle-game-to-turn-the-sequel-into-an-official-spinoff/)
- Legends of IdleOn：[Steam](https://store.steampowered.com/app/1476970/Legends_of_Idleon_MMO/)、[新手指南](https://www.digitaltq.com/wiki/idleon/beginner-guide)、[评测汇总](https://vaporlens.app/app/1476970/idle_on.md)
