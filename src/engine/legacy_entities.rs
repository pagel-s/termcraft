//! Frozen native-v9 layouts. Serde defaults cannot extend bincode structs.
use super::*;

#[derive(Clone, Debug, Deserialize)]
pub(super) struct ItemEntityV9 {
    x: f64,
    y: f64,
    vx: f64,
    vy: f64,
    item_type: ItemType,
    grounded: bool,
    age: u64,
}
impl From<ItemEntityV9> for ItemEntity {
    fn from(e: ItemEntityV9) -> Self {
        Self {
            x: e.x,
            y: e.y,
            vx: e.vx,
            vy: e.vy,
            item_type: e.item_type,
            grounded: e.grounded,
            age: e.age,
            payload: None,
            pickup_delay: 0,
        }
    }
}
#[derive(Clone, Debug, Deserialize)]
pub(super) struct FireballV9 {
    x: f64,
    y: f64,
    vx: f64,
    vy: f64,
    age: u64,
    dead: bool,
}
impl From<FireballV9> for Fireball {
    fn from(e: FireballV9) -> Self {
        Self {
            x: e.x,
            y: e.y,
            vx: e.vx,
            vy: e.vy,
            age: e.age,
            dead: e.dead,
            reflected: false,
        }
    }
}
