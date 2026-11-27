# 用七条主曲线给每个数值定价

> 状态（2026-11-27）：调研和提议，**还没有按它改任何数值**，等讨论定下文末“待讨论的决定”再动手。原稿在 `reports/`（不进仓库）。第六节的“组织等级门槛”“潮汐预报”等常数已经从代码挪进 `engine/content/balance.json`（`org_levels`、`org_perks`、`tide_horizon`……），数值没变。

其他游戏配数值的共同做法是：先定少数几条主曲线和一个通用单位，再让每件内容按曲线定价，最后用模拟检查实际进度是否贴着曲线走。放置游戏让同一个生产者每多买一个贵 7%～15%，产出只线性增加，再用 ×2 里程碑和开根号的转生公式周期性地把两条线拉开。战斗类游戏多用除法护甲，因为它让有效血量随防御线性增长，单位强度可以压成 EHP×DPS，多人时取 √(ΣEHP×ΣDPS)；敌人的生命也普遍涨得比伤害快。节奏上一般先定目标时长再反推数值，bot 负责报告到达时间、通过率、资源水位和选择分布。拿这些方法量本仓库，最要紧的发现是：“每环 +8%”只占敌人成长的一小部分，敌人名单换代加上每环波数、每波人数的台阶，让一场遭遇的强度每环跳 ×1.14～×2.01，第 3 环到第 4 环一下翻倍。这和贪心 bot 72 小时停在第 3 环对得上。同一次运行里，第 72 小时积压的废料相当于 78～113 小时的收入，说明成本没有随进度上涨。据此我们提议定义七条主曲线：时间轴、遭遇强度、玩家战力、收入、各系统成本、关注度折算和轮回转换。战斗内容用“环当量”定价，经济内容用“产出小时”定价，再用十四项 bot 指标和通过带检查曲线。

> 阅读说明：第一到第五节是来源事实，标【推导】的是我们用来源公式算出的数字。第六节是对本仓库的读码和一次 bot 实测，标【实测】。第七到第九节是我们的提议，标【提议】，所有起始参数都要用 bot 校准。

## 一、同一生产者每个贵 7%～15%，产出线性增长；加成多数相加，少数里程碑相乘

