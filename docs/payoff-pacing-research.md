# 用事件时间轴把爽点排进一周

> 状态（2026-11-27）：调研和提议。已定：一轮按一周排；组织等级是周回成长、局内不升级，局内成长靠研究学说；按第七节先做爽点时间轴的测量工具（只测，不改数值），再做战斗门槛和学说幅度，最后做规章解锁。文末“待拍板的决定”其余各项还没定。原稿在 `reports/`（不进仓库）。

其他游戏的爽点几乎都不靠平滑成长。它们的做法是：让成本或难度按指数上涨，再在门槛处给出离散的乘法跳变。放置游戏在数量里程碑上给 ×2（AdVenture Capitalist 单个生意从 25 个到 400 个共 6 次，合计 ×64），在整数关口给 ×3～×5；新系统和新层先密后疏，Antimatter Dimensions 每个新阶段要几小时到几天；自动化作为“这件事已经亲手做够了”的奖励发放；轮回制造“暂时的结局”，让下一轮能轻松跑过前段。能落到数字上的节奏模型只有四个：L4D 的 AI Director、Lopez 的峰谷图、Stout 的强度坡道和 PaceMaker。它们都可以归结为“一条强度序列加几条形状约束”：开头要有钩子，峰值要递增，强峰之前要有休息，奖励率不能突然下降。但没有任何来源给出墙该多长、跳变该多大、解锁该隔多久的权威数值，能当起点的只有 Eric Guan 举例的 ×1.2 可感差和衰减式的回访节奏。用这把尺子量我们的原型，现状是一条没有锯齿的曲线：参考玩家 `greedy` 的小队强度从第 6 小时到第 48 小时只涨了 10%（log2 幅度 0.14，低于 ×1.2）；得分速率在第 6～8 小时见顶，第 24 小时已跌到峰值的 0.66；规章开局就全开，“自动化解放”从来没有作为事件发生过；研究不研究，成果只差 1%～2%。我们提议在节奏模型旁边加一条“爽点事件时间轴”。学说、过门槛、进新环、街区连片、防线自给、新系统和规章的解锁、轮回，每类事件都配一个检测钩子、一个对数幅度和一条预测到达时间的公式，用机器人的事件日志验证，再用回访命中率、最长无爽点间隔、峰值递增、锯齿度、墙长这些指标检查形状，所有阈值都先只登记不判定。最关键的一条推导是：研究价格 10+18n 的累计值和吞吐量下限 4r(r−1)+1 都是二次的，所以只要情报收入不变，每进一环大约正好有 1.4 次学说。如果战斗按每环 ×g 卡扩张，并且只靠学说破墙，每次学说平均要让输出涨 g^(1/1.43)：g=1.4 时约 ×1.27，而现在最强的属性学说只有约 ×1.16。缺的是幅度，不是频率。战斗门槛本身就是放大器：离散的成长撞上清晰的门槛，阈下的数值变化就会变成“预演由红变绿”“进新环”这样能感觉到的事件；平滑的成长只会让格子一格一格地翻绿。

> **阅读说明**：第一到第五节是来源事实，每条附链接；标【摘要】的只读到搜索摘要，没能打开原文核实。其中标【推导】的数字是我们用来源公式或来源数字算出来的，来源里没有。第六节是本仓库的现状：【实测】是用户给出的或评估工具测得的读数，【读码】是代码里的事实。第七、八节是提议，标【提议】，里面的阈值和区间全部需要校准。以下几点是项目负责人已经定下的前提，本报告不再讨论：一轮按一周排；参考玩家是 `greedy`；扩张门槛改为由战斗强度卡（这次只调研，不实现）；组织等级是跨轮回的成长，一周之内不变；局内成长靠研究学说（12 个学说，三选一，价格 10+18n 情报）；规章目前开局就能全部打开。调研笔记里有两处推导和这些前提冲突，本报告作废或改写：一处是“把组织等级 2～6 级排进这一周”，改写成第五节的跨轮回节奏；另一处把“组织等级”“编制”算作局内的爽点线，这一点不再成立。

## 一、爽点是门槛上的离散乘法跳变，可感的起点约是 ×1.2

