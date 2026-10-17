use crate::world::item::{ItemStack, ItemType};

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct DroppedStack {
    pub stack: ItemStack,
    pub enchant_level: u8,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ItemEntity {
    pub x: f64,
    pub y: f64,
    pub vx: f64,
    pub vy: f64,
    pub item_type: ItemType,
    pub grounded: bool,
    pub age: u64, // Used for bobbing animation and despawning
    #[serde(default)]
    pub payload: Option<DroppedStack>,
    #[serde(default)]
    pub pickup_delay: u8,
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
            payload: None,
            pickup_delay: 0,
        }
    }

    pub fn get_bobbing_offset(&self) -> f64 {
        (self.age as f64 * 0.1).sin() * 0.2
    }
}