放置游戏的骨架是 Pecorella 给出的两条式子：**下一个的价格 = 基础价 × 增长率^已拥有数，总产出 = 基础产出 × 拥有数 × 乘数**。AdVenture Capitalist 的柠檬水摊增长率 1.07、基础价 4、每个 1.67/秒，拥有 10 个时下一个卖 7.87，到 25 和 50 个各翻一倍。作者把这种结构叫作“成本指数增长、产出线性或多项式增长”的跷跷板，指数终会甩开多项式，所以要靠平衡手段把等待时间压在可接受范围里（[Pecorella Part I](https://www.gamedeveloper.com/design/the-math-of-idle-games-part-i)）。各家的增长率集中在很窄的区间：Cookie Clicker 每个建筑 ×1.15（[Cookie Clicker Wiki](https://cookieclicker.wiki.gg/wiki/Buildings)）；Kittens Game 大多数建筑 1.15、农田 1.12，住房却特别陡，小屋 2.5、谷仓 1.75（[kittensgame buildings.js](https://github.com/nuclear-unicorn/kittensgame/blob/master/js/buildings.js)）；Eatventure 升级消耗每级 ×1.2、售价每级 ×1.08（[机核拆解](https://www.gcores.com/articles/212641)）；Eric Guan 设计的经济是产出每级 ×1.1、成本每级 ×1.15（[Eric Guan](https://ericguan.substack.com/p/idle-game-design-principles)）。【推导】增长率 r 下价格每 ln2/ln r 次翻倍，r=1.07 约 10 次，r=1.15 约 5 次。如果它是唯一的收入来源，相邻两次购买的等待之比是 r·n/(n+1)：前 1/(r−1) 次越买越快（r=1.15 时约 7 次），之后每次比上次慢，比值逐渐趋近 r。所以 r 同时决定“前期加速能持续几次”和“后期每次慢多少”。里程碑只是打折：AdCap 从 25 个买到 50 个，价格涨 5.43 倍，产出连同 ×2 里程碑只涨 4 倍。

线性成本适合“永远有事可做、不收敛”的线，指数成本适合要收住的线。Factorio 的采矿产能研究按 **2500×(等级−3)** 线性涨价，每级 +10%，加法累计；机器人速度研究则按 **2^(等级−6)×1000** 翻倍，每级 +65%（[Factorio Wiki: Mining productivity](https://wiki.factorio.com/Mining_productivity_(research))，[Worker robot speed](https://wiki.factorio.com/Worker_robot_speed_(research))）。Antimatter Dimensions 用台阶式指数：每买满 10 个，价格乘 1e3 到 1e15 不等（[AD 社区说明](https://raw.githack.com/jacorb90/IvarK.github.io/master/howto.html)）。中文读书笔记里的原则是刚需元素尽量用线性模型，非刚需的可以用非线性（[机核：数值之美](https://www.gcores.com/articles/175805)）。

加成的叠法分两类。数量多、单个小的加成放进同一个加法池：Cookie Clicker 天堂等级每级 +1%（[Heavenly Chips](https://cookieclicker.wiki.gg/wiki/Heavenly_Chips)），AdCap 每个天使 +2%，100 个天使是 3 倍利润（[gameplay.tips](https://gameplay.tips/guides/adventure-capitalist-basic-guide-angels-events-suits-badges.html)）。少数可数的节点才用乘法，例如 Cookie Clicker 每种建筑有 15 个各 ×2 的升级（[Cookie Clicker Wiki](https://cookieclicker.wiki.gg/wiki/Buildings)）。Kittens Game 源码里有两个防爆函数：有上限的 `getLimitedDR`，前 75% 原样生效，剩下 25% 无限逼近上限；无上限的 `getUnlimitedDR`，取三角数的反函数，大值时约等于 √(2v/stripe)（[kittensgame game.js](https://github.com/nuclear-unicorn/kittensgame/blob/master/game.js)）。Eric Guan 提醒，玩家分得出 100 和 120，分不出 100 和 101，所以增长要按当前基线的比例给（[Eric Guan](https://ericguan.substack.com/p/idle-game-design-principles)）。反面教材是 Diablo 4：第三方整理称它的伤害过度依赖乘区，后来只让部分加成保持乘法（[Mobalytics](https://mobalytics.gg/blog/diablo-4/damage-buckets-deep-dive/)）。【推导】加法池自带递减：池里第 k 个 +2% 只带来 0.02/(1+0.02(k−1)) 的相对增益。

新层级的节奏有一个可以直接拿来用的比例。Cookie Clicker 的初始回本时间（基础价 ÷ 基础产出）是 Farm 138 秒、Mine 255 秒、Factory 500 秒、Bank 1,000 秒、Temple 2,564 秒、Wizard Tower 7,500 秒（[Cookie Clicker Wiki](https://cookieclicker.wiki.gg/wiki/Buildings)）。【推导】每开一个新层级，回本时间大约 ×1.7～×2.9，接近翻倍。城建游戏很少用连续公式，多用阈值和分段线性。Anno 1800 的住宅在基本需求全满足时住 10/20/30/40 人，需求 100% 满足才能升级（[Steam 讨论](https://steamcommunity.com/app/916440/discussions/0/4626978782935270832)）。阶层同时也是劳动力来源（[Anno Union](https://www.anno-union.com/en/devblog-residential-tiers/)）。Oxygen Not Included 按“士气 − 期望”决定每周期压力，士气超出期望 2 点以上压力 −20%，每低 1 点 +2.5%（[ONI Wiki](https://oxygennotincluded.wiki.gg/wiki/Morale)）。

## 二、转生公式开根号：指数管每轮提速多少，常数钉住第一次转生在哪里

主流转生公式都取分数次幂：Realm Grinder 按历史最大值取三角根 `(√(1+8c/10¹²)−1)/2`，AdCap 按累计值取 `150·√(c/10¹⁵)`，Cookie Clicker 按累计值取 `∛(c/10¹²)`，Egg, Inc. 按本局取 `(c/10⁶)^0.14`（[Pecorella Part III](https://www.gamedeveloper.com/design/the-math-of-idle-games-part-iii)）。Pecorella 指出两点。第一，按累计值或最大值算的公式自带保护，在同一位置重置几乎没有收益；按本局算的公式没有这层保护，曲线要更平。第二，**转生货币翻倍所需的收益倍数**，Realm Grinder 约 4 倍、AdCap 3～4 倍、Cookie Clicker 8 倍、Egg Inc 128 倍。Egg Inc 曲线这么陡，部分是为了配合它 2 小时的离线上限。他的表格示例里，首次转生约 120 分钟，发生在拥有 500 个生成器、触发 16× 乘数时；他还建议转生周期快慢交替，不要单调（[Pecorella Part III](https://www.gamedeveloper.com/design/the-math-of-idle-games-part-iii)）。【推导】翻倍倍数 = 2^(1/指数)，√ 为 4，∛ 为 8，0.14 次方约 141。公式里的常数负责定第一次转生的位置：Cookie Clicker 第一级要累计 10¹² 块饼干，AdCap 第一个天使约要 4.4×10¹⁰。门槛也可以直接写成规则：Kittens Game 的 karma 在猫数超过 35 只后才开始累计，再取三角根（[kittensgame game.js](https://github.com/nuclear-unicorn/kittensgame/blob/master/game.js)）。这些游戏的本局货币动辄跨好几个数量级，所以要开根号；开根号是用来驯服指数增长的，不是转生本身的要求。

等级曲线有两种思路。一种是先给曲线：RuneScape 和 Melvor Idle 的升级经验每级约 ×1.104，即每 7 级翻倍，92 级的总经验约为 99 级的一半。高级内容给的经验也更多，所以实际耗时没有曲线看起来那么陡（[RuneScape Wiki](https://runescape.wiki/w/Experience)，[Melvor Wiki](https://testwiki.melvoridle.com/w/Exp_table)）。宝可梦用三次多项式，例如 Medium Slow = 6/5·n³ − 15n² + 100n − 140（搜索摘要，[Bulbapedia](https://bulbapedia.bulbagarden.net/wiki/Medium_Slow)）。另一种是中文网游的倒推流程：先问主策划第 1、10、20 级和满级各要多久，拟合出“等级–升级时间”表，再按 **升级经验 = 平均经验效率 × 目标升级时间** 算经验表。平均效率按各产出口的时长加权，例如 50 分钟任务（5 倍）加 30 分钟副本（9 倍），平均 6.5 倍。作者还提醒前期产出要收紧，因为已经发出去的资源收不回来（[白鲸/GameRes](https://www.baijing.cn/article/3362)）。

## 三、除法护甲让战力可以传递，单位强度取 √(ΣEHP×ΣDPS)

中文数值策划最简单的战力是 **HP×ATK**，一般写成“输出 × 承受”。求属性权重有三种办法：用初始属性反推，用“只改两个属性、互砍到同归于尽”的模拟找等价点，或者做回归。同一篇文章还警告，伤害公式里一旦出现 ATK−DEF 这样的减法，战力就成了相对值，可能 A>B、B>C 而 C>A（[CSDN](https://damodev.csdn.net/6a435ed110ee7a33f28499f6.html)）。Lanchester 平方律给了多人时的版本：火力优势要达到人数劣势的平方才能翻盘（[Wikipedia](https://en.wikipedia.org/wiki/Lanchester%27s_laws)）。【推导】一对一时，A 赢当且仅当 H_A·D_A > H_B·D_B，所以 HP×DPS 是决斗的不变量。多人时一方的强度 ∝ 总 HP × 总 DPS，开平方后的 √(ΣEHP×ΣDPS) 随人数线性增长，可以直接相加。胜方剩余兵力约为 √(1−1/ρ)，ρ 是双方乘积之比：ρ=1.5 剩 58%，ρ=2 剩 71%，ρ=4 剩 87%。平方律假设火力集中、单位一死火力就消失；AoE 多或一对一卡位时更接近线性律。

属性和技能定价用的是 Schreiber 的成本曲线。所有效果折成同一种资源，让**收益在数值上等于成本**：+1 攻击值 25 金，+10 攻击的剑就卖 250 金。曲线可以线性（作者说少见）、递增，或在阈值处跳变。推导时先给单效果对象定价，再看组合比各项之和贵还是便宜。几条经验法则：带限制的收益永远不算成本，二选一的价格至少等于较贵的那个，拿不准时**宁可做弱，不要做强**（[Schreiber Level 3](https://gamebalanceconcepts.wordpress.com/2010/07/21/level-3-transitive-mechanics-and-cost-curves/)）。公共资源要选得贴近玩法，D&D 武器用平均骰伤，Halo 武器用 DPS（同上）。中文团队的做法是选一个标杆，例如以法师火球为 100%（[GameRes 七步做数值](https://www.gameres.com/847186.html)）。克制关系不能放在成本曲线上，要解收益矩阵求目标出场比例。石头剪刀布里石头赢时得 2 分，解得布的出场率是另两个的两倍（[Schreiber Level 9](https://gamebalanceconcepts.wordpress.com/2010/09/01/level-9-intransitive-mechanics/)）。

攻防公式决定上面这些能否成立。英雄联盟护甲按 **100/(100+护甲)** 减伤，有效血量 = (1+护甲/100)×生命，每点护甲 +1% 有效血量，没有递减（[LoL Wiki: Armor](https://wiki.leagueoflegends.com/en-us/Armor)）；云顶之弈沿用同一式子（[TFT](https://wiki.leagueoflegends.com/en-us/Champions_(Teamfight_Tactics))）。Dota 2 在 7.20 版改过护甲公式，7.27 版又改回 0.06×护甲/(1+0.06×护甲)（摘要，[Liquipedia](https://liquipedia.net/dota2/Archive:Version_7.20)，[Dota 2 Wiki](https://dota2.fandom.com/wiki/Version_7.27)）。明日方舟物理伤害用减法，保底为攻击的 5%（[Arknights Wiki](https://arknights.wiki.gg/wiki/Damage/Physical)）。Path of Exile 的护甲按单次受击大小打折，物理减伤上限 90%（常数各来源不一致，[Mobalytics](https://mobalytics.gg/poe-2/guides/armour)）。GameRes 的看法是减法利于拉开装备和付费差距，除法利于体现职业差异、控制付费影响（[GameRes](https://www.gameres.com/847186.html)）。【推导】用除法时有效血量与攻击方无关，战力能拆成“攻击因子 × 防御因子”，具有传递性。每点防御的价值正比于生命，所以生命和防御天然互补：LoL 维基给的平衡点“生命 = 7.5×(护甲+100)”意味着一点护甲约值 7.5 点生命。百分比抗性从 50% 到 75% 就让有效血量翻倍，必须封顶。

稀有度和星级，公开得最清楚的是云顶之弈：**每升一星生命 ×1.8、攻击 ×1.5**，要 3 张同名卡（[LoL Wiki: TFT](https://wiki.leagueoflegends.com/en-us/Champions_(Teamfight_Tactics))）。9.24B 补丁把 3 星从 2 星的 ×2 降到 ×1.8，理由之一是让 2 星和 3 星用同一条规则（[TFT 9.24B](https://teamfighttactics.leagueoflegends.com/en-gb/news/game-updates/teamfight-tactics-patch-9-24b-notes/)）。通用的稀有度模型有线性 1+k·稀有度（k 取 0.2～0.3）和指数 c^稀有度两种（摘要，[CSDN/GitCode](https://gitcode.csdn.net/6a0c712d10ee7a33f273c081.html)）。【推导】升一星让 HP×DPS ×2.7，开方后 ×1.64。按平方律，3 个 1 星（9dh）远强于 1 个 2 星（2.7dh），所以升星有价值的前提是上场人数有上限。

## 四、敌人生命比伤害涨得快，难度呈“区内平、跨区跳”的锯齿

Risk of Rain 2 的公式最完整：难度系数 = (人数项 + 分钟数 × 0.0506 × 难度值 × 人数^0.2) × **1.15^已过关数**。敌人每升一级加基础值 30% 的生命、20% 的伤害。刷怪点数随系数线性增长，精英的价格是普通怪的 6 倍或 36 倍，宝箱价格按系数的 1.25 次方涨。开发者后来把所有难度的随时间增速提高 10%，说明是“不希望游戏突然变得简单很多”（[RoR2 Wiki](https://riskofrain2.wiki.gg/wiki/Difficulty)）。【推导】总威胁约为系数的二次方，收入只与系数线性相关，商品价格按 1.25 次方涨，所以拖延会受罚。Diablo III 的 Torment 从 T1 的生命 819%/伤害 396% 涨到 T16 的 13,888,770%/64,725%（摘要，[Diablo Wiki](https://di.diablowiki.net/T4)）；【推导】平均每档生命 ×1.91、伤害 ×1.41。BTD6 从第 81 关起，MOAB 类气球每关生命 +2%（摘要，[Bloons Wiki](https://bloons.fandom.com/wiki/Late_Game_and_Freeplay_(BTD6))）。这些例子的共同点是生命涨得比伤害快：战斗时间拉长，但不会突然把玩家秒掉。

另一维是数量和预算。植物大战僵尸按点数预算刷僵尸，越危险越贵（社区描述，[EA 论坛](https://forums.ea.com/discussions/plants-vs-zombies-3-discussion-en/pvz-3---change-how-wave-progression-works/10297487)）。RimWorld 的袭击点 = (财富点 + 殖民者点) × 难度 × 开局因子 × 适应因子。财富到点数是分段线性：14,000 以下为 0，400,000 时 2,400，1,000,000 时 4,200 封顶。开局因子从第 10 天的 0.7 爬到第 40 天的 1.0，适应因子在 0.4～2.0 之间，殖民者死亡会把它往下拉（[RimWorld Wiki](https://rimworldwiki.com/wiki/Raid_points)）。肉鸽游戏把难度拆成可单独开关的修正，不用一个全局倍率。Slay the Spire 的进阶依次提高普通怪、精英、boss 的攻击，再依次提高它们的生命，A20 连打两个 boss（[Pocket Gamer](https://www.pocketgamer.com/slay-the-spire/ascension-guide)）。二代 A8 的一个例子是精英生命 127→132（[Destructoid](https://www.destructoid.com/all-ascension-levels-in-slay-the-spire-2-and-how-they-work/)）。Hades 的 Hard Labor 每级让敌人伤害 +20%（[RPGSite](https://www.rpgsite.net/feature/10287-hades-pact-of-punishment-heat-modifiers-and-how-to-maximize-your-rewards)）。Schreiber 把这些归纳成两个工具。一是感知难度 = (技巧挑战 + 战力挑战) − (玩家技巧 + 玩家战力)。二是“锯齿”：进新区域时难度跳升、区内持平、boss 处尖峰，玩家战力则在区内小步上升、过 boss 时跳升；阶段之间的过渡应逐渐变长（[Schreiber Level 7](https://gamebalanceconcepts.wordpress.com/2010/08/18/level-7-advancement-progression-and-pacing/)）。

## 五、先定目标时长再反推数值，用多种 bot 找离群点

能直接算的节奏指标有这几个。Cookie Monster 插件对每个购买算一个回本指数，**PP = max(成本−存款, 0)/每秒产出 + 成本/Δ每秒产出**，也用它比较买 1 个和买 10 个（[CookieMonster](https://github.com/CookieMonsterTeam/CookieMonster)）。有人批评第二项没有扣掉已有存款（[Riimu issue #19](https://bitbucket.org/Riimu/cookie-clicker-optimizer/issues/19/correcting-the-efficiency-formula-to)）。Pecorella 用“收入增量/成本”模拟最优买法，但承认按每秒收入优化会过度偏好昂贵的高层生成器（[Part I](https://www.gamedeveloper.com/design/the-math-of-idle-games-part-i)）。回访节奏方面，Eric Guan 的设计是首次游玩 15～60 分钟，回访频率从每小时一次降到每天几次、再到每周一次；三层计时器的满仓周期分别是 20 分钟、5 小时、2 天（[Eric Guan](https://ericguan.substack.com/p/idle-game-design-principles)）。离线上限差别很大：Egg Inc 2 小时（[Pecorella Part III](https://www.gamedeveloper.com/design/the-math-of-idle-games-part-iii)），Clicker Heroes 不设上限（[Clicker Heroes Blog](https://blog.clickerheroes.com/?p=1902)）。货币管理上，GameRes 用“水池模型”：注水是产出，放水是消耗，警戒线是温饱线，阀门是调控手段（[GameRes](https://www.gameres.com/847186.html)）；白鲸的经验是基本消耗约占收入的 90%（[白鲸](https://www.baijing.cn/article/3362)）。一份无作者的教程把相邻等级耗时比 Tₙ/Tₙ₋₁ > 1.5 当作“成长断层”（低可信，[numeric_planner_tutorial](https://zsc.github.io/numeric_planner_tutorial/chapter2.html)）。

模拟验证的一手材料都在强调两件事：bot 擅长找离群点，看不出手感；结论高度依赖 bot 的策略。EA 和 NYU 在 The Sims Mobile 上用 A* 和 Softmax 两种 agent，报告达成目标所需行动数、会话间等待时间和多次运行的方差。结果查出一个恋爱事件要约 13 次行动，其他事件只要 3～4 次；物品只省了 5%/26%/20% 的行动；B 版本比 A 版本多花四倍以上行动，会话间等待却从约 6 小时降到约 45 分钟（[Sims Mobile 论文](https://ar5iv.arxiv.org/html/1811.06962)）。King 用 bot 在新关卡上线前预测通过率，产品经理说它“擅长识别离群点、预测通过率”，但抓不住关卡的手感；难度也被刻意设计成起伏，而不是单调上升（[PocketGamer.biz](http://www.pocketgamer.biz/crafting-candy-crushs-difficulty-blockers-level-design-ai-and-the-complexity-staircase/)）。Mega Crit 从原型期就记录选取率、胜局出现率和对每个敌人的平均受伤，图表增加到至少 90 张。Dual Wield 和 Awakened One 都是按数据改的，但他们也说“数字不告诉我们手感如何”（[Game Developer](https://www.gamedeveloper.com/design/how-i-slay-the-spire-i-s-devs-use-data-to-balance-their-roguelike-deck-builder)）。一项 Machinations 研究里，加入玩家策略后，模拟胜率从约 40% 变成 66%～81%（[JIS 2024](https://journals-sol.sbc.org.br/index.php/jis/article/download/4186/2682)）。学界把“目标曲线变成检查”写成了方法：设计者画一张目标图，优化器搜索参数，使模拟结果到目标图的距离最小（[Gbadamosi & Goodman](https://arxiv.org/abs/2006.04419)）。数据落地方面，Unreal 的 CurveTable 每行一条命名曲线，按列插值；GAS 的 FScalableFloat 取 `Value × Curve[Level]`，也就是“一个系数加一条共享曲线”（[Epic 文档](https://dev.epicgames.com/documentation/en-us/unreal-engine/data-driven-gameplay-elements-in-unreal-engine)，[FScalableFloat](https://dev.epicgames.com/documentation/en-us/unreal-engine/API/Plugins/GameplayAbilities/FScalableFloat)）。

---

> 以下第六到第九节是我们自己的读码、实测和提议，不是来源事实。

## 六、我们的现状：遭遇强度每环跳 ×1.1～×2.0，废料积压八十小时以上的收入

【实测】读码可以确认几件事。`combat::enemy_spec` 让敌人的 hp 和 atk 乘 1+0.08·(环−1)，def 每两环 +1，spd 不变。`mapgen` 按环决定遭遇结构：第 1～2 环 1 波，第 3～5 环 2 波，第 6～7 环 3 波；第 1～3 环每波 2 只，第 4 环起每波 3 只。spd 只用来排出手顺序（`turn_order`），不是攻击频率。队员稀有度每升一档，每项属性 +4，折算下来每档约 +2.7 hp、+0.7 atk、+0.5 spd，外加特质格 1/2/2/3。组织经验 = 本轮成果 1:1；等级门槛 0/100/250/500/900/1,400/2,000，4 级起约等于 56·(等级−1)²，是二次曲线。研究成本 10+18n，累计约 9n²，所以抽数随 √(情报×时间) 增长。设施和雇佣都是固定价格，不随购买次数上涨。

我们给每个单位算了一个强度指数 P = √(EHP×atk)，EHP = hp×(1+def/45.5)，取每环可出现的城市敌人的平均值。一波的强度取 S = 人数 × P。波与波之间血量延续，所以按平方律，一场遭遇的强度 D = √(Σ 各波 S²)。基础小队（先锋、射手、医疗兵）的 S≈140，ρ = (140/D)²：

| 环 | 平均单体 P | 波数×每波 | 遭遇强度 D | 比上一环 | 基础小队 ρ |
|---|---|---|---|---|---|
| 1 | 24.1 | 1×2 | 48 | — | 8.4 |
| 2 | 31.1 | 1×2 | 62 | ×1.29 | 5.1 |
| 3 | 38.8 | 2×2 | 110 | ×1.77 | 1.6 |
| 4 | 51.9 | 2×3 | 220 | ×2.01 | 0.40 |
| 5 | 61.9 | 2×3 | 263 | ×1.19 | 0.28 |
| 6 | 74.7 | 3×3 | 388 | ×1.48 | 0.13 |
| 7 | 85.0 | 3×3 | 442 | ×1.14 | 0.10 |

单体强度每环约 ×1.23，“+8%”只占其中一小部分，其余来自名单换代。真正的台阶在波数和每波人数：第 3 环 ×1.77，第 4 环 ×2.01。这张表没算技能、标签、特质和装备，只是粗算，但它和下面 bot 停在第 3 环对得上。

【实测】用现有贪心 bot（`cargo run --release --example bot -- 3 72`，地图 0，3 个种子）跑 72 小时：

| 小时 | 占地 | 最深环 | 队员 | 废料库存 | 废料收入/时 | 来潮守住/失守 | 本轮成果 |
|---|---|---|---|---|---|---|---|
| 6 | 8 | 1 | 3 | 288～366 | 48～66 | 3～4 / 2～3 | 33～39 |
| 12 | 11 | 2 | 3 | 682～1,000 | 24～72 | 10～14 / 4～7 | 58～61 |
| 24 | 17 | 2 | 3～5 | 1,625～2,163 | 33～84 | 26～36 / 16～24 | 87～110 |
| 48 | 29 | 3 | 8～9 | 3,467～5,001 | 60～90 | 63～109 / 31～49 | 177～221 |
| 72 | 41 | 3 | 9～11 | 6,770～8,702 | 60～94 | 96～176 / 69～93 | 226～322 |

这次运行能读出四点。第一，废料收入第 6 小时后基本持平，库存却一路上涨，到第 72 小时相当于 78～113 小时的收入。按水池模型，这是缺少会随进度增长的消耗口。第二，全轮失守率 28%～49%。第三，仅“守住的来潮”一项就占本轮成果的 42%～55%，和设计稿说的“各路速度相近”不符。第四，三个种子在各个时间点的占地数完全相同。这可能说明扩张速度目前由行军和路线时间决定，而不是经济或战斗，需要单独核实。只有一种 bot 策略和 3 个种子，以上都只能算基线，不能当结论。

## 七、提议七条主曲线：时间轴、遭遇、战力、收入、成本、关注度、轮回

【提议】核心思路是 Schreiber 的锯齿加白鲸的倒推：先写下“标准 bot 什么时候到哪里”，再写下“那里有多难”，两者相除，就得到玩家在每个时刻需要多强、需要多少收入。新系统只需要声明自己从哪条曲线上分走多少份额。曲线参数集中放进 `balance.json` 的一个 `curves` 段，用分段线性锚点（Unreal CurveTable 的 Linear 模式），沿用现有的 bp 整数写法。单项内容只存相对曲线的系数，类似 FScalableFloat。

| 曲线 | 含义 | 函数形式 | 起始参数 | 依据 |
|---|---|---|---|---|
| τ(r) 时间轴 | 标准 bot 首次占下第 r 环的小时数 | 分段线性锚点，段长每环 ×1.3～1.7 | 2/5/10/18/30/48/72 h（第 7 环指可以尝试核心） | Schreiber“过渡逐渐变长”；Cookie Clicker 每层回本约 ×2；白鲸先定时长 |
| D(r) 遭遇强度 | 一次路线遭遇的 √(Σ波 S²) | D₁·g^(r−1)，各波按预算填怪 | D₁≈50，g≈1.4（现状几何平均 1.45，但台阶不均） | RoR2 每关 ×1.15；D3 每档 ×1.4～1.9；PvZ、RoR2 点数预算 |
| P(r) 单体敌人（D 的分解） | 单个敌人的 √(EHP×DPR) | 角色系数 × P₁·q^(r−1)；成长的 60% 给 HP、40% 给攻击 | q 由 D 和每波人数反推，约 1.2～1.25 | RoR2 每级 +30% 生命/+20% 伤害；D3 生命涨得快 |
| S*(t) 玩家战力 | 标准 bot 主力小队的目标 √(ΣEHP×ΣDPR) | S*(t) = D(τ⁻¹(t))·√ρ_入环 | 刚入环时 ρ 取 1.3～1.6 | Lanchester；Schreiber 交叉点 |
| I(t) 收入 | 每小时产出的“工时当量” | 分段线性，跟随在岗人口 | 锚点取 bot 基线；1 工时 = 3 废料 = 1 零件 = 1 情报 | 白鲸“效率 × 时长” |
| C(n) 成本 | 某系统第 n 次购买的价格 | H × I(t_n) × r^n | 同区同类设施 r=1.15；升级线 r=1.15～1.2，每级产出 +8%～10%；研究维持线性 | Pecorella；Cookie Clicker、Kittens；Factorio；Eatventure、Eric Guan |
| κ 关注度折算 | 一级关注度相当于多少环 | 有效环 = r + κ·关注度 | κ≈0.28，与现行 +10% 等价 | RimWorld 威胁分段、开局爬坡与适应因子 |
| X 轮回转换 | 本轮成果→组织经验 | 近线性加入门槛；等级门槛按“第 k 轮后到几级”拟合 | 门槛约为首轮中位成果的 30% | Pecorella III；Kittens karma 门槛；白鲸倒推 |

遭遇强度是第一优先级。按 D₁=50、g=1.4，各环 D 为 50/70/98/137/192/269/376；要在刚入环时 ρ=1.5，小队 S 需达到 61/86/120/168/235/329/461。基础小队 140 能轻松拿下前 3 环，第 4 环要 +20%，第 7 环要 ×3.3。这样第一轮的战力成长约为 ln(461/140)/ln1.4 ≈ 3.5 个环当量。取 g=1.4 而不沿用 1.45，是为了让第一轮战力要涨的倍数还够得着，同时比现在的平均台阶略缓。实现上，建议把波数和人数的台阶交给预算：每波先给预算，再由 `roll_wave` 按敌人的 P 填到预算为止，换人数的那一环用单体强度补平，让每环的 D 比值都落在 1.3～1.6。生命和攻击按 60/40 分配，是照 RoR2 每级 30%/20% 的比例，Diablo III 生命也明显涨得比伤害快。我们的除法护甲同样让“击杀时间”主要由生命决定，这样分配可以拉长战斗、不突然秒人。关注度不再单独写一个倍率，改成“让来潮按更深的环算”，这样它和 D(r) 用的是同一条曲线。RimWorld 的适应因子提示了一个可选的负反馈：最近连续失守时，来潮暂时减弱。

玩家战力 S*(t) 是派生曲线，不手写。它的用处是做“投放比例”：第一轮约 3.5 个环当量的成长，建议起始分给条令/研究 30%、装备 25%、招募与稀有度 20%、特质成长 15%、训练和设施 10%。按基线一轮约 6～7 抽，每抽约值 0.15 环。组织等级和特别队员这些跨轮成长另算一份。设计稿要求“轮回只把内容池变宽，不用来锁核心系统”，所以建议跨轮成长对时间轴的压缩有上限，例如组织 6 级时 τ 最多 ×0.75。

经济和成本的提议都围绕一个事实：我们的瓶颈是人口和住房，不是废料。所以收入用“工时”计。按现行每人每小时产出，1 工时 = 3 废料 = 1 零件 = 1 情报，晶体的汇率留待决定。价格写成“预期购买时刻的 H 小时收入”，H 由系统类别决定。同区同类设施每多一个 ×1.15，用的是 Cookie Clicker 和 Kittens Game 的默认值，约每 5 个翻倍。如果要做“永远有得花”的废料出口，就照 Eatventure 和 Eric Guan 的比例做设施升级线：成本每级 ×1.15～1.2，产出每级 +8%～10%，加法累计。研究成本维持 Factorio 式的线性，前提是研究效果是小额加法；如果条令出现乘法效果，就改成 ×1.10～1.15 的指数，或者 Antimatter Dimensions 式的每 k 抽跳一档。

轮回转换不必照抄放置游戏的开根号。【推导】那些游戏的本局货币跨好几个数量级，必须用根号压缩；我们的成果在基线里大约按 t^1.2～t^1.6 增长，是多项式，近线性转换就够了。真正要管的是两件事。一是各路成果的配比。二是“每小时组织经验”应在目标轮长附近达到峰值，让玩家自己决定的收束时机自然落在约 72 小时，而不是 24 小时速刷或无限拖延。工具有三个：Kittens 式的门槛（成果低于门槛不给经验）、各路成果的权重，以及随关注度上升的来潮压力。组织等级门槛改用白鲸的倒推：先写“第 k 轮后应到几级”，再拟合。按现行近二次的门槛和每轮约 280 的成果，第 5 轮左右就能到 6 级，这个速度是否合适要先拍板。

## 八、提议两种通用单位：“环当量”给战斗内容定价，“产出小时”给经济内容定价

【提议】战斗侧的通用单位是**环当量**：小队强度 S = √(ΣEHP×ΣDPR) 每乘一次 g（起始 1.4），就记 1 环。EHP = hp×(1+def/45.5)，治疗和护盾按期望回合数折进去。DPR 是每回合期望伤害，按 atk × 技能倍率期望计算，状态标签和反应按“期望额外伤害/回合”或“期望减伤”折进 DPR 或 EHP，控制效果按“对方失去的出手回合”折进去。spd 在我们的引擎里只决定先手，不能像塔防模型那样乘进 DPS；它的价格用 Schreiber 和 CSDN 的等价点模拟来测：同一对阵只改 spd 或只改 hp，找到胜负翻转的点。在参考队员（hp 100、atk 19、def 6）身上，边际价格约为 1 atk ≈ 5.3 hp、1 def ≈ 1.9 hp。一般式是 1 atk ≈ hp/atk 点 hp，1 def ≈ hp/(45.5+def) 点 hp，和 LoL 的“护甲 = 7.5 生命”同理。以基础小队（S≈140）为例：

| 内容 | 变化 | 环当量（g=1.4） |
|---|---|---|
| 一名队员 +4 atk 的特质 | ΣDPR 55→59 | 0.10 |
| 一名 def 6 的队员 +20 hp | ΣEHP +22.6 | 0.09 |
| 先锋 +2 def | ΣEHP +5.8 | 0.02 |
| 一名队员从 N 换成 SSR（现行属性部分） | +8 hp、+2 atk、+1 def | 0.10（另加 2 个特质格） |
| 全队 atk +10% 的条令 | ΣDPR ×1.1 | 0.14 |

这张表也说明了现状：一个 SSR 的属性红利只相当于一个普通特质。稀有度要不要更值钱，列在文末的待决事项里。经济侧的通用单位是**产出小时**：价格除以当时的标准收入。生产类购买再看回本时间 cost/Δ收入，同类第 n 个约 ×1.15^n，新层级的回本时间约是上一层的 ×1.7～×3。两种单位可以经时间轴互换：处在第 r 段时，1 环当量约等于该段时长 d_r 小时的进度。例如第 4→5 环一段约 12 小时，0.1 环的东西约等于省下 1.2 小时。所以任何新内容最后都能写成“为标准玩家省下多少小时”，跟收入、战力还是便利都无关。

落地建议有四条。第一，所有职业的基础 P 收在 ±5% 以内，现在是 42～53，相差约 ±12%；差异只体现在形状（前排偏 hp、后排偏 atk）和技能上。第二，每个敌人在它出现的各环上，P 要落在 P(r)×角色系数的 ±15% 内，并写成 `cargo test` 里的检查。第三，确实要偏离曲线的内容写一个数值字段 `off_curve_bp`，理由写进文档；`CLAUDE.md` 规定内容 JSON 只放 id 和数字。第四，仿照 `text_snapshot` 加一个“数值快照” example，把每件内容的环当量和每个价格的产出小时打成表并提交，改曲线时看 diff，相当于 GameRes 说的验算表。输出只用指标 id 和数字，不新增中文串，避免触发 i18n 棘轮。反应和克制类内容不进成本曲线：按 Schreiber Level 9 和 Gbadamosi 的思路，给它们写目标出场比例或目标胜率矩阵。

## 九、提议十四项 bot 指标和通过带

【提议】每项检查都跑固定的 20 个种子，取中位数和 p10/p90。引擎是确定性的，失败可以逐个复现。至少用三种策略：现有贪心 bot、只投一路（经营/守城/探索）的专精 bot、保守或带随机的 bot，再用文字版做 LLM 试玩，作为手感兜底。通过带先用“大家觉得手感对”的版本跑出基线，再围绕基线收紧，不要先拍一个数。改数值时主要看 A/B 对比，不把绝对值当真理，这是 Sims Mobile 论文的做法。

| # | 指标 | 起始通过带 | 当前基线 | 思路来源 |
|---|---|---|---|---|
| 1 | 首次占下第 r 环的时间 | 中位数在 τ(r) ±20%；p90/p10 ≤ 1.6 | 第 3 环在 48～72 h，第 4 环未到 | Sims Mobile 到达代价；白鲸实测对照 |
| 2 | 相邻段长比 | 1.2～2.0 | 待测 | Cookie Clicker 层间约 ×2；低可信的 1.5 断层规则 |
| 3 | 遭遇台阶 D(r)/D(r−1)（静态检查） | 1.3～1.6 | 1.14～2.01 | RoR2、D3 每档比例固定 |
| 4 | 刚入环时的 ρ | 1.2～2.0；第 1～2 环 ≤ 6 | 第 1 环 8.4，第 3 环 1.6，第 4 环 0.40 | Lanchester；Schreiber 交叉点 |
| 5 | 路线胜率 | 前沿环 60%～85%，前沿内两环 ≥ 95% | 待测 | King 通过率；StS 分层胜率 |
| 6 | 强度指数的预测力 | ρ ≥ 1.5 胜率 ≥ 90%，ρ ≤ 0.7 胜率 ≤ 10%；否则 DPR/EHP 漏算了技能或标签 | 待测 | 逻辑函数 1/(1+ρ^−s) 拟合 |
| 7 | 来潮失守率 | 全轮 10%～25%；内圈 ≤ 5%；前沿 15%～35% | 全轮 28%～49% | RimWorld 适应因子；Schreiber 锯齿 |
| 8 | 资源库存 / 每小时收入 | 中位 2～10 h；连续 12 h 以上超过 24 h 即失败 | 废料 78～113 h | GameRes 水池；白鲸“消耗约占收入 90%” |
| 9 | 下次有意义购买的等待 | 0～72 h 中 ≥ 90% 的整点 ≤ 4 h（一个来潮间隔） | 待测 | Cookie Monster PP；Eric Guan 回访节奏 |
| 10 | 新层级回本时间比 | ×1.7～×3 | 待测 | Cookie Clicker 建筑表 |
| 11 | 三路成果占比 | 经营/守城/探索各 20%～45%；专精 bot 总分 ≤ 贪心 bot 的 115% | 仅守住来潮一项就占 42%～55% | 设计稿“各路速度相近”；Schreiber 高于曲线 |
| 12 | 每小时组织经验 | 峰值落在 48～96 h；24 h 时 ≤ 峰值的 70% | 待测 | Pecorella“等待过长即转生信号” |
| 13 | 选择分布 | 每个研究、条令、设施、职业被提供时的选取率在 5%～60% | 待测 | Mega Crit 选取率 |
| 14 | 乘数预算 | 每个最终数值的 Σln(乘数) 不超过预算，新系统不超过分到的份额 | 待测 | Diablo 4 乘区教训；GameRes 叠加方式 |

第 4 项和第 6 项要先用 `examples/battle.rs` 做一次标定。设计稿规定战斗本身没有随机，所以 s 拟合出来的是阵容和敌人组合的离散度，不是骰子。拟合出的 s 越大，说明 ρ 这一个数越能预测胜负。第 9 项和回本时间如果将来显示给玩家，按 `CLAUDE.md` 要同时做进文字版和 `cli.json`，否则 LLM 试玩看到的信息和网页不一致。

## 结论

这次研究改变了我们对“该调哪里”的判断。`balance.json` 里那些 bp 参数看起来是主旋钮，但真正决定难度形状的是写死在 `mapgen` 里的波数、人数台阶和敌人名单换代；废料积压也不是产出太高，而是所有价格都是定值，没有挂在收入上。所以第一步不是改 800 或 1000 这类数，而是把隐含曲线显式化：遭遇按预算填，价格按收入小时定。之后才轮到调参数。

第二个收获是通用单位的选法。“环当量”和“产出小时”都能换算成“为标准玩家省下多少小时”，于是战斗、经济、便利三类内容第一次可以放在同一把尺子上比较。4 小时一次的来潮又给了一个现成的节奏刻度。另一点要反过来说：放置游戏开根号的转生公式不必照搬，我们的成果是多项式增长，需要管的是各路配比和“何时收束最划算”，而不是驯服指数。

## 待讨论的决定

1. **时间轴锚点**：一轮按 72 游戏小时（bot 和文字版的默认值）还是设计稿写的约一周？第一轮标准 bot 应到第几环？提议第 6 环，并能尝试核心。
2. **遭遇曲线**：g 取 1.35～1.45 中的哪个？是否把波数和人数台阶改成按预算填怪？
3. **战力口径**：内部统一用 √(ΣEHP×ΣDPR) 还是乘积？spd 是否只管先手？
4. **稀有度的分量**：保持现在每档约 0.03 环（N→SSR 约 0.1 环/人），还是提到每档 0.2～0.3 环？
5. **废料出口**：同类设施递增（r=1.15）、设施升级线，还是维护费？选一到两种。
6. **研究成本形态**：保持线性还是改指数或台阶？取决于条令效果是加法还是乘法。
7. **轮回转换**：维持 1:1 加门槛，还是改为按各路的历史最佳值？跨轮成长最多能把时间轴压缩多少？提议组织 6 级时不超过 ×0.75。
