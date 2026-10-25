//! Frozen native-v11 animal layouts, before husbandry was stored.
use super::*;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(super) struct CowV11 {
    pub x: f64,
    pub y: f64,
    pub vx: f64,
    pub vy: f64,
    pub grounded: bool,
    pub facing_right: bool,
    pub jump_cooldown: u8,
    pub wander_timer: u16,
    pub age: u64,
    pub health: f32,
    pub hit_timer: u8,
    pub last_player_damage_tick: u64,
    #[serde(default)]
    pub hazards: crate::entities::hazards::HazardState,
}

impl From<CowV11> for Cow {
    fn from(e: CowV11) -> Self {
        Self {
            x: e.x,
            y: e.y,
            vx: e.vx,
            vy: e.vy,
            grounded: e.grounded,
            facing_right: e.facing_right,
            jump_cooldown: e.jump_cooldown,
            wander_timer: e.wander_timer,
            age: e.age,
            health: e.health,
            hit_timer: e.hit_timer,
            last_player_damage_tick: e.last_player_damage_tick,
            hazards: e.hazards,
            husbandry: Default::default(),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(super) struct SheepV11 {
    pub x: f64,
    pub y: f64,
    pub vx: f64,
    pub vy: f64,
    pub grounded: bool,
    pub facing_right: bool,
    pub jump_cooldown: u8,
    pub wander_timer: u16,
    pub age: u64,
    pub health: f32,
    pub hit_timer: u8,
    pub last_player_damage_tick: u64,
    pub sheared: bool,
    #[serde(default)]
    pub hazards: crate::entities::hazards::HazardState,
}

impl From<SheepV11> for Sheep {
    fn from(e: SheepV11) -> Self {
        Self {
            x: e.x,
            y: e.y,
            vx: e.vx,
            vy: e.vy,
            grounded: e.grounded,
            facing_right: e.facing_right,
            jump_cooldown: e.jump_cooldown,
            wander_timer: e.wander_timer,
            age: e.age,
            health: e.health,
            hit_timer: e.hit_timer,
            last_player_damage_tick: e.last_player_damage_tick,
            sheared: e.sheared,
            hazards: e.hazards,
            husbandry: Default::default(),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(super) struct PigV11 {
    pub x: f64,
    pub y: f64,
    pub vx: f64,
    pub vy: f64,
    pub grounded: bool,
    pub facing_right: bool,
    pub jump_cooldown: u8,
    pub wander_timer: u16,
    pub age: u64,
    pub health: f32,
    pub hit_timer: u8,
    pub last_player_damage_tick: u64,
    #[serde(default)]
    pub hazards: crate::entities::hazards::HazardState,
}

impl From<PigV11> for Pig {
    fn from(e: PigV11) -> Self {
        Self {
            x: e.x,
            y: e.y,
            vx: e.vx,
            vy: e.vy,
            grounded: e.grounded,
            facing_right: e.facing_right,
            jump_cooldown: e.jump_cooldown,
            wander_timer: e.wander_timer,
            age: e.age,
            health: e.health,
            hit_timer: e.hit_timer,
            last_player_damage_tick: e.last_player_damage_tick,
            hazards: e.hazards,
            husbandry: Default::default(),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(super) struct ChickenV11 {
    pub x: f64,
    pub y: f64,
    pub vx: f64,
    pub vy: f64,
    pub grounded: bool,
    pub facing_right: bool,
    pub jump_cooldown: u8,
    pub wander_timer: u16,
    pub egg_lay_timer: u16,
    pub age: u64,
    pub health: f32,
    pub hit_timer: u8,
    pub last_player_damage_tick: u64,
    #[serde(default)]
    pub hazards: crate::entities::hazards::HazardState,
}

impl From<ChickenV11> for Chicken {
    fn from(e: ChickenV11) -> Self {
        Self {
            x: e.x,
            y: e.y,
            vx: e.vx,
            vy: e.vy,
            grounded: e.grounded,
            facing_right: e.facing_right,
            jump_cooldown: e.jump_cooldown,
            wander_timer: e.wander_timer,
            egg_lay_timer: e.egg_lay_timer,
            age: e.age,
            health: e.health,
            hit_timer: e.hit_timer,
            last_player_damage_tick: e.last_player_damage_tick,
            hazards: e.hazards,
            husbandry: Default::default(),
        }
    }
}
