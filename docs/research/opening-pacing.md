# 放置游戏开局节奏调研：Evolve 与四个对照

> 状态（2026-11-28）：调研。只补 `commercial-design.md` 和 `payoff-pacing-research.md` 没有的内容，两份文档已有的（AdCap ×64 里程碑、AD 自动化顺序、Trimps zone 解锁表等）只引用不重述。
>
> **标记约定**
> - 【已证实】：读到了一手来源（源码或官方 wiki），后面带链接或文件位置。
> - 【读码推导】：数字来自源码，但结论是我们算的或解读的。
> - 【凭印象】：凭对游戏的了解写的，没找到来源。
> - 【未查到】：明确没找到，不编。
>
> **取证方式**：Evolve 源码于 2026-11-28 从 GitHub 克隆（HEAD 提交 `3436358`，2026-06-19）。Trimps、Kittens Game 同日克隆 master。Universal Paperclips 直接取自官网 `main.js` 和 `projects.js`。Melvor Idle 取自官方社区 wiki 页面。
> 源码行号随版本会变，下文给文件名和函数名。

---

## 1. 结论先行

1. **“密集轰炸”主要发生在头几分钟，不是头一小时。** Evolve 的演化阶段是一条“点几下就出下一个按钮”的连锁（2 点 RNA 出 DNA，10 点出细胞膜……）。进入文明阶段以后，科技价格按近似几何级数往上爬，到第一次重置要几天。【已证实】源码；重置时长见 2.1
2. **自动化的出场顺序是“先手动、再工人、再工匠、再队列、再总督”，且最后两层锁在时间或重置后面。** 工人（把闲人派去种田）在第一批建筑后就有。建造队列要 2500 知识。总督（governor）要先重置并花 300 质粒 + 25 噬菌体。【已证实】`tech.js`、`arpa.js`
3. **回馈衰减不靠单一倍率。** 建筑成本倍率在 1.12～1.36 之间（Trimps 1.185～1.3，Kittens 的小屋是 2.5），科技价格另走一条每级约 ×2～×3 的阶梯，两条曲线叠起来。【已证实】源码
4. **Evolve 的“墙”靠提示数字而不是弹窗。** 每个新按钮先显示价格，价格里的某种资源上限不够就是墙，破墙手段是“建仓库/换资源”。Kittens 用“资源达到价格的 30% 才出现按钮”来控制揭示。【已证实】`buildings.js`
5. **重置的奖励主要是“开局被压缩”和“成本倍率降低”，不是直接加产出。** Evolve 的 `creep` 基因把所有建筑成本倍率减 0.01～0.02；资深玩家的演化阶段预置进度。【已证实】`arpa.js`、`main.js`
6. **几乎不点击的 Melvor 靠“升级曲线”给密度。** 1 级到 2 级只要 83 XP，每 7 级成本翻一倍。头几分钟升级快，之后单次升级间隔自然拉长。【已证实】wiki

---

## 2. Evolve 详解

### 2.0 取证说明

| 项 | 状态 |
|---|---|
| 成本公式、解锁条件、重置公式（源码） | 【已证实】能查到 |
| 头 1/5/15/60 分钟的实测时间线 | 【未查到】。没有找到玩家或作者给出的分钟级记录。下面的顺序来自源码和新手攻略，时间是我们没法填的 |
| 第一次重置的总时长 | 只有搜索摘要里的论坛说法（见 2.1），没能打开原帖 |

