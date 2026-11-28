//! 实体：编号、定义、品质、标签、位置和状态（docs/design-v2-refactor.md，“目标类型”）。
//! 引擎层只管“谁是谁、在哪”，不认识任何游戏内容：定义 id 是一个不透明的字符串，状态的类型
//! 由机制层给，实体表按编号排好。

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// A definition id: names an entry of the data layer's definition table.
pub type DefId = String;

/// An entity's number. Numbers below [`HIGH`] come from the game's own counter (the one events and
/// reports also draw from); things that become entities later draw from [`Store::alloc_high`].
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct EntityId(pub u32);

impl PartialEq<u32> for EntityId {
    fn eq(&self, other: &u32) -> bool {
        self.0 == *other
    }
}

/// First number of the high range ([`Store::alloc_high`]).
pub const HIGH: u32 = 1 << 30;

/// A container's number (containers come with step C1).
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ContainerId(pub u32);

/// Where an entity is: a container and, for a container with positions, which one.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Place {
    pub container: ContainerId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub slot: Option<u16>,
}

fn is_zero(v: &u8) -> bool {
    *v == 0
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Entity<S> {
    pub id: EntityId,
    pub def: DefId,
    /// Quality; 0 everywhere during the refactor.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub tier: u8,
    /// Tags kept between battles (none yet: battle tags do not leave the battle).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
    /// The container it is in; `None` when it is nowhere (every entity until C1).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub at: Option<Place>,
    pub state: S,
}

impl<S> Entity<S> {
    pub fn new(id: EntityId, def: impl Into<DefId>, state: S) -> Self {
        Entity {
            id,
            def: def.into(),
            tier: 0,
            tags: vec![],
            at: None,
            state,
        }
    }
}

/// The entity store: every entity by number, iterated in number order.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(bound(serialize = "S: Serialize", deserialize = "S: Deserialize<'de>"))]
#[serde(from = "Repr<S>", into = "Repr<S>")]
pub struct Store<S: Clone> {
    items: BTreeMap<EntityId, Entity<S>>,
    next_high: u32,
}

#[derive(Clone, Serialize, Deserialize)]
struct Repr<S> {
    next_high: u32,
    items: Vec<Entity<S>>,
}

impl<S: Clone> From<Repr<S>> for Store<S> {
    fn from(r: Repr<S>) -> Self {
        Store {
            items: r.items.into_iter().map(|e| (e.id, e)).collect(),
            next_high: r.next_high,
        }
    }
}

impl<S: Clone> From<Store<S>> for Repr<S> {
    fn from(s: Store<S>) -> Self {
        Repr {
            next_high: s.next_high,
            items: s.items.into_values().collect(),
        }
    }
}

impl<S: Clone> Default for Store<S> {
    fn default() -> Self {
        Store {
            items: BTreeMap::new(),
            next_high: HIGH,
        }
    }
}

impl<S: Clone> Store<S> {
    /// A number from the high range, never handed out before in this store.
    pub fn alloc_high(&mut self) -> EntityId {
        let id = EntityId(self.next_high);
        self.next_high += 1;
        id
    }

    /// Put `e` in under its number. Panics when the number is taken: two things with one number
    /// would be a bug in whoever handed it out.
    pub fn insert(&mut self, e: Entity<S>) {
        let id = e.id;
        if self.items.insert(id, e).is_some() {
            panic!("entity {} inserted twice", id.0);
        }
    }

    pub fn remove(&mut self, id: EntityId) -> Option<Entity<S>> {
        self.items.remove(&id)
    }

    pub fn get(&self, id: EntityId) -> Option<&Entity<S>> {
        self.items.get(&id)
    }

    pub fn get_mut(&mut self, id: EntityId) -> Option<&mut Entity<S>> {
        self.items.get_mut(&id)
    }

    pub fn contains(&self, id: EntityId) -> bool {
        self.items.contains_key(&id)
    }

    /// Every entity in number order.
    pub fn iter(&self) -> impl Iterator<Item = &Entity<S>> + '_ {
        self.items.values()
    }

    pub fn iter_mut(&mut self) -> impl Iterator<Item = &mut Entity<S>> + '_ {
        self.items.values_mut()
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn iterates_by_number_and_round_trips() {
        let mut s: Store<i32> = Store::default();
        s.insert(Entity::new(EntityId(7), "b", 1));
        s.insert(Entity::new(EntityId(3), "a", 2));
        let h = s.alloc_high();
        assert_eq!(h, EntityId(HIGH));
        assert_eq!(s.alloc_high(), EntityId(HIGH + 1));
        let ids: Vec<u32> = s.iter().map(|e| e.id.0).collect();
        assert_eq!(ids, vec![3, 7]);
        let json = serde_json::to_string(&s).unwrap();
        let back: Store<i32> = serde_json::from_str(&json).unwrap();
        assert_eq!(serde_json::to_string(&back).unwrap(), json);
        assert_eq!(back.alloc_high_peek(), HIGH + 2);
    }

    impl<S: Clone> Store<S> {
        fn alloc_high_peek(&self) -> u32 {
            self.next_high
        }
    }
}
