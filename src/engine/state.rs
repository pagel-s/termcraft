#[path = "flight.rs"]
mod flight;
#[cfg(test)]
#[path = "interaction_tests.rs"]
mod interaction_tests;
#[cfg(test)]
#[path = "surface_spawn_tests.rs"]
mod surface_spawn_tests;
#[path = "trial.rs"]
mod trial;
pub use trial::{Trial, TrialStage};
#[path = "ambient_population.rs"]
mod ambient_population;
#[path = "animal_water.rs"]
mod animal_water;
#[path = "combat.rs"]
mod combat;
#[path = "drops.rs"]
mod drops;
#[path = "ender_pearl.rs"]
mod ender_pearl;
#[path = "environment.rs"]
mod environment;
#[path = "furnace.rs"]
mod furnace;
#[path = "player_controls.rs"]
mod player_controls;
#[path = "sleep.rs"]
mod sleep;
pub use furnace::{FURNACE_FUEL_UI_SLOT, FURNACE_INPUT_UI_SLOT, FURNACE_OUTPUT_UI_SLOT};
#[path = "husbandry.rs"]
mod husbandry;
#[path = "legacy_animals.rs"]
mod legacy_animals;
#[path = "legacy_entities.rs"]
mod legacy_entities;
use super::command::ClientCommand;
use crate::entities::arrow::Arrow;
use crate::entities::blaze::Blaze;
use crate::entities::boat::Boat;
use crate::entities::chicken::Chicken;
use crate::entities::cow::Cow;
use crate::entities::creeper::Creeper;
use crate::entities::end_crystal::EndCrystal;
use crate::entities::ender_dragon::EnderDragon;
use crate::entities::ender_pearl::EnderPearl;
use crate::entities::enderman::Enderman;
use crate::entities::experience_orb::ExperienceOrb;
use crate::entities::fireball::Fireball;
use crate::entities::ghast::Ghast;
use crate::entities::item_entity::ItemEntity;
use crate::entities::ocelot::Ocelot;
use crate::entities::pig::Pig;
use crate::entities::player::Player;
use crate::entities::sheep::Sheep;
use crate::entities::silverfish::Silverfish;
use crate::entities::skeleton::Skeleton;
use crate::entities::slime::Slime;
use crate::entities::spider::Spider;
use crate::entities::squid::Squid;
use crate::entities::villager::Villager;
use crate::entities::wolf::Wolf;
use crate::entities::zombie::Zombie;
use crate::entities::zombie_pigman::ZombiePigman;
use crate::platform::fs::File;
use crate::save_path;
use crate::world::block::BlockType;
use crate::world::chunk::{CHUNK_HEIGHT, CHUNK_WIDTH};
use crate::world::item::{Inventory, ItemStack, ItemType, Recipe};
use crate::world::{
    BiomeType, Dimension, END_TOWER_XS, STRONGHOLD_CENTER_X, STRONGHOLD_PORTAL_INNER_Y, World,
    gravel_drops_flint_at, nether_wart_drop_count_at, tall_grass_drops_seed_at,
};
use rand::Rng;
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashMap};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

#[derive(PartialEq, Clone, Copy)]
pub enum CollisionType {
    Horizontal,
    VerticalUp,
    VerticalDown(f64),
}

#[derive(PartialEq, Eq, Clone, Copy, Debug, Serialize, Deserialize)]
pub enum WeatherType {
    Clear,
    Rain,
    Thunderstorm,
}

#[derive(PartialEq, Eq, Clone, Copy, Debug)]
pub enum PrecipitationType {
    None,
    Rain,
    Snow,
}

#[derive(PartialEq, Eq, Clone, Copy, Debug)]
pub enum Difficulty {
    Peaceful,
    Easy,
    Normal,
    Hard,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum PortalKind {
    Nether,
    End,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum PortalUseTarget {
    Nether((i32, i32)),
    End,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum DoorKind {
    Wood,
    Iron,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum FishingLootCategory {
    Fish,
    Junk,
    Treasure,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
struct GameRules {
    do_mob_spawning: bool,
    do_daylight_cycle: bool,
    do_weather_cycle: bool,
    keep_inventory: bool,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum GameRulesPreset {
    Vanilla,
    KeepInventory,
    Builder,
    Custom,
}

impl GameRulesPreset {
    fn rules(self) -> GameRules {
        match self {
            Self::Vanilla => GameRules {
                do_mob_spawning: true,
                do_daylight_cycle: true,
                do_weather_cycle: true,
                keep_inventory: false,
            },
            Self::KeepInventory => GameRules {
                do_mob_spawning: true,
                do_daylight_cycle: true,
                do_weather_cycle: true,
                keep_inventory: true,
            },
            Self::Builder => GameRules {
                do_mob_spawning: false,
                do_daylight_cycle: false,
                do_weather_cycle: false,
                keep_inventory: true,
            },
            // `Custom` can hold arbitrary combinations; callers should use the
            // persisted `game_rules` state directly in that case.
            Self::Custom => Self::Vanilla.rules(),
        }
    }

    fn display_name(self) -> &'static str {
        match self {
            Self::Vanilla => "Vanilla",
            Self::KeepInventory => "KeepInv",
            Self::Builder => "Builder",
            Self::Custom => "Custom",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
enum MovementProfile {
    Classic,
    Smooth,
    Agile,
}

#[derive(Clone, Copy)]
struct MovementTuning {
    walk_speed: f64,
    sneak_speed: f64,
    ground_accel: f64,
    air_accel: f64,
    ground_drag_active: f64,
    ground_drag_idle: f64,
    air_drag: f64,
}

use crate::world::furnace::{FurnaceJob, FurnaceStorage};

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct LightningBolt {
    pub x: i32,
    pub y_top: i32,
    pub y_bottom: i32,
    pub ttl: u8,
}

#[derive(Clone, Debug, PartialEq)]
pub struct RemotePlayerState {
    pub client_id: u16,
    pub armor: [Option<ItemType>; 4],
    pub action: Option<super::action::ActionVisual>,
    pub held_item: Option<ItemType>,
    pub hurt_ticks: u8,
    pub x: f64,
    pub y: f64,
    pub vx: f64,
    pub vy: f64,
    pub grounded: bool,
    pub facing_right: bool,
    pub sneaking: bool,
    pub health: f32,
    pub max_health: f32,
    pub hunger: f32,
    pub max_hunger: f32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct GroundPathSearchNode {
    x: i32,
    y: i32,
    g_cost: i32,
    f_cost: i32,
}

impl Ord for GroundPathSearchNode {
    fn cmp(&self, other: &Self) -> Ordering {
        other
            .f_cost
            .cmp(&self.f_cost)
            .then_with(|| other.g_cost.cmp(&self.g_cost))
            .then_with(|| other.x.cmp(&self.x))
            .then_with(|| other.y.cmp(&self.y))
    }
}

impl PartialOrd for GroundPathSearchNode {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

const TNT_ENTITY_DAMAGE_BASE: f32 = 12.0;
const TNT_ENTITY_KNOCKBACK_SCALE: f64 = 1.4;
const NETHER_PIGMAN_CAP: usize = 34;
const NETHER_GHAST_CAP: usize = 7;
const NETHER_BLAZE_CAP: usize = 6;
const OVERWORLD_HOSTILE_CAP: usize = 24;
const OVERWORLD_HOSTILE_SPAWN_MIN_DIST_SQ: f64 = 12.0 * 12.0;
const OVERWORLD_PASSIVE_CAP: usize = 12;
const OVERWORLD_VILLAGER_CAP: usize = 8;
const OVERWORLD_SQUID_CAP: usize = 8;
const OVERWORLD_WOLF_CAP: usize = 5;
const OVERWORLD_OCELOT_CAP: usize = 4;
const OVERWORLD_ENDERMAN_CAP: usize = 3;
const OVERWORLD_ENDERMAN_REFILL_CHANCE: f64 = 0.08;
const OVERWORLD_SILVERFISH_CAP: usize = 10;
const OVERWORLD_SLIME_CAP: usize = 6;
const OVERWORLD_DUNGEON_SPAWNER_LOCAL_CLUSTER_LIMIT: usize = 4;
const OVERWORLD_DUNGEON_SPAWNER_LOCAL_CLUSTER_RADIUS_SQ: f64 = 14.0 * 14.0;
const PLAYER_KILL_XP_CREDIT_TICKS: u64 = 100;
const DAYLIGHT_CYCLE_STEP_PER_TICK: f32 = 4.0;
const STARTUP_SPLASH_AUTO_DISMISS_TICKS: u16 = 48;
const END_ENDERMAN_CAP: usize = 22;
const END_PRE_DRAGON_ENDERMAN_CAP: usize = 14;
const OVERWORLD_HOSTILE_DESPAWN_DIST_SQ: f64 = 128.0 * 128.0;
const OVERWORLD_PASSIVE_DESPAWN_DIST_SQ: f64 = 176.0 * 176.0;
const OVERWORLD_AQUATIC_DESPAWN_DIST_SQ: f64 = 192.0 * 192.0;
const NETHER_DESPAWN_DIST_SQ: f64 = 140.0 * 140.0;
const END_DESPAWN_DIST_SQ: f64 = 180.0 * 180.0;
const NETHER_GROUND_SPAWN_MIN_DIST_SQ: f64 = 9.0 * 9.0;
const NETHER_AIR_SPAWN_MIN_DIST_SQ: f64 = 13.0 * 13.0;
const NETHER_BLAZE_LOCAL_CLUSTER_LIMIT: usize = 2;
const NETHER_BLAZE_LOCAL_CLUSTER_RADIUS_SQ: f64 = 14.0 * 14.0;
const NETHER_BLAZE_SPAWNER_HOT_ZONE_RADIUS_X: i32 = 18;
const NETHER_BLAZE_SPAWNER_HOT_ZONE_RADIUS_Y: i32 = 12;
const OVERWORLD_SPAWN_MIN_DIST_SQ: f64 = 11.0 * 11.0;
const END_ENDERMAN_SPAWN_MIN_DIST_SQ: f64 = 10.0 * 10.0;
const GHAST_FIREBALL_SOFT_CAP: usize = 10;
const BLAZE_FIREBALL_SOFT_CAP: usize = 14;
const ITEM_ENTITY_DESPAWN_TICKS: u64 = 20 * 60 * 5;
const ITEM_ENTITY_HARD_CAP: usize = 240;
const EXPERIENCE_ORB_HARD_CAP: usize = 180;
const OVERWORLD_HOSTILE_RESPAWN_BASE: u16 = 62;
const OVERWORLD_PASSIVE_RESPAWN_BASE: u16 = 116;
const OVERWORLD_VILLAGER_RESPAWN_BASE: u16 = 156;
const OVERWORLD_SQUID_RESPAWN_BASE: u16 = 84;
const OVERWORLD_WOLF_RESPAWN_BASE: u16 = 132;
const OVERWORLD_OCELOT_RESPAWN_BASE: u16 = 148;
const NETHER_RESPAWN_BASE: u16 = 48;
const END_RESPAWN_BASE: u16 = 42;
const END_CRYSTAL_HEAL_INTERVAL: u64 = 8;
const END_CRYSTAL_HEAL_AMOUNT: f32 = 1.4;
pub const END_VICTORY_SEQUENCE_TICKS: u16 = 20 * 14;
const CREDITS_SCROLL_TICKS_PER_ROW: u32 = 3;
const CREDITS_AUTO_FINISH_TICKS: u32 = 20 * 38;
const DEATH_RESPAWN_DELAY_TICKS: u16 = 14;
const RESPAWN_GRACE_TICKS: u16 = 40;
const PLAYER_INVENTORY_CAPACITY: usize = 27;
const CHEST_INVENTORY_CAPACITY: usize = 27;
const BOW_MIN_DRAW_TICKS: u8 = 4;
const BOW_MAX_DRAW_TICKS: u8 = 20;
const FISHING_WAIT_MIN_TICKS: u16 = 36;
const FISHING_WAIT_MAX_TICKS: u16 = 118;
const FISHING_BITE_WINDOW_TICKS: u8 = 16;
const FISHING_MAX_LINE_DISTANCE: f64 = 7.8;
const FISHING_BASE_FISH_WEIGHT: i32 = 88;
const FISHING_BASE_JUNK_WEIGHT: i32 = 10;
const FISHING_BASE_TREASURE_WEIGHT: i32 = 2;
const FISHING_FISH_LOOT_TABLE: [(ItemType, u32, u16); 2] =
    [(ItemType::RawFish, 1, 88), (ItemType::RawFish, 2, 12)];
const FISHING_JUNK_LOOT_TABLE: [(ItemType, u32, u16); 7] = [
    (ItemType::Stick, 1, 24),
    (ItemType::String, 1, 20),
    (ItemType::Leather, 1, 15),
    (ItemType::Bone, 1, 15),
    (ItemType::RottenFlesh, 1, 10),
    (ItemType::WaterBottle, 1, 10),
    (ItemType::LeatherBoots, 1, 6),
];
const FISHING_TREASURE_LOOT_TABLE: [(ItemType, u32, u16); 5] = [
    (ItemType::FishingRod, 1, 34),
    (ItemType::Bow, 1, 30),
    (ItemType::IronIngot, 1, 16),
    (ItemType::GoldIngot, 1, 13),
    (ItemType::Diamond, 1, 7),
];
const FURNACE_COOK_TICKS: u16 = 80;
const FURNACE_COAL_BURN_TICKS: u16 = FURNACE_COOK_TICKS * 8;
const MOB_VERTICAL_CHASE_JUMP_DY_THRESHOLD: f64 = -1.0;
const MOB_VERTICAL_CHASE_JUMP_X_RANGE: f64 = 4.8;
const MOB_VERTICAL_RECOVERY_DY_THRESHOLD: f64 = -2.0;
const MOB_VERTICAL_RECOVERY_X_RANGE: f64 = MOB_VERTICAL_CHASE_JUMP_X_RANGE * 2.0;
const MOB_REROUTE_TRIGGER_TICKS: u8 = 9;
const MOB_REROUTE_BASE_TICKS: u8 = 14;
const MOB_REROUTE_VERTICAL_TICKS: u8 = 22;
const MOB_REROUTE_REPATH_INTERVAL: u8 = 4;
const MOB_PATHFIND_SEARCH_RADIUS_X: i32 = 34;
const MOB_PATHFIND_SEARCH_RADIUS_Y: i32 = 18;
const MOB_PATHFIND_MAX_EXPANSIONS: usize = 520;
#[cfg(test)]
const SETTINGS_MENU_ROW_DIFFICULTY: u8 = 0;
#[cfg(test)]
const SETTINGS_MENU_ROW_GAMERULE_PRESET: u8 = 1;
#[cfg(test)]
const SETTINGS_MENU_ROW_CLOSE: u8 = 6;
const PLAYER_PROGRESS_VERSION: u16 = 12;
const PLAYER_PROGRESS_VERSION_V11: u16 = 11;
const PLAYER_PROGRESS_VERSION_V10: u16 = 10;
const PLAYER_PROGRESS_VERSION_V9: u16 = 9;
const PLAYER_PROGRESS_VERSION_V8: u16 = 8;
const PLAYER_PROGRESS_VERSION_V7: u16 = 7;
const PLAYER_PROGRESS_VERSION_V6: u16 = 6;
const PLAYER_PROGRESS_VERSION_V5: u16 = 5;
const PLAYER_PROGRESS_VERSION_V4: u16 = 4;
const PLAYER_PROGRESS_VERSION_V3: u16 = 3;
const PLAYER_PROGRESS_VERSION_V2: u16 = 2;
const PLAYER_PROGRESS_VERSION_V1: u16 = 1;
const PLAYER_PROGRESS_FILE: &str = "player_progression.bin";
const PLAYER_PROGRESS_TMP_SUFFIX: &str = ".tmp";
const VILLAGE_SCAN_RADIUS: i32 = 112;
const VILLAGER_MAX_PER_HUT: usize = 2;
const VILLAGER_HOME_ASSIGN_MAX_DIST: f64 = 48.0;
const VILLAGER_HOME_REASSIGN_HYSTERESIS: f64 = 5.0;
const VILLAGER_HOME_REASSIGN_FORCE_DIST: f64 = 20.0;
const VILLAGER_DOOR_HOLD_TICKS: u8 = 16;
const SWIM_CONTROL_MIN_SUBMERSION: f64 = 0.46;
const SWIM_PHYSICS_MIN_SUBMERSION: f64 = 0.36;
const PORTAL_SEARCH_RADIUS: i32 = 48;
const QUICK_TRAVEL_PORTAL_ANCHOR_RADIUS: i32 = 12;
const NETHER_PORTAL_DWELL_TICKS: u16 = 40;
const PLAYER_HOTBAR_SLOTS: usize = 9;
const ARMOR_SLOT_COUNT: usize = 4;
const ENCHANT_MAX_LEVEL: u8 = 3;
const ENCHANT_OPTION_COUNT: usize = 3;
const ENCHANT_LEVEL_COSTS: [u32; ENCHANT_OPTION_COUNT] = [1, 2, 3];
const ANVIL_COMBINE_LEVEL_COST: u32 = 2;
const BREW_OPTION_COUNT: usize = 5;
const POTION_STRENGTH_DURATION_TICKS: u16 = 20 * 180;
const POTION_REGEN_DURATION_TICKS: u16 = 20 * 45;
const POTION_FIRE_RESIST_DURATION_TICKS: u16 = 20 * 180;
const POTION_REGEN_HEAL_INTERVAL_TICKS: u16 = 25;
const POTION_STRENGTH_MELEE_BONUS: f32 = 3.0;
const POTION_HEALING_INSTANT_HP: f32 = 8.0;
pub const ARMOR_UI_OFFSET: usize = PLAYER_INVENTORY_CAPACITY;
pub const ARMOR_UI_SLOTS: usize = ARMOR_SLOT_COUNT;
pub const CRAFT_GRID_UI_OFFSET: usize = ARMOR_UI_OFFSET + ARMOR_UI_SLOTS;
pub const CRAFT_GRID_UI_SLOTS: usize = 9;
pub const CRAFT_OUTPUT_UI_SLOT: usize = CRAFT_GRID_UI_OFFSET + CRAFT_GRID_UI_SLOTS;
const SPRINT_DOUBLE_TAP_WINDOW_TICKS: u8 = 8;
const SPRINT_SPEED_MULTIPLIER: f64 = 1.3;
const SPRINT_MIN_HUNGER: f32 = 6.0;
const WALK_HUNGER_DRAIN_PER_TICK: f32 = 0.00075;
const SPRINT_HUNGER_DRAIN_PER_TICK: f32 = 0.0025;
const PLAYER_HALF_WIDTH: f64 = 0.25;
const PLAYER_HEIGHT: f64 = 1.8;
const BOAT_HALF_WIDTH: f64 = 0.48;
const BOAT_HEIGHT: f64 = 0.45;
const BOAT_WATER_SPEED: f64 = 0.34;
const BOAT_ACCEL: f64 = 0.28;
const BOAT_WATER_DRAG: f64 = 0.82;

#[derive(Clone, Debug, Serialize, Deserialize)]
struct PlayerProgressSaveData {
    // Kept out of the legacy wire layout so versions 1–6 remain readable.
    #[serde(skip)]
    complete_session: Option<PlayerSession>,
    #[serde(skip)]
    runtime: Option<RuntimeCheckpoint>,
    #[serde(skip)]
    portal_links: Vec<PortalLink>,
    version: u16,
    player_x: f64,
    player_y: f64,
    player_vx: f64,
    player_vy: f64,
    player_grounded: bool,
    player_facing_right: bool,
    player_sneaking: bool,
    player_health: f32,
    player_hunger: f32,
    player_drowning_timer: i32,
    player_burning_timer: i32,
    player_fall_distance: f32,
    inventory: Inventory,
    armor_slots: [Option<ItemStack>; ARMOR_SLOT_COUNT],
    hotbar_index: u8,
    spawn_point_x: i32,
    spawn_point_y: i32,
    has_spawn_point: bool,
    current_dimension_code: u8,
    time_of_day: f32,
    weather_code: u8,
    weather_timer: u32,
    weather_rain_intensity: f32,
    weather_wind_intensity: f32,
    weather_thunder_intensity: f32,
    thunder_flash_timer: u8,
    dragon_defeated: bool,
    completion_credits_seen: bool,
    movement_profile_code: u8,
    portal_cooldown: u16,
    experience_level: u32,
    experience_progress: f32,
    experience_total: u32,
    difficulty_code: u8,
    game_rules_preset_code: u8,
    rule_do_mob_spawning: bool,
    rule_do_daylight_cycle: bool,
    rule_do_weather_cycle: bool,
    rule_keep_inventory: bool,
}

#[derive(Serialize, Deserialize)]
struct PlayerCheckpointV7 {
    version: u16,
    progress: PlayerProgressSaveData,
    session: actors::PlayerSessionV10,
}

type PortalLink = ((u8, i32, i32), (i32, i32));

#[derive(Serialize, Deserialize)]
struct PlayerCheckpointV8 {
    version: u16,
    progress: PlayerProgressSaveData,
    session: actors::PlayerSessionV10,
    portal_links: Vec<PortalLink>,
}

#[derive(Deserialize)]
struct PlayerCheckpointV9 {
    version: u16,
    progress: PlayerProgressSaveData,
    session: actors::PlayerSessionV10,
    portal_links: Vec<PortalLink>,
    runtime: dimensions::RuntimeCheckpointV9,
}

#[derive(Deserialize)]
struct PlayerCheckpointV10 {
    version: u16,
    progress: PlayerProgressSaveData,
    session: actors::PlayerSessionV10,
    portal_links: Vec<PortalLink>,
    runtime: dimensions::RuntimeCheckpointV10,
}

#[derive(Serialize, Deserialize)]
struct PlayerCheckpointV11 {
    version: u16,
    progress: PlayerProgressSaveData,
    session: PlayerSession,
    portal_links: Vec<PortalLink>,
    runtime: dimensions::RuntimeCheckpointV11,
}

#[derive(Serialize, Deserialize)]
struct PlayerCheckpointV12 {
    version: u16,
    progress: PlayerProgressSaveData,
    session: PlayerSession,
    portal_links: Vec<PortalLink>,
    runtime: RuntimeCheckpoint,
}

#[derive(Serialize)]
struct PlayerCheckpointRef<'a> {
    version: u16,
    progress: &'a PlayerProgressSaveData,
    session: &'a PlayerSession,
    portal_links: &'a [PortalLink],
    runtime: &'a RuntimeCheckpoint,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct PlayerProgressSaveDataV5 {
    version: u16,
    player_x: f64,
    player_y: f64,
    player_vx: f64,
    player_vy: f64,
    player_grounded: bool,
    player_facing_right: bool,
    player_sneaking: bool,
    player_health: f32,
    player_hunger: f32,
    player_drowning_timer: i32,
    player_burning_timer: i32,
    player_fall_distance: f32,
    inventory: Inventory,
    armor_slots: [Option<ItemStack>; ARMOR_SLOT_COUNT],
    hotbar_index: u8,
    spawn_point_x: i32,
    spawn_point_y: i32,
    has_spawn_point: bool,
    current_dimension_code: u8,
    time_of_day: f32,
    weather_code: u8,
    weather_timer: u32,
    weather_rain_intensity: f32,
    weather_wind_intensity: f32,
    weather_thunder_intensity: f32,
    thunder_flash_timer: u8,
    dragon_defeated: bool,
    completion_credits_seen: bool,
    movement_profile_code: u8,
    portal_cooldown: u16,
    difficulty_code: u8,
    game_rules_preset_code: u8,
    rule_do_mob_spawning: bool,
    rule_do_daylight_cycle: bool,
    rule_do_weather_cycle: bool,
    rule_keep_inventory: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct PlayerProgressSaveDataV4 {
    version: u16,
    player_x: f64,
    player_y: f64,
    player_vx: f64,
    player_vy: f64,
    player_grounded: bool,
    player_facing_right: bool,
    player_sneaking: bool,
    player_health: f32,
    player_hunger: f32,
    player_drowning_timer: i32,
    player_burning_timer: i32,
    player_fall_distance: f32,
    inventory: Inventory,
    hotbar_index: u8,
    spawn_point_x: i32,
    spawn_point_y: i32,
    has_spawn_point: bool,
    current_dimension_code: u8,
    time_of_day: f32,
    weather_code: u8,
    weather_timer: u32,
    weather_rain_intensity: f32,
    weather_wind_intensity: f32,
    weather_thunder_intensity: f32,
    thunder_flash_timer: u8,
    dragon_defeated: bool,
    completion_credits_seen: bool,
    movement_profile_code: u8,
    portal_cooldown: u16,
    difficulty_code: u8,
    game_rules_preset_code: u8,
    rule_do_mob_spawning: bool,
    rule_do_daylight_cycle: bool,
    rule_do_weather_cycle: bool,
    rule_keep_inventory: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct PlayerProgressSaveDataV3 {
    version: u16,
    player_x: f64,
    player_y: f64,
    player_vx: f64,
    player_vy: f64,
    player_grounded: bool,
    player_facing_right: bool,
    player_sneaking: bool,
    player_health: f32,
    player_hunger: f32,
    player_drowning_timer: i32,
    player_burning_timer: i32,
    player_fall_distance: f32,
    inventory: Inventory,
    hotbar_index: u8,
    current_dimension_code: u8,
    time_of_day: f32,
    weather_code: u8,
    weather_timer: u32,
    weather_rain_intensity: f32,
    weather_wind_intensity: f32,
    weather_thunder_intensity: f32,
    thunder_flash_timer: u8,
    dragon_defeated: bool,
    completion_credits_seen: bool,
    movement_profile_code: u8,
    portal_cooldown: u16,
    difficulty_code: u8,
    game_rules_preset_code: u8,
    rule_do_mob_spawning: bool,
    rule_do_daylight_cycle: bool,
    rule_do_weather_cycle: bool,
    rule_keep_inventory: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct PlayerProgressSaveDataV2 {
    version: u16,
    player_x: f64,
    player_y: f64,
    player_vx: f64,
    player_vy: f64,
    player_grounded: bool,
    player_facing_right: bool,
    player_sneaking: bool,
    player_health: f32,
    player_hunger: f32,
    player_drowning_timer: i32,
    player_burning_timer: i32,
    player_fall_distance: f32,
    inventory: Inventory,
    hotbar_index: u8,
    current_dimension_code: u8,
    time_of_day: f32,
    weather_code: u8,
    weather_timer: u32,
    weather_rain_intensity: f32,
    weather_wind_intensity: f32,
    weather_thunder_intensity: f32,
    thunder_flash_timer: u8,
    dragon_defeated: bool,
    completion_credits_seen: bool,
    movement_profile_code: u8,
    portal_cooldown: u16,
    difficulty_code: u8,
    game_rules_preset_code: u8,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct PlayerProgressSaveDataV1 {
    version: u16,
    player_x: f64,
    player_y: f64,
    player_vx: f64,
    player_vy: f64,
    player_grounded: bool,
    player_facing_right: bool,
    player_sneaking: bool,
    player_health: f32,
    player_hunger: f32,
    player_drowning_timer: i32,
    player_burning_timer: i32,
    player_fall_distance: f32,
    inventory: Inventory,
    hotbar_index: u8,
    current_dimension_code: u8,
    time_of_day: f32,
    weather_code: u8,
    weather_timer: u32,
    weather_rain_intensity: f32,
    weather_wind_intensity: f32,
    weather_thunder_intensity: f32,
    thunder_flash_timer: u8,
    dragon_defeated: bool,
    completion_credits_seen: bool,
    movement_profile_code: u8,
    portal_cooldown: u16,
}

impl From<PlayerProgressSaveDataV1> for PlayerProgressSaveData {
    fn from(v1: PlayerProgressSaveDataV1) -> Self {
        Self {
            complete_session: None,
            runtime: None,
            portal_links: Vec::new(),
            version: PLAYER_PROGRESS_VERSION,
            player_x: v1.player_x,
            player_y: v1.player_y,
            player_vx: v1.player_vx,
            player_vy: v1.player_vy,
            player_grounded: v1.player_grounded,
            player_facing_right: v1.player_facing_right,
            player_sneaking: v1.player_sneaking,
            player_health: v1.player_health,
            player_hunger: v1.player_hunger,
            player_drowning_timer: v1.player_drowning_timer,
            player_burning_timer: v1.player_burning_timer,
            player_fall_distance: v1.player_fall_distance,
            inventory: v1.inventory,
            armor_slots: [None, None, None, None],
            hotbar_index: v1.hotbar_index,
            spawn_point_x: 0,
            spawn_point_y: 0,
            has_spawn_point: false,
            current_dimension_code: v1.current_dimension_code,
            time_of_day: v1.time_of_day,
            weather_code: v1.weather_code,
            weather_timer: v1.weather_timer,
            weather_rain_intensity: v1.weather_rain_intensity,
            weather_wind_intensity: v1.weather_wind_intensity,
            weather_thunder_intensity: v1.weather_thunder_intensity,
            thunder_flash_timer: v1.thunder_flash_timer,
            dragon_defeated: v1.dragon_defeated,
            completion_credits_seen: v1.completion_credits_seen,
            movement_profile_code: v1.movement_profile_code,
            portal_cooldown: v1.portal_cooldown,
            experience_level: 0,
            experience_progress: 0.0,
            experience_total: 0,
            difficulty_code: 2,        // Normal
            game_rules_preset_code: 0, // Vanilla
            rule_do_mob_spawning: true,
            rule_do_daylight_cycle: true,
            rule_do_weather_cycle: true,
            rule_keep_inventory: false,
        }
    }
}

impl From<PlayerProgressSaveDataV5> for PlayerProgressSaveData {
    fn from(v5: PlayerProgressSaveDataV5) -> Self {
        Self {
            complete_session: None,
            runtime: None,
            portal_links: Vec::new(),
            version: PLAYER_PROGRESS_VERSION,
            player_x: v5.player_x,
            player_y: v5.player_y,
            player_vx: v5.player_vx,
            player_vy: v5.player_vy,
            player_grounded: v5.player_grounded,
            player_facing_right: v5.player_facing_right,
            player_sneaking: v5.player_sneaking,
            player_health: v5.player_health,
            player_hunger: v5.player_hunger,
            player_drowning_timer: v5.player_drowning_timer,
            player_burning_timer: v5.player_burning_timer,
            player_fall_distance: v5.player_fall_distance,
            inventory: v5.inventory,
            armor_slots: v5.armor_slots,
            hotbar_index: v5.hotbar_index,
            spawn_point_x: v5.spawn_point_x,
            spawn_point_y: v5.spawn_point_y,
            has_spawn_point: v5.has_spawn_point,
            current_dimension_code: v5.current_dimension_code,
            time_of_day: v5.time_of_day,
            weather_code: v5.weather_code,
            weather_timer: v5.weather_timer,
            weather_rain_intensity: v5.weather_rain_intensity,
            weather_wind_intensity: v5.weather_wind_intensity,
            weather_thunder_intensity: v5.weather_thunder_intensity,
            thunder_flash_timer: v5.thunder_flash_timer,
            dragon_defeated: v5.dragon_defeated,
            completion_credits_seen: v5.completion_credits_seen,
            movement_profile_code: v5.movement_profile_code,
            portal_cooldown: v5.portal_cooldown,
            experience_level: 0,
            experience_progress: 0.0,
            experience_total: 0,
            difficulty_code: v5.difficulty_code,
            game_rules_preset_code: v5.game_rules_preset_code,
            rule_do_mob_spawning: v5.rule_do_mob_spawning,
            rule_do_daylight_cycle: v5.rule_do_daylight_cycle,
            rule_do_weather_cycle: v5.rule_do_weather_cycle,
            rule_keep_inventory: v5.rule_keep_inventory,
        }
    }
}

impl From<PlayerProgressSaveDataV4> for PlayerProgressSaveData {
    fn from(v4: PlayerProgressSaveDataV4) -> Self {
        Self {
            complete_session: None,
            runtime: None,
            portal_links: Vec::new(),
            version: PLAYER_PROGRESS_VERSION,
            player_x: v4.player_x,
            player_y: v4.player_y,
            player_vx: v4.player_vx,
            player_vy: v4.player_vy,
            player_grounded: v4.player_grounded,
            player_facing_right: v4.player_facing_right,
            player_sneaking: v4.player_sneaking,
            player_health: v4.player_health,
            player_hunger: v4.player_hunger,
            player_drowning_timer: v4.player_drowning_timer,
            player_burning_timer: v4.player_burning_timer,
            player_fall_distance: v4.player_fall_distance,
            inventory: v4.inventory,
            armor_slots: [None, None, None, None],
            hotbar_index: v4.hotbar_index,
            spawn_point_x: v4.spawn_point_x,
            spawn_point_y: v4.spawn_point_y,
            has_spawn_point: v4.has_spawn_point,
            current_dimension_code: v4.current_dimension_code,
            time_of_day: v4.time_of_day,
            weather_code: v4.weather_code,
            weather_timer: v4.weather_timer,
            weather_rain_intensity: v4.weather_rain_intensity,
            weather_wind_intensity: v4.weather_wind_intensity,
            weather_thunder_intensity: v4.weather_thunder_intensity,
            thunder_flash_timer: v4.thunder_flash_timer,
            dragon_defeated: v4.dragon_defeated,
            completion_credits_seen: v4.completion_credits_seen,
            movement_profile_code: v4.movement_profile_code,
            portal_cooldown: v4.portal_cooldown,
            experience_level: 0,
            experience_progress: 0.0,
            experience_total: 0,
            difficulty_code: v4.difficulty_code,
            game_rules_preset_code: v4.game_rules_preset_code,
            rule_do_mob_spawning: v4.rule_do_mob_spawning,
            rule_do_daylight_cycle: v4.rule_do_daylight_cycle,
            rule_do_weather_cycle: v4.rule_do_weather_cycle,
            rule_keep_inventory: v4.rule_keep_inventory,
        }
    }
}

impl From<PlayerProgressSaveDataV3> for PlayerProgressSaveData {
    fn from(v3: PlayerProgressSaveDataV3) -> Self {
        Self {
            complete_session: None,
            runtime: None,
            portal_links: Vec::new(),
            version: PLAYER_PROGRESS_VERSION,
            player_x: v3.player_x,
            player_y: v3.player_y,
            player_vx: v3.player_vx,
            player_vy: v3.player_vy,
            player_grounded: v3.player_grounded,
            player_facing_right: v3.player_facing_right,
            player_sneaking: v3.player_sneaking,
            player_health: v3.player_health,
            player_hunger: v3.player_hunger,
            player_drowning_timer: v3.player_drowning_timer,
            player_burning_timer: v3.player_burning_timer,
            player_fall_distance: v3.player_fall_distance,
            inventory: v3.inventory,
            armor_slots: [None, None, None, None],
            hotbar_index: v3.hotbar_index,
            spawn_point_x: 0,
            spawn_point_y: 0,
            has_spawn_point: false,
            current_dimension_code: v3.current_dimension_code,
            time_of_day: v3.time_of_day,
            weather_code: v3.weather_code,
            weather_timer: v3.weather_timer,
            weather_rain_intensity: v3.weather_rain_intensity,
            weather_wind_intensity: v3.weather_wind_intensity,
            weather_thunder_intensity: v3.weather_thunder_intensity,
            thunder_flash_timer: v3.thunder_flash_timer,
            dragon_defeated: v3.dragon_defeated,
            completion_credits_seen: v3.completion_credits_seen,
            movement_profile_code: v3.movement_profile_code,
            portal_cooldown: v3.portal_cooldown,
            experience_level: 0,
            experience_progress: 0.0,
            experience_total: 0,
            difficulty_code: v3.difficulty_code,
            game_rules_preset_code: v3.game_rules_preset_code,
            rule_do_mob_spawning: v3.rule_do_mob_spawning,
            rule_do_daylight_cycle: v3.rule_do_daylight_cycle,
            rule_do_weather_cycle: v3.rule_do_weather_cycle,
            rule_keep_inventory: v3.rule_keep_inventory,
        }
    }
}

impl From<PlayerProgressSaveDataV2> for PlayerProgressSaveData {
    fn from(v2: PlayerProgressSaveDataV2) -> Self {
        let default_rules = match v2.game_rules_preset_code {
            0 => GameRulesPreset::Vanilla.rules(),
            1 => GameRulesPreset::KeepInventory.rules(),
            2 => GameRulesPreset::Builder.rules(),
            _ => GameRulesPreset::Vanilla.rules(),
        };
        Self {
            complete_session: None,
            runtime: None,
            portal_links: Vec::new(),
            version: PLAYER_PROGRESS_VERSION,
            player_x: v2.player_x,
            player_y: v2.player_y,
            player_vx: v2.player_vx,
            player_vy: v2.player_vy,
            player_grounded: v2.player_grounded,
            player_facing_right: v2.player_facing_right,
            player_sneaking: v2.player_sneaking,
            player_health: v2.player_health,
            player_hunger: v2.player_hunger,
            player_drowning_timer: v2.player_drowning_timer,
            player_burning_timer: v2.player_burning_timer,
            player_fall_distance: v2.player_fall_distance,
            inventory: v2.inventory,
            armor_slots: [None, None, None, None],
            hotbar_index: v2.hotbar_index,
            spawn_point_x: 0,
            spawn_point_y: 0,
            has_spawn_point: false,
            current_dimension_code: v2.current_dimension_code,
            time_of_day: v2.time_of_day,
            weather_code: v2.weather_code,
            weather_timer: v2.weather_timer,
            weather_rain_intensity: v2.weather_rain_intensity,
            weather_wind_intensity: v2.weather_wind_intensity,
            weather_thunder_intensity: v2.weather_thunder_intensity,
            thunder_flash_timer: v2.thunder_flash_timer,
            dragon_defeated: v2.dragon_defeated,
            completion_credits_seen: v2.completion_credits_seen,
            movement_profile_code: v2.movement_profile_code,
            portal_cooldown: v2.portal_cooldown,
            experience_level: 0,
            experience_progress: 0.0,
            experience_total: 0,
            difficulty_code: v2.difficulty_code,
            game_rules_preset_code: v2.game_rules_preset_code,
            rule_do_mob_spawning: default_rules.do_mob_spawning,
            rule_do_daylight_cycle: default_rules.do_daylight_cycle,
            rule_do_weather_cycle: default_rules.do_weather_cycle,
            rule_keep_inventory: default_rules.keep_inventory,
        }
    }
}

#[path = "actors.rs"]
mod actors;
#[path = "dimensions.rs"]
mod dimensions;
pub use actors::{PlayerSession, SharedSettings};
pub(crate) use dimensions::{MAX_CHECKPOINT_BYTES, RuntimeCheckpoint, SharedProgress};

pub struct GameState {
    pub quit_confirmation: bool,
    sleeping_beds: std::collections::BTreeMap<u16, (i32, i32, f32)>,
    last_pearl_throw_age: std::collections::BTreeMap<u16, u64>,
    pub(crate) player_notices: Vec<(u16, String)>,
    pub pvp_enabled: bool,
    pub host_client_id: Option<u16>,
    pub connection_address: Option<String>,
    pub(crate) shortcut_players: std::collections::BTreeMap<u16, bool>,
    solo_shortcuts: Option<bool>,
    pub(super) action_visuals: std::collections::BTreeMap<u16, super::action::ActionRecord>,
    active_peer: Option<u16>,
    cooperative_players: std::collections::BTreeMap<u16, PlayerSession>,
    cooperative_dimensions: std::collections::BTreeMap<u16, Dimension>,
    cooperative_worlds: std::collections::BTreeMap<Dimension, dimensions::DimensionRuntime>,
    pub(crate) runtime_revision: std::cell::Cell<u64>,
    pub(crate) director_encounter: Option<u64>,
    pub chat: super::director::ChatView,
    pub performance: super::metrics::PerformanceMetrics,
    pub show_debug_overlay: bool,
    pub save_error: std::cell::RefCell<Option<String>>,
    pub player: Player,
    pub remote_players: Vec<RemotePlayerState>,
    pub local_player_id: Option<u16>,
    pub inventory: Inventory,
    armor_slots: [Option<ItemStack>; ARMOR_SLOT_COUNT],
    crafting_grid: [Option<ItemStack>; CRAFT_GRID_UI_SLOTS],
    inventory_enchant_levels: [u8; PLAYER_INVENTORY_CAPACITY],
    armor_enchant_levels: [u8; ARMOR_SLOT_COUNT],
    pub trial: Option<Trial>,
    pub zombies: Vec<Zombie>,
    pub creepers: Vec<Creeper>,
    pub skeletons: Vec<Skeleton>,
    pub spiders: Vec<Spider>,
    pub silverfish: Vec<Silverfish>,
    pub slimes: Vec<Slime>,
    pub endermen: Vec<Enderman>,
    pub blazes: Vec<Blaze>,
    pub pigmen: Vec<ZombiePigman>,
    pub ghasts: Vec<Ghast>,
    pub cows: Vec<Cow>,
    pub sheep: Vec<Sheep>,
    pub pigs: Vec<Pig>,
    pub chickens: Vec<Chicken>,
    pub squids: Vec<Squid>,
    pub wolves: Vec<Wolf>,
    pub ocelots: Vec<Ocelot>,
    pub villagers: Vec<Villager>,
    pub boats: Vec<Boat>,
    pub item_entities: Vec<ItemEntity>,
    pub experience_orbs: Vec<ExperienceOrb>,
    pub arrows: Vec<Arrow>,
    pub ender_pearls: Vec<EnderPearl>,
    pub fireballs: Vec<Fireball>,
    pub end_crystals: Vec<EndCrystal>,
    pub ender_dragon: Option<EnderDragon>,
    pub lightning_bolts: Vec<LightningBolt>,
    pub world: World,
    pub hotbar_index: u8,
    pub mouse_x: u16,
    pub mouse_y: u16,
    pub left_click_down: bool,
    pub time_of_day: f32,
    pub weather: WeatherType,
    pub thunder_flash_timer: u8,
    pub weather_rain_intensity: f32,
    pub weather_wind_intensity: f32,
    pub weather_thunder_intensity: f32,
    world_tick: u64,
    pub eye_guidance_timer: u16,
    pub eye_guidance_dir: i8,
    pub eye_guidance_distance: i32,
    pub moving_left: bool,
    pub moving_right: bool,
    pub inventory_open: bool,
    pub at_crafting_table: bool,
    pub at_furnace: bool,
    pub at_chest: bool,
    pub at_enchanting_table: bool,
    pub at_anvil: bool,
    pub at_brewing_stand: bool,
    pub selected_inventory_slot: Option<usize>,
    pub current_dimension: Dimension,
    open_chest_pos: Option<(i32, i32)>,
    spawn_point: Option<(i32, i32)>,
    weather_timer: u32,
    portal_timer: u16,
    portal_cooldown: u16,
    portal_links: HashMap<(u8, i32, i32), (i32, i32)>,
    dungeon_spawner_timer: u16,
    silverfish_spawner_timer: u16,
    blaze_spawner_timer: u16,
    overworld_hostile_spawn_timer: u16,
    overworld_passive_spawn_timer: u16,
    overworld_villager_spawn_timer: u16,
    overworld_squid_spawn_timer: u16,
    overworld_wolf_spawn_timer: u16,
    overworld_ocelot_spawn_timer: u16,
    villager_open_doors: HashMap<(i32, i32), u8>,
    nether_spawn_timer: u16,
    end_spawn_timer: u16,
    end_boss_initialized: bool,
    dragon_defeated: bool,
    end_victory_ticks: u16,
    end_victory_origin: Option<(f64, f64)>,
    movement_profile: MovementProfile,
    difficulty: Difficulty,
    game_rules_preset: GameRulesPreset,
    game_rules: GameRules,
    sneak_toggled: bool,
    sneak_held: bool,
    jump_held: bool,
    sprinting: bool,
    sprint_direction: i8,
    sprint_left_tap_ticks: u8,
    sprint_right_tap_ticks: u8,
    jump_buffer_ticks: u8,
    coyote_ticks: u8,
    completion_credits_seen: bool,
    credits_active: bool,
    credits_tick: u32,
    death_screen_active: bool,
    death_screen_ticks: u16,
    startup_splash_active: bool,
    startup_splash_ticks: u16,
    settings_menu_open: bool,
    settings_menu_selected: u8,
    respawn_grace_ticks: u16,
    player_combat_hurt_cooldown: u8,
    bow_draw_ticks: u8,
    bow_draw_active: bool,
    fishing_bobber_active: bool,
    fishing_bobber_x: f64,
    fishing_bobber_y: f64,
    fishing_wait_ticks: u16,
    fishing_bite_window_ticks: u8,
    mounted_boat: Option<usize>,
    furnace_slots: [Option<ItemStack>; 3],
    furnace_explicit: bool,
    furnace_job: Option<FurnaceJob>,
    furnace_progress_ticks: u16,
    furnace_burn_ticks: u16,
    potion_strength_timer: u16,
    potion_regeneration_timer: u16,
    potion_fire_resistance_timer: u16,
}

impl Default for GameState {
    fn default() -> Self {
        Self::new()
    }
}

impl GameState {
    fn encode_dimension(dimension: Dimension) -> u8 {
        match dimension {
            Dimension::Overworld => 0,
            Dimension::Nether => 1,
            Dimension::End => 2,
        }
    }

    fn decode_dimension(code: u8) -> Option<Dimension> {
        match code {
            0 => Some(Dimension::Overworld),
            1 => Some(Dimension::Nether),
            2 => Some(Dimension::End),
            _ => None,
        }
    }

    fn encode_weather(weather: WeatherType) -> u8 {
        match weather {
            WeatherType::Clear => 0,
            WeatherType::Rain => 1,
            WeatherType::Thunderstorm => 2,
        }
    }

    fn decode_weather(code: u8) -> Option<WeatherType> {
        match code {
            0 => Some(WeatherType::Clear),
            1 => Some(WeatherType::Rain),
            2 => Some(WeatherType::Thunderstorm),
            _ => None,
        }
    }

    fn encode_movement_profile(profile: MovementProfile) -> u8 {
        match profile {
            MovementProfile::Classic => 0,
            MovementProfile::Smooth => 1,
            MovementProfile::Agile => 2,
        }
    }

    fn decode_movement_profile(code: u8) -> Option<MovementProfile> {
        match code {
            0 => Some(MovementProfile::Classic),
            1 => Some(MovementProfile::Smooth),
            2 => Some(MovementProfile::Agile),
            _ => None,
        }
    }

    fn encode_difficulty(difficulty: Difficulty) -> u8 {
        match difficulty {
            Difficulty::Peaceful => 0,
            Difficulty::Easy => 1,
            Difficulty::Normal => 2,
            Difficulty::Hard => 3,
        }
    }

    fn decode_difficulty(code: u8) -> Option<Difficulty> {
        match code {
            0 => Some(Difficulty::Peaceful),
            1 => Some(Difficulty::Easy),
            2 => Some(Difficulty::Normal),
            3 => Some(Difficulty::Hard),
            _ => None,
        }
    }

    fn encode_game_rules_preset(preset: GameRulesPreset) -> u8 {
        match preset {
            GameRulesPreset::Vanilla => 0,
            GameRulesPreset::KeepInventory => 1,
            GameRulesPreset::Builder => 2,
            GameRulesPreset::Custom => 3,
        }
    }

    fn decode_game_rules_preset(code: u8) -> Option<GameRulesPreset> {
        match code {
            0 => Some(GameRulesPreset::Vanilla),
            1 => Some(GameRulesPreset::KeepInventory),
            2 => Some(GameRulesPreset::Builder),
            3 => Some(GameRulesPreset::Custom),
            _ => None,
        }
    }

    fn progression_spawn_surface_y(world: &World, x: i32) -> Option<i32> {
        for y in 2..(CHUNK_HEIGHT as i32 - 2) {
            let ground = world.get_block(x, y);
            if matches!(
                ground,
                BlockType::Cactus
                    | BlockType::Leaves
                    | BlockType::BirchLeaves
                    | BlockType::Wood
                    | BlockType::BirchWood
            ) {
                continue;
            }
            if ground.is_solid()
                && !ground.is_fluid()
                && world.get_block(x, y - 1) == BlockType::Air
                && world.get_block(x, y - 2) == BlockType::Air
            {
                return Some(y);
            }
        }
        None
    }

    fn progression_is_exposed_to_sky(world: &World, x: i32, top_y: i32) -> bool {
        let top_y = top_y.clamp(0, CHUNK_HEIGHT as i32 - 1);
        for y in 0..=top_y {
            if world.get_block(x, y).is_solid() {
                return false;
            }
        }
        true
    }

    fn is_safe_spawn_column_base(world: &World, x: i32, y: i32, require_sky: bool) -> bool {
        if y < 2 || y >= CHUNK_HEIGHT as i32 - 1 {
            return false;
        }
        let ground = world.get_block(x, y);
        if !ground.is_solid()
            || ground.is_fluid()
            || matches!(
                ground,
                BlockType::Cactus
                    | BlockType::Water(_)
                    | BlockType::Lava(_)
                    | BlockType::Leaves
                    | BlockType::BirchLeaves
                    | BlockType::Wood
                    | BlockType::BirchWood
            )
        {
            return false;
        }
        if world.get_block(x, y - 1) != BlockType::Air
            || world.get_block(x, y - 2) != BlockType::Air
        {
            return false;
        }
        // Require at least one side to be open enough to move out after spawning.
        let left_open =
            !(world.get_block(x - 1, y - 1).is_solid() && world.get_block(x - 1, y - 2).is_solid());
        let right_open =
            !(world.get_block(x + 1, y - 1).is_solid() && world.get_block(x + 1, y - 2).is_solid());
        if !left_open && !right_open {
            return false;
        }
        if require_sky && !Self::progression_is_exposed_to_sky(world, x, y - 2) {
            return false;
        }
        true
    }

    fn spawn_has_stable_runway(world: &World, x: i32, y: i32, require_sky: bool) -> bool {
        for dir in [-1, 1] {
            let mut prev_y = y;
            let mut stable_steps = 0;
            for step in 1..=2 {
                let nx = x + dir * step;
                let Some(ny) = Self::progression_spawn_surface_y(world, nx) else {
                    break;
                };
                if !Self::is_safe_spawn_column_base(world, nx, ny, require_sky) {
                    break;
                }
                if (ny - prev_y).abs() > 1 {
                    break;
                }
                stable_steps += 1;
                prev_y = ny;
            }
            if stable_steps >= 2 {
                return true;
            }
        }
        false
    }

    fn is_safe_spawn_column(world: &World, x: i32, y: i32, require_sky: bool) -> bool {
        Self::is_safe_spawn_column_base(world, x, y, require_sky)
            && Self::spawn_has_stable_runway(world, x, y, require_sky)
    }

    fn progression_spawn_quality_penalty(world: &World, x: i32, y: i32) -> i32 {
        let ground = world.get_block(x, y);
        let mut penalty = match ground {
            BlockType::Grass => 0,
            BlockType::Dirt => 1,
            BlockType::Stone | BlockType::Cobblestone => 4,
            BlockType::Sand | BlockType::Gravel => 8,
            BlockType::Snow => 8,
            BlockType::Ice => 12,
            _ => 5,
        };

        penalty += match world.get_biome(x) {
            BiomeType::Plains => 0,
            BiomeType::Forest => 1,
            BiomeType::Taiga => 3,
            BiomeType::Jungle => 4,
            BiomeType::Tundra => 5,
            BiomeType::Desert => 6,
            BiomeType::Swamp => 7,
            BiomeType::ExtremeHills => 8,
            BiomeType::Ocean | BiomeType::River => 12,
        };

        for offset in 1..=2 {
            for nx in [x - offset, x + offset] {
                let Some(ny) = Self::progression_spawn_surface_y(world, nx) else {
                    penalty += 12;
                    continue;
                };
                penalty += (ny - y).abs() * (offset + 1);
                if !Self::is_safe_spawn_column_base(world, nx, ny, true) {
                    penalty += 6;
                }
                if ny > y + 1 {
                    penalty += (ny - y - 1) * 4;
                } else if ny < y - 1 {
                    penalty += (y - ny - 1) * 2;
                }
                if ny > 0 && world.get_block(nx, ny - 1).is_fluid() {
                    penalty += 10;
                }
            }
        }

        let canopy_probe_y = (y - 3).clamp(0, CHUNK_HEIGHT as i32 - 1);
        if matches!(
            world.get_block(x, canopy_probe_y),
            BlockType::Leaves | BlockType::BirchLeaves | BlockType::Wood | BlockType::BirchWood
        ) {
            penalty += 6;
        }

        penalty
    }

    fn find_nearest_safe_spawn(
        world: &World,
        center_x: i32,
        search_radius: i32,
        require_sky: bool,
    ) -> Option<(i32, i32)> {
        let mut best: Option<(i32, i32, i32)> = None; // (score, x, y)
        for dx in -search_radius..=search_radius {
            let x = center_x + dx;
            let Some(y) = Self::progression_spawn_surface_y(world, x) else {
                continue;
            };
            if !Self::is_safe_spawn_column(world, x, y, require_sky) {
                continue;
            }
            let score = dx.abs() * 4 + (y - 33).abs();
            match best {
                None => best = Some((score, x, y)),
                Some((best_score, _, best_y))
                    if score < best_score || (score == best_score && y < best_y) =>
                {
                    best = Some((score, x, y));
                }
                _ => {}
            }
        }
        best.map(|(_, x, y)| (x, y))
    }

    fn find_best_progression_spawn_in_radius(
        world: &World,
        center_x: i32,
        search_radius: i32,
    ) -> Option<(i32, i32)> {
        let mut best: Option<(i32, i32, i32)> = None; // (score, x, y)
        for dx in -search_radius..=search_radius {
            let x = center_x + dx;
            let Some(y) = Self::progression_spawn_surface_y(world, x) else {
                continue;
            };
            if !Self::is_safe_spawn_column(world, x, y, true) {
                continue;
            }
            let left_y = Self::progression_spawn_surface_y(world, x - 1).unwrap_or(y);
            let right_y = Self::progression_spawn_surface_y(world, x + 1).unwrap_or(y);

            let mut score = dx.abs();
            score += (y - 33).abs() * 3;
            if y > 40 {
                score += (y - 40) * 8;
            }
            // Penalize pit-like spawn columns where neighbors are significantly higher.
            score += (y - left_y - 2).max(0) * 6;
            score += (y - right_y - 2).max(0) * 6;
            score += (left_y - y - 2).max(0) * 2;
            score += (right_y - y - 2).max(0) * 2;
            score += Self::progression_spawn_quality_penalty(world, x, y);

            match best {
                None => best = Some((score, x, y)),
                Some((best_score, _, best_y))
                    if score < best_score || (score == best_score && y < best_y) =>
                {
                    best = Some((score, x, y));
                }
                _ => {}
            }
        }

        best.map(|(_, x, y)| (x, y))
    }

    fn progression_spawn_point(world: &mut World, center_x: i32) -> (i32, f64) {
        if center_x == 0
            && let Some((x, y)) = world.designed_spawn()
            && Self::is_safe_spawn_column(world, x, y, true)
        {
            return (x, y as f64 - 0.1);
        }
        world.load_chunks_for_spawn_search(center_x, 64);
        if let Some((x, y)) = Self::find_best_progression_spawn_in_radius(world, center_x, 56) {
            return (x, y as f64 - 0.1);
        }

        world.load_chunks_for_spawn_search(center_x, 160);
        if let Some((x, y)) = Self::find_best_progression_spawn_in_radius(world, center_x, 160) {
            return (x, y as f64 - 0.1);
        }

        if let Some((x, y)) = Self::find_nearest_safe_spawn(world, center_x, 160, true) {
            return (x, y as f64 - 0.1);
        }

        let fallback_y =
            Self::progression_spawn_surface_y(world, center_x).map_or(10.0, |y| y as f64 - 0.1);
        (center_x, fallback_y)
    }

    pub(crate) fn multiplayer_join_spawn_near(&mut self, center_x: i32) -> (f64, f64) {
        match self.current_dimension {
            Dimension::Overworld => {
                self.world.load_chunks_for_spawn_search(center_x, 24);
                if let Some((x, y)) =
                    Self::find_best_progression_spawn_in_radius(&self.world, center_x, 20)
                {
                    return (x as f64 + 0.5, y as f64 - 0.1);
                }
                if let Some((x, y)) = Self::find_nearest_safe_spawn(&self.world, center_x, 24, true)
                {
                    return (x as f64 + 0.5, y as f64 - 0.1);
                }
                let y = Self::progression_spawn_surface_y(&self.world, center_x)
                    .map_or(10.0, |surface_y| surface_y as f64 - 0.1);
                (center_x as f64 + 0.5, y)
            }
            Dimension::Nether | Dimension::End => {
                for dx in 0..=18 {
                    for x in [center_x + dx, center_x - dx] {
                        let surface_y = self.find_walkable_surface(x);
                        if surface_y > 2 {
                            return (x as f64 + 0.5, surface_y as f64 - 0.1);
                        }
                    }
                }
                (
                    center_x as f64 + 0.5,
                    self.find_walkable_surface(center_x) as f64 - 0.1,
                )
            }
        }
    }

    fn sanitize_inventory(mut inventory: Inventory) -> Inventory {
        inventory.capacity = PLAYER_INVENTORY_CAPACITY;
        if inventory.slots.len() < PLAYER_INVENTORY_CAPACITY {
            inventory.slots.resize(PLAYER_INVENTORY_CAPACITY, None);
        } else if inventory.slots.len() > PLAYER_INVENTORY_CAPACITY {
            inventory.slots.truncate(PLAYER_INVENTORY_CAPACITY);
        }
        inventory
    }

    fn sanitize_armor_slots(
        mut armor_slots: [Option<ItemStack>; ARMOR_SLOT_COUNT],
    ) -> [Option<ItemStack>; ARMOR_SLOT_COUNT] {
        for (idx, slot) in armor_slots.iter_mut().enumerate() {
            let Some(stack) = slot.as_mut() else {
                continue;
            };
            if stack.item_type.armor_slot_index() != Some(idx) {
                *slot = None;
                continue;
            }
            stack.count = 1;
            let Some(max_durability) = stack.item_type.max_durability() else {
                *slot = None;
                continue;
            };
            let durability = stack
                .durability
                .unwrap_or(max_durability)
                .min(max_durability);
            stack.durability = Some(durability);
        }
        armor_slots
    }

    fn progression_path() -> PathBuf {
        save_path::save_file(PLAYER_PROGRESS_FILE)
    }

    fn load_progression_data_from_path(path: impl AsRef<Path>) -> Option<PlayerProgressSaveData> {
        let mut encoded = Vec::new();
        File::open(path.as_ref())
            .ok()?
            .take(MAX_CHECKPOINT_BYTES + 1)
            .read_to_end(&mut encoded)
            .ok()?;
        if encoded.len() as u64 > MAX_CHECKPOINT_BYTES {
            return None;
        }
        let version = u16::from_le_bytes(encoded.get(..2)?.try_into().ok()?);
        if matches!(
            version,
            PLAYER_PROGRESS_VERSION
                | PLAYER_PROGRESS_VERSION_V11
                | PLAYER_PROGRESS_VERSION_V10
                | PLAYER_PROGRESS_VERSION_V9
                | PLAYER_PROGRESS_VERSION_V8
                | PLAYER_PROGRESS_VERSION_V7
        ) {
            use bincode::Options;
            let options = bincode::DefaultOptions::new()
                .with_fixint_encoding()
                .with_limit(MAX_CHECKPOINT_BYTES)
                .reject_trailing_bytes();
            let (checkpoint_version, mut data, session, links, runtime): (
                _,
                _,
                PlayerSession,
                _,
                Option<RuntimeCheckpoint>,
            ) = match version {
                PLAYER_PROGRESS_VERSION => {
                    let c: PlayerCheckpointV12 = options.deserialize(&encoded).ok()?;
                    (
                        c.version,
                        c.progress,
                        c.session,
                        c.portal_links,
                        Some(c.runtime),
                    )
                }
                PLAYER_PROGRESS_VERSION_V11 => {
                    let c: PlayerCheckpointV11 = options.deserialize(&encoded).ok()?;
                    (
                        c.version,
                        c.progress,
                        c.session,
                        c.portal_links,
                        Some(c.runtime.into()),
                    )
                }
                PLAYER_PROGRESS_VERSION_V10 => {
                    let c: PlayerCheckpointV10 = options.deserialize(&encoded).ok()?;
                    (
                        c.version,
                        c.progress,
                        c.session.into(),
                        c.portal_links,
                        Some(c.runtime.into()),
                    )
                }
                PLAYER_PROGRESS_VERSION_V9 => {
                    if let Ok(c) = options.deserialize::<PlayerCheckpointV9>(&encoded) {
                        (
                            c.version,
                            c.progress,
                            c.session.into(),
                            c.portal_links,
                            Some(c.runtime.into()),
                        )
                    } else {
                        let c: PlayerCheckpointV10 = options.deserialize(&encoded).ok()?;
                        (
                            c.version,
                            c.progress,
                            c.session.into(),
                            c.portal_links,
                            Some(c.runtime.into()),
                        )
                    }
                }
                PLAYER_PROGRESS_VERSION_V8 => {
                    let c: PlayerCheckpointV8 = options.deserialize(&encoded).ok()?;
                    (
                        c.version,
                        c.progress,
                        c.session.into(),
                        c.portal_links,
                        None,
                    )
                }
                _ => {
                    let c: PlayerCheckpointV7 = options.deserialize(&encoded).ok()?;
                    (c.version, c.progress, c.session.into(), Vec::new(), None)
                }
            };
            if let Some(runtime) = &runtime {
                runtime.validate().ok()?;
                if !runtime.contains_dimension(Self::decode_dimension(data.current_dimension_code)?)
                {
                    return None;
                }
            }
            session.validate().ok()?;
            if checkpoint_version != version
                || data.version != version
                || Self::decode_dimension(data.current_dimension_code).is_none()
                || !data.time_of_day.is_finite()
                || !data.weather_rain_intensity.is_finite()
                || !data.weather_wind_intensity.is_finite()
                || !data.weather_thunder_intensity.is_finite()
                || links
                    .iter()
                    .any(|&((dimension, x, y), (target_x, target_y))| {
                        dimension > 1
                            || !(3..CHUNK_HEIGHT as i32).contains(&y)
                            || !(3..CHUNK_HEIGHT as i32).contains(&target_y)
                            || x.checked_mul(8).is_none()
                            || target_x.checked_mul(8).is_none()
                    })
            {
                return None;
            }
            data.version = PLAYER_PROGRESS_VERSION;
            data.complete_session = Some(session);
            data.runtime = runtime;
            data.portal_links = links;
            return Some(data);
        }
        if let Ok(mut data) = bincode::deserialize::<PlayerProgressSaveData>(&encoded) {
            if data.version == PLAYER_PROGRESS_VERSION_V6 {
                data.version = PLAYER_PROGRESS_VERSION;
                return Some(data);
            }
            return None;
        }
        if let Ok(v5_data) = bincode::deserialize::<PlayerProgressSaveDataV5>(&encoded)
            && v5_data.version == PLAYER_PROGRESS_VERSION_V5
        {
            return Some(v5_data.into());
        }
        if let Ok(v4_data) = bincode::deserialize::<PlayerProgressSaveDataV4>(&encoded)
            && v4_data.version == PLAYER_PROGRESS_VERSION_V4
        {
            return Some(v4_data.into());
        }
        if let Ok(v3_data) = bincode::deserialize::<PlayerProgressSaveDataV3>(&encoded)
            && v3_data.version == PLAYER_PROGRESS_VERSION_V3
        {
            return Some(v3_data.into());
        }
        if let Ok(v2_data) = bincode::deserialize::<PlayerProgressSaveDataV2>(&encoded)
            && v2_data.version == PLAYER_PROGRESS_VERSION_V2
        {
            return Some(v2_data.into());
        }
        if let Ok(v1_data) = bincode::deserialize::<PlayerProgressSaveDataV1>(&encoded)
            && v1_data.version == PLAYER_PROGRESS_VERSION_V1
        {
            return Some(v1_data.into());
        }
        None
    }

    fn progression_temp_path(path: impl AsRef<Path>) -> PathBuf {
        let mut temp_path = path.as_ref().as_os_str().to_os_string();
        temp_path.push(PLAYER_PROGRESS_TMP_SUFFIX);
        PathBuf::from(temp_path)
    }

    fn write_progression_payload_atomically(path: &Path, payload: &[u8]) -> std::io::Result<()> {
        if let Some(parent) = path.parent() {
            crate::platform::fs::create_dir_all(parent)?;
        }
        crate::checkpoint::protect(path)?;
        let temp_path = Self::progression_temp_path(path);
        {
            let mut file = File::create(&temp_path)?;
            file.write_all(payload)?;
            file.sync_all()?;
        }
        if let Err(err) = crate::platform::fs::rename(&temp_path, path) {
            let _ = crate::platform::fs::remove_file(&temp_path);
            return Err(err);
        }
        crate::checkpoint::sync_directory(path.parent().unwrap())
    }

    #[cfg(test)]
    fn save_progression_data_to_path(
        path: impl AsRef<Path>,
        data: &PlayerProgressSaveData,
    ) -> Option<()> {
        let encoded = Self::encode_progression(data).ok()?;
        Self::write_progression_payload_atomically(path.as_ref(), &encoded).ok()?;
        Some(())
    }

    fn load_progression_data() -> Option<PlayerProgressSaveData> {
        if cfg!(test) {
            return None;
        }
        save_path::migrate_legacy_saves_once();
        Self::load_progression_data_from_path(Self::progression_path())
    }

    fn to_progression_save_data(&self) -> PlayerProgressSaveData {
        PlayerProgressSaveData {
            complete_session: Some(PlayerSession::capture(self)),
            runtime: Some(RuntimeCheckpoint::capture(self)),
            portal_links: {
                let mut links: Vec<_> = self
                    .portal_links
                    .iter()
                    .map(|(&source, &target)| (source, target))
                    .collect();
                links.sort_unstable();
                links
            },
            version: PLAYER_PROGRESS_VERSION,
            player_x: self.player.x,
            player_y: self.player.y,
            player_vx: self.player.vx,
            player_vy: self.player.vy,
            player_grounded: self.player.grounded,
            player_facing_right: self.player.facing_right,
            player_sneaking: self.player.sneaking,
            player_health: self.player.health,
            player_hunger: self.player.hunger,
            player_drowning_timer: self.player.drowning_timer,
            player_burning_timer: self.player.burning_timer,
            player_fall_distance: self.player.fall_distance,
            inventory: self.inventory.clone(),
            armor_slots: self.armor_slots.clone(),
            hotbar_index: self.hotbar_index,
            spawn_point_x: self.spawn_point.map(|(x, _)| x).unwrap_or(0),
            spawn_point_y: self.spawn_point.map(|(_, y)| y).unwrap_or(0),
            has_spawn_point: self.spawn_point.is_some(),
            current_dimension_code: Self::encode_dimension(self.current_dimension),
            time_of_day: self.time_of_day,
            weather_code: Self::encode_weather(self.weather),
            weather_timer: self.weather_timer,
            weather_rain_intensity: self.weather_rain_intensity,
            weather_wind_intensity: self.weather_wind_intensity,
            weather_thunder_intensity: self.weather_thunder_intensity,
            thunder_flash_timer: self.thunder_flash_timer,
            dragon_defeated: self.dragon_defeated,
            completion_credits_seen: self.completion_credits_seen,
            movement_profile_code: Self::encode_movement_profile(self.movement_profile),
            portal_cooldown: self.portal_cooldown,
            experience_level: self.player.experience_level,
            experience_progress: self.player.experience_progress,
            experience_total: self.player.experience_total,
            difficulty_code: Self::encode_difficulty(self.difficulty),
            game_rules_preset_code: Self::encode_game_rules_preset(self.game_rules_preset),
            rule_do_mob_spawning: self.game_rules.do_mob_spawning,
            rule_do_daylight_cycle: self.game_rules.do_daylight_cycle,
            rule_do_weather_cycle: self.game_rules.do_weather_cycle,
            rule_keep_inventory: self.game_rules.keep_inventory,
        }
    }

    fn encode_progression(data: &PlayerProgressSaveData) -> bincode::Result<Vec<u8>> {
        if let Some(session) = &data.complete_session {
            bincode::serialize(&PlayerCheckpointRef {
                version: PLAYER_PROGRESS_VERSION,
                progress: data,
                session,
                portal_links: &data.portal_links,
                runtime: data.runtime.as_ref().expect("complete runtime checkpoint"),
            })
        } else {
            // Legacy fixtures; new runtime checkpoints always contain a full session.
            bincode::serialize(data)
        }
    }

    pub fn save_progression(&mut self) {
        if cfg!(test) || self.cooperative() {
            return;
        }
        if let Err(error) = self.try_save_progression() {
            self.save_error
                .replace(Some(format!("Player save failed: {error}")));
        }
    }

    pub fn try_save_progression(&mut self) -> std::io::Result<()> {
        // Multiplayer identities are saved by the server, never over the
        // single-player checkpoint of a named world shared through co-op.
        if self.cooperative() {
            return self.save_trial();
        }
        self.world.check_storage()?;
        let data = self.to_progression_save_data();
        data.runtime
            .as_ref()
            .unwrap()
            .validate()
            .map_err(std::io::Error::other)?;
        data.complete_session
            .as_ref()
            .unwrap()
            .validate()
            .map_err(std::io::Error::other)?;
        let encoded = Self::encode_progression(&data).map_err(std::io::Error::other)?;
        if encoded.len() as u64 > MAX_CHECKPOINT_BYTES {
            return Err(std::io::Error::other(
                "runtime checkpoint exceeds 16 MiB; previous save preserved",
            ));
        }
        // Inventory, chests, terrain and entities belong to one recoverable
        // checkpoint, including dimensions currently outside the active view.
        self.save_dimension_worlds()?;
        Self::write_progression_payload_atomically(
            &self.world.save_root().join(PLAYER_PROGRESS_FILE),
            &encoded,
        )?;
        self.save_trial()?;
        crate::checkpoint::commit(self.world.save_root())?;
        self.runtime_revision
            .set(data.runtime.as_ref().unwrap().revision);
        Ok(())
    }

    pub(crate) fn write_recovery_directory(&self, root: &std::path::Path) -> std::io::Result<()> {
        self.world.write_recovery_directory(root)?;
        let data = self.to_progression_save_data();
        data.runtime
            .as_ref()
            .unwrap()
            .validate()
            .map_err(std::io::Error::other)?;
        data.complete_session
            .as_ref()
            .unwrap()
            .validate()
            .map_err(std::io::Error::other)?;
        let encoded = Self::encode_progression(&data).map_err(std::io::Error::other)?;
        if encoded.len() as u64 > MAX_CHECKPOINT_BYTES {
            return Err(std::io::Error::other(
                "runtime checkpoint exceeds 16 MiB; previous save preserved",
            ));
        }
        Self::write_progression_payload_atomically(&root.join(PLAYER_PROGRESS_FILE), &encoded)?;
        self.save_trial_in(root)
    }

    pub fn autosave_world_step(&mut self, max_chunks: usize) -> bool {
        self.world.save_dirty_chunk_budget(max_chunks)
    }

    pub fn persist_world_and_progress(&mut self) {
        if let Err(error) = self.try_persist_world_and_progress() {
            self.save_error
                .replace(Some(format!("Save failed: {error}")));
        }
    }

    pub fn try_persist_world_and_progress(&mut self) -> std::io::Result<()> {
        if self.cooperative() {
            self.save_dimension_worlds()?;
            self.save_trial()?;
        } else {
            self.try_save_progression()?;
        }
        self.save_error.replace(None);
        Ok(())
    }

    fn save_before_dimension_change(&mut self) -> bool {
        let result = self.save_dimension_worlds().and_then(|_| {
            if self.cooperative() {
                Ok(())
            } else {
                self.try_save_progression()
            }
        });
        match result {
            Ok(()) => true,
            Err(error) => {
                self.save_error
                    .replace(Some(format!("Travel cancelled; save failed: {error}")));
                false
            }
        }
    }

    pub fn new() -> Self {
        let progress = Self::load_progression_data();
        let dimension = progress
            .as_ref()
            .and_then(|saved| Self::decode_dimension(saved.current_dimension_code))
            .unwrap_or(Dimension::Overworld);
        Self::from_progress(progress, World::new_for_dimension(dimension))
    }

    pub(crate) fn network_view() -> Self {
        let mut state = Self::from_progress(None, World::network_replica(Dimension::Overworld));
        state.dismiss_startup_splash();
        state
    }

    pub fn try_new() -> std::io::Result<Self> {
        save_path::migrate_legacy_saves_once();
        Self::open_in_directory(&save_path::save_dir())
    }

    pub fn open_in_directory(root: &Path) -> std::io::Result<Self> {
        crate::checkpoint::recover(root)?;
        let path = root.join(PLAYER_PROGRESS_FILE);
        let progress = Self::load_progression_data_from_path(&path);
        if crate::platform::fs::exists(&path)? && progress.is_none() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "player save could not be decoded; the world was not reset",
            ));
        }
        let dimension = progress
            .as_ref()
            .and_then(|saved| Self::decode_dimension(saved.current_dimension_code))
            .unwrap_or(Dimension::Overworld);
        let has_runtime = progress.as_ref().is_some_and(|p| p.runtime.is_some());
        let mut state = Self::from_progress(progress, World::open_in_directory(dimension, root)?);
        state.check_dimension_storage()?;
        if !has_runtime {
            state.load_trial()?;
        }
        // Terrain is shared between solo and hosted play; use the newest entity
        // checkpoint while keeping their deliberately separate player inventories.
        if crate::platform::fs::exists(root.join("server-state.json"))? {
            let server = super::server_save::ServerSave::read(&state)?;
            if let Some(runtime) = &server.runtime
                && runtime.revision > state.runtime_revision.get()
            {
                runtime.apply_for_native(&mut state, dimension);
                state.check_dimension_storage()?;
            }
        }
        crate::checkpoint::begin(root)?;
        Ok(state)
    }

    fn from_progress(progress: Option<PlayerProgressSaveData>, mut world: World) -> Self {
        let center_x = progress
            .as_ref()
            .map_or(0, |saved| saved.player_x.floor() as i32);

        let dimension = world.dimension;
        if !world.is_replica() {
            world.load_chunks_around(center_x);
        }
        let (spawn_x, spawn_y) = if progress.is_some() || world.is_replica() {
            (center_x, 10.0)
        } else {
            let spawn = Self::progression_spawn_point(&mut world, center_x);
            world.newly_generated_chunks.clear();
            spawn
        };
        let player_x = progress
            .as_ref()
            .map_or(spawn_x as f64 + 0.5, |saved| saved.player_x);

        let mut state = Self {
            quit_confirmation: false,
            sleeping_beds: Default::default(),
            last_pearl_throw_age: Default::default(),
            player_notices: Vec::new(),
            active_peer: None,
            cooperative_players: Default::default(),
            cooperative_dimensions: Default::default(),
            cooperative_worlds: Default::default(),
            runtime_revision: std::cell::Cell::new(0),
            director_encounter: None,
            chat: super::director::ChatView::default(),
            performance: super::metrics::PerformanceMetrics::default(),
            show_debug_overlay: false,
            save_error: std::cell::RefCell::new(None),
            player: Player::new(player_x, spawn_y),
            remote_players: Vec::new(),
            local_player_id: None,
            pvp_enabled: true,
            host_client_id: None,
            connection_address: None,
            shortcut_players: Default::default(),
            solo_shortcuts: None,
            action_visuals: Default::default(),
            inventory: Inventory::new(PLAYER_INVENTORY_CAPACITY),
            armor_slots: [None, None, None, None],
            crafting_grid: std::array::from_fn(|_| None),
            inventory_enchant_levels: [0; PLAYER_INVENTORY_CAPACITY],
            armor_enchant_levels: [0; ARMOR_SLOT_COUNT],
            trial: None,
            zombies: Vec::new(),
            creepers: Vec::new(),
            skeletons: Vec::new(),
            spiders: Vec::new(),
            silverfish: Vec::new(),
            slimes: Vec::new(),
            endermen: Vec::new(),
            blazes: Vec::new(),
            pigmen: Vec::new(),
            ghasts: Vec::new(),
            cows: Vec::new(),
            sheep: Vec::new(),
            pigs: Vec::new(),
            chickens: Vec::new(),
            squids: Vec::new(),
            wolves: Vec::new(),
            ocelots: Vec::new(),
            villagers: Vec::new(),
            boats: Vec::new(),
            item_entities: Vec::new(),
            experience_orbs: Vec::new(),
            arrows: Vec::new(),
            ender_pearls: Vec::new(),
            fireballs: Vec::new(),
            end_crystals: Vec::new(),
            ender_dragon: None,
            lightning_bolts: Vec::new(),
            world,
            hotbar_index: 0,
            mouse_x: 0,
            mouse_y: 0,
            left_click_down: false,
            time_of_day: 8000.0,
            weather: WeatherType::Clear,
            thunder_flash_timer: 0,
            weather_rain_intensity: 0.0,
            weather_wind_intensity: 0.1,
            weather_thunder_intensity: 0.0,
            world_tick: 0,
            eye_guidance_timer: 0,
            eye_guidance_dir: 0,
            eye_guidance_distance: 0,
            moving_left: false,
            moving_right: false,
            inventory_open: false,
            at_crafting_table: false,
            at_furnace: false,
            at_chest: false,
            at_enchanting_table: false,
            at_anvil: false,
            at_brewing_stand: false,
            selected_inventory_slot: None,
            current_dimension: Dimension::Overworld,
            open_chest_pos: None,
            spawn_point: None,
            weather_timer: 7200,
            portal_timer: 0,
            portal_cooldown: 0,
            portal_links: HashMap::new(),
            dungeon_spawner_timer: 0,
            silverfish_spawner_timer: 0,
            blaze_spawner_timer: 0,
            overworld_hostile_spawn_timer: OVERWORLD_HOSTILE_RESPAWN_BASE,
            overworld_passive_spawn_timer: OVERWORLD_PASSIVE_RESPAWN_BASE,
            overworld_villager_spawn_timer: OVERWORLD_VILLAGER_RESPAWN_BASE,
            overworld_squid_spawn_timer: OVERWORLD_SQUID_RESPAWN_BASE,
            overworld_wolf_spawn_timer: OVERWORLD_WOLF_RESPAWN_BASE,
            overworld_ocelot_spawn_timer: OVERWORLD_OCELOT_RESPAWN_BASE,
            villager_open_doors: HashMap::new(),
            nether_spawn_timer: NETHER_RESPAWN_BASE,
            end_spawn_timer: END_RESPAWN_BASE,
            end_boss_initialized: false,
            dragon_defeated: false,
            end_victory_ticks: 0,
            end_victory_origin: None,
            movement_profile: MovementProfile::Classic,
            difficulty: Difficulty::Normal,
            game_rules_preset: GameRulesPreset::Vanilla,
            game_rules: GameRulesPreset::Vanilla.rules(),
            sneak_toggled: false,
            sneak_held: false,
            jump_held: false,
            sprinting: false,
            sprint_direction: 0,
            sprint_left_tap_ticks: 0,
            sprint_right_tap_ticks: 0,
            jump_buffer_ticks: 0,
            coyote_ticks: 0,
            completion_credits_seen: false,
            credits_active: false,
            credits_tick: 0,
            death_screen_active: false,
            death_screen_ticks: 0,
            startup_splash_active: !cfg!(test),
            startup_splash_ticks: 0,
            settings_menu_open: false,
            settings_menu_selected: 0,
            respawn_grace_ticks: 0,
            player_combat_hurt_cooldown: 0,
            bow_draw_ticks: 0,
            bow_draw_active: false,
            fishing_bobber_active: false,
            fishing_bobber_x: 0.0,
            fishing_bobber_y: 0.0,
            fishing_wait_ticks: 0,
            fishing_bite_window_ticks: 0,
            mounted_boat: None,
            furnace_slots: [None, None, None],
            furnace_explicit: false,
            furnace_job: None,
            furnace_progress_ticks: 0,
            furnace_burn_ticks: 0,
            potion_strength_timer: 0,
            potion_regeneration_timer: 0,
            potion_fire_resistance_timer: 0,
        };

        let mut runtime = None;
        if let Some(saved) = progress {
            runtime = saved.runtime;
            state.player.x = saved.player_x;
            state.player.y = saved.player_y;
            state.player.vx = saved.player_vx.clamp(-4.0, 4.0);
            state.player.vy = saved.player_vy.clamp(-4.0, 4.0);
            state.player.grounded = saved.player_grounded;
            state.player.facing_right = saved.player_facing_right;
            state.player.health = saved.player_health.clamp(0.0, state.player.max_health);
            state.player.hunger = saved.player_hunger.clamp(0.0, state.player.max_hunger);
            state.player.drowning_timer = saved.player_drowning_timer.clamp(-200, 300);
            state.player.burning_timer = saved.player_burning_timer.clamp(0, 400);
            state.player.fall_distance = saved.player_fall_distance.clamp(0.0, 120.0);
            state.sneak_toggled = saved.player_sneaking;
            state.sneak_held = false;
            state.sync_sneak_state();
            state.inventory = Self::sanitize_inventory(saved.inventory);
            state.armor_slots = Self::sanitize_armor_slots(saved.armor_slots);
            state.hotbar_index = saved.hotbar_index.min(8);
            state.spawn_point = if saved.has_spawn_point {
                Some((saved.spawn_point_x, saved.spawn_point_y))
            } else {
                None
            };
            state.current_dimension = dimension;
            state.time_of_day = saved.time_of_day.rem_euclid(24000.0);
            state.weather = Self::decode_weather(saved.weather_code).unwrap_or(WeatherType::Clear);
            state.weather_timer = saved.weather_timer.max(1);
            state.weather_rain_intensity = saved.weather_rain_intensity.clamp(0.0, 1.0);
            state.weather_wind_intensity = saved.weather_wind_intensity.clamp(0.0, 1.0);
            state.weather_thunder_intensity = saved.weather_thunder_intensity.clamp(0.0, 1.0);
            state.thunder_flash_timer = saved.thunder_flash_timer.min(6);
            state.dragon_defeated = saved.dragon_defeated;
            state.completion_credits_seen = saved.completion_credits_seen;
            state.movement_profile = Self::decode_movement_profile(saved.movement_profile_code)
                .unwrap_or(MovementProfile::Classic);
            state.portal_cooldown = saved.portal_cooldown.min(120);
            let saved_level = saved.experience_level.min(10_000);
            let saved_progress = saved.experience_progress.clamp(0.0, 0.999_999);
            let implied_total = Self::total_experience_for_level(saved_level).saturating_add(
                (saved_progress * Self::xp_to_next_level(saved_level) as f32).floor() as u32,
            );
            let saved_total = saved.experience_total.max(implied_total);
            state.set_total_experience(saved_total);
            state.difficulty =
                Self::decode_difficulty(saved.difficulty_code).unwrap_or(Difficulty::Normal);
            state.game_rules = GameRules {
                do_mob_spawning: saved.rule_do_mob_spawning,
                do_daylight_cycle: saved.rule_do_daylight_cycle,
                do_weather_cycle: saved.rule_do_weather_cycle,
                keep_inventory: saved.rule_keep_inventory,
            };
            state.game_rules_preset = Self::decode_game_rules_preset(saved.game_rules_preset_code)
                .unwrap_or_else(|| Self::infer_game_rules_preset(state.game_rules));
            state.sync_game_rules_preset_from_rules();

            state.portal_links = saved.portal_links.into_iter().collect();
            if let Some(mut session) = saved.complete_session {
                session.release_inputs();
                session.apply(&mut state);
                state.sync_sneak_state();
            }

            if state.current_dimension != Dimension::Overworld {
                state.reset_weather_for_dimension();
            }
            state
                .world
                .load_chunks_around(state.player.x.floor() as i32);
        }

        state.reset_spawn_timers_for_rules();
        if let Some(runtime) = runtime {
            let dimension = state.current_dimension;
            runtime.apply_for_native(&mut state, dimension);
        }
        state.apply_peaceful_cleanup_if_needed();

        state
    }

    pub fn is_showing_credits(&self) -> bool {
        self.credits_active
    }

    pub fn is_showing_death_screen(&self) -> bool {
        self.death_screen_active
    }

    pub fn is_showing_startup_splash(&self) -> bool {
        self.startup_splash_active
    }

    pub fn startup_splash_tick(&self) -> u16 {
        self.startup_splash_ticks
    }

    pub fn dismiss_startup_splash(&mut self) {
        self.startup_splash_active = false;
        self.startup_splash_ticks = 0;
    }

    pub fn set_remote_ui_modal_state(&mut self, death_screen_active: bool, credits_active: bool) {
        self.death_screen_active = death_screen_active;
        if death_screen_active {
            // Remote snapshots do not carry the lockout counter, so keep it respawn-ready.
            self.death_screen_ticks = DEATH_RESPAWN_DELAY_TICKS;
        } else {
            self.death_screen_ticks = 0;
        }

        self.credits_active = credits_active;
        if credits_active {
            self.credits_tick = self.credits_tick.saturating_add(1);
        } else {
            self.credits_tick = 0;
        }
    }

    pub fn is_settings_menu_open(&self) -> bool {
        self.settings_menu_open
    }

    pub fn settings_menu_selected_index(&self) -> u8 {
        self.settings_menu_selected
    }

    pub fn death_respawn_ticks_remaining(&self) -> u16 {
        DEATH_RESPAWN_DELAY_TICKS.saturating_sub(self.death_screen_ticks)
    }

    pub fn can_respawn_from_death_screen(&self) -> bool {
        self.death_screen_active && self.death_respawn_ticks_remaining() == 0
    }

    pub fn movement_profile_name(&self) -> &'static str {
        match self.movement_profile {
            MovementProfile::Classic => "Classic",
            MovementProfile::Smooth => "Smooth",
            MovementProfile::Agile => "Agile",
        }
    }

    pub fn is_sprinting(&self) -> bool {
        self.sprinting
    }

    pub fn inventory_slot_enchant_level(&self, slot_idx: usize) -> u8 {
        self.inventory_enchant_levels
            .get(slot_idx)
            .copied()
            .unwrap_or(0)
            .min(ENCHANT_MAX_LEVEL)
    }

    pub fn selected_hotbar_enchant_level(&self) -> u8 {
        self.inventory_slot_enchant_level(self.hotbar_index as usize)
    }

    pub fn fishing_bobber(&self) -> Option<(f64, f64, bool)> {
        if !self.fishing_bobber_active {
            return None;
        }
        Some((
            self.fishing_bobber_x,
            self.fishing_bobber_y,
            self.fishing_bite_window_ticks > 0,
        ))
    }

    pub fn fishing_status_line(&self) -> Option<&'static str> {
        if !self.fishing_bobber_active {
            return None;
        }
        if self.fishing_bite_window_ticks > 0 {
            Some("Fishing: Bite! Right-click")
        } else {
            Some("Fishing: Waiting...")
        }
    }

    fn can_item_be_enchanted(item: ItemType) -> bool {
        item.max_durability().is_some()
    }

    fn effective_held_damage(&self, item: Option<ItemType>) -> f32 {
        let base = item.map_or(1.0, |it| it.damage());
        let enchant_bonus = self.selected_hotbar_enchant_level() as f32;
        let potion_bonus = if self.potion_strength_timer > 0 {
            POTION_STRENGTH_MELEE_BONUS
        } else {
            0.0
        };
        (base + enchant_bonus + potion_bonus).max(1.0)
    }

    fn effective_held_efficiency(&self, item: ItemType, block: BlockType) -> f32 {
        let base = item.efficiency(block);
        let enchant_level = self.selected_hotbar_enchant_level() as f32;
        if enchant_level <= 0.0 {
            base
        } else {
            base * (1.0 + enchant_level * 0.35)
        }
    }

    pub fn difficulty_name(&self) -> &'static str {
        match self.difficulty {
            Difficulty::Peaceful => "Peaceful",
            Difficulty::Easy => "Easy",
            Difficulty::Normal => "Normal",
            Difficulty::Hard => "Hard",
        }
    }

    pub fn game_rules_preset_name(&self) -> &'static str {
        self.game_rules_preset.display_name()
    }

    pub fn game_rule_flags(&self) -> (bool, bool, bool, bool) {
        (
            self.game_rules.do_mob_spawning,
            self.game_rules.do_daylight_cycle,
            self.game_rules.do_weather_cycle,
            self.game_rules.keep_inventory,
        )
    }

    fn infer_game_rules_preset(rules: GameRules) -> GameRulesPreset {
        if rules == GameRulesPreset::Vanilla.rules() {
            GameRulesPreset::Vanilla
        } else if rules == GameRulesPreset::KeepInventory.rules() {
            GameRulesPreset::KeepInventory
        } else if rules == GameRulesPreset::Builder.rules() {
            GameRulesPreset::Builder
        } else {
            GameRulesPreset::Custom
        }
    }

    fn sync_game_rules_preset_from_rules(&mut self) {
        self.game_rules_preset = Self::infer_game_rules_preset(self.game_rules);
    }

    fn active_game_rules(&self) -> GameRules {
        self.game_rules
    }

    pub fn armor_slot_item(&self, slot_idx: usize) -> Option<&ItemStack> {
        self.armor_slots.get(slot_idx)?.as_ref()
    }

    pub fn total_armor_points(&self) -> u8 {
        let mut total = 0u8;
        for (idx, slot) in self.armor_slots.iter().enumerate() {
            let Some(stack) = slot.as_ref() else {
                continue;
            };
            total = total.saturating_add(stack.item_type.armor_points());
            let enchant_bonus = self.armor_enchant_levels[idx]
                .min(ENCHANT_MAX_LEVEL)
                .div_ceil(2);
            total = total.saturating_add(enchant_bonus);
        }
        total.min(20)
    }

    fn xp_to_next_level(level: u32) -> u32 {
        match level {
            0..=15 => 2 * level + 7,
            16..=30 => 5 * level - 38,
            _ => 9 * level - 158,
        }
    }

    fn total_experience_for_level(level: u32) -> u32 {
        match level {
            0..=16 => level * level + 6 * level,
            17..=31 => {
                ((5 * level * level)
                    .saturating_sub(81 * level)
                    .saturating_add(720))
                    / 2
            }
            _ => {
                ((9 * level * level)
                    .saturating_sub(325 * level)
                    .saturating_add(4440))
                    / 2
            }
        }
    }

    fn set_total_experience(&mut self, total: u32) {
        self.player.experience_total = total;
        let mut level = 0u32;
        let mut remaining = total;
        loop {
            let needed = Self::xp_to_next_level(level);
            if remaining < needed {
                break;
            }
            remaining = remaining.saturating_sub(needed);
            level = level.saturating_add(1);
            if level >= 10_000 {
                break;
            }
        }
        self.player.experience_level = level;
        let needed = Self::xp_to_next_level(level).max(1);
        self.player.experience_progress = (remaining as f32 / needed as f32).clamp(0.0, 0.999_999);
    }

    fn add_experience(&mut self, amount: u32) {
        if amount == 0 {
            return;
        }
        let total = self.player.experience_total.saturating_add(amount);
        self.set_total_experience(total);
    }

    pub fn experience_to_next_level(&self) -> u32 {
        Self::xp_to_next_level(self.player.experience_level)
    }

    fn spend_experience_levels(&mut self, levels: u32) -> bool {
        if levels == 0 {
            return true;
        }
        if self.player.experience_level < levels {
            return false;
        }
        let target_level = self.player.experience_level - levels;
        let progress_points = (self.player.experience_progress
            * Self::xp_to_next_level(target_level) as f32)
            .floor() as u32;
        let target_total =
            Self::total_experience_for_level(target_level).saturating_add(progress_points);
        self.set_total_experience(target_total);
        true
    }

    fn spawn_experience_orbs(&mut self, x: f64, y: f64, amount: u32, rng: &mut impl Rng) {
        if amount == 0 {
            return;
        }
        let mut remaining = amount;
        const ORB_SPLITS: [u32; 11] = [2477, 1237, 617, 307, 149, 73, 37, 17, 7, 3, 1];
        while remaining > 0 {
            let value = ORB_SPLITS
                .iter()
                .copied()
                .find(|split| *split <= remaining)
                .unwrap_or(1);
            remaining -= value;
            let mut orb = ExperienceOrb::new(x, y, value);
            orb.vx = rng.gen_range(-0.07..0.07);
            orb.vy = rng.gen_range(-0.18..-0.05);
            self.experience_orbs.push(orb);
        }
    }

    fn experience_from_mined_block(block: BlockType, rng: &mut impl Rng) -> u32 {
        match block {
            BlockType::CoalOre => rng.gen_range(1..=2),
            BlockType::RedstoneOre | BlockType::LitRedstoneOre(_) => rng.gen_range(1..=5),
            BlockType::DiamondOre => rng.gen_range(3..=7),
            BlockType::SilverfishSpawner => rng.gen_range(15..=43),
            BlockType::BlazeSpawner => rng.gen_range(18..=46),
            BlockType::ZombieSpawner | BlockType::SkeletonSpawner => rng.gen_range(15..=43),
            _ => 0,
        }
    }

    fn mob_has_player_kill_credit(&self, last_player_damage_tick: u64) -> bool {
        last_player_damage_tick > 0
            && self.world_tick.saturating_sub(last_player_damage_tick)
                <= PLAYER_KILL_XP_CREDIT_TICKS
    }

    fn apply_armor_wear_from_hit(&mut self) {
        for idx in 0..ARMOR_SLOT_COUNT {
            let Some(stack) = self.armor_slots[idx].as_mut() else {
                continue;
            };
            let Some(durability) = stack.durability.as_mut() else {
                continue;
            };
            let unbreaking_roll = self.armor_enchant_levels[idx].min(ENCHANT_MAX_LEVEL) as u32;
            if unbreaking_roll > 0 && rand::thread_rng().gen_range(0..=unbreaking_roll) > 0 {
                continue;
            }
            if *durability == 0 {
                self.armor_slots[idx] = None;
                self.armor_enchant_levels[idx] = 0;
                continue;
            }
            *durability -= 1;
            if *durability == 0 {
                self.armor_slots[idx] = None;
                self.armor_enchant_levels[idx] = 0;
            }
        }
    }

    fn sanitize_enchant_levels(&mut self) {
        for (idx, slot) in self.inventory.slots.iter().enumerate() {
            let keep = slot.as_ref().is_some_and(|stack| {
                Self::can_item_be_enchanted(stack.item_type) && stack.count == 1
            });
            if !keep {
                self.inventory_enchant_levels[idx] = 0;
            } else {
                self.inventory_enchant_levels[idx] =
                    self.inventory_enchant_levels[idx].min(ENCHANT_MAX_LEVEL);
            }
        }
        for (idx, slot) in self.armor_slots.iter().enumerate() {
            if slot
                .as_ref()
                .is_some_and(|stack| Self::can_item_be_enchanted(stack.item_type))
            {
                self.armor_enchant_levels[idx] =
                    self.armor_enchant_levels[idx].min(ENCHANT_MAX_LEVEL);
            } else {
                self.armor_enchant_levels[idx] = 0;
            }
        }
    }

    fn apply_player_damage(&mut self, amount: f32) {
        if amount <= 0.0 || self.death_screen_active || self.respawn_grace_ticks > 0 {
            return;
        }
        let armor_points = self.total_armor_points() as f32;
        let reduction = (armor_points * 0.04).clamp(0.0, 0.8);
        let adjusted = (amount * (1.0 - reduction)).max(0.1);
        self.player.health -= adjusted;
        self.player_combat_hurt_cooldown = self.player_combat_hurt_cooldown.max(10);
        if armor_points > 0.0 {
            self.apply_armor_wear_from_hit();
        }
    }

    fn apply_player_environment_damage(&mut self, amount: f32) {
        if amount <= 0.0 || self.death_screen_active || self.respawn_grace_ticks > 0 {
            return;
        }
        self.player.health -= amount;
        self.player_combat_hurt_cooldown = self.player_combat_hurt_cooldown.max(10);
    }

    pub fn player_hurt_ticks(&self) -> u8 {
        self.player_combat_hurt_cooldown
    }

    fn apply_player_combat_damage(&mut self, amount: f32) -> bool {
        if self.player_combat_hurt_cooldown > 0 {
            return false;
        }
        let before = self.player.health;
        self.apply_player_damage(amount);
        let damaged = self.player.health + f32::EPSILON < before;
        if damaged {
            self.player_combat_hurt_cooldown = self.player_combat_hurt_cooldown.max(10);
        }
        damaged
    }

    fn can_melee_contact_player(
        &self,
        mob_x: f64,
        mob_center_y: f64,
        horizontal_reach: f64,
        vertical_reach: f64,
    ) -> bool {
        let player_center_y = self.player.y - 0.9;
        if (self.player.x - mob_x).abs() > horizontal_reach {
            return false;
        }
        if (player_center_y - mob_center_y).abs() > vertical_reach {
            return false;
        }
        self.has_line_of_sight(mob_x, mob_center_y, self.player.x, player_center_y)
    }

    fn apply_player_contact_knockback(
        &mut self,
        source_x: f64,
        horizontal_knockback: f64,
        vertical_knockback: f64,
    ) {
        let push_dir = if self.player.x >= source_x { 1.0 } else { -1.0 };
        let aligned_speed = self.player.vx * push_dir;
        let damping = if aligned_speed > 0.45 {
            0.5
        } else if aligned_speed > 0.2 {
            0.75
        } else {
            1.0
        };
        self.player.vx =
            (self.player.vx + push_dir * horizontal_knockback * damping).clamp(-0.9, 0.9);
        if vertical_knockback > 0.0 {
            self.player.vy = self.player.vy.min(-vertical_knockback);
            self.player.grounded = false;
        }
    }

    fn can_spawn_mobs(&self) -> bool {
        self.active_game_rules().do_mob_spawning
    }

    fn can_spawn_hostiles(&self) -> bool {
        self.can_spawn_mobs() && self.difficulty != Difficulty::Peaceful
    }

    fn hostile_damage_multiplier(&self) -> f32 {
        match self.difficulty {
            Difficulty::Peaceful => 0.0,
            Difficulty::Easy => 0.7,
            Difficulty::Normal => 1.0,
            Difficulty::Hard => 1.4,
        }
    }

    fn scaled_hostile_damage(&self, base: f32) -> f32 {
        base * self.hostile_damage_multiplier()
    }

    fn hostile_cooldown_scale(&self) -> f32 {
        match self.difficulty {
            Difficulty::Peaceful => 1.5,
            Difficulty::Easy => 1.25,
            Difficulty::Normal => 1.0,
            Difficulty::Hard => 0.82,
        }
    }

    fn scaled_hostile_cooldown(&self, base: u8) -> u8 {
        let scaled = (base as f32 * self.hostile_cooldown_scale()).round() as i32;
        scaled.clamp(4, u8::MAX as i32) as u8
    }

    fn hostile_spawn_cap_multiplier(&self) -> f64 {
        match self.difficulty {
            Difficulty::Peaceful => 0.0,
            Difficulty::Easy => 0.72,
            Difficulty::Normal => 1.0,
            Difficulty::Hard => 1.3,
        }
    }

    fn scaled_hostile_cap(&self, base: usize) -> usize {
        if !self.can_spawn_hostiles() {
            return 0;
        }
        let scaled = (base as f64 * self.hostile_spawn_cap_multiplier()).round() as usize;
        scaled.max(1)
    }

    fn hostile_spawn_timer_scale(&self) -> f64 {
        match self.difficulty {
            Difficulty::Peaceful => 1.4,
            Difficulty::Easy => 1.24,
            Difficulty::Normal => 1.0,
            Difficulty::Hard => 0.78,
        }
    }

    fn hostile_spawn_chance_multiplier(&self) -> f64 {
        match self.difficulty {
            Difficulty::Peaceful => 0.0,
            Difficulty::Easy => 0.75,
            Difficulty::Normal => 1.0,
            Difficulty::Hard => 1.25,
        }
    }

    fn starvation_health_floor(&self) -> f32 {
        match self.difficulty {
            Difficulty::Peaceful => self.player.max_health,
            Difficulty::Easy => 10.0,
            Difficulty::Normal => 1.0,
            Difficulty::Hard => 0.0,
        }
    }

    fn overworld_chunk_hostile_rolls(&self, rng: &mut impl Rng) -> usize {
        match self.difficulty {
            Difficulty::Peaceful => 0,
            Difficulty::Easy => rng.gen_range(0..=1),
            Difficulty::Normal => rng.gen_range(0..2),
            Difficulty::Hard => rng.gen_range(1..=2),
        }
    }

    fn scaled_hostile_respawn_base(&self, base: u16) -> u16 {
        ((base as f64 * self.hostile_spawn_timer_scale()).round() as u16).max(6)
    }

    fn spawn_refill_threshold(cap: usize, numerator: usize, denominator: usize) -> usize {
        if cap == 0 {
            return 0;
        }
        cap.saturating_mul(numerator)
            .div_ceil(denominator)
            .clamp(1, cap)
    }

    fn overworld_biome_at_player(&self) -> BiomeType {
        self.world.get_biome(self.player.x.floor() as i32)
    }

    fn tuned_overworld_passive_cap(&self, biome: BiomeType) -> usize {
        match biome {
            BiomeType::Plains | BiomeType::Forest => OVERWORLD_PASSIVE_CAP + 2,
            BiomeType::Taiga | BiomeType::Tundra | BiomeType::ExtremeHills => OVERWORLD_PASSIVE_CAP,
            BiomeType::Swamp | BiomeType::Jungle => OVERWORLD_PASSIVE_CAP.saturating_sub(1),
            BiomeType::Desert => OVERWORLD_PASSIVE_CAP.saturating_sub(6),
            BiomeType::Ocean | BiomeType::River => OVERWORLD_PASSIVE_CAP.saturating_sub(8),
        }
    }

    fn tuned_overworld_squid_cap(&self, biome: BiomeType) -> usize {
        match biome {
            BiomeType::Ocean => OVERWORLD_SQUID_CAP + 4,
            BiomeType::River => OVERWORLD_SQUID_CAP + 2,
            BiomeType::Swamp => OVERWORLD_SQUID_CAP,
            _ => OVERWORLD_SQUID_CAP.saturating_sub(2),
        }
    }

    fn tuned_overworld_wolf_cap(&self, biome: BiomeType) -> usize {
        match biome {
            BiomeType::Taiga | BiomeType::Tundra => OVERWORLD_WOLF_CAP + 2,
            BiomeType::Forest => OVERWORLD_WOLF_CAP + 1,
            _ => OVERWORLD_WOLF_CAP.saturating_sub(2),
        }
    }

    fn tuned_overworld_ocelot_cap(&self, biome: BiomeType) -> usize {
        match biome {
            BiomeType::Jungle => OVERWORLD_OCELOT_CAP + 2,
            _ => OVERWORLD_OCELOT_CAP.saturating_sub(2),
        }
    }

    fn tuned_overworld_passive_respawn_base(&self, biome: BiomeType) -> u16 {
        match biome {
            BiomeType::Plains => OVERWORLD_PASSIVE_RESPAWN_BASE.saturating_sub(14),
            BiomeType::Forest => OVERWORLD_PASSIVE_RESPAWN_BASE.saturating_sub(8),
            BiomeType::Taiga | BiomeType::Tundra | BiomeType::ExtremeHills => {
                OVERWORLD_PASSIVE_RESPAWN_BASE.saturating_sub(2)
            }
            BiomeType::Swamp | BiomeType::Jungle => OVERWORLD_PASSIVE_RESPAWN_BASE + 4,
            BiomeType::Desert => OVERWORLD_PASSIVE_RESPAWN_BASE + 18,
            BiomeType::Ocean | BiomeType::River => OVERWORLD_PASSIVE_RESPAWN_BASE + 12,
        }
    }

    fn tuned_overworld_squid_respawn_base(&self, biome: BiomeType) -> u16 {
        match biome {
            BiomeType::Ocean => OVERWORLD_SQUID_RESPAWN_BASE.saturating_sub(14),
            BiomeType::River => OVERWORLD_SQUID_RESPAWN_BASE.saturating_sub(6),
            BiomeType::Swamp => OVERWORLD_SQUID_RESPAWN_BASE,
            _ => OVERWORLD_SQUID_RESPAWN_BASE + 12,
        }
    }

    fn tuned_overworld_wolf_respawn_base(&self, biome: BiomeType) -> u16 {
        match biome {
            BiomeType::Taiga | BiomeType::Tundra => OVERWORLD_WOLF_RESPAWN_BASE.saturating_sub(16),
            BiomeType::Forest => OVERWORLD_WOLF_RESPAWN_BASE.saturating_sub(8),
            _ => OVERWORLD_WOLF_RESPAWN_BASE + 12,
        }
    }

    fn tuned_overworld_ocelot_respawn_base(&self, biome: BiomeType) -> u16 {
        match biome {
            BiomeType::Jungle => OVERWORLD_OCELOT_RESPAWN_BASE.saturating_sub(24),
            _ => OVERWORLD_OCELOT_RESPAWN_BASE + 8,
        }
    }

    fn reset_spawn_timers_for_rules(&mut self) {
        self.dungeon_spawner_timer = 0;
        self.silverfish_spawner_timer = 0;
        self.blaze_spawner_timer = 0;
        let local_biome = if self.current_dimension == Dimension::Overworld {
            self.overworld_biome_at_player()
        } else {
            BiomeType::Plains
        };
        self.overworld_hostile_spawn_timer =
            self.scaled_hostile_respawn_base(OVERWORLD_HOSTILE_RESPAWN_BASE);
        self.overworld_passive_spawn_timer = self.tuned_overworld_passive_respawn_base(local_biome);
        self.overworld_villager_spawn_timer = OVERWORLD_VILLAGER_RESPAWN_BASE;
        self.overworld_squid_spawn_timer = self.tuned_overworld_squid_respawn_base(local_biome);
        self.overworld_wolf_spawn_timer = self.tuned_overworld_wolf_respawn_base(local_biome);
        self.overworld_ocelot_spawn_timer = self.tuned_overworld_ocelot_respawn_base(local_biome);
        self.nether_spawn_timer = self.scaled_hostile_respawn_base(NETHER_RESPAWN_BASE);
        self.end_spawn_timer = self.scaled_hostile_respawn_base(END_RESPAWN_BASE);
    }

    fn clear_hostile_entities(&mut self) {
        self.zombies.clear();
        self.creepers.clear();
        self.skeletons.clear();
        self.spiders.clear();
        self.silverfish.clear();
        self.slimes.clear();
        self.endermen.clear();
        self.blazes.clear();
        self.pigmen.clear();
        self.ghasts.clear();
        self.fireballs.clear();
        self.arrows.retain(|arrow| arrow.from_player);
    }

    fn apply_peaceful_cleanup_if_needed(&mut self) {
        if self.difficulty == Difficulty::Peaceful {
            self.clear_hostile_entities();
        }
    }

    pub fn credits_scroll_row(&self) -> i32 {
        (self.credits_tick / CREDITS_SCROLL_TICKS_PER_ROW) as i32
    }

    pub fn end_victory_sequence_state(&self) -> Option<(u16, f64, f64)> {
        let (origin_x, origin_y) = self.end_victory_origin?;
        (self.end_victory_ticks > 0).then_some((self.end_victory_ticks, origin_x, origin_y))
    }

    pub(crate) fn set_replicated_end_victory(&mut self, victory: Option<(u16, f64, f64)>) {
        self.end_victory_ticks = victory.map_or(0, |v| v.0);
        self.end_victory_origin = victory.map(|v| (v.1, v.2));
    }

    pub fn has_defeated_dragon(&self) -> bool {
        self.dragon_defeated
    }

    pub fn has_seen_completion_credits(&self) -> bool {
        self.completion_credits_seen
    }

    pub fn skip_completion_credits(&mut self) {
        if self.credits_active {
            self.finish_completion_credits();
        }
    }

    fn start_end_victory_sequence(&mut self, origin_x: f64, origin_y: f64) {
        self.end_victory_ticks = END_VICTORY_SEQUENCE_TICKS;
        self.end_victory_origin = Some((origin_x, origin_y));
    }

    fn update_end_victory_sequence(&mut self) {
        if self.end_victory_ticks == 0 {
            self.end_victory_origin = None;
            return;
        }
        self.end_victory_ticks -= 1;
        if self.end_victory_ticks == 0 {
            self.end_victory_origin = None;
        }
    }

    pub fn queue_jump(&mut self) {
        self.jump_buffer_ticks = self.jump_buffer_ticks.max(3);
    }

    fn stop_sprinting(&mut self) {
        self.sprinting = false;
        self.sprint_direction = 0;
    }

    fn maybe_start_sprint_from_double_tap(&mut self, direction: i8) {
        if self.player.sneaking
            || self.inventory_open
            || self.settings_menu_open
            || self.player.hunger <= SPRINT_MIN_HUNGER
        {
            return;
        }
        let (same_dir_window, opposite_dir_window) = if direction < 0 {
            (
                &mut self.sprint_left_tap_ticks,
                &mut self.sprint_right_tap_ticks,
            )
        } else {
            (
                &mut self.sprint_right_tap_ticks,
                &mut self.sprint_left_tap_ticks,
            )
        };
        if *same_dir_window > 0 {
            self.sprinting = true;
            self.sprint_direction = direction;
            *same_dir_window = 0;
            *opposite_dir_window = 0;
        } else {
            *same_dir_window = SPRINT_DOUBLE_TAP_WINDOW_TICKS;
            *opposite_dir_window = 0;
        }
    }

    fn sync_sneak_state(&mut self) {
        self.player.sneaking = self.sneak_toggled || self.sneak_held;
        if self.player.sneaking {
            self.stop_sprinting();
        }
    }

    fn active_crafting_grid_size(&self) -> Option<usize> {
        if !self.inventory_open
            || self.at_chest
            || self.at_furnace
            || self.at_enchanting_table
            || self.at_anvil
            || self.at_brewing_stand
        {
            return None;
        }
        if self.at_crafting_table {
            Some(3)
        } else {
            Some(2)
        }
    }

    fn is_valid_active_crafting_cell(&self, cell_idx: usize) -> bool {
        let Some(grid_size) = self.active_crafting_grid_size() else {
            return false;
        };
        if cell_idx >= CRAFT_GRID_UI_SLOTS {
            return false;
        }
        let x = cell_idx % 3;
        let y = cell_idx / 3;
        x < grid_size && y < grid_size
    }

    fn spill_single_item_near_player(&mut self, item_type: ItemType) {
        self.item_entities.push(ItemEntity::new(
            self.player.x,
            self.player.y - 0.5,
            item_type,
        ));
    }

    fn flush_crafting_grid_items_to_inventory(&mut self) {
        let mut buffered = Vec::new();
        for cell in &mut self.crafting_grid {
            let Some(stack) = cell.take() else {
                continue;
            };
            buffered.push(stack);
        }
        for stack in buffered {
            let overflow = self.inventory.add_item(stack.item_type, stack.count);
            for _ in 0..overflow {
                self.spill_single_item_near_player(stack.item_type);
            }
        }
    }

    pub fn clear_active_crafting_grid(&mut self) {
        if self.active_crafting_grid_size().is_some() {
            let mut after = self.inventory.clone();
            for stack in self.crafting_grid.iter().flatten() {
                if after.add_item(stack.item_type, stack.count) > 0 {
                    return;
                }
            }
            self.inventory = after;
            self.crafting_grid.fill(None);
            self.selected_inventory_slot = None;
        }
    }

    fn clear_container_interaction_state(&mut self) {
        self.clear_bound_furnace_view();
        self.flush_crafting_grid_items_to_inventory();
        self.inventory_open = false;
        self.at_crafting_table = false;
        self.at_furnace = false;
        self.at_chest = false;
        self.at_enchanting_table = false;
        self.at_anvil = false;
        self.at_brewing_stand = false;
        self.open_chest_pos = None;
        self.selected_inventory_slot = None;
        self.jump_held = false;
        self.jump_buffer_ticks = 0;
        self.stop_sprinting();
    }

    fn clear_live_input_for_container_open(&mut self) {
        self.clear_bound_furnace_view();
        self.left_click_down = false;
        self.moving_left = false;
        self.moving_right = false;
        self.jump_held = false;
        self.jump_buffer_ticks = 0;
        self.stop_sprinting();
    }

    fn open_player_inventory_view(&mut self) {
        self.clear_live_input_for_container_open();
        self.inventory_open = true;
        self.at_crafting_table = false;
        self.at_furnace = false;
        self.at_chest = false;
        self.at_enchanting_table = false;
        self.at_anvil = false;
        self.at_brewing_stand = false;
        self.open_chest_pos = None;
        self.selected_inventory_slot = None;
    }

    fn open_crafting_inventory_view(&mut self) {
        self.clear_live_input_for_container_open();
        self.inventory_open = true;
        self.at_crafting_table = true;
        self.at_furnace = false;
        self.at_chest = false;
        self.at_enchanting_table = false;
        self.at_anvil = false;
        self.at_brewing_stand = false;
        self.open_chest_pos = None;
        self.selected_inventory_slot = None;
    }

    fn open_furnace_inventory_view(&mut self) {
        self.flush_crafting_grid_items_to_inventory();
        self.clear_live_input_for_container_open();
        self.inventory_open = true;
        self.at_furnace = true;
        self.furnace_explicit = true;
        self.at_crafting_table = false;
        self.at_chest = false;
        self.at_enchanting_table = false;
        self.at_anvil = false;
        self.at_brewing_stand = false;
        self.open_chest_pos = None;
        self.selected_inventory_slot = None;
    }

    fn open_enchanting_inventory_view(&mut self) {
        self.flush_crafting_grid_items_to_inventory();
        self.clear_live_input_for_container_open();
        self.inventory_open = true;
        self.at_furnace = false;
        self.at_crafting_table = false;
        self.at_chest = false;
        self.at_enchanting_table = true;
        self.at_anvil = false;
        self.at_brewing_stand = false;
        self.open_chest_pos = None;
        self.selected_inventory_slot = None;
    }

    fn open_anvil_inventory_view(&mut self) {
        self.flush_crafting_grid_items_to_inventory();
        self.clear_live_input_for_container_open();
        self.inventory_open = true;
        self.at_furnace = false;
        self.at_crafting_table = false;
        self.at_chest = false;
        self.at_enchanting_table = false;
        self.at_anvil = true;
        self.at_brewing_stand = false;
        self.open_chest_pos = None;
        self.selected_inventory_slot = None;
    }

    fn open_brewing_inventory_view(&mut self) {
        self.flush_crafting_grid_items_to_inventory();
        self.clear_live_input_for_container_open();
        self.inventory_open = true;
        self.at_furnace = false;
        self.at_crafting_table = false;
        self.at_chest = false;
        self.at_enchanting_table = false;
        self.at_anvil = false;
        self.at_brewing_stand = true;
        self.open_chest_pos = None;
        self.selected_inventory_slot = None;
    }

    fn open_chest_inventory_view(&mut self, bx: i32, by: i32) {
        self.flush_crafting_grid_items_to_inventory();
        if self
            .world
            .ensure_chest_inventory(bx, by, CHEST_INVENTORY_CAPACITY)
            .is_none()
        {
            return;
        }
        self.clear_live_input_for_container_open();
        self.inventory_open = true;
        self.at_chest = true;
        self.at_crafting_table = false;
        self.at_furnace = false;
        self.at_enchanting_table = false;
        self.at_anvil = false;
        self.at_brewing_stand = false;
        self.open_chest_pos = Some((bx, by));
        self.selected_inventory_slot = None;
    }

    fn ensure_chest_ui_state_valid(&mut self) -> bool {
        if !self.at_chest {
            return true;
        }
        if !self.inventory_open {
            self.clear_container_interaction_state();
            return false;
        }
        let Some((bx, by)) = self.open_chest_pos else {
            self.clear_container_interaction_state();
            return false;
        };
        if self.world.get_block(bx, by) != BlockType::Chest {
            self.clear_container_interaction_state();
            return false;
        }
        if self.world.chest_inventory(bx, by).is_none()
            && self
                .world
                .ensure_chest_inventory(bx, by, CHEST_INVENTORY_CAPACITY)
                .is_none()
        {
            self.clear_container_interaction_state();
            return false;
        }
        true
    }

    fn close_chest_if_out_of_range(&mut self) {
        if self.at_furnace
            && self.inventory_open
            && let Some((x, y)) = self.open_chest_pos
        {
            let dx = self.player.x - (x as f64 + 0.5);
            let dy = self.player.y - 1.0 - (y as f64 + 0.5);
            if self.world.get_block(x, y) != BlockType::Furnace
                || dx * dx + dy * dy > 4.8 * 4.8
                || !self.can_see_block(self.player.x, self.player.y - 1.0, x, y)
            {
                self.clear_container_interaction_state();
            }
            return;
        }
        if !self.at_chest || !self.inventory_open {
            return;
        }
        if !self.ensure_chest_ui_state_valid() {
            return;
        }
        let Some((bx, by)) = self.open_chest_pos else {
            self.clear_container_interaction_state();
            return;
        };
        let px = self.player.x;
        let py = self.player.y - 1.0;
        let dx = px - (bx as f64 + 0.5);
        let dy = py - (by as f64 + 0.5);
        let dist = (dx * dx + dy * dy).sqrt();
        if dist > 4.8 || !self.can_see_block(px, py, bx, by) {
            self.clear_container_interaction_state();
        }
    }

    pub fn has_personal_armor_ui(&self) -> bool {
        !self.at_chest
            && !self.at_furnace
            && !self.at_enchanting_table
            && !self.at_anvil
            && !self.at_brewing_stand
            && !self.at_crafting_table
    }

    fn armor_ui_slot_index(&self, ui_slot: usize) -> Option<usize> {
        if self.has_personal_armor_ui()
            && (ARMOR_UI_OFFSET..(ARMOR_UI_OFFSET + ARMOR_UI_SLOTS)).contains(&ui_slot)
        {
            return Some(ui_slot - ARMOR_UI_OFFSET);
        }
        None
    }

    fn can_place_stack_in_ui_slot(&self, ui_slot: usize, stack: &ItemStack) -> bool {
        if let Some(index) = self.furnace_ui_index(ui_slot) {
            return stack.durability.is_none()
                && match index {
                    0 => Self::furnace_job_for_input_item(stack.item_type).is_some(),
                    1 => Self::furnace_fuel_ticks(stack.item_type) > 0,
                    _ => false,
                };
        }
        if self.is_crafting_grid_ui_slot(ui_slot)
            && self.is_valid_active_crafting_cell(ui_slot - CRAFT_GRID_UI_OFFSET)
        {
            return stack.durability.is_none() && !Self::can_item_be_enchanted(stack.item_type);
        }
        if self.at_chest {
            return ui_slot < CHEST_INVENTORY_CAPACITY + PLAYER_INVENTORY_CAPACITY;
        }
        if let Some(armor_slot_idx) = self.armor_ui_slot_index(ui_slot) {
            return stack.item_type.armor_slot_index() == Some(armor_slot_idx) && stack.count == 1;
        }
        ui_slot < PLAYER_INVENTORY_CAPACITY
    }

    pub(crate) fn open_container_slots(&self) -> Option<&[Option<ItemStack>]> {
        self.open_chest_pos
            .and_then(|(x, y)| self.world.chest_inventory(x, y))
            .map(|c| c.slots.as_slice())
    }
    pub(crate) fn active_ui_slot_count(&self) -> usize {
        if self.at_furnace && self.furnace_explicit {
            return FURNACE_OUTPUT_UI_SLOT + 1;
        }
        if self.at_chest {
            CHEST_INVENTORY_CAPACITY + PLAYER_INVENTORY_CAPACITY
        } else if self.has_personal_armor_ui() {
            PLAYER_INVENTORY_CAPACITY + ARMOR_UI_SLOTS
        } else {
            PLAYER_INVENTORY_CAPACITY
        }
    }

    fn ui_slot_item_clone(&self, ui_slot: usize) -> Option<Option<ItemStack>> {
        if let Some(index) = self.furnace_ui_index(ui_slot) {
            return Some(self.furnace_slot_stack(index).cloned());
        }
        if self.at_chest {
            let (bx, by) = self.open_chest_pos?;
            if ui_slot < CHEST_INVENTORY_CAPACITY {
                let chest = self.world.chest_inventory(bx, by)?;
                return chest.slots.get(ui_slot).cloned();
            }
            let player_slot = ui_slot.checked_sub(CHEST_INVENTORY_CAPACITY)?;
            return self.inventory.slots.get(player_slot).cloned();
        }
        if ui_slot < PLAYER_INVENTORY_CAPACITY {
            return self.inventory.slots.get(ui_slot).cloned();
        }
        let armor_slot_idx = self.armor_ui_slot_index(ui_slot)?;
        self.armor_slots.get(armor_slot_idx).cloned()
    }

    fn set_ui_slot_item(&mut self, ui_slot: usize, stack: Option<ItemStack>) -> bool {
        if let Some(index) = self.furnace_ui_index(ui_slot) {
            self.refresh_furnace_view();
            if let Some(item) = &stack {
                if index == 2 {
                    if !self.furnace_slots[2].as_ref().is_some_and(|old| {
                        old.item_type == item.item_type && item.count <= old.count
                    }) {
                        return false;
                    }
                } else if !self.can_place_stack_in_ui_slot(ui_slot, item) {
                    return false;
                }
            }
            self.furnace_slots[index] = stack;
            self.commit_furnace_view();
            return true;
        }
        if self.at_chest {
            let Some((bx, by)) = self.open_chest_pos else {
                return false;
            };
            if ui_slot < CHEST_INVENTORY_CAPACITY {
                if let Some(chest) = self.world.chest_inventory_mut(bx, by)
                    && let Some(slot) = chest.slots.get_mut(ui_slot)
                {
                    *slot = stack;
                    return true;
                }
                return false;
            }
            let Some(player_slot) = ui_slot.checked_sub(CHEST_INVENTORY_CAPACITY) else {
                return false;
            };
            if let Some(slot) = self.inventory.slots.get_mut(player_slot) {
                *slot = stack;
                if slot.is_none() {
                    self.inventory_enchant_levels[player_slot] = 0;
                }
                return true;
            }
            return false;
        }
        if ui_slot < PLAYER_INVENTORY_CAPACITY {
            if let Some(slot) = self.inventory.slots.get_mut(ui_slot) {
                *slot = stack;
                if slot.is_none() {
                    self.inventory_enchant_levels[ui_slot] = 0;
                }
                return true;
            }
            return false;
        }
        let Some(armor_slot_idx) = self.armor_ui_slot_index(ui_slot) else {
            return false;
        };
        if let Some(stack) = stack.as_ref()
            && !self.can_place_stack_in_ui_slot(ui_slot, stack)
        {
            return false;
        }
        if let Some(slot) = self.armor_slots.get_mut(armor_slot_idx) {
            *slot = stack;
            if slot.is_none() {
                self.armor_enchant_levels[armor_slot_idx] = 0;
            }
            return true;
        }
        false
    }

    fn ui_slot_enchant_level(&self, ui_slot: usize) -> Option<u8> {
        if self.at_chest {
            if ui_slot < CHEST_INVENTORY_CAPACITY {
                return Some(0);
            }
            let player_slot = ui_slot.checked_sub(CHEST_INVENTORY_CAPACITY)?;
            return self.inventory_enchant_levels.get(player_slot).copied();
        }
        if ui_slot < PLAYER_INVENTORY_CAPACITY {
            return self.inventory_enchant_levels.get(ui_slot).copied();
        }
        let armor_slot_idx = self.armor_ui_slot_index(ui_slot)?;
        self.armor_enchant_levels.get(armor_slot_idx).copied()
    }

    fn set_ui_slot_enchant_level(&mut self, ui_slot: usize, level: u8) -> bool {
        let level = level.min(ENCHANT_MAX_LEVEL);
        if self.at_chest {
            if ui_slot < CHEST_INVENTORY_CAPACITY {
                return true;
            }
            let Some(player_slot) = ui_slot.checked_sub(CHEST_INVENTORY_CAPACITY) else {
                return false;
            };
            if let Some(slot_level) = self.inventory_enchant_levels.get_mut(player_slot) {
                *slot_level = level;
                return true;
            }
            return false;
        }
        if ui_slot < PLAYER_INVENTORY_CAPACITY {
            if let Some(slot_level) = self.inventory_enchant_levels.get_mut(ui_slot) {
                *slot_level = level;
                return true;
            }
            return false;
        }
        let Some(armor_slot_idx) = self.armor_ui_slot_index(ui_slot) else {
            return false;
        };
        if let Some(slot_level) = self.armor_enchant_levels.get_mut(armor_slot_idx) {
            *slot_level = level;
            return true;
        }
        false
    }

    fn set_hotbar_index_from_ui_slot(&mut self, ui_slot: usize) {
        if self.at_chest {
            let Some(player_slot) = ui_slot.checked_sub(CHEST_INVENTORY_CAPACITY) else {
                return;
            };
            if player_slot < 9 {
                self.hotbar_index = player_slot as u8;
            }
            return;
        }
        if ui_slot < 9 {
            self.hotbar_index = ui_slot as u8;
        }
    }

    fn is_crafting_grid_ui_slot(&self, slot_idx: usize) -> bool {
        (CRAFT_GRID_UI_OFFSET..(CRAFT_GRID_UI_OFFSET + CRAFT_GRID_UI_SLOTS)).contains(&slot_idx)
    }

    fn any_slot_item_clone(&self, slot_idx: usize) -> Option<Option<ItemStack>> {
        let crafting_active = self.active_crafting_grid_size().is_some();
        if crafting_active && slot_idx == CRAFT_OUTPUT_UI_SLOT {
            return Some(None);
        }
        if crafting_active && self.is_crafting_grid_ui_slot(slot_idx) {
            let cell_idx = slot_idx - CRAFT_GRID_UI_OFFSET;
            if !self.is_valid_active_crafting_cell(cell_idx) {
                return None;
            }
            return Some(self.crafting_grid[cell_idx].clone());
        }
        if slot_idx < self.active_ui_slot_count() {
            return self.ui_slot_item_clone(slot_idx);
        }
        None
    }

    pub fn inventory_ui_slot_preview_item(&self, slot_idx: usize) -> Option<ItemStack> {
        if self.active_crafting_grid_size().is_some() && slot_idx == CRAFT_OUTPUT_UI_SLOT {
            return self
                .crafting_output_preview()
                .map(|(item_type, count)| ItemStack {
                    item_type,
                    count,
                    durability: None,
                });
        }
        self.any_slot_item_clone(slot_idx).flatten()
    }

    pub fn selected_inventory_preview_item(&self) -> Option<ItemStack> {
        self.selected_inventory_slot
            .and_then(|slot_idx| self.inventory_ui_slot_preview_item(slot_idx))
    }

    fn set_any_slot_item(&mut self, slot_idx: usize, stack: Option<ItemStack>) -> bool {
        let crafting_active = self.active_crafting_grid_size().is_some();
        if crafting_active && slot_idx == CRAFT_OUTPUT_UI_SLOT {
            return false;
        }
        if crafting_active && self.is_crafting_grid_ui_slot(slot_idx) {
            let cell_idx = slot_idx - CRAFT_GRID_UI_OFFSET;
            if !self.is_valid_active_crafting_cell(cell_idx) {
                return false;
            }
            match stack {
                Some(stack) => {
                    if stack.count == 0
                        || stack.durability.is_some()
                        || Self::can_item_be_enchanted(stack.item_type)
                        || stack.count > stack.item_type.max_stack_size()
                    {
                        return false;
                    }
                    self.crafting_grid[cell_idx] = Some(stack);
                }
                None => self.crafting_grid[cell_idx] = None,
            }
            return true;
        }
        if slot_idx < self.active_ui_slot_count() {
            return self.set_ui_slot_item(slot_idx, stack);
        }
        false
    }

    fn split_range_for_slot(&self, slot_idx: usize) -> Option<std::ops::Range<usize>> {
        if self.furnace_ui_index(slot_idx).is_some() {
            return Some(0..PLAYER_INVENTORY_CAPACITY);
        }
        if self.is_crafting_grid_ui_slot(slot_idx) && self.active_crafting_grid_size().is_some() {
            return Some(0..PLAYER_INVENTORY_CAPACITY);
        }
        if self.at_chest {
            if slot_idx < CHEST_INVENTORY_CAPACITY {
                return Some(0..CHEST_INVENTORY_CAPACITY);
            }
            if slot_idx < CHEST_INVENTORY_CAPACITY + PLAYER_INVENTORY_CAPACITY {
                return Some(
                    CHEST_INVENTORY_CAPACITY
                        ..(CHEST_INVENTORY_CAPACITY + PLAYER_INVENTORY_CAPACITY),
                );
            }
            return None;
        }
        if self.armor_ui_slot_index(slot_idx).is_some() {
            return Some(0..PLAYER_INVENTORY_CAPACITY);
        }
        if slot_idx < PLAYER_INVENTORY_CAPACITY {
            return Some(0..PLAYER_INVENTORY_CAPACITY);
        }
        None
    }

    fn shift_transfer_target_slots(&self, source_slot: usize) -> Option<Vec<usize>> {
        let crafting_active = self.active_crafting_grid_size().is_some();
        if crafting_active && source_slot == CRAFT_OUTPUT_UI_SLOT {
            return None;
        }

        if self.at_chest {
            if source_slot < CHEST_INVENTORY_CAPACITY {
                let player_offset = CHEST_INVENTORY_CAPACITY;
                let mut targets = Vec::with_capacity(PLAYER_INVENTORY_CAPACITY);
                targets.extend(
                    (player_offset + PLAYER_HOTBAR_SLOTS)
                        ..(player_offset + PLAYER_INVENTORY_CAPACITY),
                );
                targets.extend(player_offset..(player_offset + PLAYER_HOTBAR_SLOTS));
                return Some(targets);
            }
            if source_slot < CHEST_INVENTORY_CAPACITY + PLAYER_INVENTORY_CAPACITY {
                return Some((0..CHEST_INVENTORY_CAPACITY).collect());
            }
            return None;
        }

        if self.armor_ui_slot_index(source_slot).is_some() {
            let mut targets = Vec::with_capacity(PLAYER_INVENTORY_CAPACITY);
            targets.extend(PLAYER_HOTBAR_SLOTS..PLAYER_INVENTORY_CAPACITY);
            targets.extend(0..PLAYER_HOTBAR_SLOTS);
            return Some(targets);
        }
        if source_slot < PLAYER_HOTBAR_SLOTS {
            return Some((PLAYER_HOTBAR_SLOTS..PLAYER_INVENTORY_CAPACITY).collect());
        }
        if source_slot < PLAYER_INVENTORY_CAPACITY {
            return Some((0..PLAYER_HOTBAR_SLOTS).collect());
        }
        if crafting_active && self.is_crafting_grid_ui_slot(source_slot) {
            let mut targets = Vec::with_capacity(PLAYER_INVENTORY_CAPACITY);
            targets.extend(PLAYER_HOTBAR_SLOTS..PLAYER_INVENTORY_CAPACITY);
            targets.extend(0..PLAYER_HOTBAR_SLOTS);
            return Some(targets);
        }
        None
    }

    fn transfer_full_stack_between_slots(
        &mut self,
        source_slot: usize,
        target_slots: &[usize],
    ) -> bool {
        if self.active_crafting_grid_size().is_some() && source_slot == CRAFT_OUTPUT_UI_SLOT {
            return false;
        }
        let Some(Some(mut source_stack)) = self.any_slot_item_clone(source_slot) else {
            return false;
        };
        if source_stack.count == 0 {
            return false;
        }

        let source_is_ui_slot = source_slot < self.active_ui_slot_count();
        let source_enchant = if source_is_ui_slot {
            self.ui_slot_enchant_level(source_slot).unwrap_or(0)
        } else {
            0
        };
        let stack_limit = if source_stack.durability.is_some() {
            1
        } else {
            source_stack.item_type.max_stack_size()
        };
        let mut moved_any = false;
        let mut last_target_slot = None;

        if source_stack.durability.is_none() && stack_limit > 1 {
            for target_slot in target_slots {
                if source_stack.count == 0 {
                    break;
                }
                if *target_slot == source_slot {
                    continue;
                }
                let Some(target_stack_opt) = self.any_slot_item_clone(*target_slot) else {
                    continue;
                };
                let Some(mut target_stack) = target_stack_opt else {
                    continue;
                };
                if target_stack.item_type != source_stack.item_type
                    || target_stack.durability.is_some()
                    || target_stack.count >= stack_limit
                {
                    continue;
                }
                let add = source_stack
                    .count
                    .min(stack_limit.saturating_sub(target_stack.count));
                if add == 0 {
                    continue;
                }
                target_stack.count += add;
                if !self.set_any_slot_item(*target_slot, Some(target_stack)) {
                    continue;
                }
                source_stack.count -= add;
                moved_any = true;
                last_target_slot = Some(*target_slot);
            }
        }

        for target_slot in target_slots {
            if source_stack.count == 0 {
                break;
            }
            if *target_slot == source_slot {
                continue;
            }
            let Some(target_stack_opt) = self.any_slot_item_clone(*target_slot) else {
                continue;
            };
            if target_stack_opt.is_some() {
                continue;
            }
            let moving_count = source_stack.count.min(stack_limit);
            if moving_count == 0 {
                continue;
            }
            let moving_stack = ItemStack {
                item_type: source_stack.item_type,
                count: moving_count,
                durability: source_stack.durability,
            };
            if !self.set_any_slot_item(*target_slot, Some(moving_stack)) {
                continue;
            }
            source_stack.count -= moving_count;
            moved_any = true;
            last_target_slot = Some(*target_slot);
        }

        if !moved_any {
            return false;
        }

        let source_became_empty = source_stack.count == 0;
        let source_after = if source_became_empty {
            None
        } else {
            Some(source_stack)
        };
        if !self.set_any_slot_item(source_slot, source_after) {
            return false;
        }
        if source_became_empty && source_enchant > 0 && source_is_ui_slot {
            let _ = self.set_ui_slot_enchant_level(source_slot, 0);
            if let Some(target_slot) = last_target_slot
                && target_slot < self.active_ui_slot_count()
            {
                let _ = self.set_ui_slot_enchant_level(target_slot, source_enchant);
            }
        }
        true
    }

    fn split_selected_stack_into_empty_slot(&mut self) -> bool {
        let Some(source_slot) = self.selected_inventory_slot else {
            return false;
        };
        let Some(range) = self.split_range_for_slot(source_slot) else {
            return false;
        };
        let Some(Some(source_stack)) = self.any_slot_item_clone(source_slot) else {
            return false;
        };
        if source_stack.count < 2 || source_stack.durability.is_some() {
            return false;
        }
        let Some(target_slot) = range
            .filter(|idx| *idx != source_slot)
            .find(|idx| self.any_slot_item_clone(*idx).is_some_and(|v| v.is_none()))
        else {
            return false;
        };
        let left = source_stack.count.div_ceil(2);
        let right = source_stack.count.saturating_sub(left);
        if right == 0 {
            return false;
        }
        let mut left_stack = source_stack.clone();
        left_stack.count = left;
        let mut right_stack = source_stack;
        right_stack.count = right;
        if !self.set_any_slot_item(source_slot, Some(left_stack)) {
            return false;
        }
        if !self.set_any_slot_item(target_slot, Some(right_stack)) {
            return false;
        }
        true
    }

    fn transfer_single_between_slots(&mut self, source_slot: usize, target_slot: usize) -> bool {
        let crafting_active = self.active_crafting_grid_size().is_some();
        if source_slot == target_slot
            || (crafting_active
                && (source_slot == CRAFT_OUTPUT_UI_SLOT || target_slot == CRAFT_OUTPUT_UI_SLOT))
        {
            return false;
        }
        let Some(Some(mut source_stack)) = self.any_slot_item_clone(source_slot) else {
            return false;
        };
        if source_stack.count == 0 {
            return false;
        }
        if !self.can_place_stack_in_ui_slot(target_slot, &source_stack) {
            return false;
        }
        let source_is_ui_slot = source_slot < self.active_ui_slot_count();
        let target_is_ui_slot = target_slot < self.active_ui_slot_count();
        let source_enchant = if source_is_ui_slot {
            self.ui_slot_enchant_level(source_slot).unwrap_or(0)
        } else {
            0
        };
        if crafting_active
            && self.is_crafting_grid_ui_slot(target_slot)
            && (source_stack.durability.is_some()
                || Self::can_item_be_enchanted(source_stack.item_type))
        {
            return false;
        }
        let Some(target_stack_opt) = self.any_slot_item_clone(target_slot) else {
            return false;
        };
        if let Some(mut target_stack) = target_stack_opt {
            if source_stack.item_type != target_stack.item_type
                || source_stack.durability.is_some()
                || target_stack.durability.is_some()
                || target_stack.count >= source_stack.item_type.max_stack_size()
            {
                return false;
            }
            target_stack.count += 1;
            source_stack.count -= 1;
            let source_after = if source_stack.count == 0 {
                None
            } else {
                Some(source_stack)
            };
            self.set_any_slot_item(source_slot, source_after)
                && self.set_any_slot_item(target_slot, Some(target_stack))
        } else {
            let moving_stack = ItemStack {
                item_type: source_stack.item_type,
                count: 1,
                durability: source_stack.durability,
            };
            source_stack.count -= 1;
            let source_after = if source_stack.count == 0 {
                None
            } else {
                Some(source_stack)
            };
            let source_became_empty = source_after.is_none();
            if !self.set_any_slot_item(target_slot, Some(moving_stack)) {
                return false;
            }
            if !self.set_any_slot_item(source_slot, source_after) {
                let _ = self.set_any_slot_item(target_slot, None);
                return false;
            }
            if source_became_empty && source_enchant > 0 && source_is_ui_slot && target_is_ui_slot {
                let _ = self.set_ui_slot_enchant_level(source_slot, 0);
                let _ = self.set_ui_slot_enchant_level(target_slot, source_enchant);
            }
            true
        }
    }

    pub fn handle_inventory_shift_click(&mut self, slot_idx: usize) {
        if !self.inventory_open {
            return;
        }
        if self.at_chest && !self.ensure_chest_ui_state_valid() {
            return;
        }

        if self.quick_start_furnace_from_shift_click(slot_idx) {
            self.selected_inventory_slot = None;
            self.set_hotbar_index_from_ui_slot(slot_idx);
            return;
        }

        if slot_idx == CRAFT_OUTPUT_UI_SLOT && self.active_crafting_grid_size().is_some() {
            self.attempt_craft_from_grid_max();
            return;
        }

        if slot_idx >= self.active_ui_slot_count()
            && !(self.is_crafting_grid_ui_slot(slot_idx)
                && self.active_crafting_grid_size().is_some())
        {
            return;
        }

        let Some(target_slots) = self.shift_transfer_target_slots(slot_idx) else {
            return;
        };
        if self.transfer_full_stack_between_slots(slot_idx, &target_slots) {
            self.selected_inventory_slot = None;
            self.set_hotbar_index_from_ui_slot(slot_idx);
        }
    }

    pub fn handle_inventory_drag_place(&mut self, slot_idx: usize) {
        if !self.inventory_open {
            return;
        }
        if self.at_chest && !self.ensure_chest_ui_state_valid() {
            return;
        }

        let Some(source_slot) = self.selected_inventory_slot else {
            return;
        };
        if source_slot == slot_idx {
            return;
        }
        if slot_idx >= self.active_ui_slot_count()
            && !(self.is_crafting_grid_ui_slot(slot_idx)
                && self.active_crafting_grid_size().is_some())
        {
            return;
        }

        if self.transfer_single_between_slots(source_slot, slot_idx) {
            if slot_idx < self.active_ui_slot_count() {
                self.set_hotbar_index_from_ui_slot(slot_idx);
            }
            if self
                .any_slot_item_clone(source_slot)
                .is_some_and(|stack| stack.is_none())
            {
                self.selected_inventory_slot = None;
            }
        }
    }

    fn close_settings_menu(&mut self) {
        self.settings_menu_open = false;
    }

    fn open_settings_menu(&mut self) {
        if self.credits_active || self.death_screen_active {
            return;
        }
        self.settings_menu_open = true;
        self.settings_menu_selected = 0;
        self.clear_container_interaction_state();
        self.left_click_down = false;
        self.moving_left = false;
        self.moving_right = false;
        self.bow_draw_ticks = 0;
        self.bow_draw_active = false;
        self.player.vx = 0.0;
        self.stop_sprinting();
    }

    fn toggle_settings_menu(&mut self) {
        if self.settings_menu_open {
            self.close_settings_menu();
        } else {
            self.open_settings_menu();
        }
    }

    fn move_settings_menu_selection(&mut self, delta: i8) {
        if !self.settings_menu_open {
            return;
        }
        let next = (self.settings_menu_selected as i16 + delta as i16)
            .rem_euclid(self.settings_rows().len() as i16);
        self.settings_menu_selected = next as u8;
    }

    fn apply_settings_menu_selection(&mut self) {
        if !self.settings_menu_open {
            return;
        }
        if let Some(command) = self.settings_selected_command() {
            self.apply_client_command(command);
        } else {
            self.close_settings_menu();
        }
    }

    pub fn apply_client_command(&mut self, command: ClientCommand) {
        self.wake_for_command(command);
        match command {
            ClientCommand::TogglePvp => self.pvp_enabled = !self.pvp_enabled,
            ClientCommand::ToggleShortcuts => {
                if self.shortcuts_unlocked() {
                    self.toggle_shortcuts();
                }
            }
            ClientCommand::DropItem(stack) => {
                self.drop_selected_item(stack);
            }
            ClientCommand::AimAt(_, _) => {}
            ClientCommand::CraftRecipe(index) => self.attempt_craft(index as usize),
            ClientCommand::CraftRecipeMax(index) => self.attempt_craft_max(index as usize),
            ClientCommand::Enchant(index) => self.attempt_enchant_option(index as usize),
            ClientCommand::Anvil => self.attempt_anvil_combine(),
            ClientCommand::Brew(index) => self.attempt_brew_option(index as usize),
            ClientCommand::InventoryClick(slot, kind) => match kind {
                0 => self.handle_inventory_click(slot as usize),
                1 => self.handle_inventory_right_click(slot as usize),
                2 => self.handle_inventory_shift_click(slot as usize),
                3 => self.handle_inventory_drag_place(slot as usize),
                _ => {}
            },
            ClientCommand::CraftGrid(max) => {
                if self.inventory_open {
                    if max {
                        self.attempt_craft_from_grid_max();
                    } else {
                        self.attempt_craft_from_grid();
                    }
                }
            }
            ClientCommand::ClearCraftGrid => {
                if self.inventory_open {
                    self.clear_active_crafting_grid();
                }
            }
            ClientCommand::QueueJump => {
                if !self.inventory_open && !self.settings_menu_open {
                    self.queue_jump();
                }
            }
            ClientCommand::SetJumpHeld(held) => {
                // Releasing a real held jump gives a short, deliberate hop. Keep
                // swimming, ladder climbing, falling and duplicate releases intact.
                if self.jump_held
                    && !held
                    && self.player.vy < -0.16
                    && !self.is_player_on_ladder()
                    && !self.entity_touches_fluid(
                        self.player.x,
                        self.player.y,
                        PLAYER_HALF_WIDTH,
                        PLAYER_HEIGHT,
                    )
                {
                    self.player.vy = -0.16;
                }
                self.jump_held = held && !self.inventory_open && !self.settings_menu_open;
            }
            ClientCommand::CycleMovementProfile => self.cycle_movement_profile(),
            ClientCommand::CycleDifficulty => self.cycle_difficulty(),
            ClientCommand::CycleGameRulesPreset => self.cycle_game_rules_preset(),
            ClientCommand::ToggleRuleMobSpawning => self.toggle_rule_mob_spawning(),
            ClientCommand::ToggleRuleDaylightCycle => self.toggle_rule_daylight_cycle(),
            ClientCommand::ToggleRuleWeatherCycle => self.toggle_rule_weather_cycle(),
            ClientCommand::ToggleRuleKeepInventory => self.toggle_rule_keep_inventory(),
            ClientCommand::ToggleSettingsMenu => {
                self.toggle_settings_menu();
                if self.settings_menu_open {
                    self.jump_held = false;
                    self.jump_buffer_ticks = 0;
                }
            }
            ClientCommand::SettingsMoveUp => self.move_settings_menu_selection(-1),
            ClientCommand::SettingsMoveDown => self.move_settings_menu_selection(1),
            ClientCommand::SettingsApply => self.apply_settings_menu_selection(),
            ClientCommand::SetMoveLeft(active) => {
                if self.settings_menu_open {
                    self.moving_left = false;
                    self.moving_right = false;
                    self.stop_sprinting();
                    return;
                }
                let was_active = self.moving_left;
                self.moving_left = active;
                if active {
                    self.moving_right = false;
                    if self.sprint_direction == 1 {
                        self.stop_sprinting();
                    }
                    if !was_active {
                        self.maybe_start_sprint_from_double_tap(-1);
                    }
                } else if self.sprint_direction == -1 {
                    self.stop_sprinting();
                }
                if was_active && !active && !self.moving_left && !self.moving_right {
                    self.brake_released_movement();
                }
            }
            ClientCommand::SetMoveRight(active) => {
                if self.settings_menu_open {
                    self.moving_left = false;
                    self.moving_right = false;
                    self.stop_sprinting();
                    return;
                }
                let was_active = self.moving_right;
                self.moving_right = active;
                if active {
                    self.moving_left = false;
                    if self.sprint_direction == -1 {
                        self.stop_sprinting();
                    }
                    if !was_active {
                        self.maybe_start_sprint_from_double_tap(1);
                    }
                } else if self.sprint_direction == 1 {
                    self.stop_sprinting();
                }
                if was_active && !active && !self.moving_left && !self.moving_right {
                    self.brake_released_movement();
                }
            }
            ClientCommand::ClearDirectionalInput => {
                let was_moving = self.moving_left || self.moving_right;
                self.moving_left = false;
                self.moving_right = false;
                self.sprint_left_tap_ticks = 0;
                self.sprint_right_tap_ticks = 0;
                self.stop_sprinting();
                if was_moving {
                    self.brake_released_movement();
                }
            }
            ClientCommand::ToggleInventory => {
                self.close_settings_menu();
                self.jump_held = false;
                self.jump_buffer_ticks = 0;
                if self.inventory_open {
                    self.clear_container_interaction_state();
                } else {
                    self.open_player_inventory_view();
                    self.moving_left = false;
                    self.moving_right = false;
                    self.left_click_down = false;
                    self.stop_sprinting();
                }
            }
            ClientCommand::ToggleSneak => {
                self.sneak_toggled = !self.sneak_toggled;
                self.sync_sneak_state();
            }
            ClientCommand::SetSneakHeld(held) => {
                self.sneak_held = held;
                self.sync_sneak_state();
            }
            ClientCommand::SelectHotbarSlot(idx) => {
                if idx < 9 {
                    self.hotbar_index = idx;
                }
            }
            ClientCommand::SetPrimaryAction(down) => {
                self.left_click_down = down && !self.settings_menu_open;
            }
            ClientCommand::SkipCompletionCredits => self.skip_completion_credits(),
            ClientCommand::RespawnFromDeathScreen => self.respawn_from_death_screen(),
            ClientCommand::TravelToOverworld => {
                self.quick_travel_to_dimension(Dimension::Overworld)
            }
            ClientCommand::TravelToNether => self.quick_travel_to_dimension(Dimension::Nether),
            ClientCommand::TravelToEnd => self.quick_travel_to_dimension(Dimension::End),
            ClientCommand::TravelToSpawn => self.quick_travel_to_spawn(),
            ClientCommand::EquipDiamondLoadout => self.equip_diamond_loadout(),
            ClientCommand::UseAt(bx, by) => {
                if !self.settings_menu_open {
                    self.interact_block(bx, by, false);
                }
            }
        }
    }

    pub fn cycle_movement_profile(&mut self) {
        self.movement_profile = match self.movement_profile {
            MovementProfile::Classic => MovementProfile::Smooth,
            MovementProfile::Smooth => MovementProfile::Agile,
            MovementProfile::Agile => MovementProfile::Classic,
        };
    }

    pub fn cycle_difficulty(&mut self) {
        self.difficulty = match self.difficulty {
            Difficulty::Peaceful => Difficulty::Easy,
            Difficulty::Easy => Difficulty::Normal,
            Difficulty::Normal => Difficulty::Hard,
            Difficulty::Hard => Difficulty::Peaceful,
        };
        self.reset_spawn_timers_for_rules();
        self.apply_peaceful_cleanup_if_needed();
    }

    pub fn cycle_game_rules_preset(&mut self) {
        self.game_rules_preset = match self.game_rules_preset {
            GameRulesPreset::Vanilla => GameRulesPreset::KeepInventory,
            GameRulesPreset::KeepInventory => GameRulesPreset::Builder,
            GameRulesPreset::Builder | GameRulesPreset::Custom => GameRulesPreset::Vanilla,
        };
        self.game_rules = self.game_rules_preset.rules();
        self.reset_spawn_timers_for_rules();
        if self.game_rules_preset == GameRulesPreset::Builder {
            self.weather = WeatherType::Clear;
            self.weather_timer = 7200;
            self.thunder_flash_timer = 0;
            self.clear_hostile_entities();
        }
        self.apply_peaceful_cleanup_if_needed();
    }

    pub fn toggle_rule_mob_spawning(&mut self) {
        self.game_rules.do_mob_spawning = !self.game_rules.do_mob_spawning;
        self.sync_game_rules_preset_from_rules();
        self.reset_spawn_timers_for_rules();
        self.apply_peaceful_cleanup_if_needed();
    }

    pub fn toggle_rule_daylight_cycle(&mut self) {
        self.game_rules.do_daylight_cycle = !self.game_rules.do_daylight_cycle;
        self.sync_game_rules_preset_from_rules();
    }

    pub fn toggle_rule_weather_cycle(&mut self) {
        self.game_rules.do_weather_cycle = !self.game_rules.do_weather_cycle;
        self.sync_game_rules_preset_from_rules();
    }

    pub fn toggle_rule_keep_inventory(&mut self) {
        self.game_rules.keep_inventory = !self.game_rules.keep_inventory;
        self.sync_game_rules_preset_from_rules();
    }

    fn start_completion_credits(&mut self) {
        self.credits_active = true;
        self.credits_tick = 0;
        self.close_settings_menu();
        self.clear_container_interaction_state();
        self.left_click_down = false;
        self.moving_left = false;
        self.moving_right = false;
        self.bow_draw_ticks = 0;
        self.bow_draw_active = false;
        self.player.vx = 0.0;
        self.player.vy = 0.0;
    }

    fn update_completion_credits(&mut self) {
        if !self.credits_active {
            return;
        }
        self.credits_tick = self.credits_tick.saturating_add(1);
        self.player.age = self.player.age.saturating_add(1);
        if self.credits_tick >= CREDITS_AUTO_FINISH_TICKS {
            self.finish_completion_credits();
        }
    }

    fn finish_completion_credits(&mut self) {
        if !self.credits_active {
            return;
        }
        self.credits_active = false;
        self.credits_tick = 0;
        self.completion_credits_seen = true;
        self.transfer_overworld_end_dimension(Dimension::Overworld);
        self.portal_cooldown = self.portal_cooldown.max(40);
    }

    fn brake_released_movement(&mut self) {
        self.player.vx *= if self.player.grounded { 0.18 } else { 0.28 };
        if self.player.vx.abs() < 0.02 {
            self.player.vx = 0.0;
        }
    }

    fn movement_tuning(&self) -> MovementTuning {
        match self.movement_profile {
            MovementProfile::Classic => MovementTuning {
                walk_speed: 0.32,
                sneak_speed: 0.12,
                ground_accel: 0.85,
                air_accel: 0.42,
                ground_drag_active: 1.0,
                ground_drag_idle: 0.76,
                air_drag: 0.98,
            },
            MovementProfile::Smooth => MovementTuning {
                walk_speed: 0.34,
                sneak_speed: 0.13,
                ground_accel: 0.66,
                air_accel: 0.36,
                ground_drag_active: 0.94,
                ground_drag_idle: 0.78,
                air_drag: 0.985,
            },
            MovementProfile::Agile => MovementTuning {
                walk_speed: 0.40,
                sneak_speed: 0.15,
                ground_accel: 0.92,
                air_accel: 0.55,
                ground_drag_active: 0.95,
                ground_drag_idle: 0.8,
                air_drag: 0.988,
            },
        }
    }

    fn start_death_screen(&mut self) {
        self.cancel_ender_pearls();
        self.death_screen_active = true;
        self.death_screen_ticks = 0;
        self.close_settings_menu();
        self.clear_container_interaction_state();
        self.left_click_down = false;
        self.moving_left = false;
        self.moving_right = false;
        self.bow_draw_ticks = 0;
        self.bow_draw_active = false;
        self.clear_fishing_line();
        if !self.furnace_explicit {
            self.furnace_job = None;
            self.furnace_progress_ticks = 0;
            self.furnace_burn_ticks = 0;
        }
        self.potion_strength_timer = 0;
        self.potion_regeneration_timer = 0;
        self.potion_fire_resistance_timer = 0;
        self.player.vx = 0.0;
        self.player.vy = 0.0;
        self.player.grounded = false;
        self.respawn_grace_ticks = 0;
        self.player_combat_hurt_cooldown = 0;
    }

    pub fn respawn_from_death_screen(&mut self) {
        if !self.can_respawn_from_death_screen() {
            return;
        }
        if !self.cooperative() && !self.save_before_dimension_change() {
            return;
        }
        self.clear_container_interaction_state();
        self.death_screen_active = false;
        self.death_screen_ticks = 0;
        self.respawn_grace_ticks = RESPAWN_GRACE_TICKS;
        self.player_combat_hurt_cooldown = 0;
        self.player.health = self.player.max_health;
        self.player.hunger = self.player.max_hunger;

        self.mounted_boat = None;
        self.travel_dimension(Dimension::Overworld);
        let (spawn_x, spawn_y) = self.resolve_overworld_spawn_location();
        self.player.x = spawn_x as f64 + 0.5;
        self.player.y = spawn_y;
        self.player.vx = 0.0;
        self.player.vy = 0.0;
        self.player.grounded = false;
        self.player.drowning_timer = 300;
        self.player.burning_timer = 0;
        self.player.fall_distance = 0.0;
        self.bow_draw_ticks = 0;
        self.bow_draw_active = false;
        self.clear_fishing_line();
        if !self.furnace_explicit {
            self.furnace_job = None;
            self.furnace_progress_ticks = 0;
            self.furnace_burn_ticks = 0;
        }
        self.potion_strength_timer = 0;
        self.potion_regeneration_timer = 0;
        self.potion_fire_resistance_timer = 0;
        if !self.active_game_rules().keep_inventory {
            self.inventory = Inventory::new(PLAYER_INVENTORY_CAPACITY);
            self.armor_slots = [None, None, None, None];
            self.inventory_enchant_levels = [0; PLAYER_INVENTORY_CAPACITY];
            self.armor_enchant_levels = [0; ARMOR_SLOT_COUNT];
            self.set_total_experience(0);
        }
        if !self.cooperative() {
            self.save_progression();
        }
    }

    fn resolve_overworld_spawn_location(&mut self) -> (i32, f64) {
        let mut spawn_x = 0i32;
        let mut spawn_y = 10.0f64;
        let mut used_bed_spawn = false;

        if let Some((bed_x, bed_y)) = self.spawn_point {
            self.world.load_chunks_around(bed_x);
            let bed_exists = self.world.get_block(bed_x, bed_y) == BlockType::Bed;
            let bed_clear = bed_exists
                && self.world.get_block(bed_x, bed_y - 1) == BlockType::Air
                && self.world.get_block(bed_x, bed_y - 2) == BlockType::Air;
            if bed_clear && Self::is_safe_spawn_column(&self.world, bed_x, bed_y, false) {
                spawn_x = bed_x;
                spawn_y = bed_y as f64 - 0.1;
                used_bed_spawn = true;
            } else if bed_exists {
                if let Some((safe_x, safe_y)) =
                    Self::find_nearest_safe_spawn(&self.world, bed_x, 12, false)
                {
                    spawn_x = safe_x;
                    spawn_y = safe_y as f64 - 0.1;
                    used_bed_spawn = true;
                }
            } else {
                self.spawn_point = None;
            }
        }

        if !used_bed_spawn {
            self.world.load_chunks_around(0);
            let (default_spawn_x, default_spawn_y) =
                Self::progression_spawn_point(&mut self.world, 0);
            spawn_x = default_spawn_x;
            spawn_y = default_spawn_y;
        }

        self.world.newly_generated_chunks.clear();
        (spawn_x, spawn_y)
    }

    pub fn interact_block(&mut self, bx: i32, by: i32, is_break: bool) {
        let held = self.inventory.slots[self.hotbar_index as usize].clone();
        let block = self.world.get_block(bx, by);
        let hunger = self.player.hunger;
        let health = self.player.health;
        let was_open = self.inventory_open;
        self.interact_block_inner(bx, by, is_break);
        if !is_break
            && (held != self.inventory.slots[self.hotbar_index as usize]
                || block != self.world.get_block(bx, by)
                || was_open != self.inventory_open
                || hunger != self.player.hunger
                || health != self.player.health)
        {
            let item = held.as_ref().map(|s| s.item_type);
            let kind = if hunger < self.player.hunger {
                super::action::ActionKind::Eat
            } else if item.is_some_and(|i| {
                matches!(
                    i,
                    ItemType::WaterBottle
                        | ItemType::PotionHealing
                        | ItemType::PotionStrength
                        | ItemType::PotionRegeneration
                        | ItemType::PotionFireResistance
                )
            }) {
                super::action::ActionKind::Drink
            } else {
                super::action::ActionKind::Use
            };
            self.show_action(
                kind,
                item,
                bx,
                by,
                if matches!(
                    kind,
                    super::action::ActionKind::Eat | super::action::ActionKind::Drink
                ) {
                    14
                } else {
                    8
                },
                0.0,
            );
        }
    }

    fn interact_block_inner(&mut self, bx: i32, by: i32, is_break: bool) {
        if self.inventory_open {
            return;
        }
        if !is_break && self.try_use_farm_animal_at(bx, by) {
            return;
        }
        if !is_break {
            if let Some(stack) = self.inventory.slots[self.hotbar_index as usize].as_mut() {
                let food = matches!(
                    stack.item_type,
                    ItemType::Bread
                        | ItemType::RawBeef
                        | ItemType::RawMutton
                        | ItemType::CookedMutton
                        | ItemType::CookedBeef
                        | ItemType::RawPorkchop
                        | ItemType::CookedPorkchop
                        | ItemType::RawChicken
                        | ItemType::CookedChicken
                        | ItemType::RawFish
                        | ItemType::CookedFish
                );
                if food && self.player.hunger.ceil() >= self.player.max_hunger {
                    return;
                }
                let mut ate = false;
                match stack.item_type {
                    ItemType::Bread => {
                        self.player.hunger += 5.0;
                        ate = true;
                    }
                    ItemType::RawBeef | ItemType::RawMutton => {
                        self.player.hunger += 3.0;
                        ate = true;
                    }
                    ItemType::CookedBeef => {
                        self.player.hunger += 8.0;
                        ate = true;
                    }
                    ItemType::RawPorkchop => {
                        self.player.hunger += 3.0;
                        ate = true;
                    }
                    ItemType::CookedPorkchop => {
                        self.player.hunger += 8.0;
                        ate = true;
                    }
                    ItemType::RawChicken => {
                        self.player.hunger += 2.0;
                        ate = true;
                    }
                    ItemType::CookedChicken | ItemType::CookedMutton => {
                        self.player.hunger += 6.0;
                        ate = true;
                    }
                    ItemType::RawFish => {
                        self.player.hunger += 2.0;
                        ate = true;
                    }
                    ItemType::CookedFish => {
                        self.player.hunger += 5.0;
                        ate = true;
                    }
                    _ => {}
                }
                if ate {
                    self.player.hunger = self.player.hunger.clamp(0.0, self.player.max_hunger);
                    stack.count -= 1;
                    if stack.count == 0 {
                        self.inventory.slots[self.hotbar_index as usize] = None;
                    }
                    return;
                }
            }
            if self.consume_held_potion_if_any() {
                return;
            }
        }
        if !is_break && self.current_hotbar_item_type() == Some(ItemType::EnderPearl) {
            self.throw_ender_pearl(bx, by);
            return;
        }
        if !is_break
            && self.current_hotbar_item_type() == Some(ItemType::FishingRod)
            && self.fishing_bobber_active
        {
            let mut rng = rand::thread_rng();
            self.reel_fishing_line(&mut rng);
            return;
        }
        let px = self.player.x;
        let py = self.player.y - 1.0;
        let dx = px - (bx as f64 + 0.5);
        let dy = py - (by as f64 + 0.5);
        let dist = (dx * dx + dy * dy).sqrt();
        let wants_eye_guidance =
            !is_break && self.current_hotbar_item_type() == Some(ItemType::EyeOfEnder);
        if dist > 4.5 {
            if wants_eye_guidance {
                self.throw_eye_of_ender_for_guidance();
            }
            self.player.mining_timer = 0.0;
            return;
        }
        let portal_use_target = (!is_break)
            .then(|| self.portal_use_target_at(bx, by))
            .flatten();
        let has_use_los = if let Some(target) = portal_use_target {
            self.has_portal_use_line_of_sight(px, py, bx, by, target)
        } else {
            self.can_see_block(px, py, bx, by)
        };
        if !has_use_los {
            if wants_eye_guidance {
                self.throw_eye_of_ender_for_guidance();
            }
            self.player.mining_timer = 0.0;
            return;
        }
        if let Some(boat_idx) = self.boat_at_cell(bx, by) {
            self.player.mining_timer = 0.0;
            if is_break {
                self.remove_boat_with_drop(boat_idx);
            } else if self.mounted_boat == Some(boat_idx) {
                let _ = self.try_dismount_boat();
            } else {
                let _ = self.try_mount_boat(boat_idx);
            }
            return;
        }
        self.world.activate_redstone_ore(bx, by);
        if is_break {
            let target_block = self.world.get_block(bx, by);
            if matches!(
                target_block,
                BlockType::Air
                    | BlockType::Bedrock
                    | BlockType::NetherPortal
                    | BlockType::EndPortal
            ) {
                self.player.mining_timer = 0.0;
                return;
            }
            // Fluids are interacted with via buckets, not mined directly.
            if target_block.is_fluid() {
                self.player.mining_timer = 0.0;
                return;
            }
            if let BlockType::Crops(stage) = target_block {
                if stage == 7 {
                    self.item_entities.push(ItemEntity::new(
                        bx as f64 + 0.5,
                        by as f64 + 0.2,
                        ItemType::Wheat,
                    ));
                }
                for _ in 0..crate::world::wheat_seed_drop_count_at(bx, by, stage >= 7) {
                    self.item_entities.push(ItemEntity::new(
                        bx as f64 + 0.5,
                        by as f64 + 0.2,
                        ItemType::WheatSeeds,
                    ));
                }
                self.world.set_block(bx, by, BlockType::Air);
                self.player.mining_timer = 0.0;
                return;
            }
            if let BlockType::NetherWart(stage) = target_block {
                let drop_count = nether_wart_drop_count_at(bx, by, stage >= 3);
                for _ in 0..drop_count {
                    self.item_entities.push(ItemEntity::new(
                        bx as f64 + 0.5,
                        by as f64 + 0.2,
                        ItemType::NetherWart,
                    ));
                }
                self.world.set_block(bx, by, BlockType::Air);
                self.player.mining_timer = 0.0;
                return;
            }
            if target_block == BlockType::TallGrass {
                if tall_grass_drops_seed_at(bx, by) {
                    self.item_entities.push(ItemEntity::new(
                        bx as f64 + 0.5,
                        by as f64 + 0.2,
                        ItemType::WheatSeeds,
                    ));
                }
                self.world.set_block(bx, by, BlockType::Air);
                self.player.mining_timer = 0.0;
                return;
            }
            if self.player.last_mine_x != bx || self.player.last_mine_y != by {
                self.player.mining_timer = 0.0;
                self.player.last_mine_x = bx;
                self.player.last_mine_y = by;
            }
            let tool = self.inventory.slots[self.hotbar_index as usize].as_ref();
            let tool_type = tool.map(|s| s.item_type).unwrap_or(ItemType::Stick);
            let efficiency = self.effective_held_efficiency(tool_type, target_block);
            let hardness = target_block.hardness();
            let speed = (efficiency / (hardness * 30.0)).max(0.01);
            self.player.mining_timer += speed;
            self.player.facing_right = bx as f64 > px;
            if self.player.mining_timer >= 1.0 {
                let mined_with_correct_tool =
                    tool_type.tool_level() >= target_block.required_tool_level();
                let mut rng = rand::thread_rng();
                let drop_item = if mined_with_correct_tool {
                    match self.door_blocks_at(bx, by).map(|(kind, _)| kind) {
                        Some(DoorKind::Wood) => Some(ItemType::WoodDoor),
                        _ if target_block == BlockType::Gravel && gravel_drops_flint_at(bx, by) => {
                            Some(ItemType::Flint)
                        }
                        _ => ItemType::from_block(target_block),
                    }
                } else {
                    None
                };
                if let Some(item_type) = drop_item {
                    self.item_entities.push(ItemEntity::new(
                        bx as f64 + 0.5,
                        by as f64 + 0.2,
                        item_type,
                    ));
                }
                if target_block == BlockType::Chest
                    && let Some(chest_inventory) = self.world.remove_chest_inventory(bx, by)
                {
                    self.spill_inventory_items(chest_inventory, bx as f64 + 0.5, by as f64 + 0.2);
                }
                if !self.remove_door_blocks(bx, by) {
                    self.world.set_block(bx, by, BlockType::Air);
                }
                self.collect_world_explosion_drops();
                if mined_with_correct_tool {
                    let xp = Self::experience_from_mined_block(target_block, &mut rng);
                    self.spawn_experience_orbs(bx as f64 + 0.5, by as f64 + 0.2, xp, &mut rng);
                }
                self.player.mining_timer = 0.0;
                self.apply_inventory_slot_durability_wear(self.hotbar_index as usize);
            }
        } else {
            let target_block = self.world.get_block(bx, by);
            if self.current_hotbar_item_type() == Some(ItemType::Boat)
                && matches!(target_block, BlockType::Water(_))
            {
                self.boats.push(Boat::new(bx as f64 + 0.5, by as f64));
                let _ = self.consume_held_item(ItemType::Boat);
                return;
            }
            if self.current_hotbar_item_type() == Some(ItemType::FishingRod)
                && matches!(target_block, BlockType::Water(_))
            {
                let mut rng = rand::thread_rng();
                self.cast_fishing_line(bx, by, &mut rng);
                return;
            }
            if let Some(target) = portal_use_target {
                self.use_portal_target(target);
                return;
            }
            match target_block {
                BlockType::Lever(powered) => {
                    self.world.set_block(bx, by, BlockType::Lever(!powered));
                    return;
                }
                BlockType::StoneButton(timer) => {
                    if timer == 0 {
                        self.world.set_block(bx, by, BlockType::StoneButton(10));
                    }
                    return;
                }
                BlockType::RedstoneRepeater {
                    powered,
                    delay,
                    facing_right,
                    timer,
                    target_powered,
                } => {
                    if self.player.sneaking {
                        self.world.set_block(
                            bx,
                            by,
                            BlockType::RedstoneRepeater {
                                powered,
                                delay,
                                facing_right: !facing_right,
                                timer: 0,
                                target_powered: powered,
                            },
                        );
                    } else {
                        let next_delay = if delay >= 4 { 1 } else { delay + 1 };
                        self.world.set_block(
                            bx,
                            by,
                            BlockType::RedstoneRepeater {
                                powered,
                                delay: next_delay,
                                facing_right,
                                timer: timer.min(next_delay),
                                target_powered,
                            },
                        );
                    }
                    return;
                }
                BlockType::IronDoor(open) => {
                    self.set_door_open_state(bx, by, !open);
                    return;
                }
                BlockType::WoodDoor(open) => {
                    self.set_door_open_state(bx, by, !open);
                    return;
                }
                _ => {}
            }
            if target_block == BlockType::Bed {
                if self.current_dimension == Dimension::Overworld {
                    self.spawn_point = Some((bx, by));
                    self.use_bed(bx, by);
                } else {
                    self.world.trigger_explosion(bx, by, 2, 3.0, 8);
                    self.apply_world_explosion_impacts();
                    self.collect_world_explosion_drops();
                }
                self.save_progression();
                return;
            }

            if target_block == BlockType::Chest {
                self.open_chest_inventory_view(bx, by);
                return;
            }

            if target_block == BlockType::CraftingTable {
                self.open_crafting_inventory_view();
                return;
            }
            if target_block == BlockType::EnchantingTable {
                self.open_enchanting_inventory_view();
                return;
            }
            if target_block == BlockType::Anvil {
                self.open_anvil_inventory_view();
                return;
            }
            if target_block == BlockType::BrewingStand {
                self.open_brewing_inventory_view();
                return;
            }
            if let BlockType::EndPortalFrame { filled } = target_block {
                if !filled && self.consume_held_item(ItemType::EyeOfEnder) {
                    self.world
                        .set_block(bx, by, BlockType::EndPortalFrame { filled: true });
                    self.try_activate_end_portal_from_frame(bx, by);
                }
                return;
            }
            if target_block == BlockType::Furnace {
                self.open_placed_furnace(bx, by);
                return;
            }
            if target_block == BlockType::Water(8)
                && self.replace_held_item_with(ItemType::Bucket, ItemType::WaterBucket)
            {
                self.world.set_block(bx, by, BlockType::Air);
                return;
            }
            if target_block == BlockType::Lava(8)
                && self.replace_held_item_with(ItemType::Bucket, ItemType::LavaBucket)
            {
                self.world.set_block(bx, by, BlockType::Air);
                return;
            }
            if matches!(target_block, BlockType::Water(_))
                && self.consume_held_item(ItemType::GlassBottle)
            {
                self.add_item_to_inventory_or_drop(ItemType::WaterBottle, 1);
                return;
            }
            if self.current_hotbar_item_type() == Some(ItemType::BoneMeal)
                && self.world.apply_bone_meal(bx, by)
            {
                self.consume_held_item(ItemType::BoneMeal);
                return;
            }
            if self.current_hotbar_item_type() == Some(ItemType::FlintAndSteel)
                && target_block == BlockType::Obsidian
                && self.try_activate_portal_from_block(bx, by)
            {
                self.apply_inventory_slot_durability_wear(self.hotbar_index as usize);
                return;
            }
            if self.current_hotbar_item_type() == Some(ItemType::FlintAndSteel) {
                let (fx, fy) = if target_block.is_solid() {
                    (bx, by - 1)
                } else {
                    (bx, by)
                };
                if self.world.get_block(fx, fy) == BlockType::Air
                    && (self.world.get_block(fx, fy + 1).is_solid()
                        || [(fx - 1, fy), (fx + 1, fy), (fx, fy - 1)]
                            .iter()
                            .any(|&(x, y)| self.world.get_block(x, y).is_flammable()))
                {
                    self.world.set_block(fx, fy, BlockType::Fire(8));
                    self.apply_inventory_slot_durability_wear(self.hotbar_index as usize);
                    return;
                }
            }
            if self.current_hotbar_item_type() == Some(ItemType::Shears) {
                let sheared_drop = match target_block {
                    BlockType::Leaves => Some(ItemType::Leaves),
                    BlockType::BirchLeaves => Some(ItemType::BirchLeaves),
                    BlockType::TallGrass => Some(ItemType::TallGrass),
                    BlockType::DeadBush => Some(ItemType::DeadBush),
                    _ => None,
                };
                if let Some(drop_item) = sheared_drop {
                    self.world.set_block(bx, by, BlockType::Air);
                    self.add_item_to_inventory_or_drop(drop_item, 1);
                    self.apply_inventory_slot_durability_wear(self.hotbar_index as usize);
                    return;
                }
            }

            let holding_hoe = self.inventory.slots[self.hotbar_index as usize]
                .as_ref()
                .is_some_and(|stack| {
                    matches!(
                        stack.item_type,
                        ItemType::WoodHoe
                            | ItemType::StoneHoe
                            | ItemType::IronHoe
                            | ItemType::DiamondHoe
                    )
                });
            if holding_hoe {
                let hoe_target = match target_block {
                    BlockType::Dirt | BlockType::Grass => Some((bx, by, None)),
                    BlockType::TallGrass
                    | BlockType::RedFlower
                    | BlockType::YellowFlower
                    | BlockType::DeadBush => {
                        let ground_y = by + 1;
                        matches!(
                            self.world.get_block(bx, ground_y),
                            BlockType::Dirt | BlockType::Grass
                        )
                        .then_some((
                            bx,
                            ground_y,
                            Some((bx, by, target_block)),
                        ))
                    }
                    _ => None,
                };

                if let Some((till_x, till_y, clicked_cover)) = hoe_target {
                    let cover_y = till_y - 1;
                    let cover_block = self.world.get_block(till_x, cover_y);
                    if !matches!(cover_block, BlockType::Air) && !cover_block.is_replaceable() {
                        return;
                    }

                    if let Some((cover_x, cover_y, cover_block)) = clicked_cover {
                        self.clear_tilling_cover_block(cover_x, cover_y, cover_block);
                    } else if cover_block != BlockType::Air {
                        self.clear_tilling_cover_block(till_x, cover_y, cover_block);
                    }

                    let moisture = if self.world.has_nearby_farmland_water(till_x, till_y) {
                        7
                    } else {
                        1
                    };
                    self.world
                        .set_block(till_x, till_y, BlockType::Farmland(moisture));
                    self.apply_inventory_slot_durability_wear(self.hotbar_index as usize);
                    return;
                }
            }

            if wants_eye_guidance {
                self.throw_eye_of_ender_for_guidance();
                return;
            }

            let hotbar_idx = self.hotbar_index as usize;
            let Some(held_item) = self.inventory.slots[hotbar_idx]
                .as_ref()
                .map(|stack| stack.item_type)
            else {
                return;
            };

            let Some((place_x, place_y)) = self.resolve_block_placement_target(bx, by) else {
                return;
            };

            let placement_facing_right = place_x as f64 >= px;
            let block_type = if held_item == ItemType::Piston {
                if self.player.sneaking {
                    BlockType::StickyPiston {
                        extended: false,
                        facing_right: placement_facing_right,
                    }
                } else {
                    BlockType::Piston {
                        extended: false,
                        facing_right: placement_facing_right,
                    }
                }
            } else if held_item == ItemType::RedstoneRepeater {
                BlockType::RedstoneRepeater {
                    powered: false,
                    delay: 1,
                    facing_right: placement_facing_right,
                    timer: 0,
                    target_powered: false,
                }
            } else {
                let Some(block_type) = held_item.to_block() else {
                    return;
                };
                block_type
            };

            if matches!(block_type, BlockType::WoodDoor(_)) {
                if !self.place_wood_door(place_x, place_y) {
                    return;
                }
            } else {
                if !block_type.is_fluid() && self.block_intersects_player_bounds(place_x, place_y) {
                    return;
                }
                if matches!(
                    block_type,
                    BlockType::RedstoneDust(_) | BlockType::RedstoneRepeater { .. }
                ) && !self.world.get_block(place_x, place_y + 1).is_solid()
                {
                    return;
                }
                if matches!(
                    block_type,
                    BlockType::Lever(_) | BlockType::StoneButton(_) | BlockType::RedstoneTorch(_)
                ) && !self.has_adjacent_solid(place_x, place_y)
                {
                    return;
                }
                if matches!(block_type, BlockType::Ladder)
                    && !self.has_adjacent_solid(place_x, place_y)
                {
                    return;
                }
                if block_type == BlockType::Torch
                    && self.world.torch_attachment(place_x, place_y).is_none()
                {
                    return;
                }
                if !self.has_valid_bottom_support_for_block(place_x, place_y, block_type) {
                    return;
                }

                self.world.set_block(place_x, place_y, block_type);
            }
            self.player.facing_right = place_x as f64 > px;
            if matches!(held_item, ItemType::WaterBucket | ItemType::LavaBucket) {
                let _ = self.replace_held_item_with(held_item, ItemType::Bucket);
            } else if let Some(stack) = self.inventory.slots[hotbar_idx].as_mut() {
                stack.count -= 1;
                if stack.count == 0 {
                    self.inventory.slots[hotbar_idx] = None;
                }
            }
        }
    }

    pub(crate) fn move_inventory_slot_to_hotbar(&mut self, slot: usize) {
        if slot >= self.inventory.slots.len() {
            return;
        }
        let hotbar = self.hotbar_index as usize;
        self.inventory.slots.swap(slot, hotbar);
        self.inventory_enchant_levels.swap(slot, hotbar);
        self.selected_inventory_slot = None;
    }

    pub fn handle_inventory_click(&mut self, slot_idx: usize) {
        if !self.inventory_open {
            return;
        }
        if self.at_chest && !self.ensure_chest_ui_state_valid() {
            return;
        }

        if slot_idx == CRAFT_OUTPUT_UI_SLOT && self.active_crafting_grid_size().is_some() {
            self.attempt_craft_from_grid();
            return;
        }
        if (CRAFT_GRID_UI_OFFSET..(CRAFT_GRID_UI_OFFSET + CRAFT_GRID_UI_SLOTS)).contains(&slot_idx)
            && self.active_crafting_grid_size().is_some()
        {
            let cell_idx = slot_idx - CRAFT_GRID_UI_OFFSET;
            self.handle_crafting_grid_click(cell_idx);
            return;
        }

        let slot_count = self.active_ui_slot_count();
        if slot_idx >= slot_count {
            return;
        }

        if let Some(first_idx) = self.selected_inventory_slot {
            if first_idx == slot_idx {
                self.selected_inventory_slot = None;
                return;
            }
            if first_idx >= slot_count && !self.is_crafting_grid_ui_slot(first_idx) {
                self.selected_inventory_slot = None;
                return;
            }
            let Some(first_stack) = self.any_slot_item_clone(first_idx) else {
                self.selected_inventory_slot = None;
                return;
            };
            let Some(second_stack) = self.ui_slot_item_clone(slot_idx) else {
                self.selected_inventory_slot = None;
                return;
            };
            let first_enchant = self.ui_slot_enchant_level(first_idx).unwrap_or(0);
            let second_enchant = self.ui_slot_enchant_level(slot_idx).unwrap_or(0);
            if let (Some(source), Some(target)) = (&first_stack, &second_stack)
                && source.item_type == target.item_type
                && source.durability.is_none()
                && target.durability.is_none()
                && first_enchant == second_enchant
            {
                self.transfer_full_stack_between_slots(first_idx, &[slot_idx]);
                self.selected_inventory_slot = self
                    .ui_slot_item_clone(first_idx)
                    .flatten()
                    .map(|_| first_idx);
                return;
            }
            if let Some(stack) = second_stack.as_ref()
                && !self.can_place_stack_in_ui_slot(first_idx, stack)
            {
                return;
            }
            if let Some(stack) = first_stack.as_ref()
                && !self.can_place_stack_in_ui_slot(slot_idx, stack)
            {
                return;
            }
            if self.set_any_slot_item(first_idx, second_stack)
                && self.set_ui_slot_item(slot_idx, first_stack)
            {
                let _ = self.set_ui_slot_enchant_level(first_idx, second_enchant);
                let _ = self.set_ui_slot_enchant_level(slot_idx, first_enchant);
                self.set_hotbar_index_from_ui_slot(slot_idx);
            }
            self.selected_inventory_slot = None;
        } else {
            self.selected_inventory_slot = Some(slot_idx);
            self.set_hotbar_index_from_ui_slot(slot_idx);
        }
    }

    pub fn handle_inventory_right_click(&mut self, slot_idx: usize) {
        if !self.inventory_open {
            return;
        }
        if self.at_chest && !self.ensure_chest_ui_state_valid() {
            return;
        }

        if slot_idx == CRAFT_OUTPUT_UI_SLOT && self.active_crafting_grid_size().is_some() {
            self.attempt_craft_from_grid();
            return;
        }
        if self.is_crafting_grid_ui_slot(slot_idx) && self.active_crafting_grid_size().is_some() {
            let cell_idx = slot_idx - CRAFT_GRID_UI_OFFSET;
            if !self.is_valid_active_crafting_cell(cell_idx) {
                return;
            }
            if let Some(source_slot) = self.selected_inventory_slot {
                if self.transfer_single_between_slots(source_slot, slot_idx)
                    && self
                        .any_slot_item_clone(source_slot)
                        .is_some_and(|stack| stack.is_none())
                {
                    self.selected_inventory_slot = None;
                }
            } else if self.crafting_grid[cell_idx].is_some() {
                self.selected_inventory_slot = Some(slot_idx);
                self.split_selected_stack_into_empty_slot();
            }
            return;
        }

        let slot_count = self.active_ui_slot_count();
        if slot_idx >= slot_count {
            return;
        }
        if let Some(source_slot) = self.selected_inventory_slot {
            if source_slot == slot_idx {
                let _ = self.split_selected_stack_into_empty_slot();
                return;
            }
            if self.transfer_single_between_slots(source_slot, slot_idx) {
                self.set_hotbar_index_from_ui_slot(slot_idx);
                if self
                    .any_slot_item_clone(source_slot)
                    .is_some_and(|stack| stack.is_none())
                {
                    self.selected_inventory_slot = None;
                }
            } else if self.any_slot_item_clone(source_slot).is_none() {
                self.selected_inventory_slot = None;
            }
            return;
        }
        if self
            .ui_slot_item_clone(slot_idx)
            .is_some_and(|stack| stack.is_some())
        {
            self.selected_inventory_slot = Some(slot_idx);
            self.set_hotbar_index_from_ui_slot(slot_idx);
            let _ = self.split_selected_stack_into_empty_slot();
        }
    }

    fn furnace_job_from_recipe(recipe: &Recipe) -> Option<FurnaceJob> {
        if !recipe.needs_furnace {
            return None;
        }
        let (input, input_count) = recipe
            .ingredients
            .iter()
            .copied()
            .find(|(item, _)| *item != ItemType::Coal)?;
        Some(FurnaceJob {
            input,
            input_count,
            output: recipe.result,
            output_count: recipe.result_count,
        })
    }

    fn furnace_job_for_input_item(item: ItemType) -> Option<FurnaceJob> {
        Recipe::all()
            .iter()
            .filter_map(Self::furnace_job_from_recipe)
            .find(|job| job.input == item)
    }

    fn first_available_furnace_job(&self) -> Option<FurnaceJob> {
        Recipe::all()
            .iter()
            .filter_map(Self::furnace_job_from_recipe)
            .find(|job| self.inventory.has_item(job.input, job.input_count))
    }

    fn quick_start_furnace_from_shift_click(&mut self, slot_idx: usize) -> bool {
        if self.at_furnace && self.furnace_explicit {
            let targets = if self.furnace_ui_index(slot_idx).is_some() {
                (0..PLAYER_INVENTORY_CAPACITY).collect()
            } else if let Some(Some(item)) = self.ui_slot_item_clone(slot_idx) {
                if Self::furnace_fuel_ticks(item.item_type) > 0 {
                    vec![FURNACE_FUEL_UI_SLOT]
                } else if Self::furnace_job_for_input_item(item.item_type).is_some() {
                    vec![FURNACE_INPUT_UI_SLOT]
                } else {
                    return false;
                }
            } else {
                return false;
            };
            return self.transfer_full_stack_between_slots(slot_idx, &targets);
        }
        if !self.at_furnace || self.furnace_job.is_some() || slot_idx >= PLAYER_INVENTORY_CAPACITY {
            return false;
        }
        let Some(stack) = self.ui_slot_item_clone(slot_idx).flatten() else {
            return false;
        };
        let candidate_job = if stack.item_type == ItemType::Coal {
            self.first_available_furnace_job()
        } else {
            Self::furnace_job_for_input_item(stack.item_type)
        };
        let Some(job) = candidate_job else {
            return false;
        };
        if !self.can_start_furnace_job(job) {
            return false;
        }
        self.start_furnace_job(job)
    }

    fn has_furnace_fuel(&self) -> bool {
        self.inventory.has_item(ItemType::Coal, 1)
    }

    fn consume_one_furnace_fuel(&mut self) -> bool {
        self.inventory.remove_item(ItemType::Coal, 1)
    }

    fn can_start_furnace_job(&self, job: FurnaceJob) -> bool {
        self.inventory.has_item(job.input, job.input_count)
            && (self.furnace_burn_ticks > 0 || self.has_furnace_fuel())
    }

    fn start_furnace_job(&mut self, job: FurnaceJob) -> bool {
        if !self.can_start_furnace_job(job) {
            return false;
        }
        if !self.inventory.remove_item(job.input, job.input_count) {
            return false;
        }
        if self.furnace_burn_ticks == 0 && !self.consume_one_furnace_fuel() {
            self.inventory.add_item(job.input, job.input_count);
            return false;
        } else if self.furnace_burn_ticks == 0 {
            self.furnace_burn_ticks = FURNACE_COAL_BURN_TICKS;
        }
        self.furnace_job = Some(job);
        self.furnace_progress_ticks = 0;
        true
    }

    fn update_furnace(&mut self) {
        if self.at_furnace && self.open_chest_pos.is_some() {
            return;
        }
        if self.furnace_explicit {
            self.update_explicit_furnace();
            return;
        }
        let Some(job) = self.furnace_job else {
            return;
        };
        if self.furnace_progress_ticks < FURNACE_COOK_TICKS {
            if self.furnace_burn_ticks == 0 {
                if !self.consume_one_furnace_fuel() {
                    return;
                }
                self.furnace_burn_ticks = FURNACE_COAL_BURN_TICKS;
            }
            self.furnace_burn_ticks = self.furnace_burn_ticks.saturating_sub(1);
            self.furnace_progress_ticks = self.furnace_progress_ticks.saturating_add(1);
        }
        if self.furnace_progress_ticks < FURNACE_COOK_TICKS {
            return;
        }
        let mut after = self.inventory.clone();
        if after.add_item(job.output, job.output_count) > 0 {
            return;
        }
        self.inventory = after;
        self.furnace_progress_ticks = 0;
        if !self.inventory.remove_item(job.input, job.input_count) {
            self.furnace_job = None;
        }
    }

    pub fn can_start_furnace_recipe(&self, recipe_idx: usize) -> bool {
        if self.furnace_explicit {
            return Recipe::all()
                .get(recipe_idx)
                .and_then(Self::furnace_job_from_recipe)
                .is_some_and(|job| self.explicit_furnace_can_load(job));
        }
        if self.furnace_job.is_some() {
            return false;
        }
        let recipes = Recipe::all();
        let Some(recipe) = recipes.get(recipe_idx) else {
            return false;
        };
        let Some(job) = Self::furnace_job_from_recipe(recipe) else {
            return false;
        };
        self.can_start_furnace_job(job)
    }

    pub fn furnace_status_line(&self) -> String {
        if self.furnace_explicit {
            if let Some(job) = self.furnace_job {
                return format!(
                    "{}: {}% | {}",
                    job.output.name(),
                    self.furnace_progress_ticks as u32 * 100 / FURNACE_COOK_TICKS as u32,
                    if self.furnace_burn_ticks == 0 {
                        "Needs fuel"
                    } else {
                        "Cooking"
                    }
                );
            }
            return if self.furnace_slots[0].is_some()
                && self.furnace_slots[2]
                    .as_ref()
                    .is_some_and(|s| s.count >= s.item_type.max_stack_size())
            {
                "Output full - collect the result".into()
            } else {
                "Place input + fuel; collect output".into()
            };
        }
        if let Some(job) = self.furnace_job {
            if self.furnace_progress_ticks >= FURNACE_COOK_TICKS {
                return format!("{} ready · Free bag space to collect", job.output.name());
            }
            let progress_pct =
                (self.furnace_progress_ticks as u32 * 100 / FURNACE_COOK_TICKS as u32) as u8;
            let fuel_pct =
                (self.furnace_burn_ticks as u32 * 100 / FURNACE_COAL_BURN_TICKS as u32) as u8;
            format!(
                "Smelting: {} -> {} | Progress {}% | Fuel {}%",
                job.input.name(),
                job.output.name(),
                progress_pct,
                fuel_pct
            )
        } else {
            "Idle: click recipe or Shift+L on smelt input (Fuel: Coal)".to_string()
        }
    }

    fn brew_option_recipe(
        option_idx: usize,
    ) -> Option<(ItemType, ItemType, ItemType, &'static str)> {
        match option_idx {
            0 => Some((
                ItemType::WaterBottle,
                ItemType::NetherWart,
                ItemType::AwkwardPotion,
                "Awkward Potion",
            )),
            1 => Some((
                ItemType::AwkwardPotion,
                ItemType::RedFlower,
                ItemType::PotionHealing,
                "Potion of Healing",
            )),
            2 => Some((
                ItemType::AwkwardPotion,
                ItemType::BlazePowder,
                ItemType::PotionStrength,
                "Potion of Strength",
            )),
            3 => Some((
                ItemType::AwkwardPotion,
                ItemType::GhastTear,
                ItemType::PotionRegeneration,
                "Potion of Regeneration",
            )),
            4 => Some((
                ItemType::AwkwardPotion,
                ItemType::MagmaCream,
                ItemType::PotionFireResistance,
                "Potion of Fire Resistance",
            )),
            _ => None,
        }
    }

    pub fn brewing_status_line(&self) -> String {
        let slot_idx = self.hotbar_index as usize;
        let held = self.inventory.slots[slot_idx]
            .as_ref()
            .map(|stack| stack.item_type.name())
            .unwrap_or("---");
        format!(
            "Held: {} | Water {} | Awkward {}",
            held,
            self.inventory_item_count(ItemType::WaterBottle),
            self.inventory_item_count(ItemType::AwkwardPotion)
        )
    }

    pub fn can_apply_brew_option(&self, option_idx: usize) -> bool {
        if !self.at_brewing_stand || option_idx >= BREW_OPTION_COUNT {
            return false;
        }
        let Some((base, reagent, _, _)) = Self::brew_option_recipe(option_idx) else {
            return false;
        };
        let Some(held) = self.inventory.slots[self.hotbar_index as usize].as_ref() else {
            return false;
        };
        held.item_type == reagent && held.count > 0 && self.inventory.has_item(base, 1)
    }

    pub fn attempt_brew_option(&mut self, option_idx: usize) {
        if !self.can_apply_brew_option(option_idx) {
            return;
        }
        let Some((base, reagent, output, _)) = Self::brew_option_recipe(option_idx) else {
            return;
        };
        let brew_count = self.inventory_item_count(base).min(3);
        if brew_count == 0 {
            return;
        }
        if !self.consume_held_item(reagent) {
            return;
        }
        if !self.inventory.remove_item(base, brew_count) {
            self.add_item_to_inventory_or_drop(reagent, 1);
            return;
        }
        self.add_item_to_inventory_or_drop(output, brew_count);
    }

    pub fn enchanting_status_line(&self) -> String {
        let slot_idx = self.hotbar_index as usize;
        let Some(stack) = self.inventory.slots[slot_idx].as_ref() else {
            return "Hold an enchantable item in the selected hotbar slot.".to_string();
        };
        if !Self::can_item_be_enchanted(stack.item_type) || stack.count != 1 {
            return "Hold a tool, weapon, bow, or armor piece to enchant.".to_string();
        }
        let lvl = self.inventory_enchant_levels[slot_idx].min(ENCHANT_MAX_LEVEL);
        format!(
            "Selected: {} | Enchant L{} | Player Lv {}",
            stack.item_type.name(),
            lvl,
            self.player.experience_level
        )
    }

    pub fn can_apply_enchant_option(&self, option_idx: usize) -> bool {
        if !self.at_enchanting_table || option_idx >= ENCHANT_OPTION_COUNT {
            return false;
        }
        let slot_idx = self.hotbar_index as usize;
        let Some(stack) = self.inventory.slots[slot_idx].as_ref() else {
            return false;
        };
        if !Self::can_item_be_enchanted(stack.item_type) || stack.count != 1 {
            return false;
        }
        let max_durability = stack.item_type.max_durability().unwrap_or(0);
        let current_durability = stack
            .durability
            .unwrap_or(max_durability)
            .min(max_durability);
        let current_enchant = self.inventory_enchant_levels[slot_idx].min(ENCHANT_MAX_LEVEL);
        if current_enchant >= ENCHANT_MAX_LEVEL && current_durability >= max_durability {
            return false;
        }
        self.player.experience_level >= ENCHANT_LEVEL_COSTS[option_idx]
    }

    pub fn attempt_enchant_option(&mut self, option_idx: usize) {
        if !self.can_apply_enchant_option(option_idx) {
            return;
        }
        let cost_levels = ENCHANT_LEVEL_COSTS[option_idx];
        if !self.spend_experience_levels(cost_levels) {
            return;
        }
        let repair_ratio = match option_idx {
            0 => 0.20,
            1 => 0.45,
            _ => 0.80,
        };
        let enchant_gain = (option_idx as u8 + 1).min(ENCHANT_MAX_LEVEL);
        let slot_idx = self.hotbar_index as usize;
        if let Some(stack) = self.inventory.slots[slot_idx].as_mut()
            && let Some(max_durability) = stack.item_type.max_durability()
        {
            let current_durability = stack
                .durability
                .unwrap_or(max_durability)
                .min(max_durability);
            let repair_amount = ((max_durability as f32) * repair_ratio).ceil() as u32;
            let next_durability = current_durability
                .saturating_add(repair_amount)
                .min(max_durability);
            stack.durability = Some(next_durability);
            self.inventory_enchant_levels[slot_idx] = self.inventory_enchant_levels[slot_idx]
                .saturating_add(enchant_gain)
                .min(ENCHANT_MAX_LEVEL);
        }
    }

    fn anvil_partner_for_hotbar(&self) -> Option<usize> {
        let held_slot = self.hotbar_index as usize;
        let held = self.inventory.slots[held_slot].as_ref()?;
        if !Self::can_item_be_enchanted(held.item_type) || held.count != 1 {
            return None;
        }
        let max_durability = held.item_type.max_durability().unwrap_or(0);
        self.inventory
            .slots
            .iter()
            .enumerate()
            .find_map(|(idx, slot)| {
                if idx == held_slot {
                    return None;
                }
                let other = slot.as_ref()?;
                if other.item_type != held.item_type || other.count != 1 {
                    return None;
                }
                let other_durability = other
                    .durability
                    .unwrap_or(max_durability)
                    .min(max_durability);
                if other_durability == 0 {
                    return None;
                }
                Some(idx)
            })
    }

    pub fn anvil_status_line(&self) -> String {
        let held_slot = self.hotbar_index as usize;
        let Some(held) = self.inventory.slots[held_slot].as_ref() else {
            return "Hold a repairable item in your selected hotbar slot.".to_string();
        };
        if !Self::can_item_be_enchanted(held.item_type) || held.count != 1 {
            return "Anvil combine requires a tool/weapon/armor item.".to_string();
        }
        let held_enchant = self.inventory_enchant_levels[held_slot].min(ENCHANT_MAX_LEVEL);
        if let Some(partner_slot) = self.anvil_partner_for_hotbar() {
            let partner_hotbar = partner_slot + 1;
            format!(
                "Combine with slot {partner_hotbar} | cost {ANVIL_COMBINE_LEVEL_COST} levels | Enchant L{held_enchant}"
            )
        } else {
            "No matching item found in inventory to combine.".to_string()
        }
    }

    pub fn can_apply_anvil_combine(&self) -> bool {
        self.at_anvil
            && self.player.experience_level >= ANVIL_COMBINE_LEVEL_COST
            && self.anvil_partner_for_hotbar().is_some()
    }

    pub fn enchant_option_cost(&self, option_idx: usize) -> Option<u32> {
        ENCHANT_LEVEL_COSTS.get(option_idx).copied()
    }

    pub fn attempt_anvil_combine(&mut self) {
        if !self.can_apply_anvil_combine() {
            return;
        }
        let held_slot = self.hotbar_index as usize;
        let Some(partner_slot) = self.anvil_partner_for_hotbar() else {
            return;
        };
        let Some(held_item_type) = self.inventory.slots[held_slot]
            .as_ref()
            .map(|s| s.item_type)
        else {
            return;
        };
        let Some(max_durability) = held_item_type.max_durability() else {
            return;
        };
        if !self.spend_experience_levels(ANVIL_COMBINE_LEVEL_COST) {
            return;
        }

        let held_durability = self.inventory.slots[held_slot]
            .as_ref()
            .and_then(|stack| stack.durability)
            .unwrap_or(max_durability)
            .min(max_durability);
        let partner_durability = self.inventory.slots[partner_slot]
            .as_ref()
            .and_then(|stack| stack.durability)
            .unwrap_or(max_durability)
            .min(max_durability);
        let repair_bonus = (max_durability / 10).max(1);
        let combined_durability = held_durability
            .saturating_add(partner_durability)
            .saturating_add(repair_bonus)
            .min(max_durability);

        if let Some(held_stack) = self.inventory.slots[held_slot].as_mut() {
            held_stack.durability = Some(combined_durability);
        }
        let partner_enchant = self.inventory_enchant_levels[partner_slot].min(ENCHANT_MAX_LEVEL);
        let held_enchant = self.inventory_enchant_levels[held_slot].min(ENCHANT_MAX_LEVEL);
        self.inventory_enchant_levels[held_slot] = held_enchant.max(partner_enchant);
        self.inventory.slots[partner_slot] = None;
        self.inventory_enchant_levels[partner_slot] = 0;
    }

    pub fn chest_slot_item(&self, slot_idx: usize) -> Option<&ItemStack> {
        if !self.at_chest || slot_idx >= CHEST_INVENTORY_CAPACITY {
            return None;
        }
        let (bx, by) = self.open_chest_pos?;
        self.world
            .chest_inventory(bx, by)?
            .slots
            .get(slot_idx)?
            .as_ref()
    }

    pub fn attempt_craft(&mut self, recipe_idx: usize) {
        if self.at_chest {
            return;
        }
        if self.at_enchanting_table || self.at_anvil || self.at_brewing_stand {
            return;
        }
        let recipes = Recipe::all();
        if self.at_furnace {
            if self.furnace_explicit {
                if let Some(job) = recipes
                    .get(recipe_idx)
                    .and_then(Self::furnace_job_from_recipe)
                {
                    self.load_furnace_recipe(job);
                }
                return;
            }
            if self.furnace_job.is_some() {
                return;
            }
            let Some(recipe) = recipes.get(recipe_idx) else {
                return;
            };
            let Some(job) = Self::furnace_job_from_recipe(recipe) else {
                return;
            };
            let _ = self.start_furnace_job(job);
            return;
        }

        if let Some(recipe) = recipes.get(recipe_idx)
            && recipe.can_craft(&self.inventory, self.at_crafting_table, self.at_furnace)
        {
            recipe.craft(&mut self.inventory);
            self.selected_inventory_slot = None;
        }
    }

    pub fn attempt_craft_max(&mut self, recipe_idx: usize) {
        if self.at_furnace {
            self.attempt_craft(recipe_idx);
            return;
        }
        for _ in 0..512 {
            let before = self.inventory.slots.clone();
            self.attempt_craft(recipe_idx);
            if self.inventory.slots == before {
                break;
            }
        }
    }

    fn crafting_grid_item_at(&self, x: usize, y: usize) -> Option<ItemType> {
        self.crafting_grid[y * 3 + x]
            .as_ref()
            .map(|stack| stack.item_type)
    }

    fn count_active_crafting_items(&self, grid_size: usize) -> usize {
        let mut count = 0usize;
        for y in 0..grid_size {
            for x in 0..grid_size {
                if self.crafting_grid_item_at(x, y).is_some() {
                    count += 1;
                }
            }
        }
        count
    }

    fn find_shaped_match_offset(
        &self,
        rows: &[Vec<Option<ItemType>>],
        grid_size: usize,
    ) -> Option<(usize, usize)> {
        let shape_h = rows.len();
        let shape_w = rows.first().map_or(0, |r| r.len());
        if shape_w == 0 || shape_h == 0 || shape_w > grid_size || shape_h > grid_size {
            return None;
        }
        for offset_y in 0..=(grid_size - shape_h) {
            for offset_x in 0..=(grid_size - shape_w) {
                let mut matches = true;
                for y in 0..grid_size {
                    for x in 0..grid_size {
                        let expected = if x >= offset_x
                            && y >= offset_y
                            && x < offset_x + shape_w
                            && y < offset_y + shape_h
                        {
                            rows[y - offset_y][x - offset_x]
                        } else {
                            None
                        };
                        if self.crafting_grid_item_at(x, y) != expected {
                            matches = false;
                            break;
                        }
                    }
                    if !matches {
                        break;
                    }
                }
                if matches {
                    return Some((offset_x, offset_y));
                }
            }
        }
        None
    }

    fn recipe_matches_shapeless(&self, recipe: &Recipe, grid_size: usize) -> bool {
        let requirements = recipe.ingredient_requirements();
        let mut required_counts: HashMap<ItemType, u32> = HashMap::new();
        let mut required_total = 0u32;
        for (item, amount) in requirements {
            *required_counts.entry(item).or_insert(0) += amount;
            required_total += amount;
        }

        let mut placed_counts: HashMap<ItemType, u32> = HashMap::new();
        let mut placed_total = 0u32;
        for y in 0..grid_size {
            for x in 0..grid_size {
                if let Some(item) = self.crafting_grid_item_at(x, y) {
                    *placed_counts.entry(item).or_insert(0) += 1;
                    placed_total += 1;
                }
            }
        }
        if placed_total != required_total {
            return false;
        }
        required_counts
            .iter()
            .all(|(item, amount)| placed_counts.get(item).copied().unwrap_or(0) == *amount)
    }

    fn current_crafting_recipe_index(&self) -> Option<usize> {
        let grid_size = self.active_crafting_grid_size()?;
        if self.count_active_crafting_items(grid_size) == 0 {
            return None;
        }
        let recipes = Recipe::all();
        for (idx, recipe) in recipes.iter().enumerate() {
            if recipe.needs_furnace {
                continue;
            }
            if recipe.needs_crafting_table && !self.at_crafting_table {
                continue;
            }
            if let Some(rows) = recipe.ingredient_rows() {
                if self.find_shaped_match_offset(&rows, grid_size).is_some() {
                    return Some(idx);
                }
            } else if self.recipe_matches_shapeless(recipe, grid_size) {
                return Some(idx);
            }
        }
        None
    }

    fn consume_crafting_recipe_inputs(&mut self, recipe: &Recipe, grid_size: usize) {
        if let Some(rows) = recipe.ingredient_rows() {
            if let Some((offset_x, offset_y)) = self.find_shaped_match_offset(&rows, grid_size) {
                for (y, row) in rows.iter().enumerate() {
                    for (x, cell) in row.iter().enumerate() {
                        if cell.is_some() {
                            let idx = (offset_y + y) * 3 + (offset_x + x);
                            if let Some(stack) = self.crafting_grid[idx].as_mut() {
                                stack.count = stack.count.saturating_sub(1);
                                if stack.count == 0 {
                                    self.crafting_grid[idx] = None;
                                }
                            }
                        }
                    }
                }
            }
            return;
        }

        let mut required_counts: HashMap<ItemType, u32> = HashMap::new();
        for (item, amount) in recipe.ingredient_requirements() {
            *required_counts.entry(item).or_insert(0) += amount;
        }
        for y in 0..grid_size {
            for x in 0..grid_size {
                let idx = y * 3 + x;
                let Some(item) = self.crafting_grid[idx]
                    .as_ref()
                    .map(|stack| stack.item_type)
                else {
                    continue;
                };
                let Some(needed) = required_counts.get_mut(&item) else {
                    continue;
                };
                if *needed > 0 {
                    *needed -= 1;
                    if let Some(stack) = self.crafting_grid[idx].as_mut() {
                        stack.count = stack.count.saturating_sub(1);
                        if stack.count == 0 {
                            self.crafting_grid[idx] = None;
                        }
                    }
                }
            }
        }
    }

    pub fn crafting_grid_slot_item(&self, cell_idx: usize) -> Option<ItemType> {
        self.crafting_grid
            .get(cell_idx)
            .and_then(|stack| stack.as_ref().map(|stack| stack.item_type))
    }

    pub fn crafting_grid_slot_stack(&self, cell_idx: usize) -> Option<&ItemStack> {
        self.crafting_grid
            .get(cell_idx)
            .and_then(|stack| stack.as_ref())
    }

    pub fn crafting_output_preview(&self) -> Option<(ItemType, u32)> {
        let recipe_idx = self.current_crafting_recipe_index()?;
        let recipes = Recipe::all();
        let recipe = recipes.get(recipe_idx)?;
        Some((recipe.result, recipe.result_count))
    }

    pub fn attempt_craft_from_grid(&mut self) {
        if !self.can_take_crafting_output() {
            return;
        }
        let Some(grid_size) = self.active_crafting_grid_size() else {
            return;
        };
        let Some(recipe_idx) = self.current_crafting_recipe_index() else {
            return;
        };
        let recipes = Recipe::all();
        let Some(recipe) = recipes.get(recipe_idx) else {
            return;
        };
        self.consume_crafting_recipe_inputs(recipe, grid_size);
        self.selected_inventory_slot = None;
        let overflow = self.inventory.add_item(recipe.result, recipe.result_count);
        for _ in 0..overflow {
            self.spill_single_item_near_player(recipe.result);
        }
    }

    pub fn attempt_craft_from_grid_max(&mut self) {
        for _ in 0..512 {
            if !self.can_take_crafting_output() {
                break;
            }
            self.attempt_craft_from_grid();
        }
    }

    pub fn can_take_crafting_output(&self) -> bool {
        self.crafting_output_preview().is_some_and(|(item, count)| {
            let mut inventory = self.inventory.clone();
            inventory.add_item(item, count) == 0
        })
    }

    fn handle_crafting_grid_click(&mut self, cell_idx: usize) {
        if !self.is_valid_active_crafting_cell(cell_idx) {
            return;
        }
        let target = CRAFT_GRID_UI_OFFSET + cell_idx;
        let Some(source) = self.selected_inventory_slot else {
            if self.crafting_grid[cell_idx].is_some() {
                self.selected_inventory_slot = Some(target);
            }
            return;
        };
        if source == target {
            self.selected_inventory_slot = None;
            return;
        }
        let Some(Some(incoming)) = self.any_slot_item_clone(source) else {
            self.selected_inventory_slot = None;
            return;
        };
        if incoming.durability.is_some()
            || Self::can_item_be_enchanted(incoming.item_type)
            || self.ui_slot_enchant_level(source).unwrap_or(0) > 0
        {
            return;
        }
        let old = self.crafting_grid[cell_idx].clone();
        if old
            .as_ref()
            .is_none_or(|s| s.item_type == incoming.item_type)
        {
            self.transfer_full_stack_between_slots(source, &[target]);
            self.selected_inventory_slot =
                self.any_slot_item_clone(source).flatten().map(|_| source);
        } else if self.can_place_stack_in_ui_slot(source, old.as_ref().unwrap()) {
            self.set_any_slot_item(source, old);
            self.set_any_slot_item(target, Some(incoming));
            self.selected_inventory_slot = None;
        }
    }

    fn consume_held_item(&mut self, item_type: ItemType) -> bool {
        let slot_idx = self.hotbar_index as usize;
        let Some(stack) = self.inventory.slots[slot_idx].as_mut() else {
            return false;
        };
        if stack.item_type != item_type || stack.count == 0 {
            return false;
        }
        stack.count -= 1;
        if stack.count == 0 {
            self.inventory.slots[slot_idx] = None;
        }
        true
    }

    fn replace_held_item_with(
        &mut self,
        expected_item: ItemType,
        replacement_item: ItemType,
    ) -> bool {
        let slot_idx = self.hotbar_index as usize;
        let Some((item_type, count)) = self.inventory.slots[slot_idx]
            .as_ref()
            .map(|stack| (stack.item_type, stack.count))
        else {
            return false;
        };
        if item_type != expected_item || count == 0 {
            return false;
        }
        if count == 1 {
            if let Some(stack) = self.inventory.slots[slot_idx].as_mut() {
                stack.item_type = replacement_item;
                stack.durability = replacement_item.max_durability();
            }
            self.inventory_enchant_levels[slot_idx] = 0;
        } else if let Some(stack) = self.inventory.slots[slot_idx].as_mut() {
            stack.count -= 1;
            self.add_item_to_inventory_or_drop(replacement_item, 1);
        }
        true
    }

    fn inventory_item_count(&self, item_type: ItemType) -> u32 {
        self.inventory
            .slots
            .iter()
            .flatten()
            .filter(|stack| stack.item_type == item_type)
            .map(|stack| stack.count)
            .sum()
    }

    fn add_item_to_inventory_or_drop(&mut self, item_type: ItemType, count: u32) {
        let overflow = self.inventory.add_item(item_type, count);
        for _ in 0..overflow {
            self.spill_single_item_near_player(item_type);
        }
    }

    fn boat_at_cell(&self, bx: i32, by: i32) -> Option<usize> {
        self.boats.iter().enumerate().find_map(|(idx, boat)| {
            ((boat.x.floor() as i32 == bx) && (boat.y.floor() as i32 == by)).then_some(idx)
        })
    }

    fn boat_surface_y_near(&self, x: f64, y: f64) -> Option<f64> {
        let sample_xs = [x, x - 0.3, x + 0.3];
        let mut best: Option<f64> = None;
        for sample_x in sample_xs {
            let bx = sample_x.floor() as i32;
            let center_y = y.floor() as i32;
            for by in (center_y - 1)..=(center_y + 1) {
                if matches!(self.world.get_block(bx, by), BlockType::Water(_)) {
                    let surface_y = self.world.fluid_surface_y(bx, by).unwrap();
                    best = Some(best.map_or(surface_y, |current| current.min(surface_y)));
                }
            }
        }
        best
    }

    pub(super) fn entity_collides_with_world(
        &self,
        x: f64,
        y: f64,
        half_width: f64,
        height: f64,
    ) -> bool {
        let left = x - half_width;
        let right = x + half_width;
        let top = y - height;
        let bottom = y;
        for by in top.floor() as i32..=(bottom - 0.001).floor() as i32 {
            for bx in left.floor() as i32..=(right - 0.001).floor() as i32 {
                if self.block_has_entity_collision_at(bx, by) {
                    return true;
                }
            }
        }
        false
    }

    fn sync_player_to_boat(&mut self) {
        let Some(boat_idx) = self.mounted_boat else {
            return;
        };
        let Some(boat) = self.boats.get(boat_idx) else {
            self.mounted_boat = None;
            return;
        };
        self.player.x = boat.x;
        self.player.y = boat.y - 0.18;
        self.player.vx = boat.vx;
        self.player.vy = 0.0;
        self.player.grounded = false;
        self.player.fall_distance = 0.0;
        self.player.burning_timer = 0;
    }

    fn try_mount_boat(&mut self, boat_idx: usize) -> bool {
        if boat_idx >= self.boats.len()
            || self
                .other_players_here()
                .any(|p| p.mounted_boat == Some(boat_idx))
        {
            return false;
        }
        self.mounted_boat = Some(boat_idx);
        self.sync_player_to_boat();
        true
    }

    fn try_dismount_boat(&mut self) -> bool {
        let Some(boat_idx) = self.mounted_boat else {
            return false;
        };
        let Some(boat) = self.boats.get(boat_idx) else {
            self.mounted_boat = None;
            return false;
        };
        let candidates = [
            (boat.x + 1.05, boat.y),
            (boat.x - 1.05, boat.y),
            (boat.x, boat.y - 0.9),
        ];
        for (x, y) in candidates {
            if !self.entity_collides_with_world(x, y, PLAYER_HALF_WIDTH, PLAYER_HEIGHT) {
                self.player.x = x;
                self.player.y = y;
                self.player.vx = 0.0;
                self.player.vy = 0.0;
                self.player.grounded = false;
                self.mounted_boat = None;
                return true;
            }
        }
        false
    }

    fn remove_boat_with_drop(&mut self, boat_idx: usize) {
        if boat_idx >= self.boats.len() {
            return;
        }
        let boat = self.boats.swap_remove(boat_idx);
        self.each_player(|s| {
            if let Some(mounted_idx) = s.mounted_boat {
                if mounted_idx == boat_idx {
                    s.mounted_boat = None;
                } else if mounted_idx == s.boats.len() {
                    s.mounted_boat = Some(boat_idx);
                }
            }
        });
        self.item_entities
            .push(ItemEntity::new(boat.x, boat.y - 0.2, ItemType::Boat));
    }

    fn update_boats(&mut self) {
        let active = self.active_peer;
        for idx in 0..self.boats.len() {
            if let Some(y) = self.recover_loose_void_fall(self.boats[idx].x, self.boats[idx].y) {
                self.boats[idx].y = y;
                self.boats[idx].vx = 0.0;
                self.boats[idx].vy = 0.0;
            }
            let e = &self.boats[idx];
            if !self.body_terrain_loaded(e.x, e.y, e.vx, 0.75) {
                continue;
            }
            if self.cooperative() {
                let rider = self.peer_ids_here().into_iter().find(|id| {
                    self.peer_session(*id)
                        .is_some_and(|p| p.mounted_boat == Some(idx))
                });
                if let Some(id) = rider {
                    self.activate_peer(id);
                }
            }
            let input_dir = if self.inventory_open || self.moving_left == self.moving_right {
                0.0
            } else if self.moving_left {
                -1.0
            } else {
                1.0
            };
            let mounted = self.mounted_boat == Some(idx);
            let (mut x, mut y, mut vx, mut vy, mut grounded, mut facing_right, mut wobble_timer) = {
                let boat = &self.boats[idx];
                (
                    boat.x,
                    boat.y,
                    boat.vx,
                    boat.vy,
                    boat.grounded,
                    boat.facing_right,
                    boat.wobble_timer,
                )
            };
            wobble_timer = wobble_timer.saturating_sub(1);

            if let Some(surface_y) = self.boat_surface_y_near(x, y) {
                if mounted && input_dir != 0.0 {
                    let desired_vx = input_dir * BOAT_WATER_SPEED;
                    vx += (desired_vx - vx) * BOAT_ACCEL;
                    facing_right = input_dir > 0.0;
                } else {
                    vx *= BOAT_WATER_DRAG;
                }
                let next_x = x + vx;
                if !self.entity_collides_with_world(next_x, surface_y, BOAT_HALF_WIDTH, BOAT_HEIGHT)
                {
                    x = next_x;
                } else {
                    vx = 0.0;
                    wobble_timer = 8;
                }
                y = surface_y;
                vy = 0.0;
                grounded = false;
            } else {
                let (nx, ny, nvx, nvy, ngr, _) = self.calculate_movement_with_jump_held(
                    x,
                    y,
                    vx,
                    vy,
                    grounded,
                    BOAT_HALF_WIDTH,
                    BOAT_HEIGHT,
                    false,
                    false,
                );
                x = nx;
                y = ny;
                vx = nvx * 0.86;
                vy = nvy;
                grounded = ngr;
                if vx.abs() < 0.01 {
                    vx = 0.0;
                }
            }

            let boat = &mut self.boats[idx];
            boat.x = x;
            boat.y = y;
            boat.vx = vx;
            boat.vy = vy;
            boat.grounded = grounded;
            boat.facing_right = facing_right;
            boat.wobble_timer = wobble_timer;
            if mounted {
                self.sync_player_to_boat();
            }
        }
        if let Some(id) = active {
            self.activate_peer(id);
        }

        if self.mounted_boat.is_some() {
            self.sync_player_to_boat();
        }
    }

    fn try_shear_sheep(&mut self, sheep_idx: usize) -> bool {
        if self.current_hotbar_item_type() != Some(ItemType::Shears) {
            return false;
        }
        let Some(sheep) = self.sheep.get_mut(sheep_idx) else {
            return false;
        };
        if sheep.sheared || sheep.husbandry.child_ticks > 0 {
            return false;
        }

        sheep.sheared = true;
        sheep.hit_timer = 6;
        sheep.vy = -0.08;
        sheep.vx = if self.player.x < sheep.x { 0.16 } else { -0.16 };
        let drop_x = sheep.x;
        let drop_y = sheep.y - 0.5;
        let wool_count = rand::thread_rng().gen_range(1..=3);
        for _ in 0..wool_count {
            self.item_entities
                .push(ItemEntity::new(drop_x, drop_y, ItemType::Wool));
        }
        true
    }

    fn has_fire_resistance(&self) -> bool {
        self.potion_fire_resistance_timer > 0
    }

    fn consume_held_potion_if_any(&mut self) -> bool {
        let slot_idx = self.hotbar_index as usize;
        let Some(item_type) = self.inventory.slots[slot_idx]
            .as_ref()
            .map(|stack| stack.item_type)
        else {
            return false;
        };
        let mut drank = true;
        match item_type {
            ItemType::WaterBottle | ItemType::AwkwardPotion => {}
            ItemType::PotionHealing => {
                self.player.health =
                    (self.player.health + POTION_HEALING_INSTANT_HP).min(self.player.max_health);
            }
            ItemType::PotionStrength => {
                self.potion_strength_timer = POTION_STRENGTH_DURATION_TICKS;
            }
            ItemType::PotionRegeneration => {
                self.potion_regeneration_timer = POTION_REGEN_DURATION_TICKS;
            }
            ItemType::PotionFireResistance => {
                self.potion_fire_resistance_timer = POTION_FIRE_RESIST_DURATION_TICKS;
                self.player.burning_timer = 0;
            }
            _ => {
                drank = false;
            }
        }
        if !drank {
            return false;
        }
        if !self.consume_held_item(item_type) {
            return false;
        }
        self.add_item_to_inventory_or_drop(ItemType::GlassBottle, 1);
        true
    }

    fn apply_inventory_slot_durability_wear(&mut self, slot_idx: usize) {
        let enchant_roll = self.inventory_enchant_levels[slot_idx].min(ENCHANT_MAX_LEVEL) as u32;
        if enchant_roll > 0 && rand::thread_rng().gen_range(0..=enchant_roll) > 0 {
            return;
        }
        let Some(stack) = self.inventory.slots[slot_idx].as_mut() else {
            self.inventory_enchant_levels[slot_idx] = 0;
            return;
        };
        let Some(durability) = stack.durability.as_mut() else {
            return;
        };
        if *durability > 0 {
            *durability -= 1;
        }
        if *durability == 0 {
            self.inventory.slots[slot_idx] = None;
            self.inventory_enchant_levels[slot_idx] = 0;
        }
    }

    fn current_hotbar_item_type(&self) -> Option<ItemType> {
        self.inventory.slots[self.hotbar_index as usize]
            .as_ref()
            .map(|stack| stack.item_type)
    }

    fn clear_fishing_line(&mut self) {
        self.fishing_bobber_active = false;
        self.fishing_wait_ticks = 0;
        self.fishing_bite_window_ticks = 0;
    }

    fn next_fishing_wait_ticks(&self, rng: &mut impl Rng) -> u16 {
        let mut min_wait = FISHING_WAIT_MIN_TICKS;
        let mut max_wait = FISHING_WAIT_MAX_TICKS;
        if self.current_dimension == Dimension::Overworld
            && self.weather != WeatherType::Clear
            && self.is_exposed_to_sky(self.fishing_bobber_x, self.fishing_bobber_y - 0.2)
        {
            min_wait = min_wait.saturating_sub(8);
            max_wait = max_wait.saturating_sub(14);
        }
        rng.gen_range(min_wait..=max_wait)
    }

    fn fishing_loot_category_weights_for(
        biome: Option<BiomeType>,
        weather: WeatherType,
        sky_exposed: bool,
    ) -> [u16; 3] {
        let mut fish_weight = FISHING_BASE_FISH_WEIGHT;
        let mut junk_weight = FISHING_BASE_JUNK_WEIGHT;
        let mut treasure_weight = FISHING_BASE_TREASURE_WEIGHT;

        if let Some(biome) = biome {
            match biome {
                BiomeType::Ocean | BiomeType::River => {
                    fish_weight += 8;
                    junk_weight -= 6;
                    treasure_weight -= 2;
                }
                BiomeType::Swamp => {
                    fish_weight -= 8;
                    junk_weight += 8;
                }
                BiomeType::Jungle => {
                    fish_weight -= 4;
                    junk_weight += 2;
                    treasure_weight += 2;
                }
                BiomeType::Taiga | BiomeType::Tundra | BiomeType::ExtremeHills => {
                    fish_weight -= 3;
                    junk_weight -= 1;
                    treasure_weight += 4;
                }
                BiomeType::Desert | BiomeType::Forest | BiomeType::Plains => {}
            }
        }

        if sky_exposed {
            match weather {
                WeatherType::Rain => {
                    fish_weight += 2;
                    junk_weight -= 1;
                    treasure_weight -= 1;
                }
                WeatherType::Thunderstorm => {
                    fish_weight -= 2;
                    junk_weight -= 2;
                    treasure_weight += 4;
                }
                WeatherType::Clear => {}
            }
        }

        [
            fish_weight.max(1) as u16,
            junk_weight.max(1) as u16,
            treasure_weight.max(1) as u16,
        ]
    }

    fn fishing_loot_category_weights(&self) -> [u16; 3] {
        let bobber_x = self.fishing_bobber_x.floor() as i32;
        let bobber_y = self.fishing_bobber_y - 0.2;
        let biome = (self.current_dimension == Dimension::Overworld)
            .then(|| self.world.get_biome(bobber_x));
        let sky_exposed = self.current_dimension == Dimension::Overworld
            && self.is_exposed_to_sky(self.fishing_bobber_x, bobber_y);
        Self::fishing_loot_category_weights_for(biome, self.weather, sky_exposed)
    }

    fn roll_weighted_loot_table(
        rng: &mut impl Rng,
        table: &[(ItemType, u32, u16)],
    ) -> (ItemType, u32) {
        let total_weight = table
            .iter()
            .map(|(_, _, weight)| u32::from(*weight))
            .sum::<u32>();
        let mut roll = rng.gen_range(0..total_weight);
        for (item_type, count, weight) in table {
            let weight = u32::from(*weight);
            if roll < weight {
                return (*item_type, *count);
            }
            roll -= weight;
        }
        let (fallback_item, fallback_count, _) = table[table.len() - 1];
        (fallback_item, fallback_count)
    }

    fn roll_fishing_loot_category(&self, rng: &mut impl Rng) -> FishingLootCategory {
        let [fish_weight, junk_weight, treasure_weight] = self.fishing_loot_category_weights();
        let total_weight =
            u32::from(fish_weight) + u32::from(junk_weight) + u32::from(treasure_weight);
        let roll = rng.gen_range(0..total_weight);
        if roll < u32::from(fish_weight) {
            FishingLootCategory::Fish
        } else if roll < u32::from(fish_weight) + u32::from(junk_weight) {
            FishingLootCategory::Junk
        } else {
            FishingLootCategory::Treasure
        }
    }

    fn roll_fishing_loot_from_category(
        &self,
        category: FishingLootCategory,
        rng: &mut impl Rng,
    ) -> (ItemType, u32) {
        match category {
            FishingLootCategory::Fish => {
                Self::roll_weighted_loot_table(rng, &FISHING_FISH_LOOT_TABLE)
            }
            FishingLootCategory::Junk => {
                Self::roll_weighted_loot_table(rng, &FISHING_JUNK_LOOT_TABLE)
            }
            FishingLootCategory::Treasure => {
                Self::roll_weighted_loot_table(rng, &FISHING_TREASURE_LOOT_TABLE)
            }
        }
    }

    fn roll_fishing_loot(&self, rng: &mut impl Rng) -> (ItemType, u32) {
        let category = self.roll_fishing_loot_category(rng);
        self.roll_fishing_loot_from_category(category, rng)
    }

    fn cast_fishing_line(&mut self, bx: i32, by: i32, rng: &mut impl Rng) {
        self.fishing_bobber_active = true;
        self.fishing_bobber_x = bx as f64 + 0.5;
        self.fishing_bobber_y = by as f64 + 0.35;
        self.fishing_bite_window_ticks = 0;
        self.fishing_wait_ticks = self.next_fishing_wait_ticks(rng);
        self.player.facing_right = self.fishing_bobber_x >= self.player.x;
    }

    fn reel_fishing_line(&mut self, rng: &mut impl Rng) {
        if !self.fishing_bobber_active {
            return;
        }
        let bite_ready = self.fishing_bite_window_ticks > 0;
        self.clear_fishing_line();
        if !bite_ready {
            return;
        }
        let (loot_item, loot_count) = self.roll_fishing_loot(rng);
        self.add_item_to_inventory_or_drop(loot_item, loot_count);
        if self.current_hotbar_item_type() == Some(ItemType::FishingRod) {
            self.apply_inventory_slot_durability_wear(self.hotbar_index as usize);
        }
        // Small random pullback helps catches feel less static in terminal view.
        let pull_x = if rng.gen_bool(0.5) { -0.06 } else { 0.06 };
        self.player.vx = (self.player.vx + pull_x).clamp(-0.9, 0.9);
    }

    fn update_fishing_state(&mut self, rng: &mut impl Rng) {
        if !self.fishing_bobber_active {
            return;
        }
        if self.current_hotbar_item_type() != Some(ItemType::FishingRod) {
            self.clear_fishing_line();
            return;
        }
        if self.inventory_open {
            self.clear_fishing_line();
            return;
        }
        let block = self.world.get_block(
            self.fishing_bobber_x.floor() as i32,
            self.fishing_bobber_y.floor() as i32,
        );
        if !matches!(block, BlockType::Water(_)) {
            self.clear_fishing_line();
            return;
        }
        let line_dist = ((self.fishing_bobber_x - self.player.x).powi(2)
            + (self.fishing_bobber_y - (self.player.y - 1.0)).powi(2))
        .sqrt();
        if line_dist > FISHING_MAX_LINE_DISTANCE {
            self.clear_fishing_line();
            return;
        }
        if self.fishing_bite_window_ticks > 0 {
            self.fishing_bite_window_ticks -= 1;
            if self.fishing_bite_window_ticks == 0 {
                self.fishing_wait_ticks = self.next_fishing_wait_ticks(rng);
            }
            return;
        }
        if self.fishing_wait_ticks > 0 {
            self.fishing_wait_ticks -= 1;
        }
        if self.fishing_wait_ticks == 0 {
            self.fishing_bite_window_ticks = FISHING_BITE_WINDOW_TICKS;
        }
    }

    pub(super) fn bow_is_drawing(&self) -> bool {
        self.bow_draw_active
    }

    fn update_bow_draw_state(&mut self, mouse_target_x: i32, mouse_target_y: i32) -> bool {
        let holding_bow = self.current_hotbar_item_type() == Some(ItemType::Bow);
        let can_use_bow = holding_bow && !self.inventory_open;

        if can_use_bow && self.left_click_down && self.inventory.has_item(ItemType::Arrow, 1) {
            self.bow_draw_ticks = self
                .bow_draw_ticks
                .saturating_add(1)
                .min(BOW_MAX_DRAW_TICKS);
            self.bow_draw_active = true;
            self.show_action(
                super::action::ActionKind::BowDraw,
                Some(ItemType::Bow),
                mouse_target_x,
                mouse_target_y,
                2,
                self.bow_draw_ticks as f64 / BOW_MAX_DRAW_TICKS as f64,
            );
            self.player.mining_timer = 0.0;
            return true;
        }

        if self.bow_draw_active {
            if can_use_bow && !self.left_click_down {
                self.release_bow_shot(mouse_target_x, mouse_target_y);
            }
            self.bow_draw_active = false;
            self.bow_draw_ticks = 0;
            return can_use_bow;
        }

        can_use_bow
    }

    fn release_bow_shot(&mut self, mouse_target_x: i32, mouse_target_y: i32) {
        let draw_ticks = self.bow_draw_ticks.min(BOW_MAX_DRAW_TICKS);
        if draw_ticks < BOW_MIN_DRAW_TICKS || !self.inventory.has_item(ItemType::Arrow, 1) {
            return;
        }
        if self.current_hotbar_item_type() != Some(ItemType::Bow) {
            return;
        }

        let charge = draw_ticks as f64 / BOW_MAX_DRAW_TICKS as f64;
        let power = ((charge * charge) + (charge * 2.0)) / 3.0;
        let power = power.clamp(0.15, 1.0);

        let origin_y = self.player.y - 1.0;
        let target_x = mouse_target_x as f64 + 0.5;
        let target_y = mouse_target_y as f64 + 0.5;
        let mut dx = target_x - self.player.x;
        let mut dy = target_y - origin_y;
        let mut dist = (dx * dx + dy * dy).sqrt();
        if dist <= 0.001 {
            dx = if self.player.facing_right { 1.0 } else { -1.0 };
            dy = 0.0;
            dist = 1.0;
        }
        let dir_x = dx / dist;
        let dir_y = dy / dist;

        self.player.facing_right = dir_x >= 0.0;
        let spawn_x = self.player.x + dir_x * 0.35;
        let speed = 0.22 + 0.62 * power;
        let bow_enchant = self.selected_hotbar_enchant_level() as f32;
        let arrow_damage = 2.0 + (power as f32 * 6.0) + bow_enchant;
        let mut arrow = Arrow::new_player(
            spawn_x,
            origin_y,
            dir_x * speed,
            dir_y * speed,
            arrow_damage,
        );
        arrow.owner = self.active_peer;
        self.arrows.push(arrow);

        self.inventory.remove_item(ItemType::Arrow, 1);
        if self.inventory.slots[self.hotbar_index as usize]
            .as_ref()
            .is_some_and(|stack| stack.item_type == ItemType::Bow)
        {
            self.apply_inventory_slot_durability_wear(self.hotbar_index as usize);
        }

        self.player.attack_timer = 8;
        self.show_action(
            super::action::ActionKind::BowRelease,
            Some(ItemType::Bow),
            mouse_target_x,
            mouse_target_y,
            8,
            0.0,
        );
        self.player.mining_timer = 0.0;
    }

    #[cfg(test)]
    fn try_apply_player_arrow_hit(
        &mut self,
        ax: f64,
        ay: f64,
        _vx: f64,
        _vy: f64,
        damage: f32,
    ) -> bool {
        self.hit_player_arrow_segment((ax, ay), (ax, ay), damage, f64::INFINITY)
    }

    fn provoke_wolves_near(&mut self, center_x: f64, center_y: f64, radius: f64) {
        for wolf in &mut self.wolves {
            let dx = wolf.x - center_x;
            let dy = wolf.y - center_y;
            if dx * dx + dy * dy <= radius * radius {
                wolf.provoke();
            }
        }
    }

    fn spook_ocelots_near(&mut self, center_x: f64, center_y: f64, radius: f64, away_from_x: f64) {
        for ocelot in &mut self.ocelots {
            let dx = ocelot.x - center_x;
            let dy = ocelot.y - center_y;
            if dx * dx + dy * dy <= radius * radius {
                ocelot.spook_from(away_from_x);
            }
        }
    }

    fn throw_eye_of_ender_for_guidance(&mut self) {
        if self.current_dimension != Dimension::Overworld {
            return;
        }
        if !self.consume_held_item(ItemType::EyeOfEnder) {
            return;
        }

        let target_x = STRONGHOLD_CENTER_X as f64 + 0.5;
        let dx = target_x - self.player.x;
        self.eye_guidance_dir = if dx >= 0.0 { 1 } else { -1 };
        self.eye_guidance_distance = dx.abs().round() as i32;
        self.eye_guidance_timer = 20 * 15;
        self.player.facing_right = self.eye_guidance_dir > 0;

        let mut rng = rand::thread_rng();
        // Mirrors vanilla behavior where eyes can shatter after use.
        if rng.gen_bool(0.8) {
            let mut dropped_eye = ItemEntity::new(
                self.player.x + self.eye_guidance_dir as f64 * 2.0,
                (self.player.y - 2.4).max(1.0),
                ItemType::EyeOfEnder,
            );
            dropped_eye.vx = self.eye_guidance_dir as f64 * 0.18;
            dropped_eye.vy = -0.28;
            self.item_entities.push(dropped_eye);
        }
    }

    pub fn eye_guidance_hint(&self) -> Option<String> {
        if self.eye_guidance_timer == 0 || self.current_dimension != Dimension::Overworld {
            return None;
        }
        let dx = STRONGHOLD_CENTER_X as f64 + 0.5 - self.player.x;
        let dy = STRONGHOLD_PORTAL_INNER_Y as f64 + 1.0 - (self.player.y - 0.9);
        if dx.abs() > 8.0 {
            Some(format!(
                "Eye: stronghold ~{} blocks {}",
                dx.abs().round() as i32,
                if dx > 0.0 { "right" } else { "left" },
            ))
        } else if dy.abs() > 4.0 {
            Some(format!(
                "Eye: portal ~{} blocks {}",
                dy.abs().round() as i32,
                if dy > 0.0 { "below" } else { "above" },
            ))
        } else {
            Some("Eye: portal room nearby; fill the frame with Eyes".to_string())
        }
    }

    fn has_adjacent_placement_anchor(&self, bx: i32, by: i32) -> bool {
        !self.world.get_block(bx - 1, by).is_replaceable()
            || !self.world.get_block(bx + 1, by).is_replaceable()
            || !self.world.get_block(bx, by - 1).is_replaceable()
            || !self.world.get_block(bx, by + 1).is_replaceable()
    }

    fn has_adjacent_solid(&self, bx: i32, by: i32) -> bool {
        self.world.get_block(bx - 1, by).is_solid()
            || self.world.get_block(bx + 1, by).is_solid()
            || self.world.get_block(bx, by - 1).is_solid()
            || self.world.get_block(bx, by + 1).is_solid()
    }

    fn has_valid_bottom_support_for_block(&self, bx: i32, by: i32, block: BlockType) -> bool {
        if block == BlockType::SugarCane {
            return self.world.can_support_sugar_cane_at(bx, by);
        }
        !block.needs_bottom_support() || block.can_stay_on(self.world.get_block(bx, by + 1))
    }

    fn clear_tilling_cover_block(&mut self, bx: i32, by: i32, block: BlockType) {
        match block {
            BlockType::TallGrass => {
                if tall_grass_drops_seed_at(bx, by) {
                    self.add_item_to_inventory_or_drop(ItemType::WheatSeeds, 1);
                }
            }
            BlockType::Air => {}
            other => {
                if let Some(item) = ItemType::from_block(other) {
                    self.add_item_to_inventory_or_drop(item, 1);
                }
            }
        }
        self.world.set_block(bx, by, BlockType::Air);
    }

    fn door_kind(block: BlockType) -> Option<(DoorKind, bool)> {
        match block {
            BlockType::WoodDoor(open) => Some((DoorKind::Wood, open)),
            BlockType::IronDoor(open) => Some((DoorKind::Iron, open)),
            _ => None,
        }
    }

    fn door_block(kind: DoorKind, open: bool) -> BlockType {
        match kind {
            DoorKind::Wood => BlockType::WoodDoor(open),
            DoorKind::Iron => BlockType::IronDoor(open),
        }
    }

    fn door_blocks_at(&self, bx: i32, by: i32) -> Option<(DoorKind, Vec<(i32, i32)>)> {
        let (kind, _) = Self::door_kind(self.world.get_block(bx, by))?;
        let mut blocks = vec![(bx, by)];
        for neighbor_y in [by - 1, by + 1] {
            if Self::door_kind(self.world.get_block(bx, neighbor_y))
                .is_some_and(|(neighbor_kind, _)| neighbor_kind == kind)
            {
                blocks.push((bx, neighbor_y));
            }
        }
        blocks.sort_unstable_by_key(|&(_, y)| y);
        blocks.dedup();
        Some((kind, blocks))
    }

    fn door_base_at(&self, bx: i32, by: i32) -> Option<(DoorKind, i32)> {
        let (kind, blocks) = self.door_blocks_at(bx, by)?;
        let base_y = blocks.iter().map(|&(_, y)| y).max()?;
        Some((kind, base_y))
    }

    fn set_door_open_state(&mut self, bx: i32, by: i32, open: bool) -> bool {
        let Some((kind, blocks)) = self.door_blocks_at(bx, by) else {
            return false;
        };
        let block = Self::door_block(kind, open);
        for (door_x, door_y) in blocks {
            self.world.set_block(door_x, door_y, block);
        }
        true
    }

    fn mark_villager_door_active(&mut self, bx: i32, by: i32) {
        let Some((DoorKind::Wood, base_y)) = self.door_base_at(bx, by) else {
            return;
        };
        self.villager_open_doors
            .insert((bx, base_y), VILLAGER_DOOR_HOLD_TICKS);
    }

    fn entity_overlaps_doorway(&self, x: f64, y: f64, door_x: i32, base_y: i32) -> bool {
        // y is the feet line; base_y is the lower door block, not its floor.
        // Include the whole body so a door cannot close on a shoulder or foot.
        x + 0.3 > door_x as f64
            && x - 0.3 < door_x as f64 + 1.0
            && y > base_y as f64 - 1.0
            && y - PLAYER_HEIGHT < base_y as f64 + 1.0
    }

    fn doorway_is_occupied(&self, door_x: i32, base_y: i32) -> bool {
        if self.player.health > 0.0
            && self.entity_overlaps_doorway(self.player.x, self.player.y, door_x, base_y)
        {
            return true;
        }
        if self.other_players_here().any(|actor| {
            actor.player.health > 0.0
                && self.entity_overlaps_doorway(actor.player.x, actor.player.y, door_x, base_y)
        }) {
            return true;
        }
        self.villagers.iter().any(|villager| {
            villager.health > 0.0
                && self.entity_overlaps_doorway(villager.x, villager.y, door_x, base_y)
        })
    }

    fn refresh_villager_door_hold_for_entity(&mut self, x: f64, y: f64) {
        let bx = x.floor() as i32;
        let feet_y = y.floor() as i32;
        for probe_x in (bx - 1)..=(bx + 1) {
            for by in [feet_y + 1, feet_y, feet_y - 1] {
                if self.world.get_block(probe_x, by) != BlockType::WoodDoor(true) {
                    continue;
                }
                let Some((DoorKind::Wood, base_y)) = self.door_base_at(probe_x, by) else {
                    continue;
                };
                if self.entity_overlaps_doorway(x, y, probe_x, base_y) {
                    self.mark_villager_door_active(probe_x, by);
                }
            }
        }
    }

    fn try_close_wood_door_behind_entity(&mut self, x: f64, y: f64) {
        let bx = x.floor() as i32;
        let feet_y = y.floor() as i32;
        let mut nearby_open_doors = Vec::new();
        for probe_x in (bx - 2)..=(bx + 2) {
            for by in [feet_y + 1, feet_y, feet_y - 1] {
                if self.world.get_block(probe_x, by) != BlockType::WoodDoor(true) {
                    continue;
                }
                let Some((DoorKind::Wood, base_y)) = self.door_base_at(probe_x, by) else {
                    continue;
                };
                nearby_open_doors.push((probe_x, base_y));
            }
        }
        nearby_open_doors.sort_unstable();
        nearby_open_doors.dedup();

        for (door_x, base_y) in nearby_open_doors {
            if self.entity_overlaps_doorway(x, y, door_x, base_y) {
                continue;
            }
            if (x - (door_x as f64 + 0.5)).abs() < 0.55 {
                continue;
            }
            if self.doorway_is_occupied(door_x, base_y) {
                continue;
            }
            // Each villager uses the same doorway timer. A leading villager
            // must not close it immediately in the next villager's face.
            self.villager_open_doors
                .entry((door_x, base_y))
                .or_insert(VILLAGER_DOOR_HOLD_TICKS);
        }
    }

    fn tick_villager_door_hold_timers(&mut self) {
        if self.current_dimension != Dimension::Overworld {
            self.villager_open_doors.clear();
            return;
        }

        let tracked_doors: Vec<(i32, i32)> = self.villager_open_doors.keys().copied().collect();
        let mut doors_to_close = Vec::new();
        for (door_x, base_y) in tracked_doors {
            let Some((DoorKind::Wood, actual_base_y)) = self.door_base_at(door_x, base_y) else {
                self.villager_open_doors.remove(&(door_x, base_y));
                continue;
            };
            if actual_base_y != base_y
                || self.world.get_block(door_x, base_y) != BlockType::WoodDoor(true)
            {
                self.villager_open_doors.remove(&(door_x, base_y));
                continue;
            }
            if self.doorway_is_occupied(door_x, base_y) {
                continue;
            }
            let Some(hold_ticks) = self.villager_open_doors.get_mut(&(door_x, base_y)) else {
                continue;
            };
            if *hold_ticks > 0 {
                *hold_ticks -= 1;
            }
            if *hold_ticks == 0 {
                doors_to_close.push((door_x, base_y));
            }
        }

        for (door_x, base_y) in doors_to_close {
            if !self.doorway_is_occupied(door_x, base_y) {
                let _ = self.set_door_open_state(door_x, base_y, false);
            }
            self.villager_open_doors.remove(&(door_x, base_y));
        }
    }

    fn remove_door_blocks(&mut self, bx: i32, by: i32) -> bool {
        let Some((_, blocks)) = self.door_blocks_at(bx, by) else {
            return false;
        };
        for (door_x, door_y) in blocks {
            self.world.set_block(door_x, door_y, BlockType::Air);
        }
        true
    }

    fn place_wood_door(&mut self, bx: i32, bottom_y: i32) -> bool {
        if bottom_y <= 1 {
            return false;
        }
        if !self.world.get_block(bx, bottom_y + 1).is_solid() {
            return false;
        }
        if !self.world.get_block(bx, bottom_y).is_replaceable()
            || !self.world.get_block(bx, bottom_y - 1).is_replaceable()
        {
            return false;
        }
        if self.block_intersects_player_bounds(bx, bottom_y)
            || self.block_intersects_player_bounds(bx, bottom_y - 1)
        {
            return false;
        }

        self.world
            .set_block(bx, bottom_y, BlockType::WoodDoor(false));
        self.world
            .set_block(bx, bottom_y - 1, BlockType::WoodDoor(false));
        true
    }

    fn resolve_block_placement_target(&self, clicked_x: i32, clicked_y: i32) -> Option<(i32, i32)> {
        let clicked_block = self.world.get_block(clicked_x, clicked_y);
        // Plant on top of the soil that was clicked, regardless of approach angle.
        if matches!(
            self.current_hotbar_item_type(),
            Some(ItemType::WheatSeeds | ItemType::NetherWart)
        ) && !clicked_block.is_replaceable()
        {
            return self
                .world
                .get_block(clicked_x, clicked_y - 1)
                .is_replaceable()
                .then_some((clicked_x, clicked_y - 1));
        }
        if clicked_block.is_replaceable() {
            if self.has_adjacent_placement_anchor(clicked_x, clicked_y) {
                return Some((clicked_x, clicked_y));
            }
            // Solid blocks aim through water to a reachable submerged face,
            // just as they do through air. Water itself never supports a block.
            if matches!(clicked_block, BlockType::Water(_))
                && self
                    .current_hotbar_item_type()
                    .and_then(|item| item.to_block())
                    .is_some_and(|b| b.is_solid())
            {
                let origin = (self.player.x, self.player.y - 1.0);
                let delta = (
                    clicked_x as f64 + 0.5 - origin.0,
                    clicked_y as f64 + 0.5 - origin.1,
                );
                let length = delta.0.hypot(delta.1);
                if length > 0.001 {
                    let dir = (delta.0 / length, delta.1 / length);
                    if let Some(hit) = self.terrain_segment_hit(
                        origin.0,
                        origin.1,
                        origin.0 + dir.0 * 4.5,
                        origin.1 + dir.1 * 4.5,
                        false,
                    ) {
                        let target = (
                            (hit.0 - dir.0 * 0.01).floor() as i32,
                            (hit.1 - dir.1 * 0.01).floor() as i32,
                        );
                        if matches!(
                            self.world.get_block(target.0, target.1),
                            BlockType::Water(_)
                        ) && self.has_adjacent_placement_anchor(target.0, target.1)
                            && (target.0 as f64 + 0.5 - origin.0)
                                .hypot(target.1 as f64 + 0.5 - origin.1)
                                <= 4.5
                        {
                            return Some(target);
                        }
                    }
                }
            }
            return None;
        }

        let click_center_x = clicked_x as f64 + 0.5;
        let click_center_y = clicked_y as f64 + 0.5;
        let dx = self.player.x - click_center_x;
        let dy = (self.player.y - 1.0) - click_center_y;

        let horizontal_first = dx.abs() >= dy.abs();
        let horizontal_dir = if dx >= 0.0 { 1 } else { -1 };
        let vertical_dir = if dy >= 0.0 { 1 } else { -1 };
        let candidate_offsets = if horizontal_first {
            [
                (horizontal_dir, 0),
                (0, vertical_dir),
                (0, -vertical_dir),
                (-horizontal_dir, 0),
            ]
        } else {
            [
                (0, vertical_dir),
                (horizontal_dir, 0),
                (-horizontal_dir, 0),
                (0, -vertical_dir),
            ]
        };

        // A bucket must use the face the player aimed at. Falling back to the
        // opposite face can put water into an unseen pocket underneath a field.
        let candidate_count = if matches!(
            self.current_hotbar_item_type(),
            Some(ItemType::WaterBucket | ItemType::LavaBucket)
        ) {
            1
        } else {
            4
        };
        candidate_offsets
            .into_iter()
            .take(candidate_count)
            .find_map(|(ox, oy)| {
                let nx = clicked_x + ox;
                let ny = clicked_y + oy;
                self.world
                    .get_block(nx, ny)
                    .is_replaceable()
                    .then_some((nx, ny))
                    .filter(|(tx, ty)| self.has_adjacent_placement_anchor(*tx, *ty))
            })
    }

    fn entity_intersects_block(
        x: f64,
        y: f64,
        half_width: f64,
        height: f64,
        bx: i32,
        by: i32,
    ) -> bool {
        let px_min = x - half_width;
        let px_max = x + half_width;
        let py_min = y - height;
        let py_max = y;
        let bx_min = bx as f64;
        let bx_max = bx as f64 + 1.0;
        let by_min = by as f64;
        let by_max = by as f64 + 1.0;
        px_min < bx_max && px_max > bx_min && py_min < by_max && py_max > by_min
    }

    fn can_place_block_under_rising_player(&self, by: i32) -> bool {
        if self.player.vy >= -0.12 {
            return false;
        }
        if by != self.player.y.floor() as i32 {
            return false;
        }

        let foot_overlap = self.player.y - by as f64;
        (0.05..=0.45).contains(&foot_overlap)
    }

    fn block_intersects_player_bounds(&self, bx: i32, by: i32) -> bool {
        Self::entity_intersects_block(
            self.player.x,
            self.player.y,
            PLAYER_HALF_WIDTH,
            PLAYER_HEIGHT,
            bx,
            by,
        ) && !self.can_place_block_under_rising_player(by)
            || self.other_players_here().any(|p| {
                Self::entity_intersects_block(
                    p.player.x,
                    p.player.y,
                    PLAYER_HALF_WIDTH,
                    PLAYER_HEIGHT,
                    bx,
                    by,
                )
            })
    }

    fn is_player_on_ladder(&self) -> bool {
        self.is_entity_on_ladder(self.player.x, self.player.y, PLAYER_HEIGHT)
    }

    fn is_entity_on_ladder(&self, x: f64, y: f64, height: f64) -> bool {
        let px = x.floor() as i32;
        let y_checks = [
            y.floor() as i32,
            (y - (height * 0.5)).floor() as i32,
            (y - (height - 0.2)).floor() as i32,
        ];
        y_checks
            .iter()
            .any(|&py| self.world.get_block(px, py) == BlockType::Ladder)
    }

    fn entity_fluid_submersion(&self, x: f64, y: f64, half_width: f64, height: f64) -> (f64, f64) {
        let side_sample = (half_width - 0.02).max(0.0);
        let sample_xs = [x - side_sample, x, x + side_sample];
        let mut water = [0.0_f64; 3];
        let mut lava = [0.0_f64; 3];
        let top = y - height;
        for (x_idx, sample_x) in sample_xs.into_iter().enumerate() {
            let bx = sample_x.floor() as i32;
            for by in top.floor() as i32..=(y - 0.000001).floor() as i32 {
                if let Some(surface) = self.world.fluid_surface_y(bx, by) {
                    let overlap = (y.min(by as f64 + 1.0) - top.max(surface)).max(0.0);
                    match self.world.get_block(bx, by) {
                        BlockType::Water(_) => water[x_idx] += overlap / height,
                        BlockType::Lava(_) => lava[x_idx] += overlap / height,
                        _ => {}
                    }
                }
            }
            // Start drag on contact with the visible surface without treating
            // touching feet as one third of a submerged body.
            match self.world.fluid_at_point(sample_x, y) {
                BlockType::Water(_) => water[x_idx] = water[x_idx].max(0.01),
                BlockType::Lava(_) => lava[x_idx] = lava[x_idx].max(0.01),
                _ => {}
            }
        }
        (
            (water.iter().sum::<f64>() / 3.0)
                .max(water[1])
                .clamp(0.0, 1.0),
            (lava.iter().sum::<f64>() / 3.0)
                .max(lava[1])
                .clamp(0.0, 1.0),
        )
    }

    fn entity_touches_fluid(&self, x: f64, y: f64, half_width: f64, height: f64) -> bool {
        let (water_submersion, lava_submersion) =
            self.entity_fluid_submersion(x, y, half_width, height);
        water_submersion > 0.0 || lava_submersion > 0.0
    }

    fn can_melee_entity(&self, target_x: f64, target_y: f64, max_distance: f64) -> bool {
        let origin_x = self.player.x;
        let origin_y = self.player.y - 0.9;
        let dx = target_x - origin_x;
        let dy = target_y - origin_y;
        if (dx * dx + dy * dy) > max_distance * max_distance {
            return false;
        }
        self.terrain_segment_hit(origin_x, origin_y, target_x, target_y, false)
            .is_none()
    }

    pub(crate) fn can_see_block(&self, x: f64, y: f64, bx: i32, by: i32) -> bool {
        // Sample the interior of visible faces, never a zero-width corner gap.
        // A center-only ray would reject exposed sides of stairs and frames.
        self.has_line_of_sight(x, y, bx as f64 + 0.5, by as f64 + 0.5)
            || [(0.5, 0.04), (0.04, 0.5), (0.96, 0.5), (0.5, 0.96)]
                .into_iter()
                .any(|(dx, dy)| self.has_line_of_sight(x, y, bx as f64 + dx, by as f64 + dy))
            || self.has_line_of_sight(
                x,
                y,
                x.clamp(bx as f64 + 0.04, bx as f64 + 0.96),
                y.clamp(by as f64 + 0.04, by as f64 + 0.96),
            )
    }

    fn has_line_of_sight(&self, x0: f64, y0: f64, x1: f64, y1: f64) -> bool {
        self.terrain_segment_hit(x0, y0, x1, y1, true).is_none()
    }

    /// Exact grid traversal + segment/shape intersection, shared by rays and
    /// projectiles. Sampling endpoints could miss a half stair at high speed.
    fn terrain_segment_hit(
        &self,
        x0: f64,
        y0: f64,
        x1: f64,
        y1: f64,
        sight: bool,
    ) -> Option<(f64, f64)> {
        let (dx, dy) = (x1 - x0, y1 - y0);
        let start = (x0.floor() as i32, y0.floor() as i32);
        let end = (x1.floor() as i32, y1.floor() as i32);
        let (mut bx, mut by) = start;
        let (sx, sy) = (dx.signum() as i32, dy.signum() as i32);
        let (tx, ty) = (
            if dx == 0.0 {
                f64::INFINITY
            } else {
                1.0 / dx.abs()
            },
            if dy == 0.0 {
                f64::INFINITY
            } else {
                1.0 / dy.abs()
            },
        );
        let mut next_x = if dx == 0.0 {
            f64::INFINITY
        } else {
            (if sx > 0 {
                bx as f64 + 1.0 - x0
            } else {
                x0 - bx as f64
            }) / dx.abs()
        };
        let mut next_y = if dy == 0.0 {
            f64::INFINITY
        } else {
            (if sy > 0 {
                by as f64 + 1.0 - y0
            } else {
                y0 - by as f64
            }) / dy.abs()
        };
        let hit_cell = |bx: i32, by: i32| {
            if sight && (bx, by) == end {
                return None;
            }
            let mut first = f64::INFINITY;
            for bounds in self.collision_bounds_at(bx, by) {
                let (mut enter, mut exit) = (0.0_f64, 1.0_f64);
                for (origin, delta, low, high) in [
                    (x0, dx, bounds[0], bounds[2]),
                    (y0, dy, bounds[1], bounds[3]),
                ] {
                    if delta == 0.0 {
                        if origin < low || origin >= high {
                            exit = -1.0;
                            break;
                        }
                    } else {
                        let (a, b) = ((low - origin) / delta, (high - origin) / delta);
                        enter = enter.max(a.min(b));
                        exit = exit.min(a.max(b));
                    }
                }
                if enter <= exit && exit >= 0.0 && enter <= 1.0 {
                    first = first.min(enter);
                }
            }
            if first.is_finite() {
                return Some((x0 + dx * first, y0 + dy * first));
            }
            None
        };
        for _ in 0..=start.0.abs_diff(end.0) + start.1.abs_diff(end.1) + 1 {
            if let Some(hit) = hit_cell(bx, by) {
                return Some(hit);
            }
            if (bx, by) == end {
                break;
            }
            if next_x < next_y {
                bx += sx;
                next_x += tx;
            } else if next_y < next_x {
                by += sy;
                next_y += ty;
            } else {
                // Supercover: the two cells beside an exact corner occlude it.
                if let Some(hit) = hit_cell(bx + sx, by).or_else(|| hit_cell(bx, by + sy)) {
                    return Some(hit);
                }
                bx += sx;
                by += sy;
                next_x += tx;
                next_y += ty;
            }
        }
        None
    }

    fn should_ground_mob_chase_jump(
        &self,
        mob_x: f64,
        mob_y: f64,
        mob_vx: f64,
        mob_grounded: bool,
        half_width: f64,
        height: f64,
    ) -> bool {
        if !mob_grounded
            || mob_vx.abs() < 0.04
            || !self.has_line_of_sight(
                mob_x,
                mob_y - height * 0.7,
                self.player.x,
                self.player.y - 1.5,
            )
        {
            return false;
        }

        let dx = self.player.x - mob_x;
        let dy = self.player.y - mob_y;
        if dy >= MOB_VERTICAL_CHASE_JUMP_DY_THRESHOLD || dx.abs() > MOB_VERTICAL_CHASE_JUMP_X_RANGE
        {
            return false;
        }

        if dx.abs() >= 0.4 && dx.signum() != mob_vx.signum() {
            return false;
        }

        let head_probe_y = mob_y - height - 0.2;
        !self.is_colliding(mob_x, head_probe_y, CollisionType::VerticalUp)
            && !self.is_colliding(mob_x - half_width, head_probe_y, CollisionType::VerticalUp)
            && !self.is_colliding(mob_x + half_width, head_probe_y, CollisionType::VerticalUp)
    }

    #[allow(clippy::too_many_arguments)]
    fn should_ground_mob_vertical_recovery_jump(
        &self,
        mob_x: f64,
        mob_y: f64,
        mob_vx: f64,
        mob_grounded: bool,
        half_width: f64,
        height: f64,
        stuck_ticks: u8,
        reroute_ticks: u8,
    ) -> bool {
        if !mob_grounded {
            return false;
        }

        let dx = self.player.x - mob_x;
        let dy = self.player.y - mob_y;
        if dy >= MOB_VERTICAL_RECOVERY_DY_THRESHOLD || dx.abs() > MOB_VERTICAL_RECOVERY_X_RANGE {
            return false;
        }

        if stuck_ticks < 4 && reroute_ticks == 0 {
            return false;
        }

        let move_dir = if mob_vx.abs() > 0.04 {
            mob_vx.signum()
        } else {
            dx.signum()
        };
        if move_dir == 0.0 {
            return false;
        }

        let probe_x = mob_x + move_dir * (half_width + 0.45);
        let probe_feet_y = (mob_y - 0.1).floor() as i32;
        let probe_chest_y = (mob_y - (height - 0.5)).floor() as i32;
        let blocked_ahead = self
            .world
            .get_block(probe_x.floor() as i32, probe_feet_y)
            .is_solid()
            || self
                .world
                .get_block(probe_x.floor() as i32, probe_chest_y)
                .is_solid();
        if !blocked_ahead {
            return false;
        }

        let head_probe_y = mob_y - height - 0.2;
        !self.is_colliding(mob_x, head_probe_y, CollisionType::VerticalUp)
            && !self.is_colliding(mob_x - half_width, head_probe_y, CollisionType::VerticalUp)
            && !self.is_colliding(mob_x + half_width, head_probe_y, CollisionType::VerticalUp)
    }

    fn apply_ground_mob_reroute_velocity(
        reroute_ticks: u8,
        reroute_dir: i8,
        current_vx: f64,
        min_speed: f64,
    ) -> (u8, f64) {
        if reroute_ticks == 0 || reroute_dir == 0 {
            return (reroute_ticks, current_vx);
        }
        let dir = if reroute_dir >= 0 { 1.0 } else { -1.0 };
        let vx = dir * current_vx.abs().max(min_speed);
        (reroute_ticks.saturating_sub(1), vx)
    }

    #[allow(clippy::too_many_arguments)]
    fn next_ground_reroute_state(
        player_x: f64,
        player_y: f64,
        prev_mob_x: f64,
        next_mob_x: f64,
        next_mob_y: f64,
        hit_wall: bool,
        mob_grounded: bool,
        mut stuck_ticks: u8,
        mut reroute_ticks: u8,
        mut reroute_dir: i8,
    ) -> (u8, u8, i8) {
        let before_dx = (player_x - prev_mob_x).abs();
        let after_dx = (player_x - next_mob_x).abs();
        let improved = after_dx + 0.04 < before_dx;

        if improved {
            stuck_ticks = stuck_ticks.saturating_sub(2);
        } else if hit_wall || mob_grounded {
            let gain = if hit_wall { 2 } else { 1 };
            stuck_ticks = stuck_ticks.saturating_add(gain);
        } else {
            stuck_ticks = stuck_ticks.saturating_sub(1);
        }

        if hit_wall && reroute_ticks > 0 {
            reroute_dir = -reroute_dir;
            reroute_ticks = reroute_ticks.max(8);
        }

        let vertical_gap = player_y - next_mob_y;
        if reroute_ticks > 0 && improved && vertical_gap > -0.6 && after_dx < 5.0 {
            reroute_ticks = reroute_ticks.saturating_sub(2);
        }
        if reroute_ticks == 0 {
            reroute_dir = 0;
        }

        if reroute_ticks == 0 && stuck_ticks >= MOB_REROUTE_TRIGGER_TICKS {
            let toward_player = if player_x >= next_mob_x { 1 } else { -1 };
            reroute_dir = -toward_player;
            reroute_ticks = if vertical_gap <= MOB_VERTICAL_RECOVERY_DY_THRESHOLD {
                MOB_REROUTE_VERTICAL_TICKS
            } else {
                MOB_REROUTE_BASE_TICKS
            };
            stuck_ticks = 0;
        }

        (stuck_ticks, reroute_ticks, reroute_dir)
    }

    fn ground_path_clearance_blocks(height: f64) -> i32 {
        ((height + 0.1).ceil() as i32).clamp(1, 3)
    }

    fn is_walkable_ground_path_node(&self, x: i32, support_y: i32, clearance: i32) -> bool {
        if support_y < (1 + clearance) || support_y >= 127 {
            return false;
        }
        let ground = self.world.get_block(x, support_y);
        if !ground.is_solid() || ground.is_fluid() {
            return false;
        }
        for offset in 1..=clearance {
            if self.world.get_block(x, support_y - offset) != BlockType::Air {
                return false;
            }
        }
        true
    }

    fn nearest_walkable_ground_node_y(
        &self,
        x: i32,
        around_y: i32,
        clearance: i32,
        y_min: i32,
        y_max: i32,
    ) -> Option<i32> {
        let max_delta = (y_max - y_min).clamp(0, MOB_PATHFIND_SEARCH_RADIUS_Y);
        for delta in 0..=max_delta {
            let up = around_y - delta;
            if up >= y_min && up <= y_max && self.is_walkable_ground_path_node(x, up, clearance) {
                return Some(up);
            }
            if delta == 0 {
                continue;
            }
            let down = around_y + delta;
            if down >= y_min
                && down <= y_max
                && self.is_walkable_ground_path_node(x, down, clearance)
            {
                return Some(down);
            }
        }
        None
    }

    fn neighbor_ground_node_y(
        &self,
        next_x: i32,
        current_y: i32,
        clearance: i32,
        y_min: i32,
        y_max: i32,
    ) -> Option<i32> {
        // Allow one-block climb and larger controlled drops to route through pits/underpasses.
        for candidate_y in [current_y - 1, current_y] {
            if candidate_y >= y_min
                && candidate_y <= y_max
                && self.is_walkable_ground_path_node(next_x, candidate_y, clearance)
            {
                return Some(candidate_y);
            }
        }
        for drop in 1..=8 {
            let candidate_y = current_y + drop;
            if candidate_y < y_min || candidate_y > y_max {
                continue;
            }
            if self.is_walkable_ground_path_node(next_x, candidate_y, clearance) {
                return Some(candidate_y);
            }
        }
        None
    }

    fn ground_path_heuristic(x: i32, y: i32, goal_x: i32, goal_y: i32) -> i32 {
        (goal_x - x).abs() * 10 + (goal_y - y).abs() * 3
    }

    fn find_ground_path_first_step_dir(&self, mob_x: f64, mob_y: f64, height: f64) -> Option<i8> {
        let goal_x = self.player.x.floor() as i32;
        let start_x = mob_x.floor() as i32;
        if (goal_x - start_x).abs() <= 1 {
            return Some(if goal_x >= start_x { 1 } else { -1 });
        }

        let clearance = Self::ground_path_clearance_blocks(height);
        let start_guess_y = mob_y.floor() as i32 + 1;
        let goal_guess_y = self.player.y.floor() as i32 + 1;
        let y_min = (start_guess_y.min(goal_guess_y) - MOB_PATHFIND_SEARCH_RADIUS_Y)
            .clamp(1 + clearance, 126);
        let y_max = (start_guess_y.max(goal_guess_y) + MOB_PATHFIND_SEARCH_RADIUS_Y)
            .clamp(1 + clearance, 126);
        let x_min = start_x.min(goal_x) - MOB_PATHFIND_SEARCH_RADIUS_X;
        let x_max = start_x.max(goal_x) + MOB_PATHFIND_SEARCH_RADIUS_X;

        let start_y =
            self.nearest_walkable_ground_node_y(start_x, start_guess_y, clearance, y_min, y_max)?;
        let goal_y =
            self.nearest_walkable_ground_node_y(goal_x, goal_guess_y, clearance, y_min, y_max)?;

        let start = (start_x, start_y);
        let mut open_set = BinaryHeap::new();
        open_set.push(GroundPathSearchNode {
            x: start.0,
            y: start.1,
            g_cost: 0,
            f_cost: Self::ground_path_heuristic(start.0, start.1, goal_x, goal_y),
        });

        let mut came_from: HashMap<(i32, i32), (i32, i32)> = HashMap::new();
        let mut g_costs: HashMap<(i32, i32), i32> = HashMap::new();
        g_costs.insert(start, 0);

        let mut expansions = 0usize;
        let mut reached_goal: Option<(i32, i32)> = None;

        while let Some(current) = open_set.pop() {
            if expansions >= MOB_PATHFIND_MAX_EXPANSIONS {
                break;
            }
            expansions += 1;
            let current_key = (current.x, current.y);
            let Some(&best_known_cost) = g_costs.get(&current_key) else {
                continue;
            };
            if current.g_cost != best_known_cost {
                continue;
            }
            if (goal_x - current.x).abs() <= 1 && (goal_y - current.y).abs() <= 2 {
                reached_goal = Some(current_key);
                break;
            }

            for dir in [-1, 1] {
                let next_x = current.x + dir;
                if next_x < x_min || next_x > x_max {
                    continue;
                }
                let Some(next_y) =
                    self.neighbor_ground_node_y(next_x, current.y, clearance, y_min, y_max)
                else {
                    continue;
                };
                let climb_cost = (current.y - next_y).max(0) * 4;
                let drop_cost = (next_y - current.y).max(0) * 2;
                let step_cost = 10 + climb_cost + drop_cost;
                let tentative = current.g_cost + step_cost;
                let next_key = (next_x, next_y);
                if g_costs
                    .get(&next_key)
                    .is_some_and(|known| tentative >= *known)
                {
                    continue;
                }
                came_from.insert(next_key, current_key);
                g_costs.insert(next_key, tentative);
                open_set.push(GroundPathSearchNode {
                    x: next_x,
                    y: next_y,
                    g_cost: tentative,
                    f_cost: tentative + Self::ground_path_heuristic(next_x, next_y, goal_x, goal_y),
                });
            }
        }

        let mut cursor = reached_goal?;
        while let Some(parent) = came_from.get(&cursor) {
            if *parent == start {
                let dx = cursor.0 - start.0;
                return match dx.cmp(&0) {
                    Ordering::Less => Some(-1),
                    Ordering::Greater => Some(1),
                    Ordering::Equal => None,
                };
            }
            cursor = *parent;
        }
        None
    }

    #[allow(clippy::too_many_arguments)]
    fn refine_ground_reroute_with_path(
        &self,
        mob_x: f64,
        mob_y: f64,
        height: f64,
        prev_reroute_ticks: u8,
        stuck_ticks: u8,
        reroute_ticks: u8,
        reroute_dir: i8,
    ) -> (u8, i8) {
        if reroute_ticks == 0 {
            return (reroute_ticks, reroute_dir);
        }
        let should_repath = prev_reroute_ticks == 0
            || reroute_dir == 0
            || (stuck_ticks >= 4 && stuck_ticks.is_multiple_of(MOB_REROUTE_REPATH_INTERVAL));
        if !should_repath {
            return (reroute_ticks, reroute_dir);
        }
        let Some(path_dir) = self.find_ground_path_first_step_dir(mob_x, mob_y, height) else {
            return (reroute_ticks, reroute_dir);
        };
        let boosted_ticks = if self.player.y + 0.4 < mob_y {
            reroute_ticks.max(MOB_REROUTE_VERTICAL_TICKS)
        } else {
            reroute_ticks.max(MOB_REROUTE_BASE_TICKS + 6)
        };
        (boosted_ticks, path_dir)
    }

    fn is_valid_portal_frame_at(&self, inner_x: i32, inner_y: i32) -> bool {
        self.world.nether_portal_frame_at(inner_x, inner_y)
    }

    fn fill_portal_interior(&mut self, inner_x: i32, inner_y: i32) {
        for x in inner_x..=(inner_x + 1) {
            for y in inner_y..=(inner_y + 2) {
                self.world.set_block(x, y, BlockType::NetherPortal);
            }
        }
    }

    fn try_activate_portal_from_block(&mut self, bx: i32, by: i32) -> bool {
        for inner_x in (bx - 2)..=(bx + 1) {
            for inner_y in (by - 3)..=(by + 1) {
                if self.is_valid_portal_frame_at(inner_x, inner_y) {
                    self.fill_portal_interior(inner_x, inner_y);
                    return true;
                }
            }
        }
        false
    }

    fn is_valid_end_portal_frame_at(&self, inner_x: i32, inner_y: i32) -> bool {
        self.world.end_portal_frame_at(inner_x, inner_y)
    }

    fn fill_end_portal_interior(&mut self, inner_x: i32, inner_y: i32) {
        for x in inner_x..=(inner_x + 1) {
            for y in inner_y..=(inner_y + 1) {
                self.world.set_block(x, y, BlockType::EndPortal);
            }
        }
    }

    fn try_activate_end_portal_from_frame(&mut self, bx: i32, by: i32) -> bool {
        for inner_x in (bx - 2)..=(bx + 1) {
            for inner_y in (by - 2)..=(by + 1) {
                if self.is_valid_end_portal_frame_at(inner_x, inner_y) {
                    self.fill_end_portal_interior(inner_x, inner_y);
                    return true;
                }
            }
        }
        false
    }

    fn nether_portal_anchor_for_block(&self, block_x: i32, block_y: i32) -> Option<(i32, i32)> {
        self.world.nether_portal_anchor_at(block_x, block_y)
    }

    fn nether_portal_link_key(dimension: Dimension, inner_x: i32, base_y: i32) -> (u8, i32, i32) {
        (Self::encode_dimension(dimension), inner_x, base_y)
    }

    fn paired_nether_dimension(dimension: Dimension) -> Option<Dimension> {
        match dimension {
            Dimension::Overworld => Some(Dimension::Nether),
            Dimension::Nether => Some(Dimension::Overworld),
            Dimension::End => None,
        }
    }

    fn is_valid_nether_portal_anchor(&self, inner_x: i32, base_y: i32) -> bool {
        base_y >= 3
            && base_y < CHUNK_HEIGHT as i32
            && self.nether_portal_anchor_for_block(inner_x, base_y - 1) == Some((inner_x, base_y))
    }

    fn linked_nether_portal_anchor(
        &mut self,
        source_dimension: Dimension,
        source_anchor: (i32, i32),
    ) -> Option<(i32, i32)> {
        let target_anchor = *self.portal_links.get(&Self::nether_portal_link_key(
            source_dimension,
            source_anchor.0,
            source_anchor.1,
        ))?;
        self.world.load_chunks_around(target_anchor.0);
        if self.is_valid_nether_portal_anchor(target_anchor.0, target_anchor.1) {
            Some(target_anchor)
        } else {
            None
        }
    }

    fn remember_nether_portal_link(
        &mut self,
        source_dimension: Dimension,
        source_anchor: (i32, i32),
        target_anchor: (i32, i32),
    ) {
        let Some(target_dimension) = Self::paired_nether_dimension(source_dimension) else {
            return;
        };

        self.portal_links.insert(
            Self::nether_portal_link_key(source_dimension, source_anchor.0, source_anchor.1),
            target_anchor,
        );
        self.portal_links.insert(
            Self::nether_portal_link_key(target_dimension, target_anchor.0, target_anchor.1),
            source_anchor,
        );
    }

    fn player_portal_contact(&self) -> Option<(PortalKind, Option<(i32, i32)>)> {
        let side_sample = (PLAYER_HALF_WIDTH - 0.02).max(0.0);
        let sample_xs = [
            self.player.x - side_sample,
            self.player.x,
            self.player.x + side_sample,
        ];
        let y_checks = [
            self.player.y.floor() as i32,
            (self.player.y - 0.9).floor() as i32,
            (self.player.y - 1.5).floor() as i32,
        ];

        for &y in &y_checks {
            for &sample_x in &sample_xs {
                let bx = sample_x.floor() as i32;
                match self.world.get_block(bx, y) {
                    BlockType::EndPortal => return Some((PortalKind::End, None)),
                    BlockType::NetherPortal => {
                        return Some((
                            PortalKind::Nether,
                            self.nether_portal_anchor_for_block(bx, y),
                        ));
                    }
                    _ => {}
                }
            }
        }

        None
    }

    #[cfg(test)]
    fn player_portal_kind(&self) -> Option<PortalKind> {
        self.player_portal_contact().map(|(kind, _)| kind)
    }

    fn player_nether_portal_anchor(&self) -> Option<(i32, i32)> {
        self.player_portal_contact()
            .and_then(|(kind, anchor)| (kind == PortalKind::Nether).then_some(anchor).flatten())
    }

    fn nearby_player_nether_portal_anchor(&mut self, search_radius: i32) -> Option<(i32, i32)> {
        let approx_x = self.player.x.round() as i32;
        self.find_existing_nether_portal_near(approx_x, search_radius)
            .filter(|&(inner_x, _)| (inner_x - approx_x).abs() <= search_radius)
    }

    fn adjacent_nether_portal_anchor(&self, bx: i32, by: i32) -> Option<(i32, i32)> {
        for nx in (bx - 1)..=(bx + 1) {
            for ny in (by - 1)..=(by + 1) {
                if self.world.get_block(nx, ny) == BlockType::NetherPortal
                    && let Some(anchor) = self.nether_portal_anchor_for_block(nx, ny)
                {
                    return Some(anchor);
                }
            }
        }
        None
    }

    fn has_adjacent_end_portal(&self, bx: i32, by: i32) -> bool {
        for nx in (bx - 1)..=(bx + 1) {
            for ny in (by - 1)..=(by + 1) {
                if self.world.get_block(nx, ny) == BlockType::EndPortal {
                    return true;
                }
            }
        }
        false
    }

    fn has_nearby_end_portal(&self, bx: i32, by: i32, x_radius: i32, y_radius: i32) -> bool {
        for nx in (bx - x_radius)..=(bx + x_radius) {
            for ny in (by - y_radius)..=(by + y_radius) {
                if self.world.get_block(nx, ny) == BlockType::EndPortal {
                    return true;
                }
            }
        }
        false
    }

    fn is_end_portal_use_surface(&self, bx: i32, by: i32) -> bool {
        match self.world.get_block(bx, by) {
            BlockType::EndPortal => true,
            BlockType::EndPortalFrame { .. } | BlockType::Bedrock => {
                self.has_adjacent_end_portal(bx, by)
            }
            BlockType::Obsidian
            | BlockType::StoneBricks
            | BlockType::StoneSlab
            | BlockType::Glowstone => self.has_nearby_end_portal(bx, by, 4, 5),
            _ => false,
        }
    }

    fn portal_use_target_at(&self, bx: i32, by: i32) -> Option<PortalUseTarget> {
        match self.world.get_block(bx, by) {
            BlockType::NetherPortal => self
                .nether_portal_anchor_for_block(bx, by)
                .map(PortalUseTarget::Nether),
            BlockType::Obsidian => self
                .adjacent_nether_portal_anchor(bx, by)
                .map(PortalUseTarget::Nether)
                .or_else(|| {
                    self.is_end_portal_use_surface(bx, by)
                        .then_some(PortalUseTarget::End)
                }),
            _ => self
                .is_end_portal_use_surface(bx, by)
                .then_some(PortalUseTarget::End),
        }
    }

    fn has_portal_use_line_of_sight(
        &self,
        x0: f64,
        y0: f64,
        bx: i32,
        by: i32,
        target: PortalUseTarget,
    ) -> bool {
        let x1 = bx as f64 + 0.5;
        let y1 = by as f64 + 0.5;
        let dx = x1 - x0;
        let dy = y1 - y0;
        let dist = (dx * dx + dy * dy).sqrt();
        let steps = (dist * 2.0).ceil() as i32;
        for i in 1..steps {
            let t = i as f64 / steps as f64;
            let cx = (x0 + dx * t).floor() as i32;
            let cy = (y0 + dy * t).floor() as i32;
            if cx == bx && cy == by {
                continue;
            }
            let block = self.world.get_block(cx, cy);
            if !block.is_solid() {
                continue;
            }
            let allowed = match target {
                PortalUseTarget::Nether(anchor) => {
                    block == BlockType::Obsidian
                        && self.adjacent_nether_portal_anchor(cx, cy) == Some(anchor)
                }
                PortalUseTarget::End => self.is_end_portal_use_surface(cx, cy),
            };
            if !allowed {
                return false;
            }
        }
        true
    }

    fn use_portal_target(&mut self, target: PortalUseTarget) {
        if self.portal_cooldown > 0 {
            return;
        }
        let source_dimension = self.current_dimension;
        match target {
            PortalUseTarget::Nether(source_anchor) => {
                self.transfer_nether_dimension_from_anchor(Some(source_anchor));
            }
            PortalUseTarget::End => {
                self.transfer_end_dimension();
            }
        }
        if self.current_dimension != source_dimension || self.credits_active {
            self.portal_timer = 0;
            self.portal_cooldown = 40;
        }
    }

    pub fn portal_interaction_hint(&self, bx: i32, by: i32) -> Option<&'static str> {
        let target = self.portal_use_target_at(bx, by)?;
        Some(
            if !self.has_portal_use_line_of_sight(
                self.player.x,
                self.player.y - 1.0,
                bx,
                by,
                target,
            ) {
                "Blocked: move around"
            } else if self.portal_cooldown > 0 {
                "Portal recharging..."
            } else {
                "R-click / F: travel"
            },
        )
    }

    fn find_existing_nether_portal_near(
        &mut self,
        approx_x: i32,
        search_radius: i32,
    ) -> Option<(i32, i32)> {
        let radius = search_radius.max(0);
        self.world.load_chunks_for_spawn_search(approx_x, radius);

        let mut best: Option<(i32, i32, i32)> = None; // (dist, inner_x, base_y)
        for wx in (approx_x - radius)..=(approx_x + radius) {
            for wy in 0..CHUNK_HEIGHT as i32 {
                let Some((inner_x, base_y)) = self.nether_portal_anchor_for_block(wx, wy) else {
                    continue;
                };
                if self.current_dimension == Dimension::Overworld
                    && self.overworld_portal_anchor_is_elevated(inner_x, base_y)
                {
                    continue;
                }
                let dist = (inner_x - approx_x).abs();
                match best {
                    None => best = Some((dist, inner_x, base_y)),
                    Some((best_dist, best_x, best_base_y))
                        if dist < best_dist
                            || (dist == best_dist && (inner_x, base_y) < (best_x, best_base_y)) =>
                    {
                        best = Some((dist, inner_x, base_y));
                    }
                    _ => {}
                }
            }
        }

        best.map(|(_, inner_x, base_y)| (inner_x, base_y))
    }

    fn overworld_portal_has_grounded_shoulder(
        &self,
        sample_start_x: i32,
        sample_end_x: i32,
        base_y: i32,
    ) -> bool {
        let mut near_height_samples = 0;
        let mut total_samples = 0;
        for sample_x in sample_start_x..=sample_end_x {
            let Some(surface_y) = Self::progression_spawn_surface_y(&self.world, sample_x) else {
                continue;
            };
            total_samples += 1;
            if (surface_y - base_y).abs() <= 3 {
                near_height_samples += 1;
            }
        }
        total_samples >= 2 && near_height_samples >= 2
    }

    fn overworld_portal_surrounding_ground_samples(&self, inner_x: i32) -> Vec<i32> {
        let left = inner_x - 5;
        let right = inner_x + 7;
        let mut surrounding_ground = Vec::new();

        for sample_x in (left - 4)..=(left - 1) {
            if let Some(surface_y) = Self::progression_spawn_surface_y(&self.world, sample_x) {
                surrounding_ground.push(surface_y);
            }
        }
        for sample_x in (right + 1)..=(right + 4) {
            if let Some(surface_y) = Self::progression_spawn_surface_y(&self.world, sample_x) {
                surrounding_ground.push(surface_y);
            }
        }

        if surrounding_ground.len() < 4 {
            for sample_x in (inner_x - 16)..=(inner_x + 18) {
                if (left - 1..=right + 1).contains(&sample_x) {
                    continue;
                }
                if let Some(surface_y) = Self::progression_spawn_surface_y(&self.world, sample_x) {
                    surrounding_ground.push(surface_y);
                }
            }
        }

        surrounding_ground
    }

    fn overworld_portal_arrival_terrain_penalty(&self, inner_x: i32, base_y: i32) -> i32 {
        let mut penalty = 0;
        let mut near_height_columns = 0;
        for sample_x in (inner_x - 5)..=(inner_x + 7) {
            match Self::progression_spawn_surface_y(&self.world, sample_x) {
                Some(surface_y) => {
                    let delta = surface_y - base_y;
                    penalty += delta.abs() * 2;
                    if delta > 2 {
                        penalty += (delta - 2) * 6;
                    } else if delta < -2 {
                        penalty += (-delta - 2) * 4;
                    }
                    if delta.abs() <= 2 {
                        near_height_columns += 1;
                    }
                }
                None => penalty += 18,
            }
        }

        if near_height_columns < 7 {
            penalty += (7 - near_height_columns) * 12;
        }

        penalty
    }

    fn overworld_portal_anchor_is_elevated(&self, inner_x: i32, base_y: i32) -> bool {
        let left = inner_x - 5;
        let right = inner_x + 7;
        let left_grounded = self.overworld_portal_has_grounded_shoulder(left - 4, left - 1, base_y);
        let right_grounded =
            self.overworld_portal_has_grounded_shoulder(right + 1, right + 4, base_y);
        let mut surrounding_ground = self.overworld_portal_surrounding_ground_samples(inner_x);
        surrounding_ground.sort_unstable();
        if surrounding_ground.len() >= 4 {
            let median_ground_y = surrounding_ground[surrounding_ground.len() / 2];
            if base_y + 4 < median_ground_y {
                return true;
            }
            // A complete portal below the surface belongs to a cave or a room,
            // not a floating platform. This also lets pre-link saves rediscover it.
            if base_y > median_ground_y + 4 && self.is_valid_nether_portal_anchor(inner_x, base_y) {
                return false;
            }
        }

        !left_grounded && !right_grounded
    }

    fn find_overworld_portal_arrival_site(
        &mut self,
        approx_x: i32,
        search_radius: i32,
    ) -> (i32, i32) {
        let radius = search_radius.max(0);
        self.world.load_chunks_for_spawn_search(approx_x, radius);
        let mut best: Option<(i32, i32, i32)> = None; // (score, x, y)
        for dx in -radius..=radius {
            let x = approx_x + dx;
            let Some(y) = Self::progression_spawn_surface_y(&self.world, x) else {
                continue;
            };
            if !Self::is_safe_spawn_column(&self.world, x, y, true)
                || self.overworld_portal_anchor_is_elevated(x, y)
            {
                continue;
            }

            let score =
                dx.abs() * 4 + (y - 33).abs() + self.overworld_portal_arrival_terrain_penalty(x, y);
            match best {
                None => best = Some((score, x, y)),
                Some((best_score, _, best_y))
                    if score < best_score || (score == best_score && y < best_y) =>
                {
                    best = Some((score, x, y));
                }
                _ => {}
            }
        }
        if let Some((_, x, y)) = best {
            return (x, y);
        }
        if let Some((x, y)) = Self::find_nearest_safe_spawn(&self.world, approx_x, radius, true)
            .filter(|&(x, y)| !self.overworld_portal_anchor_is_elevated(x, y))
        {
            return (x, y);
        }
        for dx in 0..=radius {
            let right_x = approx_x + dx;
            if let Some(y) = Self::progression_spawn_surface_y(&self.world, right_x)
                && !self.overworld_portal_anchor_is_elevated(right_x, y)
            {
                return (right_x, y);
            }
            if dx == 0 {
                continue;
            }
            let left_x = approx_x - dx;
            if let Some(y) = Self::progression_spawn_surface_y(&self.world, left_x)
                && !self.overworld_portal_anchor_is_elevated(left_x, y)
            {
                return (left_x, y);
            }
        }
        (approx_x, self.find_walkable_surface(approx_x))
    }

    fn find_walkable_surface(&mut self, x: i32) -> i32 {
        self.world.load_chunks_around(x);
        if self.current_dimension == Dimension::Overworld
            && let Some(y) = Self::progression_spawn_surface_y(&self.world, x)
        {
            return y;
        }
        let walkable_range = 2..(127 - 2);
        if self.current_dimension == Dimension::Nether {
            for y in walkable_range.clone().rev() {
                let ground = self.world.get_block(x, y);
                if ground.is_solid()
                    && self.world.get_block(x, y - 1) == BlockType::Air
                    && self.world.get_block(x, y - 2) == BlockType::Air
                {
                    return y;
                }
            }
        }
        for y in walkable_range {
            let ground = self.world.get_block(x, y);
            if ground.is_solid()
                && self.world.get_block(x, y - 1) == BlockType::Air
                && self.world.get_block(x, y - 2) == BlockType::Air
            {
                return y;
            }
        }
        for y in 0..127 {
            if self.world.get_block(x, y).is_solid() {
                return y;
            }
        }
        40
    }

    fn portal_arrival_floor_block(&self) -> BlockType {
        match self.current_dimension {
            Dimension::Overworld => BlockType::Stone,
            Dimension::Nether => BlockType::Netherrack,
            Dimension::End => BlockType::EndStone,
        }
    }

    fn build_portal_arrival_vestibule(&mut self, portal_x: i32, base_y: i32) -> (f64, f64) {
        let base_y = base_y.clamp(8, CHUNK_HEIGHT as i32 - 4);
        let floor_block = self.portal_arrival_floor_block();
        let (left, right, clear_top, clear_bottom, spawn_x) =
            if self.current_dimension == Dimension::Nether {
                (
                    portal_x - 8,
                    portal_x + 16,
                    base_y - 6,
                    base_y - 1,
                    portal_x as f64 + 7.5,
                )
            } else {
                (
                    portal_x - 5,
                    portal_x + 7,
                    base_y - 4,
                    base_y - 1,
                    portal_x as f64 + 4.5,
                )
            };

        for wx in left..=right {
            self.world.set_block(wx, base_y, floor_block);
            let support_limit = if self.current_dimension == Dimension::Overworld {
                CHUNK_HEIGHT as i32 - 2
            } else {
                (base_y + 3).min(CHUNK_HEIGHT as i32 - 2)
            };
            for support_y in (base_y + 1)..=support_limit {
                let below = self.world.get_block(wx, support_y);
                if below.is_solid() && !below.is_fluid() {
                    break;
                }
                self.world.set_block(wx, support_y, floor_block);
            }
            for wy in clear_top..=clear_bottom {
                self.world.set_block(wx, wy, BlockType::Air);
            }
        }

        self.ensure_nether_portal_at(portal_x, base_y);
        (spawn_x, base_y as f64 - 0.1)
    }

    fn existing_portal_arrival(&self, portal_x: i32, base_y: i32) -> (f64, f64) {
        // Reusing a frame must not bulldoze the player's lights, storage or home.
        for offset in [3, -2, 4, -3, 5, -4, 6, -5, 7, -6] {
            let x = portal_x + offset;
            if Self::is_safe_spawn_column_base(&self.world, x, base_y, false) {
                return (x as f64 + 0.5, base_y as f64 - 0.1);
            }
        }
        // A valid opening is always two blocks wide and three high. If both
        // exits were built over, arrive inside it rather than erasing the build.
        (portal_x as f64 + 0.5, base_y as f64 - 0.1)
    }

    fn find_end_arrival_site(&mut self) -> (i32, i32) {
        self.world.load_chunks_around(0);

        let mut best: Option<(i32, i32, i32)> = None; // (score, x, y)
        for x in (-18..=-6).chain(6..=18) {
            let Some(y) = Self::progression_spawn_surface_y(&self.world, x) else {
                continue;
            };
            if !Self::is_safe_spawn_column_base(&self.world, x, y, false)
                || !Self::spawn_has_stable_runway(&self.world, x, y, false)
            {
                continue;
            }

            let left_y = Self::progression_spawn_surface_y(&self.world, x - 1).unwrap_or(y);
            let right_y = Self::progression_spawn_surface_y(&self.world, x + 1).unwrap_or(y);
            let mut score = (x.abs() - 10).abs() * 3;
            score += (y - 34).abs() * 2;
            score += (left_y - y).abs() * 5;
            score += (right_y - y).abs() * 5;

            for tower_x in END_TOWER_XS {
                let dx = (x - tower_x).abs();
                if dx < 6 {
                    score += (6 - dx) * 18;
                }
            }

            match best {
                None => best = Some((score, x, y)),
                Some((best_score, _, best_y))
                    if score < best_score || (score == best_score && y < best_y) =>
                {
                    best = Some((score, x, y));
                }
                _ => {}
            }
        }

        best.map(|(_, x, y)| (x, y))
            .unwrap_or_else(|| (8, self.find_walkable_surface(8)))
    }

    fn prepare_end_arrival_pad(&mut self, center_x: i32, floor_y: i32) -> (f64, f64) {
        let floor_y = floor_y.clamp(8, CHUNK_HEIGHT as i32 - 4);
        for wx in (center_x - 3)..=(center_x + 3) {
            self.world.set_block(wx, floor_y, BlockType::EndStone);
            self.world.set_block(wx, floor_y + 1, BlockType::EndStone);
            for wy in (floor_y - 4)..=(floor_y - 1) {
                self.world.set_block(wx, wy, BlockType::Air);
            }
        }
        (center_x as f64 + 0.5, floor_y as f64 - 0.1)
    }

    fn find_spawn_surface_for_mob(&self, x: i32) -> Option<f64> {
        for y in 2..(127 - 2) {
            let ground = self.world.get_block(x, y);
            if ground.is_solid()
                && !ground.is_fluid()
                && self.world.get_block(x, y - 1) == BlockType::Air
                && self.world.get_block(x, y - 2) == BlockType::Air
            {
                return Some(y as f64 - 0.1);
            }
        }
        None
    }

    // Find a floor on this cave level, rather than the first surface from the sky.
    fn ambient_spawn_clearance(block: BlockType) -> bool {
        !block.is_solid()
            && !block.is_fluid()
            && !matches!(block, BlockType::Fire(_) | BlockType::Cactus)
    }

    fn dark_spawn_floor_near(
        &self,
        x: i32,
        reference_y: i32,
        radius: i32,
        daylight: bool,
    ) -> Option<f64> {
        (2..CHUNK_HEIGHT as i32 - 2)
            .filter(|&y| (y - reference_y).abs() <= radius)
            .filter(|&y| {
                self.world.get_block(x, y).is_solid()
                    && Self::ambient_spawn_clearance(self.world.get_block(x, y - 1))
                    && Self::ambient_spawn_clearance(self.world.get_block(x, y - 2))
            })
            .filter(|&y| !daylight || !self.world.has_open_sky(x, y - 1))
            .filter(|&y| {
                self.world.light_at(x, y - 1, if daylight { 15 } else { 0 }) == 0
                    && self.world.light_at(x, y - 2, if daylight { 15 } else { 0 }) == 0
            })
            .min_by_key(|&y| (y - reference_y).abs())
            .map(|y| y as f64 - 0.1)
    }

    fn find_dark_overworld_spawn(&self, x: i32) -> Option<f64> {
        let daylight = self.time_of_day > 4000.0 && self.time_of_day < 20000.0;
        if !daylight
            && self.world.has_open_sky(
                self.player.x.floor() as i32,
                (self.player.y - 1.0).floor() as i32,
            )
        {
            // Outdoors at night, try actual dry surface ground. A cave under
            // a river must not consume the night refill reserved for the surface.
            let y = self.find_spawn_surface_for_daylight_mob(x)?;
            let feet = y.floor() as i32;
            return ((y - self.player.y).abs() <= 32.0
                && self.world.block_light_at(x, feet) == 0
                && self.world.block_light_at(x, feet - 1) == 0)
                .then_some(y);
        }
        self.dark_spawn_floor_near(x, self.player.y.floor() as i32, 32, daylight)
    }

    fn is_slime_chunk(world_x: i32) -> bool {
        let chunk_x = world_x.div_euclid(CHUNK_WIDTH as i32) as i64;
        let mut seed = chunk_x
            .wrapping_mul(chunk_x)
            .wrapping_mul(4987142)
            .wrapping_add(chunk_x.wrapping_mul(5947611))
            .wrapping_add(4392871);
        seed ^= seed >> 13;
        (seed & 0xF) == 0
    }

    fn find_spawn_surface_for_slime(&self, x: i32) -> Option<f64> {
        let spawn_y = self.find_dark_overworld_spawn(x)?;
        if spawn_y < 64.0 {
            return None;
        }
        if self.is_exposed_to_sky(x as f64 + 0.5, spawn_y - 1.0) {
            return None;
        }
        Some(spawn_y)
    }

    fn find_spawn_surface_for_daylight_mob(&self, x: i32) -> Option<f64> {
        // Wildlife needs a safe body-sized landing place, rather than the
        // flat escape runway reserved for the human's initial spawn. Grass,
        // flowers and gentle slopes must not suppress an entire night's mobs.
        for y in 2..CHUNK_HEIGHT as i32 - 2 {
            let ground = self.world.get_block(x, y);
            if ground.is_solid()
                && !ground.is_fluid()
                && !matches!(
                    ground,
                    BlockType::Cactus
                        | BlockType::Leaves
                        | BlockType::BirchLeaves
                        | BlockType::Wood
                        | BlockType::BirchWood
                )
                && Self::ambient_spawn_clearance(self.world.get_block(x, y - 1))
                && Self::ambient_spawn_clearance(self.world.get_block(x, y - 2))
                && self.world.has_open_sky(x, y - 2)
            {
                return Some(y as f64 - 0.1);
            }
        }
        None
    }

    fn find_water_spawn_for_squid(&self, x: i32) -> Option<f64> {
        for y in 38..(CHUNK_HEIGHT as i32 - 3) {
            if !matches!(self.world.get_block(x, y), BlockType::Water(_))
                || !matches!(self.world.get_block(x, y - 1), BlockType::Water(_))
            {
                continue;
            }
            if self.world.get_block(x, y + 1).is_solid() {
                continue;
            }
            return Some(y as f64 + 0.35);
        }
        None
    }

    fn random_overworld_slime_size(rng: &mut impl Rng) -> u8 {
        let roll = rng.gen_range(0..100);
        if roll < 8 {
            4
        } else if roll < 44 {
            2
        } else {
            1
        }
    }

    fn slime_spawn_chance_for_height(spawn_y: f64) -> f64 {
        if spawn_y >= 108.0 {
            0.05
        } else if spawn_y >= 90.0 {
            0.09
        } else if spawn_y >= 76.0 {
            0.14
        } else {
            0.18
        }
    }

    fn should_spawn_overworld_slime(&self, spawn_y: f64, rng: &mut impl Rng) -> bool {
        let chance = (Self::slime_spawn_chance_for_height(spawn_y)
            * self.hostile_spawn_chance_multiplier())
        .clamp(0.0, 0.95);
        rng.gen_bool(chance)
    }

    fn choose_overworld_hostile_spawn(&self, rng: &mut impl Rng) -> u8 {
        let roll = rng.gen_range(0..100);
        if roll < 36 {
            0 // zombie
        } else if roll < 52 {
            1 // creeper
        } else if roll < 78 {
            2 // skeleton
        } else {
            3 // spider
        }
    }

    fn spawn_overworld_hostile_at(&mut self, spawn_x: f64, spawn_y: f64, rng: &mut impl Rng) {
        match self.choose_overworld_hostile_spawn(rng) {
            0 => self.zombies.push(Zombie::new(spawn_x, spawn_y)),
            1 => self.creepers.push(Creeper::new(spawn_x, spawn_y)),
            2 => self.skeletons.push(Skeleton::new(spawn_x, spawn_y)),
            _ => self.spiders.push(Spider::new(spawn_x, spawn_y)),
        }
    }

    fn try_refill_overworld_enderman(&mut self, rng: &mut impl Rng) -> bool {
        let cap =
            Self::spawn_refill_threshold(self.scaled_hostile_cap(OVERWORLD_ENDERMAN_CAP), 1, 2);
        if self.current_dimension != Dimension::Overworld
            || !self.can_spawn_hostiles()
            || (self.time_of_day > 4000.0 && self.time_of_day < 20000.0)
            || self.endermen.len() >= cap
            || !rng.gen_bool(
                (OVERWORLD_ENDERMAN_REFILL_CHANCE * self.hostile_spawn_chance_multiplier())
                    .clamp(0.0, 0.95),
            )
        {
            return false;
        }
        for _ in 0..6 {
            let offset = rng.gen_range(12..=40);
            let wx =
                self.player.x.floor() as i32 + if rng.gen_bool(0.5) { offset } else { -offset };
            let Some(y) = self.find_dark_overworld_spawn(wx) else {
                continue;
            };
            let x = wx as f64 + 0.5;
            // Endermen need a full three-block body space and a dark head too.
            let head_y = (y - 2.6).floor() as i32;
            if self.is_spawn_too_close_to_player(x, y, OVERWORLD_HOSTILE_SPAWN_MIN_DIST_SQ)
                || self.entity_collides_with_world(x, y, 0.3, 2.7)
                || !Self::ambient_spawn_clearance(self.world.get_block(wx, head_y))
                || self.world.block_light_at(wx, head_y) != 0
            {
                continue;
            }
            self.endermen.push(Enderman::new(x, y));
            return true;
        }
        false
    }

    fn can_refill_overworld_hostiles(&self) -> bool {
        // Measure local pressure separately from the resident population. A full
        // save may contain only cave mobs; those must not block every night.
        let mut total = 0;
        let mut local = 0;
        let outdoor_night = !(4000.0..=20000.0).contains(&self.time_of_day)
            && self.world.has_open_sky(
                self.player.x.floor() as i32,
                (self.player.y - 1.0).floor() as i32,
            );
        for (x, y) in self
            .zombies
            .iter()
            .map(|m| (m.x, m.y))
            .chain(self.creepers.iter().map(|m| (m.x, m.y)))
            .chain(self.skeletons.iter().map(|m| (m.x, m.y)))
            .chain(self.spiders.iter().map(|m| (m.x, m.y)))
            .chain(self.slimes.iter().map(|m| (m.x, m.y)))
        {
            total += 1;
            local += usize::from(
                (x - self.player.x).abs() <= 40.0
                    && if outdoor_night {
                        self.world
                            .has_open_sky(x.floor() as i32, (y - 1.0).floor() as i32)
                    } else {
                        (y - self.player.y).abs() <= 16.0
                    },
            );
        }
        let cap = self.scaled_hostile_cap(OVERWORLD_HOSTILE_CAP);
        local < Self::spawn_refill_threshold(cap, 2, 3)
            && (total < cap || (total == cap && self.inactive_overworld_hostile().is_some()))
    }

    fn try_spawn_overworld_passive_mob(
        &mut self,
        wx: i32,
        spawn_y: f64,
        rng: &mut impl Rng,
    ) -> bool {
        let spawn_x = wx as f64 + 0.5;
        let biome = self.world.get_biome(wx);
        let roll = rng.gen_range(0..100);
        match biome {
            BiomeType::Desert | BiomeType::Ocean | BiomeType::River => false,
            BiomeType::Plains | BiomeType::Forest => {
                if roll < 24 {
                    self.cows.push(Cow::new(spawn_x, spawn_y));
                } else if roll < 46 {
                    self.sheep.push(Sheep::new(spawn_x, spawn_y));
                } else if roll < 78 {
                    self.pigs.push(Pig::new(spawn_x, spawn_y));
                } else {
                    self.chickens.push(Chicken::new(spawn_x, spawn_y));
                }
                true
            }
            BiomeType::Swamp => {
                if roll < 16 {
                    self.cows.push(Cow::new(spawn_x, spawn_y));
                } else if roll < 30 {
                    self.sheep.push(Sheep::new(spawn_x, spawn_y));
                } else if roll < 66 {
                    self.pigs.push(Pig::new(spawn_x, spawn_y));
                } else {
                    self.chickens.push(Chicken::new(spawn_x, spawn_y));
                }
                true
            }
            BiomeType::Jungle => {
                if roll < 14 {
                    self.cows.push(Cow::new(spawn_x, spawn_y));
                } else if roll < 24 {
                    self.sheep.push(Sheep::new(spawn_x, spawn_y));
                } else if roll < 52 {
                    self.pigs.push(Pig::new(spawn_x, spawn_y));
                } else {
                    self.chickens.push(Chicken::new(spawn_x, spawn_y));
                }
                true
            }
            BiomeType::Tundra | BiomeType::Taiga => {
                if roll < 18 {
                    self.cows.push(Cow::new(spawn_x, spawn_y));
                } else if roll < 54 {
                    self.sheep.push(Sheep::new(spawn_x, spawn_y));
                } else if roll < 70 {
                    self.pigs.push(Pig::new(spawn_x, spawn_y));
                } else {
                    self.chickens.push(Chicken::new(spawn_x, spawn_y));
                }
                true
            }
            BiomeType::ExtremeHills => {
                if roll < 20 {
                    self.cows.push(Cow::new(spawn_x, spawn_y));
                } else if roll < 58 {
                    self.sheep.push(Sheep::new(spawn_x, spawn_y));
                } else if roll < 74 {
                    self.pigs.push(Pig::new(spawn_x, spawn_y));
                } else {
                    self.chickens.push(Chicken::new(spawn_x, spawn_y));
                }
                true
            }
        }
    }

    fn try_spawn_overworld_squid(&mut self, wx: i32, rng: &mut impl Rng) -> bool {
        if !matches!(
            self.world.get_biome(wx),
            BiomeType::Ocean | BiomeType::River | BiomeType::Swamp
        ) {
            return false;
        }

        let Some(spawn_y) = self.find_water_spawn_for_squid(wx) else {
            return false;
        };
        let spawn_x = wx as f64 + 0.5;
        let spawn_y = spawn_y + rng.gen_range(-0.15..=0.15);
        if self.is_spawn_too_close_to_player(spawn_x, spawn_y, OVERWORLD_SPAWN_MIN_DIST_SQ) {
            return false;
        }
        self.squids.push(Squid::new(spawn_x, spawn_y));
        true
    }

    fn try_spawn_overworld_wolf(&mut self, wx: i32, spawn_y: f64, rng: &mut impl Rng) -> bool {
        if !matches!(
            self.world.get_biome(wx),
            BiomeType::Forest | BiomeType::Taiga | BiomeType::Tundra
        ) {
            return false;
        }

        if !rng.gen_bool(0.55) {
            return false;
        }

        let spawn_x = wx as f64 + 0.5;
        if self.is_spawn_too_close_to_player(spawn_x, spawn_y, OVERWORLD_SPAWN_MIN_DIST_SQ) {
            return false;
        }
        self.wolves.push(Wolf::new(spawn_x, spawn_y));
        true
    }

    fn try_spawn_overworld_ocelot(&mut self, wx: i32, spawn_y: f64, rng: &mut impl Rng) -> bool {
        if self.world.get_biome(wx) != BiomeType::Jungle {
            return false;
        }

        if !rng.gen_bool(0.6) {
            return false;
        }

        let spawn_x = wx as f64 + 0.5;
        if self.is_spawn_too_close_to_player(spawn_x, spawn_y, OVERWORLD_SPAWN_MIN_DIST_SQ) {
            return false;
        }
        self.ocelots.push(Ocelot::new(spawn_x, spawn_y));
        true
    }

    fn find_villager_spawn_surface_near_home(
        &self,
        home_x: i32,
        home_y: i32,
        target_x: i32,
    ) -> Option<f64> {
        let scan_start_y = home_y.clamp(2, CHUNK_HEIGHT as i32 - 3);
        let scan_end_y = (home_y + 6).clamp(scan_start_y, CHUNK_HEIGHT as i32 - 3);
        let target_floor_y = home_y + 1;
        let mut best: Option<(i32, f64)> = None;

        for y in scan_start_y..=scan_end_y {
            let ground = self.world.get_block(target_x, y);
            if !ground.is_solid() || ground.is_fluid() {
                continue;
            }
            if self.world.get_block(target_x, y - 1) != BlockType::Air
                || self.world.get_block(target_x, y - 2) != BlockType::Air
            {
                continue;
            }

            let score = (target_x - home_x).abs() * 3 + (y - target_floor_y).abs() * 2;
            let spawn_y = y as f64 - 0.1;
            match best {
                None => best = Some((score, spawn_y)),
                Some((best_score, _)) if score < best_score => best = Some((score, spawn_y)),
                _ => {}
            }
        }

        best.map(|(_, spawn_y)| spawn_y)
    }

    fn is_village_hut_anchor(&self, door_x: i32, door_y: i32) -> bool {
        let Some((DoorKind::Wood, base_y)) = self.door_base_at(door_x, door_y) else {
            return false;
        };
        if base_y != door_y {
            return false;
        }
        if !self.world.get_block(door_x, base_y + 1).is_solid() {
            return false;
        }

        let mut has_chest = false;
        let mut has_station = false;
        let mut has_glass = false;
        for wx in (door_x - 15)..=(door_x + 15) {
            // Furniture in a neighboring building does not turn a market gate into a home.
            if !(door_x.min(wx)..=door_x.max(wx))
                .all(|x| !self.is_exposed_to_sky(x as f64 + 0.5, base_y as f64 - 1.0))
            {
                continue;
            }
            for wy in (base_y - 5)..=(base_y + 2) {
                match self.world.get_block(wx, wy) {
                    BlockType::Chest => has_chest = true,
                    BlockType::CraftingTable | BlockType::Bed => has_station = true,
                    BlockType::Glass => has_glass = true,
                    _ => {}
                }
                if has_chest && has_station && has_glass {
                    return true;
                }
            }
        }
        false
    }

    fn collect_village_hut_anchors(&self, center_x: i32) -> Vec<(i32, i32)> {
        let mut anchors: Vec<(i32, i32)> = Vec::new();
        let scan_left = center_x - VILLAGE_SCAN_RADIUS;
        let scan_right = center_x + VILLAGE_SCAN_RADIUS;
        for wx in scan_left..=scan_right {
            for wy in 6..112 {
                let Some((DoorKind::Wood, base_y)) = self.door_base_at(wx, wy) else {
                    continue;
                };
                if base_y != wy {
                    continue;
                }
                if !self.is_village_hut_anchor(wx, base_y) {
                    continue;
                }
                if anchors.iter().any(|&(ax, ay)| {
                    (wx - ax).abs() <= 6 && (base_y - ay).abs() <= 4
                        || (wx - ax).abs() <= 16
                            && base_y == ay
                            && (ax..=wx).all(|x| {
                                !self.is_exposed_to_sky(x as f64 + 0.5, base_y as f64 - 1.0)
                            })
                }) {
                    continue;
                }
                anchors.push((wx, base_y));
            }
        }
        anchors
    }

    fn find_villager_entry_surfaces_near_home(
        &self,
        home_x: i32,
        home_y: i32,
    ) -> Option<((i32, f64), (i32, f64))> {
        let mut indoor: Option<(i32, i32, f64)> = None;
        let mut outdoor: Option<(i32, i32, f64)> = None;

        for candidate_x in [home_x - 2, home_x - 1, home_x + 1, home_x + 2] {
            let Some(spawn_y) =
                self.find_villager_spawn_surface_near_home(home_x, home_y, candidate_x)
            else {
                continue;
            };
            let distance_from_door = (candidate_x - home_x).abs();
            if self.is_exposed_to_sky(candidate_x as f64 + 0.5, spawn_y - 1.0) {
                match outdoor {
                    None => outdoor = Some((distance_from_door, candidate_x, spawn_y)),
                    Some((best_dist, _, _)) if distance_from_door > best_dist => {
                        outdoor = Some((distance_from_door, candidate_x, spawn_y));
                    }
                    _ => {}
                }
            } else {
                match indoor {
                    None => indoor = Some((distance_from_door, candidate_x, spawn_y)),
                    Some((best_dist, _, _)) if distance_from_door > best_dist => {
                        indoor = Some((distance_from_door, candidate_x, spawn_y));
                    }
                    _ => {}
                }
            }
        }

        indoor
            .zip(outdoor)
            .map(|((_, indoor_x, indoor_y), (_, outdoor_x, outdoor_y))| {
                ((indoor_x, indoor_y), (outdoor_x, outdoor_y))
            })
    }

    fn nearest_village_anchor(x: f64, y: f64, anchors: &[(i32, i32)]) -> Option<(i32, i32, f64)> {
        let mut best = None;
        let mut best_dist_sq = f64::INFINITY;
        for &(anchor_x, anchor_y) in anchors {
            let dx = x - (anchor_x as f64 + 0.5);
            let dy = y - (anchor_y as f64 - 0.1);
            let dist_sq = dx * dx + dy * dy;
            if dist_sq < best_dist_sq {
                best_dist_sq = dist_sq;
                best = Some((anchor_x, anchor_y, dist_sq.sqrt()));
            }
        }
        best
    }

    fn find_villager_shelter_surface_near_home(
        &self,
        home_x: i32,
        home_y: i32,
    ) -> Option<(i32, f64)> {
        if let Some((indoor, _)) = self.find_villager_entry_surfaces_near_home(home_x, home_y) {
            return Some(indoor);
        }

        let mut best: Option<(i32, i32, f64)> = None; // (score, x, spawn_y)
        for wx in (home_x - 4)..=(home_x + 4) {
            let scan_start_y = (home_y - 1).clamp(2, CHUNK_HEIGHT as i32 - 3);
            let scan_end_y = (home_y + 12).clamp(scan_start_y, CHUNK_HEIGHT as i32 - 3);
            for y in scan_start_y..=scan_end_y {
                let ground = self.world.get_block(wx, y);
                if !ground.is_solid() || ground.is_fluid() {
                    continue;
                }
                if self.world.get_block(wx, y - 1) != BlockType::Air
                    || self.world.get_block(wx, y - 2) != BlockType::Air
                {
                    continue;
                }
                let spawn_y = y as f64 - 0.1;
                if self.is_exposed_to_sky(wx as f64 + 0.5, spawn_y - 1.0) {
                    continue;
                }
                let score = (wx - home_x).abs() * 3 + (y - home_y).abs();
                match best {
                    None => best = Some((score, wx, spawn_y)),
                    Some((best_score, _, _)) if score < best_score => {
                        best = Some((score, wx, spawn_y));
                    }
                    _ => {}
                }
            }
        }
        best.map(|(_, x, y)| (x, y))
    }

    fn find_villager_outdoor_surface_near_home(
        &self,
        home_x: i32,
        home_y: i32,
    ) -> Option<(i32, f64)> {
        if let Some((_, outdoor)) = self.find_villager_entry_surfaces_near_home(home_x, home_y) {
            return Some(outdoor);
        }

        let mut best: Option<(i32, i32, f64)> = None;
        for wx in (home_x - 6)..=(home_x + 6) {
            let scan_start_y = home_y.clamp(2, CHUNK_HEIGHT as i32 - 3);
            let scan_end_y = (home_y + 8).clamp(scan_start_y, CHUNK_HEIGHT as i32 - 3);
            for y in scan_start_y..=scan_end_y {
                let ground = self.world.get_block(wx, y);
                if !ground.is_solid() || ground.is_fluid() {
                    continue;
                }
                if self.world.get_block(wx, y - 1) != BlockType::Air
                    || self.world.get_block(wx, y - 2) != BlockType::Air
                {
                    continue;
                }
                let spawn_y = y as f64 - 0.1;
                if !self.is_exposed_to_sky(wx as f64 + 0.5, spawn_y - 1.0) {
                    continue;
                }
                let score = (wx - home_x).abs() * 3 + (y - (home_y + 1)).abs() * 2;
                match best {
                    None => best = Some((score, wx, spawn_y)),
                    Some((best_score, _, _)) if score < best_score => {
                        best = Some((score, wx, spawn_y));
                    }
                    _ => {}
                }
            }
        }
        best.map(|(_, x, y)| (x, y))
    }

    fn should_villager_seek_shelter(&self, x: f64, _y: f64, is_day: bool) -> bool {
        // Reaching a roof makes the actor dry, not the weather clear. Keep
        // shelter intent while local precipitation lasts instead of walking
        // straight back outside after entering.
        !is_day || self.precipitation_at(x.floor() as i32) != PrecipitationType::None
    }

    fn villager_shelter_target(&self, index: usize) -> Option<(i32, f64)> {
        let v = &self.villagers[index];
        let base = self.find_villager_shelter_surface_near_home(v.home_x, v.home_y)?;
        let rank = self.villagers[..index]
            .iter()
            .filter(|other| {
                other.health > 0.0 && other.home_x == v.home_x && other.home_y == v.home_y
            })
            .count();
        let mut seats = Vec::new();
        for x in v.home_x - 10..=v.home_x + 10 {
            if (x - v.home_x).abs() < 2 {
                continue;
            }
            let Some(y) = self.find_villager_spawn_surface_near_home(v.home_x, v.home_y, x) else {
                continue;
            };
            if (y - base.1).abs() > 0.25 || self.is_exposed_to_sky(x as f64 + 0.5, y - 1.0) {
                continue;
            }
            // Stay in this room, rather than selecting a roof beyond a wall or
            // another entrance. A dwelling's occupants get stable separate seats.
            if (x.min(base.0)..=x.max(base.0)).any(|col| {
                self.entity_collides_with_world(col as f64 + 0.5, y, 0.3, 1.8)
                    || self.is_exposed_to_sky(col as f64 + 0.5, y - 1.0)
            }) {
                continue;
            }
            seats.push((x, y));
        }
        // Fill the far end first. A villager parked beside the door otherwise
        // blocks the queue trying to reach the more distant assigned seats.
        seats.sort_by_key(|&(x, _)| (std::cmp::Reverse((x - base.0).abs()), x));
        let occupants = self
            .villagers
            .iter()
            .filter(|other| {
                other.health > 0.0 && other.home_x == v.home_x && other.home_y == v.home_y
            })
            .count();
        let stride = if seats.len() >= occupants * 2 { 2 } else { 1 };
        seats.get(rank * stride).copied().or(Some(base))
    }

    fn try_open_wood_door_for_entity(&mut self, x: f64, y: f64, vx: f64) -> bool {
        if vx.abs() < 0.01 {
            return false;
        }
        let direction = if vx >= 0.0 { 1 } else { -1 };
        let lead_probe_x = if direction > 0 {
            (x + 0.45).floor() as i32
        } else {
            (x - 0.45).floor() as i32
        };
        let feet_y = y.floor() as i32;
        let mut opened = false;
        for probe_x in [lead_probe_x, x.floor() as i32] {
            for by in [feet_y, feet_y - 1] {
                match self.world.get_block(probe_x, by) {
                    BlockType::WoodDoor(false) => {
                        if self.set_door_open_state(probe_x, by, true) {
                            self.mark_villager_door_active(probe_x, by);
                            opened = true;
                        }
                    }
                    BlockType::WoodDoor(true) => self.mark_villager_door_active(probe_x, by),
                    _ => {}
                }
            }
        }
        opened
    }

    fn update_villager_population(&mut self, rng: &mut impl Rng, is_day: bool) {
        if self.current_dimension != Dimension::Overworld {
            self.overworld_villager_spawn_timer = OVERWORLD_VILLAGER_RESPAWN_BASE;
            return;
        }

        let player_wx = self.player.x.floor() as i32;
        let anchors = self.collect_village_hut_anchors(player_wx);
        if anchors.is_empty() {
            return;
        }

        for villager in &mut self.villagers {
            let villager_home_dist = ((villager.x - (villager.home_x as f64 + 0.5)).powi(2)
                + (villager.y - (villager.home_y as f64 - 0.1)).powi(2))
            .sqrt();
            let home_anchor_still_valid = anchors.iter().any(|&(anchor_x, anchor_y)| {
                (villager.home_x - anchor_x).abs() <= 1 && (villager.home_y - anchor_y).abs() <= 2
            });
            if let Some((anchor_x, anchor_y, dist)) =
                Self::nearest_village_anchor(villager.x, villager.y, &anchors)
                && dist < VILLAGER_HOME_ASSIGN_MAX_DIST
                && (!home_anchor_still_valid
                    || villager_home_dist > VILLAGER_HOME_REASSIGN_FORCE_DIST
                    || dist + VILLAGER_HOME_REASSIGN_HYSTERESIS < villager_home_dist)
            {
                villager.set_home(anchor_x, anchor_y);
            }
        }

        if !self.can_spawn_mobs() || !is_day {
            return;
        }

        let village_has_local_villagers = anchors.iter().any(|&(anchor_x, anchor_y)| {
            self.villagers.iter().any(|villager| {
                (villager.home_x - anchor_x).abs() <= 1 && (villager.home_y - anchor_y).abs() <= 2
            })
        });
        let bootstrap_empty_village = !village_has_local_villagers;

        if self.overworld_villager_spawn_timer > 0 && !bootstrap_empty_village {
            self.overworld_villager_spawn_timer -= 1;
            return;
        }
        self.overworld_villager_spawn_timer =
            OVERWORLD_VILLAGER_RESPAWN_BASE + rng.gen_range(0..72);

        let villager_cap = OVERWORLD_VILLAGER_CAP.min(anchors.len() * VILLAGER_MAX_PER_HUT);
        if self.villagers.len() >= villager_cap {
            return;
        }

        for (anchor_x, anchor_y) in anchors {
            if self.villagers.len() >= villager_cap {
                break;
            }
            let local_count = self
                .villagers
                .iter()
                .filter(|villager| {
                    (villager.home_x - anchor_x).abs() <= 1
                        && (villager.home_y - anchor_y).abs() <= 2
                })
                .count();
            let local_target = if bootstrap_empty_village {
                1
            } else {
                VILLAGER_MAX_PER_HUT
            };
            if local_count >= local_target {
                continue;
            }

            let mut spawned_here = 0usize;
            for _ in 0..6 {
                if self.villagers.len() >= villager_cap
                    || local_count + spawned_here >= local_target
                {
                    break;
                }
                let wx = anchor_x + rng.gen_range(-3..=3);
                let Some(spawn_y) =
                    self.find_villager_spawn_surface_near_home(anchor_x, anchor_y, wx)
                else {
                    continue;
                };
                if (spawn_y - anchor_y as f64).abs() > 8.0 {
                    continue;
                }
                let spawn_x = wx as f64 + 0.5;
                if self.is_spawn_too_close_to_player(spawn_x, spawn_y, 6.0 * 6.0) {
                    continue;
                }
                self.villagers
                    .push(Villager::new(spawn_x, spawn_y, anchor_x, anchor_y));
                spawned_here += 1;
            }
        }
    }

    fn find_nether_spawn_surface_for_hostile(&self, x: i32) -> Option<f64> {
        for y in 3..(127 - 3) {
            let ground = self.world.get_block(x, y);
            if !matches!(
                ground,
                BlockType::Netherrack
                    | BlockType::SoulSand
                    | BlockType::Gravel
                    | BlockType::Glowstone
            ) {
                continue;
            }
            if self.world.get_block(x, y - 1) != BlockType::Air
                || self.world.get_block(x, y - 2) != BlockType::Air
            {
                continue;
            }
            // Avoid lava-neighbor spawn tiles for less instant mob deaths.
            if matches!(self.world.get_block(x, y + 1), BlockType::Lava(_))
                || matches!(self.world.get_block(x - 1, y), BlockType::Lava(_))
                || matches!(self.world.get_block(x + 1, y), BlockType::Lava(_))
            {
                continue;
            }
            return Some(y as f64 - 0.1);
        }
        None
    }

    fn find_nether_air_spawn_for_ghast(&self, x: i32) -> Option<f64> {
        let preferred_y = (self.player.y - 2.5 + 0.6).round() as i32;
        let mut heights: Vec<i32> = (8..96).collect();
        heights.sort_by_key(|y| (y - preferred_y).abs());
        for y in heights {
            if self.world.get_block(x, y) != BlockType::Air
                || self.world.get_block(x, y - 1) != BlockType::Air
                || self.world.get_block(x, y - 2) != BlockType::Air
            {
                continue;
            }
            let mut clear_volume = true;
            for dx in -1..=1 {
                for dy in -1..=1 {
                    if self.world.get_block(x + dx, y + dy).is_solid() {
                        clear_volume = false;
                        break;
                    }
                }
                if !clear_volume {
                    break;
                }
            }
            if clear_volume
                && !self.entity_collides_with_world(x as f64 + 0.5, y as f64 - 0.6, 0.875, 1.875)
            {
                return Some(y as f64 - 0.6);
            }
        }
        None
    }

    fn find_nether_air_spawn_for_blaze(&self, x: i32) -> Option<f64> {
        for y in 10..96 {
            if self.world.get_block(x, y) != BlockType::Air
                || self.world.get_block(x, y - 1) != BlockType::Air
            {
                continue;
            }
            let below = self.world.get_block(x, y + 1);
            if !matches!(
                below,
                BlockType::Netherrack
                    | BlockType::SoulSand
                    | BlockType::Gravel
                    | BlockType::Glowstone
                    | BlockType::StoneBricks
                    | BlockType::StoneSlab
                    | BlockType::StoneStairs
                    | BlockType::Stone
                    | BlockType::Cobblestone
            ) {
                continue;
            }
            if self.world.get_block(x - 1, y).is_solid()
                && self.world.get_block(x + 1, y).is_solid()
            {
                continue;
            }
            if !self.entity_collides_with_world(x as f64 + 0.5, y as f64 - 0.6, 0.5, 1.75) {
                return Some(y as f64 - 0.6);
            }
        }
        None
    }

    fn find_end_spawn_surface_for_enderman(&self, x: i32) -> Option<f64> {
        for y in 2..(127 - 3) {
            if self.world.get_block(x, y) != BlockType::EndStone {
                continue;
            }
            if self.world.get_block(x, y - 1) != BlockType::Air
                || self.world.get_block(x, y - 2) != BlockType::Air
                || self.world.get_block(x, y - 3) != BlockType::Air
            {
                continue;
            }
            return Some(y as f64 - 0.1);
        }
        None
    }

    fn is_spawn_too_close_to_player(&self, x: f64, y: f64, min_dist_sq: f64) -> bool {
        let dx = x - self.player.x;
        let dy = y - self.player.y;
        (dx * dx + dy * dy) < min_dist_sq
            || self
                .other_players_here()
                .any(|p| (x - p.player.x).powi(2) + (y - p.player.y).powi(2) < min_dist_sq)
    }

    fn nearby_blaze_count(&self, x: f64, y: f64, radius_sq: f64) -> usize {
        self.blazes
            .iter()
            .filter(|blaze| {
                let dx = blaze.x - x;
                let dy = blaze.y - y;
                (dx * dx + dy * dy) <= radius_sq
            })
            .count()
    }

    fn nearby_zombie_count(&self, x: f64, y: f64, radius_sq: f64) -> usize {
        self.zombies
            .iter()
            .filter(|zombie| {
                let dx = zombie.x - x;
                let dy = zombie.y - y;
                (dx * dx + dy * dy) <= radius_sq
            })
            .count()
    }

    fn nearby_skeleton_count(&self, x: f64, y: f64, radius_sq: f64) -> usize {
        self.skeletons
            .iter()
            .filter(|skeleton| {
                let dx = skeleton.x - x;
                let dy = skeleton.y - y;
                (dx * dx + dy * dy) <= radius_sq
            })
            .count()
    }

    fn update_dungeon_spawners(&mut self, rng: &mut impl Rng) {
        if self.current_dimension != Dimension::Overworld || !self.can_spawn_hostiles() {
            self.dungeon_spawner_timer = 0;
            return;
        }
        if self.dungeon_spawner_timer > 0 {
            self.dungeon_spawner_timer -= 1;
            return;
        }

        self.dungeon_spawner_timer = 90 + rng.gen_range(0..61);
        let center_x = self.player.x.floor() as i32;
        let center_y = self.player.y.floor() as i32;
        let mut spawners = Vec::new();
        for wx in (center_x - 96)..=(center_x + 96) {
            for wy in (center_y - 40).max(4)..=(center_y + 40).min(CHUNK_HEIGHT as i32 - 4) {
                match self.world.get_block(wx, wy) {
                    BlockType::ZombieSpawner | BlockType::SkeletonSpawner => {
                        let dist = ((wx as f64 + 0.5 - self.player.x).powi(2)
                            + (wy as f64 - self.player.y).powi(2))
                        .sqrt();
                        if dist <= 20.0 && self.world.block_light_at(wx, wy - 1) == 0 {
                            spawners.push((wx, wy, self.world.get_block(wx, wy)));
                        }
                    }
                    _ => {}
                }
            }
        }
        if spawners.is_empty() {
            return;
        }

        let (spawner_x, spawner_y, spawner_block) = spawners[rng.gen_range(0..spawners.len())];
        let local_count = self.nearby_zombie_count(
            spawner_x as f64 + 0.5,
            spawner_y as f64,
            OVERWORLD_DUNGEON_SPAWNER_LOCAL_CLUSTER_RADIUS_SQ,
        ) + self.nearby_skeleton_count(
            spawner_x as f64 + 0.5,
            spawner_y as f64,
            OVERWORLD_DUNGEON_SPAWNER_LOCAL_CLUSTER_RADIUS_SQ,
        );
        if local_count >= OVERWORLD_DUNGEON_SPAWNER_LOCAL_CLUSTER_LIMIT {
            return;
        }
        for _ in 0..8 {
            let target_x = spawner_x + rng.gen_range(-4..=4);
            let Some(spawn_y) = self.dark_spawn_floor_near(target_x, spawner_y, 9, false) else {
                continue;
            };
            if (spawn_y - spawner_y as f64).abs() > 9.0 {
                continue;
            }
            let spawn_x = target_x as f64 + 0.5;
            if self.is_spawn_too_close_to_player(spawn_x, spawn_y, 4.0 * 4.0) {
                continue;
            }
            match spawner_block {
                BlockType::ZombieSpawner => {
                    if self.nearby_zombie_count(
                        spawner_x as f64 + 0.5,
                        spawner_y as f64,
                        OVERWORLD_DUNGEON_SPAWNER_LOCAL_CLUSTER_RADIUS_SQ,
                    ) >= OVERWORLD_DUNGEON_SPAWNER_LOCAL_CLUSTER_LIMIT
                    {
                        continue;
                    }
                    self.zombies.push(Zombie::new(spawn_x, spawn_y));
                }
                BlockType::SkeletonSpawner => {
                    if self.nearby_skeleton_count(
                        spawner_x as f64 + 0.5,
                        spawner_y as f64,
                        OVERWORLD_DUNGEON_SPAWNER_LOCAL_CLUSTER_RADIUS_SQ,
                    ) >= OVERWORLD_DUNGEON_SPAWNER_LOCAL_CLUSTER_LIMIT
                    {
                        continue;
                    }
                    self.skeletons.push(Skeleton::new(spawn_x, spawn_y));
                }
                _ => {}
            }
            break;
        }
    }

    fn has_block_in_box(
        &self,
        block: BlockType,
        min_x: i32,
        max_x: i32,
        min_y: i32,
        max_y: i32,
    ) -> bool {
        for wx in min_x..=max_x {
            for wy in min_y.max(2)..=max_y.min(CHUNK_HEIGHT as i32 - 3) {
                if self.world.get_block(wx, wy) == block {
                    return true;
                }
            }
        }
        false
    }

    fn nether_blaze_hot_zone_near(&self, center_x: i32, center_y: i32) -> bool {
        self.has_block_in_box(
            BlockType::BlazeSpawner,
            center_x - NETHER_BLAZE_SPAWNER_HOT_ZONE_RADIUS_X,
            center_x + NETHER_BLAZE_SPAWNER_HOT_ZONE_RADIUS_X,
            center_y - NETHER_BLAZE_SPAWNER_HOT_ZONE_RADIUS_Y,
            center_y + NETHER_BLAZE_SPAWNER_HOT_ZONE_RADIUS_Y,
        )
    }

    fn nether_ambient_roll_thresholds(
        player_in_fortress: bool,
        blaze_hot_zone: bool,
    ) -> (i32, i32) {
        if player_in_fortress && blaze_hot_zone {
            (50, 78)
        } else if player_in_fortress {
            (40, 66)
        } else {
            (56, 82)
        }
    }

    fn target_end_enderman_cap(&self) -> usize {
        if self.dragon_defeated {
            END_ENDERMAN_CAP
        } else {
            END_PRE_DRAGON_ENDERMAN_CAP
        }
    }

    fn update_silverfish_spawners(&mut self, rng: &mut impl Rng) {
        if self.current_dimension != Dimension::Overworld || !self.can_spawn_hostiles() {
            self.silverfish_spawner_timer = 0;
            return;
        }
        if self.silverfish.len() >= self.scaled_hostile_cap(OVERWORLD_SILVERFISH_CAP) {
            return;
        }
        if self.silverfish_spawner_timer > 0 {
            self.silverfish_spawner_timer -= 1;
            return;
        }

        self.silverfish_spawner_timer = 90 + rng.gen_range(0..61);
        let scan_left = self.player.x.floor() as i32 - 96;
        let scan_right = self.player.x.floor() as i32 + 96;
        let mut spawners = Vec::new();
        for wx in scan_left..=scan_right {
            for wy in (crate::world::STRONGHOLD_ROOM_TOP_Y - 10)
                ..=(crate::world::STRONGHOLD_ROOM_BOTTOM_Y + 4)
            {
                if self.world.get_block(wx, wy) == BlockType::SilverfishSpawner
                    && (wx as f64 + 0.5 - self.player.x).hypot(wy as f64 - self.player.y) <= 20.0
                    && self.world.block_light_at(wx, wy - 1) == 0
                    && self
                        .silverfish
                        .iter()
                        .filter(|m| (m.x - wx as f64).hypot(m.y - wy as f64) < 14.0)
                        .count()
                        < 4
                {
                    spawners.push((wx, wy));
                }
            }
        }
        if spawners.is_empty() {
            return;
        }

        let (spawner_x, spawner_y) = spawners[rng.gen_range(0..spawners.len())];
        for _ in 0..8 {
            let target_x = spawner_x + rng.gen_range(-4..=4);
            let Some(spawn_y) = self.dark_spawn_floor_near(target_x, spawner_y, 9, false) else {
                continue;
            };
            if (spawn_y - spawner_y as f64).abs() > 9.0 {
                continue;
            }
            let p_dist = ((target_x as f64 + 0.5 - self.player.x).powi(2)
                + (spawn_y - self.player.y).powi(2))
            .sqrt();
            if p_dist < 4.0 {
                continue;
            }
            self.silverfish
                .push(Silverfish::new(target_x as f64 + 0.5, spawn_y));
            break;
        }
    }

    fn update_blaze_spawners(&mut self, rng: &mut impl Rng) {
        if self.current_dimension != Dimension::Nether || !self.can_spawn_hostiles() {
            self.blaze_spawner_timer = 0;
            return;
        }
        if self.blazes.len() >= self.scaled_hostile_cap(NETHER_BLAZE_CAP) {
            return;
        }
        if self.blaze_spawner_timer > 0 {
            self.blaze_spawner_timer -= 1;
            return;
        }

        self.blaze_spawner_timer = 18 + rng.gen_range(0..16);
        let center_x = self.player.x.floor() as i32;
        let center_y = self.player.y.floor() as i32;
        let mut spawners = Vec::new();
        for wx in (center_x - 84)..=(center_x + 84) {
            for wy in (center_y - 24).max(4)..=(center_y + 24).min(CHUNK_HEIGHT as i32 - 4) {
                if self.world.get_block(wx, wy) == BlockType::BlazeSpawner {
                    let dist = ((wx as f64 + 0.5 - self.player.x).powi(2)
                        + (wy as f64 - self.player.y).powi(2))
                    .sqrt();
                    if dist <= 36.0
                        && self.nearby_blaze_count(
                            wx as f64 + 0.5,
                            wy as f64,
                            NETHER_BLAZE_LOCAL_CLUSTER_RADIUS_SQ,
                        ) < NETHER_BLAZE_LOCAL_CLUSTER_LIMIT
                    {
                        spawners.push((wx, wy));
                    }
                }
            }
        }
        if spawners.is_empty() {
            return;
        }

        let (spawner_x, spawner_y) = spawners[rng.gen_range(0..spawners.len())];
        for _ in 0..8 {
            let target_x = spawner_x + rng.gen_range(-4..=4);
            let Some(spawn_y) = self.find_nether_air_spawn_for_blaze(target_x) else {
                continue;
            };
            if (spawn_y - spawner_y as f64).abs() > 10.0 {
                continue;
            }
            if !self
                .world
                .is_nether_fortress_zone(target_x, spawn_y.floor() as i32)
            {
                continue;
            }
            let spawn_x = target_x as f64 + 0.5;
            if self.is_spawn_too_close_to_player(spawn_x, spawn_y, 7.5 * 7.5) {
                continue;
            }
            if self.nearby_blaze_count(
                spawner_x as f64 + 0.5,
                spawner_y as f64,
                NETHER_BLAZE_LOCAL_CLUSTER_RADIUS_SQ,
            ) >= NETHER_BLAZE_LOCAL_CLUSTER_LIMIT
            {
                continue;
            }
            self.blazes.push(Blaze::new(spawn_x, spawn_y));
            break;
        }
    }

    fn update_ambient_respawns(&mut self, rng: &mut impl Rng, is_day: bool) {
        let player_wx = self.player.x.floor() as i32;
        let mob_spawning_enabled = self.can_spawn_mobs();
        let hostile_spawning_enabled = self.can_spawn_hostiles();
        match self.current_dimension {
            Dimension::Overworld => {
                let local_biome = self.overworld_biome_at_player();
                let passive_cap = self.tuned_overworld_passive_cap(local_biome);
                let squid_cap = self.tuned_overworld_squid_cap(local_biome);
                let wolf_cap = self.tuned_overworld_wolf_cap(local_biome);
                let ocelot_cap = self.tuned_overworld_ocelot_cap(local_biome);
                let passive_refill_cap = Self::spawn_refill_threshold(passive_cap, 2, 3);
                let squid_refill_cap = Self::spawn_refill_threshold(squid_cap, 2, 3);
                let wolf_refill_cap = Self::spawn_refill_threshold(wolf_cap, 1, 2);
                let ocelot_refill_cap = Self::spawn_refill_threshold(ocelot_cap, 1, 2);

                if hostile_spawning_enabled {
                    if self.overworld_hostile_spawn_timer > 0 {
                        self.overworld_hostile_spawn_timer -= 1;
                    } else {
                        self.overworld_hostile_spawn_timer = self
                            .scaled_hostile_respawn_base(OVERWORLD_HOSTILE_RESPAWN_BASE)
                            + rng.gen_range(0..20);
                        // Share the slow refill clock, not ordinary mobs' slot
                        // budget. Existing terrain must still provide pearls.
                        self.try_refill_overworld_enderman(rng);
                        let slime_cap = self.scaled_hostile_cap(OVERWORLD_SLIME_CAP);
                        if self.can_refill_overworld_hostiles() {
                            for _ in 0..6 {
                                // A 24-block exclusion placed every stationary
                                // spawn beyond the zombie's 20-block awareness.
                                let offset = rng.gen_range(12..=40);
                                let wx =
                                    player_wx + if rng.gen_bool(0.5) { offset } else { -offset };
                                let can_try_slime = Self::is_slime_chunk(wx);
                                let spawn_y = if can_try_slime {
                                    self.find_spawn_surface_for_slime(wx)
                                        .or_else(|| self.find_dark_overworld_spawn(wx))
                                } else {
                                    self.find_dark_overworld_spawn(wx)
                                };
                                let Some(spawn_y) = spawn_y else {
                                    continue;
                                };
                                let spawn_x = wx as f64 + 0.5;
                                if self.is_spawn_too_close_to_player(
                                    spawn_x,
                                    spawn_y,
                                    OVERWORLD_HOSTILE_SPAWN_MIN_DIST_SQ,
                                ) {
                                    continue;
                                }
                                if !self.reserve_overworld_hostile_slot() {
                                    break;
                                }
                                if can_try_slime
                                    && self.slimes.len() < slime_cap
                                    && spawn_y >= 64.0
                                    && self.should_spawn_overworld_slime(spawn_y, rng)
                                {
                                    self.slimes.push(Slime::new(
                                        spawn_x,
                                        spawn_y,
                                        Self::random_overworld_slime_size(rng),
                                    ));
                                } else {
                                    self.spawn_overworld_hostile_at(spawn_x, spawn_y, rng);
                                }
                                break;
                            }
                        }
                    }
                } else {
                    self.overworld_hostile_spawn_timer =
                        self.scaled_hostile_respawn_base(OVERWORLD_HOSTILE_RESPAWN_BASE);
                }

                if is_day && mob_spawning_enabled {
                    if self.overworld_passive_spawn_timer > 0 {
                        self.overworld_passive_spawn_timer -= 1;
                    } else {
                        self.overworld_passive_spawn_timer = self
                            .tuned_overworld_passive_respawn_base(local_biome)
                            + rng.gen_range(0..36);
                        if self.cows.len()
                            + self.sheep.len()
                            + self.pigs.len()
                            + self.chickens.len()
                            < passive_refill_cap
                        {
                            for _ in 0..4 {
                                let wx = player_wx + rng.gen_range(-70..=70);
                                let Some(spawn_y) = self.find_spawn_surface_for_daylight_mob(wx)
                                else {
                                    continue;
                                };
                                let spawn_x = wx as f64 + 0.5;
                                if self.is_spawn_too_close_to_player(
                                    spawn_x,
                                    spawn_y,
                                    OVERWORLD_SPAWN_MIN_DIST_SQ,
                                ) {
                                    continue;
                                }
                                if self.try_spawn_overworld_passive_mob(wx, spawn_y, rng) {
                                    break;
                                }
                            }
                        }
                    }
                } else {
                    self.overworld_passive_spawn_timer =
                        self.tuned_overworld_passive_respawn_base(local_biome);
                }

                if is_day && mob_spawning_enabled {
                    if self.overworld_wolf_spawn_timer > 0 {
                        self.overworld_wolf_spawn_timer -= 1;
                    } else {
                        self.overworld_wolf_spawn_timer = self
                            .tuned_overworld_wolf_respawn_base(local_biome)
                            + rng.gen_range(0..34);
                        if self.wolves.len() < wolf_refill_cap {
                            for _ in 0..5 {
                                let wx = player_wx + rng.gen_range(-82..=82);
                                let Some(spawn_y) = self.find_spawn_surface_for_daylight_mob(wx)
                                else {
                                    continue;
                                };
                                if self.try_spawn_overworld_wolf(wx, spawn_y, rng) {
                                    break;
                                }
                            }
                        }
                    }
                } else {
                    self.overworld_wolf_spawn_timer =
                        self.tuned_overworld_wolf_respawn_base(local_biome);
                }

                if is_day && mob_spawning_enabled {
                    if self.overworld_ocelot_spawn_timer > 0 {
                        self.overworld_ocelot_spawn_timer -= 1;
                    } else {
                        self.overworld_ocelot_spawn_timer = self
                            .tuned_overworld_ocelot_respawn_base(local_biome)
                            + rng.gen_range(0..44);
                        if self.ocelots.len() < ocelot_refill_cap {
                            for _ in 0..5 {
                                let wx = player_wx + rng.gen_range(-88..=88);
                                let Some(spawn_y) = self.find_spawn_surface_for_daylight_mob(wx)
                                else {
                                    continue;
                                };
                                if self.try_spawn_overworld_ocelot(wx, spawn_y, rng) {
                                    break;
                                }
                            }
                        }
                    }
                } else {
                    self.overworld_ocelot_spawn_timer =
                        self.tuned_overworld_ocelot_respawn_base(local_biome);
                }

                if mob_spawning_enabled {
                    if self.overworld_squid_spawn_timer > 0 {
                        self.overworld_squid_spawn_timer -= 1;
                    } else {
                        self.overworld_squid_spawn_timer = self
                            .tuned_overworld_squid_respawn_base(local_biome)
                            + rng.gen_range(0..28);
                        if self.squids.len() < squid_refill_cap {
                            for _ in 0..5 {
                                let wx = player_wx + rng.gen_range(-96..=96);
                                if self.try_spawn_overworld_squid(wx, rng) {
                                    break;
                                }
                            }
                        }
                    }
                } else {
                    self.overworld_squid_spawn_timer =
                        self.tuned_overworld_squid_respawn_base(local_biome);
                }
            }
            Dimension::Nether => {
                if !hostile_spawning_enabled {
                    self.nether_spawn_timer = self.scaled_hostile_respawn_base(NETHER_RESPAWN_BASE);
                    return;
                }
                if self.nether_spawn_timer > 0 {
                    self.nether_spawn_timer -= 1;
                    return;
                }
                self.nether_spawn_timer =
                    self.scaled_hostile_respawn_base(NETHER_RESPAWN_BASE) + rng.gen_range(0..18);
                let player_in_fortress = self
                    .world
                    .is_nether_fortress_zone(player_wx, self.player.y.floor() as i32);
                let blaze_hot_zone = player_in_fortress
                    && self.nether_blaze_hot_zone_near(player_wx, self.player.y.floor() as i32);
                let roll = rng.gen_range(0..100);
                let pigman_cap = self.scaled_hostile_cap(NETHER_PIGMAN_CAP);
                let ghast_cap = self.scaled_hostile_cap(NETHER_GHAST_CAP);
                let blaze_cap = self.scaled_hostile_cap(NETHER_BLAZE_CAP);
                let (pigman_roll_threshold, ghast_roll_threshold) =
                    Self::nether_ambient_roll_thresholds(player_in_fortress, blaze_hot_zone);
                if roll < pigman_roll_threshold && self.pigmen.len() < pigman_cap {
                    for _ in 0..8 {
                        let spawn_span = if player_in_fortress { 70 } else { 86 };
                        let wx = player_wx + rng.gen_range(-spawn_span..=spawn_span);
                        let Some(spawn_y) = self.find_nether_spawn_surface_for_hostile(wx) else {
                            continue;
                        };
                        let spawn_x = wx as f64 + 0.5;
                        if self.is_spawn_too_close_to_player(
                            spawn_x,
                            spawn_y,
                            NETHER_GROUND_SPAWN_MIN_DIST_SQ,
                        ) {
                            continue;
                        }
                        self.pigmen.push(ZombiePigman::new(spawn_x, spawn_y));
                        break;
                    }
                } else if roll < ghast_roll_threshold
                    && self.ghasts.len() < ghast_cap
                    && self.fireballs.len() < GHAST_FIREBALL_SOFT_CAP
                {
                    for _ in 0..6 {
                        let spawn_span = if player_in_fortress { 72 } else { 90 };
                        let wx = player_wx + rng.gen_range(-spawn_span..=spawn_span);
                        let Some(spawn_y) = self.find_nether_air_spawn_for_ghast(wx) else {
                            continue;
                        };
                        if player_in_fortress
                            && self
                                .world
                                .is_nether_fortress_zone(wx, spawn_y.floor() as i32)
                        {
                            continue;
                        }
                        let spawn_x = wx as f64 + 0.5;
                        if self.is_spawn_too_close_to_player(
                            spawn_x,
                            spawn_y,
                            NETHER_AIR_SPAWN_MIN_DIST_SQ,
                        ) {
                            continue;
                        }
                        self.ghasts.push(Ghast::new(spawn_x, spawn_y));
                        break;
                    }
                } else if self.blazes.len() < blaze_cap
                    && self.fireballs.len() < BLAZE_FIREBALL_SOFT_CAP
                    && (!blaze_hot_zone || self.blazes.len() < NETHER_BLAZE_LOCAL_CLUSTER_LIMIT)
                {
                    let blaze_attempts = if player_in_fortress { 6 } else { 5 };
                    let blaze_span = if player_in_fortress { 54 } else { 82 };
                    for _ in 0..blaze_attempts {
                        let wx = player_wx + rng.gen_range(-blaze_span..=blaze_span);
                        let Some(spawn_y) = self.find_nether_air_spawn_for_blaze(wx) else {
                            continue;
                        };
                        if player_in_fortress
                            && !self
                                .world
                                .is_nether_fortress_zone(wx, spawn_y.floor() as i32)
                        {
                            continue;
                        }
                        let spawn_x = wx as f64 + 0.5;
                        if self.is_spawn_too_close_to_player(
                            spawn_x,
                            spawn_y,
                            NETHER_AIR_SPAWN_MIN_DIST_SQ,
                        ) {
                            continue;
                        }
                        if self.nearby_blaze_count(
                            spawn_x,
                            spawn_y,
                            NETHER_BLAZE_LOCAL_CLUSTER_RADIUS_SQ,
                        ) >= NETHER_BLAZE_LOCAL_CLUSTER_LIMIT
                        {
                            continue;
                        }
                        self.blazes.push(Blaze::new(spawn_x, spawn_y));
                        break;
                    }
                }
            }
            Dimension::End => {
                if !hostile_spawning_enabled {
                    self.end_spawn_timer = self.scaled_hostile_respawn_base(END_RESPAWN_BASE);
                    return;
                }
                if self.end_spawn_timer > 0 {
                    self.end_spawn_timer -= 1;
                    return;
                }
                self.end_spawn_timer =
                    self.scaled_hostile_respawn_base(END_RESPAWN_BASE) + rng.gen_range(0..18);
                let enderman_cap = self.scaled_hostile_cap(self.target_end_enderman_cap());
                if self.endermen.len() >= enderman_cap {
                    return;
                }
                for _ in 0..8 {
                    let wx = player_wx + rng.gen_range(-96..=96);
                    let Some(spawn_y) = self.find_end_spawn_surface_for_enderman(wx) else {
                        continue;
                    };
                    let spawn_x = wx as f64 + 0.5;
                    if self.is_spawn_too_close_to_player(
                        spawn_x,
                        spawn_y,
                        END_ENDERMAN_SPAWN_MIN_DIST_SQ,
                    ) {
                        continue;
                    }
                    self.endermen.push(Enderman::new(spawn_x, spawn_y));
                    break;
                }
            }
        }
    }

    fn ensure_end_boss_entities(&mut self) {
        for (tower_x, delta_y) in self.world.refine_legacy_end_towers() {
            for crystal in &mut self.end_crystals {
                if (crystal.x - (tower_x as f64 + 0.5)).abs() < 0.1 {
                    crystal.y += delta_y;
                }
            }
            if delta_y < 0.0 {
                self.each_player(|state| {
                    // A saved player standing on the former crown rises with
                    // it instead of being embedded by the two added blocks.
                    if (state.player.x - (tower_x as f64 + 0.5)).abs() < 3.0
                        && state.entity_collides_with_world(
                            state.player.x,
                            state.player.y,
                            PLAYER_HALF_WIDTH,
                            PLAYER_HEIGHT,
                        )
                        && !state.entity_collides_with_world(
                            state.player.x,
                            state.player.y + delta_y,
                            PLAYER_HALF_WIDTH,
                            PLAYER_HEIGHT,
                        )
                    {
                        state.player.y += delta_y;
                        state.player.vy = 0.0;
                        state.player.fall_distance = 0.0;
                    }
                });
            }
        }
        if self.current_dimension != Dimension::End
            || self.end_boss_initialized
            || self.dragon_defeated
        {
            return;
        }

        self.end_crystals.clear();
        for tower_x in END_TOWER_XS {
            self.world.load_chunks_around(tower_x);
            let mut crystal_y = None;
            for y in 2..96 {
                if self.world.get_block(tower_x, y) == BlockType::Glowstone
                    && self.world.get_block(tower_x, y - 1) == BlockType::Air
                {
                    crystal_y = Some(y as f64 - 1.0);
                    break;
                }
            }
            if crystal_y.is_none() {
                for y in 2..96 {
                    if self.world.get_block(tower_x, y) == BlockType::Obsidian
                        && !self.world.get_block(tower_x, y - 1).is_solid()
                    {
                        crystal_y = Some(y as f64 - 1.0);
                        break;
                    }
                }
            }
            if crystal_y.is_none() {
                // Robust fallback for legacy/corrupted End chunks: recover a pedestal spawn point.
                let mut top_solid = None;
                for y in (2..96).rev() {
                    if self.world.get_block(tower_x, y).is_solid() {
                        top_solid = Some(y);
                        break;
                    }
                }
                if let Some(y) = top_solid {
                    let spawn_y = y.saturating_sub(1) as f64;
                    crystal_y = Some(spawn_y);
                } else {
                    self.world.set_block(tower_x, 36, BlockType::Obsidian);
                    crystal_y = Some(35.0);
                }
            }
            if let Some(y) = crystal_y {
                self.end_crystals
                    .push(EndCrystal::new(tower_x as f64 + 0.5, y));
            }
        }
        self.ender_dragon = Some(EnderDragon::new(0.5, 24.0));
        self.end_boss_initialized = true;
    }

    fn update_end_boss_encounter(&mut self) {
        if self.current_dimension != Dimension::End {
            return;
        }

        for crystal in &mut self.end_crystals {
            crystal.update_tick();
        }

        let crystal_count = self.end_crystals.len();
        if let Some(dragon) = &self.ender_dragon {
            self.select_nearest_peer(dragon.x, dragon.y);
        }
        let mut contact = None;
        let dragon_contact_damage = self.scaled_hostile_damage(2.5);
        let dragon_contact_cooldown = self.scaled_hostile_cooldown(16);
        if let Some(dragon) = self.ender_dragon.as_mut() {
            if dragon.health > 0.0 {
                dragon.update_flight(&self.world, self.player.x, self.player.y, crystal_count);
            }

            if crystal_count > 0
                && dragon.health > 0.0
                && dragon.health < dragon.max_health
                && dragon.age.is_multiple_of(END_CRYSTAL_HEAL_INTERVAL)
            {
                dragon.health = (dragon.health + END_CRYSTAL_HEAL_AMOUNT).min(dragon.max_health);
            }

            if dragon.attack_cooldown == 0 && dragon.health > 0.0 {
                contact = Some((dragon.hit_regions(), dragon.x));
            }
        }
        if let Some((regions, dragon_x)) = contact {
            let mut hit = false;
            self.each_player(|state| {
                let touches = regions.iter().any(|r| {
                    ((state.player.x - r[0]) / (r[2] + PLAYER_HALF_WIDTH)).powi(2)
                        + ((state.player.y - 0.9 - r[1]) / (r[3] + 0.9)).powi(2)
                        <= 1.0
                });
                if touches && !state.death_screen_active {
                    if state.respawn_grace_ticks == 0 {
                        state.player.health -= dragon_contact_damage;
                    }
                    state.player.vx += if state.player.x > dragon_x { 0.7 } else { -0.7 };
                    state.player.vy = -0.45;
                    state.player.grounded = false;
                    hit = true;
                }
            });
            if hit && let Some(dragon) = &mut self.ender_dragon {
                dragon.attack_cooldown = dragon_contact_cooldown;
            }
        }

        let mut exploded_crystal = false;
        for i in (0..self.end_crystals.len()).rev() {
            if self.end_crystals[i].health <= 0.0 {
                let crystal = self.end_crystals.swap_remove(i);
                self.world.trigger_explosion(
                    crystal.x.floor() as i32,
                    crystal.y.floor() as i32,
                    3,
                    5.0,
                    8,
                );
                exploded_crystal = true;
            }
        }
        if exploded_crystal {
            self.apply_world_explosion_impacts();
            self.collect_world_explosion_drops();
        }

        let dragon_dead = self
            .ender_dragon
            .as_ref()
            .map(|dragon| dragon.health <= 0.0)
            .unwrap_or(false);
        if dragon_dead && let Some(dragon) = self.ender_dragon.take() {
            let dragon_xp = if self.completion_credits_seen {
                500
            } else {
                12_000
            };
            let mut rng = rand::thread_rng();
            self.spawn_experience_orbs(dragon.x, dragon.y - 0.6, dragon_xp, &mut rng);
            self.dragon_defeated = true;
            self.start_end_victory_sequence(dragon.x, dragon.y);
            for x in -1..=0 {
                for y in 32..=33 {
                    if self.world.get_block(x, y) == BlockType::Air {
                        self.world.set_block(x, y, BlockType::EndPortal);
                    }
                }
            }
            for _ in 0..8 {
                self.item_entities.push(ItemEntity::new(
                    dragon.x,
                    dragon.y - 0.6,
                    ItemType::EnderPearl,
                ));
            }
        }
    }

    fn find_lightning_surface(&self, x: i32) -> Option<i32> {
        for y in 2..127 {
            let ground = self.world.get_block(x, y);
            if ground.is_solid() && self.world.get_block(x, y - 1) == BlockType::Air {
                return Some(y);
            }
        }
        None
    }

    fn apply_lightning_strike(&mut self, strike_x: i32, strike_y: i32) {
        let strike_cx = strike_x as f64 + 0.5;
        let strike_cy = strike_y as f64 - 0.2;
        self.lightning_bolts.push(LightningBolt {
            x: strike_x,
            y_top: (strike_y - 12).max(0),
            y_bottom: strike_y,
            ttl: 4,
        });
        self.thunder_flash_timer = self.thunder_flash_timer.max(4);

        let entity_radius = 3.5f64;
        let lightning_damage = 5.0f32;
        let knockback = 0.55f64;

        self.each_player(|s| {
            let player_dist = ((s.player.x - strike_cx).powi(2)
                + ((s.player.y - 0.9) - strike_cy).powi(2))
            .sqrt();
            if player_dist < entity_radius {
                s.apply_player_damage(lightning_damage);
                if !s.has_fire_resistance() {
                    s.player.burning_timer = s.player.burning_timer.max(90);
                }
                let nx = (s.player.x - strike_cx) / player_dist.max(0.001);
                s.player.vx += nx * knockback;
                s.player.vy -= 0.35;
                s.player.grounded = false;
            }
        });

        for zombie in &mut self.zombies {
            let dist =
                ((zombie.x - strike_cx).powi(2) + ((zombie.y - 0.9) - strike_cy).powi(2)).sqrt();
            if dist < entity_radius {
                zombie.health -= lightning_damage;
                zombie.burning_timer = zombie.burning_timer.max(90);
                zombie.hit_timer = zombie.hit_timer.max(10);
                let nx = (zombie.x - strike_cx) / dist.max(0.001);
                zombie.vx += nx * knockback;
                zombie.vy -= 0.3;
                zombie.grounded = false;
            }
        }
        for skeleton in &mut self.skeletons {
            let dist = ((skeleton.x - strike_cx).powi(2)
                + ((skeleton.y - 0.9) - strike_cy).powi(2))
            .sqrt();
            if dist < entity_radius {
                skeleton.health -= lightning_damage;
                skeleton.burning_timer = skeleton.burning_timer.max(90);
                skeleton.hit_timer = skeleton.hit_timer.max(10);
                let nx = (skeleton.x - strike_cx) / dist.max(0.001);
                skeleton.vx += nx * knockback;
                skeleton.vy -= 0.3;
                skeleton.grounded = false;
            }
        }
        for creeper in &mut self.creepers {
            let dist =
                ((creeper.x - strike_cx).powi(2) + ((creeper.y - 0.9) - strike_cy).powi(2)).sqrt();
            if dist < entity_radius {
                creeper.health -= lightning_damage;
                creeper.hit_timer = creeper.hit_timer.max(10);
                creeper.charged = true;
                let nx = (creeper.x - strike_cx) / dist.max(0.001);
                creeper.vx += nx * knockback;
                creeper.vy -= 0.3;
                creeper.grounded = false;
            }
        }
        for spider in &mut self.spiders {
            let dist =
                ((spider.x - strike_cx).powi(2) + ((spider.y - 0.45) - strike_cy).powi(2)).sqrt();
            if dist < entity_radius {
                spider.health -= lightning_damage;
                spider.hit_timer = spider.hit_timer.max(10);
                let nx = (spider.x - strike_cx) / dist.max(0.001);
                spider.vx += nx * knockback;
                spider.vy -= 0.25;
                spider.grounded = false;
            }
        }
        for silverfish in &mut self.silverfish {
            let dist = ((silverfish.x - strike_cx).powi(2)
                + ((silverfish.y - 0.35) - strike_cy).powi(2))
            .sqrt();
            if dist < entity_radius - 0.8 {
                silverfish.health -= lightning_damage;
                silverfish.hit_timer = silverfish.hit_timer.max(10);
                let nx = (silverfish.x - strike_cx) / dist.max(0.001);
                silverfish.vx += nx * knockback * 0.8;
                silverfish.vy -= 0.24;
                silverfish.grounded = false;
            }
        }
        for slime in &mut self.slimes {
            let dist = ((slime.x - strike_cx).powi(2)
                + ((slime.y - slime.height() * 0.5) - strike_cy).powi(2))
            .sqrt();
            if dist < entity_radius - 0.5 {
                slime.health -= lightning_damage;
                slime.hit_timer = slime.hit_timer.max(10);
                let nx = (slime.x - strike_cx) / dist.max(0.001);
                slime.vx += nx * knockback * 0.75;
                slime.vy -= 0.22;
                slime.grounded = false;
            }
        }
        for pigman in &mut self.pigmen {
            let dist =
                ((pigman.x - strike_cx).powi(2) + ((pigman.y - 0.9) - strike_cy).powi(2)).sqrt();
            if dist < entity_radius {
                pigman.health -= lightning_damage;
                pigman.hit_timer = pigman.hit_timer.max(10);
                pigman.provoke();
                let nx = (pigman.x - strike_cx) / dist.max(0.001);
                pigman.vx += nx * knockback;
                pigman.vy -= 0.3;
                pigman.grounded = false;
            }
        }
        for ghast in &mut self.ghasts {
            let dist =
                ((ghast.x - strike_cx).powi(2) + ((ghast.y - 1.0) - strike_cy).powi(2)).sqrt();
            if dist < entity_radius + 1.0 {
                ghast.health -= lightning_damage;
                ghast.hit_timer = ghast.hit_timer.max(10);
                let nx = (ghast.x - strike_cx) / dist.max(0.001);
                ghast.vx += nx * knockback;
                ghast.vy -= 0.22;
            }
        }
        for blaze in &mut self.blazes {
            let dist =
                ((blaze.x - strike_cx).powi(2) + ((blaze.y - 0.9) - strike_cy).powi(2)).sqrt();
            if dist < entity_radius + 0.5 {
                blaze.health -= lightning_damage;
                blaze.hit_timer = blaze.hit_timer.max(10);
                let nx = (blaze.x - strike_cx) / dist.max(0.001);
                blaze.vx += nx * knockback * 0.65;
                blaze.vy -= 0.2;
            }
        }
        for enderman in &mut self.endermen {
            let dist = ((enderman.x - strike_cx).powi(2)
                + ((enderman.y - 1.5) - strike_cy).powi(2))
            .sqrt();
            if dist < entity_radius {
                enderman.health -= lightning_damage;
                enderman.hit_timer = enderman.hit_timer.max(10);
                enderman.provoke();
                let nx = (enderman.x - strike_cx) / dist.max(0.001);
                enderman.vx += nx * knockback;
                enderman.vy -= 0.28;
                enderman.grounded = false;
            }
        }
        for cow in &mut self.cows {
            let dist = ((cow.x - strike_cx).powi(2) + ((cow.y - 0.9) - strike_cy).powi(2)).sqrt();
            if dist < entity_radius {
                cow.health -= lightning_damage;
                cow.hit_timer = cow.hit_timer.max(10);
                let nx = (cow.x - strike_cx) / dist.max(0.001);
                cow.vx += nx * knockback;
                cow.vy -= 0.28;
                cow.grounded = false;
            }
        }
        for sheep in &mut self.sheep {
            let dist =
                ((sheep.x - strike_cx).powi(2) + ((sheep.y - 0.9) - strike_cy).powi(2)).sqrt();
            if dist < entity_radius {
                sheep.health -= lightning_damage;
                sheep.hit_timer = sheep.hit_timer.max(10);
                let nx = (sheep.x - strike_cx) / dist.max(0.001);
                sheep.vx += nx * knockback;
                sheep.vy -= 0.28;
                sheep.grounded = false;
            }
        }
        for pig in &mut self.pigs {
            let dist = ((pig.x - strike_cx).powi(2) + ((pig.y - 0.9) - strike_cy).powi(2)).sqrt();
            if dist < entity_radius {
                pig.health -= lightning_damage;
                pig.hit_timer = pig.hit_timer.max(10);
                let nx = (pig.x - strike_cx) / dist.max(0.001);
                pig.vx += nx * knockback;
                pig.vy -= 0.28;
                pig.grounded = false;
            }
        }
        for chicken in &mut self.chickens {
            let dist =
                ((chicken.x - strike_cx).powi(2) + ((chicken.y - 0.6) - strike_cy).powi(2)).sqrt();
            if dist < entity_radius {
                chicken.health -= lightning_damage;
                chicken.hit_timer = chicken.hit_timer.max(10);
                let nx = (chicken.x - strike_cx) / dist.max(0.001);
                chicken.vx += nx * knockback * 0.8;
                chicken.vy -= 0.22;
                chicken.grounded = false;
            }
        }
        for wolf in &mut self.wolves {
            let dist = ((wolf.x - strike_cx).powi(2) + ((wolf.y - 0.6) - strike_cy).powi(2)).sqrt();
            if dist < entity_radius {
                wolf.health -= lightning_damage;
                wolf.hit_timer = wolf.hit_timer.max(10);
                wolf.provoke();
                let nx = (wolf.x - strike_cx) / dist.max(0.001);
                wolf.vx += nx * knockback * 0.9;
                wolf.vy -= 0.25;
                wolf.grounded = false;
            }
        }
        for ocelot in &mut self.ocelots {
            let dist =
                ((ocelot.x - strike_cx).powi(2) + ((ocelot.y - 0.6) - strike_cy).powi(2)).sqrt();
            if dist < entity_radius {
                ocelot.health -= lightning_damage;
                ocelot.hit_timer = ocelot.hit_timer.max(10);
                let nx = (ocelot.x - strike_cx) / dist.max(0.001);
                ocelot.vx += nx * knockback * 0.9;
                ocelot.vy -= 0.24;
                ocelot.grounded = false;
                ocelot.spook_from(strike_cx);
            }
        }
        for squid in &mut self.squids {
            let dist =
                ((squid.x - strike_cx).powi(2) + ((squid.y - 0.45) - strike_cy).powi(2)).sqrt();
            if dist < entity_radius {
                squid.health -= lightning_damage;
                squid.hit_timer = squid.hit_timer.max(10);
                let nx = (squid.x - strike_cx) / dist.max(0.001);
                squid.vx += nx * knockback * 0.85;
                squid.vy -= 0.24;
                squid.grounded = false;
            }
        }
        for villager in &mut self.villagers {
            let dist = ((villager.x - strike_cx).powi(2)
                + ((villager.y - 0.9) - strike_cy).powi(2))
            .sqrt();
            if dist < entity_radius {
                villager.health -= lightning_damage;
                villager.hit_timer = villager.hit_timer.max(10);
                let nx = (villager.x - strike_cx) / dist.max(0.001);
                villager.vx += nx * knockback;
                villager.vy -= 0.28;
                villager.grounded = false;
            }
        }

        // Environmental hazard scaffold: lightning can instantly ignite nearby TNT.
        for dx in -1..=1 {
            for dy in -1..=1 {
                let wx = strike_x + dx;
                let wy = strike_y + dy;
                if self.world.get_block(wx, wy) == BlockType::Tnt {
                    self.world.set_block(wx, wy, BlockType::PrimedTnt(8));
                }
            }
        }
    }

    pub fn precipitation_at(&self, world_x: i32) -> PrecipitationType {
        if self.current_dimension != Dimension::Overworld {
            return PrecipitationType::None;
        }
        match self.weather {
            WeatherType::Clear => PrecipitationType::None,
            WeatherType::Rain | WeatherType::Thunderstorm => match self.world.get_biome(world_x) {
                BiomeType::Desert => PrecipitationType::None,
                BiomeType::Tundra | BiomeType::Taiga => PrecipitationType::Snow,
                BiomeType::Forest
                | BiomeType::Plains
                | BiomeType::Swamp
                | BiomeType::Jungle
                | BiomeType::ExtremeHills
                | BiomeType::Ocean
                | BiomeType::River => PrecipitationType::Rain,
            },
        }
    }

    fn is_weather_wet_at(&self, x: f64, top_y: f64) -> bool {
        let bx = x.floor() as i32;
        self.precipitation_at(bx) != PrecipitationType::None && self.is_exposed_to_sky(x, top_y)
    }

    pub fn weather_audio_mix(&self) -> (f32, f32, f32) {
        (
            self.weather_rain_intensity,
            self.weather_wind_intensity,
            self.weather_thunder_intensity,
        )
    }

    fn reset_weather_for_dimension(&mut self) {
        if self.current_dimension != Dimension::Overworld {
            self.weather = WeatherType::Clear;
            self.thunder_flash_timer = 0;
            self.weather_rain_intensity = 0.0;
            self.weather_wind_intensity = 0.08;
            self.weather_thunder_intensity = 0.0;
        } else if self.weather == WeatherType::Clear {
            self.weather_timer = self.weather_timer.max(1800);
        }
    }

    fn pick_next_weather(rng: &mut impl Rng, current: WeatherType) -> (WeatherType, u32) {
        let next = match current {
            WeatherType::Clear => {
                let roll = rng.gen_range(0..100);
                if roll < 74 {
                    WeatherType::Clear
                } else if roll < 96 {
                    WeatherType::Rain
                } else {
                    WeatherType::Thunderstorm
                }
            }
            WeatherType::Rain => {
                let roll = rng.gen_range(0..100);
                if roll < 48 {
                    WeatherType::Clear
                } else if roll < 90 {
                    WeatherType::Rain
                } else {
                    WeatherType::Thunderstorm
                }
            }
            WeatherType::Thunderstorm => {
                let roll = rng.gen_range(0..100);
                if roll < 30 {
                    WeatherType::Clear
                } else if roll < 88 {
                    WeatherType::Rain
                } else {
                    WeatherType::Thunderstorm
                }
            }
        };

        let duration = match next {
            WeatherType::Clear => rng.gen_range(4200..9600),
            WeatherType::Rain => rng.gen_range(2400..5600),
            WeatherType::Thunderstorm => rng.gen_range(1000..2600),
        };
        (next, duration)
    }

    pub(crate) fn director_weather(&mut self, weather: WeatherType) {
        self.weather = weather;
        self.weather_timer = 2400;
        self.thunder_flash_timer = 0;
    }

    fn update_weather(&mut self, rng: &mut impl Rng) {
        if self.current_dimension != Dimension::Overworld {
            self.weather = WeatherType::Clear;
            self.thunder_flash_timer = 0;
            self.weather_timer = 1800;
            self.weather_rain_intensity = 0.0;
            self.weather_wind_intensity = 0.08;
            self.weather_thunder_intensity = 0.0;
            return;
        }

        if self.active_game_rules().do_weather_cycle {
            if self.weather_timer > 0 {
                self.weather_timer -= 1;
            } else {
                let (next, duration) = Self::pick_next_weather(rng, self.weather);
                self.weather = next;
                self.weather_timer = duration;
                self.thunder_flash_timer = 0;
            }
        } else {
            self.weather_timer = self.weather_timer.max(1);
        }

        if self.weather == WeatherType::Thunderstorm {
            if self.thunder_flash_timer > 0 {
                self.thunder_flash_timer -= 1;
            } else if rng.gen_bool(0.01) {
                self.thunder_flash_timer = rng.gen_range(2..=4);
            }
        } else {
            self.thunder_flash_timer = 0;
        }

        let (rain_target, wind_target, thunder_target) = match self.weather {
            WeatherType::Clear => (0.0, 0.12, 0.0),
            WeatherType::Rain => (0.72, 0.38, 0.0),
            WeatherType::Thunderstorm => {
                let thunder_pulse = if self.thunder_flash_timer > 0 {
                    1.0
                } else {
                    0.45
                };
                (0.96, 0.62, thunder_pulse)
            }
        };
        self.weather_rain_intensity += (rain_target - self.weather_rain_intensity) * 0.08;
        self.weather_wind_intensity += (wind_target - self.weather_wind_intensity) * 0.06;
        self.weather_thunder_intensity += (thunder_target - self.weather_thunder_intensity) * 0.14;
    }

    fn trim_far_nether_mobs(&mut self) {
        let positions: Vec<_> = std::iter::once((self.player.x, self.player.y))
            .chain(
                self.other_players_here()
                    .map(|p| (p.player().x, p.player().y)),
            )
            .collect();
        let distance = |x: f64, y: f64| {
            positions
                .iter()
                .map(|&(px, py)| (x - px).powi(2) + (y - py).powi(2))
                .fold(f64::INFINITY, f64::min)
        };
        self.pigmen
            .retain(|p| distance(p.x, p.y) <= NETHER_DESPAWN_DIST_SQ);
        self.ghasts
            .retain(|p| distance(p.x, p.y) <= NETHER_DESPAWN_DIST_SQ * 1.4);
        self.blazes
            .retain(|p| distance(p.x, p.y) <= NETHER_DESPAWN_DIST_SQ * 1.2);
    }

    fn trim_far_overworld_mobs(&mut self) {
        let positions: Vec<_> = std::iter::once((self.player.x, self.player.y))
            .chain(
                self.other_players_here()
                    .map(|a| (a.player().x, a.player().y)),
            )
            .collect();
        self.zombies.retain(|z| {
            let dist_sq = positions
                .iter()
                .map(|(px, py)| (z.x - px).powi(2) + (z.y - py).powi(2))
                .fold(f64::INFINITY, f64::min);
            dist_sq <= OVERWORLD_HOSTILE_DESPAWN_DIST_SQ
        });
        self.creepers.retain(|c| {
            let dist_sq = positions
                .iter()
                .map(|(px, py)| (c.x - px).powi(2) + (c.y - py).powi(2))
                .fold(f64::INFINITY, f64::min);
            dist_sq <= OVERWORLD_HOSTILE_DESPAWN_DIST_SQ
        });
        self.skeletons.retain(|s| {
            let dist_sq = positions
                .iter()
                .map(|(px, py)| (s.x - px).powi(2) + (s.y - py).powi(2))
                .fold(f64::INFINITY, f64::min);
            dist_sq <= OVERWORLD_HOSTILE_DESPAWN_DIST_SQ
        });
        self.spiders.retain(|s| {
            let dist_sq = positions
                .iter()
                .map(|(px, py)| (s.x - px).powi(2) + (s.y - py).powi(2))
                .fold(f64::INFINITY, f64::min);
            dist_sq <= OVERWORLD_HOSTILE_DESPAWN_DIST_SQ
        });
        self.silverfish.retain(|s| {
            let dist_sq = positions
                .iter()
                .map(|(px, py)| (s.x - px).powi(2) + (s.y - py).powi(2))
                .fold(f64::INFINITY, f64::min);
            dist_sq <= OVERWORLD_HOSTILE_DESPAWN_DIST_SQ
        });
        self.slimes.retain(|s| {
            let dist_sq = positions
                .iter()
                .map(|(px, py)| (s.x - px).powi(2) + (s.y - py).powi(2))
                .fold(f64::INFINITY, f64::min);
            dist_sq <= OVERWORLD_HOSTILE_DESPAWN_DIST_SQ
        });
        self.endermen.retain(|e| {
            let dist_sq = positions
                .iter()
                .map(|(px, py)| (e.x - px).powi(2) + (e.y - py).powi(2))
                .fold(f64::INFINITY, f64::min);
            dist_sq <= OVERWORLD_HOSTILE_DESPAWN_DIST_SQ
        });
        self.cows.retain(|c| {
            let dist_sq = positions
                .iter()
                .map(|(px, py)| (c.x - px).powi(2) + (c.y - py).powi(2))
                .fold(f64::INFINITY, f64::min);
            dist_sq <= OVERWORLD_PASSIVE_DESPAWN_DIST_SQ || c.husbandry.persistent
        });
        self.sheep.retain(|s| {
            let dist_sq = positions
                .iter()
                .map(|(px, py)| (s.x - px).powi(2) + (s.y - py).powi(2))
                .fold(f64::INFINITY, f64::min);
            dist_sq <= OVERWORLD_PASSIVE_DESPAWN_DIST_SQ || s.husbandry.persistent
        });
        self.pigs.retain(|p| {
            let dist_sq = positions
                .iter()
                .map(|(px, py)| (p.x - px).powi(2) + (p.y - py).powi(2))
                .fold(f64::INFINITY, f64::min);
            dist_sq <= OVERWORLD_PASSIVE_DESPAWN_DIST_SQ || p.husbandry.persistent
        });
        self.chickens.retain(|c| {
            let dist_sq = positions
                .iter()
                .map(|(px, py)| (c.x - px).powi(2) + (c.y - py).powi(2))
                .fold(f64::INFINITY, f64::min);
            dist_sq <= OVERWORLD_PASSIVE_DESPAWN_DIST_SQ || c.husbandry.persistent
        });
        self.wolves.retain(|w| {
            let dist_sq = positions
                .iter()
                .map(|(px, py)| (w.x - px).powi(2) + (w.y - py).powi(2))
                .fold(f64::INFINITY, f64::min);
            dist_sq <= OVERWORLD_PASSIVE_DESPAWN_DIST_SQ
        });
        self.ocelots.retain(|o| {
            let dist_sq = positions
                .iter()
                .map(|(px, py)| (o.x - px).powi(2) + (o.y - py).powi(2))
                .fold(f64::INFINITY, f64::min);
            dist_sq <= OVERWORLD_PASSIVE_DESPAWN_DIST_SQ
        });
        self.squids.retain(|s| {
            let dist_sq = positions
                .iter()
                .map(|(px, py)| (s.x - px).powi(2) + (s.y - py).powi(2))
                .fold(f64::INFINITY, f64::min);
            dist_sq <= OVERWORLD_AQUATIC_DESPAWN_DIST_SQ
        });
    }

    fn trim_far_end_mobs(&mut self) {
        let px = self.player.x;
        let py = self.player.y;
        self.endermen.retain(|e| {
            let dist_sq = (e.x - px).powi(2) + (e.y - py).powi(2);
            dist_sq <= END_DESPAWN_DIST_SQ
        });
    }

    fn trim_excess_loose_entities(&mut self) {
        let px = self.player.x;
        let py = self.player.y - 1.0;

        if self.item_entities.len() > ITEM_ENTITY_HARD_CAP {
            self.item_entities.sort_by(|a, b| {
                let a_dist = (a.x - px).powi(2) + (a.y - py).powi(2);
                let b_dist = (b.x - px).powi(2) + (b.y - py).powi(2);
                let a_score = a_dist + a.age as f64 * 0.025;
                let b_score = b_dist + b.age as f64 * 0.025;
                a_score.partial_cmp(&b_score).unwrap_or(Ordering::Equal)
            });
            self.item_entities.truncate(ITEM_ENTITY_HARD_CAP);
        }

        if self.experience_orbs.len() > EXPERIENCE_ORB_HARD_CAP {
            self.experience_orbs.sort_by(|a, b| {
                let a_dist = (a.x - px).powi(2) + (a.y - py).powi(2);
                let b_dist = (b.x - px).powi(2) + (b.y - py).powi(2);
                let a_score = a_dist + a.age as f64 * 0.012;
                let b_score = b_dist + b.age as f64 * 0.012;
                a_score.partial_cmp(&b_score).unwrap_or(Ordering::Equal)
            });
            self.experience_orbs.truncate(EXPERIENCE_ORB_HARD_CAP);
        }
    }

    fn ensure_nether_portal_at(&mut self, x: i32, base_y: i32) {
        let base_y = base_y.clamp(6, 126);
        let left = x - 1;
        let right = x + 2;
        let top = base_y - 4;
        let bottom = base_y;

        for wx in left..=right {
            for wy in top..=bottom {
                let is_frame = wx == left || wx == right || wy == top || wy == bottom;
                if is_frame {
                    self.world.set_block(wx, wy, BlockType::Obsidian);
                } else {
                    self.world.set_block(wx, wy, BlockType::NetherPortal);
                }
            }
        }
    }

    fn transfer_nether_dimension(&mut self) {
        let source_portal_anchor = self.player_nether_portal_anchor();
        self.transfer_nether_dimension_from_anchor(source_portal_anchor);
    }

    fn transfer_nether_dimension_from_anchor(&mut self, source_portal_anchor: Option<(i32, i32)>) {
        let source_dimension = self.current_dimension;
        let resolved_source_anchor = source_portal_anchor
            .or_else(|| self.player_nether_portal_anchor())
            .or_else(|| self.nearby_player_nether_portal_anchor(QUICK_TRAVEL_PORTAL_ANCHOR_RADIUS));
        let source_x = resolved_source_anchor
            .map(|(inner_x, _)| inner_x)
            .unwrap_or_else(|| self.player.x.round() as i32);
        let target_dimension = match self.current_dimension {
            Dimension::Overworld | Dimension::End => Dimension::Nether,
            Dimension::Nether => Dimension::Overworld,
        };
        let target_x = match (self.current_dimension, target_dimension) {
            (Dimension::Overworld, Dimension::Nether) | (Dimension::End, Dimension::Nether) => {
                source_x.div_euclid(8)
            }
            (Dimension::Nether, Dimension::Overworld) => source_x * 8,
            _ => source_x,
        };

        self.clear_container_interaction_state();
        if !self.save_before_dimension_change() {
            return;
        }
        self.travel_dimension(target_dimension);
        self.world.load_chunks_around(target_x);

        // A saved link names the player's chosen frame, including cave homes and
        // elevated builds. Surface heuristics apply only to automatic discovery.
        let linked_portal = resolved_source_anchor
            .and_then(|anchor| self.linked_nether_portal_anchor(source_dimension, anchor));
        let existing_portal = linked_portal.or_else(|| {
            self.find_existing_nether_portal_near(target_x, PORTAL_SEARCH_RADIUS)
                .filter(|&(inner_x, base_y)| {
                    target_dimension != Dimension::Overworld
                        || !self.overworld_portal_anchor_is_elevated(inner_x, base_y)
                })
        });
        let (portal_x, base_y) = existing_portal.unwrap_or_else(|| {
            if target_dimension == Dimension::Overworld {
                self.find_overworld_portal_arrival_site(target_x, 20)
            } else {
                let base_y = self.find_walkable_surface(target_x);
                (target_x, base_y)
            }
        });
        let (arrival_x, arrival_y) = if existing_portal.is_some() {
            self.existing_portal_arrival(portal_x, base_y)
        } else {
            self.build_portal_arrival_vestibule(portal_x, base_y)
        };
        if let Some(source_anchor) = resolved_source_anchor {
            self.remember_nether_portal_link(source_dimension, source_anchor, (portal_x, base_y));
        }

        self.player.x = arrival_x;
        self.player.y = arrival_y;
        self.player.vx = 0.0;
        self.player.vy = 0.0;
        self.player.grounded = false;
        self.player.fall_distance = 0.0;
        self.jump_held = false;
        self.jump_buffer_ticks = 0;
        self.stop_sprinting();

        self.eye_guidance_timer = 0;
        self.save_progression();
    }

    fn quick_travel_to_dimension(&mut self, target_dimension: Dimension) {
        if self.current_dimension == target_dimension
            || self.death_screen_active
            || self.credits_active
        {
            return;
        }

        self.jump_held = false;
        self.jump_buffer_ticks = 0;
        self.stop_sprinting();

        match target_dimension {
            Dimension::Overworld => match self.current_dimension {
                Dimension::Overworld => return,
                Dimension::Nether => self.transfer_nether_dimension(),
                Dimension::End => self.transfer_overworld_end_dimension(Dimension::Overworld),
            },
            Dimension::Nether => self.transfer_nether_dimension(),
            Dimension::End => self.transfer_overworld_end_dimension(Dimension::End),
        }

        self.portal_timer = 0;
        self.portal_cooldown = 40;
    }

    fn quick_travel_to_spawn(&mut self) {
        if self.death_screen_active || self.credits_active {
            return;
        }

        self.jump_held = false;
        self.jump_buffer_ticks = 0;
        self.stop_sprinting();
        self.clear_container_interaction_state();

        let switched_dimension = self.current_dimension != Dimension::Overworld;
        if switched_dimension {
            if !self.save_before_dimension_change() {
                return;
            }
            self.travel_dimension(Dimension::Overworld);
        }

        let (spawn_x, spawn_y) = self.resolve_overworld_spawn_location();

        if switched_dimension {
            self.eye_guidance_timer = 0;
        }

        self.player.x = spawn_x as f64 + 0.5;
        self.player.y = spawn_y;
        self.player.vx = 0.0;
        self.player.vy = 0.0;
        self.player.grounded = false;
        self.player.fall_distance = 0.0;
        self.portal_timer = 0;
        self.portal_cooldown = 40;
        self.save_progression();
    }

    fn equip_diamond_loadout(&mut self) {
        if self.death_screen_active || self.credits_active {
            return;
        }

        self.jump_held = false;
        self.jump_buffer_ticks = 0;
        self.stop_sprinting();
        self.clear_container_interaction_state();

        for slot_idx in 0..ARMOR_SLOT_COUNT {
            let replacement = Self::fresh_item_stack(
                match slot_idx {
                    0 => ItemType::DiamondHelmet,
                    1 => ItemType::DiamondChestplate,
                    2 => ItemType::DiamondLeggings,
                    _ => ItemType::DiamondBoots,
                },
                1,
            );
            self.replace_armor_slot_with_stack(slot_idx, replacement);
        }

        let hotbar_loadout = [
            (0usize, ItemType::DiamondSword, 1u32),
            (1usize, ItemType::Bow, 1u32),
            (2usize, ItemType::Arrow, 64u32),
            (3usize, ItemType::CookedBeef, 32u32),
            (4usize, ItemType::Cobblestone, 64u32),
            (5usize, ItemType::DiamondPickaxe, 1u32),
            (6usize, ItemType::EnderPearl, 16u32),
        ];
        for (slot_idx, item_type, count) in hotbar_loadout {
            self.replace_inventory_slot_with_stack(
                slot_idx,
                Self::fresh_item_stack(item_type, count),
            );
        }

        self.hotbar_index = 0;
        self.save_progression();
    }

    fn replace_armor_slot_with_stack(&mut self, slot_idx: usize, stack: ItemStack) {
        if let Some(existing) = self.armor_slots[slot_idx].take() {
            let enchant_level = self.armor_enchant_levels[slot_idx];
            self.stow_inventory_stack_or_drop(existing, enchant_level, None);
        }
        self.armor_slots[slot_idx] = Some(stack);
        self.armor_enchant_levels[slot_idx] = 0;
    }

    fn replace_inventory_slot_with_stack(&mut self, slot_idx: usize, stack: ItemStack) {
        if let Some(existing) = self.inventory.slots[slot_idx].take() {
            let enchant_level = self.inventory_enchant_levels[slot_idx];
            self.stow_inventory_stack_or_drop(existing, enchant_level, Some(slot_idx));
        }
        self.inventory.slots[slot_idx] = Some(stack);
        self.inventory_enchant_levels[slot_idx] = 0;
    }

    fn stow_inventory_stack_or_drop(
        &mut self,
        mut stack: ItemStack,
        enchant_level: u8,
        reserved_slot: Option<usize>,
    ) {
        let stack_limit = if stack.item_type.max_durability().is_some() || enchant_level > 0 {
            1
        } else {
            stack.item_type.max_stack_size()
        };

        if stack_limit > 1 {
            for (slot_idx, slot) in self.inventory.slots.iter_mut().enumerate() {
                let Some(existing) = slot.as_mut() else {
                    continue;
                };
                if existing.item_type != stack.item_type
                    || existing.durability != stack.durability
                    || self.inventory_enchant_levels[slot_idx] != enchant_level
                {
                    continue;
                }
                let add = stack_limit.saturating_sub(existing.count).min(stack.count);
                existing.count += add;
                stack.count -= add;
                if stack.count == 0 {
                    return;
                }
            }
        }

        while stack.count > 0 {
            let Some(empty_slot_idx) =
                self.inventory
                    .slots
                    .iter()
                    .enumerate()
                    .find_map(|(idx, slot)| {
                        (slot.is_none() && reserved_slot != Some(idx)).then_some(idx)
                    })
            else {
                break;
            };
            let add = stack.count.min(stack_limit);
            self.inventory.slots[empty_slot_idx] = Some(ItemStack {
                item_type: stack.item_type,
                count: add,
                durability: stack.durability,
            });
            self.inventory_enchant_levels[empty_slot_idx] = enchant_level;
            stack.count -= add;
        }

        for _ in 0..stack.count {
            self.spill_single_item_near_player(stack.item_type);
        }
    }

    fn fresh_item_stack(item_type: ItemType, count: u32) -> ItemStack {
        ItemStack {
            item_type,
            count,
            durability: item_type.max_durability(),
        }
    }

    fn transfer_overworld_end_dimension(&mut self, target_dimension: Dimension) {
        let source_dimension = self.current_dimension;
        let target_x = match target_dimension {
            Dimension::End => 0,
            Dimension::Overworld => STRONGHOLD_CENTER_X,
            Dimension::Nether => return,
        };
        self.clear_container_interaction_state();
        if !self.save_before_dimension_change() {
            return;
        }
        self.travel_dimension(target_dimension);
        self.world.load_chunks_around(target_x);

        let (arrival_x, arrival_y) = if target_dimension == Dimension::End {
            let (best_x, best_y) = self.find_end_arrival_site();
            if Self::is_safe_spawn_column_base(&self.world, best_x, best_y, false) {
                (best_x as f64 + 0.5, best_y as f64 - 0.1)
            } else {
                self.prepare_end_arrival_pad(best_x, best_y)
            }
        } else {
            let expected_floor = STRONGHOLD_PORTAL_INNER_Y + 5;
            let site =
                if Self::is_safe_spawn_column_base(&self.world, target_x, expected_floor, false) {
                    Some((target_x, expected_floor))
                } else {
                    self.world.load_chunks_for_spawn_search(target_x, 24);
                    Self::find_nearest_safe_spawn(&self.world, target_x, 24, false)
                };
            let Some((x, floor)) = site else {
                self.travel_dimension(source_dimension);
                self.save_error.replace(Some(
                    "End return cancelled: no safe landing near the stronghold.".into(),
                ));
                return;
            };
            (x as f64 + 0.5, floor as f64 - 0.1)
        };
        self.player.x = arrival_x;
        self.player.y = arrival_y;
        self.player.vx = 0.0;
        self.player.vy = 0.0;
        self.player.grounded = false;
        self.player.fall_distance = 0.0;
        self.jump_held = false;
        self.jump_buffer_ticks = 0;
        self.stop_sprinting();
        if target_dimension == Dimension::End {
            self.respawn_grace_ticks = self.respawn_grace_ticks.max(24);
        }

        self.eye_guidance_timer = 0;
        self.save_progression();
    }

    fn transfer_end_dimension(&mut self) {
        match self.current_dimension {
            Dimension::Overworld => self.transfer_overworld_end_dimension(Dimension::End),
            Dimension::End => {
                if self.dragon_defeated && !self.completion_credits_seen {
                    self.start_completion_credits();
                } else {
                    self.transfer_overworld_end_dimension(Dimension::Overworld);
                }
            }
            Dimension::Nether => {}
        }
    }

    fn handle_portal_transition(&mut self) -> bool {
        let touching = self.player_portal_contact();
        if self.portal_cooldown > 0 {
            self.portal_cooldown -= 1;
            // An arrival latch prevents automatic bouncing. Explicit use is
            // available after the normal cooldown even if the actor stays here.
            self.portal_timer = if touching.is_some() { u16::MAX } else { 0 };
            return false;
        }
        if self.portal_timer == u16::MAX {
            if touching.is_none() {
                self.portal_timer = 0;
            }
            return false;
        }
        if self.inventory_open || self.chat.open || self.death_screen_active || self.credits_active
        {
            self.portal_timer = 0;
            return false;
        }
        let target = match touching {
            Some((PortalKind::Nether, Some(anchor))) => PortalUseTarget::Nether(anchor),
            Some((PortalKind::End, _)) => PortalUseTarget::End,
            _ => {
                self.portal_timer = 0;
                return false;
            }
        };
        self.portal_timer = self.portal_timer.saturating_add(1);
        // Walking/running past a Nether portal is ordinary movement. Automatic
        // travel needs two continuous seconds inside; explicit use stays quick.
        let dwell = match target {
            PortalUseTarget::Nether(_) => NETHER_PORTAL_DWELL_TICKS,
            PortalUseTarget::End => 4,
        };
        if self.portal_timer < dwell {
            return false;
        }
        let previous = self.current_dimension;
        self.use_portal_target(target);
        let travelled = self.current_dimension != previous || self.credits_active;
        if travelled {
            self.apply_client_command(ClientCommand::ClearDirectionalInput);
            self.jump_held = false;
            self.left_click_down = false;
            self.player.vx = 0.0;
            self.player.vy = 0.0;
        }
        travelled
    }

    fn explosion_effect_at(
        x: f64,
        y: f64,
        center_x: f64,
        center_y: f64,
        radius: f64,
    ) -> Option<(f32, f64, f64)> {
        let dx = x - center_x;
        let dy = y - center_y;
        let dist = (dx * dx + dy * dy).sqrt();
        if dist >= radius {
            return None;
        }

        let falloff = ((radius - dist) / radius).clamp(0.0, 1.0);
        let damage = TNT_ENTITY_DAMAGE_BASE * falloff as f32;
        let (nx, ny) = if dist > 0.001 {
            (dx / dist, dy / dist)
        } else {
            (0.0, -1.0)
        };
        let knockback_x = nx * TNT_ENTITY_KNOCKBACK_SCALE * falloff;
        let knockback_y = ny * TNT_ENTITY_KNOCKBACK_SCALE * falloff - (0.25 * falloff);
        Some((damage, knockback_x, knockback_y))
    }

    fn apply_world_explosion_impacts(&mut self) {
        let explosions = std::mem::take(&mut self.world.recent_explosions);
        for (cx, cy, radius_blocks) in explosions {
            let center_x = cx as f64 + 0.5;
            let center_y = cy as f64 + 0.5;
            let radius = radius_blocks as f64 + 0.5;

            self.each_player(|s| {
                if let Some((damage, kb_x, kb_y)) = Self::explosion_effect_at(
                    s.player.x,
                    s.player.y - 0.9,
                    center_x,
                    center_y,
                    radius,
                ) {
                    s.apply_player_damage(damage);
                    s.player.vx += kb_x;
                    s.player.vy += kb_y;
                    s.player.grounded = false;
                }
            });
            if let Some(dragon) = self.ender_dragon.as_mut()
                && let Some((damage, kb_x, kb_y)) =
                    Self::explosion_effect_at(dragon.x, dragon.y - 1.4, center_x, center_y, radius)
            {
                dragon.health -= damage * 0.45;
                dragon.vx += kb_x * 0.35;
                dragon.vy += kb_y * 0.35;
                dragon.hit_timer = dragon.hit_timer.max(6);
            }
            for crystal in &mut self.end_crystals {
                if let Some((damage, _, _)) =
                    Self::explosion_effect_at(crystal.x, crystal.y, center_x, center_y, radius)
                {
                    crystal.health -= damage;
                    crystal.hit_timer = crystal.hit_timer.max(6);
                }
            }

            for zombie in &mut self.zombies {
                if let Some((damage, kb_x, kb_y)) =
                    Self::explosion_effect_at(zombie.x, zombie.y - 0.9, center_x, center_y, radius)
                {
                    zombie.health -= damage;
                    zombie.vx += kb_x;
                    zombie.vy += kb_y;
                    zombie.grounded = false;
                    zombie.hit_timer = zombie.hit_timer.max(6);
                }
            }
            for pigman in &mut self.pigmen {
                if let Some((damage, kb_x, kb_y)) =
                    Self::explosion_effect_at(pigman.x, pigman.y - 0.9, center_x, center_y, radius)
                {
                    pigman.health -= damage;
                    pigman.vx += kb_x;
                    pigman.vy += kb_y;
                    pigman.grounded = false;
                    pigman.hit_timer = pigman.hit_timer.max(6);
                    pigman.provoke();
                }
            }
            for ghast in &mut self.ghasts {
                if let Some((damage, kb_x, kb_y)) =
                    Self::explosion_effect_at(ghast.x, ghast.y - 0.9, center_x, center_y, radius)
                {
                    ghast.health -= damage;
                    ghast.vx += kb_x * 0.6;
                    ghast.vy += kb_y * 0.6;
                    ghast.hit_timer = ghast.hit_timer.max(6);
                }
            }
            for blaze in &mut self.blazes {
                if let Some((damage, kb_x, kb_y)) =
                    Self::explosion_effect_at(blaze.x, blaze.y - 0.9, center_x, center_y, radius)
                {
                    blaze.health -= damage;
                    blaze.vx += kb_x * 0.65;
                    blaze.vy += kb_y * 0.65;
                    blaze.hit_timer = blaze.hit_timer.max(6);
                }
            }
            for enderman in &mut self.endermen {
                if let Some((damage, kb_x, kb_y)) = Self::explosion_effect_at(
                    enderman.x,
                    enderman.y - 1.5,
                    center_x,
                    center_y,
                    radius,
                ) {
                    enderman.health -= damage;
                    enderman.vx += kb_x;
                    enderman.vy += kb_y;
                    enderman.grounded = false;
                    enderman.hit_timer = enderman.hit_timer.max(6);
                    enderman.provoke();
                }
            }
            for creeper in &mut self.creepers {
                if let Some((damage, kb_x, kb_y)) = Self::explosion_effect_at(
                    creeper.x,
                    creeper.y - 0.9,
                    center_x,
                    center_y,
                    radius,
                ) {
                    creeper.health -= damage;
                    creeper.vx += kb_x;
                    creeper.vy += kb_y;
                    creeper.grounded = false;
                    creeper.hit_timer = creeper.hit_timer.max(6);
                }
            }
            for skeleton in &mut self.skeletons {
                if let Some((damage, kb_x, kb_y)) = Self::explosion_effect_at(
                    skeleton.x,
                    skeleton.y - 0.9,
                    center_x,
                    center_y,
                    radius,
                ) {
                    skeleton.health -= damage;
                    skeleton.vx += kb_x;
                    skeleton.vy += kb_y;
                    skeleton.grounded = false;
                    skeleton.hit_timer = skeleton.hit_timer.max(6);
                }
            }
            for spider in &mut self.spiders {
                if let Some((damage, kb_x, kb_y)) =
                    Self::explosion_effect_at(spider.x, spider.y - 0.45, center_x, center_y, radius)
                {
                    spider.health -= damage;
                    spider.vx += kb_x;
                    spider.vy += kb_y;
                    spider.grounded = false;
                    spider.hit_timer = spider.hit_timer.max(6);
                }
            }
            for silverfish in &mut self.silverfish {
                if let Some((damage, kb_x, kb_y)) = Self::explosion_effect_at(
                    silverfish.x,
                    silverfish.y - 0.35,
                    center_x,
                    center_y,
                    radius,
                ) {
                    silverfish.health -= damage;
                    silverfish.vx += kb_x;
                    silverfish.vy += kb_y;
                    silverfish.grounded = false;
                    silverfish.hit_timer = silverfish.hit_timer.max(6);
                }
            }
            for slime in &mut self.slimes {
                if let Some((damage, kb_x, kb_y)) = Self::explosion_effect_at(
                    slime.x,
                    slime.y - slime.height() * 0.5,
                    center_x,
                    center_y,
                    radius,
                ) {
                    slime.health -= damage;
                    slime.vx += kb_x * 0.8;
                    slime.vy += kb_y * 0.8;
                    slime.grounded = false;
                    slime.hit_timer = slime.hit_timer.max(6);
                }
            }
            for cow in &mut self.cows {
                if let Some((damage, kb_x, kb_y)) =
                    Self::explosion_effect_at(cow.x, cow.y - 0.9, center_x, center_y, radius)
                {
                    cow.health -= damage;
                    cow.vx += kb_x;
                    cow.vy += kb_y;
                    cow.grounded = false;
                    cow.hit_timer = cow.hit_timer.max(6);
                }
            }
            for sheep in &mut self.sheep {
                if let Some((damage, kb_x, kb_y)) =
                    Self::explosion_effect_at(sheep.x, sheep.y - 0.9, center_x, center_y, radius)
                {
                    sheep.health -= damage;
                    sheep.vx += kb_x;
                    sheep.vy += kb_y;
                    sheep.grounded = false;
                    sheep.hit_timer = sheep.hit_timer.max(6);
                }
            }
            for pig in &mut self.pigs {
                if let Some((damage, kb_x, kb_y)) =
                    Self::explosion_effect_at(pig.x, pig.y - 0.9, center_x, center_y, radius)
                {
                    pig.health -= damage;
                    pig.vx += kb_x;
                    pig.vy += kb_y;
                    pig.grounded = false;
                    pig.hit_timer = pig.hit_timer.max(6);
                }
            }
            for chicken in &mut self.chickens {
                if let Some((damage, kb_x, kb_y)) = Self::explosion_effect_at(
                    chicken.x,
                    chicken.y - 0.6,
                    center_x,
                    center_y,
                    radius,
                ) {
                    chicken.health -= damage;
                    chicken.vx += kb_x * 0.85;
                    chicken.vy += kb_y * 0.85;
                    chicken.grounded = false;
                    chicken.hit_timer = chicken.hit_timer.max(6);
                }
            }
            for wolf in &mut self.wolves {
                if let Some((damage, kb_x, kb_y)) =
                    Self::explosion_effect_at(wolf.x, wolf.y - 0.6, center_x, center_y, radius)
                {
                    wolf.health -= damage;
                    wolf.vx += kb_x * 0.9;
                    wolf.vy += kb_y * 0.85;
                    wolf.grounded = false;
                    wolf.hit_timer = wolf.hit_timer.max(6);
                    wolf.provoke();
                }
            }
            for ocelot in &mut self.ocelots {
                if let Some((damage, kb_x, kb_y)) =
                    Self::explosion_effect_at(ocelot.x, ocelot.y - 0.6, center_x, center_y, radius)
                {
                    ocelot.health -= damage;
                    ocelot.vx += kb_x * 0.9;
                    ocelot.vy += kb_y * 0.85;
                    ocelot.grounded = false;
                    ocelot.hit_timer = ocelot.hit_timer.max(6);
                    ocelot.spook_from(center_x);
                }
            }
            for squid in &mut self.squids {
                if let Some((damage, kb_x, kb_y)) =
                    Self::explosion_effect_at(squid.x, squid.y - 0.45, center_x, center_y, radius)
                {
                    squid.health -= damage;
                    squid.vx += kb_x * 0.85;
                    squid.vy += kb_y * 0.85;
                    squid.grounded = false;
                    squid.hit_timer = squid.hit_timer.max(6);
                }
            }
            for villager in &mut self.villagers {
                if let Some((damage, kb_x, kb_y)) = Self::explosion_effect_at(
                    villager.x,
                    villager.y - 0.9,
                    center_x,
                    center_y,
                    radius,
                ) {
                    villager.health -= damage;
                    villager.vx += kb_x;
                    villager.vy += kb_y;
                    villager.grounded = false;
                    villager.hit_timer = villager.hit_timer.max(6);
                }
            }
        }
    }

    fn spill_inventory_items(&mut self, inventory: Inventory, drop_x: f64, drop_y: f64) {
        for stack in inventory.slots.into_iter().flatten() {
            for _ in 0..stack.count {
                self.item_entities
                    .push(ItemEntity::new(drop_x, drop_y, stack.item_type));
            }
        }
    }

    fn collect_world_explosion_drops(&mut self) {
        let losses = std::mem::take(&mut self.world.recent_explosion_block_losses);
        for (wx, wy, block, drop_item) in losses {
            if !drop_item {
                continue;
            }
            if let Some(item_type) = ItemType::from_block(block) {
                self.item_entities.push(ItemEntity::new(
                    wx as f64 + 0.5,
                    wy as f64 + 0.2,
                    item_type,
                ));
            }
        }
        let environment_drops = std::mem::take(&mut self.world.recent_environment_drops);
        for (wx, wy, item_type) in environment_drops {
            self.item_entities
                .push(ItemEntity::new(wx as f64 + 0.5, wy as f64 + 0.2, item_type));
        }
    }

    pub fn update(&mut self, mouse_target_x: i32, mouse_target_y: i32) {
        if self.chat.open {
            return;
        }
        if self.chat.mode == "commands" {
            self.chat.notice_ticks = self.chat.notice_ticks.saturating_sub(1);
            if self.chat.notice_ticks == 0 {
                self.chat.status.clear();
            }
        }
        self.world_tick = self.world_tick.saturating_add(1);
        self.tick_placed_furnaces();
        self.update_trial();
        if !self.cooperative() {
            self.update_furnace();
        }
        self.update_end_victory_sequence();
        if !self.cooperative() {
            if self.startup_splash_active {
                self.startup_splash_ticks = self.startup_splash_ticks.saturating_add(1);
                if self.startup_splash_ticks >= STARTUP_SPLASH_AUTO_DISMISS_TICKS {
                    self.dismiss_startup_splash();
                }
                return;
            }
            if self.credits_active {
                self.update_completion_credits();
                return;
            }
            if self.death_screen_active {
                self.death_screen_ticks = self.death_screen_ticks.saturating_add(1);
                self.player.age = self.player.age.saturating_add(1);
                return;
            }
            if self.settings_menu_open {
                return;
            }
            if self.respawn_grace_ticks > 0 {
                self.respawn_grace_ticks -= 1;
            }
            if self.player_combat_hurt_cooldown > 0 {
                self.player_combat_hurt_cooldown -= 1;
            }
        }
        self.world
            .load_chunks_for_motion(self.player.x, self.player.vx);
        if self.world.check_storage().is_err() {
            return;
        }
        self.close_chest_if_out_of_range();
        self.sanitize_enchant_levels();
        let mut rng = rand::thread_rng();
        if self.current_dimension == Dimension::Overworld {
            self.trim_far_overworld_mobs();
        } else if self.current_dimension == Dimension::Nether {
            self.trim_far_nether_mobs();
        } else if self.current_dimension == Dimension::End {
            self.trim_far_end_mobs();
        }
        self.apply_peaceful_cleanup_if_needed();
        let is_day_now = self.time_of_day > 4000.0 && self.time_of_day < 20000.0;
        let mob_spawning_enabled = self.can_spawn_mobs();
        let hostile_spawning_enabled = self.can_spawn_hostiles();
        let local_overworld_biome = self.overworld_biome_at_player();
        let local_passive_cap = self.tuned_overworld_passive_cap(local_overworld_biome);
        let local_squid_cap = self.tuned_overworld_squid_cap(local_overworld_biome);
        let local_wolf_cap = self.tuned_overworld_wolf_cap(local_overworld_biome);
        let local_ocelot_cap = self.tuned_overworld_ocelot_cap(local_overworld_biome);
        let local_passive_refill_cap = Self::spawn_refill_threshold(local_passive_cap, 2, 3);
        let local_squid_refill_cap = Self::spawn_refill_threshold(local_squid_cap, 2, 3);
        let local_wolf_refill_cap = Self::spawn_refill_threshold(local_wolf_cap, 1, 2);
        let local_ocelot_refill_cap = Self::spawn_refill_threshold(local_ocelot_cap, 1, 2);
        let generated_chunks = std::mem::take(&mut self.world.newly_generated_chunks);
        let mut chunk_hostile_spawns_remaining = 1usize;
        let mut chunk_passive_spawns_remaining = 1usize;
        let mut chunk_squid_spawns_remaining = 1usize;
        let mut chunk_predator_spawns_remaining = 1usize;
        for cx in generated_chunks {
            let wx_start = cx * CHUNK_WIDTH as i32;
            match self.current_dimension {
                Dimension::Overworld => {
                    if hostile_spawning_enabled && chunk_hostile_spawns_remaining > 0 {
                        // Hostile Mobs
                        let slime_cap = self.scaled_hostile_cap(OVERWORLD_SLIME_CAP);
                        for _ in 0..self.overworld_chunk_hostile_rolls(&mut rng) {
                            if !self.can_refill_overworld_hostiles() {
                                break;
                            }
                            let wx = wx_start + rng.gen_range(0..CHUNK_WIDTH as i32);
                            let can_try_slime = Self::is_slime_chunk(wx);
                            let spawn_y = if can_try_slime {
                                self.find_spawn_surface_for_slime(wx)
                                    .or_else(|| self.find_dark_overworld_spawn(wx))
                            } else {
                                self.find_dark_overworld_spawn(wx)
                            };
                            let Some(spawn_y) = spawn_y else {
                                continue;
                            };
                            let spawn_x = wx as f64 + 0.5;
                            if self.is_spawn_too_close_to_player(
                                spawn_x,
                                spawn_y,
                                OVERWORLD_HOSTILE_SPAWN_MIN_DIST_SQ,
                            ) {
                                continue;
                            }
                            if !self.reserve_overworld_hostile_slot() {
                                break;
                            }
                            if can_try_slime
                                && self.slimes.len() < slime_cap
                                && spawn_y >= 64.0
                                && self.should_spawn_overworld_slime(spawn_y, &mut rng)
                            {
                                self.slimes.push(Slime::new(
                                    spawn_x,
                                    spawn_y,
                                    Self::random_overworld_slime_size(&mut rng),
                                ));
                                chunk_hostile_spawns_remaining =
                                    chunk_hostile_spawns_remaining.saturating_sub(1);
                            } else {
                                self.spawn_overworld_hostile_at(spawn_x, spawn_y, &mut rng);
                                chunk_hostile_spawns_remaining =
                                    chunk_hostile_spawns_remaining.saturating_sub(1);
                            }
                            if chunk_hostile_spawns_remaining == 0 {
                                break;
                            }
                        }
                    }
                    if mob_spawning_enabled && chunk_passive_spawns_remaining > 0 {
                        // Passive Mobs
                        for _ in 0..rng.gen_range(0..3) {
                            if self.cows.len()
                                + self.sheep.len()
                                + self.pigs.len()
                                + self.chickens.len()
                                >= local_passive_refill_cap
                            {
                                break;
                            }
                            let wx = wx_start + rng.gen_range(0..CHUNK_WIDTH as i32);
                            let Some(spawn_y) = self.find_spawn_surface_for_daylight_mob(wx) else {
                                continue;
                            };
                            let spawn_x = wx as f64 + 0.5;
                            if self.is_spawn_too_close_to_player(
                                spawn_x,
                                spawn_y,
                                OVERWORLD_SPAWN_MIN_DIST_SQ,
                            ) {
                                continue;
                            }
                            if self.try_spawn_overworld_passive_mob(wx, spawn_y, &mut rng) {
                                chunk_passive_spawns_remaining =
                                    chunk_passive_spawns_remaining.saturating_sub(1);
                                if chunk_passive_spawns_remaining == 0 {
                                    break;
                                }
                            }
                        }

                        if chunk_squid_spawns_remaining > 0
                            && self.squids.len() < local_squid_refill_cap
                        {
                            for _ in 0..rng.gen_range(0..=2) {
                                let wx = wx_start + rng.gen_range(0..CHUNK_WIDTH as i32);
                                if self.try_spawn_overworld_squid(wx, &mut rng) {
                                    chunk_squid_spawns_remaining =
                                        chunk_squid_spawns_remaining.saturating_sub(1);
                                    break;
                                }
                            }
                        }

                        if chunk_predator_spawns_remaining > 0
                            && is_day_now
                            && self.wolves.len() < local_wolf_refill_cap
                        {
                            for _ in 0..rng.gen_range(0..=1) {
                                let wx = wx_start + rng.gen_range(0..CHUNK_WIDTH as i32);
                                let Some(spawn_y) = self.find_spawn_surface_for_daylight_mob(wx)
                                else {
                                    continue;
                                };
                                let spawn_x = wx as f64 + 0.5;
                                if self.is_spawn_too_close_to_player(
                                    spawn_x,
                                    spawn_y,
                                    OVERWORLD_SPAWN_MIN_DIST_SQ,
                                ) {
                                    continue;
                                }
                                if self.try_spawn_overworld_wolf(wx, spawn_y, &mut rng) {
                                    chunk_predator_spawns_remaining =
                                        chunk_predator_spawns_remaining.saturating_sub(1);
                                    break;
                                }
                            }
                        }

                        if chunk_predator_spawns_remaining > 0
                            && is_day_now
                            && self.ocelots.len() < local_ocelot_refill_cap
                        {
                            for _ in 0..rng.gen_range(0..=1) {
                                let wx = wx_start + rng.gen_range(0..CHUNK_WIDTH as i32);
                                let Some(spawn_y) = self.find_spawn_surface_for_daylight_mob(wx)
                                else {
                                    continue;
                                };
                                let spawn_x = wx as f64 + 0.5;
                                if self.is_spawn_too_close_to_player(
                                    spawn_x,
                                    spawn_y,
                                    OVERWORLD_SPAWN_MIN_DIST_SQ,
                                ) {
                                    continue;
                                }
                                if self.try_spawn_overworld_ocelot(wx, spawn_y, &mut rng) {
                                    chunk_predator_spawns_remaining =
                                        chunk_predator_spawns_remaining.saturating_sub(1);
                                    break;
                                }
                            }
                        }
                    }
                }
                Dimension::Nether => {
                    if !hostile_spawning_enabled {
                        continue;
                    }
                    let pigman_cap = self.scaled_hostile_cap(NETHER_PIGMAN_CAP);
                    let ghast_cap = self.scaled_hostile_cap(NETHER_GHAST_CAP);
                    let blaze_cap = self.scaled_hostile_cap(NETHER_BLAZE_CAP);
                    let spawn_chance_mult = self.hostile_spawn_chance_multiplier();
                    let chunk_center_x = wx_start + (CHUNK_WIDTH as i32 / 2);
                    let chunk_in_fortress = self
                        .world
                        .is_nether_fortress_zone(chunk_center_x, self.player.y.floor() as i32);
                    let chunk_blaze_hot_zone = chunk_in_fortress
                        && self.nether_blaze_hot_zone_near(
                            chunk_center_x,
                            self.player.y.floor() as i32,
                        );
                    // Balance pass: keep late-game Nether pressure high without sudden burst spikes.
                    let pigman_budget = pigman_cap.saturating_sub(self.pigmen.len());
                    let pigman_attempts = if pigman_budget == 0 {
                        0
                    } else if chunk_in_fortress {
                        usize::from(
                            rng.gen_bool(
                                ((if chunk_blaze_hot_zone { 0.36 } else { 0.32 })
                                    * spawn_chance_mult)
                                    .clamp(0.0, 0.95),
                            ),
                        )
                    } else if self.pigmen.len() < pigman_cap / 3 {
                        rng.gen_range(1..=2)
                    } else if self.pigmen.len() < (pigman_cap * 2) / 3 {
                        rng.gen_range(0..=1)
                    } else {
                        usize::from(rng.gen_bool((0.25 * spawn_chance_mult).clamp(0.0, 0.95)))
                    }
                    .min(pigman_budget);
                    for _ in 0..pigman_attempts {
                        let wx = wx_start + rng.gen_range(0..CHUNK_WIDTH as i32);
                        if let Some(spawn_y) = self.find_nether_spawn_surface_for_hostile(wx) {
                            let spawn_x = wx as f64 + 0.5;
                            if self.is_spawn_too_close_to_player(
                                spawn_x,
                                spawn_y,
                                NETHER_GROUND_SPAWN_MIN_DIST_SQ,
                            ) {
                                continue;
                            }
                            self.pigmen.push(ZombiePigman::new(spawn_x, spawn_y));
                        }
                    }
                    let ghast_spawn_chance = if chunk_in_fortress {
                        if self.ghasts.len() < ghast_cap {
                            if chunk_blaze_hot_zone { 0.02 } else { 0.03 }
                        } else {
                            0.0
                        }
                    } else if self.ghasts.len() < 2 {
                        0.16
                    } else if self.ghasts.len() < ghast_cap {
                        0.08
                    } else {
                        0.0
                    };
                    if self.ghasts.len() < ghast_cap
                        && self.fireballs.len() < GHAST_FIREBALL_SOFT_CAP
                        && rng.gen_bool((ghast_spawn_chance * spawn_chance_mult).clamp(0.0, 0.95))
                    {
                        let wx = wx_start + rng.gen_range(0..CHUNK_WIDTH as i32);
                        if let Some(spawn_y) = self.find_nether_air_spawn_for_ghast(wx) {
                            if chunk_in_fortress
                                && self
                                    .world
                                    .is_nether_fortress_zone(wx, spawn_y.floor() as i32)
                            {
                                continue;
                            }
                            let spawn_x = wx as f64 + 0.5;
                            if self.is_spawn_too_close_to_player(
                                spawn_x,
                                spawn_y,
                                NETHER_AIR_SPAWN_MIN_DIST_SQ,
                            ) {
                                continue;
                            }
                            self.ghasts.push(Ghast::new(spawn_x, spawn_y));
                        }
                    }
                    let blaze_spawn_chance = if chunk_in_fortress {
                        if chunk_blaze_hot_zone
                            && self.blazes.len() >= NETHER_BLAZE_LOCAL_CLUSTER_LIMIT
                        {
                            0.0
                        } else if self.blazes.len() < 2 {
                            if chunk_blaze_hot_zone { 0.08 } else { 0.12 }
                        } else if self.blazes.len() < blaze_cap {
                            if chunk_blaze_hot_zone { 0.03 } else { 0.06 }
                        } else {
                            0.0
                        }
                    } else if self.blazes.len() < 2 {
                        0.11
                    } else if self.blazes.len() < blaze_cap {
                        0.06
                    } else {
                        0.0
                    };
                    if self.blazes.len() < blaze_cap
                        && self.fireballs.len() < BLAZE_FIREBALL_SOFT_CAP
                        && rng.gen_bool((blaze_spawn_chance * spawn_chance_mult).clamp(0.0, 0.95))
                    {
                        let wx = wx_start + rng.gen_range(0..CHUNK_WIDTH as i32);
                        if let Some(spawn_y) = self.find_nether_air_spawn_for_blaze(wx) {
                            if chunk_in_fortress
                                && !self
                                    .world
                                    .is_nether_fortress_zone(wx, spawn_y.floor() as i32)
                            {
                                continue;
                            }
                            let spawn_x = wx as f64 + 0.5;
                            if self.is_spawn_too_close_to_player(
                                spawn_x,
                                spawn_y,
                                NETHER_AIR_SPAWN_MIN_DIST_SQ,
                            ) {
                                continue;
                            }
                            if self.nearby_blaze_count(
                                spawn_x,
                                spawn_y,
                                NETHER_BLAZE_LOCAL_CLUSTER_RADIUS_SQ,
                            ) >= NETHER_BLAZE_LOCAL_CLUSTER_LIMIT
                            {
                                continue;
                            }
                            self.blazes.push(Blaze::new(spawn_x, spawn_y));
                        }
                    }
                }
                Dimension::End => {
                    if !hostile_spawning_enabled {
                        continue;
                    }
                    let enderman_cap = self.scaled_hostile_cap(self.target_end_enderman_cap());
                    let enderman_budget = enderman_cap.saturating_sub(self.endermen.len());
                    let enderman_attempts = if enderman_budget == 0 {
                        0
                    } else if self.endermen.len() < enderman_cap / 2 {
                        rng.gen_range(1..=2)
                    } else {
                        rng.gen_range(0..=1)
                    }
                    .min(enderman_budget);
                    for _ in 0..enderman_attempts {
                        let wx = wx_start + rng.gen_range(0..CHUNK_WIDTH as i32);
                        if let Some(spawn_y) = self.find_end_spawn_surface_for_enderman(wx) {
                            let spawn_x = wx as f64 + 0.5;
                            if self.is_spawn_too_close_to_player(
                                spawn_x,
                                spawn_y,
                                END_ENDERMAN_SPAWN_MIN_DIST_SQ,
                            ) {
                                continue;
                            }
                            self.endermen.push(Enderman::new(spawn_x, spawn_y));
                        }
                    }
                }
            }
        }
        self.update_ambient_respawns(&mut rng, is_day_now);
        self.update_villager_population(&mut rng, is_day_now);
        if self.current_dimension == Dimension::End {
            self.ensure_end_boss_entities();
        }
        self.update_dungeon_spawners(&mut rng);
        self.update_silverfish_spawners(&mut rng);
        self.update_blaze_spawners(&mut rng);
        if self.cooperative() {
            let positions = self
                .peer_ids_here()
                .into_iter()
                .filter_map(|id| self.peer_session(id).map(|s| s.player().x.floor() as i32))
                .collect::<Vec<_>>();
            self.world.farming_skylight =
                if self.current_dimension == Dimension::Overworld && is_day_now {
                    15
                } else {
                    4
                };
            self.world.update_for_players(&positions);
        } else {
            self.world.farming_skylight =
                if self.current_dimension == Dimension::Overworld && is_day_now {
                    15
                } else {
                    4
                };
            self.world.update(self.player.x as i32);
        }
        self.apply_world_explosion_impacts();
        self.collect_world_explosion_drops();
        self.update_end_boss_encounter();
        if !self.cooperative() && self.handle_portal_transition() {
            return;
        }

        if !self.cooperative() {
            self.tick_player_survival(&mut rng);
        }

        if self.active_game_rules().do_daylight_cycle {
            self.time_of_day += DAYLIGHT_CYCLE_STEP_PER_TICK;
            if self.time_of_day >= 24000.0 {
                self.time_of_day -= 24000.0;
            }
        }
        self.update_weather(&mut rng);

        for i in (0..self.lightning_bolts.len()).rev() {
            if self.lightning_bolts[i].ttl > 0 {
                self.lightning_bolts[i].ttl -= 1;
            }
            if self.lightning_bolts[i].ttl == 0 {
                self.lightning_bolts.swap_remove(i);
            }
        }

        if self.current_dimension == Dimension::Overworld
            && self.weather == WeatherType::Thunderstorm
            && rng.gen_bool(0.004)
        {
            let strike_x = self.player.x.floor() as i32 + rng.gen_range(-20..=20);
            if self.precipitation_at(strike_x) != PrecipitationType::None
                && let Some(surface_y) = self.find_lightning_surface(strike_x)
                && self.is_exposed_to_sky(strike_x as f64 + 0.5, surface_y as f64 - 1.0)
            {
                self.apply_lightning_strike(strike_x, surface_y);
            }
        }
        let is_day = self.time_of_day > 4000.0 && self.time_of_day < 20000.0;

        if !self.cooperative() {
            self.tick_player_actions(mouse_target_x, mouse_target_y);
        }
        self.tick_creature_environment();
        let mut dead_zombies = Vec::new();
        for i in 0..self.zombies.len() {
            if self.zombies[i].health > 0.0
                && !self.body_terrain_loaded(
                    self.zombies[i].x,
                    self.zombies[i].y,
                    self.zombies[i].vx,
                    0.3,
                )
            {
                continue;
            }
            self.select_nearest_peer(self.zombies[i].x, self.zombies[i].y);
            let prev_x = self.zombies[i].x;
            if self.zombies[i].hit_timer > 0 {
                self.zombies[i].hit_timer -= 1;
            }
            let (target_x, target_y, visible) =
                self.hostile_target(self.zombies[i].x, self.zombies[i].y, 1.5);
            self.zombies[i].update_ai(target_x, target_y);
            if !visible {
                self.zombies[i].reroute_ticks = 0;
                self.zombies[i].stuck_ticks = 0;
            }
            let rerouting = self.zombies[i].reroute_ticks > 0 && self.zombies[i].reroute_dir != 0;
            let (reroute_ticks, reroute_vx) = Self::apply_ground_mob_reroute_velocity(
                self.zombies[i].reroute_ticks,
                self.zombies[i].reroute_dir,
                self.zombies[i].vx,
                0.14,
            );
            self.zombies[i].reroute_ticks = reroute_ticks;
            if rerouting && self.zombies[i].hit_timer == 0 && self.zombies[i].attack_windup == 0 {
                self.zombies[i].vx = reroute_vx;
                self.zombies[i].facing_right = reroute_vx >= 0.0;
            }
            self.zombies[i].age += 1;
            let (z_burn, z_fire_damage) = self.apply_undead_fire_rules(
                self.zombies[i].x,
                self.zombies[i].y,
                1.8,
                self.zombies[i].age,
                is_day,
                self.zombies[i].burning_timer,
            );
            self.zombies[i].burning_timer = z_burn;
            self.zombies[i].health -= z_fire_damage;
            if self.zombies[i].health <= 0.0 {
                dead_zombies.push(i);
                continue;
            }
            let chase_jump = self.should_ground_mob_chase_jump(
                self.zombies[i].x,
                self.zombies[i].y,
                self.zombies[i].vx,
                self.zombies[i].grounded,
                0.3,
                1.8,
            );
            let recovery_jump = self.should_ground_mob_vertical_recovery_jump(
                self.zombies[i].x,
                self.zombies[i].y,
                self.zombies[i].vx,
                self.zombies[i].grounded,
                0.3,
                1.8,
                self.zombies[i].stuck_ticks,
                self.zombies[i].reroute_ticks,
            );
            if (chase_jump || recovery_jump)
                && self.zombies[i].hit_timer == 0
                && self.zombies[i].attack_windup == 0
            {
                self.zombies[i].jump();
            }
            let (nx, ny, nvx, nvy, ngr, hit_wall) = self.calculate_mob_movement(
                self.zombies[i].x,
                self.zombies[i].y,
                self.zombies[i].vx,
                self.zombies[i].vy,
                self.zombies[i].grounded,
                0.3,
                1.8,
                true,
            );
            self.zombies[i].x = nx;
            self.zombies[i].y = ny;
            self.zombies[i].vx = nvx;
            self.zombies[i].vy = nvy;
            self.zombies[i].grounded = ngr;
            let prev_reroute_ticks = self.zombies[i].reroute_ticks;
            let (stuck_ticks, mut reroute_ticks, mut reroute_dir) = Self::next_ground_reroute_state(
                self.player.x,
                self.player.y,
                prev_x,
                nx,
                ny,
                hit_wall,
                ngr,
                self.zombies[i].stuck_ticks,
                self.zombies[i].reroute_ticks,
                self.zombies[i].reroute_dir,
            );
            (reroute_ticks, reroute_dir) = self.refine_ground_reroute_with_path(
                self.zombies[i].x,
                self.zombies[i].y,
                1.8,
                prev_reroute_ticks,
                stuck_ticks,
                reroute_ticks,
                reroute_dir,
            );
            self.zombies[i].stuck_ticks = stuck_ticks;
            self.zombies[i].reroute_ticks = reroute_ticks;
            self.zombies[i].reroute_dir = reroute_dir;
            if hit_wall && self.zombies[i].hit_timer == 0 && self.zombies[i].attack_windup == 0 {
                self.zombies[i].jump();
            }
            self.zombies[i].vx *= 0.5;
            if self.zombies[i].vx.abs() < 0.02 {
                self.zombies[i].vx = 0.0;
            }
            if self.zombies[i].attack_windup == 0 && self.zombies[i].attack_cooldown == 0 {
                self.zombies[i].facing_right = self.player.x >= self.zombies[i].x;
            }
            let can_melee_hit = self.can_melee_contact_player(
                self.zombies[i].x,
                self.zombies[i].y - 0.9,
                1.2,
                1.05,
            );
            let can_melee_hit = can_melee_hit
                && (self.player.x - self.zombies[i].x)
                    * (if self.zombies[i].facing_right {
                        1.0
                    } else {
                        -1.0
                    })
                    >= -0.15;
            let (cooldown, hurt) = (
                self.zombies[i].attack_cooldown,
                self.zombies[i].hit_timer > 0,
            );
            if combat::melee_strike_ready(
                &mut self.zombies[i].attack_windup,
                cooldown,
                can_melee_hit,
                hurt,
            ) {
                if self.apply_player_combat_damage(self.scaled_hostile_damage(2.0)) {
                    self.apply_player_contact_knockback(self.zombies[i].x, 0.42, 0.28);
                }
                self.zombies[i].attack_cooldown = self.scaled_hostile_cooldown(20);
            }
        }

        for idx in dead_zombies.into_iter().rev() {
            let z = self.zombies.swap_remove(idx);
            self.credit_trial_kill(z.director_event, z.last_player_damage_tick);
            self.item_entities
                .push(ItemEntity::new(z.x, z.y - 0.5, ItemType::RottenFlesh));
            if rng.gen_bool(0.3) {
                self.item_entities
                    .push(ItemEntity::new(z.x, z.y - 0.5, ItemType::Stick));
            }
            if self.mob_has_player_kill_credit(z.last_player_damage_tick) {
                self.spawn_experience_orbs(z.x, z.y - 0.5, 5, &mut rng);
            }
        }

        let mut dead_pigmen = Vec::new();
        for i in 0..self.pigmen.len() {
            if self.pigmen[i].health > 0.0
                && !self.body_terrain_loaded(
                    self.pigmen[i].x,
                    self.pigmen[i].y,
                    self.pigmen[i].vx,
                    0.3,
                )
            {
                continue;
            }
            self.select_nearest_peer(self.pigmen[i].x, self.pigmen[i].y);
            let prev_x = self.pigmen[i].x;
            if self.pigmen[i].hit_timer > 0 {
                self.pigmen[i].hit_timer -= 1;
            }
            let (target_x, target_y, visible) =
                self.hostile_target(self.pigmen[i].x, self.pigmen[i].y, 1.5);
            self.pigmen[i].update_ai(target_x, target_y);
            if !visible {
                self.pigmen[i].reroute_ticks = 0;
                self.pigmen[i].stuck_ticks = 0;
            }
            let rerouting = self.pigmen[i].reroute_ticks > 0 && self.pigmen[i].reroute_dir != 0;
            let (reroute_ticks, reroute_vx) = Self::apply_ground_mob_reroute_velocity(
                self.pigmen[i].reroute_ticks,
                self.pigmen[i].reroute_dir,
                self.pigmen[i].vx,
                0.15,
            );
            self.pigmen[i].reroute_ticks = reroute_ticks;
            if rerouting && self.pigmen[i].hit_timer == 0 && self.pigmen[i].attack_windup == 0 {
                self.pigmen[i].vx = reroute_vx;
                self.pigmen[i].facing_right = reroute_vx >= 0.0;
            }
            self.pigmen[i].age += 1;
            if self.pigmen[i].health <= 0.0 {
                dead_pigmen.push(i);
                continue;
            }
            let chase_jump = self.should_ground_mob_chase_jump(
                self.pigmen[i].x,
                self.pigmen[i].y,
                self.pigmen[i].vx,
                self.pigmen[i].grounded,
                0.3,
                1.8,
            );
            let recovery_jump = self.should_ground_mob_vertical_recovery_jump(
                self.pigmen[i].x,
                self.pigmen[i].y,
                self.pigmen[i].vx,
                self.pigmen[i].grounded,
                0.3,
                1.8,
                self.pigmen[i].stuck_ticks,
                self.pigmen[i].reroute_ticks,
            );
            if (chase_jump || recovery_jump)
                && self.pigmen[i].hit_timer == 0
                && self.pigmen[i].attack_windup == 0
            {
                self.pigmen[i].jump();
            }

            let (nx, ny, nvx, nvy, ngr, hit_wall) = self.calculate_mob_movement(
                self.pigmen[i].x,
                self.pigmen[i].y,
                self.pigmen[i].vx,
                self.pigmen[i].vy,
                self.pigmen[i].grounded,
                0.3,
                1.8,
                true,
            );
            self.pigmen[i].x = nx;
            self.pigmen[i].y = ny;
            self.pigmen[i].vx = nvx;
            self.pigmen[i].vy = nvy;
            self.pigmen[i].grounded = ngr;
            let prev_reroute_ticks = self.pigmen[i].reroute_ticks;
            let (stuck_ticks, mut reroute_ticks, mut reroute_dir) = Self::next_ground_reroute_state(
                self.player.x,
                self.player.y,
                prev_x,
                nx,
                ny,
                hit_wall,
                ngr,
                self.pigmen[i].stuck_ticks,
                self.pigmen[i].reroute_ticks,
                self.pigmen[i].reroute_dir,
            );
            (reroute_ticks, reroute_dir) = self.refine_ground_reroute_with_path(
                self.pigmen[i].x,
                self.pigmen[i].y,
                1.8,
                prev_reroute_ticks,
                stuck_ticks,
                reroute_ticks,
                reroute_dir,
            );
            self.pigmen[i].stuck_ticks = stuck_ticks;
            self.pigmen[i].reroute_ticks = reroute_ticks;
            self.pigmen[i].reroute_dir = reroute_dir;
            if hit_wall && self.pigmen[i].hit_timer == 0 && self.pigmen[i].attack_windup == 0 {
                self.pigmen[i].jump();
            }
            self.pigmen[i].vx *= 0.5;
            if self.pigmen[i].vx.abs() < 0.02 {
                self.pigmen[i].vx = 0.0;
            }

            if self.pigmen[i].is_aggressive() {
                if self.pigmen[i].attack_windup == 0 && self.pigmen[i].attack_cooldown == 0 {
                    self.pigmen[i].facing_right = self.player.x >= self.pigmen[i].x;
                }
                let can_melee_hit = self.can_melee_contact_player(
                    self.pigmen[i].x,
                    self.pigmen[i].y - 0.9,
                    1.2,
                    1.05,
                );
                let can_melee_hit = can_melee_hit
                    && (self.player.x - self.pigmen[i].x)
                        * (if self.pigmen[i].facing_right {
                            1.0
                        } else {
                            -1.0
                        })
                        >= -0.15;
                let (cooldown, hurt) =
                    (self.pigmen[i].attack_cooldown, self.pigmen[i].hit_timer > 0);
                if combat::melee_strike_ready(
                    &mut self.pigmen[i].attack_windup,
                    cooldown,
                    can_melee_hit,
                    hurt,
                ) {
                    if self.apply_player_combat_damage(self.scaled_hostile_damage(0.5)) {
                        self.apply_player_contact_knockback(self.pigmen[i].x, 0.46, 0.3);
                    }
                    self.pigmen[i].attack_cooldown = self.scaled_hostile_cooldown(18);
                }
            }
        }
        for idx in dead_pigmen.into_iter().rev() {
            let p = self.pigmen.swap_remove(idx);
            self.item_entities
                .push(ItemEntity::new(p.x, p.y - 0.5, ItemType::RottenFlesh));
            if rng.gen_bool(0.2) {
                self.item_entities
                    .push(ItemEntity::new(p.x, p.y - 0.5, ItemType::GoldIngot));
            }
            if self.mob_has_player_kill_credit(p.last_player_damage_tick) {
                self.spawn_experience_orbs(p.x, p.y - 0.5, 5, &mut rng);
            }
        }

        let mut pending_world_explosions: Vec<(i32, i32, i32, f32, u8)> = Vec::new();
        let mut dead_creepers = Vec::new();
        for i in 0..self.creepers.len() {
            if self.creepers[i].health > 0.0
                && !self.body_terrain_loaded(
                    self.creepers[i].x,
                    self.creepers[i].y,
                    self.creepers[i].vx,
                    0.3,
                )
            {
                continue;
            }
            self.select_nearest_peer(self.creepers[i].x, self.creepers[i].y);
            let prev_x = self.creepers[i].x;
            if self.creepers[i].hit_timer > 0 {
                self.creepers[i].hit_timer -= 1;
            }
            if self.creepers[i].health <= 0.0 {
                dead_creepers.push(i);
                continue;
            }
            let (target_x, target_y, visible) =
                self.hostile_target(self.creepers[i].x, self.creepers[i].y, 1.5);
            self.creepers[i].update_ai(target_x, target_y);
            if !visible {
                self.creepers[i].reroute_ticks = 0;
                self.creepers[i].stuck_ticks = 0;
            }
            let rerouting = self.creepers[i].reroute_ticks > 0 && self.creepers[i].reroute_dir != 0;
            let (reroute_ticks, reroute_vx) = Self::apply_ground_mob_reroute_velocity(
                self.creepers[i].reroute_ticks,
                self.creepers[i].reroute_dir,
                self.creepers[i].vx,
                0.12,
            );
            self.creepers[i].reroute_ticks = reroute_ticks;
            if rerouting && self.creepers[i].hit_timer == 0 {
                self.creepers[i].vx = reroute_vx;
                self.creepers[i].facing_right = reroute_vx >= 0.0;
            }
            self.creepers[i].age += 1;
            let chase_jump = self.should_ground_mob_chase_jump(
                self.creepers[i].x,
                self.creepers[i].y,
                self.creepers[i].vx,
                self.creepers[i].grounded,
                0.3,
                1.8,
            );
            let recovery_jump = self.should_ground_mob_vertical_recovery_jump(
                self.creepers[i].x,
                self.creepers[i].y,
                self.creepers[i].vx,
                self.creepers[i].grounded,
                0.3,
                1.8,
                self.creepers[i].stuck_ticks,
                self.creepers[i].reroute_ticks,
            );
            if chase_jump || recovery_jump {
                self.creepers[i].jump();
            }
            let (nx, ny, nvx, nvy, ngr, hit_wall) = self.calculate_mob_movement(
                self.creepers[i].x,
                self.creepers[i].y,
                self.creepers[i].vx,
                self.creepers[i].vy,
                self.creepers[i].grounded,
                0.3,
                1.8,
                true,
            );
            self.creepers[i].x = nx;
            self.creepers[i].y = ny;
            self.creepers[i].vx = nvx;
            self.creepers[i].vy = nvy;
            self.creepers[i].grounded = ngr;
            let prev_reroute_ticks = self.creepers[i].reroute_ticks;
            let (stuck_ticks, mut reroute_ticks, mut reroute_dir) = Self::next_ground_reroute_state(
                self.player.x,
                self.player.y,
                prev_x,
                nx,
                ny,
                hit_wall,
                ngr,
                self.creepers[i].stuck_ticks,
                self.creepers[i].reroute_ticks,
                self.creepers[i].reroute_dir,
            );
            (reroute_ticks, reroute_dir) = self.refine_ground_reroute_with_path(
                self.creepers[i].x,
                self.creepers[i].y,
                1.8,
                prev_reroute_ticks,
                stuck_ticks,
                reroute_ticks,
                reroute_dir,
            );
            self.creepers[i].stuck_ticks = stuck_ticks;
            self.creepers[i].reroute_ticks = reroute_ticks;
            self.creepers[i].reroute_dir = reroute_dir;
            if hit_wall {
                self.creepers[i].jump();
            }
            self.creepers[i].vx *= 0.5;
            if self.creepers[i].fuse_timer >= 30 {
                let cx = self.creepers[i].x.floor() as i32;
                let cy = self.creepers[i].y.floor() as i32;
                let blast_radius = if self.creepers[i].charged { 5 } else { 3 };
                let strength = if self.creepers[i].charged { 6.0 } else { 4.0 };
                pending_world_explosions.push((cx, cy, blast_radius, strength, 8));
                dead_creepers.push(i);
            }
        }
        for idx in dead_creepers.into_iter().rev() {
            let c = self.creepers.swap_remove(idx);
            if c.health <= 0.0 {
                self.item_entities
                    .push(ItemEntity::new(c.x, c.y - 0.5, ItemType::Gunpowder));
                if self.mob_has_player_kill_credit(c.last_player_damage_tick) {
                    self.spawn_experience_orbs(c.x, c.y - 0.5, 5, &mut rng);
                }
            }
        }

        let mut dead_skeletons = Vec::new();
        for i in 0..self.skeletons.len() {
            if self.skeletons[i].health > 0.0
                && !self.body_terrain_loaded(
                    self.skeletons[i].x,
                    self.skeletons[i].y,
                    self.skeletons[i].vx,
                    0.3,
                )
            {
                continue;
            }
            self.select_nearest_peer(self.skeletons[i].x, self.skeletons[i].y);
            let prev_x = self.skeletons[i].x;
            if self.skeletons[i].hit_timer > 0 {
                self.skeletons[i].hit_timer -= 1;
            }
            let (target_x, target_y, visible) =
                self.hostile_target(self.skeletons[i].x, self.skeletons[i].y, 1.5);
            if !visible {
                self.skeletons[i].bow_windup = 0;
            }
            let skeleton_wants_shot = self.skeletons[i].update_ai(target_x, target_y);
            if !visible {
                self.skeletons[i].reroute_ticks = 0;
                self.skeletons[i].stuck_ticks = 0;
            }
            let rerouting =
                self.skeletons[i].reroute_ticks > 0 && self.skeletons[i].reroute_dir != 0;
            let (reroute_ticks, reroute_vx) = Self::apply_ground_mob_reroute_velocity(
                self.skeletons[i].reroute_ticks,
                self.skeletons[i].reroute_dir,
                self.skeletons[i].vx,
                0.18,
            );
            self.skeletons[i].reroute_ticks = reroute_ticks;
            if rerouting && self.skeletons[i].hit_timer == 0 && self.skeletons[i].bow_windup == 0 {
                self.skeletons[i].vx = reroute_vx;
                self.skeletons[i].facing_right = reroute_vx >= 0.0;
            }
            let (sx, sy) = (self.skeletons[i].x, self.skeletons[i].y - 0.9);
            if skeleton_wants_shot {
                let has_los = self.has_line_of_sight(sx, sy, self.player.x, self.player.y - 0.9);
                if has_los {
                    let dx = self.skeletons[i].aim_x - sx;
                    let dy = self.skeletons[i].aim_y - sy;
                    let dist = (dx * dx + dy * dy).sqrt();
                    if dist > 0.001 {
                        self.arrows.push(Arrow::new_hostile(
                            sx,
                            sy,
                            (dx / dist) * 0.5,
                            (dy / dist) * 0.5,
                        ));
                        self.skeletons[i].bow_cooldown = self.scaled_hostile_cooldown(60);
                    }
                } else {
                    // Retry sooner when line-of-sight is blocked so skeletons reposition faster.
                    self.skeletons[i].bow_cooldown = self.skeletons[i]
                        .bow_cooldown
                        .min(self.scaled_hostile_cooldown(14));
                }
            }
            self.skeletons[i].age += 1;
            let (s_burn, s_fire_damage) = self.apply_undead_fire_rules(
                self.skeletons[i].x,
                self.skeletons[i].y,
                1.8,
                self.skeletons[i].age,
                is_day,
                self.skeletons[i].burning_timer,
            );
            self.skeletons[i].burning_timer = s_burn;
            self.skeletons[i].health -= s_fire_damage;
            if self.skeletons[i].health <= 0.0 {
                dead_skeletons.push(i);
                continue;
            }
            let chase_jump = self.should_ground_mob_chase_jump(
                self.skeletons[i].x,
                self.skeletons[i].y,
                self.skeletons[i].vx,
                self.skeletons[i].grounded,
                0.3,
                1.8,
            );
            let recovery_jump = self.should_ground_mob_vertical_recovery_jump(
                self.skeletons[i].x,
                self.skeletons[i].y,
                self.skeletons[i].vx,
                self.skeletons[i].grounded,
                0.3,
                1.8,
                self.skeletons[i].stuck_ticks,
                self.skeletons[i].reroute_ticks,
            );
            if (chase_jump || recovery_jump)
                && self.skeletons[i].hit_timer == 0
                && self.skeletons[i].bow_windup == 0
            {
                self.skeletons[i].jump();
            }
            let (nx, ny, nvx, nvy, ngr, hit_wall) = self.calculate_mob_movement(
                self.skeletons[i].x,
                self.skeletons[i].y,
                self.skeletons[i].vx,
                self.skeletons[i].vy,
                self.skeletons[i].grounded,
                0.3,
                1.8,
                true,
            );
            self.skeletons[i].x = nx;
            self.skeletons[i].y = ny;
            self.skeletons[i].vx = nvx;
            self.skeletons[i].vy = nvy;
            self.skeletons[i].grounded = ngr;
            let prev_reroute_ticks = self.skeletons[i].reroute_ticks;
            let (stuck_ticks, mut reroute_ticks, mut reroute_dir) = Self::next_ground_reroute_state(
                self.player.x,
                self.player.y,
                prev_x,
                nx,
                ny,
                hit_wall,
                ngr,
                self.skeletons[i].stuck_ticks,
                self.skeletons[i].reroute_ticks,
                self.skeletons[i].reroute_dir,
            );
            (reroute_ticks, reroute_dir) = self.refine_ground_reroute_with_path(
                self.skeletons[i].x,
                self.skeletons[i].y,
                1.8,
                prev_reroute_ticks,
                stuck_ticks,
                reroute_ticks,
                reroute_dir,
            );
            self.skeletons[i].stuck_ticks = stuck_ticks;
            self.skeletons[i].reroute_ticks = reroute_ticks;
            self.skeletons[i].reroute_dir = reroute_dir;
            if hit_wall && self.skeletons[i].hit_timer == 0 && self.skeletons[i].bow_windup == 0 {
                self.skeletons[i].jump();
            }
            self.skeletons[i].vx *= 0.5;
        }
        for idx in dead_skeletons.into_iter().rev() {
            let s = self.skeletons.swap_remove(idx);
            self.credit_trial_kill(s.director_event, s.last_player_damage_tick);
            self.item_entities
                .push(ItemEntity::new(s.x, s.y - 0.5, ItemType::Bone));
            if rng.gen_bool(0.5) {
                self.item_entities
                    .push(ItemEntity::new(s.x, s.y - 0.5, ItemType::Arrow));
            }
            if self.mob_has_player_kill_credit(s.last_player_damage_tick) {
                self.spawn_experience_orbs(s.x, s.y - 0.5, 5, &mut rng);
            }
        }

        let mut dead_spiders = Vec::new();
        for i in 0..self.spiders.len() {
            if self.spiders[i].health > 0.0
                && !self.body_terrain_loaded(
                    self.spiders[i].x,
                    self.spiders[i].y,
                    self.spiders[i].vx,
                    0.8,
                )
            {
                continue;
            }
            self.select_nearest_peer(self.spiders[i].x, self.spiders[i].y);
            let prev_x = self.spiders[i].x;
            if self.spiders[i].hit_timer > 0 {
                self.spiders[i].hit_timer -= 1;
            }
            if self.spiders[i].health <= 0.0 {
                dead_spiders.push(i);
                continue;
            }
            let (target_x, target_y, visible) =
                self.hostile_target(self.spiders[i].x, self.spiders[i].y, 0.5);
            self.spiders[i].update_ai(target_x, target_y, is_day);
            if !visible {
                self.spiders[i].reroute_ticks = 0;
                self.spiders[i].stuck_ticks = 0;
            }
            let rerouting = self.spiders[i].reroute_ticks > 0 && self.spiders[i].reroute_dir != 0;
            let (reroute_ticks, reroute_vx) = Self::apply_ground_mob_reroute_velocity(
                self.spiders[i].reroute_ticks,
                self.spiders[i].reroute_dir,
                self.spiders[i].vx,
                0.2,
            );
            self.spiders[i].reroute_ticks = reroute_ticks;
            if rerouting && self.spiders[i].hit_timer == 0 && self.spiders[i].attack_windup == 0 {
                self.spiders[i].vx = reroute_vx;
                self.spiders[i].facing_right = reroute_vx >= 0.0;
            }
            self.spiders[i].age += 1;
            let chase_jump = self.should_ground_mob_chase_jump(
                self.spiders[i].x,
                self.spiders[i].y,
                self.spiders[i].vx,
                self.spiders[i].grounded,
                0.6,
                0.9,
            );
            let recovery_jump = self.should_ground_mob_vertical_recovery_jump(
                self.spiders[i].x,
                self.spiders[i].y,
                self.spiders[i].vx,
                self.spiders[i].grounded,
                0.6,
                0.9,
                self.spiders[i].stuck_ticks,
                self.spiders[i].reroute_ticks,
            );
            if (chase_jump || recovery_jump)
                && self.spiders[i].hit_timer == 0
                && self.spiders[i].attack_windup == 0
            {
                self.spiders[i].jump();
            }
            let (nx, ny, nvx, nvy, ngr, hit_wall) = self.calculate_mob_movement(
                self.spiders[i].x,
                self.spiders[i].y,
                self.spiders[i].vx,
                self.spiders[i].vy,
                self.spiders[i].grounded,
                0.6,
                0.9,
                true,
            );
            self.spiders[i].x = nx;
            self.spiders[i].y = ny;
            self.spiders[i].vx = nvx;
            self.spiders[i].vy = nvy;
            self.spiders[i].grounded = ngr;
            let prev_reroute_ticks = self.spiders[i].reroute_ticks;
            let (stuck_ticks, mut reroute_ticks, mut reroute_dir) = Self::next_ground_reroute_state(
                self.player.x,
                self.player.y,
                prev_x,
                nx,
                ny,
                hit_wall,
                ngr,
                self.spiders[i].stuck_ticks,
                self.spiders[i].reroute_ticks,
                self.spiders[i].reroute_dir,
            );
            (reroute_ticks, reroute_dir) = self.refine_ground_reroute_with_path(
                self.spiders[i].x,
                self.spiders[i].y,
                0.9,
                prev_reroute_ticks,
                stuck_ticks,
                reroute_ticks,
                reroute_dir,
            );
            self.spiders[i].stuck_ticks = stuck_ticks;
            self.spiders[i].reroute_ticks = reroute_ticks;
            self.spiders[i].reroute_dir = reroute_dir;
            if hit_wall && self.spiders[i].hit_timer == 0 && self.spiders[i].attack_windup == 0 {
                self.spiders[i].jump();
            }
            self.spiders[i].vx *= 0.5;
            if self.spiders[i].attack_windup == 0 && self.spiders[i].attack_cooldown == 0 {
                self.spiders[i].facing_right = self.player.x >= self.spiders[i].x;
            }
            let can_melee_hit = self.can_melee_contact_player(
                self.spiders[i].x,
                self.spiders[i].y - 0.45,
                1.2,
                0.85,
            );
            let can_melee_hit = can_melee_hit
                && (self.player.x - self.spiders[i].x)
                    * (if self.spiders[i].facing_right {
                        1.0
                    } else {
                        -1.0
                    })
                    >= -0.15;
            let (cooldown, hurt) = (
                self.spiders[i].attack_cooldown,
                self.spiders[i].hit_timer > 0,
            );
            if combat::melee_strike_ready(
                &mut self.spiders[i].attack_windup,
                cooldown,
                can_melee_hit,
                hurt,
            ) {
                if self.apply_player_combat_damage(self.scaled_hostile_damage(0.5)) {
                    self.apply_player_contact_knockback(self.spiders[i].x, 0.34, 0.0);
                }
                self.spiders[i].attack_cooldown = self.scaled_hostile_cooldown(20);
            }
        }
        for idx in dead_spiders.into_iter().rev() {
            let s = self.spiders.swap_remove(idx);
            self.item_entities
                .push(ItemEntity::new(s.x, s.y - 0.5, ItemType::String));
            if self.mob_has_player_kill_credit(s.last_player_damage_tick) {
                self.spawn_experience_orbs(s.x, s.y - 0.5, 5, &mut rng);
            }
        }

        let mut dead_silverfish = Vec::new();
        for i in 0..self.silverfish.len() {
            if self.silverfish[i].health > 0.0
                && !self.body_terrain_loaded(
                    self.silverfish[i].x,
                    self.silverfish[i].y,
                    self.silverfish[i].vx,
                    0.4,
                )
            {
                continue;
            }
            self.select_nearest_peer(self.silverfish[i].x, self.silverfish[i].y);
            let prev_x = self.silverfish[i].x;
            if self.silverfish[i].hit_timer > 0 {
                self.silverfish[i].hit_timer -= 1;
            }
            if self.silverfish[i].health <= 0.0 {
                dead_silverfish.push(i);
                continue;
            }
            let (target_x, target_y, visible) =
                self.hostile_target(self.silverfish[i].x, self.silverfish[i].y, 0.3);
            self.silverfish[i].update_ai(target_x, target_y);
            if !visible {
                self.silverfish[i].reroute_ticks = 0;
                self.silverfish[i].stuck_ticks = 0;
            }
            let rerouting =
                self.silverfish[i].reroute_ticks > 0 && self.silverfish[i].reroute_dir != 0;
            let (reroute_ticks, reroute_vx) = Self::apply_ground_mob_reroute_velocity(
                self.silverfish[i].reroute_ticks,
                self.silverfish[i].reroute_dir,
                self.silverfish[i].vx,
                0.12,
            );
            self.silverfish[i].reroute_ticks = reroute_ticks;
            if rerouting
                && self.silverfish[i].hit_timer == 0
                && self.silverfish[i].attack_windup == 0
            {
                self.silverfish[i].vx = reroute_vx;
                self.silverfish[i].facing_right = reroute_vx >= 0.0;
            }
            self.silverfish[i].age += 1;
            let chase_jump = self.should_ground_mob_chase_jump(
                self.silverfish[i].x,
                self.silverfish[i].y,
                self.silverfish[i].vx,
                self.silverfish[i].grounded,
                0.24,
                0.8,
            );
            let recovery_jump = self.should_ground_mob_vertical_recovery_jump(
                self.silverfish[i].x,
                self.silverfish[i].y,
                self.silverfish[i].vx,
                self.silverfish[i].grounded,
                0.24,
                0.8,
                self.silverfish[i].stuck_ticks,
                self.silverfish[i].reroute_ticks,
            );
            if (chase_jump || recovery_jump)
                && self.silverfish[i].hit_timer == 0
                && self.silverfish[i].attack_windup == 0
            {
                self.silverfish[i].jump();
            }
            let (nx, ny, nvx, nvy, ngr, hit_wall) = self.calculate_mob_movement(
                self.silverfish[i].x,
                self.silverfish[i].y,
                self.silverfish[i].vx,
                self.silverfish[i].vy,
                self.silverfish[i].grounded,
                0.24,
                0.8,
                true,
            );
            self.silverfish[i].x = nx;
            self.silverfish[i].y = ny;
            self.silverfish[i].vx = nvx;
            self.silverfish[i].vy = nvy;
            self.silverfish[i].grounded = ngr;
            let prev_reroute_ticks = self.silverfish[i].reroute_ticks;
            let (stuck_ticks, mut reroute_ticks, mut reroute_dir) = Self::next_ground_reroute_state(
                self.player.x,
                self.player.y,
                prev_x,
                nx,
                ny,
                hit_wall,
                ngr,
                self.silverfish[i].stuck_ticks,
                self.silverfish[i].reroute_ticks,
                self.silverfish[i].reroute_dir,
            );
            (reroute_ticks, reroute_dir) = self.refine_ground_reroute_with_path(
                self.silverfish[i].x,
                self.silverfish[i].y,
                0.8,
                prev_reroute_ticks,
                stuck_ticks,
                reroute_ticks,
                reroute_dir,
            );
            self.silverfish[i].stuck_ticks = stuck_ticks;
            self.silverfish[i].reroute_ticks = reroute_ticks;
            self.silverfish[i].reroute_dir = reroute_dir;
            if hit_wall
                && self.silverfish[i].hit_timer == 0
                && self.silverfish[i].attack_windup == 0
            {
                self.silverfish[i].jump();
            }
            self.silverfish[i].vx *= 0.58;
            if self.silverfish[i].vx.abs() < 0.02 {
                self.silverfish[i].vx = 0.0;
            }

            if self.silverfish[i].attack_windup == 0 && self.silverfish[i].attack_cooldown == 0 {
                self.silverfish[i].facing_right = self.player.x >= self.silverfish[i].x;
            }
            let can_melee_hit = self.can_melee_contact_player(
                self.silverfish[i].x,
                self.silverfish[i].y - 0.35,
                1.0,
                0.8,
            );
            let can_melee_hit = can_melee_hit
                && (self.player.x - self.silverfish[i].x)
                    * (if self.silverfish[i].facing_right {
                        1.0
                    } else {
                        -1.0
                    })
                    >= -0.15;
            let (cooldown, hurt) = (
                self.silverfish[i].attack_cooldown,
                self.silverfish[i].hit_timer > 0,
            );
            if combat::melee_strike_ready(
                &mut self.silverfish[i].attack_windup,
                cooldown,
                can_melee_hit,
                hurt,
            ) {
                if self.apply_player_combat_damage(self.scaled_hostile_damage(0.35)) {
                    self.apply_player_contact_knockback(self.silverfish[i].x, 0.22, 0.16);
                }
                self.silverfish[i].attack_cooldown = self.scaled_hostile_cooldown(18);
            }
        }
        for idx in dead_silverfish.into_iter().rev() {
            let s = self.silverfish.swap_remove(idx);
            if self.mob_has_player_kill_credit(s.last_player_damage_tick) {
                self.spawn_experience_orbs(s.x, s.y - 0.35, 5, &mut rng);
            }
        }

        let mut dead_slimes = Vec::new();
        for i in 0..self.slimes.len() {
            if self.slimes[i].health > 0.0
                && !self.body_terrain_loaded(
                    self.slimes[i].x,
                    self.slimes[i].y,
                    self.slimes[i].vx,
                    self.slimes[i].half_width(),
                )
            {
                continue;
            }
            self.select_nearest_peer(self.slimes[i].x, self.slimes[i].y);
            let prev_x = self.slimes[i].x;
            if self.slimes[i].hit_timer > 0 {
                self.slimes[i].hit_timer -= 1;
            }
            if self.slimes[i].health <= 0.0 {
                dead_slimes.push(i);
                continue;
            }
            let (target_x, target_y, visible) =
                self.hostile_target(self.slimes[i].x, self.slimes[i].y, 0.5);
            self.slimes[i].update_ai(target_x, target_y, is_day);
            if !visible {
                self.slimes[i].reroute_ticks = 0;
                self.slimes[i].stuck_ticks = 0;
            }
            let rerouting = self.slimes[i].reroute_ticks > 0 && self.slimes[i].reroute_dir != 0;
            let reroute_speed = self.slimes[i].move_speed() * 0.72;
            let (reroute_ticks, reroute_vx) = Self::apply_ground_mob_reroute_velocity(
                self.slimes[i].reroute_ticks,
                self.slimes[i].reroute_dir,
                self.slimes[i].vx,
                reroute_speed,
            );
            self.slimes[i].reroute_ticks = reroute_ticks;
            if rerouting && self.slimes[i].hit_timer == 0 && self.slimes[i].attack_windup == 0 {
                self.slimes[i].vx = reroute_vx;
                self.slimes[i].facing_right = reroute_vx >= 0.0;
            }

            self.slimes[i].age += 1;
            let half_width = self.slimes[i].half_width();
            let height = self.slimes[i].height();
            let chase_jump = self.should_ground_mob_chase_jump(
                self.slimes[i].x,
                self.slimes[i].y,
                self.slimes[i].vx,
                self.slimes[i].grounded,
                half_width,
                height,
            );
            let recovery_jump = self.should_ground_mob_vertical_recovery_jump(
                self.slimes[i].x,
                self.slimes[i].y,
                self.slimes[i].vx,
                self.slimes[i].grounded,
                half_width,
                height,
                self.slimes[i].stuck_ticks,
                self.slimes[i].reroute_ticks,
            );
            if (chase_jump || recovery_jump)
                && self.slimes[i].hit_timer == 0
                && self.slimes[i].attack_windup == 0
            {
                self.slimes[i].jump();
            }

            let (nx, ny, nvx, nvy, ngr, hit_wall) = self.calculate_mob_movement(
                self.slimes[i].x,
                self.slimes[i].y,
                self.slimes[i].vx,
                self.slimes[i].vy,
                self.slimes[i].grounded,
                half_width,
                height,
                true,
            );
            self.slimes[i].x = nx;
            self.slimes[i].y = ny;
            self.slimes[i].vx = nvx;
            self.slimes[i].vy = nvy;
            self.slimes[i].grounded = ngr;
            let prev_reroute_ticks = self.slimes[i].reroute_ticks;
            let (stuck_ticks, mut reroute_ticks, mut reroute_dir) = Self::next_ground_reroute_state(
                self.player.x,
                self.player.y,
                prev_x,
                nx,
                ny,
                hit_wall,
                ngr,
                self.slimes[i].stuck_ticks,
                self.slimes[i].reroute_ticks,
                self.slimes[i].reroute_dir,
            );
            (reroute_ticks, reroute_dir) = self.refine_ground_reroute_with_path(
                self.slimes[i].x,
                self.slimes[i].y,
                height,
                prev_reroute_ticks,
                stuck_ticks,
                reroute_ticks,
                reroute_dir,
            );
            self.slimes[i].stuck_ticks = stuck_ticks;
            self.slimes[i].reroute_ticks = reroute_ticks;
            self.slimes[i].reroute_dir = reroute_dir;
            if hit_wall && self.slimes[i].hit_timer == 0 && self.slimes[i].attack_windup == 0 {
                self.slimes[i].jump();
            }
            self.slimes[i].vx *= 0.6;
            if self.slimes[i].vx.abs() < 0.015 {
                self.slimes[i].vx = 0.0;
            }

            let center_y = self.slimes[i].y - height * 0.5;
            let contact_range = half_width + 0.75;
            let vertical_reach = (0.55 + height * 0.35).clamp(0.7, 1.45);
            if self.slimes[i].attack_windup == 0 && self.slimes[i].attack_cooldown == 0 {
                self.slimes[i].facing_right = self.player.x >= self.slimes[i].x;
            }
            let can_melee_hit = self.can_melee_contact_player(
                self.slimes[i].x,
                center_y,
                contact_range,
                vertical_reach,
            );
            let can_melee_hit = can_melee_hit
                && (self.player.x - self.slimes[i].x)
                    * (if self.slimes[i].facing_right {
                        1.0
                    } else {
                        -1.0
                    })
                    >= -0.15;
            let (cooldown, hurt) = (self.slimes[i].attack_cooldown, self.slimes[i].hit_timer > 0);
            if combat::melee_strike_ready(
                &mut self.slimes[i].attack_windup,
                cooldown,
                can_melee_hit,
                hurt,
            ) {
                let damage = self.scaled_hostile_damage(self.slimes[i].contact_damage());
                if damage > 0.0 && self.apply_player_combat_damage(damage) {
                    self.apply_player_contact_knockback(
                        self.slimes[i].x,
                        0.18 + half_width * 0.2,
                        0.16,
                    );
                }
                self.slimes[i].attack_cooldown =
                    self.scaled_hostile_cooldown(match self.slimes[i].size {
                        4 => 16,
                        2 => 22,
                        _ => 28,
                    });
            }
        }
        let mut split_slimes = Vec::new();
        for idx in dead_slimes.into_iter().rev() {
            let s = self.slimes.swap_remove(idx);
            if let Some(child_size) = s.split_size() {
                let split_count = rng.gen_range(2..=4);
                for n in 0..split_count {
                    let center = (split_count - 1) as f64 * 0.5;
                    let offset = (n as f64 - center) * (0.24 + child_size as f64 * 0.04);
                    split_slimes.push((s.x + offset, s.y, child_size));
                }
            } else {
                if rng.gen_bool(0.45) {
                    self.item_entities
                        .push(ItemEntity::new(s.x, s.y - 0.2, ItemType::Slimeball));
                }
                if rng.gen_bool(0.15) {
                    self.item_entities
                        .push(ItemEntity::new(s.x, s.y - 0.2, ItemType::Slimeball));
                }
            }
            let xp_drop = match s.size {
                4 => 6,
                2 => 3,
                _ => 1,
            };
            if self.mob_has_player_kill_credit(s.last_player_damage_tick) {
                self.spawn_experience_orbs(s.x, s.y - 0.35, xp_drop, &mut rng);
            }
        }
        for (x, y, size) in split_slimes {
            let mut child = Slime::new(x, y, size);
            child.vx = rng.gen_range(-0.12..0.12);
            child.vy = -0.18;
            child.grounded = false;
            self.slimes.push(child);
        }

        let mut dead_ghasts = Vec::new();
        for i in 0..self.ghasts.len() {
            if self.ghasts[i].health > 0.0
                && !self.body_terrain_loaded(
                    self.ghasts[i].x,
                    self.ghasts[i].y,
                    self.ghasts[i].vx,
                    0.875,
                )
            {
                continue;
            }
            self.select_nearest_peer(self.ghasts[i].x, self.ghasts[i].y);
            if self.ghasts[i].hit_timer > 0 {
                self.ghasts[i].hit_timer -= 1;
            }
            let (target_x, target_y, visible) =
                self.hostile_target(self.ghasts[i].x, self.ghasts[i].y, 1.4);
            let shoot = self.ghasts[i].update_ai_with_visibility(
                if visible { target_x } else { self.player.x },
                if visible { target_y } else { self.player.y },
                visible,
            );
            self.ghasts[i].age += 1;
            if self.ghasts[i].health <= 0.0 {
                dead_ghasts.push(i);
                continue;
            }

            let (x, y, vx, vy) = self.move_flying_body(
                (self.ghasts[i].x, self.ghasts[i].y),
                (self.ghasts[i].vx, self.ghasts[i].vy),
                (0.875, 1.875),
                0.6,
            );
            self.ghasts[i].x = x;
            self.ghasts[i].y = y;
            self.ghasts[i].vx = vx;
            self.ghasts[i].vy = vy;

            if self.ghasts[i].y < 6.0 {
                self.ghasts[i].y = 6.0;
                self.ghasts[i].vy = self.ghasts[i].vy.abs() * 0.5;
            } else if self.ghasts[i].y > 96.0 {
                self.ghasts[i].y = 96.0;
                self.ghasts[i].vy = -self.ghasts[i].vy.abs() * 0.5;
            }

            self.ghasts[i].vx *= 0.97;
            self.ghasts[i].vy *= 0.97;

            if shoot && self.fireballs.len() < GHAST_FIREBALL_SOFT_CAP {
                let sx = self.ghasts[i].x;
                let sy = self.ghasts[i].y - 1.0;
                let dx = self.player.x - sx;
                let dy = (self.player.y - 0.9) - sy;
                let dist = (dx * dx + dy * dy).sqrt();
                if dist > 0.001 {
                    self.fireballs.push(Fireball::new(
                        sx,
                        sy,
                        (dx / dist) * 0.30,
                        (dy / dist) * 0.30,
                    ));
                    self.ghasts[i].shoot_cooldown = self.scaled_hostile_cooldown(75);
                }
            }

            let can_contact_hit =
                self.can_melee_contact_player(self.ghasts[i].x, self.ghasts[i].y - 1.0, 1.4, 1.1);
            if can_contact_hit && self.apply_player_combat_damage(self.scaled_hostile_damage(0.35))
            {
                self.apply_player_contact_knockback(self.ghasts[i].x, 0.4, 0.2);
            }
        }
        for idx in dead_ghasts.into_iter().rev() {
            let g = self.ghasts.swap_remove(idx);
            self.item_entities
                .push(ItemEntity::new(g.x, g.y - 0.5, ItemType::Gunpowder));
            if rng.gen_bool(0.35) {
                self.item_entities
                    .push(ItemEntity::new(g.x, g.y - 0.5, ItemType::Gunpowder));
            }
            if rng.gen_bool(0.28) {
                self.item_entities
                    .push(ItemEntity::new(g.x, g.y - 0.5, ItemType::GhastTear));
            }
            if self.mob_has_player_kill_credit(g.last_player_damage_tick) {
                self.spawn_experience_orbs(g.x, g.y - 0.5, 5, &mut rng);
            }
        }

        let mut dead_blazes = Vec::new();
        for i in 0..self.blazes.len() {
            if self.blazes[i].health > 0.0
                && !self.body_terrain_loaded(
                    self.blazes[i].x,
                    self.blazes[i].y,
                    self.blazes[i].vx,
                    0.5,
                )
            {
                continue;
            }
            self.select_nearest_peer(self.blazes[i].x, self.blazes[i].y);
            if self.blazes[i].hit_timer > 0 {
                self.blazes[i].hit_timer -= 1;
            }
            let (target_x, target_y, _visible) =
                self.hostile_target(self.blazes[i].x, self.blazes[i].y, 1.4);
            let shoot = self.blazes[i].update_ai(target_x, target_y);
            self.blazes[i].age += 1;
            if self.blazes[i].health <= 0.0 {
                dead_blazes.push(i);
                continue;
            }

            let (x, y, vx, vy) = self.move_flying_body(
                (self.blazes[i].x, self.blazes[i].y),
                (self.blazes[i].vx, self.blazes[i].vy),
                (0.5, 1.75),
                0.55,
            );
            self.blazes[i].x = x;
            self.blazes[i].y = y;
            self.blazes[i].vx = vx;
            self.blazes[i].vy = vy;

            if self.blazes[i].y < 8.0 {
                self.blazes[i].y = 8.0;
                self.blazes[i].vy = self.blazes[i].vy.abs() * 0.5;
            } else if self.blazes[i].y > 100.0 {
                self.blazes[i].y = 100.0;
                self.blazes[i].vy = -self.blazes[i].vy.abs() * 0.5;
            }

            self.blazes[i].vx *= 0.97;
            self.blazes[i].vy *= 0.97;

            if shoot && self.fireballs.len() < BLAZE_FIREBALL_SOFT_CAP {
                let sx = self.blazes[i].x;
                let sy = self.blazes[i].y - 0.9;
                let dx = self.player.x - sx;
                let dy = (self.player.y - 0.9) - sy;
                let dist = (dx * dx + dy * dy).sqrt();
                if dist > 0.001 {
                    self.fireballs.push(Fireball::new(
                        sx,
                        sy,
                        (dx / dist) * 0.34,
                        (dy / dist) * 0.34,
                    ));
                    self.blazes[i].shoot_cooldown = self.scaled_hostile_cooldown(60);
                }
            }

            let can_melee_hit =
                self.can_melee_contact_player(self.blazes[i].x, self.blazes[i].y - 0.9, 1.25, 1.05);
            let (cooldown, hurt) = (self.blazes[i].attack_cooldown, self.blazes[i].hit_timer > 0);
            if combat::melee_strike_ready(
                &mut self.blazes[i].attack_windup,
                cooldown,
                can_melee_hit,
                hurt,
            ) {
                if self.apply_player_combat_damage(self.scaled_hostile_damage(0.85)) {
                    if !self.has_fire_resistance() {
                        self.player.burning_timer = self.player.burning_timer.max(65);
                    }
                    self.apply_player_contact_knockback(self.blazes[i].x, 0.38, 0.2);
                }
                self.blazes[i].attack_cooldown = self.scaled_hostile_cooldown(28);
            }
        }
        for idx in dead_blazes.into_iter().rev() {
            let b = self.blazes.swap_remove(idx);
            if rng.gen_bool(0.55) {
                self.item_entities
                    .push(ItemEntity::new(b.x, b.y - 0.5, ItemType::BlazeRod));
            }
            if self.mob_has_player_kill_credit(b.last_player_damage_tick) {
                self.spawn_experience_orbs(b.x, b.y - 0.5, 10, &mut rng);
            }
        }

        let mut dead_endermen = Vec::new();
        for i in 0..self.endermen.len() {
            if self.endermen[i].health > 0.0
                && !self.body_terrain_loaded(
                    self.endermen[i].x,
                    self.endermen[i].y,
                    self.endermen[i].vx,
                    0.3,
                )
            {
                continue;
            }
            self.select_nearest_peer(self.endermen[i].x, self.endermen[i].y);
            let prev_x = self.endermen[i].x;
            if self.endermen[i].hit_timer > 0 {
                self.endermen[i].hit_timer -= 1;
            }
            let (target_x, target_y, visible) =
                self.hostile_target(self.endermen[i].x, self.endermen[i].y, 2.45);
            let should_teleport = self.endermen[i].update_ai(
                target_x,
                target_y,
                self.current_dimension == Dimension::End,
            );
            let rerouting = self.endermen[i].reroute_ticks > 0 && self.endermen[i].reroute_dir != 0;
            let (reroute_ticks, reroute_vx) = Self::apply_ground_mob_reroute_velocity(
                self.endermen[i].reroute_ticks,
                self.endermen[i].reroute_dir,
                self.endermen[i].vx,
                0.16,
            );
            self.endermen[i].reroute_ticks = reroute_ticks;
            if rerouting && self.endermen[i].hit_timer == 0 && self.endermen[i].attack_windup == 0 {
                self.endermen[i].vx = reroute_vx;
                self.endermen[i].facing_right = reroute_vx >= 0.0;
            }
            self.endermen[i].age += 1;

            let feet_y = self.endermen[i].y.floor() as i32;
            let head_y = (self.endermen[i].y - 2.6).floor() as i32;
            let ex = self.endermen[i].x.floor() as i32;
            let in_water = matches!(self.world.get_block(ex, feet_y), BlockType::Water(_))
                || matches!(self.world.get_block(ex, head_y), BlockType::Water(_));
            if in_water {
                if self.endermen[i].age.is_multiple_of(10) {
                    self.endermen[i].health -= 1.0;
                }
                self.endermen[i].provoke();
            }

            if self.endermen[i].health <= 0.0 {
                dead_endermen.push(i);
                continue;
            }
            let chase_jump = self.should_ground_mob_chase_jump(
                self.endermen[i].x,
                self.endermen[i].y,
                self.endermen[i].vx,
                self.endermen[i].grounded,
                0.3,
                2.7,
            );
            let recovery_jump = self.should_ground_mob_vertical_recovery_jump(
                self.endermen[i].x,
                self.endermen[i].y,
                self.endermen[i].vx,
                self.endermen[i].grounded,
                0.3,
                2.7,
                self.endermen[i].stuck_ticks,
                self.endermen[i].reroute_ticks,
            );
            if (chase_jump || recovery_jump)
                && self.endermen[i].hit_timer == 0
                && self.endermen[i].attack_windup == 0
            {
                self.endermen[i].jump();
            }
            let (nx, ny, nvx, nvy, ngr, hit_wall) = self.calculate_mob_movement(
                self.endermen[i].x,
                self.endermen[i].y,
                self.endermen[i].vx,
                self.endermen[i].vy,
                self.endermen[i].grounded,
                0.3,
                2.7,
                true,
            );
            self.endermen[i].x = nx;
            self.endermen[i].y = ny;
            self.endermen[i].vx = nvx;
            self.endermen[i].vy = nvy;
            self.endermen[i].grounded = ngr;
            let prev_reroute_ticks = self.endermen[i].reroute_ticks;
            let (stuck_ticks, mut reroute_ticks, mut reroute_dir) = Self::next_ground_reroute_state(
                self.player.x,
                self.player.y,
                prev_x,
                nx,
                ny,
                hit_wall,
                ngr,
                self.endermen[i].stuck_ticks,
                self.endermen[i].reroute_ticks,
                self.endermen[i].reroute_dir,
            );
            (reroute_ticks, reroute_dir) = self.refine_ground_reroute_with_path(
                self.endermen[i].x,
                self.endermen[i].y,
                2.7,
                prev_reroute_ticks,
                stuck_ticks,
                reroute_ticks,
                reroute_dir,
            );
            self.endermen[i].stuck_ticks = stuck_ticks;
            self.endermen[i].reroute_ticks = reroute_ticks;
            self.endermen[i].reroute_dir = reroute_dir;
            if hit_wall && self.endermen[i].hit_timer == 0 && self.endermen[i].attack_windup == 0 {
                self.endermen[i].jump();
            }
            self.endermen[i].vx *= 0.6;

            if (visible && should_teleport) || in_water {
                let anchor_x = if self.endermen[i].aggressive_timer > 0 {
                    self.player.x.floor() as i32
                } else {
                    self.endermen[i].x.floor() as i32
                };
                let mut teleported = false;
                for _ in 0..10 {
                    let target_x = anchor_x + rng.gen_range(-14..=14);
                    self.world.load_chunks_around(target_x);
                    let maybe_y = if self.current_dimension == Dimension::End {
                        self.find_end_spawn_surface_for_enderman(target_x)
                    } else {
                        self.find_spawn_surface_for_mob(target_x)
                    };
                    if let Some(target_y) = maybe_y {
                        self.endermen[i].x = target_x as f64 + 0.5;
                        self.endermen[i].y = target_y;
                        self.endermen[i].vx = 0.0;
                        self.endermen[i].vy = 0.0;
                        self.endermen[i].grounded = false;
                        self.endermen[i].teleport_cooldown = 90;
                        teleported = true;
                        break;
                    }
                }
                if !teleported {
                    self.endermen[i].teleport_cooldown = 25;
                }
            }

            if self.endermen[i].attack_windup == 0 && self.endermen[i].attack_cooldown == 0 {
                self.endermen[i].facing_right = self.player.x >= self.endermen[i].x;
            }
            let can_melee_hit = self.can_melee_contact_player(
                self.endermen[i].x,
                self.endermen[i].y - 1.4,
                1.35,
                1.2,
            );
            let can_melee_hit = can_melee_hit
                && (self.player.x - self.endermen[i].x)
                    * (if self.endermen[i].facing_right {
                        1.0
                    } else {
                        -1.0
                    })
                    >= -0.15;
            let (cooldown, hurt) = (
                self.endermen[i].attack_cooldown,
                self.endermen[i].hit_timer > 0,
            );
            if combat::melee_strike_ready(
                &mut self.endermen[i].attack_windup,
                cooldown,
                can_melee_hit,
                hurt,
            ) {
                if self.apply_player_combat_damage(self.scaled_hostile_damage(1.5)) {
                    self.apply_player_contact_knockback(self.endermen[i].x, 0.55, 0.28);
                }
                self.endermen[i].attack_cooldown = self.scaled_hostile_cooldown(24);
            }
        }
        for idx in dead_endermen.into_iter().rev() {
            let e = self.endermen.swap_remove(idx);
            if rng.gen_bool(0.55) {
                self.item_entities
                    .push(ItemEntity::new(e.x, e.y - 0.5, ItemType::EnderPearl));
            }
            if self.mob_has_player_kill_credit(e.last_player_damage_tick) {
                self.spawn_experience_orbs(e.x, e.y - 0.5, 5, &mut rng);
            }
        }

        for i in (0..self.fireballs.len()).rev() {
            self.select_nearest_peer(self.fireballs[i].x, self.fireballs[i].y);
            let previous = (self.fireballs[i].x, self.fireballs[i].y);
            self.fireballs[i].update();
            let (mut fx, mut fy, fvx, fvy, mut remove_now) = {
                let fb = &self.fireballs[i];
                (fb.x, fb.y, fb.vx, fb.vy, fb.dead)
            };

            let mut explode = false;
            let terrain = self.terrain_segment_hit(previous.0, previous.1, fx, fy, false);
            let delta = (fx - previous.0, fy - previous.1);
            let length2 = delta.0 * delta.0 + delta.1 * delta.1;
            let terrain_t = terrain.map_or(f64::INFINITY, |(x, y)| {
                if length2 > 0.0 {
                    ((x - previous.0) * delta.0 + (y - previous.1) * delta.1) / length2
                } else {
                    0.0
                }
            });
            let reflected = self.fireballs[i].reflected;
            if reflected
                && !remove_now
                && let Some(contact) =
                    self.hit_player_projectile_segment(previous, (fx, fy), 10.0, terrain_t)
            {
                (fx, fy) = contact;
                remove_now = true;
                explode = true;
            } else if let Some(contact) = terrain {
                (fx, fy) = contact;
                remove_now = true;
                explode = true;
            }

            let pdist =
                ((fx - self.player.x).powi(2) + (fy - (self.player.y - 0.9)).powi(2)).sqrt();
            if !reflected && pdist < 0.95 {
                if self.apply_player_combat_damage(self.scaled_hostile_damage(1.2)) {
                    if !self.has_fire_resistance() {
                        self.player.burning_timer = self.player.burning_timer.max(55);
                    }
                    self.player.vx += fvx * 1.4;
                    self.player.vy += fvy * 1.1 - 0.2;
                    self.player.grounded = false;
                }
                remove_now = true;
                explode = true;
            }

            if explode {
                let cx = fx.floor() as i32;
                let cy = fy.floor() as i32;
                pending_world_explosions.push((cx, cy, 1, 2.4, 8));
            }

            if remove_now {
                self.fireballs.swap_remove(i);
            }
        }

        if !pending_world_explosions.is_empty() {
            for (cx, cy, radius, strength, chain_fuse) in pending_world_explosions {
                self.world
                    .trigger_explosion(cx, cy, radius, strength, chain_fuse);
            }
            self.apply_world_explosion_impacts();
            self.collect_world_explosion_drops();
        }

        for i in (0..self.arrows.len()).rev() {
            self.select_nearest_peer(self.arrows[i].x, self.arrows[i].y);
            let previous = (self.arrows[i].x, self.arrows[i].y);
            self.arrows[i].update();
            let ax = self.arrows[i].x;
            let ay = self.arrows[i].y;
            let avx = self.arrows[i].vx;
            let from_player = self.arrows[i].from_player;
            let arrow_damage = self.arrows[i].damage;
            let terrain = self.terrain_segment_hit(previous.0, previous.1, ax, ay, false);
            let length2 = (ax - previous.0).powi(2) + (ay - previous.1).powi(2);
            let terrain_t = terrain.map_or(f64::INFINITY, |(x, y)| {
                if length2 > 0.0 {
                    ((x - previous.0) * (ax - previous.0) + (y - previous.1) * (ay - previous.1))
                        / length2
                } else {
                    0.0
                }
            });
            let hit = if from_player {
                self.hit_owned_player_arrow_segment(
                    previous,
                    (ax, ay),
                    arrow_damage,
                    terrain_t,
                    self.arrows[i].owner,
                )
            } else {
                let bounds = [
                    self.player.x - PLAYER_HALF_WIDTH,
                    self.player.y - PLAYER_HEIGHT,
                    self.player.x + PLAYER_HALF_WIDTH,
                    self.player.y,
                ];
                if combat::segment_box_hit(previous, (ax, ay), bounds)
                    .is_some_and(|t| t < terrain_t)
                {
                    if self.apply_player_combat_damage(self.scaled_hostile_damage(arrow_damage)) {
                        self.player.vx += avx * 0.5;
                    }
                    true
                } else {
                    false
                }
            };
            if hit || terrain.is_some() {
                self.arrows.swap_remove(i);
                continue;
            }
            if self.arrows[i].dead {
                self.arrows.swap_remove(i);
            }
        }

        self.tick_ender_pearls();
        self.tick_animal_husbandry();
        let farm_goals = self.farm_animal_goals();
        let mut dead_cows = Vec::new();
        for i in 0..self.cows.len() {
            if self.cows[i].health > 0.0
                && !self.body_terrain_loaded(self.cows[i].x, self.cows[i].y, self.cows[i].vx, 0.65)
            {
                continue;
            }
            self.select_nearest_peer(self.cows[i].x, self.cows[i].y);
            if self.cows[i].hit_timer > 0 {
                self.cows[i].hit_timer -= 1;
            }
            if self.cows[i].health <= 0.0 {
                dead_cows.push(i);
                continue;
            }
            self.cows[i].update_ai();
            if let Some(&vx) = farm_goals.get(&(husbandry::AnimalKind::Cow, i)) {
                self.cows[i].vx = vx;
                self.cows[i].wander_timer = 0;
                if vx.abs() > 0.01 {
                    self.cows[i].facing_right = vx > 0.0;
                }
            }
            self.cows[i].age += 1;
            let (nx, ny, nvx, nvy, grounded, hit_wall) = self.calculate_animal_movement(
                self.cows[i].x,
                self.cows[i].y,
                self.cows[i].vx,
                self.cows[i].vy,
                self.cows[i].grounded,
                0.6 * self.cows[i].husbandry.scale(),
                1.375 * self.cows[i].husbandry.scale(),
                self.cows[i].hit_timer > 0,
            );
            self.cows[i].x = nx;
            self.cows[i].y = ny;
            self.cows[i].vx = nvx;
            if nvx.abs() > 0.01 && self.cows[i].hit_timer == 0 {
                self.cows[i].facing_right = nvx > 0.0;
            }
            self.cows[i].vy = nvy;
            self.cows[i].grounded = grounded;
            if hit_wall && self.cows[i].hit_timer == 0 {
                self.cows[i].jump();
            }
            self.cows[i].vx *= 0.5;
            if self.cows[i].vx.abs() < 0.02 {
                self.cows[i].vx = 0.0;
            }
        }
        let mut dead_sheep = Vec::new();
        for i in 0..self.sheep.len() {
            if self.sheep[i].health > 0.0
                && !self.body_terrain_loaded(
                    self.sheep[i].x,
                    self.sheep[i].y,
                    self.sheep[i].vx,
                    0.65,
                )
            {
                continue;
            }
            self.select_nearest_peer(self.sheep[i].x, self.sheep[i].y);
            if self.sheep[i].hit_timer > 0 {
                self.sheep[i].hit_timer -= 1;
            }
            if self.sheep[i].health <= 0.0 {
                dead_sheep.push(i);
                continue;
            }
            self.sheep[i].update_ai();
            if let Some(&vx) = farm_goals.get(&(husbandry::AnimalKind::Sheep, i)) {
                self.sheep[i].vx = vx;
                self.sheep[i].wander_timer = 0;
                if vx.abs() > 0.01 {
                    self.sheep[i].facing_right = vx > 0.0;
                }
            }
            self.sheep[i].age += 1;
            let (nx, ny, nvx, nvy, grounded, hit_wall) = self.calculate_animal_movement(
                self.sheep[i].x,
                self.sheep[i].y,
                self.sheep[i].vx,
                self.sheep[i].vy,
                self.sheep[i].grounded,
                0.55 * self.sheep[i].husbandry.scale(),
                1.125 * self.sheep[i].husbandry.scale(),
                self.sheep[i].hit_timer > 0,
            );
            self.sheep[i].x = nx;
            self.sheep[i].y = ny;
            self.sheep[i].vx = nvx;
            if nvx.abs() > 0.01 && self.sheep[i].hit_timer == 0 {
                self.sheep[i].facing_right = nvx > 0.0;
            }
            self.sheep[i].vy = nvy;
            self.sheep[i].grounded = grounded;
            if hit_wall && self.sheep[i].hit_timer == 0 {
                self.sheep[i].jump();
            }
            self.sheep[i].vx *= 0.5;
            if self.sheep[i].vx.abs() < 0.02 {
                self.sheep[i].vx = 0.0;
            }
        }
        let mut dead_pigs = Vec::new();
        for i in 0..self.pigs.len() {
            if self.pigs[i].health > 0.0
                && !self.body_terrain_loaded(self.pigs[i].x, self.pigs[i].y, self.pigs[i].vx, 0.65)
            {
                continue;
            }
            self.select_nearest_peer(self.pigs[i].x, self.pigs[i].y);
            if self.pigs[i].hit_timer > 0 {
                self.pigs[i].hit_timer -= 1;
            }
            if self.pigs[i].health <= 0.0 {
                dead_pigs.push(i);
                continue;
            }
            self.pigs[i].update_ai();
            if let Some(&vx) = farm_goals.get(&(husbandry::AnimalKind::Pig, i)) {
                self.pigs[i].vx = vx;
                self.pigs[i].wander_timer = 0;
                if vx.abs() > 0.01 {
                    self.pigs[i].facing_right = vx > 0.0;
                }
            }
            self.pigs[i].age += 1;
            let (nx, ny, nvx, nvy, grounded, hit_wall) = self.calculate_animal_movement(
                self.pigs[i].x,
                self.pigs[i].y,
                self.pigs[i].vx,
                self.pigs[i].vy,
                self.pigs[i].grounded,
                0.5 * self.pigs[i].husbandry.scale(),
                1.0 * self.pigs[i].husbandry.scale(),
                self.pigs[i].hit_timer > 0,
            );
            self.pigs[i].x = nx;
            self.pigs[i].y = ny;
            self.pigs[i].vx = nvx;
            if nvx.abs() > 0.01 && self.pigs[i].hit_timer == 0 {
                self.pigs[i].facing_right = nvx > 0.0;
            }
            self.pigs[i].vy = nvy;
            self.pigs[i].grounded = grounded;
            if hit_wall && self.pigs[i].hit_timer == 0 {
                self.pigs[i].jump();
            }
            self.pigs[i].vx *= 0.5;
            if self.pigs[i].vx.abs() < 0.02 {
                self.pigs[i].vx = 0.0;
            }
        }
        let mut dead_chickens = Vec::new();
        for i in 0..self.chickens.len() {
            if self.chickens[i].health > 0.0
                && !self.body_terrain_loaded(
                    self.chickens[i].x,
                    self.chickens[i].y,
                    self.chickens[i].vx,
                    0.3,
                )
            {
                continue;
            }
            self.select_nearest_peer(self.chickens[i].x, self.chickens[i].y);
            if self.chickens[i].hit_timer > 0 {
                self.chickens[i].hit_timer -= 1;
            }
            if self.chickens[i].health <= 0.0 {
                dead_chickens.push(i);
                continue;
            }
            self.chickens[i].update_ai();
            if let Some(&vx) = farm_goals.get(&(husbandry::AnimalKind::Chicken, i)) {
                self.chickens[i].vx = vx;
                self.chickens[i].wander_timer = 0;
                if vx.abs() > 0.01 {
                    self.chickens[i].facing_right = vx > 0.0;
                }
            }
            self.chickens[i].age += 1;
            if self.chickens[i].egg_lay_timer == 0 && self.chickens[i].husbandry.child_ticks == 0 {
                self.item_entities.push(ItemEntity::new(
                    self.chickens[i].x,
                    self.chickens[i].y - 0.3,
                    ItemType::Egg,
                ));
                self.chickens[i].egg_lay_timer = rng.gen_range(6000..12000);
            }
            if self.chickens[i].vy > 0.0 {
                self.chickens[i].vy = (self.chickens[i].vy - 0.028).clamp(0.0, 0.47);
            }
            let (nx, ny, nvx, nvy, grounded, hit_wall) = self.calculate_animal_movement(
                self.chickens[i].x,
                self.chickens[i].y,
                self.chickens[i].vx,
                self.chickens[i].vy,
                self.chickens[i].grounded,
                0.3 * self.chickens[i].husbandry.scale(),
                1.0 * self.chickens[i].husbandry.scale(),
                self.chickens[i].hit_timer > 0,
            );
            self.chickens[i].x = nx;
            self.chickens[i].y = ny;
            self.chickens[i].vx = nvx;
            if nvx.abs() > 0.01 && self.chickens[i].hit_timer == 0 {
                self.chickens[i].facing_right = nvx > 0.0;
            }
            self.chickens[i].vy = nvy;
            self.chickens[i].grounded = grounded;
            if hit_wall && self.chickens[i].hit_timer == 0 {
                self.chickens[i].jump();
            }
            self.chickens[i].vx *= 0.56;
            if self.chickens[i].vx.abs() < 0.02 {
                self.chickens[i].vx = 0.0;
            }
        }
        let mut dead_squids = Vec::new();
        for i in 0..self.squids.len() {
            if self.squids[i].health > 0.0
                && !self.body_terrain_loaded(
                    self.squids[i].x,
                    self.squids[i].y,
                    self.squids[i].vx,
                    0.6,
                )
            {
                continue;
            }
            self.select_nearest_peer(self.squids[i].x, self.squids[i].y);
            if self.squids[i].hit_timer > 0 {
                self.squids[i].hit_timer -= 1;
            }
            if self.squids[i].health <= 0.0 {
                dead_squids.push(i);
                continue;
            }

            self.squids[i].age += 1;
            let sx = self.squids[i].x.floor() as i32;
            let center_y = (self.squids[i].y - 0.45).floor() as i32;
            let feet_y = self.squids[i].y.floor() as i32;
            let in_water = matches!(self.world.get_block(sx, center_y), BlockType::Water(_))
                || matches!(self.world.get_block(sx, feet_y), BlockType::Water(_));
            self.squids[i].update_ai(in_water);
            let (nx, ny, mut nvx, mut nvy, ngr, hit_wall) = self.calculate_mob_movement(
                self.squids[i].x,
                self.squids[i].y,
                self.squids[i].vx,
                self.squids[i].vy,
                self.squids[i].grounded,
                0.42,
                0.9,
                false,
            );
            if hit_wall {
                nvx *= -0.3;
                self.squids[i].swim_dir_x = -self.squids[i].swim_dir_x;
            }
            if in_water {
                nvx *= 0.94;
                nvy *= 0.9;
                if nvy.abs() < 0.005 {
                    nvy = 0.0;
                }
            } else {
                nvx *= 0.78;
            }
            if nvx.abs() < 0.01 {
                nvx = 0.0;
            }

            self.squids[i].x = nx;
            self.squids[i].y = ny;
            self.squids[i].vx = nvx;
            self.squids[i].vy = nvy;
            self.squids[i].grounded = ngr;
        }

        let mut dead_wolves = Vec::new();
        for i in 0..self.wolves.len() {
            if self.wolves[i].health > 0.0
                && !self.body_terrain_loaded(
                    self.wolves[i].x,
                    self.wolves[i].y,
                    self.wolves[i].vx,
                    0.6,
                )
            {
                continue;
            }
            self.select_nearest_peer(self.wolves[i].x, self.wolves[i].y);
            if self.wolves[i].hit_timer > 0 {
                self.wolves[i].hit_timer -= 1;
            }
            if self.wolves[i].health <= 0.0 {
                dead_wolves.push(i);
                continue;
            }

            let mut sheep_target = None;
            let mut sheep_target_dist = f64::INFINITY;
            for sheep in &self.sheep {
                if sheep.health <= 0.0 {
                    continue;
                }
                let dx = sheep.x - self.wolves[i].x;
                let dy = sheep.y - self.wolves[i].y;
                let dist = (dx * dx + dy * dy).sqrt();
                if dist < sheep_target_dist
                    && dist <= 14.0
                    && self.has_line_of_sight(
                        self.wolves[i].x,
                        self.wolves[i].y - 0.6,
                        sheep.x,
                        sheep.y - 0.6,
                    )
                {
                    sheep_target_dist = dist;
                    sheep_target = Some((sheep.x, sheep.y));
                }
            }

            let (tx, ty, _) = self.hostile_target(self.wolves[i].x, self.wolves[i].y, 0.6);
            self.wolves[i].update_ai(tx, ty, sheep_target);
            self.wolves[i].age += 1;
            let chase_jump = self.should_ground_mob_chase_jump(
                self.wolves[i].x,
                self.wolves[i].y,
                self.wolves[i].vx,
                self.wolves[i].grounded,
                0.6,
                0.875,
            );
            let recovery_jump = self.should_ground_mob_vertical_recovery_jump(
                self.wolves[i].x,
                self.wolves[i].y,
                self.wolves[i].vx,
                self.wolves[i].grounded,
                0.6,
                0.875,
                0,
                0,
            );
            if (chase_jump || recovery_jump)
                && self.wolves[i].hit_timer == 0
                && self.wolves[i].attack_windup == 0
            {
                self.wolves[i].jump();
            }
            let (nx, ny, nvx, nvy, ngr, hit_wall) = self.calculate_animal_movement(
                self.wolves[i].x,
                self.wolves[i].y,
                self.wolves[i].vx,
                self.wolves[i].vy,
                self.wolves[i].grounded,
                0.6,
                0.875,
                self.wolves[i].hit_timer > 0 || self.wolves[i].attack_windup > 0,
            );
            self.wolves[i].x = nx;
            self.wolves[i].y = ny;
            self.wolves[i].vx = nvx;
            if nvx.abs() > 0.01 && self.wolves[i].hit_timer == 0 {
                self.wolves[i].facing_right = nvx > 0.0;
            }
            self.wolves[i].vy = nvy;
            self.wolves[i].grounded = ngr;
            if hit_wall && self.wolves[i].hit_timer == 0 && self.wolves[i].attack_windup == 0 {
                self.wolves[i].jump();
            }
            self.wolves[i].vx *= 0.56;
            if self.wolves[i].vx.abs() < 0.02 {
                self.wolves[i].vx = 0.0;
            }

            if self.wolves[i].is_aggressive() {
                if self.wolves[i].attack_windup == 0 && self.wolves[i].attack_cooldown == 0 {
                    self.wolves[i].facing_right = self.player.x >= self.wolves[i].x;
                }
                let can_melee_hit = self.can_melee_contact_player(
                    self.wolves[i].x,
                    self.wolves[i].y - 0.6,
                    1.1,
                    0.9,
                );
                let can_melee_hit = can_melee_hit
                    && (self.player.x - self.wolves[i].x)
                        * (if self.wolves[i].facing_right {
                            1.0
                        } else {
                            -1.0
                        })
                        >= -0.15;
                let (cooldown, hurt) =
                    (self.wolves[i].attack_cooldown, self.wolves[i].hit_timer > 0);
                if combat::melee_strike_ready(
                    &mut self.wolves[i].attack_windup,
                    cooldown,
                    can_melee_hit,
                    hurt,
                ) {
                    if self.apply_player_combat_damage(self.scaled_hostile_damage(0.45)) {
                        self.apply_player_contact_knockback(self.wolves[i].x, 0.3, 0.22);
                    }
                    self.wolves[i].attack_cooldown = self.scaled_hostile_cooldown(20);
                }
            } else if self.wolves[i].attack_cooldown == 0 {
                let mut target_sheep_idx = None;
                let mut target_sheep_dist = f64::INFINITY;
                for (sheep_idx, sheep) in self.sheep.iter().enumerate() {
                    if sheep.health <= 0.0 {
                        continue;
                    }
                    let dx = sheep.x - self.wolves[i].x;
                    let dy = sheep.y - self.wolves[i].y;
                    let dist = (dx * dx + dy * dy).sqrt();
                    if dist < target_sheep_dist && dist < 1.0 {
                        target_sheep_dist = dist;
                        target_sheep_idx = Some(sheep_idx);
                    }
                }
                if let Some(sheep_idx) = target_sheep_idx {
                    self.sheep[sheep_idx].health -= 2.0;
                    self.sheep[sheep_idx].hit_timer = self.sheep[sheep_idx].hit_timer.max(8);
                    self.sheep[sheep_idx].vx += if self.sheep[sheep_idx].x > self.wolves[i].x {
                        0.26
                    } else {
                        -0.26
                    };
                    self.sheep[sheep_idx].vy = -0.22;
                    self.sheep[sheep_idx].grounded = false;
                    self.wolves[i].attack_cooldown = 24;
                }
            }
        }

        let mut dead_ocelots = Vec::new();
        for i in 0..self.ocelots.len() {
            if self.ocelots[i].health > 0.0
                && !self.body_terrain_loaded(
                    self.ocelots[i].x,
                    self.ocelots[i].y,
                    self.ocelots[i].vx,
                    0.6,
                )
            {
                continue;
            }
            self.select_nearest_peer(self.ocelots[i].x, self.ocelots[i].y);
            if self.ocelots[i].hit_timer > 0 {
                self.ocelots[i].hit_timer -= 1;
            }
            if self.ocelots[i].health <= 0.0 {
                dead_ocelots.push(i);
                continue;
            }

            let mut chicken_target = None;
            let mut chicken_target_dist = f64::INFINITY;
            for chicken in &self.chickens {
                if chicken.health <= 0.0 {
                    continue;
                }
                let dx = chicken.x - self.ocelots[i].x;
                let dy = chicken.y - self.ocelots[i].y;
                let dist = (dx * dx + dy * dy).sqrt();
                if dist < chicken_target_dist && dist <= 13.0 {
                    chicken_target_dist = dist;
                    chicken_target = Some((chicken.x, chicken.y));
                }
            }

            self.ocelots[i].update_ai(self.player.x, self.player.y, chicken_target);
            self.ocelots[i].age += 1;
            let chase_jump = self.should_ground_mob_chase_jump(
                self.ocelots[i].x,
                self.ocelots[i].y,
                self.ocelots[i].vx,
                self.ocelots[i].grounded,
                0.6,
                0.75,
            );
            let recovery_jump = self.should_ground_mob_vertical_recovery_jump(
                self.ocelots[i].x,
                self.ocelots[i].y,
                self.ocelots[i].vx,
                self.ocelots[i].grounded,
                0.6,
                0.75,
                0,
                0,
            );
            if (chase_jump || recovery_jump) && self.ocelots[i].hit_timer == 0 {
                self.ocelots[i].jump();
            }
            let (nx, ny, nvx, nvy, ngr, hit_wall) = self.calculate_animal_movement(
                self.ocelots[i].x,
                self.ocelots[i].y,
                self.ocelots[i].vx,
                self.ocelots[i].vy,
                self.ocelots[i].grounded,
                0.6,
                0.75,
                self.ocelots[i].hit_timer > 0,
            );
            self.ocelots[i].x = nx;
            self.ocelots[i].y = ny;
            self.ocelots[i].vx = nvx;
            if nvx.abs() > 0.01 && self.ocelots[i].hit_timer == 0 {
                self.ocelots[i].facing_right = nvx > 0.0;
            }
            self.ocelots[i].vy = nvy;
            self.ocelots[i].grounded = ngr;
            if hit_wall && self.ocelots[i].hit_timer == 0 {
                self.ocelots[i].jump();
            }
            self.ocelots[i].vx *= 0.58;
            if self.ocelots[i].vx.abs() < 0.02 {
                self.ocelots[i].vx = 0.0;
            }

            if self.ocelots[i].panic_timer == 0 && self.ocelots[i].attack_cooldown == 0 {
                let mut target_chicken_idx = None;
                let mut target_chicken_dist = f64::INFINITY;
                for (chicken_idx, chicken) in self.chickens.iter().enumerate() {
                    if chicken.health <= 0.0 {
                        continue;
                    }
                    let dx = chicken.x - self.ocelots[i].x;
                    let dy = chicken.y - self.ocelots[i].y;
                    let dist = (dx * dx + dy * dy).sqrt();
                    if dist < target_chicken_dist && dist < 0.95 {
                        target_chicken_dist = dist;
                        target_chicken_idx = Some(chicken_idx);
                    }
                }
                if let Some(chicken_idx) = target_chicken_idx {
                    self.chickens[chicken_idx].health -= 2.0;
                    self.chickens[chicken_idx].hit_timer =
                        self.chickens[chicken_idx].hit_timer.max(8);
                    self.chickens[chicken_idx].vx +=
                        if self.chickens[chicken_idx].x > self.ocelots[i].x {
                            0.24
                        } else {
                            -0.24
                        };
                    self.chickens[chicken_idx].vy = -0.2;
                    self.chickens[chicken_idx].grounded = false;
                    self.ocelots[i].attack_cooldown = 20;
                }
            }
        }

        let villager_snapshot: Vec<(f64, i32, i32)> = self
            .villagers
            .iter()
            .map(|v| (v.x, v.home_x, v.home_y))
            .collect();
        let mut dead_villagers = Vec::new();
        for i in 0..self.villagers.len() {
            // Missing chunk data is not empty space. Persistent villagers must
            // sleep while their complete current/predicted body is unloaded.
            // Older builds let them fall below the world in evicted chunks;
            // return that finite, recognizable state home without losing the NPC.
            let v = &mut self.villagers[i];
            if v.y.is_finite()
                && v.y > CHUNK_HEIGHT as f64 + 2.0
                && v.x.is_finite()
                && v.x.abs() <= 1_000_000.0
                && v.home_x.abs_diff(0) <= 1_000_000
                && (0..128).contains(&v.home_y)
            {
                v.x = v.home_x as f64 + 0.5;
                v.y = v.home_y as f64 + 1.0;
                v.vx = 0.0;
                v.vy = 0.0;
                v.grounded = false;
            }
            let left = v.x.min(v.x + v.vx) - 0.31;
            let right = v.x.max(v.x + v.vx) + 0.31;
            if !self.world.is_chunk_loaded_at(left.floor() as i32)
                || !self.world.is_chunk_loaded_at(right.floor() as i32)
            {
                continue;
            }
            self.select_nearest_peer(self.villagers[i].x, self.villagers[i].y);
            if self.villagers[i].hit_timer > 0 {
                self.villagers[i].hit_timer -= 1;
            }
            if self.villagers[i].health <= 0.0 {
                dead_villagers.push(i);
                continue;
            }
            let seek_shelter = self.should_villager_seek_shelter(
                self.villagers[i].x,
                self.villagers[i].y,
                is_day_now,
            );
            let shelter_target = if seek_shelter {
                self.villager_shelter_target(i)
            } else {
                None
            };
            let sheltered_now =
                !self.is_exposed_to_sky(self.villagers[i].x, self.villagers[i].y - 1.0);
            let outdoor_target = if !seek_shelter {
                self.find_villager_outdoor_surface_near_home(
                    self.villagers[i].home_x,
                    self.villagers[i].home_y,
                )
            } else {
                None
            };
            self.villagers[i].update_ai(!seek_shelter);
            if self.villagers[i].hit_timer == 0 {
                if let Some((target_x, _target_y)) = shelter_target {
                    let dx = target_x as f64 + 0.5 - self.villagers[i].x;
                    if sheltered_now && dx.abs() <= 0.2 {
                        self.villagers[i].vx = 0.0;
                        self.villagers[i].wander_timer = 0;
                    } else if dx.abs() > 0.2 {
                        self.villagers[i].walk(dx.signum() * 0.15);
                        self.villagers[i].wander_timer = 0;
                    } else {
                        self.villagers[i].vx = 0.0;
                    }
                } else if let Some((target_x, _target_y)) = outdoor_target.filter(|_| sheltered_now)
                {
                    let dx = target_x as f64 + 0.5 - self.villagers[i].x;
                    if dx.abs() > 0.45 {
                        self.villagers[i].walk(dx.signum() * 0.13);
                        self.villagers[i].wander_timer = 0;
                    } else {
                        self.villagers[i].vx = 0.0;
                    }
                } else if seek_shelter {
                    let home_target_x = self.villagers[i].home_x as f64 + 0.5;
                    let dx = home_target_x - self.villagers[i].x;
                    if dx.abs() > 0.55 {
                        self.villagers[i].walk(dx.signum() * 0.14);
                        self.villagers[i].wander_timer = 0;
                    } else {
                        self.villagers[i].vx = 0.0;
                        self.villagers[i].wander_timer = 0;
                    }
                } else {
                    let home_x = self.villagers[i].home_x;
                    let home_y = self.villagers[i].home_y;
                    let home_target_x =
                        outdoor_target.map_or(home_x as f64 + 0.5, |(x, _)| x as f64 + 0.5);
                    let mut group_sum_x = 0.0;
                    let mut group_count = 0usize;
                    for (j, (other_x, other_home_x, other_home_y)) in
                        villager_snapshot.iter().enumerate()
                    {
                        if i == j {
                            continue;
                        }
                        if (*other_home_x - home_x).abs() <= 10
                            && (*other_home_y - home_y).abs() <= 6
                            && (*other_x - self.villagers[i].x).abs() <= 18.0
                        {
                            group_sum_x += *other_x;
                            group_count += 1;
                        }
                    }
                    let group_target_x = if group_count > 0 {
                        group_sum_x / group_count as f64
                    } else {
                        home_target_x
                    };
                    let day_target_x = (home_target_x * 0.5 + group_target_x * 0.5)
                        .clamp(home_target_x - 8.0, home_target_x + 8.0);
                    let dx = day_target_x - self.villagers[i].x;
                    let home_band_dx = (self.villagers[i].x - home_target_x).abs();
                    if home_band_dx > 10.0 || dx.abs() > 4.0 {
                        self.villagers[i].walk(dx.signum() * 0.11);
                        self.villagers[i].wander_timer = 0;
                    } else if dx.abs() > 1.4 && self.villagers[i].vx.abs() < 0.10 {
                        self.villagers[i].walk(dx.signum() * 0.09);
                    }
                }
            }
            let prefer_straight_path =
                shelter_target.is_some() || outdoor_target.is_some() || seek_shelter;
            if !seek_shelter && !sheltered_now && self.villagers[i].hit_timer == 0 {
                let v = &self.villagers[i];
                let nearest = self
                    .villagers
                    .iter()
                    .enumerate()
                    .filter(|&(j, other)| {
                        j != i
                            && other.health > 0.0
                            && (other.y - v.y).abs() < 0.6
                            && (other.x - v.x).abs() < 0.85
                    })
                    .min_by(|a, b| (a.1.x - v.x).abs().total_cmp(&(b.1.x - v.x).abs()));
                if let Some((j, other)) = nearest {
                    let dx = v.x - other.x;
                    let away = if dx.abs() < 0.01 {
                        if i < j { -1.0 } else { 1.0 }
                    } else {
                        dx.signum()
                    };
                    self.villagers[i].walk(away * (0.85 - dx.abs()).mul_add(0.14, 0.025));
                }
            }
            // Keep followers from piling into the leader at a narrow doorway.
            if self.villagers[i].hit_timer == 0 && self.villagers[i].vx.abs() > 0.01 {
                let v = &self.villagers[i];
                let direction = v.vx.signum();
                if self.villagers.iter().enumerate().any(|(j, other)| {
                    j != i
                        && other.health > 0.0
                        && (other.y - v.y).abs() < 0.6
                        && (other.x - v.x) * direction > 0.05
                        && (other.x - v.x) * direction < 0.75
                        && other.vx * direction >= 0.0
                }) {
                    self.villagers[i].vx = 0.0;
                }
            }
            self.villagers[i].age += 1;
            let mut movement = self.calculate_mob_movement(
                self.villagers[i].x,
                self.villagers[i].y,
                self.villagers[i].vx,
                self.villagers[i].vy,
                self.villagers[i].grounded,
                0.3,
                1.8,
                true,
            );
            if movement.5
                && self.villagers[i].hit_timer == 0
                && self.try_open_wood_door_for_entity(
                    self.villagers[i].x,
                    self.villagers[i].y,
                    self.villagers[i].vx,
                )
            {
                movement = self.calculate_mob_movement(
                    self.villagers[i].x,
                    self.villagers[i].y,
                    self.villagers[i].vx,
                    self.villagers[i].vy,
                    self.villagers[i].grounded,
                    0.3,
                    1.8,
                    true,
                );
            }
            self.villagers[i].x = movement.0;
            self.villagers[i].y = movement.1;
            self.villagers[i].vx = movement.2;
            self.villagers[i].vy = movement.3;
            self.villagers[i].grounded = movement.4;
            self.refresh_villager_door_hold_for_entity(self.villagers[i].x, self.villagers[i].y);
            self.try_close_wood_door_behind_entity(self.villagers[i].x, self.villagers[i].y);
            if movement.5 && self.villagers[i].hit_timer == 0 {
                self.villagers[i].jump();
                if !prefer_straight_path {
                    self.villagers[i].facing_right = !self.villagers[i].facing_right;
                }
            }
            self.villagers[i].vx *= 0.52;
            if self.villagers[i].vx.abs() < 0.02 {
                self.villagers[i].vx = 0.0;
            }
        }
        for idx in dead_villagers.into_iter().rev() {
            self.villagers.swap_remove(idx);
        }
        self.tick_villager_door_hold_timers();

        let mut resolved_item_indices: Vec<(usize, bool, Option<u16>)> = Vec::new();
        for i in 0..self.item_entities.len() {
            if let Some(y) =
                self.recover_loose_void_fall(self.item_entities[i].x, self.item_entities[i].y)
            {
                self.item_entities[i].y = y;
                self.item_entities[i].vx = 0.0;
                self.item_entities[i].vy = 0.0;
            }
            let e = &self.item_entities[i];
            if !self.body_terrain_loaded(e.x, e.y, e.vx, 0.15) {
                continue;
            }
            self.select_item_collector(
                self.item_entities[i].x,
                self.item_entities[i].y,
                self.item_entities[i].item_type,
            );
            let (ix, iy, ivx, ivy) = {
                let item = &self.item_entities[i];
                (item.x, item.y, item.vx, item.vy)
            };
            self.item_entities[i].age = self.item_entities[i].age.saturating_add(1);
            let pickup_ready = self.item_entities[i].pickup_delay == 0;
            self.item_entities[i].pickup_delay =
                self.item_entities[i].pickup_delay.saturating_sub(1);
            if iy > 160.0
                || self.item_entities[i].age > ITEM_ENTITY_DESPAWN_TICKS
                || matches!(self.world.fluid_at_point(ix, iy), BlockType::Lava(_))
                || matches!(
                    self.world.get_block(ix.floor() as i32, iy.floor() as i32),
                    BlockType::Fire(_)
                )
            {
                resolved_item_indices.push((i, false, self.active_peer));
                continue;
            }
            let mut next_ivy = ivy + 0.05;
            if next_ivy > 0.5 {
                next_ivy = 0.5;
            }
            let dx = self.player.x - ix;
            let dy = (self.player.y - 1.0) - iy;
            let dist = (dx * dx + dy * dy).sqrt();
            let (mut f_vx, mut f_vy) = (ivx, next_ivy);
            let can_collect = pickup_ready
                && !self.death_screen_active
                && self.player.health > 0.0
                && self.inventory.can_accept(self.item_entities[i].item_type);
            if can_collect && dist < 3.0 && dist > 0.001 {
                f_vx += (dx / dist) * 0.15;
                f_vy += (dy / dist) * 0.15;
            }
            if can_collect && dist < 0.8 {
                resolved_item_indices.push((i, true, self.active_peer));
                continue;
            }
            let next_ix = ix + f_vx;
            let next_iy = iy + f_vy;
            if !self.is_colliding(next_ix, iy, CollisionType::Horizontal) {
                self.item_entities[i].x = next_ix;
            } else {
                f_vx = 0.0;
            }
            if f_vy > 0.0 {
                if self.is_colliding(
                    self.item_entities[i].x,
                    next_iy,
                    CollisionType::VerticalDown(iy),
                ) {
                    self.item_entities[i].y = self
                        .collision_bounds_at_point(self.item_entities[i].x, next_iy)
                        .map_or(next_iy.floor(), |b| b[1]);
                    f_vy = 0.0;
                    self.item_entities[i].grounded = true;
                } else {
                    self.item_entities[i].y = next_iy;
                    self.item_entities[i].grounded = false;
                }
            } else {
                self.item_entities[i].y = next_iy;
            }
            self.item_entities[i].vx = f_vx * 0.9;
            self.item_entities[i].vy = f_vy;
        }
        for (index, picked_up, owner) in resolved_item_indices.into_iter().rev() {
            if let Some(id) = owner {
                self.activate_peer(id);
            }
            let mut item_ent = self.item_entities.swap_remove(index);
            if picked_up && !self.collect_loose_item(&mut item_ent) {
                self.item_entities.push(item_ent);
            }
        }

        let mut resolved_orb_indices = Vec::new();
        for i in 0..self.experience_orbs.len() {
            if let Some(y) =
                self.recover_loose_void_fall(self.experience_orbs[i].x, self.experience_orbs[i].y)
            {
                self.experience_orbs[i].y = y;
                self.experience_orbs[i].vx = 0.0;
                self.experience_orbs[i].vy = 0.0;
            }
            let e = &self.experience_orbs[i];
            if !self.body_terrain_loaded(e.x, e.y, e.vx, 0.15) {
                continue;
            }
            self.select_nearest_peer(self.experience_orbs[i].x, self.experience_orbs[i].y);
            let (ox, oy, ovx, ovy) = {
                let orb = &self.experience_orbs[i];
                (orb.x, orb.y, orb.vx, orb.vy)
            };
            self.experience_orbs[i].age = self.experience_orbs[i].age.saturating_add(1);
            if oy > 160.0 || self.experience_orbs[i].age > 6000 {
                resolved_orb_indices.push((i, false, self.active_peer));
                continue;
            }

            let dx = self.player.x - ox;
            let dy = (self.player.y - 1.0) - oy;
            let dist = (dx * dx + dy * dy).sqrt();
            if dist < 0.72 && !self.death_screen_active && self.player.health > 0.0 {
                resolved_orb_indices.push((i, true, self.active_peer));
                continue;
            }

            let mut next_vx = ovx;
            let mut next_vy = (ovy + 0.03).min(0.4);
            if dist < 6.0 && dist > 0.001 {
                let pull = 0.05 + (1.0 - dist / 6.0).max(0.0) * 0.04;
                next_vx += (dx / dist) * pull;
                next_vy += (dy / dist) * pull;
            }

            let next_x = ox + next_vx;
            if !self.is_colliding(next_x, oy, CollisionType::Horizontal) {
                self.experience_orbs[i].x = next_x;
            } else {
                next_vx = 0.0;
            }

            let next_y = oy + next_vy;
            if next_vy > 0.0 {
                if self.is_colliding(
                    self.experience_orbs[i].x,
                    next_y,
                    CollisionType::VerticalDown(oy),
                ) {
                    self.experience_orbs[i].y = self
                        .collision_bounds_at_point(self.experience_orbs[i].x, next_y)
                        .map_or(next_y.floor(), |b| b[1]);
                    next_vy = -next_vy * 0.28;
                    self.experience_orbs[i].grounded = true;
                    if next_vy.abs() < 0.02 {
                        next_vy = 0.0;
                    }
                } else {
                    self.experience_orbs[i].y = next_y;
                    self.experience_orbs[i].grounded = false;
                }
            } else {
                self.experience_orbs[i].y = next_y;
            }

            self.experience_orbs[i].vx = next_vx * 0.9;
            self.experience_orbs[i].vy = next_vy;
        }
        for (index, grant_xp, owner) in resolved_orb_indices.into_iter().rev() {
            if let Some(id) = owner {
                self.activate_peer(id);
            }
            let orb = self.experience_orbs.swap_remove(index);
            if grant_xp {
                self.add_experience(orb.value);
            }
        }

        for idx in dead_cows.into_iter().rev() {
            let cow = self.cows.swap_remove(idx);
            if cow.husbandry.child_ticks > 0 {
                continue;
            }
            for _ in 0..rng.gen_range(1..=3) {
                self.item_entities
                    .push(ItemEntity::new(cow.x, cow.y - 0.5, ItemType::RawBeef));
            }
            for _ in 0..rng.gen_range(0..=2) {
                self.item_entities
                    .push(ItemEntity::new(cow.x, cow.y - 0.5, ItemType::Leather));
            }
            if self.mob_has_player_kill_credit(cow.last_player_damage_tick) {
                self.spawn_experience_orbs(cow.x, cow.y - 0.5, rng.gen_range(1..=3), &mut rng);
            }
        }
        for idx in dead_sheep.into_iter().rev() {
            let sheep = self.sheep.swap_remove(idx);
            if sheep.husbandry.child_ticks > 0 {
                continue;
            }
            if !sheep.sheared {
                self.item_entities
                    .push(ItemEntity::new(sheep.x, sheep.y - 0.5, ItemType::Wool));
            }
            if self.mob_has_player_kill_credit(sheep.last_player_damage_tick) {
                self.spawn_experience_orbs(sheep.x, sheep.y - 0.5, rng.gen_range(1..=3), &mut rng);
            }
        }
        for idx in dead_pigs.into_iter().rev() {
            let pig = self.pigs.swap_remove(idx);
            if pig.husbandry.child_ticks > 0 {
                continue;
            }
            self.item_entities
                .push(ItemEntity::new(pig.x, pig.y - 0.5, ItemType::RawPorkchop));
            if rng.gen_bool(0.35) {
                self.item_entities
                    .push(ItemEntity::new(pig.x, pig.y - 0.5, ItemType::RawPorkchop));
            }
            if self.mob_has_player_kill_credit(pig.last_player_damage_tick) {
                self.spawn_experience_orbs(pig.x, pig.y - 0.5, rng.gen_range(1..=3), &mut rng);
            }
        }
        for idx in dead_chickens.into_iter().rev() {
            let chicken = self.chickens.swap_remove(idx);
            if chicken.husbandry.child_ticks > 0 {
                continue;
            }
            self.item_entities.push(ItemEntity::new(
                chicken.x,
                chicken.y - 0.35,
                ItemType::RawChicken,
            ));
            self.item_entities.push(ItemEntity::new(
                chicken.x,
                chicken.y - 0.35,
                ItemType::Feather,
            ));
            if rng.gen_bool(0.45) {
                self.item_entities.push(ItemEntity::new(
                    chicken.x,
                    chicken.y - 0.35,
                    ItemType::Feather,
                ));
            }
            if self.mob_has_player_kill_credit(chicken.last_player_damage_tick) {
                self.spawn_experience_orbs(
                    chicken.x,
                    chicken.y - 0.35,
                    rng.gen_range(1..=2),
                    &mut rng,
                );
            }
        }
        for idx in dead_squids.into_iter().rev() {
            let squid = self.squids.swap_remove(idx);
            if self.mob_has_player_kill_credit(squid.last_player_damage_tick) {
                self.spawn_experience_orbs(squid.x, squid.y - 0.35, rng.gen_range(1..=3), &mut rng);
            }
        }
        for idx in dead_wolves.into_iter().rev() {
            let wolf = self.wolves.swap_remove(idx);
            if self.mob_has_player_kill_credit(wolf.last_player_damage_tick) {
                self.spawn_experience_orbs(wolf.x, wolf.y - 0.35, rng.gen_range(1..=3), &mut rng);
            }
        }
        for idx in dead_ocelots.into_iter().rev() {
            let ocelot = self.ocelots.swap_remove(idx);
            if self.mob_has_player_kill_credit(ocelot.last_player_damage_tick) {
                self.spawn_experience_orbs(
                    ocelot.x,
                    ocelot.y - 0.35,
                    rng.gen_range(1..=3),
                    &mut rng,
                );
            }
        }

        self.trim_excess_loose_entities();

        if self.player.health <= 0.0 && !self.death_screen_active {
            self.start_death_screen();
        }
    }

    fn tick_player_survival(&mut self, rng: &mut impl rand::Rng) {
        if self.eye_guidance_timer > 0 {
            if self.current_dimension == Dimension::Overworld {
                let dx = STRONGHOLD_CENTER_X as f64 + 0.5 - self.player.x;
                self.eye_guidance_dir = if dx >= 0.0 { 1 } else { -1 };
                self.eye_guidance_distance = dx.abs().round() as i32;
                self.eye_guidance_timer -= 1;
            } else {
                self.eye_guidance_timer = 0;
            }
        }
        self.player.age += 1;
        let fire_resistant = self.has_fire_resistance();
        if !fire_resistant
            && [
                self.player.y - 0.1,
                self.player.y - 0.8,
                self.player.y - 1.5,
            ]
            .iter()
            .any(|y| {
                matches!(
                    self.world
                        .get_block(self.player.x.floor() as i32, y.floor() as i32),
                    BlockType::Fire(_)
                )
            })
        {
            self.player.burning_timer = self.player.burning_timer.max(80);
        }
        if self.potion_strength_timer > 0 {
            self.potion_strength_timer -= 1;
        }
        if self.potion_regeneration_timer > 0 {
            if self
                .player
                .age
                .is_multiple_of(POTION_REGEN_HEAL_INTERVAL_TICKS as u64)
            {
                self.player.health += 1.0;
            }
            self.potion_regeneration_timer -= 1;
        }
        if self.potion_fire_resistance_timer > 0 {
            self.potion_fire_resistance_timer -= 1;
        }
        self.update_fishing_state(rng);

        let head_block = self
            .world
            .fluid_at_point(self.player.x, self.player.y - 1.5);
        let (water_contact, lava_contact) = self.entity_fluid_submersion(
            self.player.x,
            self.player.y,
            PLAYER_HALF_WIDTH,
            PLAYER_HEIGHT,
        );

        if matches!(head_block, BlockType::Water(_)) {
            self.player.drowning_timer -= 1;
            if self.player.drowning_timer <= -20 {
                self.apply_player_environment_damage(2.0);
                self.player.drowning_timer = 0;
            }
        } else {
            self.player.drowning_timer = (self.player.drowning_timer.max(0) + 4).min(300);
        }

        if lava_contact > 0.0 {
            if fire_resistant {
                self.player.burning_timer = 0;
            } else {
                self.player.burning_timer = 100;
                if self.player.age.is_multiple_of(10) {
                    self.apply_player_damage(4.0);
                }
            }
        }

        if self.player.burning_timer > 0 {
            if fire_resistant
                || ((water_contact > 0.0
                    || self.is_weather_wet_at(self.player.x, self.player.y - 1.5))
                    && lava_contact == 0.0)
            {
                self.player.burning_timer = 0;
            } else {
                self.player.burning_timer -= 1;
                if self.player.burning_timer % 20 == 0 {
                    self.apply_player_damage(1.0);
                }
            }
        }

        if self.player_touches_cactus() && self.player_combat_hurt_cooldown == 0 {
            self.apply_player_damage(1.0);
        }
    }
    /// Reuses authoritative movement/collision rules without simulating the world,
    /// mobs, random survival effects or player attacks on the client.
    pub(crate) fn predict_motion_tick(&mut self) {
        if self.is_showing_death_screen()
            || self.is_showing_credits()
            || self.mounted_boat.is_some()
        {
            return;
        }
        self.left_click_down = false;
        self.tick_player_actions(100_000, 0);
    }

    pub(crate) fn copy_motion_preview_into(&self, preview: &mut Self) {
        PlayerSession::capture(self).apply(preview);
        preview.difficulty = self.difficulty;
        preview.current_dimension = self.current_dimension;
    }

    /// Clip a small visual reconciliation against the same actor/terrain shapes.
    pub(crate) fn resolve_motion_presentation(&self, x: f64, y: f64) -> (f64, f64) {
        let (x, y, ..) = self.resolve_entity_overlap(
            self.player.x,
            self.player.y,
            x,
            y,
            0.0,
            0.0,
            PLAYER_HALF_WIDTH,
            PLAYER_HEIGHT,
            self.player.grounded,
        );
        (x, y)
    }

    fn tick_player_actions(&mut self, mouse_target_x: i32, mouse_target_y: i32) {
        self.world.activate_redstone_ore(
            self.player.x.floor() as i32,
            (self.player.y + 0.02).floor() as i32,
        );
        let (water_submersion, lava_submersion) = self.entity_fluid_submersion(
            self.player.x,
            self.player.y,
            PLAYER_HALF_WIDTH,
            PLAYER_HEIGHT,
        );
        if self.difficulty == Difficulty::Peaceful {
            self.player.hunger = self.player.max_hunger;
            if self.player.health < self.player.max_health && self.player_combat_hurt_cooldown == 0
            {
                self.player.health += 0.05;
            }
        } else {
            if self.player.hunger > 0.0 {
                self.player.hunger -= 0.0005;
            }
            if self.player.hunger <= 0.0 {
                let floor = self.starvation_health_floor();
                if self.player.health > floor {
                    let starvation_damage = (self.player.health - floor).min(0.01);
                    self.player.health = (self.player.health - starvation_damage).max(floor);
                }
            } else if self.player.hunger >= 18.0
                && self.player.health < self.player.max_health
                && self.player_combat_hurt_cooldown == 0
            {
                self.player.health += 0.02;
            }
        }
        self.player.health = self.player.health.clamp(0.0, self.player.max_health);
        self.player.hunger = self.player.hunger.clamp(0.0, self.player.max_hunger);

        let tuning = self.movement_tuning();
        let input_dir = if self.moving_left == self.moving_right {
            0.0
        } else if self.moving_left {
            -1.0
        } else {
            1.0
        };
        let riding_boat = self.mounted_boat.is_some();
        if self.sprint_left_tap_ticks > 0 {
            self.sprint_left_tap_ticks -= 1;
        }
        if self.sprint_right_tap_ticks > 0 {
            self.sprint_right_tap_ticks -= 1;
        }
        let is_actively_moving = !self.inventory_open && input_dir != 0.0;
        if self.player.grounded {
            self.coyote_ticks = 2;
        } else if self.coyote_ticks > 0 {
            self.coyote_ticks -= 1;
        }
        let swim_physics_active = water_submersion >= SWIM_PHYSICS_MIN_SUBMERSION
            || lava_submersion >= SWIM_PHYSICS_MIN_SUBMERSION;
        let in_fluid_for_jump = water_submersion >= SWIM_CONTROL_MIN_SUBMERSION
            || lava_submersion >= SWIM_CONTROL_MIN_SUBMERSION;
        let on_ladder = self.is_player_on_ladder();
        if self.sprinting {
            let sprint_dir_matches = (self.sprint_direction > 0 && input_dir > 0.0)
                || (self.sprint_direction < 0 && input_dir < 0.0);
            if self.inventory_open
                || self.player.sneaking
                || !sprint_dir_matches
                || self.player.hunger <= SPRINT_MIN_HUNGER
                || on_ladder
                || swim_physics_active
            {
                self.stop_sprinting();
            }
        }

        if !self.inventory_open && !riding_boat {
            if on_ladder {
                // Ladders reduce fall speed and let jump input climb upward.
                if self.player.vy > 0.12 {
                    self.player.vy = 0.12;
                }
                self.player.fall_distance = 0.0;
            }
            let mut consumed_jump_input = false;
            if self.jump_buffer_ticks > 0 {
                let can_jump =
                    on_ladder || in_fluid_for_jump || self.player.grounded || self.coyote_ticks > 0;
                if can_jump {
                    // Enable "coyote time": a jump shortly after leaving the edge still works.
                    if on_ladder {
                        self.player.vy = -0.28;
                        self.player.grounded = false;
                    } else if !in_fluid_for_jump && !self.player.grounded && self.coyote_ticks > 0 {
                        self.player.grounded = true;
                        self.player.jump(0.0, 0.0);
                    } else {
                        self.player.jump(
                            if in_fluid_for_jump {
                                water_submersion
                            } else {
                                0.0
                            },
                            if in_fluid_for_jump {
                                lava_submersion
                            } else {
                                0.0
                            },
                        );
                    }
                    if input_dir != 0.0 && !on_ladder && !in_fluid_for_jump {
                        // Preserve a minimum forward impulse so jumping from flush against
                        // a block edge still clears a one-block rise reliably.
                        let base_forward = if self.player.sneaking {
                            tuning.sneak_speed * 0.72
                        } else if self.sprinting {
                            tuning.walk_speed * SPRINT_SPEED_MULTIPLIER * 0.8
                        } else {
                            tuning.walk_speed * 0.6
                        };
                        if input_dir > 0.0 {
                            self.player.vx = self.player.vx.max(base_forward);
                        } else {
                            self.player.vx = self.player.vx.min(-base_forward);
                        }
                    }
                    self.jump_buffer_ticks = 0;
                    self.coyote_ticks = 0;
                    consumed_jump_input = true;
                } else {
                    self.jump_buffer_ticks -= 1;
                }
            }

            if on_ladder && !consumed_jump_input {
                if self.jump_held {
                    self.player.vy = -0.28;
                } else if self.player.sneaking {
                    self.player.vy = -0.03;
                }
                self.player.fall_distance = 0.0;
            }

            if self.jump_held && !consumed_jump_input && !on_ladder && in_fluid_for_jump {
                self.player.swim_up(water_submersion, lava_submersion);
                self.player.fall_distance = 0.0;
            }
            if self.player.sneaking && !consumed_jump_input && !on_ladder && swim_physics_active {
                self.player.swim_down(water_submersion, lava_submersion);
                self.player.fall_distance = 0.0;
            }

            if input_dir != 0.0 {
                let player_half_width = 0.25;
                let mut desired_speed = if self.player.sneaking {
                    tuning.sneak_speed
                } else {
                    tuning.walk_speed
                };
                if self.sprinting {
                    desired_speed *= SPRINT_SPEED_MULTIPLIER;
                }
                // Water scales the chosen walking speed; the former minimum
                // of 0.6 cancelled drag on a submerged floor. Lava retains its
                // existing escape stroke, which its stronger resistance needs.
                if !self.player.sneaking && lava_submersion > 0.0 {
                    desired_speed = desired_speed.max(0.6);
                }
                let fluid_speed_scale = if water_submersion > 0.0 {
                    1.0 - water_submersion * 0.46
                } else if lava_submersion > 0.0 {
                    0.62 - lava_submersion * 0.22
                } else {
                    1.0
                };
                desired_speed *= fluid_speed_scale;
                desired_speed *= input_dir;
                let next_x = self.player.x + desired_speed;
                let edge_probe_x = next_x + input_dir * player_half_width;
                let can_move = !self.player.sneaking
                    || self.is_colliding(
                        edge_probe_x,
                        self.player.y + 0.1,
                        CollisionType::VerticalDown(self.player.y),
                    );
                if can_move {
                    let accel = if self.player.grounded {
                        tuning.ground_accel
                    } else {
                        tuning.air_accel
                    };
                    let accel = if water_submersion >= SWIM_CONTROL_MIN_SUBMERSION {
                        // Responsive steering toward a lower top speed, rather
                        // than a high target speed approached sluggishly.
                        (accel * 1.2).clamp(0.48, 0.65)
                    } else if water_submersion > 0.0 {
                        (accel * 0.72).clamp(0.12, 0.28)
                    } else if lava_submersion > 0.0 {
                        (accel * 0.48).clamp(0.08, 0.16)
                    } else {
                        accel
                    };
                    self.player.vx += (desired_speed - self.player.vx) * accel;
                } else if self.player.grounded {
                    self.player.vx *= 0.45;
                }
                self.player.facing_right = input_dir > 0.0;
                if self.player.hunger > 0.0 {
                    let drain = if self.sprinting {
                        SPRINT_HUNGER_DRAIN_PER_TICK
                    } else {
                        WALK_HUNGER_DRAIN_PER_TICK
                    };
                    self.player.hunger -= drain;
                    if self.player.hunger <= SPRINT_MIN_HUNGER {
                        self.stop_sprinting();
                    }
                }
            }
        } else {
            self.jump_buffer_ticks = 0;
            self.stop_sprinting();
        }

        if !self.cooperative() {
            self.update_boats();
        }

        if self.player.attack_timer > 0 {
            self.player.attack_timer -= 1;
        }

        let bow_uses_primary = self.update_bow_draw_state(mouse_target_x, mouse_target_y);
        let mut hit_entity = false;
        if self.left_click_down
            && !self.inventory_open
            && self.player.attack_timer == 0
            && !bow_uses_primary
        {
            hit_entity =
                self.perform_melee(mouse_target_x as f64 + 0.5, mouse_target_y as f64 + 0.5);

            self.player.attack_timer = 10;
            self.show_action(
                super::action::ActionKind::Swing,
                self.current_hotbar_item_type(),
                mouse_target_x,
                mouse_target_y,
                10,
                0.0,
            );
            if hit_entity {
                self.player.mining_timer = 0.0;
                self.apply_inventory_slot_durability_wear(self.hotbar_index as usize);
            }
        }

        if self.left_click_down && !self.inventory_open && !hit_entity && !bow_uses_primary {
            self.interact_block(mouse_target_x, mouse_target_y, true);
        } else {
            self.player.mining_timer = 0.0;
        }

        let friction = if is_actively_moving {
            if self.player.grounded {
                tuning.ground_drag_active
            } else {
                tuning.air_drag
            }
        } else if self.player.grounded {
            tuning.ground_drag_idle
        } else {
            tuning.air_drag
        };

        if riding_boat {
            self.sync_player_to_boat();
            self.player.fall_distance = 0.0;
        } else {
            let old_y = self.player.y;
            let was_grounded = self.player.grounded;
            let was_in_fluid = self.entity_touches_fluid(
                self.player.x,
                self.player.y,
                PLAYER_HALF_WIDTH,
                PLAYER_HEIGHT,
            );

            // Ledge protection also applies to residual velocity without directional input.
            if self.player.sneaking
                && self.player.grounded
                && self.player.vy >= 0.0
                && !was_in_fluid
                && !on_ladder
            {
                let velocity = self.player.vx;
                let steps = ((velocity.abs() / 0.025).ceil() as usize).clamp(1, 128);
                let mut supported_velocity = 0.0;
                for step in 1..=steps {
                    let candidate = velocity * step as f64 / steps as f64;
                    let edge =
                        self.player.x + candidate + velocity.signum() * (PLAYER_HALF_WIDTH - 0.001);
                    if !self.is_colliding(
                        edge,
                        self.player.y + 0.05,
                        CollisionType::VerticalDown(self.player.y),
                    ) {
                        break;
                    }
                    supported_velocity = candidate;
                }
                self.player.vx = supported_velocity;
            }

            let (nx, ny, nvx, nvy, ngr, _) = self.calculate_movement(
                self.player.x,
                self.player.y,
                self.player.vx,
                self.player.vy,
                self.player.grounded,
                PLAYER_HALF_WIDTH,
                PLAYER_HEIGHT,
                true, // auto_step
            );

            let in_fluid_after_move =
                self.entity_touches_fluid(nx, ny, PLAYER_HALF_WIDTH, PLAYER_HEIGHT);

            if was_in_fluid || in_fluid_after_move {
                self.player.fall_distance = 0.0;
            } else {
                if ny > old_y {
                    self.player.fall_distance += (ny - old_y) as f32;
                }
                if ngr && !was_grounded {
                    if self.player.fall_distance > 3.0 {
                        let damage = (self.player.fall_distance - 3.0).ceil();
                        if damage > 0.0 {
                            self.apply_player_damage(damage);
                        }
                    }
                    self.player.fall_distance = 0.0;
                } else if nvy <= 0.0 {
                    self.player.fall_distance = 0.0;
                }
            }

            self.player.x = nx;
            self.player.y = ny;
            self.player.vx = nvx;
            self.player.vy = nvy;
            self.player.grounded = ngr;
            self.player.vx *= friction;
            if self.player.vx.abs() < 0.02 {
                self.player.vx = 0.0;
            }
        }
    }

    fn wood_acts_as_tree_platform_at(&self, wx: i32, wy: i32) -> bool {
        let (wood_block, leaf_block) = match self.world.get_block(wx, wy) {
            BlockType::Wood => (BlockType::Wood, BlockType::Leaves),
            BlockType::BirchWood => (BlockType::BirchWood, BlockType::BirchLeaves),
            _ => return false,
        };

        for trunk_y in (wy - 2)..=(wy + 6) {
            if self.world.get_block(wx, trunk_y) != wood_block {
                continue;
            }

            for dy in -2..=2 {
                for dx in -2..=2 {
                    if self.world.get_block(wx + dx, trunk_y + dy) == leaf_block {
                        return true;
                    }
                }
            }
        }

        false
    }

    fn collision_bounds_at(&self, wx: i32, wy: i32) -> impl Iterator<Item = [f64; 4]> {
        self.world.collision_boxes_at(wx, wy).iter().map(move |b| {
            [
                wx as f64 + b[0],
                wy as f64 + b[1],
                wx as f64 + b[2],
                wy as f64 + b[3],
            ]
        })
    }

    fn collision_bounds_at_point(&self, x: f64, y: f64) -> Option<[f64; 4]> {
        self.collision_bounds_at(x.floor() as i32, y.floor() as i32)
            .find(|b| x >= b[0] && x < b[2] && y >= b[1] && y < b[3])
    }

    fn block_has_entity_collision_at(&self, wx: i32, wy: i32) -> bool {
        let block = self.world.get_block(wx, wy);
        block.is_solid()
            && !matches!(block, BlockType::Leaves | BlockType::BirchLeaves)
            && !(matches!(block, BlockType::Wood | BlockType::BirchWood)
                && self.wood_acts_as_tree_platform_at(wx, wy))
    }

    #[allow(clippy::too_many_arguments)]
    fn resolve_entity_overlap(
        &self,
        prev_x: f64,
        prev_y: f64,
        mut x: f64,
        mut y: f64,
        mut vx: f64,
        mut vy: f64,
        half_width: f64,
        height: f64,
        mut grounded: bool,
    ) -> (f64, f64, f64, f64, bool, bool) {
        const COLLISION_EPSILON: f64 = 0.001;
        let mut hit_wall = false;

        for _ in 0..6 {
            let entity_left = x - half_width;
            let entity_right = x + half_width;
            let entity_top = y - height;
            let entity_bottom = y;
            let min_x = entity_left.floor() as i32;
            let max_x = (entity_right - COLLISION_EPSILON).floor() as i32;
            let min_y = entity_top.floor() as i32;
            let max_y = (entity_bottom - COLLISION_EPSILON).floor() as i32;

            let mut best_resolution: Option<(f64, f64, bool)> = None;
            for by in min_y..=max_y {
                for bx in min_x..=max_x {
                    if !self.block_has_entity_collision_at(bx, by) {
                        continue;
                    }

                    for [block_left, block_top, block_right, block_bottom] in
                        self.collision_bounds_at(bx, by)
                    {
                        let overlap_x = entity_right.min(block_right) - entity_left.max(block_left);
                        let overlap_y = entity_bottom.min(block_bottom) - entity_top.max(block_top);
                        if overlap_x <= COLLISION_EPSILON || overlap_y <= COLLISION_EPSILON {
                            continue;
                        }

                        let prev_left = prev_x - half_width;
                        let prev_right = prev_x + half_width;
                        let prev_top = prev_y - height;
                        let prev_bottom = prev_y;

                        let inferred_resolution =
                            if prev_bottom <= block_top + COLLISION_EPSILON && prev_y <= y {
                                Some((0.0, -(entity_bottom - block_top + COLLISION_EPSILON), true))
                            } else if prev_top >= block_bottom - COLLISION_EPSILON && prev_y >= y {
                                Some((0.0, block_bottom - entity_top + COLLISION_EPSILON, true))
                            } else if prev_right <= block_left + COLLISION_EPSILON && prev_x <= x {
                                Some((-(entity_right - block_left + COLLISION_EPSILON), 0.0, false))
                            } else if prev_left >= block_right - COLLISION_EPSILON && prev_x >= x {
                                Some((block_right - entity_left + COLLISION_EPSILON, 0.0, false))
                            } else {
                                None
                            };

                        let resolution = inferred_resolution.unwrap_or_else(|| {
                            let push_left = -(entity_right - block_left + COLLISION_EPSILON);
                            let push_right = block_right - entity_left + COLLISION_EPSILON;
                            let push_up = -(entity_bottom - block_top + COLLISION_EPSILON);
                            let push_down = block_bottom - entity_top + COLLISION_EPSILON;

                            let horizontal = if push_left.abs() <= push_right.abs() {
                                (push_left, 0.0, false)
                            } else {
                                (push_right, 0.0, false)
                            };
                            let vertical = if push_up.abs() <= push_down.abs() {
                                (0.0, push_up, true)
                            } else {
                                (0.0, push_down, true)
                            };

                            if vertical.1.abs() <= horizontal.0.abs() {
                                vertical
                            } else {
                                horizontal
                            }
                        });

                        match best_resolution {
                            Some((best_dx, best_dy, _))
                                if resolution.0.abs() + resolution.1.abs()
                                    >= best_dx.abs() + best_dy.abs() => {}
                            _ => best_resolution = Some(resolution),
                        }
                    }
                }
            }

            let Some((resolve_dx, resolve_dy, vertical)) = best_resolution else {
                break;
            };
            if vertical {
                y += resolve_dy;
                vy = 0.0;
                if resolve_dy < 0.0 {
                    grounded = true;
                }
            } else {
                x += resolve_dx;
                vx = 0.0;
                hit_wall = true;
            }
        }

        (x, y, vx, vy, grounded, hit_wall)
    }

    fn find_ground_snap_y(
        &self,
        x: f64,
        y: f64,
        half_width: f64,
        max_snap_distance: f64,
    ) -> Option<f64> {
        const COLLISION_EPSILON: f64 = 0.001;
        const SNAP_UP_TOLERANCE: f64 = 0.08;
        if max_snap_distance <= 0.0 {
            return None;
        }

        let side_probe = (half_width - COLLISION_EPSILON).max(0.0);
        let probe_xs = [x, x - side_probe, x + side_probe];
        let mut best_support_y: Option<f64> = None;

        for probe_x in probe_xs {
            let block_x = probe_x.floor() as i32;
            for block_y in y.floor() as i32..=(y + max_snap_distance).floor() as i32 {
                if !self.block_has_entity_collision_at(block_x, block_y) {
                    continue;
                }
                for bounds in self.collision_bounds_at(block_x, block_y) {
                    if probe_x < bounds[0] || probe_x >= bounds[2] {
                        continue;
                    }
                    let support_y = bounds[1];
                    let drop = support_y - y;
                    if (-SNAP_UP_TOLERANCE..=max_snap_distance).contains(&drop) {
                        best_support_y =
                            Some(best_support_y.map_or(support_y, |best_y| best_y.min(support_y)));
                    }
                }
            }
        }

        best_support_y
    }

    fn has_stable_ground_support(&self, x: f64, y: f64, half_width: f64, _vx: f64) -> bool {
        const COLLISION_EPSILON: f64 = 0.001;

        let side_probe = (half_width - COLLISION_EPSILON).max(0.0);
        let probe_xs = [x, x - side_probe, x + side_probe];
        let mut supported = [false; 3];
        for (idx, probe_x) in probe_xs.into_iter().enumerate() {
            supported[idx] = self.is_colliding(probe_x, y + 0.05, CollisionType::VerticalDown(y));
        }

        // An actor remains supported until its entire foot leaves the surface.
        // Looking only at the leading edge caused a false airborne tick on stairs.
        supported.into_iter().any(|hit| hit)
    }

    #[allow(clippy::too_many_arguments)]
    fn calculate_mob_movement(
        &self,
        x: f64,
        y: f64,
        vx: f64,
        vy: f64,
        grounded: bool,
        half_width: f64,
        height: f64,
        do_auto_step: bool,
    ) -> (f64, f64, f64, f64, bool, bool) {
        self.calculate_movement_with_jump_held(
            x,
            y,
            vx,
            vy,
            grounded,
            half_width,
            height,
            do_auto_step,
            false,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn calculate_movement(
        &self,
        x: f64,
        y: f64,
        vx: f64,
        vy: f64,
        grounded: bool,
        half_width: f64,
        height: f64,
        do_auto_step: bool,
    ) -> (f64, f64, f64, f64, bool, bool) {
        self.calculate_movement_with_jump_held(
            x,
            y,
            vx,
            vy,
            grounded,
            half_width,
            height,
            do_auto_step,
            self.jump_held,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn calculate_movement_with_jump_held(
        &self,
        mut x: f64,
        mut y: f64,
        mut vx: f64,
        mut vy: f64,
        mut grounded: bool,
        half_width: f64,
        height: f64,
        do_auto_step: bool,
        jump_held: bool,
    ) -> (f64, f64, f64, f64, bool, bool) {
        const COLLISION_EPSILON: f64 = 0.001;
        const SHORELINE_STEP_LOOKAHEAD: f64 = 0.08;
        let started_grounded = grounded;
        let (resolved_x, resolved_y, resolved_vx, resolved_vy, resolved_grounded, mut hit_wall) =
            self.resolve_entity_overlap(x, y, x, y, vx, vy, half_width, height, grounded);
        x = resolved_x;
        y = resolved_y;
        vx = resolved_vx;
        vy = resolved_vy;
        grounded = resolved_grounded;

        let center_block = self
            .world
            .get_block(x.floor() as i32, (y - height / 2.0).floor() as i32);
        let feet_block = self.world.get_block(x.floor() as i32, y.floor() as i32);
        let head_block = self
            .world
            .get_block(x.floor() as i32, (y - height + 0.1).floor() as i32);
        let (water_submersion, lava_submersion) =
            self.entity_fluid_submersion(x, y, half_width, height);
        let in_water = water_submersion >= SWIM_PHYSICS_MIN_SUBMERSION;
        let shallow_water_contact = water_submersion > 0.0 && !in_water;
        let in_lava = lava_submersion > 0.0;
        let on_ladder = matches!(center_block, BlockType::Ladder)
            || matches!(feet_block, BlockType::Ladder)
            || matches!(head_block, BlockType::Ladder);
        let on_soul_sand = matches!(feet_block, BlockType::SoulSand);
        let fluid_exit_step =
            ((in_water || shallow_water_contact) && water_submersion < 0.98 && vy < 0.18)
                || (jump_held && water_submersion >= SWIM_CONTROL_MIN_SUBMERSION && vy < 0.12)
                || (in_lava && lava_submersion < 0.98 && vy < 0.18)
                || (jump_held && lava_submersion >= SWIM_CONTROL_MIN_SUBMERSION && vy < 0.14);

        let gravity = if in_water {
            0.03 - water_submersion * 0.006
        } else if shallow_water_contact {
            0.055 - water_submersion * 0.01
        } else if in_lava {
            0.016 - lava_submersion * 0.006
        } else if on_ladder {
            0.03
        } else {
            0.08
        };
        let max_fall_speed = if in_water {
            0.2 - water_submersion * 0.06
        } else if shallow_water_contact {
            0.4 - water_submersion * 0.05
        } else if in_lava {
            0.16 - lava_submersion * 0.06
        } else if on_ladder {
            0.16
        } else {
            1.0
        };

        if in_water {
            vx *= 0.95 - water_submersion * 0.06;
            if vy < 0.0 {
                vy *= 0.84 - water_submersion * 0.06;
            } else {
                vy *= 0.96 - water_submersion * 0.03;
            }
        } else if shallow_water_contact {
            vx *= 0.70 - water_submersion * 0.1;
            if vy < 0.0 {
                vy *= 0.92;
            }
        } else if in_lava {
            vx *= 0.92 - lava_submersion * 0.1;
            vy *= 0.92 - lava_submersion * 0.12;
        }
        if on_ladder {
            vx *= 0.75;
        }
        if on_soul_sand && grounded {
            vx *= 0.55;
        }

        vy += gravity;
        if vy > max_fall_speed {
            vy = max_fall_speed;
        }

        // Probing exactly at the feet line makes tiny ground-settling errors feel
        // like horizontal wall hits against flowers and other low decor.
        let low_probe_offset = 0.04;
        let high_probe_offset = (height - 0.3).clamp(low_probe_offset, height - COLLISION_EPSILON);
        let mid_probe_offset = ((low_probe_offset + high_probe_offset) * 0.5)
            .clamp(low_probe_offset, high_probe_offset);
        let horizontal_probe_offsets = [low_probe_offset, mid_probe_offset, high_probe_offset];
        let horizontal_collides = |probe_x: f64, probe_feet_y: f64| {
            horizontal_probe_offsets.iter().any(|offset| {
                self.is_colliding(probe_x, probe_feet_y - *offset, CollisionType::Horizontal)
            })
        };
        let rising_jump_assist = (-0.55..-0.14).contains(&vy)
            && self.find_ground_snap_y(x, y, half_width, 0.35).is_some();

        let start_x = x;
        let start_y = y;
        let next_x = x + vx;
        let mut force_land_after_step = false;
        let mut step_target_x: Option<f64> = None;
        let check_x = if vx > 0.0 {
            Some(next_x + half_width - COLLISION_EPSILON)
        } else if vx < 0.0 {
            Some(next_x - half_width + COLLISION_EPSILON)
        } else {
            None
        };
        let edge_offset = if vx > 0.0 {
            half_width - COLLISION_EPSILON
        } else {
            -half_width + COLLISION_EPSILON
        };
        let sweep_steps = ((vx.abs() / 0.25).ceil() as usize).clamp(1, 256);
        let first_wall_probe = check_x.and_then(|_| {
            (1..=sweep_steps)
                .map(|step| x + vx * step as f64 / sweep_steps as f64 + edge_offset)
                .find(|&probe| horizontal_collides(probe, y))
        });
        let step_probe_x = first_wall_probe.or_else(|| {
            check_x.and_then(|probe_x| {
                if fluid_exit_step {
                    let lookahead_probe_x = probe_x + vx.signum() * SHORELINE_STEP_LOOKAHEAD;
                    if horizontal_collides(lookahead_probe_x, y) {
                        return Some(lookahead_probe_x);
                    }
                }
                None
            })
        });
        if first_wall_probe.is_some() {
            hit_wall = true;
        }

        // While rising from a jump, allow a one-block step assist so pressing
        // into a ledge doesn't feel sticky or inconsistent.
        let can_attempt_step =
            grounded || fluid_exit_step || (0.0..0.08).contains(&vy) || rising_jump_assist;
        if (hit_wall || step_probe_x.is_some())
            && do_auto_step
            && can_attempt_step
            && let Some(step_probe_x) = step_probe_x
        {
            let step_heights: &[f64] = if fluid_exit_step || rising_jump_assist {
                &[0.5, 0.6, 0.78, 0.92, 1.0]
            } else {
                &[0.5, 0.6]
            };
            for &step_up in step_heights {
                let step_y = y - step_up;
                if !horizontal_collides(step_probe_x, step_y)
                    && !self.is_colliding(x, step_y - height, CollisionType::VerticalUp)
                    && !self.is_colliding(
                        x - half_width,
                        step_y - height,
                        CollisionType::VerticalUp,
                    )
                    && !self.is_colliding(
                        x + half_width,
                        step_y - height,
                        CollisionType::VerticalUp,
                    )
                {
                    y -= step_up;
                    if step_up >= 0.78 {
                        // Big jump-assist steps should settle on top immediately instead of
                        // continuing upward with stale jump velocity.
                        vy = 0.0;
                        grounded = true;
                        force_land_after_step = true;
                    }
                    let shoreline_forward_nudge = if fluid_exit_step {
                        if in_lava { 0.09 } else { 0.06 }
                    } else {
                        0.0
                    };
                    step_target_x = Some(if vx > 0.0 {
                        next_x.max(
                            step_probe_x.floor() - half_width
                                + COLLISION_EPSILON
                                + shoreline_forward_nudge,
                        )
                    } else if vx < 0.0 {
                        next_x.min(
                            step_probe_x.floor() + 1.0 + half_width
                                - COLLISION_EPSILON
                                - shoreline_forward_nudge,
                        )
                    } else {
                        next_x
                    });
                    hit_wall = false;
                    break;
                }
            }
        }

        let destination_x = step_target_x.unwrap_or(next_x);
        // Recheck the path at the raised height after auto-stepping, including
        // any second obstacle beyond the first ledge.
        if !hit_wall && first_wall_probe.is_some() {
            let distance = destination_x - x;
            let steps = ((distance.abs() / 0.25).ceil() as usize).clamp(1, 256);
            hit_wall = (1..=steps).any(|step| {
                horizontal_collides(x + distance * step as f64 / steps as f64 + edge_offset, y)
            });
        }
        if !hit_wall {
            x = destination_x;
        } else if check_x.is_some() {
            let horizontal_edge_offset = if vx > 0.0 {
                half_width - COLLISION_EPSILON
            } else {
                -half_width + COLLISION_EPSILON
            };
            let sweep_steps = ((vx.abs() / 0.04).ceil() as usize).clamp(1, 256);
            let mut furthest_x = x;
            for step in 1..=sweep_steps {
                let t = step as f64 / sweep_steps as f64;
                let candidate_x = x + vx * t;
                if horizontal_collides(candidate_x + horizontal_edge_offset, y) {
                    let mut clear = furthest_x;
                    let mut blocked = candidate_x;
                    for _ in 0..12 {
                        let middle = (clear + blocked) * 0.5;
                        if horizontal_collides(middle + horizontal_edge_offset, y) {
                            blocked = middle;
                        } else {
                            clear = middle;
                        }
                    }
                    furthest_x = clear;
                    break;
                }
                furthest_x = candidate_x;
            }
            x = furthest_x;
            vx = 0.0;
        }

        let next_y = y + vy;
        let prev_y = y;
        let side_probe = (half_width - COLLISION_EPSILON).max(0.0);
        if force_land_after_step {
            y = self
                .find_ground_snap_y(x, y, half_width, 1.05)
                .unwrap_or_else(|| y.floor());
            vy = 0.0;
            grounded = true;
        } else if vy != 0.0 {
            let falling = vy > 0.0;
            let steps = ((vy.abs() / 0.25).ceil() as usize).clamp(1, 256);
            let collision_y = (1..=steps)
                .map(|step| prev_y + vy * step as f64 / steps as f64)
                .find(|&candidate_y| {
                    let probe_y = if falling {
                        candidate_y
                    } else {
                        candidate_y - height
                    };
                    let kind = if falling {
                        CollisionType::VerticalDown(prev_y)
                    } else {
                        CollisionType::VerticalUp
                    };
                    [x, x - side_probe, x + side_probe]
                        .into_iter()
                        .any(|probe_x| self.is_colliding(probe_x, probe_y, kind))
                });
            if let Some(collision_y) = collision_y {
                let probe_y = if falling {
                    collision_y
                } else {
                    collision_y - height
                };
                let surfaces = [x, x - side_probe, x + side_probe]
                    .into_iter()
                    .filter_map(|probe_x| self.collision_bounds_at_point(probe_x, probe_y));
                y = if falling {
                    surfaces.map(|b| b[1]).fold(collision_y, f64::min)
                } else {
                    surfaces.map(|b| b[3]).fold(probe_y, f64::max) + height + COLLISION_EPSILON
                };
                vy = 0.0;
                grounded = falling;
            } else {
                y = next_y;
                grounded = false;
            }
        }

        let (resolved_x, resolved_y, resolved_vx, resolved_vy, resolved_grounded, overlap_hit_wall) =
            self.resolve_entity_overlap(
                start_x, start_y, x, y, vx, vy, half_width, height, grounded,
            );
        x = resolved_x;
        y = resolved_y;
        vx = resolved_vx;
        vy = resolved_vy;
        grounded = resolved_grounded;
        hit_wall |= overlap_hit_wall;

        if !in_water && !in_lava && !on_ladder && (0.0..=0.14).contains(&vy) {
            let max_snap_distance = if started_grounded && !jump_held && vy >= 0.0 {
                0.55
            } else if grounded {
                0.24
            } else if vx.abs() < 0.02 {
                0.18
            } else if started_grounded && vx.abs() < 0.08 {
                0.1
            } else {
                0.04
            };
            if let Some(snap_y) = self.find_ground_snap_y(x, y, half_width, max_snap_distance)
                && snap_y > y + COLLISION_EPSILON
                && self.has_stable_ground_support(x, snap_y, half_width, vx)
            {
                y = snap_y;
                vy = 0.0;
                grounded = true;
            }
        }

        if grounded && !self.has_stable_ground_support(x, y, half_width, vx) {
            grounded = false;
        }

        (x, y, vx, vy, grounded, hit_wall)
    }

    fn is_colliding(&self, x: f64, y: f64, coll_type: CollisionType) -> bool {
        let block_x = x.floor() as i32;
        let block_y = y.floor() as i32;
        let block = self.world.get_block(block_x, block_y);
        if !block.is_solid() {
            return false;
        }
        // Trees act as one-way platforms (can stand on them, walk past them, but not hit head on them)
        if matches!(block, BlockType::Leaves | BlockType::BirchLeaves)
            || (matches!(block, BlockType::Wood | BlockType::BirchWood)
                && self.wood_acts_as_tree_platform_at(block_x, block_y))
        {
            if let CollisionType::VerticalDown(prev_y) = coll_type {
                return (prev_y as f32) <= (block_y as f32) + 0.05;
            }
            return false;
        }
        self.collision_bounds_at_point(x, y).is_some()
    }

    fn is_exposed_to_sky(&self, x: f64, top_y: f64) -> bool {
        let bx = x.floor() as i32;
        let top_block_y = top_y.floor() as i32;
        for y in 0..=top_block_y {
            if self.world.get_block(bx, y).is_solid() {
                return false;
            }
        }
        true
    }

    fn player_touches_cactus(&self) -> bool {
        let left = self.player.x - PLAYER_HALF_WIDTH - 0.02;
        let right = self.player.x + PLAYER_HALF_WIDTH + 0.02;
        let top = self.player.y - PLAYER_HEIGHT - 0.02;
        let bottom = self.player.y + 0.02;

        for by in top.floor() as i32..=(bottom - 0.001).floor() as i32 {
            for bx in left.floor() as i32..=(right - 0.001).floor() as i32 {
                if self.world.get_block(bx, by) != BlockType::Cactus {
                    continue;
                }

                let block_left = bx as f64;
                let block_right = block_left + 1.0;
                let block_top = by as f64;
                let block_bottom = block_top + 1.0;
                let overlap_x = right.min(block_right) - left.max(block_left);
                let overlap_y = bottom.min(block_bottom) - top.max(block_top);
                if overlap_x > 0.0 && overlap_y > 0.0 {
                    return true;
                }
            }
        }

        false
    }

    fn apply_undead_fire_rules(
        &self,
        x: f64,
        y: f64,
        height: f64,
        age: u64,
        is_day: bool,
        burning_timer: i32,
    ) -> (i32, f32) {
        let head_y = (y - (height - 0.3)).floor() as i32;
        let feet_y = y.floor() as i32;
        let bx = x.floor() as i32;
        let head_block = self.world.get_block(bx, head_y);
        let feet_block = self.world.get_block(bx, feet_y);

        let in_water =
            matches!(head_block, BlockType::Water(_)) || matches!(feet_block, BlockType::Water(_));
        let in_lava =
            matches!(head_block, BlockType::Lava(_)) || matches!(feet_block, BlockType::Lava(_));
        let weather_wet = self.is_weather_wet_at(x, y - (height - 0.3));

        let sun_exposed = is_day && self.is_exposed_to_sky(x, y - (height - 0.3));

        let mut next_burning_timer = burning_timer;
        let mut damage = 0.0;

        if in_water || weather_wet {
            return (0, 0.0);
        }

        if in_lava {
            return (0, 0.0);
        }

        if sun_exposed {
            next_burning_timer = 10;
            if age.is_multiple_of(20) {
                damage += 1.0;
            }
            return (next_burning_timer, damage);
        }

        if next_burning_timer > 0 {
            next_burning_timer -= 1;
        }

        (next_burning_timer, damage)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entities::creeper::Creeper;
    use crate::entities::ocelot::Ocelot;
    use crate::entities::skeleton::Skeleton;
    use crate::entities::wolf::Wolf;
    use crate::entities::zombie::Zombie;
    use crate::world::block::BlockType;
    use crate::world::item::{ItemStack, ItemType, Recipe};
    use rand::SeedableRng;

    fn configure_quiet_world(state: &mut GameState) {
        state.world.load_chunks_around(0);
        state.world.newly_generated_chunks.clear();
        state.game_rules.do_mob_spawning = false;
        state.overworld_hostile_spawn_timer = u16::MAX;
        state.overworld_passive_spawn_timer = u16::MAX;
        state.overworld_squid_spawn_timer = u16::MAX;
        state.overworld_wolf_spawn_timer = u16::MAX;
        state.overworld_ocelot_spawn_timer = u16::MAX;
        state.nether_spawn_timer = u16::MAX;
        state.end_spawn_timer = u16::MAX;
        state.zombies.clear();
        state.creepers.clear();
        state.skeletons.clear();
        state.spiders.clear();
        state.silverfish.clear();
        state.slimes.clear();
        state.endermen.clear();
        state.pigmen.clear();
        state.ghasts.clear();
        state.blazes.clear();
        state.cows.clear();
        state.sheep.clear();
        state.pigs.clear();
        state.chickens.clear();
        state.squids.clear();
        state.wolves.clear();
        state.ocelots.clear();
        state.item_entities.clear();
        state.arrows.clear();
        state.fireballs.clear();
    }

    fn prepare_test_respawn_ground(state: &mut GameState) {
        // Respawn-state tests need a known safe bank inside their asserted
        // radius, independent of a randomly seeded ocean at the origin.
        state.world.load_chunks_for_spawn_search(0, 56);
        for x in -56..=56 {
            for y in 0..32 {
                state.world.set_block(x, y, BlockType::Air);
            }
            state.world.set_block(x, 32, BlockType::Grass);
        }
    }

    fn setup_player_in_lava_column(state: &mut GameState) {
        configure_quiet_world(state);
        state.player.x = 0.5;
        state.player.y = 10.0;
        state.player.vx = 0.0;
        state.player.vy = 0.0;
        state.player.grounded = false;
        for y in 7..13 {
            state.world.set_block(0, y, BlockType::Lava(8));
        }
    }

    fn build_side_entry_hut(state: &mut GameState, door_x: i32, base_y: i32, door_on_left: bool) {
        let left = if door_on_left { door_x } else { door_x - 5 };
        let right = if door_on_left { door_x + 5 } else { door_x };
        for x in (left - 4)..=(right + 4) {
            for y in 0..28 {
                state.world.set_block(x, y, BlockType::Air);
            }
            state.world.set_block(x, base_y + 1, BlockType::Stone);
        }

        for x in left..=right {
            state.world.set_block(x, base_y - 2, BlockType::Planks);
        }
        let opposite_wall_x = if door_on_left { right } else { left };
        for y in (base_y - 1)..=base_y {
            state.world.set_block(opposite_wall_x, y, BlockType::Wood);
        }
        state
            .world
            .set_block(door_x, base_y, BlockType::WoodDoor(false));
        state
            .world
            .set_block(door_x, base_y - 1, BlockType::WoodDoor(false));
    }

    fn fishing_loot_total_count(state: &GameState) -> u32 {
        [
            ItemType::RawFish,
            ItemType::Stick,
            ItemType::String,
            ItemType::Leather,
            ItemType::Bone,
            ItemType::RottenFlesh,
            ItemType::WaterBottle,
            ItemType::LeatherBoots,
            ItemType::FishingRod,
            ItemType::Bow,
            ItemType::IronIngot,
            ItemType::GoldIngot,
            ItemType::Diamond,
        ]
        .into_iter()
        .map(|item| state.inventory_item_count(item))
        .sum()
    }

    #[test]
    fn test_physics_falling_and_landing() {
        let mut state = GameState::new();
        state.world.load_chunks_around(0);
        // Clear a space and put a floor at y=10
        for x in -5..=5 {
            for y in 0..15 {
                state.world.set_block(x, y, BlockType::Air);
            }
            state.world.set_block(x, 10, BlockType::Stone);
        }

        // Drop entity from y=8.0 (feet). It should accelerate down and land exactly on y=10.0
        let (nx, mut ny, nvx, mut nvy, mut ngr, _) =
            state.calculate_movement(0.0, 8.0, 0.0, 0.0, false, 0.3, 1.8, false);

        assert!(nvy > 0.0); // Accelerated down
        assert_eq!(nx, 0.0);
        assert!(!ngr);

        // Simulate falling until hitting the ground
        for _ in 0..50 {
            let res = state.calculate_movement(0.0, ny, nvx, nvy, ngr, 0.3, 1.8, false);
            ny = res.1;
            nvy = res.3;
            ngr = res.4;
            if ngr {
                break;
            }
        }

        assert!(ngr);
        assert_eq!(ny, 10.0); // Snapped exactly to the floor
        assert_eq!(nvy, 0.0);
    }

    #[test]
    fn swept_collision_stops_fast_motion_at_thin_walls_and_ceilings() {
        let mut state = GameState::new();
        state.world.load_chunks_around(0);
        for x in -8..=8 {
            for y in 2..=12 {
                state.world.set_block(x, y, BlockType::Air);
            }
        }
        for x in [-2, 2] {
            state.world.set_block(x, 9, BlockType::Stone);
            state.world.set_block(x, 8, BlockType::Stone);
        }
        for velocity in [-4.0, 4.0] {
            let (x, _, vx, _, _, wall) =
                state.calculate_movement(0.5, 10.0, velocity, 0.0, false, 0.3, 1.8, false);
            assert!(wall, "fast motion {velocity} crossed a thin wall to {x}");
            assert_eq!(vx, 0.0);
            assert!((-0.71..=1.71).contains(&x));
        }
        state.world.set_block(0, 7, BlockType::Stone);
        let (_, y, _, vy, grounded, _) =
            state.calculate_movement(0.5, 10.0, 0.0, -4.0, false, 0.3, 1.8, false);
        assert!(
            y >= 9.8 - 0.002,
            "fast upward motion crossed the ceiling to {y}"
        );
        assert_eq!(vy, 0.0);
        assert!(!grounded, "a ceiling is not ground support");
    }

    #[test]
    fn dimension_travel_keeps_current_world_when_its_save_fails() {
        let root = std::env::temp_dir().join(format!(
            "termcraft-travel-{}-{}",
            crate::platform::process_id(),
            rand::random::<u64>()
        ));
        let store = crate::worlds::WorldStore::new(root.clone());
        let path = store
            .create(
                "test",
                "travel fixture",
                &crate::world::recipe::WorldRecipe::example(),
            )
            .unwrap();
        let mut state = GameState::open_in_directory(&path).unwrap();
        state.world.try_save_all().unwrap();
        state.world.set_block(1, 30, BlockType::DiamondOre);
        crate::platform::fs::create_dir(path.join("overworld_chunk_0.bin.tmp")).unwrap();
        let position = (state.player.x, state.player.y);
        state.transfer_nether_dimension();
        assert_eq!(state.current_dimension, Dimension::Overworld);
        assert_eq!((state.player.x, state.player.y), position);
        assert_eq!(state.world.get_block(1, 30), BlockType::DiamondOre);
        assert!(
            state
                .save_error
                .borrow()
                .as_ref()
                .unwrap()
                .contains("Travel cancelled")
        );
        drop(state);
        crate::platform::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn test_physics_horizontal_wall_collision() {
        let mut state = GameState::new();
        state.world.load_chunks_around(0);
        for x in -5..=5 {
            for y in 0..15 {
                state.world.set_block(x, y, BlockType::Air);
            }
        }
        state.world.set_block(2, 9, BlockType::Stone); // Wall block at x=2, y=9

        // Move right towards the wall from x=1.0. With half_width=0.3, right edge is 1.3.
        // Wall is at x=2.0 to 3.0. Moving right by 0.8 puts right edge at 2.1 (inside wall).
        let (nx, _, _, _, _, hit_wall) =
            state.calculate_movement(1.0, 10.0, 0.8, 0.0, false, 0.3, 1.8, false);

        assert!(hit_wall);
        assert!(nx > 1.0); // Should still move as far as possible toward the wall.
        assert!(nx < 1.8); // But must not pass through it.
    }

    #[test]
    fn test_horizontal_wall_collision_zeroes_horizontal_velocity() {
        let mut state = GameState::new();
        state.world.load_chunks_around(0);
        for x in -5..=5 {
            for y in 0..15 {
                state.world.set_block(x, y, BlockType::Air);
            }
            state.world.set_block(x, 10, BlockType::Stone);
        }
        state.world.set_block(2, 9, BlockType::Stone);

        let (_nx, ny, nvx, nvy, ngr, hit_wall) =
            state.calculate_movement(1.0, 10.0, 0.8, 0.0, true, 0.3, 1.8, true);

        assert!(hit_wall);
        assert_eq!(nvx, 0.0);
        assert_eq!(ny, 10.0);
        assert_eq!(nvy, 0.0);
        assert!(ngr);
    }

    #[test]
    fn test_horizontal_collision_checks_mid_body_for_tall_entities() {
        let mut state = GameState::new();
        state.world.load_chunks_around(0);
        for x in -5..=5 {
            for y in 0..15 {
                state.world.set_block(x, y, BlockType::Air);
            }
        }

        // For a tall entity (height 2.9), the body spans y=7..9 when feet are at y=10.
        // A block in that middle row should still block horizontal movement.
        state.world.set_block(2, 8, BlockType::Stone);

        let (nx, _, _, _, _, hit_wall) =
            state.calculate_movement(1.0, 10.0, 0.8, 0.0, false, 0.3, 2.9, false);

        assert!(hit_wall);
        assert!(nx < 1.8);
    }

    #[test]
    fn test_physics_auto_step() {
        let mut state = GameState::new();
        state.world.load_chunks_around(0);
        for x in -5..=5 {
            for y in 0..15 {
                state.world.set_block(x, y, BlockType::Air);
            }
            state.world.set_block(x, 10, BlockType::Stone); // floor at y=10
        }

        // Block at x=2, y=9 (1 block higher than floor)
        state.world.set_block(2, 9, BlockType::Stone);

        // Player is at x=1.0, y=10.0, grounded. Move right by 0.8.
        // Because the block is 1.0 high, and auto_step limit is 0.6, it should NOT step up.
        let (nx1, ny1, _, _, _, hit_wall1) =
            state.calculate_movement(1.0, 10.0, 0.8, 0.0, true, 0.3, 1.8, true);
        assert!(hit_wall1);
        assert!(nx1 > 1.0);
        assert!(nx1 < 1.8);
        assert_eq!(ny1, 10.0); // y remains the same

        // Now imagine a scenario where the obstacle is less than 0.6 units high relative to player feet.
        // If player is at y=9.5, moving into block at y=9 (top=9.0).
        // Height diff is 9.5 - 9.0 = 0.5 <= 0.6.
        let (nx2, ny2, nvx2, nvy2, ngr2, hit_wall2) =
            state.calculate_movement(1.0, 9.5, 0.8, 0.0, true, 0.3, 1.8, true);

        assert!(!hit_wall2); // It stepped up!
        assert_eq!(nx2, 1.8);
        assert_eq!(ny2, 9.0); // Uses the smallest valid step and lands on the raised block.
        assert_eq!(nvy2, 0.0);
        assert!(ngr2);

        // Next tick should snap it precisely to the floor at 9.0
        let (_, ny3, _, _, ngr3, _) =
            state.calculate_movement(nx2, ny2, nvx2, nvy2, ngr2, 0.3, 1.8, true);
        assert_eq!(ny3, 9.0);
        assert!(ngr3);
    }

    #[test]
    fn test_jump_assist_steps_up_single_block_when_rising() {
        let mut state = GameState::new();
        state.world.load_chunks_around(0);
        for x in -5..=5 {
            for y in 0..15 {
                state.world.set_block(x, y, BlockType::Air);
            }
            state.world.set_block(x, 10, BlockType::Stone);
        }

        // One-block rise directly ahead.
        state.world.set_block(2, 9, BlockType::Stone);

        // Already jumping upward into the block edge.
        let (nx, ny, _, _, _, hit_wall) =
            state.calculate_movement(1.0, 10.0, 0.8, -0.35, false, 0.3, 1.8, true);

        assert!(!hit_wall);
        assert!(nx > 1.6);
        assert!(ny <= 9.0);
    }

    #[test]
    fn test_jump_assist_big_step_lands_without_extra_upward_velocity() {
        let mut state = GameState::new();
        state.world.load_chunks_around(0);
        for x in -5..=5 {
            for y in 0..15 {
                state.world.set_block(x, y, BlockType::Air);
            }
            state.world.set_block(x, 10, BlockType::Stone);
        }
        state.world.set_block(2, 9, BlockType::Stone);

        let (_nx, ny, _nvx, nvy, ngr, hit_wall) =
            state.calculate_movement(1.0, 10.0, 0.8, -0.35, false, 0.3, 1.8, true);

        assert!(!hit_wall);
        assert!(ny <= 9.0);
        assert!(ngr);
        assert_eq!(nvy, 0.0);
    }

    #[test]
    fn test_jump_assist_does_not_step_two_block_wall() {
        let mut state = GameState::new();
        state.world.load_chunks_around(0);
        for x in -5..=5 {
            for y in 0..15 {
                state.world.set_block(x, y, BlockType::Air);
            }
            state.world.set_block(x, 10, BlockType::Stone);
        }

        // Two-block obstruction ahead: should still block movement.
        state.world.set_block(2, 9, BlockType::Stone);
        state.world.set_block(2, 8, BlockType::Stone);

        let (nx, ny, _, _, _, hit_wall) =
            state.calculate_movement(1.0, 10.0, 0.8, -0.35, false, 0.3, 1.8, true);

        assert!(hit_wall);
        assert!(nx < 1.8);
        assert!(ny < 10.0);
    }

    #[test]
    fn test_non_solid_flower_does_not_block_horizontal_movement() {
        let mut state = GameState::new();
        state.world.load_chunks_around(0);
        for x in -5..=5 {
            for y in 0..15 {
                state.world.set_block(x, y, BlockType::Air);
            }
            state.world.set_block(x, 10, BlockType::Stone);
        }
        state.world.set_block(2, 9, BlockType::RedFlower);

        let (nx, _, _, _, _, hit_wall) =
            state.calculate_movement(1.0, 10.0, 0.6, 0.0, true, 0.25, 1.8, true);

        assert!(!hit_wall);
        assert!(nx > 1.0);
    }

    #[test]
    fn test_flower_does_not_snag_player_with_small_ground_overlap() {
        let mut state = GameState::new();
        state.world.load_chunks_around(0);
        for x in -5..=5 {
            for y in 0..15 {
                state.world.set_block(x, y, BlockType::Air);
            }
            state.world.set_block(x, 10, BlockType::Stone);
        }
        state.world.set_block(2, 9, BlockType::YellowFlower);

        let (nx, ny, nvx, _nvy, ngr, hit_wall) =
            state.calculate_movement(1.0, 10.03, 0.38, 0.0, true, 0.25, 1.8, true);

        assert!(!hit_wall, "flower movement should not register as a wall");
        assert!(ngr);
        assert!(ny >= 10.0);
        assert!(nx > 1.2);
        assert!(nvx > 0.0);
    }

    #[test]
    fn test_falling_onto_flower_column_lands_on_ground_not_flower_height() {
        let mut state = GameState::new();
        state.world.load_chunks_around(0);
        for x in -3..=3 {
            for y in 0..15 {
                state.world.set_block(x, y, BlockType::Air);
            }
            state.world.set_block(x, 10, BlockType::Stone);
        }
        state.world.set_block(1, 9, BlockType::RedFlower);

        let mut x = 1.5;
        let mut y = 9.72;
        let mut vx = 0.0;
        let mut vy = 0.14;
        let mut grounded = false;

        for _ in 0..4 {
            let (nx, ny, nvx, nvy, ngr, _hit_wall) = state.calculate_movement(
                x,
                y,
                vx,
                vy,
                grounded,
                PLAYER_HALF_WIDTH,
                PLAYER_HEIGHT,
                true,
            );
            x = nx;
            y = ny;
            vx = nvx;
            vy = nvy;
            grounded = ngr;
            if grounded {
                break;
            }
        }

        assert!(grounded);
        assert_eq!(y, 10.0);
    }

    #[test]
    fn test_player_placed_wood_blocks_horizontal_movement() {
        let mut state = GameState::new();
        state.world.load_chunks_around(0);
        for x in -4..=4 {
            for y in 0..15 {
                state.world.set_block(x, y, BlockType::Air);
            }
            state.world.set_block(x, 10, BlockType::Stone);
        }
        state.world.set_block(2, 9, BlockType::Wood);

        assert!(
            state.block_has_entity_collision_at(2, 9),
            "player-placed wood should register as a solid collision block"
        );
        assert!(state.is_colliding(2.0, 9.1, CollisionType::Horizontal));
    }

    #[test]
    fn test_tree_trunk_still_behaves_as_one_way_platform() {
        let mut state = GameState::new();
        state.world.load_chunks_around(0);
        for x in -4..=4 {
            for y in 0..15 {
                state.world.set_block(x, y, BlockType::Air);
            }
            state.world.set_block(x, 10, BlockType::Stone);
        }
        state.world.set_block(2, 9, BlockType::Wood);
        state.world.set_block(2, 8, BlockType::Wood);
        state.world.set_block(1, 7, BlockType::Leaves);
        state.world.set_block(2, 7, BlockType::Leaves);
        state.world.set_block(3, 7, BlockType::Leaves);

        let (nx, _ny, _nvx, _nvy, _ngr, hit_wall) =
            state.calculate_movement(1.0, 10.0, 0.65, 0.0, true, 0.25, 1.8, true);

        assert!(
            !hit_wall,
            "natural tree trunks should stay pass-through laterally"
        );
        assert!(nx > 1.2);
        assert!(state.is_colliding(2.5, 9.1, CollisionType::VerticalDown(8.95)));
    }

    #[test]
    fn test_idle_at_tile_boundary_does_not_report_horizontal_collision() {
        let mut state = GameState::new();
        state.world.load_chunks_around(0);
        for x in -5..=5 {
            for y in 0..15 {
                state.world.set_block(x, y, BlockType::Air);
            }
            state.world.set_block(x, 10, BlockType::Stone);
        }
        state.world.set_block(2, 9, BlockType::Stone);

        let (nx, ny, _, _, _, hit_wall) =
            state.calculate_movement(1.75, 10.0, 0.0, 0.0, true, 0.25, 1.8, true);

        assert!(!hit_wall);
        assert_eq!(nx, 1.75);
        assert_eq!(ny, 10.0);
    }

    #[test]
    fn test_calculate_movement_pushes_player_out_of_embedded_floor_block() {
        let mut state = GameState::new();
        state.world.load_chunks_around(0);
        for x in -2..=2 {
            for y in 0..15 {
                state.world.set_block(x, y, BlockType::Air);
            }
        }
        state.world.set_block(0, 9, BlockType::Stone);

        let (_nx, ny, _nvx, nvy, ngr, _hit_wall) = state.calculate_movement(
            0.5,
            9.7,
            0.0,
            0.0,
            false,
            PLAYER_HALF_WIDTH,
            PLAYER_HEIGHT,
            true,
        );

        assert!(ngr);
        assert_eq!(ny, 9.0);
        assert_eq!(nvy, 0.0);
    }

    #[test]
    fn test_calculate_movement_pushes_player_out_of_embedded_wall_block() {
        let mut state = GameState::new();
        state.world.load_chunks_around(0);
        for x in -3..=3 {
            for y in 0..15 {
                state.world.set_block(x, y, BlockType::Air);
            }
            state.world.set_block(x, 10, BlockType::Stone);
        }
        state.world.set_block(1, 9, BlockType::Stone);

        let (nx, ny, nvx, nvy, ngr, hit_wall) = state.calculate_movement(
            1.1,
            10.0,
            0.0,
            0.0,
            true,
            PLAYER_HALF_WIDTH,
            PLAYER_HEIGHT,
            true,
        );

        assert!(hit_wall);
        assert!(nx < 0.76);
        assert_eq!(ny, 10.0);
        assert_eq!(nvx, 0.0);
        assert_eq!(nvy, 0.0);
        assert!(ngr);
    }

    #[test]
    fn test_calculate_movement_snaps_near_ground_player_to_surface() {
        let mut state = GameState::new();
        state.world.load_chunks_around(0);
        for x in -2..=2 {
            for y in 0..15 {
                state.world.set_block(x, y, BlockType::Air);
            }
            state.world.set_block(x, 10, BlockType::Stone);
        }

        let (_nx, ny, _nvx, nvy, ngr, _hit_wall) = state.calculate_movement(
            0.5,
            9.82,
            0.0,
            0.04,
            false,
            PLAYER_HALF_WIDTH,
            PLAYER_HEIGHT,
            true,
        );

        assert!(ngr);
        assert_eq!(ny, 10.0);
        assert_eq!(nvy, 0.0);
    }

    #[test]
    fn test_calculate_movement_drops_after_clearing_single_block_ledge() {
        let mut state = GameState::new();
        state.world.load_chunks_around(0);
        for x in -2..=4 {
            for y in 0..15 {
                state.world.set_block(x, y, BlockType::Air);
            }
        }
        state.world.set_block(0, 10, BlockType::Stone);

        let (_nx, ny, _nvx, nvy, ngr, _hit_wall) = state.calculate_movement(
            0.74,
            10.0,
            0.6, // The whole foot, including its trailing edge, clears the block.
            0.0,
            true,
            PLAYER_HALF_WIDTH,
            PLAYER_HEIGHT,
            true,
        );

        assert!(!ngr, "expected to lose grounding after clearing the ledge");
        assert!(
            ny >= 10.0,
            "expected to stay at or below ledge height, got y={ny}"
        );
        assert!(
            nvy >= 0.0,
            "expected non-upward motion after leaving the ledge"
        );
    }

    #[test]
    fn test_repeated_movement_ticks_fall_promptly_after_running_off_ledge() {
        let mut state = GameState::new();
        state.world.load_chunks_around(0);
        for x in -2..=6 {
            for y in 0..15 {
                state.world.set_block(x, y, BlockType::Air);
            }
        }
        for x in -2..=1 {
            state.world.set_block(x, 10, BlockType::Stone);
        }

        let mut x = 0.72;
        let mut y = 10.0;
        let mut vx = 0.6;
        let mut vy = 0.0;
        let mut grounded = true;

        for _ in 0..3 {
            let (nx, ny, nvx, nvy, ngr, _) = state.calculate_movement(
                x,
                y,
                vx,
                vy,
                grounded,
                PLAYER_HALF_WIDTH,
                PLAYER_HEIGHT,
                true,
            );
            x = nx;
            y = ny;
            vx = nvx;
            vy = nvy;
            grounded = ngr;
        }

        assert!(!grounded, "expected to be airborne after leaving the ledge");
        assert!(
            y >= 10.08,
            "expected visible descent after three ticks, got y={y}"
        );
        assert!(
            vy > 0.0,
            "expected downward velocity after leaving the ledge, got vy={vy}"
        );
    }

    #[test]
    fn test_calculate_movement_uses_partial_water_contact_for_fluid_drag() {
        let mut state = GameState::new();
        state.world.load_chunks_around(0);
        for x in -2..=2 {
            for y in 0..15 {
                state.world.set_block(x, y, BlockType::Air);
            }
        }
        state.world.set_block(0, 10, BlockType::Water(8));

        let (_nx, _ny, nvx, nvy, ngr, _hit_wall) = state.calculate_movement(
            0.5,
            10.0,
            0.5,
            0.0,
            false,
            PLAYER_HALF_WIDTH,
            PLAYER_HEIGHT,
            false,
        );

        assert!(!ngr);
        assert!(nvx.abs() < 0.45);
        assert!(nvy <= 0.08);
    }

    #[test]
    fn test_calculate_movement_fully_submerged_water_caps_fall_speed() {
        let mut state = GameState::new();
        state.world.load_chunks_around(0);
        for x in -2..=2 {
            for y in 0..15 {
                state.world.set_block(x, y, BlockType::Air);
            }
        }
        for y in 8..=10 {
            state.world.set_block(0, y, BlockType::Water(8));
        }

        let (_nx, _ny, _nvx, nvy, ngr, _hit_wall) = state.calculate_movement(
            0.5,
            10.0,
            0.0,
            0.9,
            false,
            PLAYER_HALF_WIDTH,
            PLAYER_HEIGHT,
            false,
        );

        assert!(!ngr);
        assert!(nvy <= 0.15, "expected water-capped fall speed, got {nvy}");
    }

    #[test]
    fn test_calculate_movement_steps_out_of_shallow_water_onto_one_block_bank() {
        let mut state = GameState::new();
        state.world.load_chunks_around(0);
        for x in -2..=4 {
            for y in 0..15 {
                state.world.set_block(x, y, BlockType::Air);
            }
            state.world.set_block(x, 10, BlockType::Stone);
        }
        for x in 0..=1 {
            // One block deep is genuinely partial immersion for a 1.8-block
            // actor. Two full cells cover the head and require swimming up.
            for y in 9..=9 {
                state.world.set_block(x, y, BlockType::Water(8));
            }
        }
        state.world.set_block(2, 9, BlockType::Stone);
        state.world.set_block(3, 9, BlockType::Stone);

        let (nx, ny, _nvx, _nvy, ngr, hit_wall) = state.calculate_movement(
            1.5,
            10.0,
            0.45,
            0.0,
            false,
            PLAYER_HALF_WIDTH,
            PLAYER_HEIGHT,
            true,
        );

        assert!(!hit_wall);
        assert!(ngr);
        assert!(nx > 1.5);
        assert!(ny < 9.5);
    }

    #[test]
    fn test_calculate_movement_steps_out_of_deep_water_onto_one_block_bank_with_jump_hold() {
        let mut state = GameState::new();
        state.world.load_chunks_around(0);
        for x in -2..=4 {
            for y in 0..18 {
                state.world.set_block(x, y, BlockType::Air);
            }
            state.world.set_block(x, 11, BlockType::Stone);
        }
        for x in 0..=1 {
            for y in 8..=10 {
                state.world.set_block(x, y, BlockType::Water(8));
            }
        }
        state.world.set_block(2, 10, BlockType::Stone);
        state.world.set_block(3, 10, BlockType::Stone);
        state.apply_client_command(ClientCommand::SetJumpHeld(true));

        let (nx, ny, _nvx, _nvy, ngr, hit_wall) = state.calculate_movement(
            1.5,
            10.8,
            0.28,
            -0.04,
            false,
            PLAYER_HALF_WIDTH,
            PLAYER_HEIGHT,
            true,
        );

        assert!(
            !hit_wall,
            "expected clean shoreline step, got wall at x={nx} y={ny}"
        );
        assert!(ngr, "expected grounded shoreline exit, got x={nx} y={ny}");
        assert!(
            nx > 1.6,
            "expected forward progress out of water, got x={nx}"
        );
        assert!(ny <= 10.0, "expected shoreline landing height, got y={ny}");
    }

    #[test]
    fn test_lava_escape_with_jump_hold_and_forward_input_reaches_bank() {
        let mut state = GameState::new();
        configure_quiet_world(&mut state);
        for x in -2..=4 {
            for y in 0..18 {
                state.world.set_block(x, y, BlockType::Air);
            }
            state.world.set_block(x, 11, BlockType::Netherrack);
        }
        for x in 0..=1 {
            for y in 8..=10 {
                state.world.set_block(x, y, BlockType::Lava(8));
            }
        }
        state.world.set_block(2, 10, BlockType::Netherrack);
        state.world.set_block(3, 10, BlockType::Netherrack);
        state.player.x = 1.5;
        state.player.y = 10.8;
        state.player.vx = 0.0;
        state.player.vy = 0.0;
        state.player.grounded = false;
        state.apply_client_command(ClientCommand::SetMoveRight(true));
        state.apply_client_command(ClientCommand::SetJumpHeld(true));

        for _ in 0..14 {
            state.update(0, 0);
            if state.player.grounded && state.player.x > 1.8 && state.player.y <= 10.0 {
                break;
            }
        }

        assert!(
            state.player.grounded,
            "expected grounded lava exit, got x={} y={}",
            state.player.x, state.player.y
        );
        assert!(
            state.player.x > 1.8,
            "expected forward progress out of lava, got x={}",
            state.player.x
        );
        assert!(
            state.player.y <= 10.0,
            "expected lava-bank landing height, got y={}",
            state.player.y
        );
    }

    #[test]
    fn test_swimming_horizontal_movement_is_slower_than_ground_walk() {
        let mut walk_state = GameState::new();
        configure_quiet_world(&mut walk_state);
        for x in -8..=8 {
            for y in 0..20 {
                walk_state.world.set_block(x, y, BlockType::Air);
            }
            walk_state.world.set_block(x, 10, BlockType::Stone);
        }
        walk_state.player.x = 0.0;
        walk_state.player.y = 10.0;
        walk_state.player.vx = 0.0;
        walk_state.player.vy = 0.0;
        walk_state.player.grounded = true;
        walk_state.apply_client_command(ClientCommand::SetMoveRight(true));
        walk_state.update(0, 0);
        let walk_vx = walk_state.player.vx.abs();

        let mut swim_state = GameState::new();
        configure_quiet_world(&mut swim_state);
        for x in -8..=8 {
            for y in 0..20 {
                swim_state.world.set_block(x, y, BlockType::Air);
            }
        }
        for x in 0..=2 {
            for y in 8..=10 {
                swim_state.world.set_block(x, y, BlockType::Water(8));
            }
        }
        swim_state.player.x = 0.5;
        swim_state.player.y = 10.0;
        swim_state.player.vx = 0.0;
        swim_state.player.vy = 0.0;
        swim_state.player.grounded = false;
        swim_state.apply_client_command(ClientCommand::SetMoveRight(true));
        swim_state.update(0, 0);
        let swim_vx = swim_state.player.vx.abs();

        assert!(swim_vx < walk_vx);
    }

    #[test]
    fn test_held_jump_in_lava_provides_meaningful_ascent() {
        let mut state = GameState::new();
        setup_player_in_lava_column(&mut state);
        state.apply_client_command(ClientCommand::SetJumpHeld(true));

        for _ in 0..8 {
            state.update(0, 0);
        }

        assert!(
            state.player.y < 9.85,
            "expected visible lava ascent, got y={}",
            state.player.y
        );
    }

    #[test]
    fn test_swimming_horizontal_movement_stays_responsive_over_multiple_ticks() {
        let mut state = GameState::new();
        configure_quiet_world(&mut state);
        for x in -8..=8 {
            for y in 0..20 {
                state.world.set_block(x, y, BlockType::Air);
            }
        }
        for x in -1..=4 {
            for y in 8..=11 {
                state.world.set_block(x, y, BlockType::Water(8));
            }
        }
        state.player.x = 0.5;
        state.player.y = 10.0;
        state.player.vx = 0.0;
        state.player.vy = 0.0;
        state.player.grounded = false;
        state.apply_client_command(ClientCommand::SetMoveRight(true));

        for _ in 0..4 {
            state.update(0, 0);
        }

        assert!(
            state.player.vx > 0.12,
            "expected responsive swim vx, got {}",
            state.player.vx
        );
        assert!(
            state.player.x > 0.86,
            "expected forward swim progress, got {}",
            state.player.x
        );
    }

    #[test]
    fn test_holding_jump_in_water_continues_swim_ascent() {
        let mut state = GameState::new();
        configure_quiet_world(&mut state);
        for x in -3..=3 {
            for y in 0..20 {
                state.world.set_block(x, y, BlockType::Air);
            }
        }
        for x in -1..=1 {
            for y in 8..=11 {
                state.world.set_block(x, y, BlockType::Water(8));
            }
        }

        state.player.x = 0.5;
        state.player.y = 10.0;
        state.player.vx = 0.0;
        state.player.vy = 0.08;
        state.player.grounded = false;
        state.apply_client_command(ClientCommand::SetJumpHeld(true));
        state.update(0, 0);

        assert!(state.player.y < 10.0);
        assert!(state.player.vy < 0.0);
    }

    #[test]
    fn test_releasing_jump_in_water_allows_player_to_sink_again() {
        let mut state = GameState::new();
        configure_quiet_world(&mut state);
        for x in -3..=3 {
            for y in 0..24 {
                state.world.set_block(x, y, BlockType::Air);
            }
        }
        for x in -1..=1 {
            for y in 8..=14 {
                state.world.set_block(x, y, BlockType::Water(8));
            }
        }

        state.player.x = 0.5;
        state.player.y = 11.0;
        state.player.vx = 0.0;
        state.player.vy = -0.16;
        state.player.grounded = false;

        for _ in 0..12 {
            state.update(0, 0);
        }

        assert!(state.player.y > 11.0);
        assert!(state.player.vy > 0.0);
    }

    #[test]
    fn test_holding_sneak_in_water_dives_downward() {
        let mut state = GameState::new();
        configure_quiet_world(&mut state);
        for x in -3..=3 {
            for y in 0..24 {
                state.world.set_block(x, y, BlockType::Air);
            }
        }
        for x in -1..=1 {
            for y in 8..=14 {
                state.world.set_block(x, y, BlockType::Water(8));
            }
        }

        state.player.x = 0.5;
        state.player.y = 11.0;
        state.player.vx = 0.0;
        state.player.vy = 0.0;
        state.player.grounded = false;
        state.apply_client_command(ClientCommand::SetSneakHeld(true));
        state.update(0, 0);

        assert!(state.player.y > 11.0);
        assert!(state.player.vy > 0.02);
    }

    #[test]
    fn test_holding_jump_in_narrow_water_column_uses_center_submersion() {
        let mut state = GameState::new();
        configure_quiet_world(&mut state);
        for x in -3..=4 {
            for y in 0..24 {
                state.world.set_block(x, y, BlockType::Air);
            }
        }
        for y in 8..=14 {
            state.world.set_block(1, y, BlockType::Water(8));
        }

        state.player.x = 1.78;
        state.player.y = 11.0;
        state.player.vx = 0.0;
        state.player.vy = 0.02;
        state.player.grounded = false;
        state.apply_client_command(ClientCommand::SetJumpHeld(true));
        state.update(0, 0);

        assert!(state.player.y < 11.0);
        assert!(state.player.vy < 0.0);
    }

    #[test]
    fn test_holding_jump_in_shallow_surface_water_does_not_enable_swim_ascent() {
        let mut state = GameState::new();
        configure_quiet_world(&mut state);
        for x in -3..=3 {
            for y in 0..20 {
                state.world.set_block(x, y, BlockType::Air);
            }
        }
        for x in -1..=1 {
            state.world.set_block(x, 10, BlockType::Water(8));
        }

        state.player.x = 0.5;
        state.player.y = 10.0;
        state.player.vx = 0.0;
        state.player.vy = 0.0;
        state.player.grounded = false;
        state.apply_client_command(ClientCommand::SetJumpHeld(true));
        state.update(0, 0);

        assert!(state.player.y >= 10.0);
        assert!(state.player.vy >= 0.0);
    }

    #[test]
    fn test_partial_water_jump_is_softened_to_prevent_surface_skimming() {
        let mut state = GameState::new();
        configure_quiet_world(&mut state);
        for x in -3..=6 {
            for y in 0..20 {
                state.world.set_block(x, y, BlockType::Air);
            }
        }
        for x in 0..=4 {
            state.world.set_block(x, 10, BlockType::Water(8));
        }

        state.player.x = 0.5;
        state.player.y = 10.0;
        state.player.vx = 0.48;
        state.player.vy = 0.0;
        state.player.grounded = false;
        state.apply_client_command(ClientCommand::QueueJump);
        state.update(0, 0);

        assert!(state.player.vy >= 0.0);
        assert!(state.player.vx.abs() < 0.34);
    }

    #[test]
    fn test_falling_into_water_column_cancels_fall_damage() {
        let mut state = GameState::new();
        configure_quiet_world(&mut state);

        for x in -4..=4 {
            for y in 0..60 {
                state.world.set_block(x, y, BlockType::Air);
            }
            state.world.set_block(x, 40, BlockType::Stone);
        }
        for x in -1..=1 {
            for y in 20..40 {
                state.world.set_block(x, y, BlockType::Water(8));
            }
        }

        state.player.x = 0.5;
        state.player.y = 5.0;
        state.player.vx = 0.0;
        state.player.vy = 0.0;
        state.player.grounded = false;
        state.player.fall_distance = 0.0;
        let starting_health = state.player.health;

        for _ in 0..500 {
            state.update(0, 0);
            if state.player.grounded {
                break;
            }
        }

        assert!(state.player.grounded);
        assert_eq!(state.player.health, starting_health);
        assert_eq!(state.player.fall_distance, 0.0);
        assert_eq!(state.player.y, 40.0);
    }

    #[test]
    fn test_movement_flags_persist_without_key_repeat_timeout() {
        let mut state = GameState::new();
        state.world.load_chunks_around(0);
        state.world.newly_generated_chunks.clear();
        state.zombies.clear();
        state.creepers.clear();
        state.skeletons.clear();
        state.spiders.clear();
        state.pigmen.clear();
        state.ghasts.clear();
        state.cows.clear();
        state.sheep.clear();
        state.fireballs.clear();
        state.lightning_bolts.clear();

        for x in -8..=8 {
            for y in 0..20 {
                state.world.set_block(x, y, BlockType::Air);
            }
            state.world.set_block(x, 10, BlockType::Stone);
        }

        state.player.x = 0.0;
        state.player.y = 10.0;
        state.player.vx = 0.0;
        state.player.vy = 0.0;
        state.player.grounded = true;
        state.moving_right = true;
        state.moving_left = false;

        state.update(0, 0);
        let x1 = state.player.x;
        state.update(0, 0);
        let x2 = state.player.x;

        assert!(x2 > x1);
        assert!(state.moving_right);
    }

    #[test]
    fn test_classic_ground_acceleration_is_gradual() {
        let mut state = GameState::new();
        configure_quiet_world(&mut state);

        for x in -8..=8 {
            for y in 0..20 {
                state.world.set_block(x, y, BlockType::Air);
            }
            state.world.set_block(x, 10, BlockType::Stone);
        }

        state.player.x = 0.0;
        state.player.y = 10.0;
        state.player.vx = 0.0;
        state.player.vy = 0.0;
        state.player.grounded = true;
        state.moving_right = true;

        state.update(0, 0);
        let first_tick_vx = state.player.vx;
        state.update(0, 0);
        let second_tick_vx = state.player.vx;

        assert!(first_tick_vx > 0.0);
        assert!(first_tick_vx < state.movement_tuning().walk_speed);
        assert!(second_tick_vx > first_tick_vx);
        assert!(second_tick_vx <= state.movement_tuning().walk_speed);
    }

    #[test]
    fn test_cycle_movement_profile_wraps() {
        let mut state = GameState::new();
        assert_eq!(state.movement_profile_name(), "Classic");

        state.cycle_movement_profile();
        assert_eq!(state.movement_profile_name(), "Smooth");
        state.cycle_movement_profile();
        assert_eq!(state.movement_profile_name(), "Agile");
        state.cycle_movement_profile();
        assert_eq!(state.movement_profile_name(), "Classic");
    }

    #[test]
    fn test_client_commands_toggle_inventory_and_clear_inputs() {
        let mut state = GameState::new();
        state.inventory_open = false;
        state.moving_left = true;
        state.moving_right = false;
        state.left_click_down = true;
        state.at_crafting_table = true;

        state.apply_client_command(ClientCommand::ToggleInventory);
        assert!(state.inventory_open);
        assert!(!state.moving_left);
        assert!(!state.moving_right);
        assert!(!state.left_click_down);

        state.apply_client_command(ClientCommand::ToggleInventory);
        assert!(!state.inventory_open);
        assert!(!state.at_crafting_table);
    }

    #[test]
    fn test_enchant_option_repairs_item_boosts_enchant_and_spends_levels() {
        let mut state = GameState::new();
        state.at_enchanting_table = true;
        state.inventory_open = true;
        state.hotbar_index = 0;
        state.inventory.slots[0] = Some(ItemStack {
            item_type: ItemType::IronSword,
            count: 1,
            durability: Some(40),
        });
        state.inventory_enchant_levels[0] = 0;
        state.set_total_experience(GameState::total_experience_for_level(8));
        let before_level = state.player.experience_level;

        assert!(state.can_apply_enchant_option(1));
        state.attempt_enchant_option(1);

        let new_durability = state.inventory.slots[0]
            .as_ref()
            .and_then(|stack| stack.durability)
            .unwrap_or(0);
        assert!(new_durability > 40);
        assert!(state.inventory_enchant_levels[0] >= 1);
        assert!(state.player.experience_level < before_level);
    }

    #[test]
    fn test_anvil_combine_merges_durability_and_keeps_best_enchant() {
        let mut state = GameState::new();
        state.at_anvil = true;
        state.inventory_open = true;
        state.hotbar_index = 0;
        state.inventory.slots[0] = Some(ItemStack {
            item_type: ItemType::IronSword,
            count: 1,
            durability: Some(60),
        });
        state.inventory.slots[1] = Some(ItemStack {
            item_type: ItemType::IronSword,
            count: 1,
            durability: Some(45),
        });
        state.inventory_enchant_levels[0] = 1;
        state.inventory_enchant_levels[1] = 3;
        state.set_total_experience(GameState::total_experience_for_level(7));
        let before_level = state.player.experience_level;

        assert!(state.can_apply_anvil_combine());
        state.attempt_anvil_combine();

        assert!(state.inventory.slots[1].is_none());
        assert_eq!(state.inventory_enchant_levels[1], 0);
        assert_eq!(state.inventory_enchant_levels[0], 3);
        let max_durability = ItemType::IronSword.max_durability().unwrap_or(0);
        let combined_durability = state.inventory.slots[0]
            .as_ref()
            .and_then(|stack| stack.durability)
            .unwrap_or(0);
        assert!(combined_durability <= max_durability);
        assert!(combined_durability > 60);
        assert!(state.player.experience_level < before_level);
    }

    #[test]
    fn test_brewing_option_turns_water_into_awkward_and_consumes_reagent() {
        let mut state = GameState::new();
        state.at_brewing_stand = true;
        state.inventory_open = true;
        state.hotbar_index = 0;
        state.inventory.slots[0] = Some(ItemStack {
            item_type: ItemType::NetherWart,
            count: 1,
            durability: None,
        });
        state.inventory.slots[1] = Some(ItemStack {
            item_type: ItemType::WaterBottle,
            count: 1,
            durability: None,
        });

        assert!(state.can_apply_brew_option(0));
        state.attempt_brew_option(0);

        assert!(state.inventory.has_item(ItemType::AwkwardPotion, 1));
        assert!(!state.inventory.has_item(ItemType::WaterBottle, 1));
        assert!(!state.inventory.has_item(ItemType::NetherWart, 1));
    }

    #[test]
    fn test_water_source_fills_glass_bottle() {
        let mut state = GameState::new();
        state.world.load_chunks_around(6);
        state.player.x = 5.5;
        state.player.y = 10.0;
        state.inventory.slots[0] = Some(ItemStack {
            item_type: ItemType::GlassBottle,
            count: 1,
            durability: None,
        });
        state.world.set_block(6, 9, BlockType::Water(8));

        state.interact_block(6, 9, false);

        assert!(state.inventory.has_item(ItemType::WaterBottle, 1));
        assert!(!state.inventory.has_item(ItemType::GlassBottle, 1));
    }

    #[test]
    fn test_breaking_water_does_not_drop_or_remove_fluid() {
        let mut state = GameState::new();
        state.world.load_chunks_around(0);
        for x in -4..=4 {
            for y in 0..20 {
                state.world.set_block(x, y, BlockType::Air);
            }
            state.world.set_block(x, 10, BlockType::Stone);
        }
        state.player.x = 0.5;
        state.player.y = 10.0;
        state.item_entities.clear();
        state.world.set_block(1, 9, BlockType::Water(8));

        state.interact_block(1, 9, true);

        assert_eq!(state.world.get_block(1, 9), BlockType::Water(8));
        assert!(
            state
                .item_entities
                .iter()
                .all(|item| item.item_type != ItemType::WaterBucket)
        );
    }

    #[test]
    fn test_bucket_fills_from_water_source_and_removes_source() {
        let mut state = GameState::new();
        state.world.load_chunks_around(0);
        for x in -4..=4 {
            for y in 0..20 {
                state.world.set_block(x, y, BlockType::Air);
            }
            state.world.set_block(x, 10, BlockType::Stone);
        }
        state.player.x = 0.5;
        state.player.y = 10.0;
        state.hotbar_index = 0;
        state.inventory.slots[0] = Some(ItemStack {
            item_type: ItemType::Bucket,
            count: 1,
            durability: None,
        });
        state.world.set_block(1, 9, BlockType::Water(8));

        state.interact_block(1, 9, false);

        assert_eq!(state.world.get_block(1, 9), BlockType::Air);
        assert_eq!(
            state.inventory.slots[0]
                .as_ref()
                .map(|stack| stack.item_type),
            Some(ItemType::WaterBucket)
        );
    }

    #[test]
    fn test_bucket_does_not_fill_from_flowing_water() {
        let mut state = GameState::new();
        state.world.load_chunks_around(0);
        for x in -4..=4 {
            for y in 0..20 {
                state.world.set_block(x, y, BlockType::Air);
            }
            state.world.set_block(x, 10, BlockType::Stone);
        }
        state.player.x = 0.5;
        state.player.y = 10.0;
        state.hotbar_index = 0;
        state.inventory.slots[0] = Some(ItemStack {
            item_type: ItemType::Bucket,
            count: 1,
            durability: None,
        });
        state.world.set_block(1, 9, BlockType::Water(6));

        state.interact_block(1, 9, false);

        assert_eq!(state.world.get_block(1, 9), BlockType::Water(6));
        assert_eq!(
            state.inventory.slots[0]
                .as_ref()
                .map(|stack| stack.item_type),
            Some(ItemType::Bucket)
        );
    }

    #[test]
    fn test_bucket_fills_from_lava_source_and_removes_source() {
        let mut state = GameState::new();
        state.world.load_chunks_around(0);
        for x in -4..=4 {
            for y in 0..20 {
                state.world.set_block(x, y, BlockType::Air);
            }
            state.world.set_block(x, 10, BlockType::Stone);
        }
        state.player.x = 0.5;
        state.player.y = 10.0;
        state.hotbar_index = 0;
        state.inventory.slots[0] = Some(ItemStack {
            item_type: ItemType::Bucket,
            count: 1,
            durability: None,
        });
        state.world.set_block(1, 9, BlockType::Lava(8));

        state.interact_block(1, 9, false);

        assert_eq!(state.world.get_block(1, 9), BlockType::Air);
        assert_eq!(
            state.inventory.slots[0]
                .as_ref()
                .map(|stack| stack.item_type),
            Some(ItemType::LavaBucket)
        );
    }

    #[test]
    fn test_clicking_solid_block_places_block_on_near_face() {
        let mut state = GameState::new();
        state.world.load_chunks_around(0);
        for x in -4..=4 {
            for y in 0..20 {
                state.world.set_block(x, y, BlockType::Air);
            }
            state.world.set_block(x, 10, BlockType::Stone);
        }
        state.world.set_block(2, 9, BlockType::Stone);
        state.player.x = 0.5;
        state.player.y = 10.0;
        state.hotbar_index = 0;
        state.inventory.slots[0] = Some(ItemStack {
            item_type: ItemType::Dirt,
            count: 1,
            durability: None,
        });

        state.interact_block(2, 9, false);

        assert_eq!(state.world.get_block(2, 9), BlockType::Stone);
        assert_eq!(state.world.get_block(1, 9), BlockType::Dirt);
        assert!(state.inventory.slots[0].is_none());
    }

    #[test]
    fn test_rising_player_can_place_block_in_current_feet_cell() {
        let mut state = GameState::new();
        state.world.load_chunks_around(0);
        for x in -4..=4 {
            for y in 0..20 {
                state.world.set_block(x, y, BlockType::Air);
            }
            state.world.set_block(x, 10, BlockType::Stone);
        }
        state.player.x = 0.5;
        state.player.y = 9.34;
        state.player.vy = -0.22;
        state.player.grounded = false;
        state.hotbar_index = 0;
        state.inventory.slots[0] = Some(ItemStack {
            item_type: ItemType::Dirt,
            count: 1,
            durability: None,
        });

        state.interact_block(0, 10, false);

        assert_eq!(state.world.get_block(0, 9), BlockType::Dirt);
        assert!(state.inventory.slots[0].is_none());
    }

    #[test]
    fn test_standing_player_cannot_place_block_in_current_feet_cell() {
        let mut state = GameState::new();
        state.world.load_chunks_around(0);
        for x in -4..=4 {
            for y in 0..20 {
                state.world.set_block(x, y, BlockType::Air);
            }
            state.world.set_block(x, 10, BlockType::Stone);
        }
        state.player.x = 0.5;
        state.player.y = 10.0;
        state.player.vy = 0.0;
        state.player.grounded = true;
        state.hotbar_index = 0;
        state.inventory.slots[0] = Some(ItemStack {
            item_type: ItemType::Dirt,
            count: 1,
            durability: None,
        });

        state.interact_block(0, 10, false);

        assert_eq!(state.world.get_block(0, 9), BlockType::Air);
        assert_eq!(
            state.inventory.slots[0]
                .as_ref()
                .map(|stack| stack.count)
                .unwrap_or(0),
            1
        );
    }

    #[test]
    fn test_wool_item_places_wool_block() {
        let mut state = GameState::new();
        state.world.load_chunks_around(0);
        for x in -4..=4 {
            for y in 0..20 {
                state.world.set_block(x, y, BlockType::Air);
            }
            state.world.set_block(x, 10, BlockType::Stone);
        }
        state.player.x = 0.5;
        state.player.y = 10.0;
        state.hotbar_index = 0;
        state.inventory.slots[0] = Some(ItemStack {
            item_type: ItemType::Wool,
            count: 1,
            durability: None,
        });

        state.interact_block(1, 9, false);

        assert_eq!(state.world.get_block(1, 9), BlockType::Wool);
        assert!(state.inventory.slots[0].is_none());
    }

    #[test]
    fn test_nether_wart_places_on_soul_sand_only() {
        let mut state = GameState::new();
        configure_quiet_world(&mut state);
        for x in -4..=4 {
            for y in 0..20 {
                state.world.set_block(x, y, BlockType::Air);
            }
        }
        state.player.x = 0.5;
        state.player.y = 10.0;
        state.hotbar_index = 0;
        state.inventory.slots[0] = Some(ItemStack {
            item_type: ItemType::NetherWart,
            count: 2,
            durability: None,
        });
        state.world.set_block(1, 10, BlockType::SoulSand);
        state.world.set_block(2, 10, BlockType::Dirt);

        state.interact_block(1, 9, false);
        assert_eq!(state.world.get_block(1, 9), BlockType::NetherWart(0));

        state.interact_block(2, 9, false);
        assert_eq!(state.world.get_block(2, 9), BlockType::Air);
        assert_eq!(
            state.inventory.slots[0].as_ref().map(|stack| stack.count),
            Some(1)
        );
    }

    #[test]
    fn test_breaking_mature_nether_wart_drops_multiple_items() {
        let mut state = GameState::new();
        configure_quiet_world(&mut state);
        for x in -4..=4 {
            for y in 0..20 {
                state.world.set_block(x, y, BlockType::Air);
            }
        }
        state.player.x = 0.5;
        state.player.y = 10.0;
        state.world.set_block(1, 10, BlockType::SoulSand);
        state.world.set_block(1, 9, BlockType::NetherWart(3));

        state.interact_block(1, 9, true);

        let wart_drops = state
            .item_entities
            .iter()
            .filter(|item| item.item_type == ItemType::NetherWart)
            .count();
        assert!(wart_drops >= 2, "wart drops were {wart_drops}");
        assert_eq!(state.world.get_block(1, 9), BlockType::Air);
    }

    #[test]
    fn test_air_placement_does_not_treat_water_as_support_anchor() {
        let mut state = GameState::new();
        state.world.load_chunks_around(0);
        for x in -4..=4 {
            for y in 0..20 {
                state.world.set_block(x, y, BlockType::Air);
            }
        }
        state.player.x = 0.5;
        state.player.y = 7.0;
        state.hotbar_index = 0;
        state.inventory.slots[0] = Some(ItemStack {
            item_type: ItemType::Dirt,
            count: 1,
            durability: None,
        });
        state.world.set_block(1, 9, BlockType::Water(8));

        state.interact_block(2, 9, false);

        assert_eq!(state.world.get_block(2, 9), BlockType::Air);
        assert_eq!(
            state.inventory.slots[0]
                .as_ref()
                .map(|stack| stack.count)
                .unwrap_or(0),
            1
        );
    }

    #[test]
    fn test_clicking_solid_block_with_food_still_eats() {
        let mut state = GameState::new();
        state.world.load_chunks_around(0);
        for x in -4..=4 {
            for y in 0..20 {
                state.world.set_block(x, y, BlockType::Air);
            }
            state.world.set_block(x, 10, BlockType::Stone);
        }
        state.world.set_block(1, 9, BlockType::Stone);
        state.player.x = 0.5;
        state.player.y = 10.0;
        state.player.hunger = 10.0;
        state.hotbar_index = 0;
        state.inventory.slots[0] = Some(ItemStack {
            item_type: ItemType::Bread,
            count: 1,
            durability: None,
        });

        state.interact_block(1, 9, false);

        assert_eq!(state.player.hunger, 15.0);
        assert!(state.inventory.slots[0].is_none());
    }

    #[test]
    fn test_placing_water_bucket_turns_into_empty_bucket() {
        let mut state = GameState::new();
        state.world.load_chunks_around(0);
        for x in -4..=4 {
            for y in 0..20 {
                state.world.set_block(x, y, BlockType::Air);
            }
            state.world.set_block(x, 10, BlockType::Stone);
        }
        state.player.x = 0.5;
        state.player.y = 10.0;
        state.hotbar_index = 0;
        state.inventory.slots[0] = Some(ItemStack {
            item_type: ItemType::WaterBucket,
            count: 1,
            durability: None,
        });

        state.interact_block(1, 9, false);

        assert!(matches!(state.world.get_block(1, 9), BlockType::Water(_)));
        assert_eq!(
            state.inventory.slots[0]
                .as_ref()
                .map(|stack| stack.item_type),
            Some(ItemType::Bucket)
        );
    }

    #[test]
    fn test_clicking_solid_block_with_water_bucket_places_on_near_face() {
        let mut state = GameState::new();
        state.world.load_chunks_around(0);
        for x in -4..=4 {
            for y in 0..20 {
                state.world.set_block(x, y, BlockType::Air);
            }
            state.world.set_block(x, 10, BlockType::Stone);
        }
        state.world.set_block(2, 9, BlockType::Stone);
        state.player.x = 0.5;
        state.player.y = 10.0;
        state.hotbar_index = 0;
        state.inventory.slots[0] = Some(ItemStack {
            item_type: ItemType::WaterBucket,
            count: 1,
            durability: None,
        });

        state.interact_block(2, 9, false);

        assert!(matches!(state.world.get_block(1, 9), BlockType::Water(_)));
        assert_eq!(
            state.inventory.slots[0]
                .as_ref()
                .map(|stack| stack.item_type),
            Some(ItemType::Bucket)
        );
    }

    #[test]
    fn test_water_does_not_block_line_of_sight_for_block_placement() {
        let mut state = GameState::new();
        state.world.load_chunks_around(0);
        for x in -4..=4 {
            for y in 0..20 {
                state.world.set_block(x, y, BlockType::Air);
            }
            state.world.set_block(x, 10, BlockType::Stone);
        }
        state.world.set_block(1, 9, BlockType::Water(8));
        state.world.set_block(2, 9, BlockType::Stone);
        state.player.x = 0.5;
        state.player.y = 10.0;
        state.hotbar_index = 0;
        state.inventory.slots[0] = Some(ItemStack {
            item_type: ItemType::Dirt,
            count: 1,
            durability: None,
        });

        state.interact_block(2, 9, false);

        assert_eq!(state.world.get_block(1, 9), BlockType::Dirt);
        assert!(state.inventory.slots[0].is_none());
    }

    #[test]
    fn test_item_entities_despawn_after_lifetime() {
        let mut state = GameState::new();
        state.world.load_chunks_around(0);
        state.game_rules.do_mob_spawning = false;
        state.world.newly_generated_chunks.clear();
        state.zombies.clear();
        state.creepers.clear();
        state.skeletons.clear();
        state.spiders.clear();
        state.pigmen.clear();
        state.ghasts.clear();
        state.endermen.clear();
        state.cows.clear();
        state.sheep.clear();
        state.pigs.clear();
        state.chickens.clear();
        state.squids.clear();
        state.wolves.clear();
        state.ocelots.clear();
        state.villagers.clear();
        state.experience_orbs.clear();
        state.arrows.clear();
        state.fireballs.clear();
        state.lightning_bolts.clear();

        for x in -8..=8 {
            for y in 0..20 {
                state.world.set_block(x, y, BlockType::Air);
            }
            state.world.set_block(x, 10, BlockType::Stone);
        }
        state.player.x = 0.5;
        state.player.y = 10.0;

        let mut old_item = ItemEntity::new(6.5, 9.5, ItemType::Cobblestone);
        old_item.age = ITEM_ENTITY_DESPAWN_TICKS + 1;
        state.item_entities.push(old_item);

        state.update(0, 0);

        assert!(state.item_entities.is_empty());
    }

    #[test]
    fn test_strength_potion_consumption_grants_melee_bonus_and_bottle() {
        let mut state = GameState::new();
        state.hotbar_index = 0;
        state.inventory.slots[0] = Some(ItemStack {
            item_type: ItemType::PotionStrength,
            count: 1,
            durability: None,
        });

        let damage_before = state.effective_held_damage(Some(ItemType::WoodSword));
        assert!(state.consume_held_potion_if_any());
        let damage_after = state.effective_held_damage(Some(ItemType::WoodSword));

        assert!(state.potion_strength_timer > 0);
        assert!(damage_after > damage_before);
        assert!(state.inventory.has_item(ItemType::GlassBottle, 1));
    }

    #[test]
    fn test_fire_resistance_blocks_lava_and_burning_damage() {
        let mut resistant = GameState::new();
        setup_player_in_lava_column(&mut resistant);
        resistant.potion_fire_resistance_timer = 80;
        let resistant_hp = resistant.player.health;
        for _ in 0..30 {
            resistant.update(0, 0);
        }
        assert!(
            resistant.player.health >= resistant_hp - 0.001,
            "fire resistance should block lava damage"
        );
        assert_eq!(resistant.player.burning_timer, 0);

        let mut normal = GameState::new();
        setup_player_in_lava_column(&mut normal);
        let normal_hp = normal.player.health;
        for _ in 0..30 {
            normal.update(0, 0);
        }
        assert!(
            normal.player.health < normal_hp,
            "without fire resistance, lava should damage player"
        );
    }

    #[test]
    fn test_double_tap_direction_starts_and_stops_sprint() {
        let mut state = GameState::new();
        state.player.hunger = 20.0;

        state.apply_client_command(ClientCommand::SetMoveRight(true));
        state.apply_client_command(ClientCommand::SetMoveRight(false));
        state.apply_client_command(ClientCommand::SetMoveRight(true));
        assert!(state.is_sprinting());

        state.apply_client_command(ClientCommand::SetMoveRight(false));
        assert!(!state.is_sprinting());
    }

    #[test]
    fn test_double_tap_sprint_requires_hunger_above_threshold() {
        let mut state = GameState::new();
        state.player.hunger = SPRINT_MIN_HUNGER;

        state.apply_client_command(ClientCommand::SetMoveRight(true));
        state.apply_client_command(ClientCommand::SetMoveRight(false));
        state.apply_client_command(ClientCommand::SetMoveRight(true));
        assert!(!state.is_sprinting());
    }

    #[test]
    fn test_sprint_movement_is_faster_than_walk() {
        let mut walk_state = GameState::new();
        walk_state.game_rules.do_mob_spawning = false;
        walk_state.world.load_chunks_around(0);
        for x in -8..=8 {
            for y in 0..20 {
                walk_state.world.set_block(x, y, BlockType::Air);
            }
            walk_state.world.set_block(x, 10, BlockType::Stone);
        }
        walk_state.player.x = 0.0;
        walk_state.player.y = 10.0;
        walk_state.player.vx = 0.0;
        walk_state.player.vy = 0.0;
        walk_state.player.grounded = true;
        walk_state.player.hunger = 20.0;
        walk_state.apply_client_command(ClientCommand::SetMoveRight(true));
        walk_state.update(0, 0);
        let walk_vx = walk_state.player.vx.abs();

        let mut sprint_state = GameState::new();
        sprint_state.game_rules.do_mob_spawning = false;
        sprint_state.world.load_chunks_around(0);
        for x in -8..=8 {
            for y in 0..20 {
                sprint_state.world.set_block(x, y, BlockType::Air);
            }
            sprint_state.world.set_block(x, 10, BlockType::Stone);
        }
        sprint_state.player.x = 0.0;
        sprint_state.player.y = 10.0;
        sprint_state.player.vx = 0.0;
        sprint_state.player.vy = 0.0;
        sprint_state.player.grounded = true;
        sprint_state.player.hunger = 20.0;
        sprint_state.apply_client_command(ClientCommand::SetMoveRight(true));
        sprint_state.apply_client_command(ClientCommand::SetMoveRight(false));
        sprint_state.apply_client_command(ClientCommand::SetMoveRight(true));
        assert!(sprint_state.is_sprinting());
        sprint_state.update(0, 0);
        let sprint_vx = sprint_state.player.vx.abs();

        assert!(sprint_vx > walk_vx);
    }

    #[test]
    fn test_client_commands_sneak_toggle_and_hold_stack() {
        let mut state = GameState::new();
        assert!(!state.player.sneaking);

        state.apply_client_command(ClientCommand::ToggleSneak);
        assert!(state.player.sneaking);

        state.apply_client_command(ClientCommand::SetSneakHeld(true));
        assert!(state.player.sneaking);

        state.apply_client_command(ClientCommand::ToggleSneak);
        assert!(state.player.sneaking);

        state.apply_client_command(ClientCommand::SetSneakHeld(false));
        assert!(!state.player.sneaking);
    }

    #[test]
    fn test_client_commands_can_toggle_individual_game_rules() {
        let mut state = GameState::new();
        assert_eq!(state.game_rules_preset_name(), "Vanilla");

        state.apply_client_command(ClientCommand::ToggleRuleKeepInventory);
        assert_eq!(state.game_rules_preset_name(), "KeepInv");

        state.apply_client_command(ClientCommand::ToggleRuleDaylightCycle);
        assert_eq!(state.game_rules_preset_name(), "Custom");
        let (_, day, _, keep_inv) = state.game_rule_flags();
        assert!(!day);
        assert!(keep_inv);
    }

    #[test]
    fn test_startup_splash_blocks_world_simulation_until_dismissed() {
        let mut state = GameState::new();
        state.startup_splash_active = true;
        state.startup_splash_ticks = 0;
        let start_age = state.player.age;

        state.update(0, 0);

        assert!(state.is_showing_startup_splash());
        assert_eq!(state.player.age, start_age);

        state.dismiss_startup_splash();
        state.update(0, 0);

        assert!(!state.is_showing_startup_splash());
        assert!(state.player.age > start_age);
    }

    #[test]
    fn test_startup_splash_auto_dismisses_after_timeout() {
        let mut state = GameState::new();
        state.startup_splash_active = true;
        state.startup_splash_ticks = 0;

        for _ in 0..STARTUP_SPLASH_AUTO_DISMISS_TICKS {
            state.update(0, 0);
        }

        assert!(!state.is_showing_startup_splash());
    }

    #[test]
    fn test_settings_menu_open_clears_live_inputs() {
        let mut state = GameState::new();
        state.inventory_open = true;
        state.moving_right = true;
        state.left_click_down = true;
        state.player.vx = 0.41;

        state.apply_client_command(ClientCommand::ToggleSettingsMenu);

        assert!(state.is_settings_menu_open());
        assert!(!state.inventory_open);
        assert!(!state.moving_left);
        assert!(!state.moving_right);
        assert!(!state.left_click_down);
        assert_eq!(state.player.vx, 0.0);
        assert_eq!(state.settings_menu_selected_index(), 0);
    }

    #[test]
    fn test_settings_menu_selection_wraps_and_applies_preset() {
        let mut state = GameState::new();
        state.apply_client_command(ClientCommand::ToggleSettingsMenu);
        assert!(state.is_settings_menu_open());

        state.apply_client_command(ClientCommand::SettingsMoveUp);
        assert_eq!(
            state.settings_menu_selected_index(),
            SETTINGS_MENU_ROW_CLOSE
        );

        state.apply_client_command(ClientCommand::SettingsMoveDown);
        assert_eq!(
            state.settings_menu_selected_index(),
            SETTINGS_MENU_ROW_DIFFICULTY
        );
        state.apply_client_command(ClientCommand::SettingsMoveDown);
        assert_eq!(
            state.settings_menu_selected_index(),
            SETTINGS_MENU_ROW_GAMERULE_PRESET
        );

        state.apply_client_command(ClientCommand::SettingsApply);
        assert!(state.is_settings_menu_open());
        assert_eq!(state.game_rules_preset_name(), "KeepInv");
    }

    #[test]
    fn test_settings_menu_close_entry_closes_menu() {
        let mut state = GameState::new();
        state.apply_client_command(ClientCommand::ToggleSettingsMenu);
        state.apply_client_command(ClientCommand::SettingsMoveUp);
        state.apply_client_command(ClientCommand::SettingsApply);

        assert!(!state.is_settings_menu_open());
    }

    #[test]
    fn test_settings_menu_pauses_world_update_tick() {
        let mut state = GameState::new();
        state.respawn_grace_ticks = 3;

        state.apply_client_command(ClientCommand::ToggleSettingsMenu);
        state.update(0, 0);
        assert_eq!(state.respawn_grace_ticks, 3);

        state.apply_client_command(ClientCommand::ToggleSettingsMenu);
        state.update(0, 0);
        assert_eq!(state.respawn_grace_ticks, 2);
    }

    #[test]
    fn test_jump_buffer_and_coyote_time_allow_jump_shortly_after_edge() {
        let mut state = GameState::new();
        state.world.load_chunks_around(0);
        state.world.newly_generated_chunks.clear();
        state.zombies.clear();
        state.creepers.clear();
        state.skeletons.clear();
        state.spiders.clear();
        state.pigmen.clear();
        state.ghasts.clear();
        state.cows.clear();
        state.sheep.clear();
        state.fireballs.clear();
        state.lightning_bolts.clear();

        for x in -8..=8 {
            for y in 0..20 {
                state.world.set_block(x, y, BlockType::Air);
            }
            state.world.set_block(x, 10, BlockType::Stone);
        }

        state.time_of_day = 0.0;
        state.player.x = 0.5;
        state.player.y = 9.4;
        state.player.vx = 0.0;
        state.player.vy = 0.0;
        state.player.grounded = false;
        state.coyote_ticks = 2;
        state.queue_jump();

        state.update(0, 0);

        assert!(state.player.vy < -0.2);
        assert!(state.jump_buffer_ticks == 0);
    }

    #[test]
    fn test_jump_against_single_block_edge_keeps_forward_progress() {
        let mut state = GameState::new();
        state.world.load_chunks_around(0);
        state.world.newly_generated_chunks.clear();
        state.zombies.clear();
        state.creepers.clear();
        state.skeletons.clear();
        state.spiders.clear();
        state.pigmen.clear();
        state.ghasts.clear();
        state.cows.clear();
        state.sheep.clear();
        state.pigs.clear();
        state.chickens.clear();
        state.wolves.clear();
        state.ocelots.clear();
        state.squids.clear();
        state.fireballs.clear();
        state.lightning_bolts.clear();

        state.overworld_hostile_spawn_timer = u16::MAX;
        state.overworld_passive_spawn_timer = u16::MAX;
        state.overworld_squid_spawn_timer = u16::MAX;
        state.overworld_wolf_spawn_timer = u16::MAX;
        state.overworld_ocelot_spawn_timer = u16::MAX;
        state.nether_spawn_timer = u16::MAX;
        state.end_spawn_timer = u16::MAX;

        for x in -12..=12 {
            for y in 0..24 {
                state.world.set_block(x, y, BlockType::Air);
            }
            state.world.set_block(x, 10, BlockType::Stone);
        }
        // A one-block rise directly ahead.
        state.world.set_block(2, 9, BlockType::Stone);

        state.time_of_day = 0.0;
        state.player.x = 1.55;
        state.player.y = 10.0;
        state.player.vx = 0.0;
        state.player.vy = 0.0;
        state.player.grounded = true;
        state.apply_client_command(ClientCommand::SetMoveRight(true));
        state.queue_jump();

        for _ in 0..10 {
            state.update(4, 8);
        }

        assert!(state.player.x > 2.2);
    }

    #[test]
    fn test_jump_from_standstill_does_not_get_near_full_run_speed_boost() {
        let mut state = GameState::new();
        state.world.load_chunks_around(0);
        state.world.newly_generated_chunks.clear();
        state.zombies.clear();
        state.creepers.clear();
        state.skeletons.clear();
        state.spiders.clear();
        state.pigmen.clear();
        state.ghasts.clear();
        state.cows.clear();
        state.sheep.clear();
        state.pigs.clear();
        state.chickens.clear();
        state.wolves.clear();
        state.ocelots.clear();
        state.squids.clear();
        state.fireballs.clear();
        state.lightning_bolts.clear();

        for x in -8..=8 {
            for y in 0..20 {
                state.world.set_block(x, y, BlockType::Air);
            }
            state.world.set_block(x, 10, BlockType::Stone);
        }

        state.player.x = 0.5;
        state.player.y = 10.0;
        state.player.vx = 0.0;
        state.player.vy = 0.0;
        state.player.grounded = true;
        state.apply_client_command(ClientCommand::SetMoveRight(true));
        state.queue_jump();

        state.update(0, 0);

        assert!(state.player.vx > state.movement_tuning().walk_speed * 0.6);
        assert!(state.player.vx < state.movement_tuning().walk_speed * 0.8);
    }

    #[test]
    fn test_melee_attack_does_not_hit_entity_through_wall() {
        let mut state = GameState::new();
        state.world.load_chunks_around(0);
        state.world.newly_generated_chunks.clear();
        state.zombies.clear();
        state.creepers.clear();
        state.skeletons.clear();
        state.spiders.clear();
        state.pigmen.clear();
        state.ghasts.clear();
        state.cows.clear();
        state.sheep.clear();
        state.fireballs.clear();
        state.lightning_bolts.clear();

        for x in -8..=8 {
            for y in 0..20 {
                state.world.set_block(x, y, BlockType::Air);
            }
            state.world.set_block(x, 10, BlockType::Stone);
        }
        for y in 8..=10 {
            state.world.set_block(2, y, BlockType::Stone);
        }

        state.time_of_day = 0.0;
        state.player.x = 0.5;
        state.player.y = 10.0;
        state.player.attack_timer = 0;
        state.left_click_down = true;
        state.zombies.push(Zombie::new(3.5, 10.0));
        let start_health = state.zombies[0].health;

        state.update(3, 9);

        assert!(!state.zombies.is_empty());
        assert_eq!(state.zombies[0].health, start_health);
    }

    #[test]
    fn test_melee_attack_hits_entity_with_clear_line_of_sight() {
        let mut state = GameState::new();
        state.world.load_chunks_around(0);
        state.world.newly_generated_chunks.clear();
        state.zombies.clear();
        state.creepers.clear();
        state.skeletons.clear();
        state.spiders.clear();
        state.pigmen.clear();
        state.ghasts.clear();
        state.cows.clear();
        state.sheep.clear();
        state.fireballs.clear();
        state.lightning_bolts.clear();

        for x in -8..=8 {
            for y in 0..20 {
                state.world.set_block(x, y, BlockType::Air);
            }
            state.world.set_block(x, 10, BlockType::Stone);
        }

        state.time_of_day = 0.0;
        state.player.x = 0.5;
        state.player.y = 10.0;
        state.player.attack_timer = 0;
        state.left_click_down = true;
        state.zombies.push(Zombie::new(2.5, 10.0));
        let start_health = state.zombies[0].health;

        state.update(2, 9);

        assert!(!state.zombies.is_empty());
        assert!(state.zombies[0].health < start_health);
    }

    #[test]
    fn test_skeleton_burns_in_daylight_when_exposed() {
        let mut state = GameState::new();
        state.world.load_chunks_around(0);
        state.world.newly_generated_chunks.clear();
        state.zombies.clear();
        state.creepers.clear();
        state.skeletons.clear();
        state.spiders.clear();
        state.pigmen.clear();
        state.ghasts.clear();
        state.cows.clear();
        state.sheep.clear();
        state.fireballs.clear();
        state.lightning_bolts.clear();

        for x in -8..=8 {
            for y in 0..20 {
                state.world.set_block(x, y, BlockType::Air);
            }
            state.world.set_block(x, 10, BlockType::Stone);
        }

        state.skeletons.push(Skeleton::new(0.0, 10.0));
        state.time_of_day = 8000.0;
        let start_health = state.skeletons[0].health;

        for _ in 0..40 {
            state.update(0, 0);
            if state.skeletons.is_empty() {
                break;
            }
        }

        assert!(!state.skeletons.is_empty());
        assert!(state.skeletons[0].health < start_health);
        assert!(state.skeletons[0].burning_timer > 0);
    }

    #[test]
    fn test_precipitation_type_depends_on_weather_and_biome() {
        let mut state = GameState::new();
        state.current_dimension = Dimension::Overworld;
        state.weather = WeatherType::Rain;

        let mut desert_x = None;
        let mut tundra_x = None;
        let mut taiga_x = None;
        let mut wet_biome_x = None;
        for x in -20_000..=20_000 {
            match state.world.get_biome(x) {
                BiomeType::Desert if desert_x.is_none() => desert_x = Some(x),
                BiomeType::Tundra if tundra_x.is_none() => tundra_x = Some(x),
                BiomeType::Taiga if taiga_x.is_none() => taiga_x = Some(x),
                BiomeType::Forest
                | BiomeType::Plains
                | BiomeType::Swamp
                | BiomeType::Jungle
                | BiomeType::ExtremeHills
                | BiomeType::Ocean
                | BiomeType::River
                    if wet_biome_x.is_none() =>
                {
                    wet_biome_x = Some(x)
                }
                _ => {}
            }
            if desert_x.is_some()
                && tundra_x.is_some()
                && taiga_x.is_some()
                && wet_biome_x.is_some()
            {
                break;
            }
        }

        let desert_x = desert_x.expect("Expected to find a desert biome sample");
        let tundra_x = tundra_x.expect("Expected to find a tundra biome sample");
        let taiga_x = taiga_x.expect("Expected to find a taiga biome sample");
        let wet_biome_x = wet_biome_x.expect("Expected to find a wet biome sample");

        assert_eq!(state.precipitation_at(desert_x), PrecipitationType::None);
        assert_eq!(state.precipitation_at(tundra_x), PrecipitationType::Snow);
        assert_eq!(state.precipitation_at(taiga_x), PrecipitationType::Snow);
        assert_eq!(state.precipitation_at(wet_biome_x), PrecipitationType::Rain);

        state.weather = WeatherType::Clear;
        assert_eq!(state.precipitation_at(tundra_x), PrecipitationType::None);
        assert_eq!(state.precipitation_at(taiga_x), PrecipitationType::None);
        assert_eq!(state.precipitation_at(wet_biome_x), PrecipitationType::None);
    }

    #[test]
    fn test_weather_forces_clear_in_nether() {
        let mut state = GameState::new();
        state.current_dimension = Dimension::Nether;
        state.weather = WeatherType::Thunderstorm;
        state.thunder_flash_timer = 3;
        let mut rng = rand::thread_rng();

        state.update_weather(&mut rng);

        assert_eq!(state.weather, WeatherType::Clear);
        assert_eq!(state.thunder_flash_timer, 0);
        assert_eq!(state.precipitation_at(0), PrecipitationType::None);
        assert_eq!(state.weather_rain_intensity, 0.0);
        assert_eq!(state.weather_thunder_intensity, 0.0);
    }

    #[test]
    fn test_weather_mix_ramps_up_in_rain() {
        let mut state = GameState::new();
        state.current_dimension = Dimension::Overworld;
        state.weather = WeatherType::Rain;
        state.weather_timer = 100;
        state.weather_rain_intensity = 0.0;
        state.weather_thunder_intensity = 0.0;
        let mut rng = rand::thread_rng();

        state.update_weather(&mut rng);

        assert!(state.weather_rain_intensity > 0.01);
        assert!(state.weather_thunder_intensity < 0.05);
    }

    #[test]
    fn test_throw_eye_guidance_consumes_eye_and_sets_direction() {
        let mut state = GameState::new();
        state.world.load_chunks_around(0);
        state.world.newly_generated_chunks.clear();
        for x in -4..=4 {
            for y in 0..20 {
                state.world.set_block(x, y, BlockType::Air);
            }
            state.world.set_block(x, 10, BlockType::Stone);
        }

        state.current_dimension = Dimension::Overworld;
        state.player.x = 0.5;
        state.player.y = 10.0;
        state.hotbar_index = 0;
        state.inventory.slots[0] = Some(ItemStack {
            item_type: ItemType::EyeOfEnder,
            count: 2,
            durability: None,
        });

        state.interact_block(1, 9, false);

        let remaining = state.inventory.slots[0]
            .as_ref()
            .map(|s| s.count)
            .unwrap_or(0);
        assert_eq!(remaining, 1);
        assert!(state.eye_guidance_timer > 0);
        assert_eq!(state.eye_guidance_dir, 1);
        assert!(state.eye_guidance_distance > 0);
    }

    #[test]
    fn test_throw_eye_guidance_is_disabled_outside_overworld() {
        let mut state = GameState::new();
        state.world.load_chunks_around(0);
        state.world.newly_generated_chunks.clear();
        for x in -4..=4 {
            for y in 0..20 {
                state.world.set_block(x, y, BlockType::Air);
            }
            state.world.set_block(x, 10, BlockType::Stone);
        }

        state.current_dimension = Dimension::Nether;
        state.player.x = 0.5;
        state.player.y = 10.0;
        state.hotbar_index = 0;
        state.inventory.slots[0] = Some(ItemStack {
            item_type: ItemType::EyeOfEnder,
            count: 1,
            durability: None,
        });

        state.interact_block(1, 9, false);

        let remaining = state.inventory.slots[0]
            .as_ref()
            .map(|s| s.count)
            .unwrap_or(0);
        assert_eq!(remaining, 1);
        assert_eq!(state.eye_guidance_timer, 0);
    }

    #[test]
    fn test_eye_guidance_finds_the_portal_depth_from_both_directions() {
        let mut state = GameState::network_view();
        state.eye_guidance_timer = 300;
        state.player.x = STRONGHOLD_CENTER_X as f64 - 100.0;
        state.player.y = 40.0;
        assert!(state.eye_guidance_hint().unwrap().contains("right"));
        state.player.x = STRONGHOLD_CENTER_X as f64 + 100.0;
        assert!(state.eye_guidance_hint().unwrap().contains("left"));
        state.player.x = STRONGHOLD_CENTER_X as f64 + 0.5;
        assert!(state.eye_guidance_hint().unwrap().contains("below"));
        state.player.y = 110.0;
        assert!(state.eye_guidance_hint().unwrap().contains("above"));
        state.player.y = STRONGHOLD_PORTAL_INNER_Y as f64 + 1.9;
        assert!(
            state
                .eye_guidance_hint()
                .unwrap()
                .contains("portal room nearby")
        );
        state.eye_guidance_timer = 0;
        assert!(state.eye_guidance_hint().is_none());
        state.eye_guidance_timer = 300;
        state.current_dimension = Dimension::Nether;
        assert!(state.eye_guidance_hint().is_none());
    }

    #[test]
    fn test_eye_guidance_accepts_a_far_cursor_without_loading_its_target_chunk() {
        let mut state = GameState::network_view();
        state.player.x = 0.5;
        state.player.y = 40.0;
        state.inventory.slots[0] = Some(ItemStack {
            item_type: ItemType::EyeOfEnder,
            count: 2,
            durability: None,
        });
        assert!(!state.world.is_chunk_loaded_at(2000));
        state.interact_block(2000, 12, false);
        assert_eq!(state.inventory.slots[0].as_ref().unwrap().count, 1);
        assert_eq!(state.eye_guidance_timer, 300);
        assert!(state.eye_guidance_hint().unwrap().contains("right"));
        assert!(!state.world.is_chunk_loaded_at(2000));
    }

    #[test]
    fn test_progression_spawn_point_prefers_surface_over_hole_column() {
        let mut state = GameState::new();
        state.world.load_chunks_for_spawn_search(0, 64);
        for x in -64..=64 {
            for y in 0..70 {
                state.world.set_block(x, y, BlockType::Air);
            }
            let ground_y = if (-2..=2).contains(&x) { 46 } else { 32 };
            state.world.set_block(x, ground_y, BlockType::Stone);
        }

        let (spawn_x, spawn_y) = GameState::progression_spawn_point(&mut state.world, 0);
        assert!(!(-2..=2).contains(&spawn_x));
        assert!(spawn_y < 40.0);
    }

    #[test]
    fn test_progression_spawn_point_avoids_roofed_cave_columns() {
        let mut state = GameState::new();
        state.world.load_chunks_for_spawn_search(0, 64);
        for x in -64..=64 {
            for y in 0..70 {
                state.world.set_block(x, y, BlockType::Air);
            }
            state.world.set_block(x, 34, BlockType::Stone);
        }

        // Cave-like column near center: ground exists, but a roof blocks sky.
        state.world.set_block(0, 32, BlockType::Stone);
        for y in 10..=26 {
            state.world.set_block(0, y, BlockType::Stone);
        }

        let (spawn_x, _spawn_y) = GameState::progression_spawn_point(&mut state.world, 0);
        assert_ne!(spawn_x, 0);
    }

    #[test]
    fn test_progression_spawn_point_avoids_narrow_shoulder_over_drop() {
        let mut state = GameState::new();
        state.world.load_chunks_for_spawn_search(0, 64);
        for x in -64..=64 {
            for y in 0..70 {
                state.world.set_block(x, y, BlockType::Air);
            }
            let ground_y = if (-3..=-2).contains(&x) || (2..=3).contains(&x) {
                48
            } else {
                32
            };
            state.world.set_block(x, ground_y, BlockType::Stone);
        }

        let (spawn_x, spawn_y) = GameState::progression_spawn_point(&mut state.world, 0);
        assert!(spawn_x.abs() >= 4);
        assert!(spawn_y < 40.0);
    }

    #[test]
    fn test_progression_spawn_point_prefers_grass_over_sand_near_center() {
        let mut state = GameState::new();
        state.world.load_chunks_for_spawn_search(0, 64);
        for x in -64..=64 {
            for y in 0..70 {
                state.world.set_block(x, y, BlockType::Air);
            }
            let surface = if (-2..=2).contains(&x) {
                BlockType::Sand
            } else {
                BlockType::Grass
            };
            state.world.set_block(x, 32, surface);
        }

        let (spawn_x, spawn_y) = GameState::progression_spawn_point(&mut state.world, 0);
        assert!(!(-2..=2).contains(&spawn_x));
        assert!(spawn_y < 40.0);
    }

    #[test]
    fn test_progression_spawn_point_avoids_tree_top_columns() {
        let mut state = GameState::new();
        state.world.load_chunks_for_spawn_search(0, 64);
        for x in -64..=64 {
            for y in 0..70 {
                state.world.set_block(x, y, BlockType::Air);
            }
            state.world.set_block(x, 32, BlockType::Grass);
        }

        for y in 29..=31 {
            state.world.set_block(0, y, BlockType::Wood);
        }
        state.world.set_block(-1, 28, BlockType::Leaves);
        state.world.set_block(0, 28, BlockType::Leaves);
        state.world.set_block(1, 28, BlockType::Leaves);

        let (spawn_x, spawn_y) = GameState::progression_spawn_point(&mut state.world, 0);
        assert_ne!(spawn_x, 0);
        assert!(spawn_y < 40.0);
    }

    #[test]
    fn test_progression_spawn_point_prefers_grass_over_ice_near_center() {
        let mut state = GameState::new();
        state.world.load_chunks_for_spawn_search(0, 64);
        for x in -64..=64 {
            for y in 0..70 {
                state.world.set_block(x, y, BlockType::Air);
            }
            let surface = if (-2..=2).contains(&x) {
                BlockType::Ice
            } else {
                BlockType::Grass
            };
            state.world.set_block(x, 32, surface);
        }

        let (spawn_x, spawn_y) = GameState::progression_spawn_point(&mut state.world, 0);
        assert!(!(-2..=2).contains(&spawn_x));
        assert!(spawn_y < 40.0);
    }

    #[test]
    fn test_nether_spawn_surface_uses_nether_ground_blocks() {
        let mut state = GameState::new();
        state.world.load_chunks_around(0);
        for x in -2..=2 {
            for y in 0..(CHUNK_HEIGHT as i32 - 2) {
                state.world.set_block(x, y, BlockType::Air);
            }
        }

        state.world.set_block(0, 60, BlockType::Stone);
        state.world.set_block(1, 60, BlockType::Netherrack);
        state.world.set_block(2, 60, BlockType::SoulSand);

        assert_eq!(state.find_nether_spawn_surface_for_hostile(0), None);
        assert!(state.find_nether_spawn_surface_for_hostile(1).is_some());
        assert!(state.find_nether_spawn_surface_for_hostile(2).is_some());
    }

    #[test]
    fn test_nether_blaze_air_spawn_accepts_fortress_floor_blocks() {
        let mut state = GameState::new();
        state.world.load_chunks_around(0);
        for x in -2..=2 {
            for y in 0..(CHUNK_HEIGHT as i32 - 2) {
                state.world.set_block(x, y, BlockType::Air);
            }
        }

        state.world.set_block(-1, 60, BlockType::StoneStairs);
        state.world.set_block(0, 60, BlockType::StoneBricks);
        state.world.set_block(1, 60, BlockType::StoneSlab);

        assert!(state.find_nether_air_spawn_for_blaze(-1).is_some());
        assert!(state.find_nether_air_spawn_for_blaze(0).is_some());
        assert!(state.find_nether_air_spawn_for_blaze(1).is_some());
    }

    #[test]
    fn test_silverfish_spawner_spawns_silverfish_nearby() {
        use rand::{SeedableRng, rngs::StdRng};

        let mut state = GameState::network_view();
        let center_x = crate::world::STRONGHOLD_CENTER_X;
        state.current_dimension = Dimension::Overworld;
        for cx in center_x.div_euclid(CHUNK_WIDTH as i32) - 6
            ..=center_x.div_euclid(CHUNK_WIDTH as i32) + 6
        {
            state
                .world
                .apply_chunk_column_snapshot(cx, &vec![BlockType::Air; CHUNK_WIDTH * CHUNK_HEIGHT]);
        }
        state.world.newly_generated_chunks.clear();
        state.silverfish.clear();

        state.player.x = center_x as f64 + 0.5;
        // Keep player far enough from all candidate spawn columns so near-player rejection
        // does not introduce non-determinism in this test.
        state.player.y = 78.0;

        for x in (center_x - 96)..=(center_x + 96) {
            for y in (crate::world::STRONGHOLD_ROOM_TOP_Y - 10)
                ..=(crate::world::STRONGHOLD_ROOM_BOTTOM_Y + 4)
            {
                if state.world.get_block(x, y) == BlockType::SilverfishSpawner {
                    state.world.set_block(x, y, BlockType::Air);
                }
            }
        }

        for x in (center_x - 10)..=(center_x + 10) {
            for y in 2..110 {
                state.world.set_block(x, y, BlockType::Air);
            }
            state.world.set_block(x, 90, BlockType::StoneBricks);
        }
        // Place spawner above the floor so every target_x in [-4, 4] shares the same first
        // valid spawn surface at y=90 -> spawn_y=89.9.
        state
            .world
            .set_block(center_x, 92, BlockType::SilverfishSpawner);

        state.silverfish_spawner_timer = 0;
        let mut rng = StdRng::seed_from_u64(42);
        state.update_silverfish_spawners(&mut rng);

        assert!(!state.silverfish.is_empty());
        let spawned = &state.silverfish[0];
        assert!((spawned.x - (center_x as f64 + 0.5)).abs() <= 6.5);
        assert!((spawned.y - 89.9).abs() < 0.2);
    }

    #[test]
    fn test_blaze_spawner_spawns_blaze_nearby() {
        use rand::{SeedableRng, rngs::StdRng};

        let mut state = GameState::new();
        state.current_dimension = Dimension::Nether;
        state.world = World::new_for_dimension(Dimension::Nether);

        let mut spawner_x = None;
        for x in -512..=512 {
            if state.world.is_nether_fortress_zone(x, 52) {
                spawner_x = Some(x);
                break;
            }
        }
        let spawner_x = spawner_x.expect("expected fortress zone for blaze spawner test");
        state.world.load_chunks_around(spawner_x);
        state.world.newly_generated_chunks.clear();
        state.blazes.clear();

        let mut zone_min_y = i32::MAX;
        let mut zone_max_y = i32::MIN;
        for y in 4..(CHUNK_HEIGHT as i32 - 4) {
            if state.world.is_nether_fortress_zone(spawner_x, y) {
                zone_min_y = zone_min_y.min(y);
                zone_max_y = zone_max_y.max(y);
            }
        }
        assert!(zone_min_y <= zone_max_y);
        let floor_y = (zone_max_y - 1).clamp(14, 92);

        for x in (spawner_x - 10)..=(spawner_x + 10) {
            for y in 2..(CHUNK_HEIGHT as i32 - 2) {
                state.world.set_block(x, y, BlockType::Air);
            }
            state.world.set_block(x, floor_y, BlockType::StoneBricks);
        }
        state
            .world
            .set_block(spawner_x, floor_y + 3, BlockType::BlazeSpawner);

        state.player.x = spawner_x as f64 + 18.5;
        state.player.y = (floor_y - 1) as f64;
        state.blaze_spawner_timer = 0;
        let mut rng = StdRng::seed_from_u64(1337);
        state.update_blaze_spawners(&mut rng);

        assert!(!state.blazes.is_empty());
        let spawned = &state.blazes[0];
        assert!((spawned.x - (spawner_x as f64 + 0.5)).abs() <= 8.5);
    }

    #[test]
    fn test_zombie_spawner_spawns_zombie_nearby() {
        use rand::{SeedableRng, rngs::StdRng};

        let mut state = GameState::new();
        state.current_dimension = Dimension::Overworld;
        state.world = World::new_for_dimension(Dimension::Overworld);
        state.world.load_chunks_around(0);
        state.world.newly_generated_chunks.clear();
        state.zombies.clear();

        for x in -10..=10 {
            for y in 2..110 {
                state.world.set_block(x, y, BlockType::Air);
            }
            state.world.set_block(x, 90, BlockType::StoneBricks);
        }
        state.world.set_block(0, 92, BlockType::ZombieSpawner);

        state.player.x = 18.5;
        state.player.y = 89.0;
        state.dungeon_spawner_timer = 0;
        let mut rng = StdRng::seed_from_u64(4242);
        state.update_dungeon_spawners(&mut rng);

        assert!(!state.zombies.is_empty());
        let spawned = &state.zombies[0];
        assert!((spawned.x - 0.5).abs() <= 6.5);
        assert!((spawned.y - 89.9).abs() < 0.2);
    }

    #[test]
    fn test_skeleton_spawner_spawns_skeleton_nearby() {
        use rand::{SeedableRng, rngs::StdRng};

        let mut state = GameState::new();
        state.current_dimension = Dimension::Overworld;
        state.world = World::new_for_dimension(Dimension::Overworld);
        state.world.load_chunks_around(0);
        state.world.newly_generated_chunks.clear();
        state.skeletons.clear();

        for x in -10..=10 {
            for y in 2..110 {
                state.world.set_block(x, y, BlockType::Air);
            }
            state.world.set_block(x, 90, BlockType::StoneBricks);
        }
        state.world.set_block(0, 92, BlockType::SkeletonSpawner);

        state.player.x = 18.5;
        state.player.y = 89.0;
        state.dungeon_spawner_timer = 0;
        let mut rng = StdRng::seed_from_u64(5252);
        state.update_dungeon_spawners(&mut rng);

        assert!(!state.skeletons.is_empty());
        let spawned = &state.skeletons[0];
        assert!((spawned.x - 0.5).abs() <= 6.5);
        assert!((spawned.y - 89.9).abs() < 0.2);
    }

    #[test]
    fn test_blaze_spawner_respects_local_cluster_limit() {
        use rand::{SeedableRng, rngs::StdRng};

        let mut state = GameState::new();
        state.current_dimension = Dimension::Nether;
        state.world = World::new_for_dimension(Dimension::Nether);

        let mut spawner_x = None;
        for x in -512..=512 {
            if state.world.is_nether_fortress_zone(x, 52) {
                spawner_x = Some(x);
                break;
            }
        }
        let spawner_x = spawner_x.expect("expected fortress zone for blaze cluster test");
        state.world.load_chunks_around(spawner_x);
        state.world.newly_generated_chunks.clear();
        state.blazes.clear();

        for x in (spawner_x - 8)..=(spawner_x + 8) {
            for y in 2..(CHUNK_HEIGHT as i32 - 2) {
                state.world.set_block(x, y, BlockType::Air);
            }
            state.world.set_block(x, 90, BlockType::StoneBricks);
        }
        state
            .world
            .set_block(spawner_x, 86, BlockType::BlazeSpawner);
        state.blazes.push(Blaze::new(spawner_x as f64 + 0.5, 89.0));
        state.blazes.push(Blaze::new(spawner_x as f64 + 2.5, 89.0));

        state.player.x = spawner_x as f64 + 16.5;
        state.player.y = 89.0;
        state.blaze_spawner_timer = 0;
        let mut rng = StdRng::seed_from_u64(7);
        state.update_blaze_spawners(&mut rng);

        assert_eq!(state.blazes.len(), 2);
    }

    #[test]
    fn test_nether_ambient_roll_thresholds_bias_hot_fortress_zones_toward_pigmen() {
        assert_eq!(
            GameState::nether_ambient_roll_thresholds(false, false),
            (56, 82)
        );
        assert_eq!(
            GameState::nether_ambient_roll_thresholds(true, false),
            (40, 66)
        );
        assert_eq!(
            GameState::nether_ambient_roll_thresholds(true, true),
            (50, 78)
        );
    }

    #[test]
    fn test_trim_far_nether_mobs_removes_far_entities() {
        let mut state = GameState::new();
        state.player.x = 0.0;
        state.player.y = 10.0;

        state
            .pigmen
            .push(ZombiePigman::new(NETHER_DESPAWN_DIST_SQ.sqrt() + 5.0, 10.0));
        state.pigmen.push(ZombiePigman::new(3.0, 10.0));
        state
            .ghasts
            .push(Ghast::new(NETHER_DESPAWN_DIST_SQ.sqrt() * 2.0, 15.0));
        state.ghasts.push(Ghast::new(5.0, 15.0));
        state
            .blazes
            .push(Blaze::new(NETHER_DESPAWN_DIST_SQ.sqrt() * 2.0, 18.0));
        state.blazes.push(Blaze::new(6.0, 18.0));

        state.trim_far_nether_mobs();

        assert_eq!(state.pigmen.len(), 1);
        assert_eq!(state.ghasts.len(), 1);
        assert_eq!(state.blazes.len(), 1);
    }

    #[test]
    fn test_trim_far_overworld_mobs_removes_far_entities() {
        let mut state = GameState::new();
        state.current_dimension = Dimension::Overworld;
        state.player.x = 0.0;
        state.player.y = 10.0;

        state.zombies.push(Zombie::new(
            OVERWORLD_HOSTILE_DESPAWN_DIST_SQ.sqrt() + 10.0,
            10.0,
        ));
        state.zombies.push(Zombie::new(5.0, 10.0));
        state.cows.push(Cow::new(
            OVERWORLD_PASSIVE_DESPAWN_DIST_SQ.sqrt() + 10.0,
            10.0,
        ));
        state.cows.push(Cow::new(6.0, 10.0));
        state.squids.push(Squid::new(
            OVERWORLD_AQUATIC_DESPAWN_DIST_SQ.sqrt() + 10.0,
            11.0,
        ));
        state.squids.push(Squid::new(7.0, 11.0));

        state.trim_far_overworld_mobs();

        assert_eq!(state.zombies.len(), 1);
        assert_eq!(state.cows.len(), 1);
        assert_eq!(state.squids.len(), 1);
    }

    #[test]
    fn test_end_enderman_cap_scales_with_dragon_progression() {
        let mut state = GameState::new();
        state.dragon_defeated = false;
        assert_eq!(state.target_end_enderman_cap(), END_PRE_DRAGON_ENDERMAN_CAP);

        state.dragon_defeated = true;
        assert_eq!(state.target_end_enderman_cap(), END_ENDERMAN_CAP);
    }

    #[test]
    fn test_hostile_spawn_distance_gate_rejects_near_player() {
        let mut state = GameState::new();
        state.player.x = 0.5;
        state.player.y = 20.0;

        assert!(state.is_spawn_too_close_to_player(2.0, 20.1, NETHER_GROUND_SPAWN_MIN_DIST_SQ,));
        assert!(!state.is_spawn_too_close_to_player(18.0, 20.0, NETHER_GROUND_SPAWN_MIN_DIST_SQ,));
    }

    #[test]
    fn test_lightning_strike_charges_creeper_and_damages_player() {
        let mut state = GameState::new();
        state.world.load_chunks_around(0);
        state.world.newly_generated_chunks.clear();
        for x in -6..=6 {
            for y in 0..20 {
                state.world.set_block(x, y, BlockType::Air);
            }
            state.world.set_block(x, 10, BlockType::Stone);
        }
        state.weather = WeatherType::Thunderstorm;
        state.current_dimension = Dimension::Overworld;
        state.player.x = 0.5;
        state.player.y = 10.0;
        let player_health_before = state.player.health;

        state.creepers.push(Creeper::new(1.0, 10.0));
        state.apply_lightning_strike(1, 10);

        assert!(state.player.health < player_health_before);
        assert!(!state.creepers.is_empty());
        assert!(state.creepers[0].charged);
        assert!(!state.lightning_bolts.is_empty());
    }

    #[test]
    fn test_lightning_strike_primes_tnt() {
        let mut state = GameState::new();
        state.world.load_chunks_around(0);
        state.world.newly_generated_chunks.clear();
        for x in -4..=4 {
            for y in 0..20 {
                state.world.set_block(x, y, BlockType::Air);
            }
            state.world.set_block(x, 10, BlockType::Stone);
        }

        state.world.set_block(2, 10, BlockType::Tnt);
        state.apply_lightning_strike(1, 10);

        assert!(matches!(
            state.world.get_block(2, 10),
            BlockType::PrimedTnt(_)
        ));
    }

    #[test]
    fn test_collect_world_explosion_drops_spawns_item_entities() {
        let mut state = GameState::new();
        state.item_entities.clear();
        state
            .world
            .recent_explosion_block_losses
            .push((2, 8, BlockType::Stone, true));

        state.collect_world_explosion_drops();

        assert!(
            state
                .item_entities
                .iter()
                .any(|item| item.item_type == ItemType::Cobblestone)
        );
        assert!(state.world.recent_explosion_block_losses.is_empty());
    }

    #[test]
    fn test_tnt_explosion_damages_and_knocks_back_player() {
        let mut state = GameState::new();
        state.world.load_chunks_around(0);
        state.world.newly_generated_chunks.clear();
        state.zombies.clear();
        state.creepers.clear();
        state.skeletons.clear();
        state.spiders.clear();
        state.pigmen.clear();
        state.ghasts.clear();
        state.cows.clear();
        state.sheep.clear();
        state.fireballs.clear();
        state.lightning_bolts.clear();

        for x in -8..=8 {
            for y in 0..20 {
                state.world.set_block(x, y, BlockType::Air);
            }
            state.world.set_block(x, 10, BlockType::Stone);
        }

        state.player.x = 0.0;
        state.player.y = 10.0;
        state.player.vx = 0.0;
        state.player.vy = 0.0;
        state.player.grounded = true;
        let start_health = state.player.health;

        state.world.set_block(1, 9, BlockType::PrimedTnt(0));
        state.update(0, 0);
        state.update(0, 0);

        assert!(state.player.health < start_health);
        assert!(state.player.vx.abs() > 0.01 || state.player.vy.abs() > 0.01);
    }

    #[test]
    fn test_tnt_explosion_damages_nearby_zombie() {
        let mut state = GameState::new();
        state.world.load_chunks_around(0);
        state.world.newly_generated_chunks.clear();
        state.zombies.clear();
        state.creepers.clear();
        state.skeletons.clear();
        state.spiders.clear();
        state.pigmen.clear();
        state.ghasts.clear();
        state.cows.clear();
        state.sheep.clear();
        state.fireballs.clear();
        state.lightning_bolts.clear();

        for x in -8..=8 {
            for y in 0..20 {
                state.world.set_block(x, y, BlockType::Air);
            }
            state.world.set_block(x, 10, BlockType::Stone);
        }

        state.player.x = 0.0;
        state.player.y = 10.0;
        state.zombies.push(Zombie::new(1.0, 10.0));
        let start_health = state.zombies[0].health;

        state.world.set_block(3, 9, BlockType::PrimedTnt(0));
        state.update(0, 0);
        state.update(0, 0);

        assert!(state.zombies.is_empty() || state.zombies[0].health < start_health);
    }

    #[test]
    fn test_sneak_placing_piston_creates_sticky_piston() {
        let mut state = GameState::new();
        state.world.load_chunks_around(0);
        state.world.newly_generated_chunks.clear();

        for x in -8..=8 {
            for y in 0..20 {
                state.world.set_block(x, y, BlockType::Air);
            }
            state.world.set_block(x, 10, BlockType::Stone);
        }

        state.player.x = 0.0;
        state.player.y = 10.0;
        state.player.sneaking = true;
        state.hotbar_index = 0;
        state.inventory.slots[0] = Some(ItemStack {
            item_type: ItemType::Piston,
            count: 1,
            durability: None,
        });

        state.interact_block(1, 9, false);

        assert_eq!(
            state.world.get_block(1, 9),
            BlockType::StickyPiston {
                extended: false,
                facing_right: true
            }
        );
    }

    #[test]
    fn test_placing_repeater_uses_player_facing_direction() {
        let mut state = GameState::new();
        state.world.load_chunks_around(0);
        state.world.newly_generated_chunks.clear();

        for x in -8..=8 {
            for y in 0..20 {
                state.world.set_block(x, y, BlockType::Air);
            }
            state.world.set_block(x, 10, BlockType::Stone);
        }

        state.player.x = 0.0;
        state.player.y = 10.0;
        state.hotbar_index = 0;
        state.inventory.slots[0] = Some(ItemStack {
            item_type: ItemType::RedstoneRepeater,
            count: 1,
            durability: None,
        });

        state.interact_block(1, 9, false);

        assert_eq!(
            state.world.get_block(1, 9),
            BlockType::RedstoneRepeater {
                powered: false,
                delay: 1,
                facing_right: true,
                timer: 0,
                target_powered: false
            }
        );
    }

    #[test]
    fn test_interacting_repeater_cycles_delay_and_wraps() {
        let mut state = GameState::new();
        state.world.load_chunks_around(0);
        state.world.newly_generated_chunks.clear();

        for x in -8..=8 {
            for y in 0..20 {
                state.world.set_block(x, y, BlockType::Air);
            }
            state.world.set_block(x, 10, BlockType::Stone);
        }
        state.world.set_block(
            1,
            9,
            BlockType::RedstoneRepeater {
                powered: false,
                delay: 1,
                facing_right: true,
                timer: 0,
                target_powered: false,
            },
        );

        state.player.x = 0.0;
        state.player.y = 10.0;
        for expected_delay in [2, 3, 4, 1] {
            state.interact_block(1, 9, false);
            assert_eq!(
                state.world.get_block(1, 9),
                BlockType::RedstoneRepeater {
                    powered: false,
                    delay: expected_delay,
                    facing_right: true,
                    timer: 0,
                    target_powered: false
                }
            );
        }
    }

    #[test]
    fn test_sneak_interacting_repeater_flips_direction() {
        let mut state = GameState::new();
        state.world.load_chunks_around(0);
        state.world.newly_generated_chunks.clear();

        for x in -8..=8 {
            for y in 0..20 {
                state.world.set_block(x, y, BlockType::Air);
            }
            state.world.set_block(x, 10, BlockType::Stone);
        }
        state.world.set_block(
            1,
            9,
            BlockType::RedstoneRepeater {
                powered: false,
                delay: 3,
                facing_right: true,
                timer: 2,
                target_powered: true,
            },
        );

        state.player.x = 0.0;
        state.player.y = 10.0;
        state.player.sneaking = true;
        state.interact_block(1, 9, false);

        assert_eq!(
            state.world.get_block(1, 9),
            BlockType::RedstoneRepeater {
                powered: false,
                delay: 3,
                facing_right: false,
                timer: 0,
                target_powered: false
            }
        );
    }

    #[test]
    fn test_activate_portal_from_valid_obsidian_frame() {
        let mut state = GameState::new();
        state.world.load_chunks_around(0);
        state.world.newly_generated_chunks.clear();

        for x in -4..=4 {
            for y in 0..20 {
                state.world.set_block(x, y, BlockType::Air);
            }
            state.world.set_block(x, 10, BlockType::Stone);
        }

        // 4x5 frame, interior bottom-left at (0, 7)
        for x in -1..=2 {
            state.world.set_block(x, 6, BlockType::Obsidian);
            state.world.set_block(x, 10, BlockType::Obsidian);
        }
        for y in 6..=10 {
            state.world.set_block(-1, y, BlockType::Obsidian);
            state.world.set_block(2, y, BlockType::Obsidian);
        }

        state.player.x = 0.5;
        state.player.y = 10.0;
        state.hotbar_index = 0;
        state.inventory.slots[0] = Some(ItemStack {
            item_type: ItemType::FlintAndSteel,
            count: 1,
            durability: Some(65),
        });
        state.interact_block(-1, 9, false);

        assert_eq!(state.world.get_block(0, 7), BlockType::NetherPortal);
        assert_eq!(state.world.get_block(1, 8), BlockType::NetherPortal);
        assert_eq!(state.world.get_block(0, 9), BlockType::NetherPortal);
        assert_eq!(
            state.inventory.slots[0]
                .as_ref()
                .and_then(|stack| stack.durability),
            Some(64)
        );
    }

    #[test]
    fn test_portal_frame_break_extinguishes_the_whole_interior() {
        let mut state = GameState::new();
        configure_quiet_world(&mut state);
        state.ensure_nether_portal_at(0, 12);
        state.world.set_block(-1, 10, BlockType::Air);
        for x in 0..=1 {
            for y in 9..=11 {
                assert_eq!(state.world.get_block(x, y), BlockType::Air);
            }
        }
    }

    #[test]
    fn test_end_portal_frame_can_be_broken_repaired_and_reactivated() {
        let mut state = GameState::new();
        configure_quiet_world(&mut state);
        for x in -1..=2 {
            for y in 8..=11 {
                state.world.set_block(
                    x,
                    y,
                    if x == -1 || x == 2 || y == 8 || y == 11 {
                        BlockType::EndPortalFrame { filled: true }
                    } else {
                        BlockType::Air
                    },
                );
            }
        }
        assert!(state.try_activate_end_portal_from_frame(-1, 10));
        state.world.set_block(-1, 10, BlockType::Air);
        for x in 0..=1 {
            for y in 9..=10 {
                assert_eq!(state.world.get_block(x, y), BlockType::Air);
            }
        }
        state
            .world
            .set_block(-1, 10, BlockType::EndPortalFrame { filled: true });
        assert!(state.try_activate_end_portal_from_frame(-1, 10));
        assert_eq!(state.world.get_block(1, 10), BlockType::EndPortal);
    }

    #[test]
    fn test_cornerless_portal_ignites_and_shows_travel_and_cooldown_hints() {
        let mut state = GameState::new();
        configure_quiet_world(&mut state);
        for x in -4..=4 {
            for y in 0..12 {
                state.world.set_block(x, y, BlockType::Air);
            }
        }
        state.ensure_nether_portal_at(0, 12);
        // Extinguish, remove optional corners, then ignite with the ordinary command.
        state.world.set_block(-1, 10, BlockType::Air);
        state.world.set_block(-1, 10, BlockType::Obsidian);
        for (x, y) in [(-1, 8), (2, 8), (-1, 12), (2, 12)] {
            state.world.set_block(x, y, BlockType::Air);
        }
        state.player.x = 0.5;
        state.player.y = 12.0;
        state.inventory.slots[0] = Some(GameState::fresh_item_stack(ItemType::FlintAndSteel, 1));
        state.hotbar_index = 0;
        state.apply_client_command(ClientCommand::UseAt(-1, 10));
        assert_eq!(state.nether_portal_anchor_for_block(1, 9), Some((0, 12)));
        assert_eq!(
            state.portal_interaction_hint(0, 10),
            Some("R-click / F: travel")
        );
        state.portal_cooldown = 40;
        assert_eq!(
            state.portal_interaction_hint(0, 10),
            Some("Portal recharging...")
        );
    }

    #[test]
    fn test_portal_surface_cannot_be_mined_into_a_broken_half_portal() {
        let mut state = GameState::new();
        configure_quiet_world(&mut state);
        for x in -4..=4 {
            for y in 0..12 {
                state.world.set_block(x, y, BlockType::Air);
            }
        }
        state.ensure_nether_portal_at(0, 12);
        state.player.x = 0.5;
        state.player.y = 12.0;
        state.interact_block(0, 10, true);
        assert_eq!(state.world.get_block(0, 10), BlockType::NetherPortal);
        state.world.set_block(0, 10, BlockType::EndPortal);
        state.interact_block(0, 10, true);
        assert_eq!(state.world.get_block(0, 10), BlockType::EndPortal);
    }

    #[test]
    fn test_return_portal_preserves_workshop_and_glowstone() {
        let mut state = GameState::new();
        configure_quiet_world(&mut state);
        for x in 8..=28 {
            for y in 0..20 {
                state.world.set_block(x, y, BlockType::Air);
            }
            state.world.set_block(x, 10, BlockType::Stone);
        }
        state.ensure_nether_portal_at(16, 10);
        // A roof must not make an intentionally linked underground portal invalid.
        for x in 8..=28 {
            state.world.set_block(x, 4, BlockType::StoneBricks);
        }
        assert_eq!(
            state.find_existing_nether_portal_near(16, 2),
            Some((16, 10))
        );
        state.world.set_block(20, 9, BlockType::Glowstone);
        state.world.set_block(21, 9, BlockType::Chest);
        state
            .world
            .ensure_chest_inventory(21, 9, 27)
            .unwrap()
            .add_item(ItemType::Diamond, 3);
        state.player.x = 16.5;
        state.player.y = 10.0;
        state.player.fall_distance = 100.0;
        state.apply_client_command(ClientCommand::UseAt(16, 8));
        assert_eq!(state.current_dimension, Dimension::Nether);
        assert_eq!(state.player.fall_distance, 0.0);
        let (x, y) = state.find_existing_nether_portal_near(2, 8).unwrap();
        state.player.x = x as f64 + 0.5;
        state.player.y = y as f64;
        state.portal_cooldown = 0;
        state.apply_client_command(ClientCommand::UseAt(x, y - 2));
        assert_eq!(state.current_dimension, Dimension::Overworld);
        assert!(
            (state.player.y - 9.9).abs() < 0.01,
            "return must use the original underground frame"
        );
        assert_eq!(state.world.get_block(20, 9), BlockType::Glowstone);
        assert_eq!(state.world.get_block(21, 9), BlockType::Chest);
        assert_eq!(
            state.world.chest_inventory(21, 9).unwrap().slots[0]
                .as_ref()
                .unwrap()
                .count,
            3
        );
    }

    #[test]
    fn test_end_arrival_preserves_lights_and_storage_beside_a_safe_landing() {
        let mut state = GameState::new();
        configure_quiet_world(&mut state);
        state.current_dimension = Dimension::End;
        state.world = state.world.for_dimension(Dimension::End);
        let (x, y) = state.find_end_arrival_site();
        state.world.set_block(x - 2, y - 1, BlockType::Chest);
        state.world.set_block(x + 2, y - 1, BlockType::Glowstone);
        state.world.save_all();
        state.current_dimension = Dimension::Overworld;
        state.world = state.world.for_dimension(Dimension::Overworld);
        state.world.load_chunks_around(0);

        state.transfer_overworld_end_dimension(Dimension::End);

        assert_eq!(state.current_dimension, Dimension::End);
        assert_eq!(state.world.get_block(x - 2, y - 1), BlockType::Chest);
        assert_eq!(state.world.get_block(x + 2, y - 1), BlockType::Glowstone);
        assert!(GameState::is_safe_spawn_column_base(
            &state.world,
            state.player.x.floor() as i32,
            state.player.y.ceil() as i32,
            false
        ));
    }

    #[test]
    fn test_saved_portal_link_loads_a_frame_outside_the_mapped_chunk_radius() {
        let mut state = GameState::new();
        configure_quiet_world(&mut state);
        state.world.load_chunks_around(1024);
        state.ensure_nether_portal_at(1024, 12);
        state.world.save_all();
        state.world = state.world.for_dimension(Dimension::Overworld);
        state.world.load_chunks_around(0);
        state.remember_nether_portal_link(Dimension::Nether, (0, 12), (1024, 12));

        assert_eq!(
            state.linked_nether_portal_anchor(Dimension::Nether, (0, 12)),
            Some((1024, 12))
        );
    }

    #[test]
    fn test_blocked_portal_arrival_uses_the_opening_without_erasing_surroundings() {
        let mut state = GameState::new();
        configure_quiet_world(&mut state);
        for x in -8..=8 {
            for y in 7..=13 {
                state.world.set_block(x, y, BlockType::StoneBricks);
            }
        }
        state.ensure_nether_portal_at(0, 12);
        let arrival = state.existing_portal_arrival(0, 12);
        assert_eq!(arrival, (0.5, 11.9));
        assert_eq!(state.world.get_block(3, 10), BlockType::StoneBricks);
        assert_eq!(state.world.get_block(-2, 10), BlockType::StoneBricks);
    }

    #[test]
    fn test_glowstone_can_be_placed_and_mined_with_a_drop() {
        let mut state = GameState::new();
        configure_quiet_world(&mut state);
        for x in -4..=4 {
            for y in 0..10 {
                state.world.set_block(x, y, BlockType::Air);
            }
            state.world.set_block(x, 10, BlockType::Stone);
        }
        state.player.x = 0.5;
        state.player.y = 10.0;
        state.inventory.slots[0] = Some(ItemStack {
            item_type: ItemType::Glowstone,
            count: 1,
            durability: None,
        });
        state.hotbar_index = 0;
        state.apply_client_command(ClientCommand::UseAt(1, 9));
        assert_eq!(state.world.get_block(1, 9), BlockType::Glowstone);
        assert!(state.inventory.slots[0].is_none());
        for _ in 0..10 {
            state.interact_block(1, 9, true);
        }
        assert_eq!(state.world.get_block(1, 9), BlockType::Air);
        assert!(
            state
                .item_entities
                .iter()
                .any(|drop| drop.item_type == ItemType::Glowstone)
        );
    }

    #[test]
    fn test_obsidian_frame_does_not_activate_without_flint_and_steel() {
        let mut state = GameState::new();
        state.world.load_chunks_around(0);
        state.world.newly_generated_chunks.clear();

        for x in -4..=4 {
            for y in 0..20 {
                state.world.set_block(x, y, BlockType::Air);
            }
            state.world.set_block(x, 10, BlockType::Stone);
        }

        for x in -1..=2 {
            state.world.set_block(x, 6, BlockType::Obsidian);
            state.world.set_block(x, 10, BlockType::Obsidian);
        }
        for y in 6..=10 {
            state.world.set_block(-1, y, BlockType::Obsidian);
            state.world.set_block(2, y, BlockType::Obsidian);
        }

        state.player.x = 0.5;
        state.player.y = 10.0;
        state.interact_block(-1, 10, false);

        assert_eq!(state.world.get_block(0, 7), BlockType::Air);
        assert_eq!(state.world.get_block(1, 8), BlockType::Air);
    }

    #[test]
    fn test_portal_transfer_moves_player_to_nether() {
        let mut state = GameState::new();
        state.world.load_chunks_around(0);
        state.world.newly_generated_chunks.clear();
        state.zombies.clear();
        state.creepers.clear();
        state.skeletons.clear();
        state.spiders.clear();
        state.pigmen.clear();
        state.ghasts.clear();
        state.cows.clear();
        state.sheep.clear();
        state.item_entities.clear();
        state.arrows.clear();
        state.fireballs.clear();
        state.lightning_bolts.clear();

        for x in 8..=24 {
            for y in 0..20 {
                state.world.set_block(x, y, BlockType::Air);
            }
            state.world.set_block(x, 10, BlockType::Stone);
        }

        state.ensure_nether_portal_at(16, 10);
        state.player.x = 16.5;
        state.player.y = 10.0;
        state.interact_block(16, 8, false);

        assert_eq!(state.current_dimension, Dimension::Nether);
        assert_eq!(state.world.dimension, Dimension::Nether);
        let (portal_x, _base_y) = state
            .find_existing_nether_portal_near(16_i32.div_euclid(8), PORTAL_SEARCH_RADIUS)
            .expect("expected a Nether portal near mapped target");
        assert!((state.player.x - (portal_x as f64 + 7.5)).abs() < 1.0);
        assert!(state.player_portal_kind().is_none());
    }

    #[test]
    fn test_clicking_active_obsidian_frame_uses_nether_portal() {
        let mut state = GameState::new();
        configure_quiet_world(&mut state);
        for x in 12..=20 {
            for y in 0..20 {
                state.world.set_block(x, y, BlockType::Air);
            }
            state.world.set_block(x, 10, BlockType::Stone);
        }

        state.ensure_nether_portal_at(16, 10);
        state.player.x = 16.5;
        state.player.y = 10.0;
        state.portal_cooldown = 0;

        state.interact_block(15, 6, false);

        assert_eq!(state.current_dimension, Dimension::Nether);
    }

    #[test]
    fn test_nether_portal_anchor_is_stable_from_right_inner_column() {
        let mut state = GameState::new();
        configure_quiet_world(&mut state);
        for x in 12..=20 {
            for y in 0..20 {
                state.world.set_block(x, y, BlockType::Air);
            }
            state.world.set_block(x, 10, BlockType::Stone);
        }

        state.ensure_nether_portal_at(16, 10);
        state.player.x = 17.25;
        state.player.y = 10.0;

        assert_eq!(state.player_portal_kind(), Some(PortalKind::Nether));
        assert_eq!(state.player_nether_portal_anchor(), Some((16, 10)));
    }

    #[test]
    fn test_returning_through_same_nether_portal_reuses_overworld_frame() {
        let mut state = GameState::new();
        configure_quiet_world(&mut state);
        for x in 8..=28 {
            for y in 0..20 {
                state.world.set_block(x, y, BlockType::Air);
            }
            state.world.set_block(x, 10, BlockType::Stone);
        }

        state.ensure_nether_portal_at(16, 10);
        state.player.x = 17.25;
        state.player.y = 10.0;
        state.portal_cooldown = 0;
        state.portal_timer = 0;

        state.interact_block(17, 8, false);

        assert_eq!(state.current_dimension, Dimension::Nether);
        let (nether_portal_x, nether_base_y) = state
            .find_existing_nether_portal_near(2, 8)
            .expect("expected generated Nether portal");

        state.player.x = nether_portal_x as f64 + 1.25;
        state.player.y = nether_base_y as f64 - 0.1;
        state.player.vx = 0.0;
        state.player.vy = 0.0;
        state.player.grounded = false;
        state.portal_cooldown = 0;
        state.portal_timer = 0;

        state.interact_block(nether_portal_x, nether_base_y - 2, false);

        assert_eq!(state.current_dimension, Dimension::Overworld);
        let mut portal_anchors = Vec::new();
        for x in 14..=18 {
            for y in 0..20 {
                if let Some(anchor) = state.nether_portal_anchor_for_block(x, y) {
                    portal_anchors.push(anchor);
                }
            }
        }
        portal_anchors.sort_unstable();
        portal_anchors.dedup();

        assert_eq!(portal_anchors.len(), 1);
        assert_eq!(portal_anchors[0].0, 16);
        assert_eq!(
            state.find_existing_nether_portal_near(16, 2),
            Some(portal_anchors[0])
        );
        assert_eq!(state.find_existing_nether_portal_near(24, 2), None);
    }

    #[test]
    fn test_overworld_portal_arrival_site_avoids_tree_top_columns() {
        let mut state = GameState::new();
        configure_quiet_world(&mut state);
        let target_x = 16;

        state.current_dimension = Dimension::Overworld;
        state.world = World::new_for_dimension(Dimension::Overworld);
        state.world.load_chunks_for_spawn_search(target_x, 32);
        state.world.newly_generated_chunks.clear();
        for x in (target_x - 20)..=(target_x + 20) {
            for y in 0..40 {
                state.world.set_block(x, y, BlockType::Air);
            }
            state.world.set_block(x, 28, BlockType::Stone);
        }
        for y in 22..28 {
            state.world.set_block(target_x, y, BlockType::Wood);
        }

        let (portal_x, base_y) = state.find_overworld_portal_arrival_site(target_x, 20);
        assert_ne!(portal_x, target_x);
        assert_eq!(base_y, 28);

        let (arrival_x, arrival_y) = state.build_portal_arrival_vestibule(portal_x, base_y);
        assert!((arrival_x - (portal_x as f64 + 4.5)).abs() < 0.01);
        assert!((arrival_y - 27.9).abs() < 0.01);
    }

    #[test]
    fn test_overworld_portal_arrival_vestibule_does_not_build_overhead_cap() {
        let mut state = GameState::new();
        configure_quiet_world(&mut state);

        state.current_dimension = Dimension::Overworld;
        state.world = World::new_for_dimension(Dimension::Overworld);
        state.world.load_chunks_around(0);
        state.world.newly_generated_chunks.clear();
        for x in -12..=12 {
            for y in 0..40 {
                state.world.set_block(x, y, BlockType::Air);
            }
            state.world.set_block(x, 28, BlockType::Stone);
        }

        state.build_portal_arrival_vestibule(0, 28);

        for wx in -3..=5 {
            assert_eq!(state.world.get_block(wx, 23), BlockType::Air);
        }
    }

    #[test]
    fn test_quick_travel_reuses_nearby_portal_anchor_for_roundtrip() {
        let mut state = GameState::new();
        configure_quiet_world(&mut state);

        state.current_dimension = Dimension::Overworld;
        state.world = World::new_for_dimension(Dimension::Overworld);
        state.world.load_chunks_around(0);
        state.world.newly_generated_chunks.clear();
        for x in -24..=32 {
            for y in 0..24 {
                state.world.set_block(x, y, BlockType::Air);
            }
            state.world.set_block(x, 10, BlockType::Stone);
        }

        state.ensure_nether_portal_at(16, 10);
        state.player.x = 20.5;
        state.player.y = 9.9;
        state.quick_travel_to_dimension(Dimension::Nether);
        assert_eq!(state.current_dimension, Dimension::Nether);

        let nether_anchor = state
            .find_existing_nether_portal_near(16_i32.div_euclid(8), PORTAL_SEARCH_RADIUS)
            .expect("expected generated Nether portal");
        state.player.x = nether_anchor.0 as f64 + 7.5;
        state.player.y = nether_anchor.1 as f64 - 0.1;

        state.quick_travel_to_dimension(Dimension::Overworld);
        assert_eq!(state.current_dimension, Dimension::Overworld);
        assert_eq!(
            state.find_existing_nether_portal_near(16, 4),
            Some((16, 10))
        );
        assert_eq!(state.find_existing_nether_portal_near(80, 12), None);
    }

    #[test]
    fn test_overworld_portal_search_prefers_grounded_portal_over_elevated_one() {
        let mut state = GameState::new();
        configure_quiet_world(&mut state);
        let center_x = 64;

        state.current_dimension = Dimension::Overworld;
        state.world = World::new_for_dimension(Dimension::Overworld);
        state.world.load_chunks_for_spawn_search(center_x, 48);
        state.world.newly_generated_chunks.clear();
        for x in (center_x - 32)..=(center_x + 32) {
            for y in 0..48 {
                state.world.set_block(x, y, BlockType::Air);
            }
            state.world.set_block(x, 30, BlockType::Stone);
        }

        state.ensure_nether_portal_at(center_x, 18);
        state.ensure_nether_portal_at(center_x + 12, 30);

        assert_eq!(
            state.find_existing_nether_portal_near(center_x, 20),
            Some((center_x + 12, 30))
        );
    }

    #[test]
    fn test_overworld_portal_arrival_site_avoids_narrow_floating_plateaus() {
        let mut state = GameState::new();
        configure_quiet_world(&mut state);
        let target_x = 48;

        state.current_dimension = Dimension::Overworld;
        state.world = World::new_for_dimension(Dimension::Overworld);
        state.world.load_chunks_for_spawn_search(target_x, 40);
        state.world.newly_generated_chunks.clear();

        for x in (target_x - 24)..=(target_x + 24) {
            for y in 0..48 {
                state.world.set_block(x, y, BlockType::Air);
            }
            state.world.set_block(x, 30, BlockType::Stone);
        }

        for x in (target_x - 2)..=(target_x + 2) {
            state.world.set_block(x, 18, BlockType::Stone);
        }

        let (portal_x, base_y) = state.find_overworld_portal_arrival_site(target_x, 20);
        assert_eq!(base_y, 30);
        assert!((portal_x - target_x).abs() >= 3);
    }

    #[test]
    fn test_overworld_portal_search_rejects_only_floating_portal() {
        let mut state = GameState::new();
        configure_quiet_world(&mut state);
        let portal_x = 72;

        state.current_dimension = Dimension::Overworld;
        state.world = World::new_for_dimension(Dimension::Overworld);
        state.world.load_chunks_for_spawn_search(portal_x, 40);
        state.world.newly_generated_chunks.clear();

        for x in (portal_x - 24)..=(portal_x + 24) {
            for y in 0..48 {
                state.world.set_block(x, y, BlockType::Air);
            }
            state.world.set_block(x, 30, BlockType::Stone);
        }

        state.ensure_nether_portal_at(portal_x, 18);

        assert_eq!(state.find_existing_nether_portal_near(portal_x, 20), None);
    }

    #[test]
    fn test_end_portal_activation_consumes_eyes() {
        let mut state = GameState::new();
        state.world.load_chunks_around(0);
        state.world.newly_generated_chunks.clear();

        for x in -6..=6 {
            for y in 0..20 {
                state.world.set_block(x, y, BlockType::Air);
            }
            state.world.set_block(x, 12, BlockType::Stone);
        }

        let inner_x = 0;
        let inner_y = 8;
        let left = inner_x - 1;
        let right = inner_x + 2;
        let top = inner_y - 1;
        let bottom = inner_y + 2;
        for x in left..=right {
            for y in top..=bottom {
                let is_frame = x == left || x == right || y == top || y == bottom;
                state.world.set_block(
                    x,
                    y,
                    if is_frame {
                        BlockType::EndPortalFrame { filled: false }
                    } else {
                        BlockType::Air
                    },
                );
            }
        }

        state.player.x = 0.5;
        state.player.y = 10.0;
        state.hotbar_index = 0;
        state.inventory.slots[0] = Some(ItemStack {
            item_type: ItemType::EyeOfEnder,
            count: 12,
            durability: None,
        });

        for x in left..=right {
            for y in top..=bottom {
                if x == left || x == right || y == top || y == bottom {
                    state.player.x = if x == left {
                        left as f64 - 0.5
                    } else if x == right {
                        right as f64 + 1.5
                    } else {
                        x as f64 + 0.5
                    };
                    state.player.y = if x == left || x == right {
                        y as f64 + 1.5
                    } else if y == top {
                        top as f64
                    } else {
                        bottom as f64
                    };
                    state.interact_block(x, y, false);
                }
            }
        }

        for x in inner_x..=(inner_x + 1) {
            for y in inner_y..=(inner_y + 1) {
                assert_eq!(state.world.get_block(x, y), BlockType::EndPortal);
            }
        }
        assert!(state.inventory.slots[0].is_none());
    }

    #[test]
    fn test_end_portal_transfer_moves_player_to_end() {
        let mut state = GameState::new();
        state.world.load_chunks_around(0);
        state.world.newly_generated_chunks.clear();
        state.zombies.clear();
        state.creepers.clear();
        state.skeletons.clear();
        state.spiders.clear();
        state.pigmen.clear();
        state.ghasts.clear();
        state.cows.clear();
        state.sheep.clear();
        state.item_entities.clear();
        state.arrows.clear();
        state.fireballs.clear();
        state.lightning_bolts.clear();

        for x in -4..=4 {
            for y in 0..20 {
                state.world.set_block(x, y, BlockType::Air);
            }
            state.world.set_block(x, 11, BlockType::Stone);
        }
        state.world.set_block(0, 10, BlockType::EndPortal);

        state.player.x = 0.5;
        state.player.y = 10.0;
        state.interact_block(0, 10, false);

        assert_eq!(state.current_dimension, Dimension::End);
        assert_eq!(state.world.dimension, Dimension::End);
        assert!(state.player.x.abs() >= 6.5);
        assert!(state.player.x.abs() <= 18.5);
        assert!(state.player_portal_kind().is_none());
    }

    #[test]
    fn test_clicking_end_portal_frame_uses_active_end_portal() {
        let mut state = GameState::new();
        configure_quiet_world(&mut state);
        state
            .world
            .set_block(-1, 10, BlockType::EndPortalFrame { filled: true });
        state.world.set_block(0, 10, BlockType::EndPortal);
        state.player.x = 0.5;
        state.player.y = 10.0;
        state.portal_cooldown = 0;

        state.interact_block(-1, 10, false);

        assert_eq!(state.current_dimension, Dimension::End);
    }

    #[test]
    fn test_clicking_bedrock_frame_uses_end_return_portal() {
        let mut state = GameState::new();
        configure_quiet_world(&mut state);
        state.current_dimension = Dimension::End;
        state.world = World::new_for_dimension(Dimension::End);
        state.world.load_chunks_around(0);
        for x in -4..=4 {
            for y in 20..40 {
                state.world.set_block(x, y, BlockType::Air);
            }
            state.world.set_block(x, 35, BlockType::EndStone);
        }
        state.world.set_block(-1, 31, BlockType::Bedrock);
        state.world.set_block(0, 31, BlockType::EndPortal);
        state.player.x = 0.5;
        state.player.y = 34.0;
        state.portal_cooldown = 0;

        state.interact_block(-1, 31, false);

        assert_eq!(state.current_dimension, Dimension::Overworld);
        assert_eq!(state.world.dimension, Dimension::Overworld);
    }

    #[test]
    fn test_clicking_stronghold_dais_uses_active_end_portal() {
        let mut state = GameState::new();
        configure_quiet_world(&mut state);
        state.world.set_block(0, 10, BlockType::EndPortal);
        state.world.set_block(0, 14, BlockType::StoneBricks);
        state.player.x = 0.5;
        state.player.y = 14.0;
        state.portal_cooldown = 0;

        state.interact_block(0, 14, false);

        assert_eq!(state.current_dimension, Dimension::End);
    }

    #[test]
    fn walking_or_sprinting_past_a_nether_portal_does_not_travel() {
        for (right, sprint) in [(false, false), (true, false), (false, true), (true, true)] {
            let mut state = GameState::new();
            configure_quiet_world(&mut state);
            for x in 0..=40 {
                for y in 0..20 {
                    state.world.set_block(x, y, BlockType::Air);
                }
                state.world.set_block(x, 10, BlockType::Stone);
            }

            state.ensure_nether_portal_at(16, 10);
            state.player.x = if right { 13.5 } else { 20.5 };
            state.player.y = 10.0;
            state.player.grounded = true;
            let direction = if right {
                ClientCommand::SetMoveRight(true)
            } else {
                ClientCommand::SetMoveLeft(true)
            };
            state.apply_client_command(direction);
            if sprint {
                state.apply_client_command(if right {
                    ClientCommand::SetMoveRight(false)
                } else {
                    ClientCommand::SetMoveLeft(false)
                });
                state.apply_client_command(direction);
                assert!(state.is_sprinting());
            }

            for _ in 0..40 {
                state.update(16, 8);
            }
            assert_eq!(state.current_dimension, Dimension::Overworld);
            assert!(if right {
                state.player.x > 18.0
            } else {
                state.player.x < 16.0
            });
        }
    }

    #[test]
    fn nether_portal_requires_continuous_dwell_and_does_not_bounce_on_arrival() {
        let mut state = GameState::new();
        configure_quiet_world(&mut state);
        for x in 12..=20 {
            for y in 0..20 {
                state.world.set_block(x, y, BlockType::Air);
            }
            state.world.set_block(x, 10, BlockType::Stone);
        }
        state.ensure_nether_portal_at(16, 10);
        state.player.x = 16.5;
        state.player.y = 10.0;
        for _ in 0..NETHER_PORTAL_DWELL_TICKS - 1 {
            assert!(!state.handle_portal_transition());
        }
        state.player.x = 20.5;
        assert!(!state.handle_portal_transition());
        state.player.x = 16.5;
        for _ in 0..NETHER_PORTAL_DWELL_TICKS - 1 {
            assert!(!state.handle_portal_transition());
        }
        assert!(state.handle_portal_transition());
        assert_eq!(state.current_dimension, Dimension::Nether);
        for _ in 0..80 {
            state.update(0, 0);
        }
        assert_eq!(
            state.current_dimension,
            Dimension::Nether,
            "arrival bounced back"
        );
    }

    #[test]
    fn test_client_command_travel_shortcuts_switch_dimensions_for_exploration() {
        let mut state = GameState::new();
        configure_quiet_world(&mut state);
        state.player.x = 40.5;
        state.player.y = 10.0;

        state.apply_client_command(ClientCommand::TravelToNether);
        assert_eq!(state.current_dimension, Dimension::Nether);
        assert_eq!(state.world.dimension, Dimension::Nether);
        assert!(state.portal_cooldown >= 40);
        assert!(state.player_portal_kind().is_none());

        state.apply_client_command(ClientCommand::TravelToEnd);
        assert_eq!(state.current_dimension, Dimension::End);
        assert_eq!(state.world.dimension, Dimension::End);
        assert!(state.portal_cooldown >= 40);
        assert!(state.player_portal_kind().is_none());

        state.apply_client_command(ClientCommand::TravelToOverworld);
        assert_eq!(state.current_dimension, Dimension::Overworld);
        assert_eq!(state.world.dimension, Dimension::Overworld);
        assert!(
            (state.player.x - (crate::world::STRONGHOLD_PORTAL_INNER_X as f64 + 0.5)).abs() < 8.0
        );
    }

    #[test]
    fn test_client_command_travel_to_spawn_returns_to_saved_bed_from_other_dimension() {
        let mut state = GameState::new();
        let bed_x = 4096;
        let bed_y = 9;
        state.current_dimension = Dimension::Overworld;
        state.world = World::new_for_dimension(Dimension::Overworld);
        state.world.load_chunks_around(bed_x);
        for x in (bed_x - 3)..=(bed_x + 3) {
            for y in 0..20 {
                state.world.set_block(x, y, BlockType::Air);
            }
            state.world.set_block(x, 10, BlockType::Stone);
        }
        state.world.set_block(bed_x, bed_y, BlockType::Bed);
        state.spawn_point = Some((bed_x, bed_y));
        state.world.save_all();

        state.current_dimension = Dimension::Nether;
        state.world = World::new_for_dimension(Dimension::Nether);
        state.world.load_chunks_around(0);
        state.player.x = 0.5;
        state.player.y = 48.0;
        state.portal_cooldown = 0;

        state.apply_client_command(ClientCommand::TravelToSpawn);

        assert_eq!(state.current_dimension, Dimension::Overworld);
        assert_eq!(state.world.dimension, Dimension::Overworld);
        assert!((state.player.x - (bed_x as f64 + 0.5)).abs() < 0.01);
        assert!((state.player.y - (bed_y as f64 - 0.1)).abs() < 0.01);
        assert_eq!(state.portal_cooldown, 40);
    }

    #[test]
    fn test_client_command_equip_diamond_loadout_equips_full_armor_and_hotbar() {
        let mut state = GameState::new();
        state.inventory = Inventory::new(PLAYER_INVENTORY_CAPACITY);
        state.inventory_enchant_levels = [0; PLAYER_INVENTORY_CAPACITY];
        state.armor_slots = [None, None, None, None];
        state.armor_enchant_levels = [0; ARMOR_SLOT_COUNT];
        state.armor_slots[1] = Some(ItemStack {
            item_type: ItemType::IronChestplate,
            count: 1,
            durability: Some(87),
        });
        state.inventory.slots[0] = Some(ItemStack {
            item_type: ItemType::StoneSword,
            count: 1,
            durability: Some(61),
        });
        state.inventory_enchant_levels[0] = 2;

        state.apply_client_command(ClientCommand::EquipDiamondLoadout);

        let equipped = [
            ItemType::DiamondHelmet,
            ItemType::DiamondChestplate,
            ItemType::DiamondLeggings,
            ItemType::DiamondBoots,
        ];
        for (slot_idx, item_type) in equipped.into_iter().enumerate() {
            let stack = state.armor_slots[slot_idx]
                .as_ref()
                .expect("diamond loadout should fill every armor slot");
            assert_eq!(stack.item_type, item_type);
            assert_eq!(stack.count, 1);
            assert_eq!(stack.durability, item_type.max_durability());
        }

        assert!(state.inventory.slots.iter().flatten().any(|stack| {
            stack.item_type == ItemType::IronChestplate && stack.durability == Some(87)
        }));
        assert_eq!(
            state.inventory.slots[0]
                .as_ref()
                .map(|stack| (stack.item_type, stack.count)),
            Some((ItemType::DiamondSword, 1))
        );
        assert_eq!(
            state.inventory.slots[1]
                .as_ref()
                .map(|stack| (stack.item_type, stack.count)),
            Some((ItemType::Bow, 1))
        );
        assert_eq!(
            state.inventory.slots[2]
                .as_ref()
                .map(|stack| (stack.item_type, stack.count)),
            Some((ItemType::Arrow, 64))
        );
        assert_eq!(
            state.inventory.slots[3]
                .as_ref()
                .map(|stack| (stack.item_type, stack.count)),
            Some((ItemType::CookedBeef, 32))
        );
        assert_eq!(
            state.inventory.slots[4]
                .as_ref()
                .map(|stack| (stack.item_type, stack.count)),
            Some((ItemType::Cobblestone, 64))
        );
        assert_eq!(
            state.inventory.slots[5]
                .as_ref()
                .map(|stack| (stack.item_type, stack.count)),
            Some((ItemType::DiamondPickaxe, 1))
        );
        assert_eq!(
            state.inventory.slots[6]
                .as_ref()
                .map(|stack| (stack.item_type, stack.count)),
            Some((ItemType::EnderPearl, 16))
        );
        assert_eq!(state.hotbar_index, 0);
        assert!(state.inventory.slots.iter().enumerate().any(|(idx, slot)| {
            slot.as_ref().is_some_and(|stack| {
                stack.item_type == ItemType::StoneSword
                    && stack.durability == Some(61)
                    && state.inventory_enchant_levels[idx] == 2
            })
        }));
    }

    #[test]
    fn raising_a_saved_pillar_moves_its_crystal_and_standing_player_safely() {
        let root =
            std::env::temp_dir().join(format!("termcraft-raised-pillar-{}", rand::random::<u64>()));
        let mut state = GameState::open_in_directory(&root).unwrap();
        state.current_dimension = Dimension::End;
        state.world = World::open_in_directory(Dimension::End, &root).unwrap();
        state.world.load_chunks_for_spawn_search(0, 40);
        state.end_crystals.clear();
        for tower in END_TOWER_XS {
            let top = (2..40)
                .find(|&y| state.world.get_block(tower, y) == BlockType::Obsidian)
                .unwrap();
            let old_top = top + 2;
            for x in tower - 2..=tower + 2 {
                for y in top - 1..old_top {
                    state.world.set_block(x, y, BlockType::Air);
                }
                state.world.set_block(x, old_top, BlockType::Obsidian);
            }
            state
                .world
                .set_block(tower, old_top - 1, BlockType::Glowstone);
            let mut crystal = EndCrystal::new(tower as f64 + 0.5, (old_top - 2) as f64);
            crystal.health = 3.0;
            state.end_crystals.push(crystal);
            if tower == 14 {
                state.player.x = 14.5;
                state.player.y = (old_top - 1) as f64;
            }
        }
        state.end_boss_initialized = true;
        state.ender_dragon = Some(EnderDragon::new(6.5, 18.0));
        let before = state.player.y;
        state.try_persist_world_and_progress().unwrap();
        drop(state);
        let mut state = GameState::open_in_directory(&root).unwrap();
        assert_eq!(state.current_dimension, Dimension::End);
        state.ensure_end_boss_entities();
        assert_eq!(state.player.y, before - 2.0);
        assert!(!state.entity_collides_with_world(
            state.player.x,
            state.player.y,
            PLAYER_HALF_WIDTH,
            PLAYER_HEIGHT
        ));
        assert_eq!(state.end_crystals.len(), 4);
        let crystal = state
            .end_crystals
            .iter()
            .find(|c| (c.x - 14.5).abs() < 0.01)
            .unwrap();
        assert_eq!(crystal.y, before - 3.0);
        assert_eq!(crystal.health, 3.0);
        state.try_persist_world_and_progress().unwrap();
        drop(state);
        let state = GameState::open_in_directory(&root).unwrap();
        assert_eq!(state.player.y, before - 2.0);
        assert_eq!(state.end_crystals.len(), 4);
        drop(state);
        crate::platform::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn test_end_boss_initializes_dragon_and_crystals() {
        let mut state = GameState::new();
        state.current_dimension = Dimension::End;
        state.world = World::new_for_dimension(Dimension::End);
        state.world.load_chunks_around(0);
        state.end_boss_initialized = false;
        state.end_crystals.clear();
        state.ender_dragon = None;

        state.ensure_end_boss_entities();

        assert!(state.end_boss_initialized);
        assert!(state.ender_dragon.is_some());
        assert!(state.end_crystals.len() >= 2);
    }

    #[test]
    fn test_end_crystals_heal_dragon_over_time() {
        let mut state = GameState::new();
        state.current_dimension = Dimension::End;
        state.world = World::new_for_dimension(Dimension::End);
        state.world.load_chunks_around(0);
        state.end_boss_initialized = false;
        state.ensure_end_boss_entities();

        let Some(dragon) = state.ender_dragon.as_mut() else {
            panic!("dragon should be initialized");
        };
        dragon.health = dragon.max_health - 20.0;
        let before = dragon.health;

        for _ in 0..24 {
            state.update_end_boss_encounter();
        }

        let after = state.ender_dragon.as_ref().map(|d| d.health).unwrap_or(0.0);
        assert!(after > before);
    }

    #[test]
    fn test_destroyed_end_crystal_is_removed() {
        let mut state = GameState::new();
        state.current_dimension = Dimension::End;
        state.world = World::new_for_dimension(Dimension::End);
        state.world.load_chunks_around(0);
        state.end_boss_initialized = false;
        state.ensure_end_boss_entities();

        let before = state.end_crystals.len();
        assert!(before > 0);
        state.end_crystals[0].health = 0.0;

        state.update_end_boss_encounter();

        assert_eq!(state.end_crystals.len(), before - 1);
    }

    #[test]
    fn test_dragon_death_starts_end_victory_sequence() {
        let mut state = GameState::new();
        state.current_dimension = Dimension::End;
        state.world = World::new_for_dimension(Dimension::End);
        state.world.load_chunks_around(0);
        state.end_boss_initialized = false;
        state.ensure_end_boss_entities();

        let Some(dragon) = state.ender_dragon.as_mut() else {
            panic!("dragon should be initialized");
        };
        dragon.health = 0.0;

        state.update_end_boss_encounter();

        assert!(state.dragon_defeated);
        assert!(state.ender_dragon.is_none());
        let Some((ticks, origin_x, origin_y)) = state.end_victory_sequence_state() else {
            panic!("dragon death should start the victory sequence");
        };
        assert_eq!(ticks, END_VICTORY_SEQUENCE_TICKS);
        assert!(origin_x.abs() < 2.0);
        assert!((8.0..=26.0).contains(&origin_y));
    }

    #[test]
    fn test_end_victory_sequence_ticks_down_and_clears() {
        let mut state = GameState::new();
        state.start_end_victory_sequence(4.5, 18.0);

        for expected in (1..END_VICTORY_SEQUENCE_TICKS).rev() {
            state.update_end_victory_sequence();
            assert_eq!(
                state.end_victory_sequence_state().map(|s| s.0),
                Some(expected)
            );
        }

        state.update_end_victory_sequence();
        assert_eq!(state.end_victory_sequence_state(), None);
    }

    #[test]
    fn test_defeated_end_does_not_respawn_dragon_or_crystals_on_reentry() {
        let mut state = GameState::new();
        state.current_dimension = Dimension::End;
        state.world = World::new_for_dimension(Dimension::End);
        state.world.load_chunks_around(0);
        state.dragon_defeated = true;
        state.end_boss_initialized = false;
        state.end_crystals.clear();
        state.ender_dragon = None;

        state.ensure_end_boss_entities();

        assert!(state.end_crystals.is_empty());
        assert!(state.ender_dragon.is_none());
        assert!(!state.end_boss_initialized);
    }

    #[test]
    fn test_end_portal_transfer_returns_to_overworld_stronghold() {
        let mut state = GameState::new();
        state.current_dimension = Dimension::End;
        state.world = World::new_for_dimension(Dimension::End);
        state.world.load_chunks_around(0);
        state.world.newly_generated_chunks.clear();
        state.player.x = 0.5;
        state.player.y = 34.0;
        state.world.set_block(0, 34, BlockType::EndPortal);
        state.interact_block(0, 34, false);

        assert_eq!(state.current_dimension, Dimension::Overworld);
        assert_eq!(state.world.dimension, Dimension::Overworld);
        assert!(
            (state.player.x - (crate::world::STRONGHOLD_PORTAL_INNER_X as f64 + 0.5)).abs() < 8.0
        );
    }

    #[test]
    fn test_end_portal_after_dragon_defeat_starts_credits() {
        let mut state = GameState::new();
        state.current_dimension = Dimension::End;
        state.world = World::new_for_dimension(Dimension::End);
        state.world.load_chunks_around(0);
        state.world.newly_generated_chunks.clear();
        state.player.x = 0.5;
        state.player.y = 34.0;
        state.world.set_block(0, 34, BlockType::EndPortal);
        state.dragon_defeated = true;

        state.interact_block(0, 34, false);

        assert!(state.is_showing_credits());
        assert_eq!(state.current_dimension, Dimension::End);
    }

    #[test]
    fn test_skip_completion_credits_returns_to_overworld() {
        let mut state = GameState::new();
        state.current_dimension = Dimension::End;
        state.world = World::new_for_dimension(Dimension::End);
        state.world.load_chunks_around(0);
        state.player.x = 0.5;
        state.player.y = 34.0;
        state.world.set_block(0, 34, BlockType::EndPortal);
        state.dragon_defeated = true;
        state.start_completion_credits();

        state.skip_completion_credits();

        assert!(!state.is_showing_credits());
        assert_eq!(state.current_dimension, Dimension::Overworld);
        assert!(state.has_seen_completion_credits());
    }

    #[test]
    fn test_player_death_starts_death_screen() {
        let mut state = GameState::new();
        state.player.health = -1.0;
        state.player.hunger = 0.0;

        state.update(0, 0);

        assert!(state.is_showing_death_screen());
    }

    #[test]
    fn test_respawn_from_death_screen_restores_player() {
        let mut state = GameState::new();
        prepare_test_respawn_ground(&mut state);
        state.player.health = -1.0;
        state.player.hunger = 0.0;
        state.update(0, 0);
        assert!(state.is_showing_death_screen());
        assert!(!state.can_respawn_from_death_screen());

        state.player.health = 0.0;
        state.player.hunger = 0.0;
        state.player.x = 42.0;
        state.player.y = 42.0;
        state.death_screen_ticks = DEATH_RESPAWN_DELAY_TICKS;
        state.respawn_from_death_screen();

        assert!(!state.is_showing_death_screen());
        assert_eq!(state.player.health, state.player.max_health);
        assert_eq!(state.player.hunger, state.player.max_hunger);
        assert!((-55.5..=56.5).contains(&state.player.x));
        assert!(state.player.y > 0.0);
        assert_eq!(state.respawn_grace_ticks, RESPAWN_GRACE_TICKS);
    }

    #[test]
    fn test_sleeping_in_bed_sets_spawn_and_skips_night() {
        let mut state = GameState::new();
        state.current_dimension = Dimension::Overworld;
        state.world = World::new_for_dimension(Dimension::Overworld);
        state.world.load_chunks_around(0);

        for x in -4..=4 {
            for y in 0..20 {
                state.world.set_block(x, y, BlockType::Air);
            }
            state.world.set_block(x, 10, BlockType::Stone);
        }
        state.world.set_block(1, 9, BlockType::Bed);
        state.player.x = 0.5;
        state.player.y = 9.9;
        state.time_of_day = 22000.0;
        state.weather = WeatherType::Thunderstorm;
        state.weather_thunder_intensity = 0.8;
        state.weather_rain_intensity = 0.9;

        state.interact_block(1, 9, false);

        assert_eq!(state.spawn_point, Some((1, 9)));
        assert_eq!(state.time_of_day, 4000.0);
        assert_eq!(state.weather, WeatherType::Clear);
        assert_eq!(state.weather_thunder_intensity, 0.0);
    }

    #[test]
    fn test_interacting_with_chest_opens_ui_and_swaps_with_player_inventory() {
        let mut state = GameState::new();
        state.current_dimension = Dimension::Overworld;
        state.world = World::new_for_dimension(Dimension::Overworld);
        state.world.load_chunks_around(0);

        for x in -4..=4 {
            for y in 0..20 {
                state.world.set_block(x, y, BlockType::Air);
            }
            state.world.set_block(x, 10, BlockType::Stone);
        }
        state.world.set_block(1, 9, BlockType::Chest);
        if let Some(chest) = state
            .world
            .ensure_chest_inventory(1, 9, CHEST_INVENTORY_CAPACITY)
        {
            chest.slots[0] = Some(ItemStack {
                item_type: ItemType::Diamond,
                count: 2,
                durability: None,
            });
        }
        state.inventory.slots[0] = Some(ItemStack {
            item_type: ItemType::Planks,
            count: 5,
            durability: None,
        });
        state.player.x = 0.5;
        state.player.y = 10.0;

        state.interact_block(1, 9, false);

        assert!(state.inventory_open);
        assert!(state.at_chest);
        assert!(!state.at_crafting_table);
        assert!(!state.at_furnace);

        state.handle_inventory_click(0);
        state.handle_inventory_click(27);

        assert!(
            state.inventory.slots[0]
                .as_ref()
                .is_some_and(|stack| stack.item_type == ItemType::Diamond && stack.count == 2)
        );
        let chest_stack = state
            .world
            .chest_inventory(1, 9)
            .and_then(|inv| inv.slots[0].as_ref());
        assert!(
            chest_stack
                .is_some_and(|stack| stack.item_type == ItemType::Planks && stack.count == 5)
        );
    }

    #[test]
    fn test_use_at_command_opens_chest_ui() {
        let mut state = GameState::new();
        state.current_dimension = Dimension::Overworld;
        state.world = World::new_for_dimension(Dimension::Overworld);
        state.world.load_chunks_around(0);

        for x in -4..=4 {
            for y in 0..20 {
                state.world.set_block(x, y, BlockType::Air);
            }
            state.world.set_block(x, 10, BlockType::Stone);
        }
        state.world.set_block(1, 9, BlockType::Chest);
        state.player.x = 0.5;
        state.player.y = 10.0;

        state.apply_client_command(ClientCommand::UseAt(1, 9));

        assert!(state.inventory_open);
        assert!(state.at_chest);
    }

    #[test]
    fn test_opening_chest_clears_live_inputs_in_cluttered_room() {
        let mut state = GameState::new();
        state.current_dimension = Dimension::Overworld;
        state.world = World::new_for_dimension(Dimension::Overworld);
        state.world.load_chunks_around(0);

        for x in -4..=4 {
            for y in 0..20 {
                state.world.set_block(x, y, BlockType::Air);
            }
            state.world.set_block(x, 10, BlockType::Stone);
        }
        state.world.set_block(0, 9, BlockType::Torch);
        state.world.set_block(1, 9, BlockType::Chest);
        state.world.set_block(2, 9, BlockType::SilverfishSpawner);

        state.player.x = 0.5;
        state.player.y = 9.55;
        state.player.vx = 0.42;
        state.player.vy = -0.18;
        state.player.grounded = false;
        state.moving_right = true;
        state.left_click_down = true;
        state.jump_held = true;
        state.jump_buffer_ticks = 3;
        state.sprinting = true;
        state.sprint_direction = 1;

        state.interact_block(1, 9, false);

        assert!(state.inventory_open);
        assert!(state.at_chest);
        assert!(!state.left_click_down);
        assert!(!state.moving_left);
        assert!(!state.moving_right);
        assert!(!state.jump_held);
        assert_eq!(state.jump_buffer_ticks, 0);
        assert!(!state.sprinting);
        assert_eq!(state.sprint_direction, 0);
    }

    #[test]
    fn test_shift_click_moves_chest_stack_into_player_inventory() {
        let mut state = GameState::new();
        state.current_dimension = Dimension::Overworld;
        state.world = World::new_for_dimension(Dimension::Overworld);
        state.world.load_chunks_around(0);

        for x in -4..=4 {
            for y in 0..20 {
                state.world.set_block(x, y, BlockType::Air);
            }
            state.world.set_block(x, 10, BlockType::Stone);
        }
        state.world.set_block(1, 9, BlockType::Chest);
        if let Some(chest) = state
            .world
            .ensure_chest_inventory(1, 9, CHEST_INVENTORY_CAPACITY)
        {
            chest.slots[0] = Some(ItemStack {
                item_type: ItemType::Planks,
                count: 10,
                durability: None,
            });
        }
        state.inventory.slots[9] = Some(ItemStack {
            item_type: ItemType::Planks,
            count: 60,
            durability: None,
        });
        state.open_chest_inventory_view(1, 9);
        assert!(state.at_chest);

        state.handle_inventory_shift_click(0);

        let chest_slot_0 = state
            .world
            .chest_inventory(1, 9)
            .and_then(|inv| inv.slots[0].as_ref());
        assert!(chest_slot_0.is_none());
        assert_eq!(state.inventory.slots[9].as_ref().map(|s| s.count), Some(64));
        assert_eq!(
            state.inventory.slots[10]
                .as_ref()
                .map(|s| (s.item_type, s.count)),
            Some((ItemType::Planks, 6))
        );
    }

    #[test]
    fn test_shift_click_moves_player_stack_into_chest_inventory() {
        let mut state = GameState::new();
        state.current_dimension = Dimension::Overworld;
        state.world = World::new_for_dimension(Dimension::Overworld);
        state.world.load_chunks_around(0);

        for x in -4..=4 {
            for y in 0..20 {
                state.world.set_block(x, y, BlockType::Air);
            }
            state.world.set_block(x, 10, BlockType::Stone);
        }
        state.world.set_block(1, 9, BlockType::Chest);
        if let Some(chest) = state
            .world
            .ensure_chest_inventory(1, 9, CHEST_INVENTORY_CAPACITY)
        {
            chest.slots[0] = Some(ItemStack {
                item_type: ItemType::Planks,
                count: 62,
                durability: None,
            });
        }
        state.inventory.slots[0] = Some(ItemStack {
            item_type: ItemType::Planks,
            count: 5,
            durability: None,
        });
        state.open_chest_inventory_view(1, 9);
        assert!(state.at_chest);

        state.handle_inventory_shift_click(CHEST_INVENTORY_CAPACITY);

        assert!(state.inventory.slots[0].is_none());
        let chest_inv = state.world.chest_inventory(1, 9).expect("chest exists");
        assert_eq!(chest_inv.slots[0].as_ref().map(|s| s.count), Some(64));
        assert_eq!(
            chest_inv.slots[1].as_ref().map(|s| (s.item_type, s.count)),
            Some((ItemType::Planks, 3))
        );
    }

    #[test]
    fn test_shift_click_routes_between_hotbar_and_main_inventory() {
        let mut state = GameState::new();
        state.inventory = Inventory::new(PLAYER_INVENTORY_CAPACITY);
        state.inventory_open = true;
        state.at_chest = false;
        state.at_furnace = false;
        state.at_crafting_table = false;
        state.at_enchanting_table = false;
        state.at_anvil = false;
        state.at_brewing_stand = false;
        state.selected_inventory_slot = Some(0);
        state.inventory.slots[0] = Some(ItemStack {
            item_type: ItemType::Cobblestone,
            count: 12,
            durability: None,
        });

        state.handle_inventory_shift_click(0);

        assert!(state.inventory.slots[0].is_none());
        assert_eq!(
            state.inventory.slots[PLAYER_HOTBAR_SLOTS]
                .as_ref()
                .map(|s| (s.item_type, s.count)),
            Some((ItemType::Cobblestone, 12))
        );
        assert_eq!(state.selected_inventory_slot, None);

        state.handle_inventory_shift_click(PLAYER_HOTBAR_SLOTS);

        assert!(state.inventory.slots[PLAYER_HOTBAR_SLOTS].is_none());
        assert_eq!(
            state.inventory.slots[0]
                .as_ref()
                .map(|s| (s.item_type, s.count)),
            Some((ItemType::Cobblestone, 12))
        );
    }

    #[test]
    fn test_breaking_chest_drops_stored_items() {
        let mut state = GameState::new();
        state.current_dimension = Dimension::Overworld;
        state.world = World::new_for_dimension(Dimension::Overworld);
        state.world.load_chunks_around(0);

        for x in -4..=4 {
            for y in 0..20 {
                state.world.set_block(x, y, BlockType::Air);
            }
            state.world.set_block(x, 10, BlockType::Stone);
        }
        state.world.set_block(1, 9, BlockType::Chest);
        if let Some(chest) = state
            .world
            .ensure_chest_inventory(1, 9, CHEST_INVENTORY_CAPACITY)
        {
            chest.slots[0] = Some(ItemStack {
                item_type: ItemType::Diamond,
                count: 2,
                durability: None,
            });
        }
        state.player.x = 0.5;
        state.player.y = 10.0;
        state.hotbar_index = 0;
        state.inventory.slots[0] = Some(ItemStack {
            item_type: ItemType::DiamondPickaxe,
            count: 1,
            durability: Some(1562),
        });

        for _ in 0..90 {
            state.interact_block(1, 9, true);
            if state.world.get_block(1, 9) == BlockType::Air {
                break;
            }
        }

        assert_eq!(state.world.get_block(1, 9), BlockType::Air);
        assert!(state.world.chest_inventory(1, 9).is_none());
        let chest_drop_count = state
            .item_entities
            .iter()
            .filter(|item| item.item_type == ItemType::Chest)
            .count();
        let diamond_drop_count = state
            .item_entities
            .iter()
            .filter(|item| item.item_type == ItemType::Diamond)
            .count();
        assert!(chest_drop_count >= 1);
        assert!(diamond_drop_count >= 2);
    }

    #[test]
    fn test_breaking_tall_grass_can_drop_seeds() {
        let mut state = GameState::new();
        state.current_dimension = Dimension::Overworld;
        state.world = World::new_for_dimension(Dimension::Overworld);
        state.world.load_chunks_around(0);

        for x in -4..=4 {
            for y in 0..20 {
                state.world.set_block(x, y, BlockType::Air);
            }
            state.world.set_block(x, 10, BlockType::Grass);
        }

        let grass_x = (0..=4)
            .find(|&x| tall_grass_drops_seed_at(x, 9))
            .expect("expected a deterministic seed-dropping grass position");
        state.world.set_block(grass_x, 9, BlockType::TallGrass);
        state.player.x = grass_x as f64 - 0.5;
        state.player.y = 10.0;

        state.interact_block(grass_x, 9, true);

        assert_eq!(state.world.get_block(grass_x, 9), BlockType::Air);
        assert!(
            state
                .item_entities
                .iter()
                .any(|item| item.item_type == ItemType::WheatSeeds)
        );
    }

    #[test]
    fn test_breaking_gravel_can_drop_flint() {
        let mut state = GameState::new();
        state.current_dimension = Dimension::Overworld;
        state.world = World::new_for_dimension(Dimension::Overworld);
        state.world.load_chunks_around(0);

        for x in -4..=4 {
            for y in 0..20 {
                state.world.set_block(x, y, BlockType::Air);
            }
            state.world.set_block(x, 10, BlockType::Stone);
        }

        let gravel_x = (-4..=4)
            .find(|&x| gravel_drops_flint_at(x, 9))
            .expect("expected a deterministic flint-dropping gravel position");
        state.world.set_block(gravel_x, 9, BlockType::Gravel);
        state.player.x = gravel_x as f64 - 0.5;
        state.player.y = 10.0;

        for _ in 0..90 {
            state.interact_block(gravel_x, 9, true);
            if state.world.get_block(gravel_x, 9) == BlockType::Air {
                break;
            }
        }

        assert_eq!(state.world.get_block(gravel_x, 9), BlockType::Air);
        assert!(
            state
                .item_entities
                .iter()
                .any(|item| item.item_type == ItemType::Flint),
            "expected gravel to drop flint"
        );
    }

    #[test]
    fn test_using_bone_meal_fully_grows_crops() {
        let mut state = GameState::new();
        configure_quiet_world(&mut state);
        for x in -4..=4 {
            for y in 0..20 {
                state.world.set_block(x, y, BlockType::Air);
            }
            state.world.set_block(x, 10, BlockType::Dirt);
        }
        state.world.set_block(1, 10, BlockType::Farmland(7));
        state.world.set_block(1, 9, BlockType::Crops(2));
        state.player.x = 0.5;
        state.player.y = 10.0;
        state.hotbar_index = 0;
        state.inventory.slots[0] = Some(ItemStack {
            item_type: ItemType::BoneMeal,
            count: 1,
            durability: None,
        });

        state.interact_block(1, 9, false);

        assert_eq!(state.world.get_block(1, 9), BlockType::Crops(7));
        assert!(state.inventory.slots[0].is_none());
    }

    #[test]
    fn test_using_hoe_near_water_creates_wet_farmland() {
        let mut state = GameState::new();
        configure_quiet_world(&mut state);
        for x in -4..=4 {
            for y in 0..20 {
                state.world.set_block(x, y, BlockType::Air);
            }
            state.world.set_block(x, 10, BlockType::Dirt);
        }
        state.world.set_block(3, 10, BlockType::Water(8));
        state.player.x = 0.5;
        state.player.y = 10.0;
        state.hotbar_index = 0;
        state.inventory.slots[0] = Some(ItemStack {
            item_type: ItemType::WoodHoe,
            count: 1,
            durability: Some(59),
        });

        state.interact_block(1, 10, false);

        assert_eq!(state.world.get_block(1, 10), BlockType::Farmland(7));
    }

    #[test]
    fn test_using_hoe_on_tall_grass_tills_ground_below() {
        let mut state = GameState::new();
        configure_quiet_world(&mut state);
        for x in -4..=4 {
            for y in 0..20 {
                state.world.set_block(x, y, BlockType::Air);
            }
            state.world.set_block(x, 10, BlockType::Grass);
        }
        state.world.set_block(1, 9, BlockType::TallGrass);
        state.player.x = 0.5;
        state.player.y = 10.0;
        state.hotbar_index = 0;
        state.inventory.slots[0] = Some(ItemStack {
            item_type: ItemType::WoodHoe,
            count: 1,
            durability: Some(59),
        });

        state.interact_block(1, 9, false);

        assert_eq!(state.world.get_block(1, 9), BlockType::Air);
        assert_eq!(state.world.get_block(1, 10), BlockType::Farmland(1));
    }

    #[test]
    fn test_using_bone_meal_on_sapling_grows_tree() {
        let mut state = GameState::new();
        configure_quiet_world(&mut state);
        for x in -6..=6 {
            for y in 0..24 {
                state.world.set_block(x, y, BlockType::Air);
            }
            state.world.set_block(x, 12, BlockType::Grass);
        }
        state.world.set_block(1, 11, BlockType::Sapling);
        state.player.x = 0.5;
        state.player.y = 12.0;
        state.hotbar_index = 0;
        state.inventory.slots[0] = Some(ItemStack {
            item_type: ItemType::BoneMeal,
            count: 1,
            durability: None,
        });

        state.interact_block(1, 11, false);

        assert!(matches!(
            state.world.get_block(1, 11),
            BlockType::Wood | BlockType::BirchWood
        ));
        assert!(state.inventory.slots[0].is_none());
    }

    #[test]
    fn test_using_bone_meal_on_birch_sapling_grows_birch_tree() {
        let mut state = GameState::new();
        configure_quiet_world(&mut state);
        for x in -6..=6 {
            for y in 0..24 {
                state.world.set_block(x, y, BlockType::Air);
            }
            state.world.set_block(x, 12, BlockType::Grass);
        }
        state.world.set_block(1, 11, BlockType::BirchSapling);
        state.player.x = 0.5;
        state.player.y = 12.0;
        state.hotbar_index = 0;
        state.inventory.slots[0] = Some(ItemStack {
            item_type: ItemType::BoneMeal,
            count: 1,
            durability: None,
        });

        state.interact_block(1, 11, false);

        assert_eq!(state.world.get_block(1, 11), BlockType::BirchWood);
        assert!(state.inventory.slots[0].is_none());
    }

    #[test]
    fn test_using_bone_meal_on_grass_spawns_flora() {
        let mut state = GameState::new();
        configure_quiet_world(&mut state);
        for x in -6..=6 {
            for y in 0..24 {
                state.world.set_block(x, y, BlockType::Air);
            }
            state.world.set_block(x, 12, BlockType::Grass);
        }
        state.player.x = 0.5;
        state.player.y = 12.0;
        state.hotbar_index = 0;
        state.inventory.slots[0] = Some(ItemStack {
            item_type: ItemType::BoneMeal,
            count: 1,
            durability: None,
        });

        state.interact_block(0, 12, false);

        let flora_count = (-3..=3)
            .filter(|&x| {
                matches!(
                    state.world.get_block(x, 11),
                    BlockType::TallGrass | BlockType::RedFlower | BlockType::YellowFlower
                )
            })
            .count();
        assert!(flora_count >= 1);
        assert!(state.inventory.slots[0].is_none());
    }

    #[test]
    fn test_using_bone_meal_on_flower_spreads_more_flora() {
        let mut state = GameState::new();
        configure_quiet_world(&mut state);
        for x in -6..=6 {
            for y in 0..24 {
                state.world.set_block(x, y, BlockType::Air);
            }
            state.world.set_block(x, 12, BlockType::Grass);
        }
        state.world.set_block(0, 11, BlockType::RedFlower);
        state.player.x = 0.5;
        state.player.y = 12.0;
        state.hotbar_index = 0;
        state.inventory.slots[0] = Some(ItemStack {
            item_type: ItemType::BoneMeal,
            count: 1,
            durability: None,
        });

        state.interact_block(0, 11, false);

        let flora_count = (-3..=3)
            .filter(|&x| {
                matches!(
                    state.world.get_block(x, 11),
                    BlockType::TallGrass | BlockType::RedFlower | BlockType::YellowFlower
                )
            })
            .count();
        assert!(flora_count >= 2);
        assert!(state.inventory.slots[0].is_none());
    }

    #[test]
    fn test_interacting_toggles_wood_door() {
        let mut state = GameState::new();
        state.current_dimension = Dimension::Overworld;
        state.world = World::new_for_dimension(Dimension::Overworld);
        state.world.load_chunks_around(0);

        for x in -4..=4 {
            for y in 0..20 {
                state.world.set_block(x, y, BlockType::Air);
            }
            state.world.set_block(x, 10, BlockType::Stone);
        }
        state.world.set_block(1, 9, BlockType::WoodDoor(false));
        state.world.set_block(1, 8, BlockType::WoodDoor(false));
        state.player.x = 0.5;
        state.player.y = 10.0;

        state.interact_block(1, 9, false);
        assert_eq!(state.world.get_block(1, 9), BlockType::WoodDoor(true));
        assert_eq!(state.world.get_block(1, 8), BlockType::WoodDoor(true));

        state.interact_block(1, 9, false);
        assert_eq!(state.world.get_block(1, 9), BlockType::WoodDoor(false));
        assert_eq!(state.world.get_block(1, 8), BlockType::WoodDoor(false));
    }

    #[test]
    fn test_placing_wood_door_requires_solid_support() {
        let mut state = GameState::new();
        state.current_dimension = Dimension::Overworld;
        state.world = World::new_for_dimension(Dimension::Overworld);
        state.world.load_chunks_around(0);

        for x in -4..=4 {
            for y in 0..20 {
                state.world.set_block(x, y, BlockType::Air);
            }
        }
        state.player.x = 0.5;
        state.player.y = 10.0;
        state.hotbar_index = 0;
        state.inventory.slots[0] = Some(ItemStack {
            item_type: ItemType::WoodDoor,
            count: 2,
            durability: None,
        });

        state.interact_block(1, 9, false);
        assert_eq!(state.world.get_block(1, 9), BlockType::Air);

        state.world.set_block(1, 10, BlockType::Stone);
        state.interact_block(1, 9, false);
        assert_eq!(state.world.get_block(1, 9), BlockType::WoodDoor(false));
        assert_eq!(state.world.get_block(1, 8), BlockType::WoodDoor(false));
    }

    #[test]
    fn test_breaking_wood_door_clears_both_halves() {
        let mut state = GameState::new();
        state.current_dimension = Dimension::Overworld;
        state.world = World::new_for_dimension(Dimension::Overworld);
        state.world.load_chunks_around(0);

        for x in -4..=4 {
            for y in 0..20 {
                state.world.set_block(x, y, BlockType::Air);
            }
            state.world.set_block(x, 10, BlockType::Stone);
        }
        state.world.set_block(1, 9, BlockType::WoodDoor(false));
        state.world.set_block(1, 8, BlockType::WoodDoor(false));
        state.player.x = 0.5;
        state.player.y = 10.0;
        state.hotbar_index = 0;
        state.inventory.slots[0] = Some(ItemStack {
            item_type: ItemType::DiamondAxe,
            count: 1,
            durability: Some(1562),
        });

        for _ in 0..48 {
            state.interact_block(1, 9, true);
            if state.world.get_block(1, 9) == BlockType::Air {
                break;
            }
        }

        assert_eq!(state.world.get_block(1, 9), BlockType::Air);
        assert_eq!(state.world.get_block(1, 8), BlockType::Air);
        let door_drop_count = state
            .item_entities
            .iter()
            .filter(|item| item.item_type == ItemType::WoodDoor)
            .count();
        assert_eq!(door_drop_count, 1);
    }

    #[test]
    fn test_ladder_clamps_fall_speed_in_movement_solver() {
        let mut state = GameState::new();
        state.current_dimension = Dimension::Overworld;
        state.world = World::new_for_dimension(Dimension::Overworld);
        state.world.load_chunks_around(0);

        for x in -2..=2 {
            for y in 0..20 {
                state.world.set_block(x, y, BlockType::Air);
            }
        }
        state.world.set_block(0, 9, BlockType::Ladder);

        let (_, _, _, nvy, _, _) =
            state.calculate_movement(0.5, 9.0, 0.0, 1.0, false, 0.25, 1.8, true);
        assert!(nvy <= 0.16 + f64::EPSILON);
    }

    #[test]
    fn test_respawn_uses_valid_bed_spawnpoint() {
        let mut state = GameState::new();
        let bed_x = 2048;
        let bed_y = 9;
        state.current_dimension = Dimension::Overworld;
        state.world = World::new_for_dimension(Dimension::Overworld);
        state.world.load_chunks_around(bed_x);
        for x in (bed_x - 3)..=(bed_x + 3) {
            for y in 0..20 {
                state.world.set_block(x, y, BlockType::Air);
            }
            state.world.set_block(x, 10, BlockType::Stone);
        }
        state.world.set_block(bed_x, bed_y, BlockType::Bed);
        state.spawn_point = Some((bed_x, bed_y));
        state.death_screen_active = true;
        state.death_screen_ticks = DEATH_RESPAWN_DELAY_TICKS;

        state.respawn_from_death_screen();

        assert_eq!(state.current_dimension, Dimension::Overworld);
        assert!((state.player.x - (bed_x as f64 + 0.5)).abs() < 0.01);
        assert!((state.player.y - (bed_y as f64 - 0.1)).abs() < 0.01);
        assert_eq!(state.spawn_point, Some((bed_x, bed_y)));
    }

    #[test]
    fn test_respawn_clears_invalid_bed_spawnpoint_and_falls_back() {
        let mut state = GameState::new();
        let bad_bed_x = 3072;
        let bad_bed_y = 9;
        state.current_dimension = Dimension::Overworld;
        state.world = World::new_for_dimension(Dimension::Overworld);
        prepare_test_respawn_ground(&mut state);
        state.world.load_chunks_around(bad_bed_x);
        state.world.set_block(bad_bed_x, bad_bed_y, BlockType::Air);
        state.spawn_point = Some((bad_bed_x, bad_bed_y));
        state.death_screen_active = true;
        state.death_screen_ticks = DEATH_RESPAWN_DELAY_TICKS;

        state.respawn_from_death_screen();

        assert_eq!(state.spawn_point, None);
        assert!((-55.5..=56.5).contains(&state.player.x));
    }

    #[test]
    fn test_keep_inventory_preset_preserves_inventory_on_respawn() {
        let mut state = GameState::new();
        state.game_rules_preset = GameRulesPreset::KeepInventory;
        state.game_rules = GameRulesPreset::KeepInventory.rules();
        state.inventory.slots[0] = Some(ItemStack {
            item_type: ItemType::DiamondSword,
            count: 1,
            durability: Some(1200),
        });
        state.set_total_experience(250);
        state.player.health = -1.0;
        state.update(0, 0);
        assert!(state.is_showing_death_screen());

        state.death_screen_ticks = DEATH_RESPAWN_DELAY_TICKS;
        state.respawn_from_death_screen();

        assert!(
            state.inventory.slots[0]
                .as_ref()
                .is_some_and(|stack| stack.item_type == ItemType::DiamondSword)
        );
        assert_eq!(state.player.experience_total, 250);
    }

    #[test]
    fn test_respawn_without_keep_inventory_clears_experience() {
        let mut state = GameState::new();
        state.game_rules_preset = GameRulesPreset::Vanilla;
        state.game_rules = GameRulesPreset::Vanilla.rules();
        state.set_total_experience(250);
        state.player.health = -1.0;
        state.update(0, 0);
        assert!(state.is_showing_death_screen());

        state.death_screen_ticks = DEATH_RESPAWN_DELAY_TICKS;
        state.respawn_from_death_screen();

        assert_eq!(state.player.experience_total, 0);
        assert_eq!(state.player.experience_level, 0);
    }

    #[test]
    fn test_experience_orb_pickup_awards_experience() {
        let mut state = GameState::new();
        state.game_rules.do_mob_spawning = false;
        state.set_total_experience(0);
        state.experience_orbs.clear();
        state
            .experience_orbs
            .push(ExperienceOrb::new(state.player.x, state.player.y - 1.0, 7));

        state.update(state.player.x.floor() as i32, state.player.y.floor() as i32);

        assert_eq!(state.player.experience_total, 7);
        assert_eq!(state.player.experience_level, 1);
        assert!(state.experience_orbs.is_empty());
    }

    #[test]
    fn test_daylight_cycle_advances_more_slowly() {
        let mut state = GameState::new();
        configure_quiet_world(&mut state);
        state.time_of_day = 1000.0;

        state.update(state.player.x.floor() as i32, state.player.y.floor() as i32);

        assert!((state.time_of_day - 1004.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_mob_death_without_player_credit_drops_no_experience() {
        let mut state = GameState::new();
        configure_quiet_world(&mut state);
        state.experience_orbs.clear();
        state
            .cows
            .push(Cow::new(state.player.x + 1.0, state.player.y));
        state.cows[0].health = 0.0;

        state.update(state.player.x.floor() as i32, state.player.y.floor() as i32);

        assert!(state.experience_orbs.is_empty());
    }

    #[test]
    fn test_mob_death_with_recent_player_credit_drops_experience() {
        let mut state = GameState::new();
        configure_quiet_world(&mut state);
        state.experience_orbs.clear();
        state.world_tick = 10;
        state
            .cows
            .push(Cow::new(state.player.x + 1.0, state.player.y));
        state.cows[0].last_player_damage_tick = 10;
        state.cows[0].health = 0.0;

        state.update(state.player.x.floor() as i32, state.player.y.floor() as i32);

        assert!(!state.experience_orbs.is_empty());
    }

    #[test]
    fn test_item_pickup_does_not_delete_item_when_inventory_is_full() {
        let mut state = GameState::new();
        state.game_rules.do_mob_spawning = false;
        state.item_entities.clear();
        for slot in &mut state.inventory.slots {
            *slot = Some(ItemStack {
                item_type: ItemType::Dirt,
                count: ItemType::Dirt.max_stack_size(),
                durability: None,
            });
        }

        state.item_entities.push(ItemEntity::new(
            state.player.x,
            state.player.y - 1.0,
            ItemType::Diamond,
        ));

        state.update(state.player.x.floor() as i32, state.player.y.floor() as i32);

        assert!(!state.inventory.has_item(ItemType::Diamond, 1));
        assert_eq!(state.item_entities.len(), 1);
        assert_eq!(state.item_entities[0].item_type, ItemType::Diamond);
    }

    #[test]
    fn test_sheep_death_drops_wool_only() {
        let mut state = GameState::new();
        state.game_rules.do_mob_spawning = false;
        state.sheep.clear();
        state.item_entities.clear();
        state
            .sheep
            .push(Sheep::new(state.player.x + 1.0, state.player.y));
        state.sheep[0].health = 0.0;

        state.update(state.player.x.floor() as i32, state.player.y.floor() as i32);

        let wool_drops = state
            .item_entities
            .iter()
            .filter(|drop| drop.item_type == ItemType::Wool)
            .count();
        let mutton_drops = state
            .item_entities
            .iter()
            .filter(|drop| drop.item_type == ItemType::RawMutton)
            .count();
        assert!(wool_drops >= 1);
        assert_eq!(mutton_drops, 0);
    }

    #[test]
    fn test_shears_shear_sheep_without_dealing_damage() {
        let mut state = GameState::new();
        configure_quiet_world(&mut state);
        for x in -8..=8 {
            for y in 0..20 {
                state.world.set_block(x, y, BlockType::Air);
            }
            state.world.set_block(x, 10, BlockType::Stone);
        }

        state.player.x = 0.5;
        state.player.y = 10.0;
        state.player.attack_timer = 0;
        state.left_click_down = true;
        state.hotbar_index = 0;
        state.inventory.slots[0] = Some(ItemStack {
            item_type: ItemType::Shears,
            count: 1,
            durability: Some(12),
        });
        state.sheep.push(Sheep::new(2.5, 10.0));
        let start_health = state.sheep[0].health;

        state.update(2, 9);

        assert_eq!(state.sheep.len(), 1);
        assert!(state.sheep[0].sheared);
        assert_eq!(state.sheep[0].health, start_health);
        let wool_drops = state
            .item_entities
            .iter()
            .filter(|drop| drop.item_type == ItemType::Wool)
            .count();
        assert!((1..=3).contains(&wool_drops));
        assert_eq!(
            state.inventory.slots[0]
                .as_ref()
                .and_then(|stack| stack.durability),
            Some(11)
        );
    }

    #[test]
    fn test_sheared_sheep_death_drops_no_additional_wool() {
        let mut state = GameState::new();
        state.game_rules.do_mob_spawning = false;
        state.sheep.clear();
        state.item_entities.clear();
        let mut sheep = Sheep::new(state.player.x + 1.0, state.player.y);
        sheep.sheared = true;
        sheep.health = 0.0;
        state.sheep.push(sheep);

        state.update(state.player.x.floor() as i32, state.player.y.floor() as i32);

        let wool_drops = state
            .item_entities
            .iter()
            .filter(|drop| drop.item_type == ItemType::Wool)
            .count();
        assert_eq!(wool_drops, 0);
    }

    #[test]
    fn test_pig_death_drops_porkchop() {
        let mut state = GameState::new();
        state.game_rules.do_mob_spawning = false;
        state.pigs.clear();
        state.item_entities.clear();
        state
            .pigs
            .push(Pig::new(state.player.x + 1.0, state.player.y));
        state.pigs[0].health = 0.0;

        state.update(state.player.x.floor() as i32, state.player.y.floor() as i32);

        let porkchop_drops = state
            .item_entities
            .iter()
            .filter(|drop| drop.item_type == ItemType::RawPorkchop)
            .count();
        assert!(porkchop_drops >= 1);
    }

    #[test]
    fn test_chicken_death_drops_raw_chicken_and_feather() {
        let mut state = GameState::new();
        state.game_rules.do_mob_spawning = false;
        state.chickens.clear();
        state.item_entities.clear();
        state
            .chickens
            .push(Chicken::new(state.player.x + 1.0, state.player.y));
        state.chickens[0].health = 0.0;

        state.update(state.player.x.floor() as i32, state.player.y.floor() as i32);

        let raw_chicken_drops = state
            .item_entities
            .iter()
            .filter(|drop| drop.item_type == ItemType::RawChicken)
            .count();
        let feather_drops = state
            .item_entities
            .iter()
            .filter(|drop| drop.item_type == ItemType::Feather)
            .count();
        assert!(raw_chicken_drops >= 1);
        assert!(feather_drops >= 1);
    }

    #[test]
    fn test_respawn_is_blocked_until_delay_elapsed() {
        let mut state = GameState::new();
        state.player.health = -1.0;
        state.update(0, 0);
        assert!(state.is_showing_death_screen());
        assert!(!state.can_respawn_from_death_screen());

        state.respawn_from_death_screen();
        assert!(state.is_showing_death_screen());

        state.death_screen_ticks = DEATH_RESPAWN_DELAY_TICKS;
        assert!(state.can_respawn_from_death_screen());
        state.respawn_from_death_screen();
        assert!(!state.is_showing_death_screen());
    }

    #[test]
    fn test_respawn_grace_blocks_damage_temporarily() {
        let mut state = GameState::new();
        state.player.health = -1.0;
        state.update(0, 0);
        state.death_screen_ticks = DEATH_RESPAWN_DELAY_TICKS;
        state.respawn_from_death_screen();

        let hp_after_respawn = state.player.health;
        state.apply_player_damage(4.0);
        assert_eq!(state.player.health, hp_after_respawn);

        state.respawn_grace_ticks = 0;
        state.apply_player_damage(4.0);
        assert!(state.player.health < hp_after_respawn);
    }

    #[test]
    fn test_player_combat_hurt_cooldown_blocks_immediate_rehit() {
        let mut state = GameState::new();
        state.respawn_grace_ticks = 0;
        state.player.health = 20.0;

        assert!(state.apply_player_combat_damage(2.0));
        let after_first = state.player.health;
        assert!(after_first < 20.0);
        assert!(state.player_combat_hurt_cooldown > 0);

        assert!(!state.apply_player_combat_damage(2.0));
        assert!((state.player.health - after_first).abs() < 0.001);
    }

    #[test]
    fn test_cactus_contact_damages_player() {
        let mut state = GameState::new();
        configure_quiet_world(&mut state);
        state.respawn_grace_ticks = 0;
        state.player.health = 20.0;
        state.player.age = 9;

        for x in -4..=4 {
            for y in 0..20 {
                state.world.set_block(x, y, BlockType::Air);
            }
        }
        state.world.set_block(1, 10, BlockType::Sand);
        state.world.set_block(1, 9, BlockType::Cactus);

        state.player.x = 1.25;
        state.player.y = 10.0;
        state.player.vx = 0.0;
        state.player.vy = 0.0;
        state.player.grounded = false;

        state.update(0, 0);

        assert!(state.player.health < 20.0);
    }

    #[test]
    fn test_zombie_contact_attack_has_cadence_cooldown() {
        let mut state = GameState::new();
        configure_quiet_world(&mut state);
        for x in -8..=8 {
            for y in 0..20 {
                state.world.set_block(x, y, BlockType::Air);
            }
            state.world.set_block(x, 10, BlockType::Stone);
        }
        state.time_of_day = 23000.0; // Night: avoid daylight burning for undead.
        state.player.x = 0.5;
        state.player.y = 10.0;
        state.player.vx = 0.0;
        state.player.vy = 0.0;
        state.player.grounded = true;
        state.player.health = 20.0;
        state.player.hunger = 10.0; // No passive regen or starvation damage.
        state.zombies.push(Zombie::new(0.6, 10.0));
        state.zombies[0].grounded = true;

        state.update(0, 0);
        assert_eq!(
            state.player.health, 20.0,
            "first contact must warn, not deal instant damage"
        );
        assert_eq!(state.zombies[0].attack_windup, 8);
        for _ in 0..8 {
            state.update(0, 0);
        }
        let after_first_hit = state.player.health;
        assert!(after_first_hit < 20.0);

        for _ in 0..5 {
            state.player.x = 0.5;
            state.player.y = 10.0;
            state.player.vx = 0.0;
            state.player.vy = 0.0;
            state.player.grounded = true;
            state.zombies[0].x = 0.6;
            state.zombies[0].y = 10.0;
            state.zombies[0].vx = 0.0;
            state.zombies[0].vy = 0.0;
            state.zombies[0].grounded = true;
            state.update(0, 0);
        }
        let during_cooldown_hp = state.player.health;
        assert!((during_cooldown_hp - after_first_hit).abs() < 0.001);

        for _ in 0..25 {
            state.player.x = 0.5;
            state.player.y = 10.0;
            state.player.vx = 0.0;
            state.player.vy = 0.0;
            state.player.grounded = true;
            state.zombies[0].x = 0.6;
            state.zombies[0].y = 10.0;
            state.zombies[0].vx = 0.0;
            state.zombies[0].vy = 0.0;
            state.zombies[0].grounded = true;
            state.update(0, 0);
            if state.player.health + 0.001 < during_cooldown_hp {
                break;
            }
        }
        assert!(state.player.health + 0.001 < during_cooldown_hp);
    }

    #[test]
    fn test_starvation_damage_respects_difficulty_floors() {
        fn setup_starvation_state(difficulty: Difficulty) -> GameState {
            let mut state = GameState::new();
            configure_quiet_world(&mut state);
            state.difficulty = difficulty;
            state.player.health = 20.0;
            state.player.hunger = 0.0;
            state.player.x = 0.5;
            state.player.y = 10.0;
            state.player.vx = 0.0;
            state.player.vy = 0.0;
            state.player.grounded = true;
            for x in -8..=8 {
                for y in 0..20 {
                    state.world.set_block(x, y, BlockType::Air);
                }
                state.world.set_block(x, 10, BlockType::Stone);
            }
            state
        }

        let mut easy = setup_starvation_state(Difficulty::Easy);
        for _ in 0..4000 {
            easy.update(0, 0);
        }
        assert!(easy.player.health >= 10.0);

        let mut normal = setup_starvation_state(Difficulty::Normal);
        for _ in 0..4000 {
            normal.update(0, 0);
        }
        assert!(normal.player.health >= 1.0);

        let mut hard = setup_starvation_state(Difficulty::Hard);
        for _ in 0..4000 {
            hard.update(0, 0);
            if hard.player.health <= 0.0 {
                break;
            }
        }
        assert!(hard.player.health <= 0.0);
    }

    #[test]
    fn test_melee_contact_requires_los_and_vertical_overlap() {
        let mut state = GameState::new();
        state.world.load_chunks_around(0);
        for x in -6..=6 {
            for y in 0..20 {
                state.world.set_block(x, y, BlockType::Air);
            }
            state.world.set_block(x, 10, BlockType::Stone);
        }
        state.player.x = 0.5;
        state.player.y = 10.0;

        assert!(state.can_melee_contact_player(1.95, 9.1, 1.6, 1.05));

        state.world.set_block(1, 9, BlockType::Stone);
        assert!(!state.can_melee_contact_player(1.95, 9.1, 1.6, 1.05));

        assert!(!state.can_melee_contact_player(1.95, 7.6, 1.6, 1.05));
    }

    #[test]
    fn test_player_contact_knockback_applies_damping_when_aligned() {
        let mut state = GameState::new();
        state.player.x = 2.0;
        state.player.vx = 0.5;
        state.player.vy = 0.0;
        state.player.grounded = true;

        state.apply_player_contact_knockback(0.0, 0.4, 0.0);
        assert!((state.player.vx - 0.7).abs() < 0.001);
        assert!(state.player.grounded);

        state.apply_player_contact_knockback(0.0, 0.3, 0.22);
        assert!(state.player.vy <= -0.22);
        assert!(!state.player.grounded);
    }

    #[test]
    fn test_peaceful_mode_clears_hostiles() {
        let mut state = GameState::new();
        state.zombies.push(Zombie::new(1.5, 10.0));
        state.creepers.push(Creeper::new(2.5, 10.0));
        state.skeletons.push(Skeleton::new(3.5, 10.0));
        state.spiders.push(Spider::new(4.5, 10.0));
        state.silverfish.push(Silverfish::new(5.5, 10.0));
        state.pigmen.push(ZombiePigman::new(6.5, 10.0));
        state.ghasts.push(Ghast::new(7.5, 20.0));
        state.blazes.push(Blaze::new(8.5, 20.0));
        state.endermen.push(Enderman::new(9.5, 10.0));
        state.arrows.push(Arrow::new_hostile(1.0, 8.0, 0.2, 0.0));
        state.fireballs.push(Fireball::new(1.0, 8.0, 0.1, 0.0));
        state.difficulty = Difficulty::Peaceful;

        state.update(0, 0);

        assert!(state.zombies.is_empty());
        assert!(state.creepers.is_empty());
        assert!(state.skeletons.is_empty());
        assert!(state.spiders.is_empty());
        assert!(state.silverfish.is_empty());
        assert!(state.pigmen.is_empty());
        assert!(state.ghasts.is_empty());
        assert!(state.blazes.is_empty());
        assert!(state.endermen.is_empty());
        assert!(state.arrows.is_empty());
        assert!(state.fireballs.is_empty());
    }

    #[test]
    fn test_builder_preset_freezes_daylight_progression() {
        let mut state = GameState::new();
        state.game_rules_preset = GameRulesPreset::Builder;
        state.game_rules = GameRulesPreset::Builder.rules();
        state.time_of_day = 12345.0;

        state.update(0, 0);

        assert_eq!(state.time_of_day, 12345.0);
    }

    #[test]
    fn test_weather_cycle_gamerule_freezes_weather_timer() {
        use rand::{SeedableRng, rngs::StdRng};

        let mut state = GameState::new();
        state.game_rules_preset = GameRulesPreset::Builder;
        state.game_rules = GameRulesPreset::Builder.rules();
        state.current_dimension = Dimension::Overworld;
        state.weather = WeatherType::Rain;
        state.weather_timer = 321;
        let mut rng = StdRng::seed_from_u64(7);

        state.update_weather(&mut rng);

        assert_eq!(state.weather, WeatherType::Rain);
        assert_eq!(state.weather_timer, 321);
    }

    #[test]
    fn test_hostile_damage_scaling_changes_with_difficulty() {
        let mut state = GameState::new();
        state.difficulty = Difficulty::Easy;
        let easy = state.scaled_hostile_damage(1.0);
        state.difficulty = Difficulty::Normal;
        let normal = state.scaled_hostile_damage(1.0);
        state.difficulty = Difficulty::Hard;
        let hard = state.scaled_hostile_damage(1.0);

        assert!(easy < normal);
        assert!(hard > normal);
    }

    #[test]
    fn test_ambient_overworld_hostile_respawn_adds_pressure() {
        use rand::{SeedableRng, rngs::StdRng};

        let mut state = GameState::new();
        state.time_of_day = 23000.0; // The fixture exercises an unlit outdoor night.
        for center in [-96, -64, -32, 0, 32, 64, 96] {
            state.world.load_chunks_around(center);
        }
        state.world.newly_generated_chunks.clear();
        state.current_dimension = Dimension::Overworld;
        state.player.x = 0.5;
        state.player.y = 11.9;
        state.zombies.clear();
        state.creepers.clear();
        state.skeletons.clear();
        state.spiders.clear();

        for x in -96..=96 {
            for y in 0..24 {
                state.world.set_block(x, y, BlockType::Air);
            }
            state.world.set_block(x, 12, BlockType::Stone);
        }

        state.overworld_hostile_spawn_timer = 0;
        let before = state.zombies.len()
            + state.creepers.len()
            + state.skeletons.len()
            + state.spiders.len();
        let mut rng = StdRng::seed_from_u64(42);
        state.update_ambient_respawns(&mut rng, false);
        let after = state.zombies.len()
            + state.creepers.len()
            + state.skeletons.len()
            + state.spiders.len();
        assert!(after > before);
    }

    fn enderman_refill_fixture() -> GameState {
        let mut state = GameState::network_view();
        state.difficulty = Difficulty::Normal;
        state.time_of_day = 23000.0;
        state.player.x = 0.5;
        state.player.y = 19.9;
        for cx in -2..=2 {
            let mut blocks = vec![BlockType::Air; CHUNK_WIDTH * CHUNK_HEIGHT];
            for x in 0..CHUNK_WIDTH {
                blocks[20 * CHUNK_WIDTH + x] = BlockType::Stone;
            }
            state.world.apply_chunk_column_snapshot(cx, &blocks);
        }
        assert!(state.world.newly_generated_chunks.is_empty());
        state
    }

    #[test]
    fn test_enderman_refill_works_in_explored_world_at_full_ordinary_mob_cap() {
        use rand::{SeedableRng, rngs::StdRng};
        let mut state = enderman_refill_fixture();
        state.zombies = (0..OVERWORLD_HOSTILE_CAP)
            .map(|_| Zombie::new(20.5, 19.9))
            .collect();
        let mut rng = StdRng::seed_from_u64(20261026);
        for _ in 0..20 * 60 * 3 {
            state.update_ambient_respawns(&mut rng, false);
            assert!(state.endermen.len() <= 2);
        }
        assert_eq!(state.endermen.len(), 2);
        assert!(state.world.newly_generated_chunks.is_empty());
        for enderman in &state.endermen {
            assert!((enderman.x - state.player.x).abs() >= 12.0);
            assert!((enderman.x - state.player.x).abs() <= 40.0);
            assert!(!state.entity_collides_with_world(enderman.x, enderman.y, 0.3, 2.7));
        }
        state.endermen.clear();
        state.overworld_hostile_spawn_timer = 60;
        for _ in 0..60 {
            state.update_ambient_respawns(&mut rng, false);
        }
        assert!(
            state.endermen.is_empty(),
            "refill must respect the shared clock"
        );
    }

    #[test]
    fn test_enderman_refill_is_rare_and_respects_daylight_rules_and_headroom() {
        use rand::{SeedableRng, rngs::StdRng};
        let mut rng = StdRng::seed_from_u64(912);
        let mut state = enderman_refill_fixture();
        let mut arrivals = 0;
        for _ in 0..1000 {
            state.endermen.clear();
            arrivals += usize::from(state.try_refill_overworld_enderman(&mut rng));
        }
        assert!(
            (40..130).contains(&arrivals),
            "unexpected refill share: {arrivals}"
        );
        for scenario in 0..5 {
            let mut state = enderman_refill_fixture();
            match scenario {
                0 => state.time_of_day = 12000.0,
                1 => state.difficulty = Difficulty::Peaceful,
                2 => state.game_rules.do_mob_spawning = false,
                3 => {
                    for x in -48..=48 {
                        state.world.set_block(x, 17, BlockType::Stone);
                    }
                }
                _ => {
                    for x in (-48..=48).step_by(4) {
                        state.world.set_block(x, 19, BlockType::Torch);
                    }
                }
            }
            for _ in 0..300 {
                assert!(
                    !state.try_refill_overworld_enderman(&mut rng),
                    "scenario {scenario}"
                );
            }
            assert!(state.endermen.is_empty());
        }
    }

    #[test]
    fn test_overworld_hostile_selection_keeps_creepers_as_minor_share() {
        use rand::{SeedableRng, rngs::StdRng};

        let state = GameState::new();
        let mut rng = StdRng::seed_from_u64(1337);
        let mut zombies = 0usize;
        let mut creepers = 0usize;
        let mut skeletons = 0usize;
        let mut spiders = 0usize;

        for _ in 0..4000 {
            match state.choose_overworld_hostile_spawn(&mut rng) {
                0 => zombies += 1,
                1 => creepers += 1,
                2 => skeletons += 1,
                _ => spiders += 1,
            }
        }

        assert!(
            creepers < skeletons,
            "creepers should be rarer than skeletons"
        );
        assert!(creepers < spiders, "creepers should be rarer than spiders");
        assert!(creepers < zombies, "creepers should be rarer than zombies");
    }

    #[test]
    fn test_overworld_slime_spawn_chance_scales_down_with_height() {
        use rand::{SeedableRng, rngs::StdRng};

        let state = GameState::new();
        let mut low_rng = StdRng::seed_from_u64(17);
        let mut high_rng = StdRng::seed_from_u64(17);

        let mut low_hits = 0;
        let mut high_hits = 0;
        for _ in 0..2000 {
            if state.should_spawn_overworld_slime(70.0, &mut low_rng) {
                low_hits += 1;
            }
            if state.should_spawn_overworld_slime(110.0, &mut high_rng) {
                high_hits += 1;
            }
        }
        assert!(low_hits > high_hits);
    }

    #[test]
    fn test_find_water_spawn_for_squid_needs_two_water_blocks() {
        let mut state = GameState::new();
        state.world.load_chunks_around(0);
        for y in 0..CHUNK_HEIGHT as i32 {
            state.world.set_block(0, y, BlockType::Air);
        }
        state.world.set_block(0, 44, BlockType::Water(8));
        assert!(state.find_water_spawn_for_squid(0).is_none());

        state.world.set_block(0, 43, BlockType::Water(8));
        assert!(state.find_water_spawn_for_squid(0).is_some());
    }

    #[test]
    fn test_daylight_passive_spawn_surface_uses_exposed_floor_and_never_a_cave_below_water() {
        let mut state = GameState::new();
        state.world.load_chunks_around(0);
        state.world.newly_generated_chunks.clear();

        for x in -4..=4 {
            for y in 0..32 {
                state.world.set_block(x, y, BlockType::Air);
            }
        }
        for x in -1..=1 {
            state.world.set_block(x, 20, BlockType::Stone);
        }
        state.world.set_block(0, 26, BlockType::Stone);

        assert_eq!(state.find_spawn_surface_for_mob(0), Some(19.9));
        // A three-block ledge is valid habitat without a player spawn runway.
        assert_eq!(state.find_spawn_surface_for_daylight_mob(0), Some(19.9));
        state.world.set_block(0, 19, BlockType::Water(8));
        assert_eq!(state.find_spawn_surface_for_daylight_mob(0), None);
    }

    #[test]
    fn test_overworld_squid_spawn_helper_respects_biome_filter() {
        use rand::{SeedableRng, rngs::StdRng};

        let mut state = GameState::new();
        let mut ocean_x = None;
        let mut plains_x = None;
        for x in -80_000..=80_000 {
            match state.world.get_biome(x) {
                BiomeType::Ocean if ocean_x.is_none() => ocean_x = Some(x),
                BiomeType::Plains if plains_x.is_none() => plains_x = Some(x),
                _ => {}
            }
            if ocean_x.is_some() && plains_x.is_some() {
                break;
            }
        }

        let ocean_x = ocean_x.expect("expected ocean sample");
        let plains_x = plains_x.expect("expected plains sample");
        state.world.load_chunks_around(ocean_x);
        state.world.load_chunks_around(plains_x);
        for y in 30..58 {
            state.world.set_block(ocean_x, y, BlockType::Air);
            state.world.set_block(plains_x, y, BlockType::Air);
        }
        state.world.set_block(ocean_x, 44, BlockType::Water(8));
        state.world.set_block(ocean_x, 43, BlockType::Water(8));
        state.world.set_block(plains_x, 44, BlockType::Water(8));
        state.world.set_block(plains_x, 43, BlockType::Water(8));
        state.player.x = -200_000.0;
        state.player.y = 10.0;

        let mut rng = StdRng::seed_from_u64(91);
        assert!(state.try_spawn_overworld_squid(ocean_x, &mut rng));
        assert!(!state.try_spawn_overworld_squid(plains_x, &mut rng));
    }

    #[test]
    fn test_overworld_wolf_spawn_helper_respects_biome_filter() {
        use rand::{SeedableRng, rngs::StdRng};

        let mut state = GameState::new();
        let mut forest_x = None;
        let mut plains_x = None;
        for x in -80_000..=80_000 {
            match state.world.get_biome(x) {
                BiomeType::Forest if forest_x.is_none() => forest_x = Some(x),
                BiomeType::Plains if plains_x.is_none() => plains_x = Some(x),
                _ => {}
            }
            if forest_x.is_some() && plains_x.is_some() {
                break;
            }
        }

        let forest_x = forest_x.expect("expected forest sample");
        let plains_x = plains_x.expect("expected plains sample");
        state.player.x = -200_000.0;
        state.player.y = 10.0;
        let mut rng = StdRng::seed_from_u64(93);

        let mut forest_spawned = false;
        for _ in 0..20 {
            if state.try_spawn_overworld_wolf(forest_x, 20.0, &mut rng) {
                forest_spawned = true;
                break;
            }
        }
        assert!(forest_spawned);
        assert!(!state.try_spawn_overworld_wolf(plains_x, 20.0, &mut rng));
    }

    #[test]
    fn test_overworld_ocelot_spawn_helper_respects_biome_filter() {
        use rand::{SeedableRng, rngs::StdRng};

        let mut state = GameState::new();
        let mut jungle_x = None;
        let mut plains_x = None;
        for x in -80_000..=80_000 {
            match state.world.get_biome(x) {
                BiomeType::Jungle if jungle_x.is_none() => jungle_x = Some(x),
                BiomeType::Plains if plains_x.is_none() => plains_x = Some(x),
                _ => {}
            }
            if jungle_x.is_some() && plains_x.is_some() {
                break;
            }
        }

        let jungle_x = jungle_x.expect("expected jungle sample");
        let plains_x = plains_x.expect("expected plains sample");
        state.player.x = -200_000.0;
        state.player.y = 10.0;
        let mut rng = StdRng::seed_from_u64(97);

        let mut jungle_spawned = false;
        for _ in 0..24 {
            if state.try_spawn_overworld_ocelot(jungle_x, 20.0, &mut rng) {
                jungle_spawned = true;
                break;
            }
        }
        assert!(jungle_spawned);
        assert!(!state.try_spawn_overworld_ocelot(plains_x, 20.0, &mut rng));
    }

    #[test]
    fn test_arrow_hit_on_wolf_provokes_nearby_pack() {
        let mut state = GameState::new();
        state.wolves.clear();
        state.wolves.push(Wolf::new(0.5, 10.0));
        state.wolves.push(Wolf::new(2.0, 10.0));

        let hit = state.try_apply_player_arrow_hit(0.5, 9.4, 0.3, 0.0, 4.0);
        assert!(hit);
        assert!(state.wolves[0].is_aggressive());
        assert!(state.wolves[1].is_aggressive());
    }

    #[test]
    fn test_arrow_hit_on_ocelot_spooks_nearby_group() {
        let mut state = GameState::new();
        state.ocelots.clear();
        state.ocelots.push(Ocelot::new(0.5, 10.0));
        state.ocelots.push(Ocelot::new(2.0, 10.0));

        let hit = state.try_apply_player_arrow_hit(0.5, 9.4, 0.3, 0.0, 4.0);
        assert!(hit);
        assert!(state.ocelots[0].health < 10.0);
        assert!(state.ocelots[0].panic_timer > 0);
        assert!(state.ocelots[1].panic_timer > 0);
    }

    #[test]
    fn test_wolf_hunts_nearby_sheep() {
        let mut state = GameState::new();
        configure_quiet_world(&mut state);
        for x in -8..=8 {
            for y in 0..20 {
                state.world.set_block(x, y, BlockType::Air);
            }
            state.world.set_block(x, 10, BlockType::Stone);
        }

        state.wolves.push(Wolf::new(0.5, 10.0));
        state.sheep.push(Sheep::new(1.1, 10.0));
        let start_hp = state.sheep[0].health;

        for _ in 0..30 {
            state.update(0, 0);
            if state.sheep[0].health < start_hp {
                break;
            }
        }

        assert!(state.sheep[0].health < start_hp);
    }

    #[test]
    fn test_ocelot_hunts_nearby_chicken() {
        let mut state = GameState::new();
        configure_quiet_world(&mut state);
        for x in -8..=8 {
            for y in 0..20 {
                state.world.set_block(x, y, BlockType::Air);
            }
            state.world.set_block(x, 10, BlockType::Stone);
        }

        state.player.x = -20.0;
        state.player.y = 10.0;
        state.ocelots.push(Ocelot::new(0.5, 10.0));
        state.chickens.push(Chicken::new(1.1, 10.0));
        let start_hp = state.chickens[0].health;
        let mut damaged_or_killed = false;

        for _ in 0..40 {
            state.update(0, 0);
            if state.chickens.is_empty() || state.chickens[0].health < start_hp {
                damaged_or_killed = true;
                break;
            }
        }

        assert!(damaged_or_killed);
    }

    #[test]
    fn test_passive_spawn_helper_respects_biome_filters_and_can_spawn() {
        use rand::{SeedableRng, rngs::StdRng};

        let mut state = GameState::new();
        let mut plains_x = None;
        let mut desert_x = None;
        let mut ocean_x = None;
        for x in -80_000..=80_000 {
            match state.world.get_biome(x) {
                BiomeType::Plains if plains_x.is_none() => plains_x = Some(x),
                BiomeType::Desert if desert_x.is_none() => desert_x = Some(x),
                BiomeType::Ocean if ocean_x.is_none() => ocean_x = Some(x),
                _ => {}
            }
            if plains_x.is_some() && desert_x.is_some() && ocean_x.is_some() {
                break;
            }
        }

        let plains_x = plains_x.expect("expected plains sample");
        let desert_x = desert_x.expect("expected desert sample");
        let ocean_x = ocean_x.expect("expected ocean sample");

        let mut rng = StdRng::seed_from_u64(33);
        let before = state.cows.len() + state.sheep.len() + state.pigs.len() + state.chickens.len();
        assert!(state.try_spawn_overworld_passive_mob(plains_x, 20.0, &mut rng));
        let after = state.cows.len() + state.sheep.len() + state.pigs.len() + state.chickens.len();
        assert_eq!(after, before + 1);

        let blocked_before =
            state.cows.len() + state.sheep.len() + state.pigs.len() + state.chickens.len();
        assert!(!state.try_spawn_overworld_passive_mob(desert_x, 20.0, &mut rng));
        assert!(!state.try_spawn_overworld_passive_mob(ocean_x, 20.0, &mut rng));
        let blocked_after =
            state.cows.len() + state.sheep.len() + state.pigs.len() + state.chickens.len();
        assert_eq!(blocked_before, blocked_after);
    }

    #[test]
    fn test_overworld_ecology_tuning_adjusts_caps_by_biome() {
        let state = GameState::new();
        assert!(
            state.tuned_overworld_passive_cap(BiomeType::Plains)
                > state.tuned_overworld_passive_cap(BiomeType::Desert)
        );
        assert!(
            state.tuned_overworld_squid_cap(BiomeType::Ocean)
                > state.tuned_overworld_squid_cap(BiomeType::Plains)
        );
        assert!(
            state.tuned_overworld_wolf_cap(BiomeType::Taiga)
                > state.tuned_overworld_wolf_cap(BiomeType::Plains)
        );
        assert!(
            state.tuned_overworld_ocelot_cap(BiomeType::Jungle)
                > state.tuned_overworld_ocelot_cap(BiomeType::Plains)
        );
    }

    #[test]
    fn test_reset_spawn_timers_uses_local_biome_tuning() {
        let mut state = GameState::new();
        let mut plains_x = None;
        let mut ocean_x = None;
        for x in -80_000..=80_000 {
            match state.world.get_biome(x) {
                BiomeType::Plains if plains_x.is_none() => plains_x = Some(x),
                BiomeType::Ocean if ocean_x.is_none() => ocean_x = Some(x),
                _ => {}
            }
            if plains_x.is_some() && ocean_x.is_some() {
                break;
            }
        }

        let plains_x = plains_x.expect("expected plains sample");
        let ocean_x = ocean_x.expect("expected ocean sample");

        state.current_dimension = Dimension::Overworld;
        state.player.x = plains_x as f64 + 0.5;
        state.reset_spawn_timers_for_rules();
        let plains_passive_timer = state.overworld_passive_spawn_timer;
        let plains_squid_timer = state.overworld_squid_spawn_timer;

        state.player.x = ocean_x as f64 + 0.5;
        state.reset_spawn_timers_for_rules();
        let ocean_passive_timer = state.overworld_passive_spawn_timer;
        let ocean_squid_timer = state.overworld_squid_spawn_timer;

        assert!(ocean_passive_timer > plains_passive_timer);
        assert!(ocean_squid_timer < plains_squid_timer);
    }

    #[test]
    fn test_collect_village_hut_anchors_detects_hut_signature() {
        let mut state = GameState::new();
        state.current_dimension = Dimension::Overworld;
        state.world.load_chunks_around(0);
        for x in -8..=8 {
            for y in 0..24 {
                state.world.set_block(x, y, BlockType::Air);
            }
            state.world.set_block(x, 10, BlockType::Stone);
        }
        state.world.set_block(1, 9, BlockType::WoodDoor(true));
        state.world.set_block(1, 8, BlockType::WoodDoor(true));
        state.world.set_block(-1, 9, BlockType::Chest);
        state.world.set_block(3, 9, BlockType::CraftingTable);
        state.world.set_block(-2, 7, BlockType::Glass);
        state.world.set_block(4, 7, BlockType::Glass);

        for roof_x in -2..=4 {
            state.world.set_block(roof_x, 6, BlockType::Planks);
        }
        let anchors = state.collect_village_hut_anchors(0);
        assert!(!anchors.is_empty());
        assert!(anchors.contains(&(1, 9)));
        assert!(!anchors.contains(&(1, 8)));
    }

    #[test]
    fn test_update_villager_population_spawns_villagers_near_hut() {
        use rand::{SeedableRng, rngs::StdRng};

        let mut state = GameState::new();
        state.current_dimension = Dimension::Overworld;
        // Population should see only this fixture's home, never an unrelated
        // settlement from a randomly seeded neighbouring generated chunk.
        state.world = World::network_replica(Dimension::Overworld);
        for chunk in -2..=2 {
            state.world.apply_chunk_column_snapshot(
                chunk,
                &vec![BlockType::Air; CHUNK_WIDTH * CHUNK_HEIGHT],
            );
        }
        state.villagers.clear();
        state.player.x = -16.5;
        state.player.y = 9.9;
        for x in -12..=12 {
            for y in 0..28 {
                state.world.set_block(x, y, BlockType::Air);
            }
            state.world.set_block(x, 10, BlockType::Stone);
        }
        state.world.set_block(1, 9, BlockType::WoodDoor(true));
        state.world.set_block(1, 8, BlockType::WoodDoor(true));
        for roof_x in -1..=3 {
            state.world.set_block(roof_x, 7, BlockType::Planks);
        }
        state.world.set_block(-1, 9, BlockType::Chest);
        state.world.set_block(3, 9, BlockType::CraftingTable);
        state.world.set_block(-2, 7, BlockType::Glass);
        state.world.set_block(4, 7, BlockType::Glass);

        state.overworld_villager_spawn_timer = 0;
        let mut rng = StdRng::seed_from_u64(7);
        state.update_villager_population(&mut rng, true);

        assert!(!state.villagers.is_empty());
        assert!(state.villagers.len() <= VILLAGER_MAX_PER_HUT);
        assert!(
            state
                .villagers
                .iter()
                .all(|v| (v.home_x - 1).abs() <= 2 && (v.home_y - 9).abs() <= 1)
        );
        assert!(state.villagers.iter().all(|villager| villager.y > 8.8));
    }

    #[test]
    fn test_update_villager_population_bootstraps_empty_village_before_timer_expires() {
        use rand::{SeedableRng, rngs::StdRng};

        let mut state = isolated_villager_behavior_fixture();
        state.game_rules.do_mob_spawning = true;
        state.current_dimension = Dimension::Overworld;
        state.world.load_chunks_around(0);
        state.villagers.clear();
        state.player.x = -16.5;
        state.player.y = 9.9;
        for x in -12..=12 {
            for y in 0..28 {
                state.world.set_block(x, y, BlockType::Air);
            }
            state.world.set_block(x, 10, BlockType::Stone);
        }
        state.world.set_block(1, 9, BlockType::WoodDoor(true));
        state.world.set_block(1, 8, BlockType::WoodDoor(true));
        for roof_x in -1..=3 {
            state.world.set_block(roof_x, 7, BlockType::Planks);
        }
        state.world.set_block(-1, 9, BlockType::Chest);
        state.world.set_block(3, 9, BlockType::CraftingTable);
        state.world.set_block(-2, 7, BlockType::Glass);
        state.world.set_block(4, 7, BlockType::Glass);

        state.overworld_villager_spawn_timer = OVERWORLD_VILLAGER_RESPAWN_BASE;
        let mut rng = StdRng::seed_from_u64(19);
        state.update_villager_population(&mut rng, true);

        assert_eq!(state.villagers.len(), 1);
        assert_eq!(state.villagers[0].home_x, 1);
        assert_eq!(state.villagers[0].home_y, 9);
    }

    #[test]
    fn test_find_villager_spawn_surface_near_home_rejects_roof_surface() {
        let mut state = GameState::new();
        state.current_dimension = Dimension::Overworld;
        state.world = World::new_for_dimension(Dimension::Overworld);
        state.world.load_chunks_around(0);

        for x in -8..=8 {
            for y in 0..24 {
                state.world.set_block(x, y, BlockType::Air);
            }
            state.world.set_block(x, 10, BlockType::Stone);
        }
        for roof_x in -1..=3 {
            state.world.set_block(roof_x, 7, BlockType::Planks);
        }

        let spawn_y = state
            .find_villager_spawn_surface_near_home(1, 9, 1)
            .expect("expected villager spawn surface near hut");
        assert_eq!(spawn_y, 9.9);
    }

    #[test]
    fn test_find_villager_shelter_surface_prefers_covered_column() {
        let mut state = GameState::new();
        state.current_dimension = Dimension::Overworld;
        state.world.load_chunks_around(0);
        for x in -8..=8 {
            for y in 0..24 {
                state.world.set_block(x, y, BlockType::Air);
            }
            state.world.set_block(x, 10, BlockType::Stone);
        }

        // Roof only over x=1 so this should be the preferred shelter column.
        state.world.set_block(1, 7, BlockType::Planks);

        let shelter = state
            .find_villager_shelter_surface_near_home(1, 9)
            .expect("expected sheltered spawn column");
        assert_eq!(shelter.0, 1);
        assert!(!state.is_exposed_to_sky(1.5, shelter.1 - 1.0));
    }

    fn isolated_villager_behavior_fixture() -> GameState {
        let mut state = GameState::network_view();
        for chunk in -2..=2 {
            state.world.apply_chunk_column_snapshot(
                chunk,
                &vec![BlockType::Air; CHUNK_WIDTH * CHUNK_HEIGHT],
            );
        }
        state.game_rules.do_mob_spawning = false;
        state.game_rules.do_daylight_cycle = false;
        state.game_rules.do_weather_cycle = false;
        state.weather = WeatherType::Clear;
        state
    }

    #[test]
    fn test_night_villager_moves_toward_shelter_and_settles() {
        let mut state = isolated_villager_behavior_fixture();
        state.current_dimension = Dimension::Overworld;
        state.game_rules.do_mob_spawning = false;
        state.world.load_chunks_around(0);
        state.villagers.clear();
        for x in -12..=12 {
            for y in 0..28 {
                state.world.set_block(x, y, BlockType::Air);
            }
            state.world.set_block(x, 10, BlockType::Stone);
        }
        state.world.set_block(1, 7, BlockType::Planks);
        state.villagers.push(Villager::new(-4.5, 9.9, 1, 9));
        state.time_of_day = 23000.0;

        let start_x = state.villagers[0].x;
        for _ in 0..80 {
            state.update(0, 0);
        }
        let villager = &state.villagers[0];
        assert!(villager.x > start_x + 2.0);
        assert!(!state.is_exposed_to_sky(villager.x, villager.y - 1.0));
    }

    #[test]
    fn test_day_villager_bias_keeps_them_closer_to_home_band() {
        let mut state = isolated_villager_behavior_fixture();
        state.current_dimension = Dimension::Overworld;
        state.game_rules.do_mob_spawning = false;
        state.world.load_chunks_around(0);
        state.villagers.clear();
        for x in -24..=24 {
            for y in 0..28 {
                state.world.set_block(x, y, BlockType::Air);
            }
            state.world.set_block(x, 10, BlockType::Stone);
        }
        state.villagers.push(Villager::new(-18.5, 9.9, 1, 9));
        state.time_of_day = 6000.0;

        let start_abs_home_dx = (state.villagers[0].x - 1.5).abs();
        for _ in 0..120 {
            state.update(0, 0);
        }
        let end_abs_home_dx = (state.villagers[0].x - 1.5).abs();
        assert!(end_abs_home_dx + 4.0 < start_abs_home_dx);
    }

    #[test]
    fn test_day_villager_grouping_reduces_same_village_spread() {
        let mut state = isolated_villager_behavior_fixture();
        state.current_dimension = Dimension::Overworld;
        state.game_rules.do_mob_spawning = false;
        state.world.load_chunks_around(0);
        state.villagers.clear();
        for x in -24..=24 {
            for y in 0..28 {
                state.world.set_block(x, y, BlockType::Air);
            }
            state.world.set_block(x, 10, BlockType::Stone);
        }
        state.villagers.push(Villager::new(-8.5, 9.9, 1, 9));
        state.villagers.push(Villager::new(10.5, 9.9, 1, 9));
        // Give both villagers a stable outward wandering intention so that
        // convergence measures the group bias, rather than a random AI roll.
        for (i, villager) in state.villagers.iter_mut().enumerate() {
            villager.wander_timer = u16::MAX;
            villager.facing_right = i == 1;
        }
        state.game_rules.do_weather_cycle = false;
        state.weather = WeatherType::Clear;
        state.time_of_day = 6500.0;

        let start_spread = (state.villagers[0].x - state.villagers[1].x).abs();
        for _ in 0..120 {
            state.update(0, 0);
        }
        let end_spread = (state.villagers[0].x - state.villagers[1].x).abs();
        assert!(end_spread + 5.0 < start_spread);
    }

    #[test]
    fn test_day_rain_villager_seeks_shelter_even_in_daylight() {
        let mut state = isolated_villager_behavior_fixture();
        state.current_dimension = Dimension::Overworld;
        state.game_rules.do_mob_spawning = false;
        state.game_rules.do_weather_cycle = false;
        state.weather = WeatherType::Rain;

        let rain_x = 0; // Replica fixtures use the known Plains biome.
        assert_eq!(state.precipitation_at(rain_x), PrecipitationType::Rain);
        state.world.load_chunks_around(rain_x);
        state.villagers.clear();
        for x in (rain_x - 12)..=(rain_x + 12) {
            for y in 0..28 {
                state.world.set_block(x, y, BlockType::Air);
            }
            state.world.set_block(x, 10, BlockType::Stone);
        }

        let home_x = rain_x + 1;
        for roof_x in (home_x - 1)..=(home_x + 1) {
            state.world.set_block(roof_x, 7, BlockType::Planks);
        }
        state
            .villagers
            .push(Villager::new(home_x as f64 - 5.0, 9.9, home_x, 9));
        state.player.x = home_x as f64 - 8.0;
        state.player.y = 9.9;
        state.time_of_day = 6000.0;

        assert!(state.should_villager_seek_shelter(home_x as f64 + 0.5, 9.9, true));
        let start_x = state.villagers[0].x;
        for _ in 0..100 {
            state.update(home_x, 9);
        }

        let villager = &state.villagers[0];
        assert!(
            villager.x > start_x + 1.5,
            "expected villager to move toward shelter, start_x={start_x}, villager_x={}",
            villager.x
        );
        let home_dx = (villager.x - (home_x as f64 + 0.5)).abs();
        assert!(home_dx < 1.6);
        assert!(state.should_villager_seek_shelter(villager.x, villager.y, true));
        assert!(
            !state.is_exposed_to_sky(villager.x, villager.y - 1.0)
                || !state.is_weather_wet_at(villager.x, villager.y - 1.0)
        );
    }

    #[test]
    fn test_villager_home_reassignment_has_hysteresis_between_huts() {
        use rand::{SeedableRng, rngs::StdRng};

        let mut state = GameState::new();
        state.current_dimension = Dimension::Overworld;
        state.game_rules.do_mob_spawning = false;
        state.world.load_chunks_around(0);
        state.villagers.clear();
        for x in -24..=24 {
            for y in 0..28 {
                state.world.set_block(x, y, BlockType::Air);
            }
            state.world.set_block(x, 10, BlockType::Stone);
        }

        for door_x in [-12, -4] {
            for roof_x in (door_x - 2)..=(door_x + 3) {
                state.world.set_block(roof_x, 6, BlockType::Planks);
            }
            state.world.set_block(door_x, 9, BlockType::WoodDoor(true));
            state.world.set_block(door_x, 8, BlockType::WoodDoor(true));
            state.world.set_block(door_x - 2, 9, BlockType::Chest);
            state
                .world
                .set_block(door_x + 2, 9, BlockType::CraftingTable);
            state.world.set_block(door_x - 1, 7, BlockType::Glass);
            state.world.set_block(door_x + 3, 7, BlockType::Glass);
        }

        state.villagers.push(Villager::new(-7.0, 9.9, -12, 9));
        state.player.x = -7.0;
        state.player.y = 9.9;
        let mut rng = StdRng::seed_from_u64(11);

        state.update_villager_population(&mut rng, true);
        assert_eq!(state.villagers[0].home_x, -12);

        state.villagers[0].x = -1.5;
        state.update_villager_population(&mut rng, true);
        assert_eq!(state.villagers[0].home_x, -4);
    }

    #[test]
    fn test_villager_opens_closed_wood_door_when_pathing() {
        let mut state = GameState::new();
        state.current_dimension = Dimension::Overworld;
        state.game_rules.do_mob_spawning = false;
        state.world.load_chunks_around(0);
        state.villagers.clear();
        build_side_entry_hut(&mut state, 0, 9, true);
        assert!(state.try_open_wood_door_for_entity(-0.2, 9.9, 0.15));
        assert_eq!(state.world.get_block(0, 9), BlockType::WoodDoor(true));
        assert_eq!(state.world.get_block(0, 8), BlockType::WoodDoor(true));
        assert!(state.villager_open_doors.contains_key(&(0, 9)));
    }

    #[test]
    fn test_day_villager_leaves_hut_through_side_door_and_recloses_it() {
        // A generated neighboring village can reassign this deliberately minimal
        // hut's home. Keep the door route independent of random settlement seeds.
        let mut state = isolated_villager_behavior_fixture();
        state.current_dimension = Dimension::Overworld;
        state.game_rules.do_mob_spawning = false;
        state.world.load_chunks_around(0);
        state.villagers.clear();
        build_side_entry_hut(&mut state, 0, 9, true);
        state.villagers.push(Villager::new(2.5, 9.9, 0, 9));
        state.player.x = -10.0;
        state.player.y = 9.9;
        state.time_of_day = 6000.0;

        let start_x = state.villagers[0].x;
        let mut saw_open_door = false;
        let mut was_outside = false;
        let mut reached_outside_offset = false;
        let mut saw_closed_after_exit = false;
        for _ in 0..160 {
            state.update(0, 0);
            if state.world.get_block(0, 9) == BlockType::WoodDoor(true) {
                saw_open_door = true;
            }
            let outside = state.is_exposed_to_sky(state.villagers[0].x, state.villagers[0].y - 1.0);
            was_outside |= outside;
            reached_outside_offset |= state.villagers[0].x < start_x - 1.2;
            if saw_open_door && outside && state.world.get_block(0, 9) == BlockType::WoodDoor(false)
            {
                saw_closed_after_exit = true;
            }
        }

        assert!(
            saw_open_door,
            "day door fixture: villager={:?}, weather={:?}",
            state.villagers[0], state.weather
        );
        assert!(reached_outside_offset);
        assert!(was_outside);
        assert!(saw_closed_after_exit);
    }

    #[test]
    fn test_night_villager_enters_hut_through_side_door_and_recloses_it() {
        let mut state = isolated_villager_behavior_fixture();
        state.current_dimension = Dimension::Overworld;
        state.game_rules.do_mob_spawning = false;
        state.world.load_chunks_around(0);
        state.villagers.clear();
        build_side_entry_hut(&mut state, 0, 9, true);
        state.villagers.push(Villager::new(-3.5, 9.9, 0, 9));
        state.player.x = -10.0;
        state.player.y = 9.9;
        state.time_of_day = 23000.0;

        let mut saw_open_door = false;
        let mut reached_shelter = false;
        let mut saw_closed_after_entry = false;
        for _ in 0..360 {
            state.update(0, 0);
            if state.world.get_block(0, 9) == BlockType::WoodDoor(true) {
                saw_open_door = true;
            }
            let sheltered =
                !state.is_exposed_to_sky(state.villagers[0].x, state.villagers[0].y - 1.0);
            reached_shelter |= sheltered && state.villagers[0].x > 0.8;
            if saw_open_door
                && sheltered
                && state.world.get_block(0, 9) == BlockType::WoodDoor(false)
            {
                saw_closed_after_entry = true;
            }
        }

        if reached_shelter && !saw_closed_after_entry {
            state.villagers[0].x = 2.5;
            state.villagers[0].y = 9.9;
            state.villagers[0].vx = 0.0;
            state.villagers[0].vy = 0.0;
            state.try_close_wood_door_behind_entity(state.villagers[0].x, state.villagers[0].y);
            saw_closed_after_entry = state.world.get_block(0, 9) == BlockType::WoodDoor(false);
        }

        assert!(saw_open_door);
        assert!(reached_shelter);
        assert!(saw_closed_after_entry);
    }

    #[test]
    fn test_two_villagers_enter_one_door_without_repeatedly_closing_it_on_each_other() {
        let mut state = isolated_villager_behavior_fixture();
        state.game_rules.do_mob_spawning = false;
        build_side_entry_hut(&mut state, 0, 9, true);
        state.villagers = vec![
            Villager::new(-3.5, 9.9, 0, 9),
            Villager::new(-4.5, 9.9, 0, 9),
        ];
        state.player.x = -10.0;
        state.player.y = 9.9;
        state.time_of_day = 23000.0;
        let mut previous = false;
        let mut changes = 0;
        for _ in 0..300 {
            state.update(0, 0);
            let open = state.world.get_block(0, 9) == BlockType::WoodDoor(true);
            changes += usize::from(open != previous);
            previous = open;
        }
        assert!(
            state.villagers.iter().all(|v| v.x > 0.8),
            "queue stalled: {:?}",
            state.villagers
        );
        assert!(
            changes > 0 && changes <= 4,
            "door toggled {changes} times for two arrivals"
        );
        assert!(
            !previous,
            "door must close after both villagers have entered"
        );
    }

    #[test]
    fn test_villager_auto_step_crosses_single_block_obstacle() {
        let mut state = isolated_villager_behavior_fixture();
        state.current_dimension = Dimension::Overworld;
        state.game_rules.do_mob_spawning = false;
        state.world.load_chunks_around(0);
        state.villagers.clear();
        for x in -12..=12 {
            for y in 0..24 {
                state.world.set_block(x, y, BlockType::Air);
            }
            state.world.set_block(x, 10, BlockType::Stone);
        }
        state.world.set_block(0, 9, BlockType::Stone);
        state.villagers.push(Villager::new(-3.5, 9.9, 20, 9));
        state.time_of_day = 7000.0;

        for _ in 0..100 {
            state.update(0, 0);
        }
        assert!(state.villagers[0].x > 2.0);
    }

    #[test]
    fn test_zero_distance_skeleton_shot_does_not_create_invalid_velocity() {
        let mut state = GameState::new();
        state.world.load_chunks_around(0);
        state.world.newly_generated_chunks.clear();
        state.zombies.clear();
        state.creepers.clear();
        state.skeletons.clear();
        state.spiders.clear();
        state.pigmen.clear();
        state.ghasts.clear();
        state.cows.clear();
        state.sheep.clear();
        state.item_entities.clear();
        state.arrows.clear();
        state.fireballs.clear();

        for x in -4..=4 {
            for y in 0..20 {
                state.world.set_block(x, y, BlockType::Air);
            }
            state.world.set_block(x, 10, BlockType::Stone);
        }

        state.player.x = 0.5;
        state.player.y = 10.0;
        state.skeletons.push(Skeleton::new(0.5, 10.0));
        state.update(0, 10);

        assert!(
            state
                .arrows
                .iter()
                .all(|a| a.vx.is_finite() && a.vy.is_finite())
        );
    }

    #[test]
    fn test_skeleton_ranged_attack_requires_line_of_sight() {
        let mut state = GameState::new();
        state.world.load_chunks_around(0);
        state.world.newly_generated_chunks.clear();
        state.zombies.clear();
        state.creepers.clear();
        state.skeletons.clear();
        state.spiders.clear();
        state.pigmen.clear();
        state.ghasts.clear();
        state.cows.clear();
        state.sheep.clear();
        state.item_entities.clear();
        state.arrows.clear();
        state.fireballs.clear();
        state.lightning_bolts.clear();

        for x in -8..=8 {
            for y in 0..20 {
                state.world.set_block(x, y, BlockType::Air);
            }
            state.world.set_block(x, 10, BlockType::Stone);
        }
        for y in 6..=12 {
            state.world.set_block(2, y, BlockType::Stone);
        }

        state.player.x = 4.5;
        state.player.y = 10.0;
        let mut skeleton = Skeleton::new(0.5, 10.0);
        skeleton.bow_cooldown = 0;
        skeleton.grounded = true;
        state.skeletons.push(skeleton);

        for _ in 0..30 {
            state.update(4, 9);
        }

        assert!(state.arrows.is_empty());
    }

    #[test]
    fn test_zombie_chase_jump_triggers_when_player_is_above() {
        let mut state = GameState::new();
        state.world.load_chunks_around(0);
        state.world.newly_generated_chunks.clear();
        state.zombies.clear();
        state.creepers.clear();
        state.skeletons.clear();
        state.spiders.clear();
        state.pigmen.clear();
        state.ghasts.clear();
        state.cows.clear();
        state.sheep.clear();
        state.item_entities.clear();
        state.arrows.clear();
        state.fireballs.clear();
        state.lightning_bolts.clear();

        for x in -8..=8 {
            for y in 0..20 {
                state.world.set_block(x, y, BlockType::Air);
            }
            state.world.set_block(x, 12, BlockType::Stone);
        }

        state.player.x = 2.5;
        state.player.y = 10.0;
        let mut zombie = Zombie::new(0.5, 12.0);
        zombie.grounded = true;
        zombie.vx = 0.0;
        zombie.vy = 0.0;
        state.zombies.push(zombie);

        state.update(2, 10);

        assert!(!state.zombies.is_empty());
        assert!(state.zombies[0].vy < -0.2);
    }

    #[test]
    fn test_next_ground_reroute_state_triggers_detour_after_stall() {
        let (stuck_ticks, reroute_ticks, reroute_dir) = GameState::next_ground_reroute_state(
            10.0,
            10.0,
            0.5,
            0.5,
            12.0,
            true,
            true,
            MOB_REROUTE_TRIGGER_TICKS,
            0,
            0,
        );

        assert_eq!(stuck_ticks, 0);
        assert!(reroute_ticks >= MOB_REROUTE_BASE_TICKS);
        assert_eq!(reroute_dir, -1);
    }

    #[test]
    fn test_vertical_recovery_jump_requires_obstacle_ahead() {
        let mut state = GameState::new();
        state.world.load_chunks_around(0);
        state.world.newly_generated_chunks.clear();

        for x in -4..=4 {
            for y in 0..20 {
                state.world.set_block(x, y, BlockType::Air);
            }
            state.world.set_block(x, 12, BlockType::Stone);
        }
        state.world.set_block(1, 11, BlockType::Stone);
        state.player.x = 3.5;
        state.player.y = 9.0;

        let should_jump =
            state.should_ground_mob_vertical_recovery_jump(0.5, 12.0, 0.2, true, 0.3, 1.8, 5, 0);
        assert!(should_jump);

        state.world.set_block(1, 11, BlockType::Air);
        let no_obstacle_jump =
            state.should_ground_mob_vertical_recovery_jump(0.5, 12.0, 0.2, true, 0.3, 1.8, 5, 0);
        assert!(!no_obstacle_jump);
    }

    #[test]
    fn test_ground_pathfinding_direction_on_flat_ground() {
        let mut state = GameState::new();
        state.world.load_chunks_around(0);
        state.world.newly_generated_chunks.clear();

        for x in -12..=12 {
            for y in 0..22 {
                state.world.set_block(x, y, BlockType::Air);
            }
            state.world.set_block(x, 10, BlockType::Stone);
        }

        state.player.x = 7.5;
        state.player.y = 9.9;
        assert_eq!(
            state.find_ground_path_first_step_dir(-6.5, 9.9, 1.8),
            Some(1)
        );

        state.player.x = -7.5;
        assert_eq!(
            state.find_ground_path_first_step_dir(6.5, 9.9, 1.8),
            Some(-1)
        );
    }

    #[test]
    fn test_ground_pathfinding_returns_none_when_fully_blocked() {
        let mut state = GameState::new();
        state.world.load_chunks_around(0);
        state.world.newly_generated_chunks.clear();

        for x in -12..=12 {
            for y in 0..30 {
                state.world.set_block(x, y, BlockType::Air);
            }
            state.world.set_block(x, 20, BlockType::Stone);
        }
        for y in 0..30 {
            state.world.set_block(0, y, BlockType::Stone);
        }

        state.player.x = 6.5;
        state.player.y = 19.9;
        assert_eq!(state.find_ground_path_first_step_dir(-6.5, 19.9, 1.8), None);
    }

    #[test]
    fn test_ground_pathfinding_finds_underpass_route() {
        let mut state = GameState::new();
        state.world.load_chunks_around(0);
        state.world.newly_generated_chunks.clear();

        for x in -16..=16 {
            for y in 0..40 {
                state.world.set_block(x, y, BlockType::Air);
            }
            state.world.set_block(x, 20, BlockType::Stone);
            state.world.set_block(x, 28, BlockType::Stone);
        }
        for x in 1..=5 {
            for y in 17..=27 {
                state.world.set_block(x, y, BlockType::Stone);
            }
            state.world.set_block(x, 27, BlockType::Air);
            state.world.set_block(x, 26, BlockType::Air);
        }
        state.world.set_block(-1, 20, BlockType::Air);
        state.world.set_block(0, 20, BlockType::Air);

        state.player.x = 10.5;
        state.player.y = 27.9;
        assert_eq!(
            state.find_ground_path_first_step_dir(-8.5, 19.9, 1.8),
            Some(1)
        );
    }

    #[test]
    fn test_player_bow_release_fires_arrow_and_consumes_ammo() {
        let mut state = GameState::new();
        state.world.load_chunks_around(0);
        state.world.newly_generated_chunks.clear();
        state.zombies.clear();
        state.creepers.clear();
        state.skeletons.clear();
        state.spiders.clear();
        state.pigmen.clear();
        state.ghasts.clear();
        state.cows.clear();
        state.sheep.clear();
        state.item_entities.clear();
        state.arrows.clear();
        state.fireballs.clear();

        for x in -8..=8 {
            for y in 0..20 {
                state.world.set_block(x, y, BlockType::Air);
            }
            state.world.set_block(x, 10, BlockType::Stone);
        }

        state.player.x = 0.5;
        state.player.y = 10.0;
        state.hotbar_index = 0;
        state.inventory.slots[0] = Some(ItemStack {
            item_type: ItemType::Bow,
            count: 1,
            durability: Some(3),
        });
        state.inventory.slots[1] = Some(ItemStack {
            item_type: ItemType::Arrow,
            count: 5,
            durability: None,
        });

        state.left_click_down = true;
        for _ in 0..8 {
            state.update(6, 9);
        }
        assert!(state.arrows.is_empty());

        state.left_click_down = false;
        state.update(6, 9);

        assert_eq!(state.arrows.len(), 1);
        assert!(state.arrows[0].from_player);
        assert!(state.inventory.has_item(ItemType::Arrow, 4));
        let remaining_bow_durability = state.inventory.slots[0]
            .as_ref()
            .and_then(|stack| stack.durability)
            .unwrap_or(0);
        assert_eq!(remaining_bow_durability, 2);
    }

    #[test]
    fn test_player_arrow_damages_hostile() {
        let mut state = GameState::new();
        state.world.load_chunks_around(0);
        state.world.newly_generated_chunks.clear();
        state.zombies.clear();
        state.creepers.clear();
        state.skeletons.clear();
        state.spiders.clear();
        state.pigmen.clear();
        state.ghasts.clear();
        state.cows.clear();
        state.sheep.clear();
        state.item_entities.clear();
        state.arrows.clear();
        state.fireballs.clear();

        for x in -12..=12 {
            for y in 0..24 {
                state.world.set_block(x, y, BlockType::Air);
            }
            state.world.set_block(x, 10, BlockType::Stone);
        }

        state.player.x = 0.5;
        state.player.y = 10.0;
        state.hotbar_index = 0;
        state.inventory.slots[0] = Some(ItemStack {
            item_type: ItemType::Bow,
            count: 1,
            durability: Some(8),
        });
        state.inventory.slots[1] = Some(ItemStack {
            item_type: ItemType::Arrow,
            count: 6,
            durability: None,
        });
        state.zombies.push(Zombie::new(4.5, 10.0));
        let start_health = state.zombies[0].health;

        state.left_click_down = true;
        for _ in 0..16 {
            state.update(5, 9);
        }
        state.left_click_down = false;
        state.update(5, 9);

        for _ in 0..24 {
            state.update(5, 9);
            if state.zombies.is_empty() || state.zombies[0].health < start_health {
                break;
            }
        }

        assert!(state.zombies.is_empty() || state.zombies[0].health < start_health);
    }

    #[test]
    fn test_fishing_rod_cast_and_reel_catches_fish_when_biting() {
        let mut state = GameState::new();
        state.world.load_chunks_around(0);
        state.world.newly_generated_chunks.clear();
        state.overworld_hostile_spawn_timer = u16::MAX;
        state.overworld_passive_spawn_timer = u16::MAX;
        state.overworld_wolf_spawn_timer = u16::MAX;
        state.overworld_ocelot_spawn_timer = u16::MAX;
        state.nether_spawn_timer = u16::MAX;
        state.end_spawn_timer = u16::MAX;
        state.game_rules = GameRulesPreset::Builder.rules();
        state.game_rules_preset = GameRulesPreset::Builder;

        for x in -8..=8 {
            for y in 0..20 {
                state.world.set_block(x, y, BlockType::Air);
            }
            state.world.set_block(x, 10, BlockType::Stone);
        }
        state.world.set_block(3, 9, BlockType::Water(8));

        state.player.x = 0.5;
        state.player.y = 10.0;
        state.hotbar_index = 0;
        state.inventory.slots[0] = Some(ItemStack {
            item_type: ItemType::FishingRod,
            count: 1,
            durability: Some(5),
        });
        let pre_reel_loot_total = fishing_loot_total_count(&state);

        state.interact_block(3, 9, false);
        assert!(state.fishing_bobber().is_some());

        state.fishing_wait_ticks = 0;
        state.update(3, 9);
        assert!(state.fishing_bobber().is_some_and(|(_, _, bite)| bite));

        state.interact_block(0, 0, false);
        assert!(fishing_loot_total_count(&state) > pre_reel_loot_total);
        assert!(state.fishing_bobber().is_none());
        assert_eq!(
            state.inventory.slots[0]
                .as_ref()
                .and_then(|s| s.durability)
                .unwrap_or(0),
            4
        );
    }

    #[test]
    fn test_fishing_rod_reel_without_bite_does_not_consume_durability() {
        let mut state = GameState::new();
        state.world.load_chunks_around(0);
        state.world.newly_generated_chunks.clear();
        state.overworld_hostile_spawn_timer = u16::MAX;
        state.overworld_passive_spawn_timer = u16::MAX;
        state.overworld_wolf_spawn_timer = u16::MAX;
        state.overworld_ocelot_spawn_timer = u16::MAX;
        state.nether_spawn_timer = u16::MAX;
        state.end_spawn_timer = u16::MAX;
        state.game_rules = GameRulesPreset::Builder.rules();
        state.game_rules_preset = GameRulesPreset::Builder;

        for x in -8..=8 {
            for y in 0..20 {
                state.world.set_block(x, y, BlockType::Air);
            }
            state.world.set_block(x, 10, BlockType::Stone);
        }
        state.world.set_block(2, 9, BlockType::Water(8));

        state.player.x = 0.5;
        state.player.y = 10.0;
        state.hotbar_index = 0;
        state.inventory.slots[0] = Some(ItemStack {
            item_type: ItemType::FishingRod,
            count: 1,
            durability: Some(7),
        });
        let pre_reel_loot_total = fishing_loot_total_count(&state);

        state.interact_block(2, 9, false);
        assert!(state.fishing_bobber().is_some());
        state.interact_block(0, 0, false);

        assert_eq!(fishing_loot_total_count(&state), pre_reel_loot_total);
        assert!(state.fishing_bobber().is_none());
        assert_eq!(
            state.inventory.slots[0]
                .as_ref()
                .and_then(|s| s.durability)
                .unwrap_or(0),
            7
        );
    }

    #[test]
    fn test_switching_off_fishing_rod_clears_bobber() {
        let mut state = GameState::new();
        state.world.load_chunks_around(0);
        state.world.newly_generated_chunks.clear();
        state.overworld_hostile_spawn_timer = u16::MAX;
        state.overworld_passive_spawn_timer = u16::MAX;
        state.overworld_wolf_spawn_timer = u16::MAX;
        state.overworld_ocelot_spawn_timer = u16::MAX;
        state.nether_spawn_timer = u16::MAX;
        state.end_spawn_timer = u16::MAX;
        state.game_rules = GameRulesPreset::Builder.rules();
        state.game_rules_preset = GameRulesPreset::Builder;

        for x in -6..=6 {
            for y in 0..20 {
                state.world.set_block(x, y, BlockType::Air);
            }
            state.world.set_block(x, 10, BlockType::Stone);
        }
        state.world.set_block(1, 9, BlockType::Water(8));

        state.player.x = 0.5;
        state.player.y = 10.0;
        state.hotbar_index = 0;
        state.inventory.slots[0] = Some(ItemStack {
            item_type: ItemType::FishingRod,
            count: 1,
            durability: Some(10),
        });
        state.inventory.slots[1] = Some(ItemStack {
            item_type: ItemType::Stick,
            count: 1,
            durability: None,
        });

        state.interact_block(1, 9, false);
        assert!(state.fishing_bobber().is_some());
        state.hotbar_index = 1;
        state.update(1, 9);
        assert!(state.fishing_bobber().is_none());
    }

    #[test]
    fn test_using_boat_on_water_spawns_boat_and_consumes_item() {
        let mut state = GameState::new();
        configure_quiet_world(&mut state);
        for x in -4..=4 {
            for y in 0..18 {
                state.world.set_block(x, y, BlockType::Air);
            }
            state.world.set_block(x, 10, BlockType::Stone);
            state.world.set_block(x, 9, BlockType::Water(8));
        }
        state.player.x = 0.5;
        state.player.y = 10.0;
        state.hotbar_index = 0;
        state.inventory.slots[0] = Some(ItemStack {
            item_type: ItemType::Boat,
            count: 1,
            durability: None,
        });

        state.interact_block(1, 9, false);

        assert_eq!(state.boats.len(), 1);
        assert_eq!(state.boats[0].x, 1.5);
        assert_eq!(state.boats[0].y, 9.0);
        assert!(state.inventory.slots[0].is_none());
    }

    #[test]
    fn test_clicking_boat_mounts_and_clicking_again_dismounts() {
        let mut state = GameState::new();
        configure_quiet_world(&mut state);
        for x in -4..=4 {
            for y in 0..18 {
                state.world.set_block(x, y, BlockType::Air);
            }
            state.world.set_block(x, 10, BlockType::Stone);
            state.world.set_block(x, 9, BlockType::Water(8));
        }
        state.player.x = 0.5;
        state.player.y = 10.0;
        state.boats.push(Boat::new(1.5, 9.0));

        state.interact_block(1, 9, false);
        assert_eq!(state.mounted_boat, Some(0));
        assert!((state.player.x - 1.5).abs() < 0.01);

        state.interact_block(1, 9, false);
        assert!(state.mounted_boat.is_none());
        assert!((state.player.x - 1.5).abs() > 0.2);
    }

    #[test]
    fn test_mounted_boat_moves_across_water() {
        let mut state = GameState::new();
        configure_quiet_world(&mut state);
        for x in -8..=16 {
            for y in 0..18 {
                state.world.set_block(x, y, BlockType::Air);
            }
            state.world.set_block(x, 10, BlockType::Stone);
            state.world.set_block(x, 9, BlockType::Water(8));
        }
        state.boats.push(Boat::new(0.5, 9.0));
        assert!(state.try_mount_boat(0));
        let start_x = state.boats[0].x;

        state.moving_right = true;
        for _ in 0..12 {
            state.update(0, 0);
        }

        assert!(
            state.boats[0].x > start_x + 0.5,
            "boat x was {}",
            state.boats[0].x
        );
        assert_eq!(state.mounted_boat, Some(0));
    }

    #[test]
    fn test_breaking_boat_drops_boat_item() {
        let mut state = GameState::new();
        configure_quiet_world(&mut state);
        for x in -4..=4 {
            for y in 0..18 {
                state.world.set_block(x, y, BlockType::Air);
            }
            state.world.set_block(x, 10, BlockType::Stone);
            state.world.set_block(x, 9, BlockType::Water(8));
        }
        state.player.x = 0.5;
        state.player.y = 10.0;
        state.boats.push(Boat::new(1.5, 9.0));

        state.interact_block(1, 9, true);

        assert!(state.boats.is_empty());
        assert!(
            state
                .item_entities
                .iter()
                .any(|item| item.item_type == ItemType::Boat)
        );
    }

    #[test]
    fn test_fishing_loot_weights_shift_with_biome_and_weather() {
        let ocean_weights = GameState::fishing_loot_category_weights_for(
            Some(BiomeType::Ocean),
            WeatherType::Clear,
            false,
        );
        let swamp_weights = GameState::fishing_loot_category_weights_for(
            Some(BiomeType::Swamp),
            WeatherType::Clear,
            false,
        );
        assert!(ocean_weights[0] > swamp_weights[0]);
        assert!(swamp_weights[1] > ocean_weights[1]);

        let clear_weights = GameState::fishing_loot_category_weights_for(
            Some(BiomeType::Plains),
            WeatherType::Clear,
            true,
        );
        let thunder_weights = GameState::fishing_loot_category_weights_for(
            Some(BiomeType::Plains),
            WeatherType::Thunderstorm,
            true,
        );
        assert!(thunder_weights[2] > clear_weights[2]);
    }

    #[test]
    fn test_roll_fishing_loot_uses_known_item_pool() {
        let mut state = GameState::new();
        state.current_dimension = Dimension::Overworld;
        state.fishing_bobber_x = 0.5;
        state.fishing_bobber_y = 10.0;
        let mut rng = rand::rngs::StdRng::seed_from_u64(17);

        for _ in 0..512 {
            let (item, count) = state.roll_fishing_loot(&mut rng);
            assert!(count >= 1);
            assert!(matches!(
                item,
                ItemType::RawFish
                    | ItemType::Stick
                    | ItemType::String
                    | ItemType::Leather
                    | ItemType::Bone
                    | ItemType::RottenFlesh
                    | ItemType::WaterBottle
                    | ItemType::LeatherBoots
                    | ItemType::FishingRod
                    | ItemType::Bow
                    | ItemType::IronIngot
                    | ItemType::GoldIngot
                    | ItemType::Diamond
            ));
        }
    }

    #[test]
    fn test_furnace_smelting_is_timed_and_consumes_fuel() {
        let mut state = GameState::new();
        state.world.load_chunks_around(0);
        state.world.newly_generated_chunks.clear();
        state.overworld_hostile_spawn_timer = u16::MAX;
        state.overworld_passive_spawn_timer = u16::MAX;
        state.overworld_wolf_spawn_timer = u16::MAX;
        state.overworld_ocelot_spawn_timer = u16::MAX;
        state.nether_spawn_timer = u16::MAX;
        state.end_spawn_timer = u16::MAX;
        state.zombies.clear();
        state.creepers.clear();
        state.skeletons.clear();
        state.spiders.clear();
        state.pigmen.clear();
        state.ghasts.clear();
        state.cows.clear();
        state.sheep.clear();
        state.item_entities.clear();
        state.arrows.clear();
        state.fireballs.clear();

        state.at_furnace = true;
        state.at_crafting_table = false;
        state.inventory.slots[0] = Some(ItemStack {
            item_type: ItemType::RawIron,
            count: 1,
            durability: None,
        });
        state.inventory.slots[1] = Some(ItemStack {
            item_type: ItemType::Coal,
            count: 1,
            durability: None,
        });

        let iron_idx = Recipe::all()
            .iter()
            .position(|r| r.result == ItemType::IronIngot && r.needs_furnace)
            .expect("iron smelt recipe should exist");
        state.attempt_craft(iron_idx);

        assert!(!state.inventory.has_item(ItemType::IronIngot, 1));
        assert!(!state.inventory.has_item(ItemType::RawIron, 1));

        for _ in 0..(FURNACE_COOK_TICKS - 1) {
            state.update(0, 0);
        }
        assert!(!state.inventory.has_item(ItemType::IronIngot, 1));

        state.update(0, 0);
        assert!(state.inventory.has_item(ItemType::IronIngot, 1));
        assert!(!state.inventory.has_item(ItemType::Coal, 1));
    }

    #[test]
    fn test_furnace_keeps_smelting_with_same_job_when_input_remains() {
        let mut state = GameState::new();
        state.world.load_chunks_around(0);
        state.world.newly_generated_chunks.clear();
        state.overworld_hostile_spawn_timer = u16::MAX;
        state.overworld_passive_spawn_timer = u16::MAX;
        state.overworld_wolf_spawn_timer = u16::MAX;
        state.overworld_ocelot_spawn_timer = u16::MAX;
        state.nether_spawn_timer = u16::MAX;
        state.end_spawn_timer = u16::MAX;
        state.zombies.clear();
        state.creepers.clear();
        state.skeletons.clear();
        state.spiders.clear();
        state.pigmen.clear();
        state.ghasts.clear();
        state.cows.clear();
        state.sheep.clear();
        state.item_entities.clear();
        state.arrows.clear();
        state.fireballs.clear();

        state.at_furnace = true;
        state.at_crafting_table = false;
        state.inventory.slots[0] = Some(ItemStack {
            item_type: ItemType::RawIron,
            count: 2,
            durability: None,
        });
        state.inventory.slots[1] = Some(ItemStack {
            item_type: ItemType::Coal,
            count: 1,
            durability: None,
        });

        let iron_idx = Recipe::all()
            .iter()
            .position(|r| r.result == ItemType::IronIngot && r.needs_furnace)
            .expect("iron smelt recipe should exist");
        state.attempt_craft(iron_idx);

        for _ in 0..(FURNACE_COOK_TICKS * 2) {
            state.update(0, 0);
        }

        assert!(state.inventory.has_item(ItemType::IronIngot, 2));
        assert!(!state.inventory.has_item(ItemType::RawIron, 1));
    }

    #[test]
    fn test_furnace_shift_click_smeltable_item_quick_starts_job() {
        let mut state = GameState::new();
        state.inventory_open = true;
        state.at_furnace = true;
        state.at_crafting_table = false;
        state.at_chest = false;
        state.at_enchanting_table = false;
        state.at_anvil = false;
        state.at_brewing_stand = false;
        state.furnace_job = None;
        state.furnace_burn_ticks = 0;
        state.inventory = Inventory::new(PLAYER_INVENTORY_CAPACITY);
        state.inventory.slots[0] = Some(ItemStack {
            item_type: ItemType::RawIron,
            count: 1,
            durability: None,
        });
        state.inventory.slots[1] = Some(ItemStack {
            item_type: ItemType::Coal,
            count: 1,
            durability: None,
        });

        state.handle_inventory_shift_click(0);

        let job = state.furnace_job.expect("furnace should start quick job");
        assert_eq!(job.input, ItemType::RawIron);
        assert_eq!(job.output, ItemType::IronIngot);
        assert!(!state.inventory.has_item(ItemType::RawIron, 1));
        assert!(!state.inventory.has_item(ItemType::Coal, 1));
        assert!(state.furnace_burn_ticks > 0);
    }

    #[test]
    fn test_furnace_shift_click_coal_quick_starts_available_job() {
        let mut state = GameState::new();
        state.inventory_open = true;
        state.at_furnace = true;
        state.at_crafting_table = false;
        state.at_chest = false;
        state.at_enchanting_table = false;
        state.at_anvil = false;
        state.at_brewing_stand = false;
        state.furnace_job = None;
        state.furnace_burn_ticks = 0;
        state.inventory = Inventory::new(PLAYER_INVENTORY_CAPACITY);
        state.inventory.slots[0] = Some(ItemStack {
            item_type: ItemType::Coal,
            count: 1,
            durability: None,
        });
        state.inventory.slots[1] = Some(ItemStack {
            item_type: ItemType::RawChicken,
            count: 1,
            durability: None,
        });

        state.handle_inventory_shift_click(0);

        let job = state
            .furnace_job
            .expect("coal quick-start should pick a job");
        assert_eq!(job.input, ItemType::RawChicken);
        assert_eq!(job.output, ItemType::CookedChicken);
        assert!(!state.inventory.has_item(ItemType::RawChicken, 1));
        assert!(!state.inventory.has_item(ItemType::Coal, 1));
    }

    #[test]
    fn test_furnace_shift_click_without_fuel_falls_back_to_inventory_transfer() {
        let mut state = GameState::new();
        state.inventory_open = true;
        state.at_furnace = true;
        state.at_crafting_table = false;
        state.at_chest = false;
        state.at_enchanting_table = false;
        state.at_anvil = false;
        state.at_brewing_stand = false;
        state.furnace_job = None;
        state.furnace_burn_ticks = 0;
        state.inventory = Inventory::new(PLAYER_INVENTORY_CAPACITY);
        state.inventory.slots[0] = Some(ItemStack {
            item_type: ItemType::RawIron,
            count: 1,
            durability: None,
        });

        state.handle_inventory_shift_click(0);

        assert!(state.furnace_job.is_none());
        assert!(state.inventory.slots[0].is_none());
        assert_eq!(
            state.inventory.slots[PLAYER_HOTBAR_SLOTS]
                .as_ref()
                .map(|s| (s.item_type, s.count)),
            Some((ItemType::RawIron, 1))
        );
    }

    #[test]
    fn test_two_by_two_grid_crafting_makes_sticks() {
        let mut state = GameState::new();
        state.inventory = Inventory::new(PLAYER_INVENTORY_CAPACITY);
        state.inventory_enchant_levels = [0; PLAYER_INVENTORY_CAPACITY];
        state.inventory_open = true;
        state.at_crafting_table = false;
        state.at_furnace = false;
        state.at_chest = false;
        state.at_enchanting_table = false;
        state.at_anvil = false;
        state.inventory.slots[0] = Some(ItemStack {
            item_type: ItemType::Planks,
            count: 2,
            durability: None,
        });

        state.handle_inventory_click(0);
        for _ in 0..1 {
            state.handle_inventory_right_click(CRAFT_GRID_UI_OFFSET);
        }
        for _ in 0..1 {
            state.handle_inventory_right_click(CRAFT_GRID_UI_OFFSET + 3);
        }

        assert_eq!(state.crafting_output_preview(), Some((ItemType::Stick, 4)));
        state.handle_inventory_click(CRAFT_OUTPUT_UI_SLOT);
        assert!(state.inventory.has_item(ItemType::Stick, 4));
        assert!(state.crafting_grid.iter().all(Option::is_none));
    }

    #[test]
    fn test_inventory_right_click_splits_and_distributes_single_items() {
        let mut state = GameState::new();
        state.inventory = Inventory::new(PLAYER_INVENTORY_CAPACITY);
        state.inventory_enchant_levels = [0; PLAYER_INVENTORY_CAPACITY];
        state.inventory_open = true;
        state.at_crafting_table = false;
        state.at_furnace = false;
        state.at_chest = false;
        state.at_enchanting_table = false;
        state.at_anvil = false;
        state.inventory.slots[0] = Some(ItemStack {
            item_type: ItemType::Cobblestone,
            count: 8,
            durability: None,
        });

        state.handle_inventory_right_click(0);

        assert_eq!(state.selected_inventory_slot, Some(0));
        assert_eq!(
            state.inventory.slots[0].as_ref().map(|stack| stack.count),
            Some(4)
        );
        assert_eq!(
            state.inventory.slots[1].as_ref().map(|stack| stack.count),
            Some(4)
        );

        state.handle_inventory_right_click(2);

        assert_eq!(
            state.inventory.slots[0].as_ref().map(|stack| stack.count),
            Some(3)
        );
        assert_eq!(
            state.inventory.slots[2].as_ref().map(|stack| stack.count),
            Some(1)
        );
        assert_eq!(state.selected_inventory_slot, Some(0));
    }

    #[test]
    fn test_inventory_right_click_places_one_per_crafting_grid_cell() {
        let mut state = GameState::new();
        state.inventory = Inventory::new(PLAYER_INVENTORY_CAPACITY);
        state.inventory_enchant_levels = [0; PLAYER_INVENTORY_CAPACITY];
        state.inventory_open = true;
        state.at_crafting_table = false;
        state.at_furnace = false;
        state.at_chest = false;
        state.at_enchanting_table = false;
        state.at_anvil = false;
        state.inventory.slots[0] = Some(ItemStack {
            item_type: ItemType::Planks,
            count: 4,
            durability: None,
        });

        state.handle_inventory_right_click(0);
        state.handle_inventory_right_click(CRAFT_GRID_UI_OFFSET);
        state.handle_inventory_right_click(CRAFT_GRID_UI_OFFSET + 3);

        assert_eq!(
            state.crafting_grid_slot_item(0),
            Some(ItemType::Planks),
            "first crafting cell should receive one plank"
        );
        assert_eq!(
            state.crafting_grid_slot_item(3),
            Some(ItemType::Planks),
            "second crafting cell should receive one plank"
        );
        assert_eq!(
            state.crafting_output_preview(),
            Some((ItemType::Stick, 4)),
            "vertical 2x2 plank pair should craft sticks"
        );
    }

    #[test]
    fn test_shift_click_crafting_output_crafts_max_from_stacked_inputs() {
        let mut state = GameState::new();
        state.inventory = Inventory::new(PLAYER_INVENTORY_CAPACITY);
        state.inventory_enchant_levels = [0; PLAYER_INVENTORY_CAPACITY];
        state.inventory_open = true;
        state.at_crafting_table = false;
        state.at_furnace = false;
        state.at_chest = false;
        state.at_enchanting_table = false;
        state.at_anvil = false;
        state.inventory.slots[0] = Some(ItemStack {
            item_type: ItemType::Planks,
            count: 16,
            durability: None,
        });

        state.handle_inventory_click(0);
        for _ in 0..4 {
            state.handle_inventory_right_click(CRAFT_GRID_UI_OFFSET);
        }
        for _ in 0..4 {
            state.handle_inventory_right_click(CRAFT_GRID_UI_OFFSET + 3);
        }

        assert_eq!(
            state.crafting_grid_slot_stack(0).map(|stack| stack.count),
            Some(4)
        );
        assert_eq!(
            state.crafting_grid_slot_stack(3).map(|stack| stack.count),
            Some(4)
        );
        assert_eq!(state.crafting_output_preview(), Some((ItemType::Stick, 4)));

        state.handle_inventory_shift_click(CRAFT_OUTPUT_UI_SLOT);

        assert!(state.inventory.has_item(ItemType::Stick, 16));
        assert_eq!(
            state.inventory.slots[0]
                .as_ref()
                .map(|stack| (stack.item_type, stack.count)),
            Some((ItemType::Planks, 8))
        );
        assert!(state.crafting_grid.iter().all(Option::is_none));
    }

    #[test]
    fn test_inventory_right_click_splits_a_grid_stack_then_merges_it_back() {
        let mut state = GameState::network_view();
        state.inventory.slots.fill(None);
        state.inventory_open = true;
        state.inventory.slots[0] = Some(ItemStack {
            item_type: ItemType::Planks,
            count: 8,
            durability: None,
        });
        state.handle_inventory_click(0);
        state.handle_inventory_click(CRAFT_GRID_UI_OFFSET);
        assert_eq!(state.crafting_grid_slot_stack(0).unwrap().count, 8);
        assert!(state.inventory.slots[0].is_none());
        state.handle_inventory_right_click(CRAFT_GRID_UI_OFFSET);
        assert_eq!(state.crafting_grid_slot_stack(0).unwrap().count, 4);
        assert_eq!(state.inventory.slots[0].as_ref().unwrap().count, 4);
        assert_eq!(state.selected_inventory_slot, Some(CRAFT_GRID_UI_OFFSET));
        state.handle_inventory_click(0);
        assert!(state.crafting_grid_slot_stack(0).is_none());
        assert_eq!(state.inventory.slots[0].as_ref().unwrap().count, 8);
        assert!(state.item_entities.is_empty());
    }

    #[test]
    fn test_inventory_drag_place_spreads_single_items_across_crafting_pattern() {
        let mut state = GameState::new();
        state.inventory = Inventory::new(PLAYER_INVENTORY_CAPACITY);
        state.inventory_enchant_levels = [0; PLAYER_INVENTORY_CAPACITY];
        state.inventory_open = true;
        state.at_crafting_table = true;
        state.at_furnace = false;
        state.at_chest = false;
        state.at_enchanting_table = false;
        state.at_anvil = false;
        state.inventory.slots[0] = Some(ItemStack {
            item_type: ItemType::Planks,
            count: 3,
            durability: None,
        });

        state.handle_inventory_click(0);
        state.handle_inventory_drag_place(CRAFT_GRID_UI_OFFSET);
        state.handle_inventory_drag_place(CRAFT_GRID_UI_OFFSET + 1);
        state.handle_inventory_drag_place(CRAFT_GRID_UI_OFFSET + 2);

        assert_eq!(
            state.crafting_grid_slot_stack(0).map(|stack| stack.count),
            Some(1)
        );
        assert_eq!(
            state.crafting_grid_slot_stack(1).map(|stack| stack.count),
            Some(1)
        );
        assert_eq!(
            state.crafting_grid_slot_stack(2).map(|stack| stack.count),
            Some(1)
        );
        assert!(state.inventory.slots[0].is_none());
        assert_eq!(state.selected_inventory_slot, None);
    }

    #[test]
    fn test_clear_active_crafting_grid_returns_stacked_items_to_inventory() {
        let mut state = GameState::new();
        state.inventory = Inventory::new(PLAYER_INVENTORY_CAPACITY);
        state.inventory_enchant_levels = [0; PLAYER_INVENTORY_CAPACITY];
        state.inventory_open = true;
        state.at_crafting_table = false;
        state.at_furnace = false;
        state.at_chest = false;
        state.at_enchanting_table = false;
        state.at_anvil = false;
        state.inventory.slots[0] = Some(ItemStack {
            item_type: ItemType::Planks,
            count: 8,
            durability: None,
        });

        state.handle_inventory_click(0);
        for _ in 0..2 {
            state.handle_inventory_right_click(CRAFT_GRID_UI_OFFSET);
        }
        for _ in 0..2 {
            state.handle_inventory_right_click(CRAFT_GRID_UI_OFFSET + 3);
        }
        assert_eq!(
            state.crafting_grid_slot_stack(0).map(|stack| stack.count),
            Some(2)
        );
        assert_eq!(
            state.crafting_grid_slot_stack(3).map(|stack| stack.count),
            Some(2)
        );

        state.clear_active_crafting_grid();

        assert!(state.crafting_grid.iter().all(Option::is_none));
        assert_eq!(
            state.inventory.slots[0]
                .as_ref()
                .map(|stack| (stack.item_type, stack.count)),
            Some((ItemType::Planks, 8))
        );
    }

    #[test]
    fn test_three_by_three_grid_crafting_makes_wood_pickaxe() {
        let mut state = GameState::new();
        state.inventory = Inventory::new(PLAYER_INVENTORY_CAPACITY);
        state.inventory_enchant_levels = [0; PLAYER_INVENTORY_CAPACITY];
        state.inventory_open = true;
        state.at_crafting_table = true;
        state.at_furnace = false;
        state.at_chest = false;
        state.at_enchanting_table = false;
        state.at_anvil = false;
        state.inventory.slots[0] = Some(ItemStack {
            item_type: ItemType::Planks,
            count: 3,
            durability: None,
        });
        state.inventory.slots[1] = Some(ItemStack {
            item_type: ItemType::Stick,
            count: 2,
            durability: None,
        });

        state.handle_inventory_click(0);
        for _ in 0..1 {
            state.handle_inventory_right_click(CRAFT_GRID_UI_OFFSET);
        }
        for _ in 0..1 {
            state.handle_inventory_right_click(CRAFT_GRID_UI_OFFSET + 1);
        }
        for _ in 0..1 {
            state.handle_inventory_right_click(CRAFT_GRID_UI_OFFSET + 2);
        }
        state.handle_inventory_click(1);
        for _ in 0..1 {
            state.handle_inventory_right_click(CRAFT_GRID_UI_OFFSET + 4);
        }
        for _ in 0..1 {
            state.handle_inventory_right_click(CRAFT_GRID_UI_OFFSET + 7);
        }

        assert_eq!(
            state.crafting_output_preview(),
            Some((ItemType::WoodPickaxe, 1))
        );
        state.handle_inventory_click(CRAFT_OUTPUT_UI_SLOT);
        assert!(state.inventory.has_item(ItemType::WoodPickaxe, 1));
        assert!(state.crafting_grid.iter().all(Option::is_none));
    }

    #[test]
    fn test_player_progression_save_data_roundtrip_helpers() {
        use std::time::{SystemTime, UNIX_EPOCH};

        let mut state = GameState::new();
        state.player.x = 42.5;
        state.player.y = 17.25;
        state.player.vx = 0.3;
        state.player.vy = -0.2;
        state.player.health = 13.5;
        state.player.hunger = 11.0;
        state.player.sneaking = true;
        state.inventory.slots[0] = Some(ItemStack {
            item_type: ItemType::DiamondSword,
            count: 1,
            durability: Some(1500),
        });
        state.armor_slots[1] = Some(ItemStack {
            item_type: ItemType::IronChestplate,
            count: 1,
            durability: Some(ItemType::IronChestplate.max_durability().unwrap_or(241)),
        });
        state.hotbar_index = 2;
        state.current_dimension = Dimension::Overworld;
        state.time_of_day = 13337.0;
        state.weather = WeatherType::Rain;
        state.weather_timer = 2222;
        state.weather_rain_intensity = 0.7;
        state.weather_wind_intensity = 0.4;
        state.weather_thunder_intensity = 0.1;
        state.dragon_defeated = true;
        state.completion_credits_seen = true;
        state.difficulty = Difficulty::Hard;
        state.game_rules_preset = GameRulesPreset::Builder;
        state.game_rules = GameRulesPreset::Builder.rules();
        state.spawn_point = Some((12, 9));
        state.set_total_experience(321);
        state.remember_nether_portal_link(Dimension::Overworld, (16, 36), (2, 58));

        let data = state.to_progression_save_data();
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock should be monotonic enough")
            .as_nanos();
        let path = format!("saves/test_player_progress_{unique}.bin");

        let _ = crate::platform::fs::remove_file(&path);
        GameState::save_progression_data_to_path(&path, &data).expect("helper save should succeed");
        let loaded =
            GameState::load_progression_data_from_path(&path).expect("helper load should succeed");

        assert_eq!(loaded.version, PLAYER_PROGRESS_VERSION);
        assert_eq!(loaded.portal_links, data.portal_links);
        assert_eq!(loaded.portal_links.len(), 2);
        assert_eq!(loaded.player_x, 42.5);
        assert_eq!(loaded.player_y, 17.25);
        assert_eq!(loaded.player_health, 13.5);
        assert_eq!(loaded.player_hunger, 11.0);
        assert_eq!(
            loaded.inventory.slots[0].as_ref().map(|s| s.item_type),
            Some(ItemType::DiamondSword)
        );
        assert_eq!(
            loaded.armor_slots[1].as_ref().map(|s| s.item_type),
            Some(ItemType::IronChestplate)
        );
        assert_eq!(loaded.hotbar_index, 2);
        assert_eq!(loaded.current_dimension_code, 0);
        assert_eq!(loaded.time_of_day, 13337.0);
        assert_eq!(loaded.weather_code, 1);
        assert!(loaded.dragon_defeated);
        assert!(loaded.completion_credits_seen);
        assert_eq!(loaded.difficulty_code, 3);
        assert_eq!(loaded.game_rules_preset_code, 2);
        assert!(loaded.has_spawn_point);
        assert_eq!(loaded.spawn_point_x, 12);
        assert_eq!(loaded.spawn_point_y, 9);
        assert!(!loaded.rule_do_mob_spawning);
        assert!(!loaded.rule_do_daylight_cycle);
        assert!(!loaded.rule_do_weather_cycle);
        assert!(loaded.rule_keep_inventory);
        assert_eq!(loaded.experience_total, 321);
        assert_eq!(loaded.experience_level, state.player.experience_level);

        let _ = crate::platform::fs::remove_file(path);
    }

    #[test]
    fn test_player_progression_save_does_not_leave_temporary_file() {
        use std::time::{SystemTime, UNIX_EPOCH};

        let state = GameState::new();
        let data = state.to_progression_save_data();
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock should be monotonic enough")
            .as_nanos();
        let path = format!("saves/test_player_progress_tmp_{unique}.bin");
        let temp_path = GameState::progression_temp_path(&path);

        let _ = crate::platform::fs::remove_file(&path);
        let _ = crate::platform::fs::remove_file(&temp_path);
        GameState::save_progression_data_to_path(&path, &data).expect("helper save should succeed");

        assert!(std::path::Path::new(&path).exists());
        assert!(!temp_path.exists());

        let _ = crate::platform::fs::remove_file(path);
    }

    #[test]
    fn test_player_progression_loader_upgrades_v7_without_discarding_the_session() {
        let state = GameState::new();
        let mut progress = state.to_progression_save_data();
        progress.version = PLAYER_PROGRESS_VERSION_V7;
        let path = state.world.save_root().join("legacy-v7-checkpoint.bin");
        let payload = bincode::serialize(&PlayerCheckpointV7 {
            version: PLAYER_PROGRESS_VERSION_V7,
            progress,
            session: PlayerSession::capture(&state).into(),
        })
        .unwrap();
        GameState::write_progression_payload_atomically(&path, &payload).unwrap();
        let upgraded = GameState::load_progression_data_from_path(&path).unwrap();
        assert_eq!(upgraded.version, PLAYER_PROGRESS_VERSION);
        assert_eq!(upgraded.complete_session.unwrap().player.x, state.player.x);
        assert!(upgraded.portal_links.is_empty());
    }

    #[test]
    fn test_player_progression_loader_upgrades_v1_data() {
        use std::time::{SystemTime, UNIX_EPOCH};

        let legacy = PlayerProgressSaveDataV1 {
            version: PLAYER_PROGRESS_VERSION_V1,
            player_x: 4.5,
            player_y: 12.0,
            player_vx: 0.0,
            player_vy: 0.0,
            player_grounded: false,
            player_facing_right: true,
            player_sneaking: false,
            player_health: 17.0,
            player_hunger: 19.0,
            player_drowning_timer: 300,
            player_burning_timer: 0,
            player_fall_distance: 0.0,
            inventory: Inventory::new(PLAYER_INVENTORY_CAPACITY),
            hotbar_index: 0,
            current_dimension_code: 0,
            time_of_day: 8000.0,
            weather_code: 0,
            weather_timer: 1200,
            weather_rain_intensity: 0.0,
            weather_wind_intensity: 0.1,
            weather_thunder_intensity: 0.0,
            thunder_flash_timer: 0,
            dragon_defeated: false,
            completion_credits_seen: false,
            movement_profile_code: 1,
            portal_cooldown: 0,
        };

        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock should be monotonic enough")
            .as_nanos();
        let path = format!("saves/test_player_progress_v1_{unique}.bin");
        let _ = crate::platform::fs::remove_file(&path);
        let encoded = bincode::serialize(&legacy).expect("legacy v1 serialization should succeed");
        crate::platform::fs::write(&path, encoded).expect("legacy v1 fixture should write");

        let loaded =
            GameState::load_progression_data_from_path(&path).expect("v1 payload should upgrade");
        assert_eq!(loaded.version, PLAYER_PROGRESS_VERSION);
        assert_eq!(loaded.player_x, 4.5);
        assert_eq!(loaded.difficulty_code, 2);
        assert_eq!(loaded.game_rules_preset_code, 0);
        assert!(loaded.rule_do_mob_spawning);
        assert!(loaded.rule_do_daylight_cycle);
        assert!(loaded.rule_do_weather_cycle);
        assert!(!loaded.rule_keep_inventory);

        let _ = crate::platform::fs::remove_file(path);
    }

    #[test]
    fn test_player_progression_loader_upgrades_v2_data() {
        use std::time::{SystemTime, UNIX_EPOCH};

        let legacy = PlayerProgressSaveDataV2 {
            version: PLAYER_PROGRESS_VERSION_V2,
            player_x: 7.5,
            player_y: 13.0,
            player_vx: 0.0,
            player_vy: 0.0,
            player_grounded: false,
            player_facing_right: true,
            player_sneaking: false,
            player_health: 16.0,
            player_hunger: 18.0,
            player_drowning_timer: 300,
            player_burning_timer: 0,
            player_fall_distance: 0.0,
            inventory: Inventory::new(PLAYER_INVENTORY_CAPACITY),
            hotbar_index: 0,
            current_dimension_code: 0,
            time_of_day: 9000.0,
            weather_code: 0,
            weather_timer: 1300,
            weather_rain_intensity: 0.0,
            weather_wind_intensity: 0.1,
            weather_thunder_intensity: 0.0,
            thunder_flash_timer: 0,
            dragon_defeated: false,
            completion_credits_seen: false,
            movement_profile_code: 1,
            portal_cooldown: 0,
            difficulty_code: 2,
            game_rules_preset_code: 2, // Builder
        };

        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock should be monotonic enough")
            .as_nanos();
        let path = format!("saves/test_player_progress_v2_{unique}.bin");
        let _ = crate::platform::fs::remove_file(&path);
        let encoded = bincode::serialize(&legacy).expect("legacy v2 serialization should succeed");
        crate::platform::fs::write(&path, encoded).expect("legacy v2 fixture should write");

        let loaded =
            GameState::load_progression_data_from_path(&path).expect("v2 payload should upgrade");
        assert_eq!(loaded.version, PLAYER_PROGRESS_VERSION);
        assert_eq!(loaded.player_x, 7.5);
        assert_eq!(loaded.game_rules_preset_code, 2);
        assert!(!loaded.rule_do_mob_spawning);
        assert!(!loaded.rule_do_daylight_cycle);
        assert!(!loaded.rule_do_weather_cycle);
        assert!(loaded.rule_keep_inventory);

        let _ = crate::platform::fs::remove_file(path);
    }

    #[test]
    fn test_player_progression_loader_upgrades_v3_data() {
        use std::time::{SystemTime, UNIX_EPOCH};

        let legacy = PlayerProgressSaveDataV3 {
            version: PLAYER_PROGRESS_VERSION_V3,
            player_x: 9.5,
            player_y: 15.0,
            player_vx: 0.0,
            player_vy: 0.0,
            player_grounded: false,
            player_facing_right: true,
            player_sneaking: false,
            player_health: 16.0,
            player_hunger: 18.0,
            player_drowning_timer: 300,
            player_burning_timer: 0,
            player_fall_distance: 0.0,
            inventory: Inventory::new(PLAYER_INVENTORY_CAPACITY),
            hotbar_index: 0,
            current_dimension_code: 0,
            time_of_day: 9000.0,
            weather_code: 0,
            weather_timer: 1300,
            weather_rain_intensity: 0.0,
            weather_wind_intensity: 0.1,
            weather_thunder_intensity: 0.0,
            thunder_flash_timer: 0,
            dragon_defeated: false,
            completion_credits_seen: false,
            movement_profile_code: 1,
            portal_cooldown: 0,
            difficulty_code: 2,
            game_rules_preset_code: 0,
            rule_do_mob_spawning: true,
            rule_do_daylight_cycle: true,
            rule_do_weather_cycle: true,
            rule_keep_inventory: false,
        };

        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock should be monotonic enough")
            .as_nanos();
        let path = format!("saves/test_player_progress_v3_{unique}.bin");
        let _ = crate::platform::fs::remove_file(&path);
        let encoded = bincode::serialize(&legacy).expect("legacy v3 serialization should succeed");
        crate::platform::fs::write(&path, encoded).expect("legacy v3 fixture should write");

        let loaded =
            GameState::load_progression_data_from_path(&path).expect("v3 payload should upgrade");
        assert_eq!(loaded.version, PLAYER_PROGRESS_VERSION);
        assert_eq!(loaded.player_x, 9.5);
        assert!(!loaded.has_spawn_point);
        assert_eq!(loaded.spawn_point_x, 0);
        assert_eq!(loaded.spawn_point_y, 0);

        let _ = crate::platform::fs::remove_file(path);
    }

    #[test]
    fn test_player_progression_loader_upgrades_v4_data() {
        use std::time::{SystemTime, UNIX_EPOCH};

        let legacy = PlayerProgressSaveDataV4 {
            version: PLAYER_PROGRESS_VERSION_V4,
            player_x: 10.5,
            player_y: 18.0,
            player_vx: 0.0,
            player_vy: 0.0,
            player_grounded: false,
            player_facing_right: true,
            player_sneaking: false,
            player_health: 16.0,
            player_hunger: 18.0,
            player_drowning_timer: 300,
            player_burning_timer: 0,
            player_fall_distance: 0.0,
            inventory: Inventory::new(PLAYER_INVENTORY_CAPACITY),
            hotbar_index: 0,
            spawn_point_x: 0,
            spawn_point_y: 0,
            has_spawn_point: false,
            current_dimension_code: 0,
            time_of_day: 9000.0,
            weather_code: 0,
            weather_timer: 1300,
            weather_rain_intensity: 0.0,
            weather_wind_intensity: 0.1,
            weather_thunder_intensity: 0.0,
            thunder_flash_timer: 0,
            dragon_defeated: false,
            completion_credits_seen: false,
            movement_profile_code: 1,
            portal_cooldown: 0,
            difficulty_code: 2,
            game_rules_preset_code: 0,
            rule_do_mob_spawning: true,
            rule_do_daylight_cycle: true,
            rule_do_weather_cycle: true,
            rule_keep_inventory: false,
        };

        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock should be monotonic enough")
            .as_nanos();
        let path = format!("saves/test_player_progress_v4_{unique}.bin");
        let _ = crate::platform::fs::remove_file(&path);
        let encoded = bincode::serialize(&legacy).expect("legacy v4 serialization should succeed");
        crate::platform::fs::write(&path, encoded).expect("legacy v4 fixture should write");

        let loaded =
            GameState::load_progression_data_from_path(&path).expect("v4 payload should upgrade");
        assert_eq!(loaded.version, PLAYER_PROGRESS_VERSION);
        assert_eq!(loaded.player_x, 10.5);
        assert!(loaded.armor_slots.iter().all(Option::is_none));
        assert_eq!(loaded.experience_total, 0);

        let _ = crate::platform::fs::remove_file(path);
    }

    #[test]
    fn test_player_progression_loader_upgrades_v5_data() {
        use std::time::{SystemTime, UNIX_EPOCH};

        let legacy = PlayerProgressSaveDataV5 {
            version: PLAYER_PROGRESS_VERSION_V5,
            player_x: 11.5,
            player_y: 19.0,
            player_vx: 0.0,
            player_vy: 0.0,
            player_grounded: false,
            player_facing_right: true,
            player_sneaking: false,
            player_health: 15.0,
            player_hunger: 17.0,
            player_drowning_timer: 300,
            player_burning_timer: 0,
            player_fall_distance: 0.0,
            inventory: Inventory::new(PLAYER_INVENTORY_CAPACITY),
            armor_slots: [None, None, None, None],
            hotbar_index: 0,
            spawn_point_x: 0,
            spawn_point_y: 0,
            has_spawn_point: false,
            current_dimension_code: 0,
            time_of_day: 9000.0,
            weather_code: 0,
            weather_timer: 1300,
            weather_rain_intensity: 0.0,
            weather_wind_intensity: 0.1,
            weather_thunder_intensity: 0.0,
            thunder_flash_timer: 0,
            dragon_defeated: false,
            completion_credits_seen: false,
            movement_profile_code: 1,
            portal_cooldown: 0,
            difficulty_code: 2,
            game_rules_preset_code: 0,
            rule_do_mob_spawning: true,
            rule_do_daylight_cycle: true,
            rule_do_weather_cycle: true,
            rule_keep_inventory: false,
        };

        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock should be monotonic enough")
            .as_nanos();
        let path = format!("saves/test_player_progress_v5_{unique}.bin");
        let _ = crate::platform::fs::remove_file(&path);
        let encoded = bincode::serialize(&legacy).expect("legacy v5 serialization should succeed");
        crate::platform::fs::write(&path, encoded).expect("legacy v5 fixture should write");

        let loaded =
            GameState::load_progression_data_from_path(&path).expect("v5 payload should upgrade");
        assert_eq!(loaded.version, PLAYER_PROGRESS_VERSION);
        assert_eq!(loaded.player_x, 11.5);
        assert_eq!(loaded.experience_total, 0);

        let _ = crate::platform::fs::remove_file(path);
    }

    #[test]
    fn test_manual_armor_equip_reduces_damage_and_wears_durability() {
        let mut state = GameState::new();
        state.inventory_open = true;
        state.armor_slots = [None, None, None, None];
        state.inventory.slots[0] = Some(ItemStack {
            item_type: ItemType::IronChestplate,
            count: 1,
            durability: ItemType::IronChestplate.max_durability(),
        });

        state.handle_inventory_click(0);
        state.handle_inventory_click(ARMOR_UI_OFFSET + 1);
        assert_eq!(state.total_armor_points(), 6);
        assert!(state.inventory.slots[0].is_none());
        assert_eq!(
            state.armor_slots[1].as_ref().map(|s| s.item_type),
            Some(ItemType::IronChestplate)
        );

        let hp_before = state.player.health;
        let durability_before = state.armor_slots[1]
            .as_ref()
            .and_then(|s| s.durability)
            .expect("equipped chestplate should have durability");
        state.apply_player_damage(10.0);
        let hp_loss = hp_before - state.player.health;
        assert!(hp_loss < 10.0);
        let durability_after = state.armor_slots[1]
            .as_ref()
            .and_then(|s| s.durability)
            .expect("equipped chestplate should still have durability");
        assert_eq!(durability_after, durability_before.saturating_sub(1));
    }

    #[test]
    fn test_update_does_not_auto_equip_armor_from_inventory() {
        let mut state = GameState::new();
        state.armor_slots = [None, None, None, None];
        state.inventory.slots[0] = Some(ItemStack {
            item_type: ItemType::IronChestplate,
            count: 1,
            durability: ItemType::IronChestplate.max_durability(),
        });

        state.update(0, 0);

        assert_eq!(
            state.inventory.slots[0]
                .as_ref()
                .map(|stack| stack.item_type),
            Some(ItemType::IronChestplate)
        );
        assert!(state.armor_slots.iter().all(Option::is_none));
    }

    #[test]
    fn test_armor_slot_rejects_wrong_item_type() {
        let mut state = GameState::new();
        state.inventory_open = true;
        state.inventory.slots[0] = Some(ItemStack {
            item_type: ItemType::Stick,
            count: 1,
            durability: None,
        });

        state.handle_inventory_click(0);
        state.handle_inventory_click(ARMOR_UI_OFFSET + 1);

        assert_eq!(
            state.inventory.slots[0]
                .as_ref()
                .map(|stack| stack.item_type),
            Some(ItemType::Stick)
        );
        assert!(state.armor_slots[1].is_none());
    }

    #[test]
    fn test_player_progression_loader_rejects_unknown_version() {
        use std::time::{SystemTime, UNIX_EPOCH};

        let data = PlayerProgressSaveData {
            complete_session: None,
            runtime: None,
            portal_links: Vec::new(),
            version: PLAYER_PROGRESS_VERSION + 1,
            player_x: 0.5,
            player_y: 9.9,
            player_vx: 0.0,
            player_vy: 0.0,
            player_grounded: false,
            player_facing_right: true,
            player_sneaking: false,
            player_health: 20.0,
            player_hunger: 20.0,
            player_drowning_timer: 300,
            player_burning_timer: 0,
            player_fall_distance: 0.0,
            inventory: Inventory::new(PLAYER_INVENTORY_CAPACITY),
            armor_slots: [None, None, None, None],
            hotbar_index: 0,
            spawn_point_x: 0,
            spawn_point_y: 0,
            has_spawn_point: false,
            current_dimension_code: 0,
            time_of_day: 8000.0,
            weather_code: 0,
            weather_timer: 1000,
            weather_rain_intensity: 0.0,
            weather_wind_intensity: 0.1,
            weather_thunder_intensity: 0.0,
            thunder_flash_timer: 0,
            dragon_defeated: false,
            completion_credits_seen: false,
            movement_profile_code: 1,
            portal_cooldown: 0,
            experience_level: 0,
            experience_progress: 0.0,
            experience_total: 0,
            difficulty_code: 2,
            game_rules_preset_code: 0,
            rule_do_mob_spawning: true,
            rule_do_daylight_cycle: true,
            rule_do_weather_cycle: true,
            rule_keep_inventory: false,
        };
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock should be monotonic enough")
            .as_nanos();
        let path = format!("saves/test_player_progress_badver_{unique}.bin");
        let _ = crate::platform::fs::remove_file(&path);
        GameState::save_progression_data_to_path(&path, &data).expect("helper save should succeed");
        assert!(GameState::load_progression_data_from_path(&path).is_none());
        let _ = crate::platform::fs::remove_file(path);
    }
}

#[cfg(test)]
#[path = "physics_tests.rs"]
mod physics_tests;

#[cfg(test)]
#[path = "playtest_regressions.rs"]
mod playtest_regressions;

#[cfg(test)]
#[path = "persistence_tests.rs"]
mod persistence_tests;