源码位置：[`src/actions.js`](https://github.com/pmotschmann/Evolve/blob/master/src/actions.js)、[`src/tech.js`](https://github.com/pmotschmann/Evolve/blob/master/src/tech.js)、[`src/functions.js`](https://github.com/pmotschmann/Evolve/blob/master/src/functions.js)、[`src/main.js`](https://github.com/pmotschmann/Evolve/blob/master/src/main.js)、[`src/arpa.js`](https://github.com/pmotschmann/Evolve/blob/master/src/arpa.js)。新手攻略：[GreyCat 的 Evolve 指南](https://wooledge.org/~greg/evolve/guide.html)（针对 v1.0.1，2020-11 更新，比较旧）。

### 2.1 开局时间线（头 1 / 5 / 15 / 60 分钟）

**时间点本身没查到，下面按顺序列，不标分钟。**

**阶段 A：演化（纯点击）**【已证实】`main.js` 的 `protoplasm` 分支

- 开局只有一个按钮：点一下得 1 RNA。
- 揭示链（每一步满足上一步才出现）：

| 出现的按钮 | 触发条件 |
|---|---|
| DNA（2 RNA 换 1 DNA） | 拥有 2 RNA |
| 细胞膜 | 拥有 10 RNA |
| 细胞器 | 拥有 4 DNA |
| 细胞核 | 细胞器买了 2 个 |
| 真核细胞 | 细胞核买了 1 个 |
| 线粒体 | 真核细胞买了 1 个 |

- 价格是线性的，不是倍率：`evolveCosts` 返回 `已买数 × 增量 + 基数`（`actions.js`）。例：细胞膜 RNA = 2 + 2n；细胞器 RNA = 12 + 8n、DNA = 4 + 4n；细胞核 RNA = 38 + 16n（进度 ≥ 4 时）、DNA = 18 + 12n。
- 细胞器给每 tick 自动产 RNA，细胞核自动把 RNA 转成 DNA。所以这一段**在 1 到 2 分钟内就从“纯点击”变成“点击加自动”**。【读码推导】具体分钟没有实测。
- 《GreyCat 指南》称这段“somewhat click intensive”但“relatively short”。【已证实】

**阶段 B：原始文明（点击采集）**【已证实】`tech.js`、`actions.js`

顺序和价格：

| 步骤 | 价格 | 来源 |
|---|---|---|
| 点“采集木材”（无前置） | 免费，每次 +1 | `actions.js` city-lumber |
| 科技 `club`（木棒） | 5 木材 | `tech.js` |
| 点“采集食物”（要 club） | 免费，每次 +1 | `actions.js` city-food |
| 科技 `bone_tools`（骨器） | 10 食物 | `tech.js` |
| 点“采集石头”（要 bone_tools） | 免费，每次 +1 | `actions.js` city-stone |
| 科技 `sundial`（日晷） | 8 木材 + 10 石头 | `tech.js` |
| 科技 `housing`、`agriculture`、`mining`、`storage` 等 | 10～45 知识 | `tech.js` |

- 起始资源上限：食物 250、木材 200、石头 200、知识 100。【已证实】`resources.js` 的 `defineResources`
- 日晷之后才有被动的知识积累（`primitive` 到 3 级才有 `sundial_base = 1`，`main.js`）。【读码推导】每秒多少没核对
- 这一段全是手动点击，但**每点都能换来一个新按钮**。例：5 次点木材 → 木棒 → 食物按钮出现。

**阶段 C：文明初期（工人出现）**

新手攻略的顺序【已证实】（GreyCat 指南）：

1. 先点木材，研究木棒；点食物，研究骨器；点石头；然后造农场和住房。
2. 有了农场和住房，市民才会出现；在 Civics 标签把默认职业从“无业”改成“农民”，**“这就是被动食物”，之后不用点食物了（紧急时除外）**。
3. 造采石场和伐木场之后，把一部分农民改去伐木和采石，“基本结束手动采集”。
4. 铜矿工 → 铸造厂（Foundry）→ 工匠，把手动合成也自动化。

**1 小时内大概到哪**：【未查到】。攻略给出的整体顺序是：农场、住房、青铜锄 → 日晷、大学 → 铸造厂、图书馆 → 政府 → 疯狂科学、电力 → 遗传学 → 贸易、工业化 → 铀、火箭 → MAD。

**第一次重置要多久**：搜索摘要里转述了 2019 年的开发者 FAQ（Reddit r/EvolveIdle 帖），说第一次重置“约 3～4 天，如果非常活跃”，另有玩家说 4～5 天。【摘要】没能打开原帖（镜像 403）。时间是 2019 年版本的，之后游戏有过大改。

### 2.2 从手动到自动

| 手动行为 | 变自动的方式 | 触发时机 | 手动大概重复多少次 |
|---|---|---|---|
| 点 RNA | 买细胞器（自动产 RNA） | 揭示链第 3 步 | 约 10～20 次点击后第一个细胞器可买。【读码推导】10 RNA 出细胞膜，4 DNA 出细胞器，DNA 要 2 RNA 一个，所以约 14 RNA 的点击量 |
| RNA 转 DNA | 买细胞核 | 买 2 个细胞器后 | 同上，十几次 |
| 点食物 | 把无业市民设成农民 | 农场 + 住房建好、市民出现 | 取决于农场价格（50 钱 ×1.32ⁿ，20 木 ×1.36ⁿ，10 石 ×1.36ⁿ），【读码推导】首个农场前约 30 次木材/石头点击 |
| 点木材 / 石头 | 伐木场、采石场 + 改派工人 | 造出这两个建筑后 | 同上量级 |
| 手动合成（砖、板材等） | 铸造厂的工匠 | 铸造厂科技 650 知识，要 `mining: 2` | 未查到次数 |
| 手动逐个建造 | 建造队列（`urban_planning`） | 2500 知识，要 `banking: 2`、`currency: 2` | 整个文明初期都是手动 |
| 手动分配市民 | 总督（governor）自动任务 | 要先做 `governance` 基因：300 质粒 + 25 噬菌体，且要有 `queue: 2` | **第一轮根本没有**，要重置后才有 |

来源：`tech.js`（`club`、`foundry`、`urban_planning`、`governor`）、`arpa.js`（`governance`）。总督科技本身 1000 知识，但 `condition` 里要求 `global.genes['governor']`，即基因已解锁。

**要点**：Evolve 最后一级自动化（总督）被锁在第一次重置之后，这是我们没想到的做法。

### 2.3 回馈怎么衰减

**建筑成本公式**（`costMultiplier` 在 `functions.js`）：

> 价格 = round(倍率 ^ 已有数量 × 基数)；倍率下限 1.005。

| 建筑 | 基数 | 倍率 | 备注 |
|---|---|---|---|
| 基础住房 钱 | 20 | 1.17 | 前 5 座不收钱 |
| 基础住房 木 | 10 | 1.23 | |
| 农场 钱 / 木 / 石 | 50 / 20 / 10 | 1.32 / 1.36 / 1.36 | 钱从第 3 座起收 |
| 仓棚 钱 / 木 / 石 | 75 / 55 / 45 | 1.22 / 1.32 / 1.32 | |
| 筒仓 钱 / 木 / 石 | 85 / 65 / 50 | 1.32 / 1.36 / 1.36 | |
| 木屋（cottage）钱 | 900 | 1.15 | 价格里还有胶合板、砖、锻铁 ×1.25 |
| 公寓 钱 | 1750（再减 500） | 1.26 | 其他资源 1.32 |
| 磨坊 钱 | 1000 | 1.31 | |

来源：`actions.js` 中各建筑 `cost`。

**科技价格**（知识，`tech.js`）：10、10、20、22、45、55、65、80、90、100、180、350、500、650、720、750、900、1500、2500、4500、5000……一直到第一次重置科技 `mad` = 120,000 知识 + 8500 石油 + 1250 铀。【已证实】
【读码推导】从 10 知识（农业）到 120,000 知识（MAD）约 4 个数量级。早期 10 → 100 之间的科技密集，每升一级大约 ×1.5～×3。

**倍率会被重置降低**：`genetic_memory`（25 质粒）把建筑倍率减 0.01，`animus`（75 质粒）减 0.02（`arpa.js`、`strings.json`）。住房有专门的科技 `housing_reduction`，每级减 0.02（`functions.js`）。

**收入怎么涨**：
- 手动点击固定 +1，农民、矿工等按人头线性涨。【已证实】`actions.js`
- 住房加人口上限，人口按需增长。【凭印象】公式没细读
- 质粒给全局产出加成，是对数形：`(ln(质粒 + 50) − 3.91202) / 2.888`，质粒超过 250 + 噬菌体后增速再放缓（`resources.js` 的 `plasmidBonus`）。【读码推导】按公式 10 质粒约 +6%，100 质粒约 +38%，250 质粒约 +62%。是否直接按这个百分比乘到产出上，没核对

**里程碑间隔**：【未查到】分钟级实测。能确定的是知识价格每一步往上抬，而知识收入靠大学和教授线性涨，所以间隔变长。

### 2.4 玩家什么时候开始主动寻求提升

- 第一道墙：【未查到】玩家实际在哪里卡住。
- 源码里的“提示”手段：【已证实】
  - 价格直接展示。资源上限不够买，玩家自然去找仓库科技（`storage` 20 知识 → 仓棚；`containerization` 2700 知识）。
  - 揭示链让“下一个东西”总是已知的。
  - 攻略里的明确提示是：**“Anarchy 在人口 20～30 时放弃”**，换政府；以及铸造厂是铜矿工之后的下一个节点。
- Evolve 的“换思路”更多是**换职业配比、换政府、换科技线**，不是弹窗提示。【读码推导】
- 第一次重置（MAD）不强迫：官方 wiki 写“建议第一次尽早用”（`strings.json` 里 `wiki_resets_mad_para2`）。【已证实】

### 2.5 重置之后的开局

**保留什么**【已证实】：
- 质粒（MAD 重置所得）：`round(人口 / 3)` + 每花 10 万知识累加一次（步长按 ×1.1 递增），上限 150 × (1 + (难度等级 − 1)/8)，超出部分压缩（`functions.js` 的 `calcPrestige`、`getResetConstants`）。
- 基因（`arpa.js` 的 `genePool`）：25～75 质粒就能买 creep 系列。

**第二轮开局怎么变快**：
- 演化阶段预置：如果有 `master` 成就，演化阶段的细胞膜、细胞器、细胞核等直接按等级预置，跳过大部分点击（`main.js` 约 775～795 行）。【已证实】
- 质粒产出加成、建筑倍率降低。
- 总督（要 300 质粒 + 25 噬菌体）接管分配工人。

**第二轮比第一轮快多少**：【未查到】没有实测。

### 2.6 新系统的出场节奏

按科技价格排（知识）【已证实】`tech.js`：

| 知识价格 | 系统 |
|---|---|
| 10～45 | 农业、住房、采矿、堆肥 |
| 55～100 | 灌溉、仓库、银行、驻军 |
| 350～750 | 金属加工、铸造厂、水泥、政府、剧场 |
| 1000～2500 | 总督（需基因）、研究队列（2500）、狂热 / 人类学 |
| 3600～6750 | 木屋、冶炼、钢铁、贸易 |
| 13,500 | 电力 |
| 27,000～72,000 | 石油、铀等 |
| 120,000 | MAD（第一次重置） |

**价格差约 ×2～×3 一档**。真实时间间隔【未查到】。

---

## 3. 对照游戏

### 3.1 Trimps（带战斗和地图扩张）

- **开局**：【凭印象】手动点“搜索”“采集”，再造陷阱捕获小人。陷阱（Trap）价格 10 食物 + 10 木材，没有倍率。【已证实】`config.js` buildings.Trap
- **手动变自动**：
  - 战斗：`Battle` 科技 10 科学，`Bloodlust`（“教小人自己战斗”）60 科学 + 150 食物。【已证实】`config.js` upgrades
  - 陷阱：`Trapstorm`（自动补陷阱）10,000 科学 + 10 万食物 + 10 万木材。【已证实】
  - 其余见 `payoff-pacing-research.md` 第三节（第 1 / 10 / 20 个 zone 的自动化）。
- **成本倍率**：小屋 125 食物 / 75 木 ×1.24；房子 1500 / 750 / 150 ×1.22；健身房 400 木 ×1.185；Coordination 科学 250、食物 600 等 ×1.3；速度类书籍（Speedminer 等）×1.4，每本 +25% 效率。【已证实】`config.js`
- **墙**：打不过当前 zone → 刷地图。地图加成见 `payoff-pacing-research.md`。
- **重置**：Portal 在第 20 个 zone 开放，见已有文档。
- **时间线（分钟）**：【未查到】

### 3.2 Kittens Game（以解锁新系统闻名）

- **开局**：点“采集猫薄荷”，攒够后造田地。【凭印象】
- **揭示规则（我们没想到）**：建筑按钮在**库存达到价格的 30%** 时才出现（`unlockRatio: 0.3`）。【已证实】`js/buildings.js`（约 2838～2846 行）。矿井是 0.15，工坊是 0.0025。
- **价格**【已证实】`buildings.js`：

| 建筑 | 基础价 | 倍率 |
|---|---|---|
| 田地 | 10 猫薄荷 | 1.12 |
| 小屋 | 5 木 | **2.5** |
| 谷仓 | 50 木 | 1.75 |
| 矿井 | 100 木 | 1.15 |
| 工坊 | 100 木 + 400 矿 | 1.15 |

- **科技阶梯（科学）**【已证实】`js/science.js`：日历 30、农业 100、弓术 300、采矿 500、金属 900、建筑学 1300。
- **自动化**：工坊和 `factoryAutomation` 等，见已有文档。
- **时间**：游戏内 1 日 = 10 tick（`calendar.js`：`ticksPerDay: 10`，每季 100 天）。真实时间节奏【未查到】。

### 3.3 Universal Paperclips（解锁新系统）

- **开局**：点“制造回形针”。【已证实】
- **揭示**（`main.js`、`projects.js`）：
  - 自动剪辑机在资金 ≥ $5 时出现。
  - 计算 / 项目面板在累计 2000 个回形针时开放，或者缺线又缺钱时提前开放（`compFlag`）。
  - 第一个项目“Improved AutoClippers”要 750 运算，触发条件是已有至少 1 台自动剪辑机；后面几个是 2500、5000。
- **成本曲线**：自动剪辑机 `1.1^n + 5`；兆级剪辑机 `1000 × 1.07^n`；广告每次翻倍（`adCost × 2`）。【已证实】
- **阶段切换**：第一阶段约 3～5 小时，见已有文档。
- **手动变自动**：自动剪辑机在第 1 分钟内出现；线材购买器等后来开放（`wireBuyerFlag`）。时间【未查到】

### 3.4 Melvor Idle（几乎不点击）

- **开局**：选一个技能开始，如钓鱼。1 级只能钓生虾，每次 +5 XP。【已证实】[Beginner's Guide](https://wiki.melvoridle.com/w/Beginner%27s_Guide)
- **升级曲线**：相邻两级 XP 差约 `floor((L − 1 + 300 × 2^((L−1)/7)) / 4)`，约每 7 级翻一倍；1→2 级 83 XP，9→10 级 185 XP，19→20 级 497 XP，98→99 级约 90 万。【已证实】[Experience Table](https://wiki.melvoridle.com/w/Experience)
- **【读码推导】**生虾 5 XP，一级 83 XP，约 17 次动作升第一级。每次动作间隔没查到。
- **“解决开局密度”的做法**：靠多个技能并行升级、前期每级都很快、新物品和装备按等级逐个解锁。攻略建议“把所有技能升到 30 左右”。【已证实】
- **离线**：最多补算 24 小时，战斗和偷窃默认关闭。【已证实】[Offline Progression](https://wiki.melvoridle.com/w/Offline_Progression)
- **没有重置**：Melvor 本体不靠重置回路，密度靠技能数量。其他方面（时间线、自动化时点）【未查到】。

---

## 4. 横向比较表

“头一小时里程碑数”和“第一次重置时间”大多没有实测，写【未查到】，不拿估计值填。

| 游戏 | 头 1 小时里程碑数 | 第一个自动化出现 | 成本倍率 | 第一次重置 |
|---|---|---|---|---|
| Evolve | 【未查到】（演化段 6 步揭示链，源码确认） | 约 1～2 分钟（细胞器自动产 RNA）；文明段第一批农场和市民后改工人 | 建筑 1.15～1.36；科技约 ×2～×3 一档 | 约 3～5 天（2019 论坛转述，摘要，未核实） |
| Trimps | 【未查到】 | 第 1 个 zone 自动出战（已有文档，源码） | 1.185～1.4 | 第 20 个 zone 开 Portal（已有文档） |
| Kittens Game | 【未查到】 | 【未查到】（有工坊、猫工人） | 1.12～2.5；小屋 2.5 | 【未查到】 |
| Universal Paperclips | 【未查到】 | 资金 $5 时出现自动剪辑机 | 1.1（自动）、1.07（兆级）、2（广告） | 无重置；第一阶段 3～5 小时（已有文档，wiki） |
| Melvor Idle | 【未查到】 | 开局就是自动（选了技能就开始） | 经验曲线每 7 级 ×2（约每级 ×1.10） | 无 |

---

## 5. 对本项目的启示

### 5.1 被调研支持的

- **“先手动，再逐个自动”**：Evolve 演化段、文明段、Paperclips 自动剪辑机都是这样。每个手动动作到自动之间的重复次数很少，Evolve 大约十几次点击。【已证实】
- **成本倍增让回馈自然衰减**：所有对照游戏都用 1.1～1.4 的倍率。
- **撞墙后换思路**：Evolve 的科技阶梯、Trimps 的刷地图都是“墙 + 破墙手段”。

### 5.2 要修正的

1. **“开局密集”只覆盖头几分钟。** Evolve 第一次重置要几天，而不是“密集一小时后就转入墙”。我们的第一周如果也按一周排，要有第二层、第三层“小爆发”接上，不要只靠一个开局爆发。【读码推导】
2. **倍率不是唯一的衰减来源。** Evolve、Kittens 的科技（学说）价格是另一条阶梯；Evolve 的建筑倍率范围是 1.15～1.36，不是单一数字。我们的“成本按倍数涨”要区分“建筑倍率”和“研究价格阶梯”。
3. **最后一级自动化放在重置之后。** 总督要先花质粒才能开，而且它的前置里还有“建造队列”科技。我们如果让规章开局全开（`payoff-pacing-research.md` 第六节已经指出这一点），就失去了一个爽点。

### 5.3 我们没想到的做法

| 做法 | 来源 | 怎么用 |
|---|---|---|
| 揭示链：每个按钮出现前要先买上一个的少量 | Evolve `main.js` | 开局每次派遣后才露出下一个部门 |
| 库存达到价格的 30% 才显示按钮（Kittens `unlockRatio`） | `buildings.js` | 价格对玩家可见前先给“快够了”的预告 |
| 重置降低成本倍率而不是加产出 | Evolve `creep` 基因 | 轮回奖励改成“下一轮建筑倍率 −0.01” |
| 重置奖励有上限，超出部分压缩（防滚雪球） | `calcPrestige` | 控制第二轮不会比第一轮快过头 |
| 演化阶段按等级预置进度 | `main.js` | 第二轮跳过手动派遣教学段 |
| 点击收益被物种放大（Giants 点击 ×4） | GreyCat 指南 | 手动派遣的收益可以按小队构成变化，让“继续手动”也有选择 |
| 紧急时仍然手动（攻略建议保留“采集食物”作为备用） | GreyCat 指南 | 自动后仍保留手动入口，兜底而不是废掉 |
| 经验曲线每 7 级翻倍（Melvor） | wiki | 用于研究价格：每级约 ×1.10，比我们的线性 10+18n 更接近“自然衰减” |

### 5.4 没法下结论的

- 开局头 5 / 15 / 60 分钟各应该放几个里程碑：所有对照游戏都没找到实测。建议拿 `eval` 的 `greedy` 机器人在原型里自己测，不要照搬数字。
- 手动要重复多少次：Evolve 演化段约十几次点击是源码推导，文明段次数没查到。

---

## 6. 来源与未查到

### 6.1 来源

- Evolve 源码（GitHub master，2026-11-28 克隆，HEAD `3436358`）：[actions.js](https://github.com/pmotschmann/Evolve/blob/master/src/actions.js)、[tech.js](https://github.com/pmotschmann/Evolve/blob/master/src/tech.js)、[functions.js](https://github.com/pmotschmann/Evolve/blob/master/src/functions.js)、[main.js](https://github.com/pmotschmann/Evolve/blob/master/src/main.js)、[arpa.js](https://github.com/pmotschmann/Evolve/blob/master/src/arpa.js)、[resources.js](https://github.com/pmotschmann/Evolve/blob/master/src/resources.js)、[strings.json](https://github.com/pmotschmann/Evolve/blob/master/strings/strings.json)
- 在线版：[Evolve](https://pmotschmann.github.io/Evolve/)（未直接试玩）
- [GreyCat 的 Evolve 指南](https://wooledge.org/~greg/evolve/guide.html)
- Evolve 开发者 FAQ（Reddit r/EvolveIdle，经搜索摘要转述，原帖镜像 403）：<https://libreddit.winscloud.net/r/EvolveIdle/comments/bkc7mw/evolve_faq>
- Trimps 源码：[config.js](https://github.com/Trimps/Trimps.github.io/blob/master/config.js)
- Kittens Game 源码：[buildings.js](https://github.com/nuclear-unicorn/kittensgame/blob/master/js/buildings.js)、[science.js](https://github.com/nuclear-unicorn/kittensgame/blob/master/js/science.js)、[calendar.js](https://github.com/nuclear-unicorn/kittensgame/blob/master/js/calendar.js)
- Universal Paperclips 源码：[main.js](https://www.decisionproblem.com/paperclips/main.js)、[projects.js](https://www.decisionproblem.com/paperclips/projects.js)
- Melvor Idle wiki：[Experience Table](https://wiki.melvoridle.com/w/Experience)、[Beginner's Guide](https://wiki.melvoridle.com/w/Beginner%27s_Guide)、[Offline Progression](https://wiki.melvoridle.com/w/Offline_Progression)

### 6.2 没查到的事项

1. Evolve 头 1 / 5 / 15 / 60 分钟的实测时间线。
2. Evolve 第一次重置的确切时长（只有 2019 年论坛转述的“几天”，没核对原帖）。
3. Evolve 第二轮比第一轮快多少。
4. Evolve 文明段手动采集在转为工人前要点多少次的实测。
5. Evolve 第一道墙在哪一步（没有玩家记录）。
6. Evolve 质粒加成在游戏界面上的实际百分比（公式读到了，没核对单位）。
7. Trimps、Kittens Game 头一小时的时间线和重置时间。
8. Paperclips 各项目的触发时间。
9. Melvor 每次动作的间隔，以及升到 30 级所需的真实时间。
10. 作者对“开局节奏”的任何公开说明。Evolve 作者没找到。
