# 图标集（icons/）

"纸面地图 + 墨线"风格的单色线性图标，一个图标一个文件：`icons/<id>.svg`。

## 风格规则
- 24 网格：根元素固定为 `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round">`，逐字不改。
- 1.75 描边，圆头圆角；单色 `currentColor`，颜色由使用处的 CSS `color` 决定，文件里不写任何颜色。
- 不填充；唯一例外是小圆点 `<circle r≤1.5 fill="currentColor" stroke="none"/>`。
- 2px 内边距：图形落在 2..22 之内，坐标对齐 0.5。
- 每个图标 1–6 个元素（path/circle/line/rect/polyline）；禁止 text、transform、id/class/style 和注释；16px 下必须能认出来。
- 六个 tag-* 图标最常用、显示最小，修改时保持它们的轮廓互不相似。
- 内容数据（engine/content/*.json）只存 icon id（如 `"icon": "tag-shock"`），不存 SVG 或路径；界面按 id 读取 `icons/<id>.svg`。

## 新增图标
1. 定 id：内容图标用类别前缀（class-/enemy-/equip-/fac-/prof-/rift-/skill-/tag-/trait-），界面图标用语义名（如 res-scrap、tile-hq、lock）。
2. 新建 `icons/<id>.svg`，复制上面的根元素，从相近的已有图标改起，在 24 网格上画线。
3. 在白底 16/24/48px 下预览，确认清晰、不出界、不与同类图标撞轮廓，再在 content JSON 里引用这个 id。
