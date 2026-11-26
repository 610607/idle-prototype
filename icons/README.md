# 图标集（icons/）

16×16 像素图标，一个图标一个文本文件：`icons/pixel/<id>.txt`。总览见 [preview.svg](preview.svg)。

## 格式
每个文件正好 16 行、每行 16 个字符，末尾换行，没有别的内容：

| 字符 | 含义 | 界面里的颜色 |
|---|---|---|
| `.` | 透明 | — |
| `#` | 主墨 | 使用处的文字颜色（`currentColor`） |
| `+` | 阴影 | 同上，45% 不透明 |
| `*` | 点缀 | 警示黄（CSS 变量 `--px-accent`），每个图标最多 10 个 |

界面（`web/src/icons.tsx`）把每行的连续同色像素合成一条 SVG 路径，用 `crispEdges` 画，只在 16 的整数倍尺寸（16/32/48px）显示，所以不会糊。

## 风格规则
- 民防 × 锈蚀工业的末世调子：像老式 16px 物品图标，不是线稿。
- 四周留 1px 空白（画在第 1..14 行/列）。
- 1× 下要认得出：主轮廓尽量 2px 粗；不要孤立的单个像素，不要 1px 的斜线噪点。
- 去掉 `+` 以后仍然能认出来；`*` 只用在眼睛、灯、火花、裂隙核心这类点睛处。
- 同一类（技能、设施……）视觉分量一致，彼此轮廓不相似；六个 `tag-*` 最常用、显示最小，尤其要互相区分。
- 内容数据（`engine/content/*.json`）只存 icon id（如 `"icon": "tag-shock"`），不存图形。

## 新增或修改图标
1. 定 id：内容图标用类别前缀（class-/enemy-/equip-/fac-/prof-/rift-/skill-/tag-/trait-），界面图标用语义名（如 res-scrap、tile-hq、lock）。
2. 新建 `icons/pixel/<id>.txt`，从相近的已有图标改起。
3. 运行 `node icons/preview.mjs` 重新生成 `preview.svg`，在深色底上看 1× 和 2×，确认清晰、不与同类撞轮廓，再在 content JSON 里引用这个 id。