放置游戏发爽点的标准手法，是在数量门槛上给离散的乘法倍率。AdVenture Capitalist（AdCap）的柠檬水摊，下一个的价格是 **基础价 × 1.07^已拥有数**，产出只随数量线性增长。它在拥有 25、50、100、200、300、400 个时各给一次速度翻倍，合计 ×64；500～900 个之间每 100 个 ×4；1000 和 2000 个各 ×5。报纸生意到 500 个时，还会给虾船一个 ×11 的交叉加成（[AdCap Wiki: Unlocks](https://adventure-capitalist.fandom.com/wiki/Unlocks_(Earth))）。Cookie Clicker 每种建筑有 15 档“twice as efficient”升级，在拥有 1、5、25、50、100、150……600 个时依次解锁（[Cookie Clicker Wiki: Upgrades](https://cookieclicker.wiki.gg/wiki/Upgrades)）。Antimatter Dimensions（AD）每买满 10 个给 ×2，每次 Dimension Boost 再给 ×2（[AD 源码](https://github.com/IvarK/AntimatterDimensionsSourceCode/blob/master/src/core/dimensions/antimatter-dimension.js)，[dimboost.js](https://github.com/IvarK/AntimatterDimensionsSourceCode/blob/master/src/core/dimboost.js)）。连续的小加成也有，只是量级小得多。Trimps 的 Coordination 每买一次，出战人数 ×1.25（[Trimps 源码](https://github.com/Trimps/Trimps.github.io/blob/master/config.js)）；打一张同级地图伤害 +20%，最多叠到 +200%（[Trimps Wiki: Maps](https://trimps.fandom.com/wiki/Maps)）。Pecorella 发现最优买法会扎堆在 25 和 50 个附近，因为倍率让这时的生产者“temporarily overvalued”，把产出推到“closer to, or ahead of, costs”（[Pecorella Part I](https://www.gamedeveloper.com/design/the-math-of-idle-games-part-i)）。

【推导】里程碑为什么有效，算一下就清楚。线性生产下，已有 n 个时再买 1 个，收入只涨 1/n：n=5 时还有 +20%，n=24 时只剩约 4%。第 25 个的 ×2 把一串感觉不到的小增量，一次换成翻倍。墙的来历也在同一组数里。从 25 个买到 50 个，单价涨 1.07^25≈5.4 倍，产出连同 ×2 一共涨 4 倍；从 100 个到 200 个，单价涨约 868 倍，产出还是只涨 4 倍。也就是说，里程碑在前段追得上成本，到后段就追不上了。

能感觉到多大的变化，资料只给了几个参照点，没有专门的研究。心理物理学的 Weber 定律说，可察觉的差异与原刺激成正比，重量大约要变 5% 才能被可靠察觉；但它只在中等强度附近成立（[Wikipedia: Weber–Fechner law](https://en.wikipedia.org/wiki/Weber%E2%80%93Fechner_law)）。Eric Guan 把这个原理用到放置游戏上：5 和 6 个豆子、100 和 120 个豆子都能分出来，100 和 101 分不出来。他自己的经济却是每级产出 ×1.1、成本 ×1.15。有读者在评论里问，既然例子说约 20% 才有感，为什么用 ×1.1，这两个数测过没有；文章没有回答（[Eric Guan](https://ericguan.substack.com/p/idle-game-design-principles)）。Stout 说“从强度 5 到 11 是大跳跃，从 10 到 11 不是”，而且变化之前要先让玩家休息，否则“11 就成了新的 5”（[Stout, Trinity Part 6](https://www.gamedeveloper.com/design/trinity-part-6-intensity-ramps)）。机核作者 FU1W 把“多打 5% 的血”当作弱反馈的例子，把“每秒金币从 10 跳到 100”当作清楚可感的例子（[FU1W，机核](https://www.gcores.com/articles/213312)）。难度调整方面的研究恰好说明小幅变化察觉不到：Denisova 等人选择调节计时器来改难度，理由就是玩家难以察觉这类调整【摘要】（[Denisova CHI PLAY 2015](https://www-users.york.ac.uk/~pc530/pubs/Denisova_CHIPLAY15.pdf)）。至于常被引用的“提升 20%～30% 才有感”，没有找到可靠出处。

| 倍率 | log2 幅度 m | 出处 | 在来源里是什么 |
|---|---|---|---|
| ×1.05 | 0.07 | Weber 定律（重量） | 实验室里先后直接对比时的阈值 |
| ×1.1～×1.25 | 0.14～0.32 | Guan 每级 ×1.1；Coordination ×1.25；地图每层 +20% | 连续的小步成长 |
| ×1.2 | 0.26 | Guan 的“100 对 120” | 举例中“能分出来”的差别 |
| ×2 | 1 | AdCap 前 400 个、Cookie Clicker 每档、AD 买 10 和 Boost | 最常见的离散跳变 |
| ×2.2 | 1.14 | Stout 的 5→11 | “大跳跃” |
| ×3～×5 | 1.6～2.3 | AdCap 的 1000、2000、3000 个 | 整数关口 |
| ×10～×11 | 3.3～3.5 | FU1W 的 10→100；AdCap 报纸 ×11 | 强反馈；“惊喜”级 |

【推导】我们建议统一用对数幅度 m = log2(之后 ÷ 之前) 衡量每次成长事件，并把“小”（m≥0.26，约 ×1.2）、“大”（m≥1，×2）、“巨”（m≥3.3，约 ×10）作为候选分级。这三个阈值是从上表的例子里挑出来的，不是任何来源给的标准。游戏里“之前”的数值往往是几小时前看到的，中间还夹着别的数字，所以实际可感的阈值应该明显高于实验室的 5%。单步低于阈值、但几步累积能到 ×1.2 的情况，单独记作“缓慢可感”。Pecorella 还指出另一个感知问题：当一个生成器在对数图上压倒性地占优时，旧生成器就“感觉不到”了，解决办法是错开倍率，让“最值得买的”轮流换（[Pecorella Part I](https://www.gamedeveloper.com/design/the-math-of-idle-games-part-i)）。放到我们的三选一学说上，意思是：如果某个学说在一轮里始终是最优，选择本身就不再是爽点。

## 二、能落到数字的节奏模型只有“强度序列加形状约束”

公开资料里，唯一把节奏写成带时长参数的算法的，是 Valve 的 L4D AI Director。每名幸存者有一个强度值：受伤、倒地、附近有感染者死亡时上升，随时间衰减，交战期间不衰减；全队取四人中的最大值。四个状态循环切换：Build Up 满威胁，直到强度越过峰值阈值；Sustain Peak 维持 **3–5 秒**；Peak Fade 切到最低威胁，等战斗自然告一段落；Relax 维持 **30–45 秒**，然后回到 Build Up。Boss 不受这个节奏控制，因为它本来就是用来打破节奏的。尸潮按 90–180 秒的随机间隔出现，设计目标是“结构化的不可预测”：游荡者是高频层，尸潮和特感是中频层，Boss 和武器箱是低频层（[Booth 2009, The AI Systems of Left 4 Dead](https://cdn.fastly.steamstatic.com/apps/valve/2009/ai_systems_of_l4d_mike_booth.pdf)）。L4D2 的 Director 脚本 wiki 列出的 SustainPeak 默认值是 5～10 秒，和演讲里的 3～5 秒对不上，可能是版本差异【摘要】（[Valve Developer Wiki](https://developer.valvesoftware.com/wiki/L4D2_Director_Scripts)）。

关卡制游戏的节奏模型是“画曲线再检查形状”。Lopez（前 EA/THQ 设计总监）把强度定义为事件的兴奋幅度，把节奏定义为强度相近事件之间的频率。他认为一条完美单调上升的曲线缺乏对比，既做不到，也不可取。他给的排布规则有三条：高强度事件要按递增排列；每个波的长度保持不变或变短；谷底必须有，但不必像峰那样精确安排。他还建议每关至少 3 个独特事件，15 关的战役约 60 个独特事件，另加约 5 个可复用的通用事件（[Lopez 2008](https://www.gamedeveloper.com/design/gameplay-fundamentals-revisited-harnessed-pacing-intensity)）。Stout 把节奏定义为强度与休息之比（[Stout 2015](https://www.gamedeveloper.com/design/trinity-part-6-intensity-ramps)）。Schell 的兴趣曲线要有一个开头的钩子，在起伏中上升，最后以全程最高的高潮结束（[notesbylex 摘录](https://notesbylex.com/interest-curve)）。PaceMaker 给了学术版本：节奏由“节律参数”在时间上的重复模式构成，参数分为设计者直接能调的“制品参数”和玩家感受到的“体验参数”，强度可以用制品参数按任意公式近似。受访的职业设计师说，他们平时就在 Excel 里给每个节拍打强度分、画柱状图，最大的抱怨是每次迭代都要手工重画（[PaceMaker, arXiv 2408.15001](https://arxiv.org/pdf/2408.15001)）。另外两种失败方式，Chen 的心流理论和 Koster 的乐趣理论分别讲过：挑战超出能力会焦虑，不足会无聊；模式一旦被掌握，游戏就无聊了（[Chen, CACM 2007](https://khoury.northeastern.edu/~lieber/courses/csu670/f08/materials/p31-chen-flow-in-games.pdf)，[Koster 2003](https://www.raphkoster.com/wp-content/uploads/2026/07/Austin-Game-Conference-2003-Keynote-Theory-of-Fun.pdf)）。

奖励日程这一侧，最可操作的是 Hopson 的行为主义框架。比率逐步提高（比如每级所需经验递增）会拉长拿到奖励后的停顿，最终“停顿可能变成无限”；突然失去奖励非常令人反感，奖励率最好不要剧烈变化；要“永远、永远有一个让玩家继续玩的理由”，并同时提供多项活动；链式日程里，玩家会把“进入下一阶段”本身当成奖励（[Hopson 2001](https://www.gamedeveloper.com/design/behavioral-game-design)）。放置游戏的回访节奏，最具体的设计是 Eric Guan 的多时钟：牛 20 分钟产满；奶酪坊每 30 分钟出一块、最多存 10 块，5 小时不收就停产；船坞 2 天。玩家忽略某些系统时，仍能在别的系统上感到成功。他描述的理想参与节奏是衰减的：首局 **15～60 分钟**，然后约每小时回来一次，再到每天几次，最后每周一次（[Eric Guan](https://ericguan.substack.com/p/idle-game-design-principles)）。Alharthi 等人分析了 66 款放置游戏，把这类游戏界定为“等待就是游玩”，随进程推进奖励“做得更少”的玩家；Kittens Game 用提示框告诉玩家离目标还要多少现实时间（[Alharthi 等, CHI 2018](https://par.nsf.gov/servlets/purl/10061230)）。GameAnalytics 的文章称放置游戏的平均会话时长为 8 分钟，黏性 18%，其他超休闲游戏是 10.5%（[GameAnalytics](https://www.gameanalytics.com/blog/how-to-make-an-idle-game-adjust)）；Egg, Inc. 把离线收益限制在 2 小时（[Pecorella Part III](https://www.gamedeveloper.com/design/the-math-of-idle-games-part-iii)）。没有找到“4～8 小时回访一次”的权威来源。

中文资料给的是经验分级，而且都是为连续游玩的游戏写的。MeowMeowRanger 以 RPG 和 ACT 为例，把正向刺激分成三级：大里程碑约 1～5 小时，中里程碑 5～30 分钟，小里程碑基本在 2 分钟以内（[MeowMeowRanger，机核](https://www.gcores.com/articles/192141)）。琋琯在 TapTap 发布的“心流工程规范”要求“每 5 分钟至少一次‘变强’反馈”，首次尝试的过关率在 70%～85%。这是个人整理的规范，不是行业标准（[琋琯，TapTap](https://www.taptap.cn/moment/797458355978243984)）。FU1W 把游戏的本质概括为“压力的积攒和释放”，并建议在释放期就开始下一轮积攒（[FU1W，机核](https://www.gcores.com/articles/213312)）。遥测方面，GameAnalytics 的进度漏斗用开始/完成比、失败/完成比暴露难度尖峰（[GameAnalytics Funnels](https://docs.gameanalytics.com/products-and-features/analytics-iq/funnels)），并建议叠加胜率和耗时，来区分一关是“难”还是“长”【摘要】（[GameAnalytics 博客](https://www.gameanalytics.com/blog/exploring-gaming-funnels)）。没有找到把“距下一次解锁的时间”和流失直接挂钩的公开研究。

【推导】这些模型有一副共同的骨架：一条时间轴上的强度序列 I(t)，加上若干形状约束。能直接翻译成检查项的有五条：前段要有钩子（Schell）；峰高序列与时间的秩相关不小于 0（Lopez）；强峰之前有休息段（Stout）；相邻时间窗的奖励率不突降（Hopson）；等待下一次奖励的时间不能无限拉长（Hopson）。这里有一个需要调和的矛盾。Lopez 要求波长不变或变短，Guan 却说放置游戏的回访间隔应该越拉越长。两者其实可以同时成立：**间隔按回访次数计时保持不变或变短，按小时计时就可以变长**。L4D 的放松段是峰值段的 6～15 倍（30/5 到 45/3），这只能借用“谷远长于峰”这个结构，秒级的数字不能搬到放置游戏上。

## 三、新系统和自动化先密后疏，自动化奖励已经做腻的操作

新层和大系统的间隔是“几小时到几天，越往后越长”。AD 社区攻略对各阶段的估计，全部是区间：Introduction 5 小时～2 天，Galaxies 3 小时～2 天，早期 Infinity 6 小时～6 天，Challenge Era 2 小时～3 天，Replicanti Era 1.5～12 天（“with timewalls”），Eternity Era 1.5～16 天，第一次 Reality 目标约 5 天（[AD Wiki: Guide](https://antimatter-dimensions.fandom.com/wiki/Guide)）。层内的小解锁要密得多。Trimps 在第 8、14、18、23、25、28、30、34、37、40、47、50 个 zone 依次解锁建筑或巨型升级，第 20 个 zone 开放重置层 Portal（[Trimps 源码](https://github.com/Trimps/Trimps.github.io/blob/master/config.js)）。Universal Paperclips 的第一幕大约 3～5 小时，之后整体换界面。作者 Frank Lantz 说要“establish a rhythm, establish a pattern and you throw in a twist”；研究者把每次范式转换看作叙事的一拍：“Whenever the system changes … a new beat in the narrative unfolds”（[Paperclips Wiki](https://universalpaperclips.fandom.com/wiki/Stages)，[Spiel 等, CHI PLAY 2019](https://par.nsf.gov/servlets/purl/10174274)）。Realm Grinder 则按重置次数解锁新机制：第 4 次重置开中立种族挑战，第 5 次开助手，第 20 次开同类建筑互相加成（[Realm Grinder Wiki](https://realm-grinder.fandom.com/wiki/Reincarnation)）。

商业手游的开放表按关卡或建筑等级排，都是强烈前置的。AFK Arena 在 1-12 关一次开放 5 个系统，【推导】前 4 章一共约 10 次开放；之后要到 12-40 关才有下一次，自动战斗更是放到了 35-60 关（[Theria Games](https://theriagames.com/?p=45992)）。Clash of Clans 2014 年版本的大本营升级时间是：2 级 5 分钟、3 级 3 小时、4 级 1 天、5 级 2 天、6 级 4 天，此后每级再多约 2 天；1～9 级每级至少开放一种新建筑（[Pocket Gamer](https://www.pocketgamer.com/clash-of-clans/town-hall-clash-of-clans-building-breakdown)）。后来官方多次缩短过这些时间，所以绝对值只代表那个版本【摘要】（[Supercell 2023](https://supercell.com/en/games/clashofclans/blog/news/cost-and-time-reductions-december-2023/)）。中文设计文章讲的是原则：新手期的时间刻度要切得比后期细；每个新系统先判断它是在“弥补战力瓶颈”还是“制造新障碍”（[GameRes 845964](https://www.gameres.com/845964.html)）；模块的开放要跟着“玩家需求”走（[GameRes 878972](https://www.gameres.com/878972.html)）。SimCity BuildIt 的复盘给了一条直接适用于我们的批评：“Leveling up is not rewarding and does thereby not qualify as a goal for the player”（[Zupke, Game Developer](https://www.gamedeveloper.com/design/progression-control-in-sim-city-buildit)）。Rise of Kingdoms 的学院是小加成的反例：1～20 级每级研究加速 +0.5%，21～24 级每级 +2%，25 级直接到 25%；升级时间从 1 级的 6 秒涨到 25 级的 25 天 9 小时（[RoK Guides](https://riseofkingdomsguides.com/academy/)）。

研究树和“N 选 1”的频率，可以拿来对照我们的学说。Kittens Game 的科技价格从第一项的 30 科学涨到第 15 项 machinery 的 15,000；每项科技的收益是解锁新建筑、新职业或新升级，而不是百分比加成。自动化升级 factoryAutomation 要 10,000 科学加 25 齿轮（[science.js](https://raw.githubusercontent.com/nuclear-unicorn/kittensgame/master/js/science.js)，[workshop.js](https://raw.githubusercontent.com/nuclear-unicorn/kittensgame/master/js/workshop.js)）。肉鸽游戏选得勤、每次力量小：Slay the Spire 几乎每场战斗后都有一次 3 张选 1，可以跳过，一局 3 幕各 17 层【摘要】（[StS Wiki](https://slay-the-spire.fandom.com/wiki/Card_Rewards)）；Vampire Survivors 每升一级 3 选 1，武器和被动各限 6 格【摘要】（[Game Rant](https://gamerant.com/vampire-survivors-luck-effects-stat/)）。【推导】我们一周大约 9 次三选一（第七节推导），按每局算已经比肉鸽少，按每小时算更是差得远，更接近 SLG 的科技树。所以每次选择的分量要更大，也更适合跟“解锁新东西”绑在一起。

自动化本身就是奖励，常见的给法有三种。第一种是用当期货币买。AdCap 的经理人让生意在离线时也自动开工，这正是它能成为放置游戏的原因。柠檬水摊经理人卖 1,000，【推导】约是首个单位价格的 250 倍；而且重置后要重新买（[AdCap Wiki: Managers](https://adventure-capitalist.fandom.com/wiki/Managers)，[Angel Investors](https://adventure-capitalist.fandom.com/wiki/Angel_Investors)）。第二种是先通过挑战。AD 的 12 个普通挑战依次奖励第 1～8 维、Tickspeed、Dimension Boost、Galaxy、Big Crunch 的自动购买器，后三个要先完成 16 次 Infinity 才开放（[normal-challenges.js](https://github.com/IvarK/AntimatterDimensionsSourceCode/blob/master/src/core/secret-formula/challenges/normal-challenges.js)）。第三种是按重置次数逐步保留。AD 第 2 次 Eternity 起，开局就完成全部普通挑战、拥有全部普通自动购买器；第 11～18 次 Eternity 每次再开一个新维度的自动购买器；第 100 次才开放 Eternity 本身的自动化（[eternity-milestones.js](https://github.com/IvarK/AntimatterDimensionsSourceCode/blob/master/src/core/secret-formula/eternity/eternity-milestones.js)）。Trimps 在第 1 个 zone 就给自动出战，第 10 个给自动造陷阱，第 20 个给自动扩仓（[Trimps 源码](https://github.com/Trimps/Trimps.github.io/blob/master/config.js)）。AFK Arena 的 2 倍速、快速挂机、悬赏自动填充分散在 2-16 关到 12-40 关之间，大多可以用 VIP 提前开（[Theria Games](https://theriagames.com/?p=45992)）。Alharthi 等人干脆用自动化来定义增量游戏：玩家选择生成哪种资源，等它累积，再花资源把生成过程自动化（[Alharthi 等](https://par.nsf.gov/servlets/purl/10061230)）。

【推导】AD 的顺序可以概括为“越重复的越早自动化，越接近重置或战略决策的越晚”：先是买单一维度，然后是 Tickspeed，再是重置类操作，最后是下一层的重置。它的自动化总是要求玩家先在受限条件下把这件事亲手做一遍。自动化的幅度可以记为 m = log2(维持同等收入所需操作数，自动化前 ÷ 自动化后)。对应到我们的三条规章（稳妥占领 `auto_claim`、告急反击 `auto_counter`、择优招募 `auto_recruit`），来源支持三种做法：开局即有（Trimps 的自动出战）；局内亲手做够若干次后开放（AD 挑战）；跨轮回逐条开放或保留（AD 的 Eternity 里程碑）。AdCap 的“每轮重买”则说明，“重新自动化”本身也可以成为每轮开局的一个小爽点。

## 四、墙来自成本跑赢产出，每道墙要配一个玩家够得着的破墙手段

墙的成因在数学上很清楚：成本指数增长，产出线性或多项式增长，指数终究会压过多项式。乘数只能暂时把产出推到成本前面，转生则让玩家“get farther through the cost curve before falling behind”（[Pecorella Part I](https://www.gamedeveloper.com/design/the-math-of-idle-games-part-i)）。Clicker Heroes 的难度曲线“very steep”，主线靠反复重置跨过“progress wall”；由于升级价格是指数增长，“this does effectively take a log of the overall growth curve”（[Pecorella Part III](https://www.gamedeveloper.com/design/the-math-of-idle-games-part-iii)）。GameRes 的分析把卡关归结为“boss 的血量增长率比玩家数值的增长更快”，每 5 关一个 Boss，Boss 战对 DPS 有特殊要求；重置是“以退为进”，长边难以增长时，“把资源用在短边上，反而能获得更大的面积增益”（[GameRes 857892](https://www.gameres.com/857892.html)）。Schreiber 把这种形状叫锯齿：进入新区域时难度跳升，区内持平，Boss 处出现尖峰；玩家战力在区内小步上升，过 Boss 时跳升；阶段之间的过渡应逐渐变长（[Schreiber Level 7](https://gamebalanceconcepts.wordpress.com/2010/08/18/level-7-advancement-progression-and-pacing/)）。AD 攻略直接用了“timewall”这个词（[AD Wiki: Guide](https://antimatter-dimensions.fandom.com/wiki/Guide)）。

对于墙该怎么设计，各家的说法相当一致。机核作者认为，玩家到下一个数量级所需的最小等待时间“往往是经过设计的”，阶段内容做完后，好的设计会告诉玩家“想要后面的内容？等一段时间再来吧”；他反对每几秒就能买下一样东西的过度平滑，并估计玩家的操作时间“可能还不到总时间的1%”（[机核 193130](https://www.gcores.com/articles/193130)）。一篇《Unnamed Space Idle》的分析把新系统、新层级带来的“质变式增长”和随后的放缓称为整理期，并建议低速期仍保留短期目标【摘要】（[bilibili cv45029663](https://www.bilibili.com/read/cv45029663)）。卡点可以分为“挑战卡点”和“成长卡点”两类【摘要】（[知乎 113352737](https://zhuanlan.zhihu.com/p/113352737)）。整数关口有强迫效应：49 级的玩家很想升到 50 级，“升到51级的时候就有不少玩家会流失”；作者的做法是先定几个关键等级的目标时间，再拟合整张表（[百鲸 3362](https://www.baijing.cn/article/3362)）。Tap Titans 的美术 Lucas Mills 描述了锯齿的手感：转生后“pass through the beginning stages with ease”，撞墙时“you start to think about what things you should be upgrading for your next run”（[CHI PLAY 2019](https://par.nsf.gov/servlets/purl/10174274)）。Trimps 的地图加成是一个可控的破墙工具：打不过当前 zone，就去刷同级地图，每张 +20%，过 zone 清零（[Trimps Wiki: Maps](https://trimps.fandom.com/wiki/Maps)）。

没有任何来源给出“墙该持续多久”或“突破该多大”的目标值，Pecorella 也明确只给模型、不给推荐间隔（[Pecorella Part III](https://www.gamedeveloper.com/design/the-math-of-idle-games-part-iii)）。【推导】墙从哪里开始，可以从公式直接算：下一件的价格每次 ×r，多买一件带来的收入增长是 (n+1)/n；当 n > 1/(r−1) 时，攒钱时间开始逐件变长，r=1.07 时这个点约在第 14 件。锯齿的形状由两件事决定：两次离散倍率之间成本涨了多少，以及下一个倍率有多大。在机器人试玩的时间序列上，可以这样识别：窗口对数增长 g_w(t) = log2(X(t+w)/X(t)) 低于“小”阈值的连续区间算墙，记录墙的数量、最长墙和墙占总时长的比例；墙结束后短窗口内的对数增长算突破幅度；单次可感跳变贡献的对数增长占总增长的比例算锯齿度。GameAnalytics 区分“难”和“长”的思路，在这里变成：卡住期间如果还有有意义的操作可做，就是可以接受的积攒期；如果无事可做，就是 Hopson 说的“停顿趋于无限”。

## 五、轮回的爽点在开局的轻松和“暂时的结局”，组织等级要按轮来排

转生货币普遍取累计产出的分数次幂，所以想让转生货币翻倍，要付出越来越多的产出。Realm Grinder 约需上一轮 4 倍，AdCap（150·√(c/10¹⁵)）需 3～4 倍，Cookie Clicker（立方根）需 8 倍，Egg, Inc.（约 0.14 次方）需 128 倍；原文的说法是“Most prestiges are a fractional exponent”。Pecorella 还建议，重置的节奏也要有快有慢（[Pecorella Part III](https://www.gamedeveloper.com/design/the-math-of-idle-games-part-iii)）。AdCap 每个天使给 +2% 利润，加法叠加，累计收入 10¹⁵ 时有 150 个，相当于下一轮利润 ×4（[AdCap Wiki: Angel Investors](https://adventure-capitalist.fandom.com/wiki/Angel_Investors)）。重置后能快多少，AD 攻略给出了一组数：早期 Infinity 阶段末，最快一轮约 10 分钟；第一次 Reality 约 5 天，第二次 1.25～5 天，之后活跃玩家降到约 2 小时，再到 15～30 分钟（[AD Wiki: Guide](https://antimatter-dimensions.fandom.com/wiki/Guide)）。【推导】规律是第二轮只快一点点，几轮之后才快一两个数量级。

有几款游戏把“轮长”和“第一次值不值”写成了规则。NGU Idle 按轮次时长乘一个因子：1 小时 1.02，1 天 1.5，7 天 4.5，14 天 8.0，60 分钟以上的公式是 1 + 天数/2（[NGU Wiki: Rebirths](https://ngu-idle.fandom.com/wiki/Rebirths)）。Kittens Game 规定猫超过 70 只才开始给 paragon，超过 35 只才开始累计 karma（[Kittens 源码 prestige.js](https://github.com/nuclear-unicorn/kittensgame/blob/master/js/prestige.js)）。Trimps 打完第 20 个 zone 开放 Portal，并当场给 45 Helium（[Trimps Wiki: Portal](https://trimps.fandom.com/wiki/Portal)）。AD 的 Eternity 里程碑把“开局就拥有上一轮的成果”拆成很多小步：第 2 次保留挑战和自动化，第 4 次保留 Infinity 升级，第 10 次开局就有 Replicanti，第 30 次开局所有维度可买（[eternity-milestones.js](https://github.com/IvarK/AntimatterDimensionsSourceCode/blob/master/src/core/secret-formula/eternity/eternity-milestones.js)）。SLG 换赛季时，三国志·战略版保留武将、战法、装备和货币，清空地图上的城池【摘要】（[小米游戏中心转载](https://game.xiaomi.com/viewpoint/1444108958_1682776471399_9)）。研究者认为，重置机制制造了“additional temporary endings”，让放置游戏能提供一连串可以连载的封闭叙事（[CHI PLAY 2019](https://par.nsf.gov/servlets/purl/10174274)）。

【推导】组织等级是周回成长，所以它的节奏要按“第几轮”来排，不能按“一周里的第几天”排。【读码】现行门槛是 `org_levels` [0, 100, 250, 500, 900, 1400, 2000]，组织经验等于本轮成果，福利分布是：2 级第二支小队，3 级档案位和快速响应，4 级招募池，5 级档案位和封存位，6 级开局加成（`start_bonus_bp` 5000）。拿 `greedy` 72 小时的成果中位数 255 当每轮成果的下限（一周轮的成果会更高，具体多少还没测），累计成果和等级的对应如下：

| 第几次收束后 | 1 | 2 | 3 | 4 | 5 | 6 | 8 |
|---|---|---|---|---|---|---|---|
| 累计成果（每轮 255） | 255 | 510 | 765 | 1020 | 1275 | 1530 | 2040 |
| 组织等级 | 3 | 4 | 4 | 5 | 5 | 6 | 7 |

【推导】第一次收束一下跳两级，第二支小队和档案位同时到手；之后每 1～2 轮升一级，第 6 轮左右到 6 级，按一周一轮就是六周左右。这个形状和 AD 很像：第 2 次 Eternity 一次性给一大包，之后逐条给；也和 CoC、RoK 的“前几级快、后几级慢”一致。它在一周之内的体现，是每轮第 0 小时的“重置后加速”：等级福利让第 1～2 环跑得更快，也就是 Lucas Mills 说的“pass through the beginning stages with ease”。另一个推论跟一周的长度有关。按边际值定理，理性玩家会在“每小时成果”降到含重建开销的平均值时收束（[Wikipedia: Marginal value theorem](https://en.wikipedia.org/wiki/Marginal_value_theorem)）。现在得分速率第 6～8 小时就见顶，所以要让一周一轮对玩家来说是划算的选择，就得靠第 2～7 天持续出现的局内爽点把速率撑住。NGU 那样显式的轮长因子是另一个杠杆，但它奖励的是“熬时间”，不是“局内发生了什么”。

## 六、我们的现状：一条没有锯齿的曲线

把第一节的对数幅度用到现有读数上，可以看出现在哪些线在给爽点，哪些线是平的。下表的读数来自用户给出的 `greedy` 20 个种子 p50，以及评估工具和代码。

| 成长线 | 现状读数 | 对数幅度（推导） | 读法 |
|---|---|---|---|
| 小队强度 | 第 6 到 48 小时 149 → 164【实测】 | 两天合计 m≈0.14 | 低于“小”阈值，战斗这条线两天里没有一次可感的跃升 |
| 得分速率 | 第 6～8 小时见顶，第 24 小时只剩峰值的 0.66【实测】 | 第一天内 m≈−0.60 | 违反 Lopez 的“峰值递增”和 Hopson 的“奖励率不突降” |
| 占地 | 6 / 24 / 48 / 72 小时：8 / 17 / 29 / 41 格【实测】 | 各段 m≈1.09 / 0.77 / 0.50 | 唯一有可感增长的线，但逐段衰减 |
| 学说 | 72 小时研究 6 次，约 12 小时一次【实测】 | 最强的属性学说 `overclock`（全队 +3 攻击、+2 速度）让参考小队输出约 ×1.16，m≈0.21；`plating`（+12 生命、+3 防御）让总有效血量约 ×1.18，m≈0.24。按先锋、射手、医疗兵的职业基础属性和战斗模型的 μ 系数粗算 | 都低于“小”阈值；而且实战里更紧的是输出检查 |
| 学说的价值 | 价格卡 12 个学说合计约 1.17 环当量，最高 0.30（`overclock`）【读码，`eval/prices.json`】；研究和不研究，脚本玩家的成果只差 1%～2%【实测，`docs/llm-playtest.md`】 | — | 战斗不卡扩张，学说就没有墙可破 |
| 规章 | 三条开局即可全开【读码】 | 不产生事件 | 设计稿里的“自动化解放”从未发生 |
| 街区连片 | 该街区产出 +25%（`contiguity_output_bp` 2500），信任 +10，成果 +5；本轮第一次连片会开放定制招募【读码】 | 局部 m≈0.32 | 唯一已有的“门槛 → 解锁”链，作用范围是一个街区 |
| 防线自给 | `sim::self_sufficient()` 判定所有边界格都“被证明守得住”，计入 `self_sufficient_hours`【读码】 | 类别事件 | 有状态，没有事件记录 |

【推导】三个结论。第一，现在只有占地在给可感的增长，战斗和研究这两条线是平的。原因不在学说本身。节奏模型显示，参考小队在第 1～6 环的余量是 8.1 / 6.9 / 5.8 / 1.7 / 1.5 / 0.99，战斗根本卡不住扩张，成长也就无处兑现（`docs/balance-model.md`）。所以“扩张门槛改为战斗强度卡”，不只是在修节奏，也是让学说变成爽点的前提。第二，价格卡上的学说价格被“天花板”压低了：参考小队本来就能稳拿前 5 环，任何学说最多只能再添两环多一点。等遭遇曲线重新调过、门槛落在每一环上之后，价格要重新测。第三，【读码】评估工具的事件钩子 `Trace` 现在只有 `Route` 和 `Surge` 两种。游戏自己的事件日志里已经有 `Claimed`、`Contiguous`、`Core`、`Rule`、`Candidate`、`Trait`、`Drop` 这些种类，但 `docs/balance-eval.md` 规定，按事件计的指标要走 `Trace` 钩子，不靠轮询状态去猜。研究次数目前就是 `run.rs` 轮询 `research_count` 得来的。

## 七、提议一：在节奏模型旁加一条爽点事件时间轴

【提议】现在的节奏模型回答“什么时候进第 r 环”。爽点时间轴要回答三件事：每类爽点事件预计在什么时候发生、幅度多大、在机器人的事件日志里实际什么时候发生；然后用一组形状指标检查整条时间轴。做法和战斗模型、节奏模型一样：公式先算预期，机器人负责验证，两者对不上超过两成，就补模型或写明原因。这一条是项目已有的规则。

### 每类爽点都要有检测钩子、幅度和预测公式

下表把设计稿的十一种爽点分成三类：能进时间轴的事件、战斗内的效果、结构性的设定。只有第一类进时间轴。

| 设计稿的爽点 | 时间轴事件 | 检测钩子 | 幅度怎么量 | 预测公式 | 现状 |
|---|---|---|---|---|---|
| 解题（预演由红变绿） | 参考小队对下一环的模型余量越过 1；前沿格的预演由负转胜 | 新增 `Trace`（现在的 `frontier_win.hN` 只在检查点采样） | log2(余量之后 ÷ 之前)；翻绿格数 | 门槛时刻 = O(t) 首次达到 目标余量 × HP_r ÷ (15 × m(def_r)) 的时刻（`docs/balance-model.md`“反推”的同一式子解出 O） | 有模型，事件没记 |
| （局内成长）研究学说 | 第 n 次研究和所选学说 | 新增 `Trace::Research`，取代轮询 | 价格卡环当量；输出比；翻绿格数 | 累计情报 ∫I(t)dt = 10n + 9n(n−1) 的解 | 缺情报收入模型 |
| 世界放大 | 首次进入第 r 环 | 已有 `reach_h.rK` | 领地比 (1+4r(r+1)) ÷ (1+4r(r−1))；新敌人、特殊格 | 节奏模型的 t_r | 已有，误差 10% 以内 |
| 数字跳变 | 街区连片 | 游戏事件 `Contiguous`，补一个 `Trace` | 该街区产出 ×1.25，信任 +10 | 街区各格预测占领时刻的最大值（节奏模型逐格展开） | 公式可得 |
| 挂机的满足感 | 首次防线自给；每段自给时长 | `self_sufficient()`、`self_sufficient_hours` | 类别事件；时长 | 暂无，防守模型还没做 | 只测，不预测 |
| 自动化解放 | 规章解锁或首次代做 | 游戏事件 `Rule` | log2(每小时所需操作数，之前 ÷ 之后) | 由解锁条件推出 | 开局全开，没有事件 |
| 新系统 | 命令首次可用（定制招募、政策、异界核心等） | 新增 `Trace::Unlock` | 类别事件 | 由各自的门槛推出，例如定制招募 = 本轮第一次连片 | 缺 |
| 回来有礼物 | 两次回访之间新增的候选、词条、掉落、可占领格 | 游戏事件 `Candidate`、`Trait`、`Drop`、`Claimable` | 每次回访的件数 | 由各产出速率推出 | 只测 |
| 重置后加速 | 轮回后第 1～2 环提前到达 | 要多轮评估 | 到达时间的压缩比 | 组织等级 L(累计成果) 带来的福利 | 没有机器人会轮回 |
| 连锁、构筑组合 | 战斗内 | 战报 | 价格卡成对测 | — | 不进时间轴 |
| 稀缺、方向感 | 结构性设定 | — | — | — | 不进时间轴 |

### 幅度分三种量法，分级阈值全部待校准

【提议】数值事件，在事件前后取一个速率或强度量，记 m = log2(后 ÷ 前)。战斗用参考小队的模型余量或输出 O，经济用得分速率或收入；连续漂移不算事件，只计入锯齿度的分母。类别事件（解锁、进新环、首次自给）不比较数值，至少记为“小”。这是调研笔记推导里的做法，没有来源。自动化事件用操作数之比。解题事件另记翻绿格数：在战斗当门槛的设计下，同样一次 ×1.16 的成长，余量从 0.9 推到 1.04 就是“进不去”变成“进得去”，所以翻绿格数比 m 更能反映玩家看到了什么。分级候选是第一节的“小、大、巨”（m ≥ 0.26 / 1 / 3.3），用 LLM 试玩校准：看 LLM 在回访时的局面摘要和下一步动作，有没有对这个事件作出反应。按 `CLAUDE.md`，试玩每批最多 3 个子代理。

### 模型侧：学说、门槛、连片都能写成公式

【推导】研究第 n 次（从 0 起计）的价格是 10 + 18n，所以前 n 次的累计价格 C(n) = 10n + 9n(n−1) ≈ 9n²。第 n 次研究的时刻 t_n 满足 ∫₀^{t_n} I(t)dt = C(n)，I 是情报收入。经济模型还没有，可以先用机器人实测的情报收入曲线当输入。假设情报收入不变、研究价格一到就花，用“72 小时 6 次”反推 I ≈ 330 ÷ 72 ≈ 4.6 每小时（这个假设没有核实），各次研究的时刻是 2.2、8.3、18.3、32.3、50.2、72.0、97.7、127.4、161.0 小时，第 10 次要到 198.5 小时。相邻两次的间隔线性拉长：2.2、6.1、10.0、14.0……33.6 小时。这正是 Hopson 说的“比率逐步加大”。

【推导】吞吐量下限 τ_r = 4r(r−1)+1 也是二次的。两条二次曲线相除，得到一个与环号无关的常数：到第 r 环时的研究次数约为 √(8I/18)·(r−½)，**每进一环约有 √(8I/18) ≈ 1.43 次学说**。按上面的时刻，到第 2～7 环时累计的研究次数是 2.1、3.5、4.9、6.4、7.8、9.2。所以 `research_step`（现在是 18）和情报收入，共同决定“每环几次学说”；减半 step，每环的次数就 ×1.41。如果情报收入随领地增长，后面几环分到的次数会更多。

【推导】战斗当门槛之后，第 r 环的门槛时刻就是 O(t) 首次让余量达到目标的时刻（`docs/balance-model.md`“反推”）。如果 O(t) 只在研究时跳变，门槛时刻就会落在某一次研究上，墙长 W_r = 破门那次研究的时刻 − τ_r（取不小于 0）。设每环敌人的血量按 g 倍增长（g 还没定，此前提议过 1.4），输出检查要求 O 也每环 ×g。如果只靠学说，平均每次学说要 ×g^(1/1.43)：g=1.4 时约 **×1.27**（m≈0.34），一周累计 1.4⁶ ≈ 7.5 倍。现在最强的属性学说约 ×1.16，大多数学说在价格卡上只值 0.03～0.05 环。现行遭遇台阶也不均匀，按 L0 的遭遇强度，第 3→4 环一跳就是 ×2.40，相当于一次 m≈1.26 的“大”跳变（这是旧强度指数的比值，只能当量级参考）。连片时刻等于该街区各格预测占领时刻的最大值，节奏模型已经按格给出每格的耗时 h_r，可以逐格展开。组织等级用上一节的累计成果表。防线自给要等防守模型。

### 指标和候选区间

【提议】指标 id 沿用现有写法，输出只有 id 和数字，不碰 i18n 棘轮。区间一栏凡是写“登记”的，都是先用基线 p10～p90 登记成观察项，不判失败，因为来源没有给数字。

| 指标 id（提议） | 定义 | 依据 | 候选区间（全部待校准） |
|---|---|---|---|
| `payoff.t.<类>.<k>`、`payoff.model_ratio.<类>` | 第 k 次某类事件的时刻；实测 ÷ 模型 | 项目规则：模型与机器人差两成要补模型 | 比值 0.8～1.2（来自项目规则，不来自来源） |
| `payoff.hook_h` | 第一个“小”及以上事件的时刻 | Schell 的钩子；Guan 首局 15～60 分钟 | ≤ 1 小时，取 Guan 区间的上限作起点 |
| `payoff.gap_max_h.<级>.<时段>`、`payoff.gap_p90_h.…` | 各时段内同级事件的最长间隔和 p90 间隔，时段为第 1 天、第 2～3 天、第 4～7 天 | MeowMeowRanger 的分级；Hopson | 登记；形状约束是按回访次数计不增（Lopez 与 Guan 的调和） |
| `payoff.visit_hit.<节奏>` | 按某种回访日程，在“自上次回访以来至少有 1 个可感事件或新决策”的回访中所占比例 | Guan 的多时钟；Hopson 的“永远有理由” | 登记；日程用 `greedy` 时间轴事后开窗，再用 `greedy@4` 实跑对照 |
| `payoff.wait_vs_visit` | 距下一个爽点的等待 w(t) 的增长 ÷ 回访间隔的增长 | Hopson 的“停顿趋于无限”；Guan 的衰减参与 | ≤ 1，这是定义上的边界，不是经验数 |
| `payoff.peak_rank` | 每天最大事件幅度与天序的秩相关 | Lopez 的“峰值递增” | ≥ 0，同上 |
| `payoff.finale` | 最后一段是否有全轮最大或次大的事件 | Schell 的大结局 | 是／否 |
| `payoff.rate_drop.max` | 相邻窗口得分速率的最大降幅（扣除玩家主动造成的） | Hopson 的“避免突降” | 登记；现状第 24 小时已降到峰值的 0.66 |
| `payoff.sawtooth.<序列>` | 输出 O(t)、得分速率中，来自单次“小”及以上跳变的对数增长占总增长的比例 | Stout；Pecorella 的里程碑 | 登记；现状接近 0 |
| `payoff.wall.{count,max_h,share}` | g_w(t) 低于“小”阈值的连续区间 | Pecorella；GameRes 的“突进、放缓、再突进” | 来源没有墙长数字；建议按回访次数表达 |
| `payoff.wall.idle_share` | 墙内没有任何有意义操作的小时占比 | GameAnalytics 区分“难”和“长”；Hopson | 越低越好；先登记 |
| `payoff.flip.<事件>` | 事件当刻前沿格由负转胜的格数 | 设计稿的“解题”；门槛放大效应 | 登记 |
| `payoff.novelty.d<天>` | 当天首次可用的命令、系统、规章的数量 | Koster；Alharthi；AFK 和 CoC 的先密后疏 | 形状约束：第 1 天最多，之后不增 |
| `payoff.lines.<时段>` | 同一时段内给出可感事件的成长线数（学说、领地、街区、防线） | GameAnalytics 的“不止一种推进方式”；Guan 的多时钟 | 调研笔记推导的起点是 ≥ 2，没有来源 |

### 接入现有框架要改的地方

【提议】`eval pace` 在现有的“进第 r 环”那几列旁边，打印预测时间轴：第 n 次研究、各环门槛、各街区连片的预计时刻。`eval run` 通过新增的 `Trace` 变体记录实际时间轴：研究、解锁、连片、自给开始和结束、规章代做、余量越过 1。`Trace` 不存档，所以不需要把 `SAVE_VERSION` 加一。`eval check` 先只报告 `payoff.*`，不让它们判失败；`bands.json` 现在只有 min/max，没有“观察”状态，这些指标暂时不进去。`docs/balance-eval.md`“新系统怎么接入”的清单加第 9 条“爽点声明”：新系统要说明它产生哪类时间轴事件、在哪里埋钩子、幅度怎么算、预计时刻的公式是什么。按 `CLAUDE.md`，如果为此给玩家加了可见信息，比如“下一个解锁还差什么”“下一次研究还要多少情报”，文字版和 `cli.json` 要在同一个改动里一起做。这类信息 Alharthi 称为“解锁作为可用性支持”，Kittens Game 的“离目标还要多少时间”提示也是一例。回访命中率先在 `greedy` 的时间轴上事后开窗计算；`greedy` 每小时都行动，代表的是最勤快的玩家，所以还要拿 `greedy@4` 的实跑结果对照。

## 八、提议二：一周骨架草案（推导，供讨论）

【推导】下表只是把几条已有的线叠在一起：吞吐量下限 τ_r、上一节在“情报收入不变”假设下推出的研究时刻、设计稿的阶段表（开局、扩张、成形、终局），以及来源给的节奏形状。它说明的是各种爽点“可以落在哪里”，不是定稿。

| 时段 | 进环（吞吐量下限） | 学说（第几次，约在第几小时） | 候选爽点 | 墙与突破 | 依据 |
|---|---|---|---|---|---|
| 第 0 小时（第 2 轮起） | 第 1～2 环比上一轮快 | 无 | 组织等级福利生效（第二支小队等）；若规章改成跨轮保留，开局就有 | 无墙，轻松跑过前段 | AD 的 Eternity 里程碑；Lucas Mills |
| 第 1 天（0～24 小时） | 第 1 环约 1 小时，第 2 环约 9 小时 | #1 约 2，#2 约 8，#3 约 18 | 首格占领；首次学说；第一个连片（街区多为 3～5 格，落在内圈）；进第 2 环；若规章改成局内解锁，亲手占够若干格后开放 `auto_claim` | 第一道短墙：第 2 环的门槛对准 #2 | AFK 和 CoC 前置；Guan 首局 15～60 分钟；Schell 的钩子 |
| 第 1～3 天（24～72 小时） | 第 3 环约 25 小时，第 4 环约 49 小时 | #4 约 32，#5 约 50，#6 约 72 | 新系统（政策、定制招募）；首次防线自给；继续连片；回来有礼物 | 墙变长；第 4 环的现行台阶最大（旧指数 ×2.40），需要一次“大”突破 | Lopez 的峰值递增；Hopson 的奖励率不突降 |
| 第 3～7 天（72～168 小时） | 第 5 环约 81 小时，第 6 环约 121 小时 | #7 约 98，#8 约 127，#9 约 161；间隔 1～1.4 天 | 镇压裂隙；深入异界；较长的防线自给；每天至少一个“大”事件 | 每环一道墙，墙长以“天”计，要靠非战斗线填满 | Guan 的“每天几次”；AD 层间半天到几天 |
| 第 7 天（约 169 小时起） | 第 7 环约 169 小时 | #10 约 199，一周内基本拿不到 | 冲击异界核心（全轮最大峰）；收束结算；组织升级 | 终局墙就是核心 | Schell 的大结局；CHI PLAY 的“暂时的结局”；NGU 的轮长因子 |

【推导】这张表有三点值得讨论。第一，学说落在各环之间的分布不均匀。到第 2～7 环之前，各环分别有 2、1、1、2、1、2 次研究可以用来破门（第 2 环 #1～#2，第 3 环 #3，第 4 环 #4，第 5 环 #5～#6，第 6 环 #7，第 7 环 #8～#9）。所以每环的血量要分别对准某一次研究。门槛对准 τ_r 之前的最后一次研究，玩家感觉不到墙，研究那一刻就是“下一环翻绿”。门槛对准 τ_r 之后的第一次研究，按上面的时刻，各环的墙长依次约为 9、7、1、17、6、30 小时，参差很大，而且第 7 环的墙会跨出一周。第二，第 3～7 天研究间隔已经拉到 1～1.4 天。按 Guan 的衰减参与，这时玩家每天回来几次，一道墙会跨好几次回访。这几天的回访命中率只能靠连片、防线自给、回来有礼物和新系统兜底，也就是 Hopson 说的“多项活动并行”。第三，局内战斗成长的总量是 g⁶（g=1.4 时约 7.5 倍），但现在第 6 到 48 小时只涨了 10%。这个差距，要么由学说的量级补上（每次约 ×1.27），要么分一部分给设施和装备，要么把每环的台阶 g 调小。三条路都要先拍板“局内成长算哪几样”。表里所有时刻都建立在“情报收入不变、价格一到就花”的假设上，情报收入的实测曲线一出来，这张表就要重算。

## 结论

这次调研最大的改变，是把“爽点”从一张定性清单变成了一个和战斗、节奏同构的建模问题。其他游戏的爽点都是“离散成长撞上门槛”，所以我们的爽点问题和“战斗当门槛”是同一个问题。门槛不卡人，学说就只是 +1% 的小斜坡；门槛卡得准，同样一次 ×1.16 也能变成“下一环翻绿”。这也解释了为什么 LLM 试玩和脚本都觉得研究没用：不是学说太弱，而是它们没有墙可破。所以要先让战斗成为门槛，再调学说的大小，顺序不能反。

第二个收获是一个结构上的巧合：研究价格的累计值和环的面积都是二次的，所以每环分到的学说次数天生近似恒定（约 1.4 次）。频率不用重新设计，只要决定幅度，以及每环门槛对准哪一次研究，而 `research_step` 就是调节“每环几次”的那个旋钮。第三，来源唯一能落实的节奏约束都是“形状”而不是“数值”：峰值递增、奖励率不突降、等待不无限拉长。把间隔按回访次数而不是按小时计，Lopez 和 Guan 看似相反的要求就可以同时满足。这也意味着，爽点指标里真正能先硬起来的是形状检查，绝对区间只能等 LLM 试玩校准之后再收紧。

## 待拍板的决定

| # | 决定 | 选项与说明 |
|---|---|---|
| 1 | 爽点幅度的分级 | 先采用 ×1.2 / ×2 / ×10（m = 0.26 / 1 / 3.3）作为候选，再用 LLM 试玩校准；还是先不分级，只记 m |
| 2 | 局内成长算哪几样、各占多少 | 只靠学说，还是设施、装备也算。只靠学说且 g=1.4 时，每次学说平均要约 ×1.27，现在最强的约 ×1.16 |
| 3 | 学说的形态 | 少而大的属性跳变（×2 级）、“改规则”型（靠翻绿和连锁体现），或者两类分层；`research_step` 是否维持 18（决定每环约几次） |
| 4 | 每环门槛对准哪次研究，墙多长 | τ_r 之前的最后一次（几乎无墙，研究即翻绿），或之后的第一次（每环一道墙，长度从约 1 小时到 30 小时不等）；墙长的上限用几次回访来表达 |
| 5 | 规章怎么解锁 | 维持开局全开（“自动化解放”不会作为事件出现）；局内亲手做够 N 次后开放（AD 挑战式）；跨轮回逐条开放或保留（AD 的 Eternity 里程碑，也是设计稿“快反规章 = 重置后加速”的写法） |
| 6 | 回访命中率按什么日程算 | `greedy` 每小时、每 4 小时，还是 Guan 式的衰减日程；“4～8 小时回访一次”没有来源，要自己定 |
| 7 | 组织等级的跨轮节奏 | 按现行门槛，第一次收束跳两级，之后每 1～2 轮一级，约第 6 轮到 6 级，是否接受；开局加成是否做成最后一级的大奖励 |
| 8 | 轮长的激励 | 现有区间“得分速率峰值在 48～96 小时”是按 72 小时一轮定的。一周一轮时，是否改成“含重建开销的每小时组织经验峰值落在成形阶段”；要不要另加 NGU 式的显式轮长因子 |
