//! 定义表：职业、敌人、设施、装备、学说、政策，以及民兵和总部守卫，读入时各自转成一份
//! [`EntityDef`]，进同一张按 id 查的表（docs/design-v2-refactor.md，B1）。
//!
//! 旧的定义结构（`ClassDef`、`EnemyDef`……）暂时都还在，机制层大多仍读它们；这张表是将来
//! 实体（`kernel::entity`）的 `def` 指向的地方。一个定义有哪些接口块，决定了这种东西能做什么：
//! 能出战的有 `fighter`，能放进格子槽的有 `placeable`，能装到别人身上的有 `fitting`……
//! 代码问“有没有这个块”或“带不带这个类别标签”，不问“是哪一种”。
//!
//! 还没有读者的块（`mover`、`passable`、`scoring`）先是空的结构，不往里放没用到的东西。

use super::{
    BaseStats, ClassDef, DoctrineDef, EnemyDef, EquipDef, FacilityDef, Gambit, PolicyDef,
    Resources, Row, Stat, Tag, TraitEffect,
};
use crate::kernel::entity::DefId;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// 类别标签（定义的 `tags`）。城市那边的类别和战斗标签将来进同一张标签表（D1），现在只是字符串。
pub mod cat {
    /// 队员（职业）。
    pub const OPERATIVE: &str = "operative";
    pub const ENEMY: &str = "enemy";
    pub const FACILITY: &str = "facility";
    pub const EQUIPMENT: &str = "equipment";
    pub const DOCTRINE: &str = "doctrine";
    pub const POLICY: &str = "policy";
    /// 不是队员的守军：街区民兵和总部守卫（军械的 `militia` 轨道给的就是它们）。
    pub const MILITIA: &str = "militia";
}

/// 出战方式。
#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum Deploy {
    /// 上场：占位置、能被选中、有回合。
    #[default]
    Field,
    /// 支援：不占位置、不能被选中、没有回合，只用被动技能（守城建筑）。
    Support,
}

/// 默认指令（战术规则）。
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Orders {
    /// 写在定义里的一串。
    Listed(Vec<Gambit>),
    /// 按技能生成（敌人：带“消耗某标签”的技能配“敌人有这个标签”的条件，见 `combat::enemy_spec`）。
    Auto,
}

/// 可出战：基础数值、技能、默认指令、出战方式。
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FighterDef {
    pub row: Row,
    pub base: BaseStats,
    /// 攻击力按哪项属性换算（队员）。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub atk_stat: Option<Stat>,
    pub skills: Vec<String>,
    pub orders: Orders,
    #[serde(default)]
    pub deploy: Deploy,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub start_tags: Vec<(Tag, i32)>,
}

/// 可放置：能放进什么槽。
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Placeable {
    pub slot: String,
}

/// 可装配：装到带什么类别标签的宿主的什么槽。
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Fitting {
    pub host: String,
    pub slot: String,
}

/// 可转化：经济上的投入产出（保持设施现有的字段）。
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Converter {
    pub profession: Option<String>,
    pub jobs: i32,
    pub power: i32,
    pub supply: i32,
    pub cost: Resources,
    pub output: BTreeMap<String, i32>,
}

/// 有槽位：一个槽的名字。
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SlotDef {
    pub id: String,
}

/// 可行进。还没有读者。
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Mover {}

/// 可经过。还没有读者。
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Passable {}

/// 计分。还没有读者（分数的权重在 `balance.score`）。
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Scoring {}

/// 一种东西的定义。
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EntityDef {
    pub id: DefId,
    /// 图标 id（`icons/pixel/<icon>.txt`）；政策没有图标，是空的。
    pub icon: String,
    /// 类别标签（[`cat`]）。
    pub tags: Vec<String>,
    pub placeable: Option<Placeable>,
    pub fighter: Option<FighterDef>,
    pub mover: Option<Mover>,
    pub passable: Option<Passable>,
    pub fitting: Option<Fitting>,
    pub converter: Option<Converter>,
    pub slots: Vec<SlotDef>,
    pub scoring: Option<Scoring>,
    /// 这个东西带的效果。现在是旧的词条效果，D 段换成句式。
    pub effects: Vec<TraitEffect>,
}

impl EntityDef {
    fn new(id: &str, icon: &str, tags: Vec<String>) -> Self {
        EntityDef {
            id: id.to_string(),
            icon: icon.to_string(),
            tags,
            placeable: None,
            fighter: None,
            mover: None,
            passable: None,
            fitting: None,
            converter: None,
            slots: vec![],
            scoring: None,
            effects: vec![],
        }
    }

    pub fn has_tag(&self, tag: &str) -> bool {
        self.tags.iter().any(|t| t == tag)
    }

    /// The fighter block (panics for a definition without one: a caller that fields it knows).
    pub fn fighter(&self) -> &FighterDef {
        self.fighter
            .as_ref()
            .unwrap_or_else(|| panic!("definition {} cannot fight", self.id))
    }
}

