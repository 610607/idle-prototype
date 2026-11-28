//! 实体：编号、定义、品质、标签、位置和状态（docs/design-v2-refactor.md，“目标类型”）。
//! 引擎层只管“谁是谁、在哪”，不认识任何游戏内容：定义 id 是一个不透明的字符串，状态的类型
//! 由机制层给。

/// A definition id: names an entry of the data layer's definition table.
pub type DefId = String;
