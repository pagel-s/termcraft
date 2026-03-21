use crate::world::item::ItemType;

pub struct ItemEntity {
    pub x: f64,
    pub y: f64,
    pub vx: f64,
    pub vy: f64,
    pub item_type: ItemType,
    pub grounded: bool,
    pub age: u64, // Used for bobbing animation and despawning
}

impl ItemEntity {
    pub fn new(x: f64, y: f64, item_type: ItemType) -> Self {
        Self {
            x,
            y,
            vx: 0.0,
            vy: 0.0,
            item_type,
            grounded: false,
            age: 0,
        }
    }

    pub fn get_bobbing_offset(&self) -> f64 {
        (self.age as f64 * 0.1).sin() * 0.2
    }
}