/// 民兵和总部守卫（`content/defenders.json`）：不是队员的守军。名字随人数变，在代码里拼；
/// 生命和攻击来自 `balance.json`（民兵还按人数加），这里的 `base.hp`、`base.atk` 不用。
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DefenderDef {
    pub id: String,
    pub icon: String,
    #[serde(default)]
    pub tags: Vec<String>,
    pub row: Row,
    pub base: BaseStats,
    pub skills: Vec<String>,
    pub gambits: Vec<Gambit>,
}

fn tags(list: &[&str]) -> Vec<String> {
    list.iter().map(|s| s.to_string()).collect()
}

pub fn of_class(c: &ClassDef) -> EntityDef {
    let mut d = EntityDef::new(&c.id, &c.icon, tags(&[cat::OPERATIVE, &c.side]));
    d.fighter = Some(FighterDef {
        row: c.row,
        base: c.base.clone(),
        atk_stat: Some(c.atk_stat),
        skills: c.skills.clone(),
        orders: Orders::Listed(c.gambits.clone()),
        deploy: Deploy::Field,
        start_tags: vec![],
    });
    d.slots = vec![SlotDef {
        id: cat::EQUIPMENT.into(),
    }];
    d
}

pub fn of_enemy(e: &EnemyDef) -> EntityDef {
    let mut d = EntityDef::new(&e.id, &e.icon, tags(&[cat::ENEMY, &e.terrain]));
    d.fighter = Some(FighterDef {
        row: e.row,
        base: e.base.clone(),
        atk_stat: None,
        skills: e.skills.clone(),
        orders: Orders::Auto,
        deploy: Deploy::Field,
        start_tags: e.start_tags.clone(),
    });
    d
}

pub fn of_defender(m: &DefenderDef) -> EntityDef {
    let mut t = m.tags.clone();
    if !t.iter().any(|x| x == cat::MILITIA) {
        t.insert(0, cat::MILITIA.into());
    }
    let mut d = EntityDef::new(&m.id, &m.icon, t);
    d.fighter = Some(FighterDef {
        row: m.row,
        base: m.base.clone(),
        atk_stat: None,
        skills: m.skills.clone(),
        orders: Orders::Listed(m.gambits.clone()),
        deploy: Deploy::Field,
        start_tags: vec![],
    });
    d
}

pub fn of_facility(f: &FacilityDef) -> EntityDef {
    let kind = serde_json::to_value(f.kind)
        .ok()
        .and_then(|v| v.as_str().map(str::to_string))
        .unwrap_or_default();
    let mut d = EntityDef::new(&f.id, &f.icon, vec![cat::FACILITY.into(), kind]);
    d.placeable = Some(Placeable {
        slot: "building".into(),
    });
    d.converter = Some(Converter {
        profession: f.profession.clone(),
        jobs: f.jobs,
        power: f.power,
        supply: f.supply,
        cost: f.cost.clone(),
        output: f.output.clone(),
    });
    if let Some(fx) = &f.fixture {
        d.fighter = Some(FighterDef {
            row: Row::Back,
            base: BaseStats {
                hp: 1,
                atk: fx.atk,
                def: 0,
                spd: 0,
                ap: 0,
                pp: 1,
            },
            atk_stat: None,
            skills: fx.skills.clone(),
            orders: Orders::Listed(vec![]),
            deploy: Deploy::Support,
            start_tags: vec![],
        });
        d.effects = fx.effects.clone();
    }
    d
}

pub fn of_equip(e: &EquipDef) -> EntityDef {
    let mut d = EntityDef::new(&e.id, &e.icon, tags(&[cat::EQUIPMENT]));
    d.fitting = Some(Fitting {
        host: cat::OPERATIVE.into(),
        slot: cat::EQUIPMENT.into(),
    });
    d.effects = e.effects.clone();
    d
}

pub fn of_doctrine(x: &DoctrineDef) -> EntityDef {
    let mut d = EntityDef::new(&x.id, &x.icon, tags(&[cat::DOCTRINE]));
    // 学说现在挂在阵营上（对全体己方生效）；挪到小队是阶段 4 的事。
    d.fitting = Some(Fitting {
        host: "faction".into(),
        slot: cat::DOCTRINE.into(),
    });
    d.effects = x.effects.clone();
    d
}

pub fn of_policy(p: &PolicyDef) -> EntityDef {
    let mut d = EntityDef::new(&p.id, "", tags(&[cat::POLICY]));
    d.fitting = Some(Fitting {
        host: "district".into(),
        slot: cat::POLICY.into(),
    });
    d
}

/// The one table, keyed by id. Ids are unique across kinds today; a clash would be kept under
/// `<kind>:<id>` and reported in `clashes` (content validation fails on it).
pub fn table(defs: impl IntoIterator<Item = (&'static str, EntityDef)>) -> Table {
    let mut t = Table::default();
    for (kind, d) in defs {
        if t.by_id.contains_key(&d.id) {
            t.clashes
                .push(format!("definition id {} is used twice ({kind})", d.id));
            let key = format!("{kind}:{}", d.id);
            t.by_id.insert(key, d);
        } else {
            t.by_id.insert(d.id.clone(), d);
        }
    }
    t
}

#[derive(Default)]
pub struct Table {
    pub by_id: BTreeMap<DefId, EntityDef>,
    pub clashes: Vec<String>,
}
