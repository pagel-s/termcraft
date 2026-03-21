pub mod block;
pub mod chunk;
pub mod item;

use block::BlockType;
use chunk::{CHUNK_HEIGHT, CHUNK_WIDTH, Chunk};
use item::{Inventory, ItemType};
use noise::{NoiseFn, Perlin};
use rand::Rng;
use serde::{Deserialize, Serialize};
use std::collections::{BinaryHeap, HashMap, HashSet};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::time::Instant;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Dimension {
    Overworld,
    Nether,
    End,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum BiomeType {
    Desert,
    Tundra,
    Taiga,
    Forest,
    Plains,
    Swamp,
    Jungle,
    ExtremeHills,
    Ocean,
    River,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct ChunkLoadMetrics {
    pub sync_loaded: u64,
    pub sync_generated: u64,
    pub async_loaded: u64,
    pub async_generated: u64,
    pub pending_requests: u64,
    pub last_load_us: u64,
    pub max_load_us: u64,
}

const ACTIVE_CHUNK_RADIUS: i32 = 1;
const PREFETCH_CHUNK_RADIUS: i32 = 3;
const CHUNK_REQUEST_RETENTION_RADIUS: i32 = PREFETCH_CHUNK_RADIUS + 2;
const CHUNK_MOTION_LOAD_THRESHOLD_RATIO: f64 = 0.65;
const SHORELINE_BLEND_RADIUS: i32 = 3;
const SHORELINE_SHELF_RADIUS: i32 = 4;
const OVERWORLD_BIOME_CLIMATE_MACRO_SCALE: f64 = 0.0042;
const OVERWORLD_BIOME_CLIMATE_DETAIL_SCALE: f64 = 0.0105;
const OVERWORLD_BIOME_CONTINENTAL_SCALE: f64 = 0.0021;
const OVERWORLD_BIOME_CONTINENTAL_DETAIL_SCALE: f64 = 0.0056;
const OVERWORLD_BIOME_RIVER_SCALE: f64 = 0.0135;
const OVERWORLD_BIOME_RIVER_DETAIL_SCALE: f64 = 0.0175;
const OVERWORLD_VILLAGE_HUT_MAX_SURFACE_DELTA: i32 = 2;
const CHUNK_PIPELINE_WORKERS: usize = 3;
const CHUNK_PIPELINE_MAX_PENDING: usize = 24;
const CHUNK_PIPELINE_MAX_IN_FLIGHT: usize = 12;
const REDSTONE_MAX_POWER: u8 = 15;
const REDSTONE_TICK_INTERVAL: u64 = 2;
const OVERWORLD_VISIBLE_ORE_MIN_Y: i32 = 40;
const OVERWORLD_IRON_ORE_MIN_Y: i32 = 38;
const OVERWORLD_IRON_ORE_RICH_Y: i32 = 118;
const REDSTONE_ORE_MIN_Y: i32 = 68;
const REDSTONE_ORE_RICH_Y: i32 = 118;
const OVERWORLD_SEA_LEVEL: i32 = 36;
// In a 2D slice, cave openings read much larger than they do in 3D, so keep
// a thicker surface cap and lower the near-surface carve rate.
const OVERWORLD_CAVE_MIN_SURFACE_DEPTH: i32 = 5;
const OVERWORLD_RAVINE_MIN_SURFACE_DEPTH: i32 = 10;
const OVERWORLD_NEAR_SURFACE_CAVE_BAND: i32 = 10;
const OVERWORLD_CHAMBER_MIN_SURFACE_DEPTH: i32 = 14;
const OVERWORLD_CHAMBER_CELL_WIDTH: i32 = 22;
const OVERWORLD_CHAMBER_CELL_HEIGHT: i32 = 18;
const OVERWORLD_CHAMBER_MIN_RADIUS_X: i32 = 4;
const OVERWORLD_CHAMBER_RADIUS_X_SPAN: i32 = 4;
const OVERWORLD_CHAMBER_MIN_RADIUS_Y: i32 = 3;
const OVERWORLD_CHAMBER_RADIUS_Y_SPAN: i32 = 3;
const OVERWORLD_CHAMBER_EDGE_ROUGHNESS: f64 = 0.18;
const OVERWORLD_RAVINE_CELL_WIDTH: i32 = 54;
const OVERWORLD_RAVINE_MIN_HALF_WIDTH: f64 = 1.8;
const OVERWORLD_RAVINE_HALF_WIDTH_SPAN: f64 = 2.9;
const OVERWORLD_RAVINE_MEANDER_AMPLITUDE: f64 = 2.8;
const OVERWORLD_RAVINE_LEDGE_EDGE_START: f64 = 0.56;
const OVERWORLD_RAVINE_LEDGE_NOISE_THRESHOLD: f64 = 0.24;
const OVERWORLD_RAVINE_RIB_NOISE_THRESHOLD: f64 = 0.80;
const OVERWORLD_GRAVEL_MIN_SURFACE_DEPTH: i32 = 9;
const OVERWORLD_CAVE_FLOOR_VARIATION_MIN_DEPTH: i32 = 12;
const OVERWORLD_CAVE_POOL_MIN_DEPTH: i32 = 20;
const OVERWORLD_CAVE_POOL_LAVA_DEPTH: i32 = 56;
const OVERWORLD_CAVE_BRIDGE_MAX_WALL_WIDTH: usize = 3;
const OVERWORLD_CAVE_STAIR_MIN_DROP: usize = 3;
const OVERWORLD_CAVE_STAIR_MAX_DROP: usize = 6;
const OVERWORLD_CAVE_CONNECTOR_MIN_OPEN_SCORE: usize = 3;
const OVERWORLD_CAVE_DEAD_END_TRIM_PASSES: usize = 2;
const OVERWORLD_CAVE_DEAD_END_MAX_LOCAL_AIR_SCORE: usize = 6;
const OVERWORLD_WORM_CAVE_THRESHOLD: f64 = 0.12;
const OVERWORLD_NEAR_SURFACE_WORM_CAVE_THRESHOLD: f64 = 0.08;
const OVERWORLD_DUNGEON_CHUNK_CADENCE: i32 = 5;
const OVERWORLD_DUNGEON_CHUNK_PHASE: i32 = 2;
const OVERWORLD_LAVA_TICK_INTERVAL: u64 = 30;
const WATER_MIN_HORIZONTAL_FLOW_LEVEL: u8 = 1;
const SLOW_LAVA_MIN_HORIZONTAL_FLOW_LEVEL: u8 = 5;
const TNT_FUSE_TICKS: u8 = 35;
const TNT_CHAIN_FUSE_TICKS: u8 = 5;
const TNT_BLAST_RADIUS: i32 = 3;
const TNT_BLAST_STRENGTH: f32 = 4.0;
const PISTON_PUSH_LIMIT: usize = 12;
const REDSTONE_REPEATER_MIN_DELAY: u8 = 1;
const REDSTONE_REPEATER_MAX_DELAY: u8 = 4;
pub const STRONGHOLD_CENTER_X: i32 = 640;
pub const STRONGHOLD_ROOM_TOP_Y: i32 = 66;
pub const STRONGHOLD_ROOM_BOTTOM_Y: i32 = 92;
pub const STRONGHOLD_PORTAL_INNER_X: i32 = STRONGHOLD_CENTER_X;
pub const STRONGHOLD_PORTAL_INNER_Y: i32 = 86;
pub const END_TOWER_XS: [i32; 4] = [-28, -14, 14, 28];

pub(crate) fn tall_grass_drops_seed_at(world_x: i32, world_y: i32) -> bool {
    ((world_x.wrapping_mul(29) ^ world_y.wrapping_mul(43)).rem_euclid(25)) < 7
}

pub(crate) fn gravel_drops_flint_at(world_x: i32, world_y: i32) -> bool {
    ((world_x.wrapping_mul(17) ^ world_y.wrapping_mul(37)).rem_euclid(10)) == 0
}

pub(crate) fn nether_wart_drop_count_at(world_x: i32, world_y: i32, mature: bool) -> u32 {
    if mature {
        2 + ((world_x.wrapping_mul(13) ^ world_y.wrapping_mul(19)).rem_euclid(3) as u32)
    } else {
        1
    }
}

fn sapling_growth_due_at(world_x: i32, world_y: i32, growth_phase: i32) -> bool {
    ((world_x.wrapping_mul(31) ^ world_y.wrapping_mul(47) ^ growth_phase.wrapping_mul(19)) & 7) == 0
}

fn sugar_cane_growth_due_at(world_x: i32, world_y: i32, growth_phase: i32) -> bool {
    ((world_x.wrapping_mul(43) ^ world_y.wrapping_mul(29) ^ growth_phase.wrapping_mul(11)) & 3) == 0
}

fn nether_wart_growth_due_at(world_x: i32, world_y: i32, growth_phase: i32) -> bool {
    ((world_x.wrapping_mul(37) ^ world_y.wrapping_mul(53) ^ growth_phase.wrapping_mul(17)) & 5) == 0
}

fn dry_farmland_revert_due_at(world_x: i32, world_y: i32, growth_phase: i32) -> bool {
    ((world_x.wrapping_mul(41) ^ world_y.wrapping_mul(61) ^ growth_phase.wrapping_mul(13)) & 7) == 0
}

fn bone_meal_flora_block_at(world_x: i32, world_y: i32) -> BlockType {
    match (world_x.wrapping_mul(23) ^ world_y.wrapping_mul(41)).rem_euclid(12) {
        0 | 1 => BlockType::RedFlower,
        2 | 3 => BlockType::YellowFlower,
        _ => BlockType::TallGrass,
    }
}
const NETHER_FORTRESS_REGION_CHUNKS: i32 = 24;
const NETHER_FORTRESS_CENTER_MIN_OFFSET_CHUNKS: i32 = 6;
const NETHER_FORTRESS_CENTER_OFFSET_SPAN_CHUNKS: i32 = 20;
const NETHER_FORTRESS_BASE_Y_MIN: i32 = 40;
const NETHER_FORTRESS_BASE_Y_SPAN: i32 = 20;
const NETHER_FORTRESS_HALF_SPAN_BLOCKS: i32 = 96;
const NETHER_FORTRESS_CHEST_OFFSET_BLOCKS: i32 = 54;
const NETHER_FORTRESS_ROOF_HEIGHT: i32 = 5;
const NETHER_FORTRESS_GATE_OFFSET_BLOCKS: i32 = 72;
const NETHER_FORTRESS_BASTION_OFFSET_BLOCKS: i32 = 38;
const NETHER_FORTRESS_WATCHTOWER_INSET_BLOCKS: i32 = 18;
const NETHER_CAVERN_CEILING_BASE_Y: i32 = 18;
const NETHER_CAVERN_CEILING_VARIATION: f64 = 2.4;
const NETHER_CAVERN_CEILING_DETAIL_VARIATION: f64 = 0.6;
const NETHER_CEILING_VAULT_CELL_WIDTH: i32 = 24;
const NETHER_CEILING_VAULT_MIN_RADIUS: i32 = 7;
const NETHER_CEILING_VAULT_RADIUS_SPAN: i32 = 6;
const NETHER_CEILING_VAULT_MAX_LIFT: i32 = 12;
const NETHER_CEILING_INTRUSION_CELL_WIDTH: i32 = 19;
const NETHER_CEILING_INTRUSION_MIN_RADIUS: i32 = 3;
const NETHER_CEILING_INTRUSION_RADIUS_SPAN: i32 = 4;
const NETHER_CEILING_INTRUSION_MAX_DROP: i32 = 7;
const NETHER_CAVERN_FLOOR_BASE_Y: i32 = 60;
const NETHER_CAVERN_FLOOR_VARIATION: f64 = 3.0;
const NETHER_CAVERN_FLOOR_DETAIL_VARIATION: f64 = 0.9;
const NETHER_CAVERN_MIN_OPEN_HEIGHT: i32 = 32;
const NETHER_CHAMBER_CELL_WIDTH: i32 = 42;
const NETHER_CHAMBER_MIN_RADIUS: i32 = 10;
const NETHER_CHAMBER_RADIUS_SPAN: i32 = 8;
const NETHER_CHAMBER_MAX_CEILING_LIFT: i32 = 9;
const NETHER_CHAMBER_MAX_FLOOR_DROP: i32 = 8;
const NETHER_LAVA_SEA_BASE_Y: i32 = 108;
const NETHER_LAVA_SEA_VARIATION: f64 = 1.4;
const NETHER_HANGING_FORMATION_MIN_CLEARANCE: i32 = 12;
const NETHER_SHELF_CELL_WIDTH: i32 = 30;
const NETHER_SHELF_MIN_CLEARANCE_ABOVE: i32 = 8;
const NETHER_SHELF_MIN_CLEARANCE_BELOW: i32 = 6;
const NETHER_LANDMARK_CELL_WIDTH: i32 = 44;
const NETHER_LANDMARK_MIN_CLEARANCE: i32 = 20;
type PistonPush = (i32, i32, i32, i32, BlockType);
type ExplosionEvent = (i32, i32, i32);
pub type ExplosionBlockLoss = (i32, i32, BlockType, bool);
pub type EnvironmentDrop = (i32, i32, ItemType);

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
struct WorldSeedData {
    terrain_seed: u32,
    temp_seed: u32,
    moist_seed: u32,
    cave_seed: u32,
}

#[derive(Clone, Copy, Debug)]
struct ChunkWorkItem {
    chunk_x: i32,
}

#[derive(Clone, Copy, Debug)]
struct NetherFortressLayout {
    center_x: i32,
    left_x: i32,
    right_x: i32,
    base_y: i32,
    roof_y: i32,
    chest_left_x: i32,
    chest_right_x: i32,
    seed: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum NetherRouteLandmarkVariant {
    Shrine,
    Bridge,
    Reliquary,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ChunkRequest {
    chunk_x: i32,
    distance: i32,
    seq: u64,
}

impl Ord for ChunkRequest {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        other
            .distance
            .cmp(&self.distance)
            .then_with(|| other.seq.cmp(&self.seq))
    }
}

impl PartialOrd for ChunkRequest {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

pub struct World {
    chunks: HashMap<i32, Chunk>,
    pub newly_generated_chunks: Vec<i32>,
    pub dimension: Dimension,
    save_key: String,
    perlin: Perlin,
    temp_perlin: Perlin,
    moist_perlin: Perlin,
    cave_perlin: Perlin,
    tick_counter: u64,
    pending_chunks: HashSet<i32>,
    queued_chunks: HashSet<i32>,
    in_flight_chunks: HashSet<i32>,
    chunk_request_queue: BinaryHeap<ChunkRequest>,
    chunk_request_seq: u64,
    chunk_request_tx: Sender<ChunkWorkItem>,
    chunk_result_rx: Receiver<(i32, Chunk, bool)>,
    pub chunk_metrics: ChunkLoadMetrics,
    pub recent_explosions: Vec<ExplosionEvent>,
    pub recent_explosion_block_losses: Vec<ExplosionBlockLoss>,
    pub recent_environment_drops: Vec<EnvironmentDrop>,
    active_fluid_chunks: HashSet<i32>,
}

impl Default for World {
    fn default() -> Self {
        Self::new()
    }
}

impl World {
    pub fn new() -> Self {
        Self::new_for_dimension(Dimension::Overworld)
    }

    pub fn new_for_dimension(dimension: Dimension) -> Self {
        let save_key = match dimension {
            Dimension::Overworld => "overworld".to_string(),
            Dimension::Nether => "nether".to_string(),
            Dimension::End => "end".to_string(),
        };
        let seeds = Self::load_or_create_world_seed_data(&save_key);
        let terrain_seed = seeds.terrain_seed;
        let temp_seed = seeds.temp_seed;
        let moist_seed = seeds.moist_seed;
        let cave_seed = seeds.cave_seed;
        let worker_save_key = save_key.clone();

        let (chunk_request_tx, chunk_request_rx) = mpsc::channel::<ChunkWorkItem>();
        let (chunk_result_tx, chunk_result_rx) = mpsc::channel::<(i32, Chunk, bool)>();
        let shared_request_rx = Arc::new(Mutex::new(chunk_request_rx));
        for _ in 0..CHUNK_PIPELINE_WORKERS {
            let request_rx = Arc::clone(&shared_request_rx);
            let result_tx = chunk_result_tx.clone();
            let worker_save_key = worker_save_key.clone();
            std::thread::spawn(move || {
                let perlin = Perlin::new(terrain_seed);
                let temp_perlin = Perlin::new(temp_seed);
                let moist_perlin = Perlin::new(moist_seed);
                let cave_perlin = Perlin::new(cave_seed);

                loop {
                    let work_item = {
                        let Ok(guard) = request_rx.lock() else {
                            return;
                        };
                        guard.recv()
                    };
                    let Ok(work_item) = work_item else {
                        return;
                    };
                    let chunk_x = work_item.chunk_x;
                    if let Some(loaded_chunk) = Chunk::load_from_disk(chunk_x, &worker_save_key) {
                        if result_tx.send((chunk_x, loaded_chunk, false)).is_err() {
                            return;
                        }
                    } else {
                        let generated_chunk = Self::build_chunk_with_noise(
                            chunk_x,
                            dimension,
                            &worker_save_key,
                            &perlin,
                            &temp_perlin,
                            &moist_perlin,
                            &cave_perlin,
                        );
                        if result_tx.send((chunk_x, generated_chunk, true)).is_err() {
                            return;
                        }
                    }
                }
            });
        }

        Self {
            chunks: HashMap::new(),
            newly_generated_chunks: Vec::new(),
            dimension,
            save_key,
            perlin: Perlin::new(terrain_seed),
            temp_perlin: Perlin::new(temp_seed),
            moist_perlin: Perlin::new(moist_seed),
            cave_perlin: Perlin::new(cave_seed),
            tick_counter: 0,
            pending_chunks: HashSet::new(),
            queued_chunks: HashSet::new(),
            in_flight_chunks: HashSet::new(),
            chunk_request_queue: BinaryHeap::new(),
            chunk_request_seq: 0,
            chunk_request_tx,
            chunk_result_rx,
            chunk_metrics: ChunkLoadMetrics::default(),
            recent_explosions: Vec::new(),
            recent_explosion_block_losses: Vec::new(),
            recent_environment_drops: Vec::new(),
            active_fluid_chunks: HashSet::new(),
        }
    }

    fn world_seed_path(save_key: &str) -> String {
        format!("saves/{}_world_seed.bin", save_key)
    }

    fn load_world_seed_data(save_key: &str) -> Option<WorldSeedData> {
        let path = Self::world_seed_path(save_key);
        let encoded = std::fs::read(path).ok()?;
        bincode::deserialize::<WorldSeedData>(&encoded).ok()
    }

    fn create_world_seed_data() -> WorldSeedData {
        let mut rng = rand::thread_rng();
        WorldSeedData {
            terrain_seed: rng.gen_range(0..1_000_000),
            temp_seed: rng.gen_range(0..1_000_000),
            moist_seed: rng.gen_range(0..1_000_000),
            cave_seed: rng.gen_range(0..1_000_000),
        }
    }

    fn load_or_create_world_seed_data(save_key: &str) -> WorldSeedData {
        if let Some(seed_data) = Self::load_world_seed_data(save_key) {
            return seed_data;
        }

        let seed_data = Self::create_world_seed_data();
        let _ = std::fs::create_dir_all("saves");
        if let Ok(encoded) = bincode::serialize(&seed_data) {
            let _ = std::fs::write(Self::world_seed_path(save_key), encoded);
        }
        seed_data
    }

    pub fn get_block(&self, world_x: i32, world_y: i32) -> BlockType {
        if world_y < 0 || world_y >= CHUNK_HEIGHT as i32 {
            return BlockType::Air;
        }
        let chunk_x = world_x.div_euclid(CHUNK_WIDTH as i32);
        let local_x = world_x.rem_euclid(CHUNK_WIDTH as i32) as usize;
        let local_y = world_y as usize;
        if let Some(chunk) = self.chunks.get(&chunk_x) {
            chunk.get_block(local_x, local_y)
        } else {
            BlockType::Air
        }
    }

    pub fn chunk_column_snapshot(&self, chunk_x: i32) -> Option<(u64, Vec<BlockType>)> {
        self.chunks
            .get(&chunk_x)
            .map(|chunk| (chunk.blocks_revision(), chunk.blocks.clone()))
    }

    pub fn apply_chunk_column_snapshot(&mut self, chunk_x: i32, blocks: &[BlockType]) -> bool {
        if blocks.len() != CHUNK_WIDTH * CHUNK_HEIGHT {
            return false;
        }
        let chunk_center_x = chunk_x * CHUNK_WIDTH as i32 + (CHUNK_WIDTH as i32 / 2);
        self.load_chunks_around(chunk_center_x);
        let Some(chunk) = self.chunks.get_mut(&chunk_x) else {
            return false;
        };
        let applied = chunk.apply_block_snapshot(blocks);
        if applied && chunk.has_fluids() {
            self.activate_fluid_chunk_neighbors(chunk_x);
        }
        applied
    }

    pub fn set_block(&mut self, world_x: i32, world_y: i32, block: BlockType) {
        if world_y < 0 || world_y >= CHUNK_HEIGHT as i32 {
            return;
        }
        let chunk_x = world_x.div_euclid(CHUNK_WIDTH as i32);
        let local_x = world_x.rem_euclid(CHUNK_WIDTH as i32) as usize;
        let local_y = world_y as usize;
        let Some(chunk) = self.chunks.get_mut(&chunk_x) else {
            return;
        };
        let previous = chunk.get_block(local_x, local_y);
        chunk.set_block(local_x, local_y, block);
        if previous == block {
            return;
        }
        if previous.is_fluid() || block.is_fluid() || self.has_adjacent_fluid(world_x, world_y) {
            self.activate_fluid_chunk_neighbors(chunk_x);
        }
        self.prune_unsupported_blocks_near(world_x, world_y, true);
    }

    fn drop_items_for_support_break(&mut self, world_x: i32, world_y: i32, block: BlockType) {
        match block {
            BlockType::RedFlower => {
                self.recent_environment_drops
                    .push((world_x, world_y, ItemType::RedFlower));
            }
            BlockType::YellowFlower => {
                self.recent_environment_drops
                    .push((world_x, world_y, ItemType::YellowFlower));
            }
            BlockType::DeadBush => {
                self.recent_environment_drops
                    .push((world_x, world_y, ItemType::Stick));
            }
            BlockType::TallGrass => {
                if tall_grass_drops_seed_at(world_x, world_y) {
                    self.recent_environment_drops
                        .push((world_x, world_y, ItemType::WheatSeeds));
                }
            }
            BlockType::Crops(stage) => {
                self.recent_environment_drops
                    .push((world_x, world_y, ItemType::WheatSeeds));
                if stage >= 7 {
                    self.recent_environment_drops
                        .push((world_x, world_y, ItemType::Wheat));
                }
            }
            BlockType::Sapling => {
                self.recent_environment_drops
                    .push((world_x, world_y, ItemType::Sapling));
            }
            BlockType::BirchSapling => {
                self.recent_environment_drops
                    .push((world_x, world_y, ItemType::BirchSapling));
            }
            BlockType::NetherWart(stage) => {
                let drop_count = nether_wart_drop_count_at(world_x, world_y, stage >= 3);
                for _ in 0..drop_count {
                    self.recent_environment_drops
                        .push((world_x, world_y, ItemType::NetherWart));
                }
            }
            BlockType::Cactus => {
                self.recent_environment_drops
                    .push((world_x, world_y, ItemType::Cactus));
            }
            BlockType::SugarCane => {
                self.recent_environment_drops
                    .push((world_x, world_y, ItemType::SugarCane));
            }
            _ => {}
        }
    }

    fn prune_unsupported_block_at(&mut self, world_x: i32, world_y: i32, drop_items: bool) {
        let block = self.get_block(world_x, world_y);
        if !block.needs_bottom_support() {
            return;
        }
        let ground = self.get_block(world_x, world_y + 1);
        let supported = if block == BlockType::SugarCane {
            self.can_support_sugar_cane_at(world_x, world_y)
        } else {
            block.can_stay_on(ground)
        };
        if supported {
            return;
        }
        if drop_items {
            self.drop_items_for_support_break(world_x, world_y, block);
        }
        self.set_block(world_x, world_y, BlockType::Air);
    }

    fn prune_unsupported_blocks_near(&mut self, world_x: i32, world_y: i32, drop_items: bool) {
        self.prune_unsupported_block_at(world_x, world_y, drop_items);
        self.prune_unsupported_block_at(world_x, world_y - 1, drop_items);
    }

    pub fn chest_inventory(&self, world_x: i32, world_y: i32) -> Option<&Inventory> {
        if world_y < 0 || world_y >= CHUNK_HEIGHT as i32 {
            return None;
        }
        let chunk_x = world_x.div_euclid(CHUNK_WIDTH as i32);
        let local_x = world_x.rem_euclid(CHUNK_WIDTH as i32) as usize;
        let local_y = world_y as usize;
        self.chunks.get(&chunk_x)?.chest_inventory(local_x, local_y)
    }

    pub fn chest_inventory_mut(&mut self, world_x: i32, world_y: i32) -> Option<&mut Inventory> {
        if world_y < 0 || world_y >= CHUNK_HEIGHT as i32 {
            return None;
        }
        let chunk_x = world_x.div_euclid(CHUNK_WIDTH as i32);
        let local_x = world_x.rem_euclid(CHUNK_WIDTH as i32) as usize;
        let local_y = world_y as usize;
        self.chunks
            .get_mut(&chunk_x)?
            .chest_inventory_mut(local_x, local_y)
    }

    pub fn ensure_chest_inventory(
        &mut self,
        world_x: i32,
        world_y: i32,
        capacity: usize,
    ) -> Option<&mut Inventory> {
        if world_y < 0 || world_y >= CHUNK_HEIGHT as i32 {
            return None;
        }
        let chunk_x = world_x.div_euclid(CHUNK_WIDTH as i32);
        let local_x = world_x.rem_euclid(CHUNK_WIDTH as i32) as usize;
        let local_y = world_y as usize;
        self.chunks
            .get_mut(&chunk_x)?
            .ensure_chest_inventory(local_x, local_y, capacity)
    }

    pub fn remove_chest_inventory(&mut self, world_x: i32, world_y: i32) -> Option<Inventory> {
        if world_y < 0 || world_y >= CHUNK_HEIGHT as i32 {
            return None;
        }
        let chunk_x = world_x.div_euclid(CHUNK_WIDTH as i32);
        let local_x = world_x.rem_euclid(CHUNK_WIDTH as i32) as usize;
        let local_y = world_y as usize;
        self.chunks
            .get_mut(&chunk_x)?
            .remove_chest_inventory(local_x, local_y)
    }

    pub fn redstone_power_at(&self, world_x: i32, world_y: i32) -> u8 {
        self.redstone_source_power(self.get_block(world_x, world_y))
    }

    fn redstone_power_from_to(
        &self,
        src_x: i32,
        src_y: i32,
        dst_x: i32,
        dst_y: i32,
        decay_dust: bool,
    ) -> u8 {
        match self.get_block(src_x, src_y) {
            BlockType::Lever(true) => REDSTONE_MAX_POWER,
            BlockType::StoneButton(timer) if timer > 0 => REDSTONE_MAX_POWER,
            BlockType::RedstoneTorch(true) => REDSTONE_MAX_POWER,
            BlockType::RedstoneRepeater {
                powered,
                facing_right,
                ..
            } if powered => {
                let out_x = if facing_right { src_x + 1 } else { src_x - 1 };
                if dst_x == out_x && dst_y == src_y {
                    REDSTONE_MAX_POWER
                } else {
                    0
                }
            }
            BlockType::RedstoneDust(level) => {
                if decay_dust {
                    level.saturating_sub(1)
                } else {
                    level
                }
            }
            _ => 0,
        }
    }

    fn redstone_transmission_power_from_to(
        &self,
        src_x: i32,
        src_y: i32,
        dst_x: i32,
        dst_y: i32,
    ) -> u8 {
        self.redstone_power_from_to(src_x, src_y, dst_x, dst_y, true)
    }

    fn redstone_component_power_from_to(
        &self,
        src_x: i32,
        src_y: i32,
        dst_x: i32,
        dst_y: i32,
    ) -> u8 {
        self.redstone_power_from_to(src_x, src_y, dst_x, dst_y, false)
    }

    pub fn redstone_neighbor_power(&self, world_x: i32, world_y: i32) -> u8 {
        let mut power = 0u8;
        for (nx, ny) in [
            (world_x - 1, world_y),
            (world_x + 1, world_y),
            (world_x, world_y - 1),
            (world_x, world_y + 1),
        ] {
            power = power.max(self.redstone_component_power_from_to(nx, ny, world_x, world_y));
        }
        power
    }

    pub fn is_redstone_powered(&self, world_x: i32, world_y: i32) -> bool {
        self.redstone_neighbor_power(world_x, world_y) > 0
    }

    pub fn load_chunks_around(&mut self, player_x: i32) {
        self.load_chunks_for_motion(player_x as f64, 0.0);
    }

    pub fn load_chunks_for_spawn_search(&mut self, center_x: i32, search_radius: i32) {
        let load_start = Instant::now();
        self.collect_ready_chunks();

        let center_chunk = center_x.div_euclid(CHUNK_WIDTH as i32);
        self.trim_stale_chunk_requests(center_chunk);

        let radius = search_radius.max(0);
        let min_chunk = (center_x - radius).div_euclid(CHUNK_WIDTH as i32);
        let max_chunk = (center_x + radius).div_euclid(CHUNK_WIDTH as i32);
        for chunk_x in min_chunk..=max_chunk {
            self.ensure_chunk_loaded_now(chunk_x);
        }

        self.chunk_metrics.pending_requests = self.pending_chunks.len() as u64;
        let elapsed_us = load_start.elapsed().as_micros() as u64;
        self.chunk_metrics.last_load_us = elapsed_us;
        self.chunk_metrics.max_load_us = self.chunk_metrics.max_load_us.max(elapsed_us);
    }

    pub fn load_chunks_for_motion(&mut self, player_x: f64, player_vx: f64) {
        let load_start = Instant::now();
        self.newly_generated_chunks.clear();
        self.collect_ready_chunks();

        let player_block_x = player_x.floor() as i32;
        let center_chunk = player_block_x.div_euclid(CHUNK_WIDTH as i32);
        self.trim_stale_chunk_requests(center_chunk);

        let chunk_origin_x = center_chunk * CHUNK_WIDTH as i32;
        let local_x = player_x - chunk_origin_x as f64;
        let motion_lead = if player_vx > 0.08
            && local_x >= CHUNK_WIDTH as f64 * CHUNK_MOTION_LOAD_THRESHOLD_RATIO
        {
            1
        } else if player_vx < -0.08
            && local_x <= CHUNK_WIDTH as f64 * (1.0 - CHUNK_MOTION_LOAD_THRESHOLD_RATIO)
        {
            -1
        } else {
            0
        };

        let active_min = center_chunk - ACTIVE_CHUNK_RADIUS + motion_lead.min(0);
        let active_max = center_chunk + ACTIVE_CHUNK_RADIUS + motion_lead.max(0);
        for chunk_x in active_min..=active_max {
            self.ensure_chunk_loaded_now(chunk_x);
        }

        let prefetch_min = center_chunk - PREFETCH_CHUNK_RADIUS + motion_lead.min(0);
        let prefetch_max = center_chunk + PREFETCH_CHUNK_RADIUS + motion_lead.max(0);
        for chunk_x in prefetch_min..=prefetch_max {
            if (active_min..=active_max).contains(&chunk_x) {
                continue;
            }
            self.enqueue_chunk_request(chunk_x, center_chunk);
        }
        self.dispatch_chunk_requests();

        self.chunk_metrics.pending_requests = self.pending_chunks.len() as u64;
        let elapsed_us = load_start.elapsed().as_micros() as u64;
        self.chunk_metrics.last_load_us = elapsed_us;
        self.chunk_metrics.max_load_us = self.chunk_metrics.max_load_us.max(elapsed_us);
    }

    pub fn save_all(&mut self) {
        for chunk in self.chunks.values_mut() {
            chunk.save_to_disk();
        }
    }

    fn has_adjacent_fluid(&self, world_x: i32, world_y: i32) -> bool {
        [
            (world_x - 1, world_y),
            (world_x + 1, world_y),
            (world_x, world_y - 1),
            (world_x, world_y + 1),
        ]
        .into_iter()
        .any(|(x, y)| self.get_block(x, y).is_fluid())
    }

    fn activate_fluid_chunk_neighbors(&mut self, center_chunk: i32) {
        for chunk_x in (center_chunk - 1)..=(center_chunk + 1) {
            self.active_fluid_chunks.insert(chunk_x);
        }
    }

    fn activate_fluid_chunk_neighbors_in(target: &mut HashSet<i32>, center_chunk: i32) {
        for chunk_x in (center_chunk - 1)..=(center_chunk + 1) {
            target.insert(chunk_x);
        }
    }

    pub fn save_dirty_chunk_budget(&mut self, max_chunks: usize) -> bool {
        if max_chunks == 0 {
            return self.chunks.values().any(|chunk| chunk.dirty);
        }

        let mut saved = 0usize;
        for chunk in self.chunks.values_mut() {
            if !chunk.dirty {
                continue;
            }
            chunk.save_to_disk();
            saved += 1;
            if saved >= max_chunks {
                break;
            }
        }

        self.chunks.values().any(|chunk| chunk.dirty)
    }

    pub fn update(&mut self, center_x: i32) {
        self.tick_counter += 1;
        self.recent_explosions.clear();
        self.recent_explosion_block_losses.clear();
        let center_chunk = center_x.div_euclid(CHUNK_WIDTH as i32);
        if self.tick_counter.is_multiple_of(REDSTONE_TICK_INTERVAL) {
            self.update_redstone(center_chunk);
            self.update_redstone_outputs(center_chunk);
        }
        if self.tick_counter.is_multiple_of(2) {
            self.update_falling_blocks(center_chunk);
        }
        if self.tick_counter.is_multiple_of(5) {
            self.update_fluids(center_chunk);
        }
        if self.tick_counter.is_multiple_of(20) {
            self.update_leaf_decay(center_chunk);
            self.update_farming(center_chunk);
            self.update_grass_spread(center_chunk);
        }
    }

    fn update_farming(&mut self, center_chunk: i32) {
        let mut changes = Vec::new();
        let mut saplings_to_grow = Vec::new();
        let mut support_drops = Vec::new();
        let mut rng = rand::thread_rng();
        let growth_phase = (self.tick_counter / 20) as i32;
        for (&cx, chunk) in &self.chunks {
            if (cx - center_chunk).abs() > 2 || !chunk.has_farming_blocks() {
                continue;
            }
            for y in 0..CHUNK_HEIGHT {
                for x in 0..CHUNK_WIDTH {
                    let block = chunk.get_block(x, y);
                    let wx = cx * CHUNK_WIDTH as i32 + x as i32;
                    let wy = y as i32;

                    match block {
                        BlockType::Farmland(moisture) => {
                            let has_water = self.has_nearby_farmland_water(wx, wy);

                            if has_water && moisture < 7 {
                                changes.push((wx, wy, BlockType::Farmland(7)));
                            } else if !has_water && moisture > 0 {
                                changes.push((wx, wy, BlockType::Farmland(moisture - 1)));
                            } else if !has_water
                                && moisture == 0
                                && self.get_block(wx, wy - 1) == BlockType::Air
                                && dry_farmland_revert_due_at(wx, wy, growth_phase)
                            {
                                // Dry farmland should linger for a while before reverting,
                                // otherwise hoeing bare soil feels broken and crops are too fiddly to place.
                                changes.push((wx, wy, BlockType::Dirt));
                            }
                        }
                        BlockType::Crops(stage) => {
                            let ground = self.get_block(wx, wy + 1);
                            if let BlockType::Farmland(moisture) = ground {
                                if stage < 7 {
                                    // Chance to grow based on moisture
                                    let grow_chance = if moisture > 0 { 0.2 } else { 0.05 };
                                    if rng.gen_bool(grow_chance) {
                                        changes.push((wx, wy, BlockType::Crops(stage + 1)));
                                    }
                                }
                            } else {
                                // Break if not on farmland
                                changes.push((wx, wy, BlockType::Air));
                            }
                        }
                        BlockType::Sapling | BlockType::BirchSapling => {
                            if !block.can_stay_on(self.get_block(wx, wy + 1)) {
                                changes.push((wx, wy, BlockType::Air));
                            } else if sapling_growth_due_at(wx, wy, growth_phase)
                                && self.can_grow_sapling_tree(wx, wy)
                            {
                                saplings_to_grow.push((wx, wy));
                            }
                        }
                        BlockType::SugarCane => {
                            if !self.can_support_sugar_cane_at(wx, wy) {
                                changes.push((wx, wy, BlockType::Air));
                                support_drops.push((wx, wy, ItemType::SugarCane));
                            } else if self.get_block(wx, wy - 1) == BlockType::Air
                                && self.sugar_cane_height_at(wx, wy) < 3
                                && sugar_cane_growth_due_at(wx, wy, growth_phase)
                            {
                                changes.push((wx, wy - 1, BlockType::SugarCane));
                            }
                        }
                        BlockType::NetherWart(stage) => {
                            if !block.can_stay_on(self.get_block(wx, wy + 1)) {
                                changes.push((wx, wy, BlockType::Air));
                                let drop_count = nether_wart_drop_count_at(wx, wy, stage >= 3);
                                for _ in 0..drop_count {
                                    support_drops.push((wx, wy, ItemType::NetherWart));
                                }
                            } else if stage < 3 && nether_wart_growth_due_at(wx, wy, growth_phase) {
                                changes.push((wx, wy, BlockType::NetherWart(stage + 1)));
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
        for drop in support_drops {
            self.recent_environment_drops.push(drop);
        }
        for (wx, wy, block) in changes {
            self.set_block(wx, wy, block);
        }
        for (wx, wy) in saplings_to_grow {
            self.grow_sapling_tree(wx, wy);
        }
    }

    pub fn has_nearby_farmland_water(&self, wx: i32, wy: i32) -> bool {
        for dx in -4..=4 {
            for dy in -1..=1 {
                if matches!(self.get_block(wx + dx, wy + dy), BlockType::Water(_)) {
                    return true;
                }
            }
        }
        false
    }

    pub fn can_support_sugar_cane_at(&self, wx: i32, wy: i32) -> bool {
        let ground = self.get_block(wx, wy + 1);
        if ground == BlockType::SugarCane {
            return true;
        }
        if !matches!(ground, BlockType::Grass | BlockType::Dirt | BlockType::Sand) {
            return false;
        }
        matches!(self.get_block(wx - 1, wy + 1), BlockType::Water(_))
            || matches!(self.get_block(wx + 1, wy + 1), BlockType::Water(_))
    }

    fn sugar_cane_height_at(&self, wx: i32, wy: i32) -> i32 {
        let mut height = 1;
        let mut current_y = wy;
        while self.get_block(wx, current_y + 1) == BlockType::SugarCane {
            height += 1;
            current_y += 1;
        }
        height
    }

    fn tree_blocks_for_biome(biome: BiomeType) -> (BlockType, BlockType) {
        if matches!(biome, BiomeType::Tundra | BiomeType::ExtremeHills) {
            (BlockType::BirchWood, BlockType::BirchLeaves)
        } else {
            (BlockType::Wood, BlockType::Leaves)
        }
    }

    fn tree_blocks_for_sapling(
        &self,
        sapling_block: BlockType,
        biome: BiomeType,
    ) -> Option<(BlockType, BlockType)> {
        match sapling_block {
            BlockType::Sapling => Some(Self::tree_blocks_for_biome(biome)),
            BlockType::BirchSapling => Some((BlockType::BirchWood, BlockType::BirchLeaves)),
            _ => None,
        }
    }

    fn sapling_tree_height_for(
        &self,
        wx: i32,
        wy: i32,
        sapling_block: BlockType,
        biome: BiomeType,
    ) -> i32 {
        let variation = (wx.wrapping_mul(17) ^ wy.wrapping_mul(13)).rem_euclid(3);
        match sapling_block {
            BlockType::BirchSapling => 4 + variation.min(1),
            _ => match biome {
                BiomeType::Jungle => 5 + variation,
                BiomeType::Taiga => 4 + variation.min(1),
                BiomeType::Swamp => 4,
                BiomeType::Tundra | BiomeType::ExtremeHills => 4,
                _ => 3 + variation,
            },
        }
    }

    fn sapling_item_for_leaf_block(block: BlockType) -> Option<ItemType> {
        match block {
            BlockType::Leaves => Some(ItemType::Sapling),
            BlockType::BirchLeaves => Some(ItemType::BirchSapling),
            _ => None,
        }
    }

    fn is_tree_growth_replaceable(block: BlockType) -> bool {
        matches!(
            block,
            BlockType::Air
                | BlockType::Leaves
                | BlockType::BirchLeaves
                | BlockType::TallGrass
                | BlockType::RedFlower
                | BlockType::YellowFlower
                | BlockType::Sapling
                | BlockType::BirchSapling
        )
    }

    fn can_grow_sapling_tree(&self, wx: i32, wy: i32) -> bool {
        let sapling_block = self.get_block(wx, wy);
        if !matches!(sapling_block, BlockType::Sapling | BlockType::BirchSapling) {
            return false;
        }
        if !sapling_block.can_stay_on(self.get_block(wx, wy + 1)) {
            return false;
        }

        let biome = self.get_biome(wx);
        let Some((_, leaf_block)) = self.tree_blocks_for_sapling(sapling_block, biome) else {
            return false;
        };
        let trunk_height = self.sapling_tree_height_for(wx, wy, sapling_block, biome);
        let crown_y = wy - (trunk_height - 1);
        if crown_y - 3 < 0 {
            return false;
        }

        for dy in 0..trunk_height {
            if !Self::is_tree_growth_replaceable(self.get_block(wx, wy - dy)) {
                return false;
            }
        }

        for ly in (crown_y - 2)..=crown_y {
            let width = if biome == BiomeType::Taiga {
                if ly == crown_y - 2 { 0 } else { 1 }
            } else if ly == crown_y - 2 {
                1
            } else {
                2
            };
            for lx in (wx - width)..=(wx + width) {
                let block = self.get_block(lx, ly);
                if !Self::is_tree_growth_replaceable(block) && block != leaf_block {
                    return false;
                }
            }
        }

        if matches!(biome, BiomeType::Taiga | BiomeType::Jungle) {
            let cap_y = crown_y - 3;
            let block = self.get_block(wx, cap_y);
            if !Self::is_tree_growth_replaceable(block) && block != leaf_block {
                return false;
            }
        }

        true
    }

    fn grow_sapling_tree(&mut self, wx: i32, wy: i32) {
        if !self.can_grow_sapling_tree(wx, wy) {
            return;
        }

        let sapling_block = self.get_block(wx, wy);
        let biome = self.get_biome(wx);
        let Some((wood_block, leaf_block)) = self.tree_blocks_for_sapling(sapling_block, biome)
        else {
            return;
        };
        let trunk_height = self.sapling_tree_height_for(wx, wy, sapling_block, biome);
        let crown_y = wy - (trunk_height - 1);

        for dy in 0..trunk_height {
            self.set_block(wx, wy - dy, wood_block);
        }

        for ly in (crown_y - 2)..=crown_y {
            let width = if biome == BiomeType::Taiga {
                if ly == crown_y - 2 { 0 } else { 1 }
            } else if ly == crown_y - 2 {
                1
            } else {
                2
            };
            for lx in (wx - width)..=(wx + width) {
                if self.get_block(lx, ly) == BlockType::Air {
                    self.set_block(lx, ly, leaf_block);
                }
            }
        }

        if matches!(biome, BiomeType::Taiga | BiomeType::Jungle) {
            let cap_y = crown_y - 3;
            if self.get_block(wx, cap_y) == BlockType::Air {
                self.set_block(wx, cap_y, leaf_block);
            }
        }
    }

    fn find_bone_meal_grass_surface_near(&self, wx: i32, center_ground_y: i32) -> Option<i32> {
        let min_y = (center_ground_y - 2).clamp(1, CHUNK_HEIGHT as i32 - 2);
        let max_y = (center_ground_y + 2).clamp(min_y, CHUNK_HEIGHT as i32 - 2);
        let mut best: Option<(i32, i32)> = None;
        for ground_y in min_y..=max_y {
            if self.get_block(wx, ground_y) != BlockType::Grass
                || self.get_block(wx, ground_y - 1) != BlockType::Air
            {
                continue;
            }
            let score = (ground_y - center_ground_y).abs();
            match best {
                None => best = Some((score, ground_y)),
                Some((best_score, _)) if score < best_score => best = Some((score, ground_y)),
                _ => {}
            }
        }
        best.map(|(_, ground_y)| ground_y)
    }

    fn apply_bone_meal_to_grass(&mut self, center_x: i32, center_ground_y: i32) -> bool {
        let mut chosen = Vec::new();
        let mut fallback: Option<(i32, i32, BlockType)> = None;
        for dx in -3..=3 {
            let wx = center_x + dx;
            let Some(ground_y) = self.find_bone_meal_grass_surface_near(wx, center_ground_y) else {
                continue;
            };
            let place_y = ground_y - 1;
            let flora = bone_meal_flora_block_at(wx, place_y);
            let should_place = dx == 0
                || ((center_x.wrapping_mul(29)
                    ^ center_ground_y.wrapping_mul(31)
                    ^ wx.wrapping_mul(17)
                    ^ ground_y.wrapping_mul(13))
                .rem_euclid(3))
                    != 0;
            if should_place {
                chosen.push((wx, place_y, flora));
            } else if fallback.is_none() {
                fallback = Some((wx, place_y, flora));
            }
        }

        if chosen.is_empty()
            && let Some(candidate) = fallback
        {
            chosen.push(candidate);
        }
        if chosen.is_empty() {
            return false;
        }

        for (wx, place_y, flora) in chosen {
            self.set_block(wx, place_y, flora);
        }
        true
    }

    pub fn apply_bone_meal(&mut self, wx: i32, wy: i32) -> bool {
        match self.get_block(wx, wy) {
            BlockType::Crops(stage) if stage < 7 => {
                self.set_block(wx, wy, BlockType::Crops(7));
                true
            }
            BlockType::Sapling | BlockType::BirchSapling if self.can_grow_sapling_tree(wx, wy) => {
                self.grow_sapling_tree(wx, wy);
                true
            }
            BlockType::Grass => self.apply_bone_meal_to_grass(wx, wy),
            BlockType::TallGrass | BlockType::RedFlower | BlockType::YellowFlower
                if self.get_block(wx, wy + 1) == BlockType::Grass =>
            {
                self.apply_bone_meal_to_grass(wx, wy + 1)
            }
            _ => false,
        }
    }

    fn update_leaf_decay(&mut self, center_chunk: i32) {
        let mut to_remove = Vec::new();
        let decay_phase = (self.tick_counter / 20) as i32;
        for (&cx, chunk) in &self.chunks {
            if (cx - center_chunk).abs() > 2 || !chunk.has_leaf_blocks() {
                continue;
            }
            for y in 0..CHUNK_HEIGHT {
                for x in 0..CHUNK_WIDTH {
                    let block = chunk.get_block(x, y);
                    if block.is_leaf_block() {
                        let wx = cx * CHUNK_WIDTH as i32 + x as i32;
                        let wy = y as i32;
                        let decay_due = ((wx.wrapping_mul(31)
                            ^ wy.wrapping_mul(17)
                            ^ decay_phase.wrapping_mul(13))
                            & 3)
                            == 0;
                        if !self.is_near_wood(wx, wy, 4) && decay_due {
                            to_remove.push((wx, wy, block));
                        }
                    }
                }
            }
        }
        for (wx, wy, block) in to_remove {
            let sapling_drop =
                ((wx.wrapping_mul(19) ^ wy.wrapping_mul(23) ^ decay_phase.wrapping_mul(7))
                    .rem_euclid(7))
                    == 0;
            if let Some(sapling_item) = Self::sapling_item_for_leaf_block(block)
                && sapling_drop
            {
                self.recent_environment_drops.push((wx, wy, sapling_item));
            }
            self.set_block(wx, wy, BlockType::Air);
        }
    }

    fn grass_can_survive_at(&self, wx: i32, wy: i32) -> bool {
        let above = self.get_block(wx, wy - 1);
        !above.is_solid() && !above.is_fluid()
    }

    fn dirt_can_gain_grass(&self, wx: i32, wy: i32) -> bool {
        if !self.grass_can_survive_at(wx, wy) {
            return false;
        }
        for dx in -1..=1 {
            for dy in -1..=1 {
                if dx == 0 && dy == 0 {
                    continue;
                }
                if self.get_block(wx + dx, wy + dy) != BlockType::Grass {
                    continue;
                }
                if self.grass_can_survive_at(wx + dx, wy + dy) {
                    return true;
                }
            }
        }
        false
    }

    fn update_grass_spread(&mut self, center_chunk: i32) {
        let mut changes = Vec::new();
        let spread_phase = (self.tick_counter / 20) as i32;
        for (&cx, chunk) in &self.chunks {
            if (cx - center_chunk).abs() > 2 {
                continue;
            }
            for y in 1..CHUNK_HEIGHT {
                for x in 0..CHUNK_WIDTH {
                    let block = chunk.get_block(x, y);
                    let wx = cx * CHUNK_WIDTH as i32 + x as i32;
                    let wy = y as i32;
                    match block {
                        BlockType::Grass => {
                            if !self.grass_can_survive_at(wx, wy) {
                                changes.push((wx, wy, BlockType::Dirt));
                            }
                        }
                        BlockType::Dirt => {
                            let spread_due = ((wx.wrapping_mul(13)
                                ^ wy.wrapping_mul(7)
                                ^ spread_phase.wrapping_mul(5))
                                & 3)
                                == 0;
                            if self.dirt_can_gain_grass(wx, wy) && spread_due {
                                changes.push((wx, wy, BlockType::Grass));
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
        for (wx, wy, block) in changes {
            self.set_block(wx, wy, block);
        }
    }

    fn is_near_wood(&self, wx: i32, wy: i32, range: i32) -> bool {
        for dy in -range..=range {
            for dx in -range..=range {
                let b = self.get_block(wx + dx, wy + dy);
                if b == BlockType::Wood || b == BlockType::BirchWood {
                    return true;
                }
            }
        }
        false
    }

    fn update_falling_blocks(&mut self, center_chunk: i32) {
        let mut moves = Vec::new();
        for (&cx, chunk) in &self.chunks {
            if (cx - center_chunk).abs() > 2 || !chunk.has_falling_blocks() {
                continue;
            }
            for y in (0..(CHUNK_HEIGHT - 1)).rev() {
                for x in 0..CHUNK_WIDTH {
                    let block = chunk.get_block(x, y);
                    if block.obeys_gravity() && chunk.get_block(x, y + 1) == BlockType::Air {
                        moves.push((cx * CHUNK_WIDTH as i32 + x as i32, y as i32, block));
                    }
                }
            }
        }
        for (wx, wy, block) in moves {
            if self.get_block(wx, wy + 1) == BlockType::Air {
                self.set_block(wx, wy, BlockType::Air);
                self.set_block(wx, wy + 1, block);
            }
        }
    }

    fn can_form_infinite_water_source(&self, x: i32, y: i32) -> bool {
        if y + 1 >= CHUNK_HEIGHT as i32 {
            return false;
        }
        let below = self.get_block(x, y + 1);
        (below.is_solid() || matches!(below, BlockType::Water(_)))
            && matches!(self.get_block(x - 1, y), BlockType::Water(8))
            && matches!(self.get_block(x + 1, y), BlockType::Water(8))
    }

    fn min_horizontal_fluid_level(&self, is_water: bool) -> u8 {
        if is_water || self.dimension == Dimension::Nether {
            WATER_MIN_HORIZONTAL_FLOW_LEVEL
        } else {
            SLOW_LAVA_MIN_HORIZONTAL_FLOW_LEVEL
        }
    }

    fn should_process_lava(&self) -> bool {
        self.dimension == Dimension::Nether
            || self
                .tick_counter
                .is_multiple_of(OVERWORLD_LAVA_TICK_INTERVAL)
    }

    fn fluid_mix_result(
        &self,
        is_water: bool,
        target: BlockType,
        vertical_flow: bool,
    ) -> Option<BlockType> {
        match (is_water, target) {
            (true, BlockType::Lava(level)) => Some(if level == 8 {
                BlockType::Obsidian
            } else {
                BlockType::Cobblestone
            }),
            (false, BlockType::Water(_)) => Some(if vertical_flow {
                BlockType::Stone
            } else {
                BlockType::Cobblestone
            }),
            _ => None,
        }
    }

    fn maybe_insert_infinite_water_source(
        &self,
        next_state: &mut std::collections::HashMap<(i32, i32), BlockType>,
        source_x: i32,
        source_y: i32,
        gap_dir: i32,
    ) -> bool {
        let gap_x = source_x + gap_dir;
        if self.get_block(gap_x, source_y) != BlockType::Air {
            return false;
        }

        if self.can_form_infinite_water_source(gap_x, source_y) {
            return next_state.insert((gap_x, source_y), BlockType::Water(8))
                != Some(BlockType::Water(8));
        }
        false
    }

    fn update_fluids(&mut self, center_chunk: i32) {
        use std::collections::{HashMap, HashSet};
        let scheduled_chunks: Vec<i32> = self
            .active_fluid_chunks
            .iter()
            .copied()
            .filter(|chunk_x| {
                (chunk_x - center_chunk).abs() <= 2
                    && self
                        .chunks
                        .get(chunk_x)
                        .is_some_and(|chunk| chunk.has_fluids())
            })
            .collect();
        let mut next_active_chunks: HashSet<i32> = self
            .active_fluid_chunks
            .iter()
            .copied()
            .filter(|chunk_x| {
                (chunk_x - center_chunk).abs() > 2
                    && self
                        .chunks
                        .get(chunk_x)
                        .is_some_and(|chunk| chunk.has_fluids())
            })
            .collect();
        if scheduled_chunks.is_empty() {
            self.active_fluid_chunks = next_active_chunks;
            return;
        }

        let mut next_state = HashMap::new();
        let process_lava = self.should_process_lava();

        for cx in scheduled_chunks {
            let Some(chunk) = self.chunks.get(&cx) else {
                continue;
            };
            let mut skipped_lava = false;
            for y in 0..CHUNK_HEIGHT {
                for x in 0..CHUNK_WIDTH {
                    let block = chunk.get_block(x, y);
                    let wx = cx * CHUNK_WIDTH as i32 + x as i32;
                    let wy = y as i32;

                    let (is_water, mut level) = match block {
                        BlockType::Water(l) => (true, l),
                        BlockType::Lava(l) => (false, l),
                        _ => continue,
                    };
                    if !is_water && !process_lava {
                        skipped_lava = true;
                        continue;
                    }

                    if is_water && level == 8 {
                        if self.maybe_insert_infinite_water_source(&mut next_state, wx, wy, -1) {
                            Self::activate_fluid_chunk_neighbors_in(&mut next_active_chunks, cx);
                        }
                        if self.maybe_insert_infinite_water_source(&mut next_state, wx, wy, 1) {
                            Self::activate_fluid_chunk_neighbors_in(&mut next_active_chunks, cx);
                        }
                    }

                    if is_water && self.can_form_infinite_water_source(wx, wy) {
                        if next_state.insert((wx, wy), BlockType::Water(8))
                            != Some(BlockType::Water(8))
                        {
                            Self::activate_fluid_chunk_neighbors_in(&mut next_active_chunks, cx);
                        }
                        level = 8;
                    }

                    let is_same = |b: BlockType| {
                        if is_water {
                            matches!(b, BlockType::Water(_))
                        } else {
                            matches!(b, BlockType::Lava(_))
                        }
                    };
                    let get_l = |b: BlockType| match b {
                        BlockType::Water(l) => l,
                        BlockType::Lava(l) => l,
                        _ => 0,
                    };

                    let above = self.get_block(wx, wy - 1);
                    let left = self.get_block(wx - 1, wy);
                    let right = self.get_block(wx + 1, wy);

                    let supported = if level == 8 {
                        true
                    } else {
                        is_same(above)
                            || (is_same(left) && get_l(left) > level)
                            || (is_same(right) && get_l(right) > level)
                    };

                    if !supported {
                        let new_block = if level > 1 {
                            if is_water {
                                BlockType::Water(level - 1)
                            } else {
                                BlockType::Lava(level - 1)
                            }
                        } else {
                            BlockType::Air
                        };
                        if Self::insert_fluid_change(&mut next_state, wx, wy, new_block) {
                            Self::activate_fluid_chunk_neighbors_in(&mut next_active_chunks, cx);
                        }
                        continue;
                    }

                    let below = self.get_block(wx, wy + 1);
                    let mut spread_horizontally = false;

                    if wy + 1 < CHUNK_HEIGHT as i32 {
                        if let Some(mixed_block) = self.fluid_mix_result(is_water, below, true) {
                            if next_state.insert((wx, wy + 1), mixed_block) != Some(mixed_block) {
                                Self::activate_fluid_chunk_neighbors_in(
                                    &mut next_active_chunks,
                                    wx.div_euclid(CHUNK_WIDTH as i32),
                                );
                            }
                        } else if below.is_replaceable() {
                            let fall_block = if is_water {
                                BlockType::Water(7)
                            } else {
                                BlockType::Lava(7)
                            };
                            if Self::insert_fluid_change(&mut next_state, wx, wy + 1, fall_block) {
                                Self::activate_fluid_chunk_neighbors_in(
                                    &mut next_active_chunks,
                                    wx.div_euclid(CHUNK_WIDTH as i32),
                                );
                            }
                        } else {
                            spread_horizontally = true;
                        }
                    }

                    if spread_horizontally && level > self.min_horizontal_fluid_level(is_water) {
                        let spread_block = if is_water {
                            BlockType::Water(level - 1)
                        } else {
                            BlockType::Lava(level - 1)
                        };

                        for nx in [wx - 1, wx + 1] {
                            let side = self.get_block(nx, wy);
                            if let Some(mixed_block) = self.fluid_mix_result(is_water, side, false)
                            {
                                if next_state.insert((nx, wy), mixed_block) != Some(mixed_block) {
                                    Self::activate_fluid_chunk_neighbors_in(
                                        &mut next_active_chunks,
                                        nx.div_euclid(CHUNK_WIDTH as i32),
                                    );
                                }
                            } else if side.is_replaceable()
                                && Self::insert_fluid_change(&mut next_state, nx, wy, spread_block)
                            {
                                Self::activate_fluid_chunk_neighbors_in(
                                    &mut next_active_chunks,
                                    nx.div_euclid(CHUNK_WIDTH as i32),
                                );
                            }
                        }
                    }
                }
            }
            if skipped_lava {
                Self::activate_fluid_chunk_neighbors_in(&mut next_active_chunks, cx);
            }
        }

        for ((wx, wy), block) in next_state {
            self.set_block(wx, wy, block);
        }
        self.active_fluid_chunks = next_active_chunks;
    }

    fn update_redstone(&mut self, center_chunk: i32) {
        let mut trigger_changes = Vec::new();
        for (&cx, chunk) in &self.chunks {
            if (cx - center_chunk).abs() > 2 || !chunk.has_redstone_blocks() {
                continue;
            }
            for y in 0..CHUNK_HEIGHT {
                for x in 0..CHUNK_WIDTH {
                    let block = chunk.get_block(x, y);
                    let wx = cx * CHUNK_WIDTH as i32 + x as i32;
                    let wy = y as i32;
                    match block {
                        BlockType::StoneButton(timer) => {
                            if timer > 0 {
                                trigger_changes.push((wx, wy, BlockType::StoneButton(timer - 1)));
                            }
                        }
                        BlockType::RedstoneTorch(lit) => {
                            let should_lit = !self.has_adjacent_redstone_input(wx, wy);
                            if lit != should_lit {
                                trigger_changes.push((
                                    wx,
                                    wy,
                                    BlockType::RedstoneTorch(should_lit),
                                ));
                            }
                        }
                        BlockType::RedstoneRepeater {
                            powered,
                            delay,
                            facing_right,
                            timer,
                            target_powered,
                        } => {
                            if !self.get_block(wx, wy + 1).is_solid() {
                                trigger_changes.push((wx, wy, BlockType::Air));
                                continue;
                            }
                            let clamped_delay = delay
                                .clamp(REDSTONE_REPEATER_MIN_DELAY, REDSTONE_REPEATER_MAX_DELAY);
                            let rear_x = if facing_right { wx - 1 } else { wx + 1 };
                            let desired_powered =
                                self.redstone_component_power_from_to(rear_x, wy, wx, wy) > 0;

                            let mut next_powered = powered;
                            let mut next_timer = timer;
                            let mut next_target = target_powered;

                            if desired_powered == powered {
                                next_timer = 0;
                                next_target = powered;
                            } else if timer == 0 || target_powered != desired_powered {
                                next_timer = clamped_delay;
                                next_target = desired_powered;
                            } else {
                                next_timer = next_timer.saturating_sub(1);
                                if next_timer == 0 {
                                    next_powered = next_target;
                                }
                            }

                            if next_powered != powered
                                || next_timer != timer
                                || next_target != target_powered
                                || clamped_delay != delay
                            {
                                trigger_changes.push((
                                    wx,
                                    wy,
                                    BlockType::RedstoneRepeater {
                                        powered: next_powered,
                                        delay: clamped_delay,
                                        facing_right,
                                        timer: next_timer,
                                        target_powered: next_target,
                                    },
                                ));
                            }
                        }
                        _ => {}
                    }
                }
            }
        }

        for (wx, wy, block) in trigger_changes {
            self.set_block(wx, wy, block);
        }

        let mut dust_changes = Vec::new();
        for (&cx, chunk) in &self.chunks {
            if (cx - center_chunk).abs() > 2 || !chunk.has_redstone_blocks() {
                continue;
            }
            for y in 0..CHUNK_HEIGHT {
                for x in 0..CHUNK_WIDTH {
                    let block = chunk.get_block(x, y);
                    let old_power = match block {
                        BlockType::RedstoneDust(level) => level,
                        _ => continue,
                    };

                    let wx = cx * CHUNK_WIDTH as i32 + x as i32;
                    let wy = y as i32;
                    let below = self.get_block(wx, wy + 1);
                    if !below.is_solid() {
                        dust_changes.push((wx, wy, BlockType::Air));
                        continue;
                    }

                    let mut next_power = 0u8;
                    for (nx, ny) in [(wx - 1, wy), (wx + 1, wy), (wx, wy - 1), (wx, wy + 1)] {
                        next_power = next_power
                            .max(self.redstone_transmission_power_from_to(nx, ny, wx, wy));
                    }

                    if next_power != old_power {
                        dust_changes.push((wx, wy, BlockType::RedstoneDust(next_power)));
                    }
                }
            }
        }

        for (wx, wy, block) in dust_changes {
            self.set_block(wx, wy, block);
        }
    }

    fn update_redstone_outputs(&mut self, center_chunk: i32) {
        let mut state_changes = Vec::new();
        let mut piston_moves = Vec::new();
        let mut explosions = Vec::new();

        for (&cx, chunk) in &self.chunks {
            if (cx - center_chunk).abs() > 2 || !chunk.has_redstone_blocks() {
                continue;
            }
            for y in 0..CHUNK_HEIGHT {
                for x in 0..CHUNK_WIDTH {
                    let wx = cx * CHUNK_WIDTH as i32 + x as i32;
                    let wy = y as i32;
                    match chunk.get_block(x, y) {
                        BlockType::Tnt => {
                            if self.is_redstone_powered(wx, wy) {
                                state_changes.push((wx, wy, BlockType::PrimedTnt(TNT_FUSE_TICKS)));
                            }
                        }
                        BlockType::PrimedTnt(fuse) => {
                            if fuse == 0 {
                                explosions.push((wx, wy));
                            } else {
                                state_changes.push((wx, wy, BlockType::PrimedTnt(fuse - 1)));
                            }
                        }
                        BlockType::Piston {
                            extended,
                            facing_right,
                        } => {
                            let powered = self.is_redstone_powered(wx, wy);
                            if powered && !extended {
                                if let Some(pushes) =
                                    self.compute_piston_extension(wx, wy, facing_right)
                                {
                                    state_changes.push((
                                        wx,
                                        wy,
                                        BlockType::Piston {
                                            extended: true,
                                            facing_right,
                                        },
                                    ));
                                    for (from_x, from_y, to_x, to_y, moved_block) in pushes {
                                        piston_moves.push((from_x, from_y, BlockType::Air));
                                        piston_moves.push((to_x, to_y, moved_block));
                                    }
                                }
                            } else if !powered && extended {
                                state_changes.push((
                                    wx,
                                    wy,
                                    BlockType::Piston {
                                        extended: false,
                                        facing_right,
                                    },
                                ));
                            }
                        }
                        BlockType::StickyPiston {
                            extended,
                            facing_right,
                        } => {
                            let powered = self.is_redstone_powered(wx, wy);
                            if powered && !extended {
                                if let Some(pushes) =
                                    self.compute_piston_extension(wx, wy, facing_right)
                                {
                                    state_changes.push((
                                        wx,
                                        wy,
                                        BlockType::StickyPiston {
                                            extended: true,
                                            facing_right,
                                        },
                                    ));
                                    for (from_x, from_y, to_x, to_y, moved_block) in pushes {
                                        piston_moves.push((from_x, from_y, BlockType::Air));
                                        piston_moves.push((to_x, to_y, moved_block));
                                    }
                                }
                            } else if !powered && extended {
                                state_changes.push((
                                    wx,
                                    wy,
                                    BlockType::StickyPiston {
                                        extended: false,
                                        facing_right,
                                    },
                                ));
                                if let Some((from_x, from_y, to_x, to_y, moved_block)) =
                                    self.compute_sticky_retraction(wx, wy, facing_right)
                                {
                                    piston_moves.push((from_x, from_y, BlockType::Air));
                                    piston_moves.push((to_x, to_y, moved_block));
                                }
                            }
                        }
                        _ => {}
                    }
                }
            }
        }

        for (wx, wy, block) in state_changes {
            self.set_block(wx, wy, block);
        }
        for (wx, wy, block) in piston_moves {
            self.set_block(wx, wy, block);
        }
        for (wx, wy) in explosions {
            self.explode_tnt(wx, wy);
        }
    }

    fn compute_piston_extension(
        &self,
        x: i32,
        y: i32,
        facing_right: bool,
    ) -> Option<Vec<PistonPush>> {
        let dir = if facing_right { 1 } else { -1 };
        let mut push_chain: Vec<(i32, BlockType)> = Vec::new();
        for step in 1..=(PISTON_PUSH_LIMIT as i32 + 1) {
            let wx = x + dir * step;
            let block = self.get_block(wx, y);
            if block == BlockType::Air {
                let mut moves = Vec::new();
                for (from_x, moved_block) in push_chain.iter().rev() {
                    moves.push((*from_x, y, *from_x + dir, y, *moved_block));
                }
                return Some(moves);
            }
            if !Self::can_piston_push(block) || push_chain.len() >= PISTON_PUSH_LIMIT {
                return None;
            }
            push_chain.push((wx, block));
        }
        None
    }

    fn compute_sticky_retraction(&self, x: i32, y: i32, facing_right: bool) -> Option<PistonPush> {
        let dir = if facing_right { 1 } else { -1 };
        let pull_to_x = x + dir;
        if self.get_block(pull_to_x, y) != BlockType::Air {
            return None;
        }
        let pull_from_x = x + dir * 2;
        let block = self.get_block(pull_from_x, y);
        if block == BlockType::Air || !Self::can_piston_push(block) {
            return None;
        }
        Some((pull_from_x, y, pull_to_x, y, block))
    }

    fn can_piston_push(block: BlockType) -> bool {
        !matches!(
            block,
            BlockType::Air
                | BlockType::Bedrock
                | BlockType::Obsidian
                | BlockType::Water(_)
                | BlockType::Lava(_)
                | BlockType::NetherPortal
                | BlockType::EndPortal
                | BlockType::EndPortalFrame { .. }
                | BlockType::PrimedTnt(_)
                | BlockType::IronDoor(_)
                | BlockType::WoodDoor(_)
                | BlockType::SilverfishSpawner
                | BlockType::BlazeSpawner
                | BlockType::ZombieSpawner
                | BlockType::SkeletonSpawner
                | BlockType::Piston { .. }
                | BlockType::StickyPiston { .. }
        )
    }

    fn explode_tnt(&mut self, cx: i32, cy: i32) {
        self.trigger_explosion(
            cx,
            cy,
            TNT_BLAST_RADIUS,
            TNT_BLAST_STRENGTH,
            TNT_CHAIN_FUSE_TICKS,
        );
    }

    fn explosion_block_resistance(block: BlockType) -> f32 {
        match block {
            BlockType::Air => 0.0,
            BlockType::Bedrock => 1_000_000.0,
            BlockType::Obsidian => 1_200.0,
            BlockType::EndPortalFrame { .. } => 1_200.0,
            BlockType::NetherPortal => 10_000.0,
            BlockType::EndPortal => 10_000.0,
            BlockType::Water(_) | BlockType::Lava(_) => 900.0,
            BlockType::Stone
            | BlockType::StoneBricks
            | BlockType::CoalOre
            | BlockType::IronOre
            | BlockType::GoldOre
            | BlockType::DiamondOre
            | BlockType::RedstoneOre
            | BlockType::Cobblestone
            | BlockType::EndStone
            | BlockType::StoneSlab
            | BlockType::StoneStairs
            | BlockType::Furnace
            | BlockType::Piston { .. }
            | BlockType::StickyPiston { .. } => 6.0,
            BlockType::Netherrack => 2.0,
            BlockType::SoulSand => 1.2,
            BlockType::Wood
            | BlockType::BirchWood
            | BlockType::Planks
            | BlockType::Bookshelf
            | BlockType::CraftingTable
            | BlockType::EnchantingTable
            | BlockType::BrewingStand
            | BlockType::Bed
            | BlockType::Chest
            | BlockType::Wool
            | BlockType::WoodDoor(_)
            | BlockType::Ladder => 3.0,
            BlockType::Glowstone => 1.5,
            BlockType::Glass => 0.3,
            BlockType::Dirt
            | BlockType::Grass
            | BlockType::Sand
            | BlockType::Gravel
            | BlockType::Farmland(_) => 0.8,
            BlockType::Snow | BlockType::Ice => 0.25,
            BlockType::Leaves
            | BlockType::BirchLeaves
            | BlockType::Torch
            | BlockType::Lever(_)
            | BlockType::StoneButton(_)
            | BlockType::RedstoneTorch(_)
            | BlockType::RedstoneRepeater { .. }
            | BlockType::RedFlower
            | BlockType::YellowFlower
            | BlockType::TallGrass
            | BlockType::DeadBush
            | BlockType::Sapling
            | BlockType::BirchSapling
            | BlockType::SugarCane
            | BlockType::NetherWart(_)
            | BlockType::RedstoneDust(_)
            | BlockType::Crops(_)
            | BlockType::Cactus => 0.15,
            BlockType::IronDoor(_) => 5.0,
            BlockType::Anvil => 8.0,
            BlockType::SilverfishSpawner => 25.0,
            BlockType::BlazeSpawner => 25.0,
            BlockType::ZombieSpawner => 25.0,
            BlockType::SkeletonSpawner => 25.0,
            BlockType::Tnt | BlockType::PrimedTnt(_) => 0.0,
        }
    }

    fn should_explosion_drop(
        &self,
        block: BlockType,
        pos: (i32, i32),
        center: (i32, i32),
        strength: f32,
        radius: i32,
    ) -> bool {
        // About one-third baseline drop chance, with strong central blasts more likely to vaporize.
        let (wx, wy) = pos;
        let (cx, cy) = center;
        let dx = wx - cx;
        let dy = wy - cy;
        let dist = ((dx * dx + dy * dy) as f64).sqrt() as f32;
        let norm = (dist / (radius as f32 + 0.5)).clamp(0.0, 1.0);
        let resistance = Self::explosion_block_resistance(block);
        let vapor = (strength * (1.0 - norm) / (resistance + 1.0)).clamp(0.0, 1.0);
        let drop_chance = (0.34 - vapor * 0.18).clamp(0.08, 0.34);

        let hash = (wx as i64)
            .wrapping_mul(73_856_093)
            .wrapping_add((wy as i64).wrapping_mul(19_349_663))
            .wrapping_add((cx as i64).wrapping_mul(83_492_791))
            .wrapping_add((cy as i64).wrapping_mul(2_654_435_761_i64))
            .wrapping_add(self.tick_counter as i64);
        let roll = (hash.rem_euclid(10_000) as f32) / 10_000.0;
        roll < drop_chance
    }

    pub fn trigger_explosion(
        &mut self,
        cx: i32,
        cy: i32,
        radius: i32,
        strength: f32,
        chain_fuse_ticks: u8,
    ) {
        if radius <= 0 {
            return;
        }
        self.recent_explosions.push((cx, cy, radius));

        let blast_limit = (radius as f64 + 0.5).powi(2);
        for dx in -radius..=radius {
            for dy in -radius..=radius {
                let dist_sq = (dx * dx + dy * dy) as f64;
                if dist_sq > blast_limit {
                    continue;
                }

                let wx = cx + dx;
                let wy = cy + dy;
                let block = self.get_block(wx, wy);
                if block == BlockType::Air {
                    continue;
                }
                if wx == cx && wy == cy {
                    self.set_block(wx, wy, BlockType::Air);
                    continue;
                }

                if matches!(
                    block,
                    BlockType::Bedrock | BlockType::Obsidian | BlockType::NetherPortal
                ) {
                    continue;
                }

                match block {
                    BlockType::Tnt => {
                        self.set_block(wx, wy, BlockType::PrimedTnt(chain_fuse_ticks));
                        continue;
                    }
                    BlockType::PrimedTnt(fuse) => {
                        if fuse > chain_fuse_ticks {
                            self.set_block(wx, wy, BlockType::PrimedTnt(chain_fuse_ticks));
                        }
                        continue;
                    }
                    _ => {}
                }

                let dist = dist_sq.sqrt() as f32;
                let pressure = strength * (1.0 - dist / (radius as f32 + 0.5));
                if pressure <= 0.0 {
                    continue;
                }
                let resistance = Self::explosion_block_resistance(block);
                if pressure * 8.0 <= resistance {
                    continue;
                }

                let drop_item =
                    self.should_explosion_drop(block, (wx, wy), (cx, cy), strength, radius);
                self.recent_explosion_block_losses
                    .push((wx, wy, block, drop_item));
                self.set_block(wx, wy, BlockType::Air);
            }
        }
    }

    fn has_adjacent_redstone_input(&self, x: i32, y: i32) -> bool {
        for (nx, ny) in [(x - 1, y), (x + 1, y), (x, y - 1), (x, y + 1)] {
            if self.redstone_input_power_from_to(nx, ny, x, y) > 0 {
                return true;
            }
        }
        false
    }

    fn redstone_input_power_from_to(&self, src_x: i32, src_y: i32, dst_x: i32, dst_y: i32) -> u8 {
        match self.get_block(src_x, src_y) {
            BlockType::Lever(true) => REDSTONE_MAX_POWER,
            BlockType::StoneButton(timer) if timer > 0 => REDSTONE_MAX_POWER,
            BlockType::RedstoneRepeater {
                powered: true,
                facing_right,
                ..
            } => {
                let out_x = if facing_right { src_x + 1 } else { src_x - 1 };
                if dst_x == out_x && dst_y == src_y {
                    REDSTONE_MAX_POWER
                } else {
                    0
                }
            }
            _ => 0,
        }
    }

    fn redstone_source_power(&self, block: BlockType) -> u8 {
        match block {
            BlockType::Lever(true) => REDSTONE_MAX_POWER,
            BlockType::StoneButton(timer) if timer > 0 => REDSTONE_MAX_POWER,
            BlockType::RedstoneTorch(true) => REDSTONE_MAX_POWER,
            BlockType::RedstoneRepeater { powered: true, .. } => REDSTONE_MAX_POWER,
            BlockType::RedstoneDust(level) => level.saturating_sub(1),
            _ => 0,
        }
    }

    fn insert_fluid_change(
        next_state: &mut std::collections::HashMap<(i32, i32), BlockType>,
        x: i32,
        y: i32,
        new_block: BlockType,
    ) -> bool {
        let current = next_state.get(&(x, y)).copied().unwrap_or(BlockType::Air);
        if current.is_solid() {
            return false;
        }
        if new_block.is_solid() || new_block == BlockType::Air {
            return next_state.insert((x, y), new_block) != Some(new_block);
        }

        let cur_l = match current {
            BlockType::Water(l) => l,
            BlockType::Lava(l) => l,
            _ => 0,
        };
        let new_l = match new_block {
            BlockType::Water(l) => l,
            BlockType::Lava(l) => l,
            _ => 0,
        };

        if new_l > cur_l {
            next_state.insert((x, y), new_block);
            return true;
        }
        false
    }

    fn biome_climate_noise(
        perlin: &Perlin,
        world_x: i32,
        macro_scale: f64,
        detail_scale: f64,
        offset: f64,
    ) -> f64 {
        let macro_noise = perlin.get([world_x as f64 * macro_scale, offset]);
        let detail_noise = perlin.get([world_x as f64 * detail_scale, offset + 197.0]);
        macro_noise * 0.82 + detail_noise * 0.18
    }

    fn biome_for_x(temp_perlin: &Perlin, moist_perlin: &Perlin, world_x: i32) -> BiomeType {
        let temp = Self::biome_climate_noise(
            temp_perlin,
            world_x,
            OVERWORLD_BIOME_CLIMATE_MACRO_SCALE,
            OVERWORLD_BIOME_CLIMATE_DETAIL_SCALE,
            0.0,
        );
        let moist = Self::biome_climate_noise(
            moist_perlin,
            world_x,
            OVERWORLD_BIOME_CLIMATE_MACRO_SCALE * 0.94,
            OVERWORLD_BIOME_CLIMATE_DETAIL_SCALE * 0.96,
            247.0,
        );
        let continental =
            temp_perlin.get([world_x as f64 * OVERWORLD_BIOME_CONTINENTAL_SCALE, 911.0]) * 0.78
                + temp_perlin.get([
                    world_x as f64 * OVERWORLD_BIOME_CONTINENTAL_DETAIL_SCALE,
                    467.0,
                ]) * 0.22;
        let river_band = (temp_perlin.get([world_x as f64 * OVERWORLD_BIOME_RIVER_SCALE, 123.0])
            * 0.65
            + moist_perlin.get([world_x as f64 * OVERWORLD_BIOME_RIVER_DETAIL_SCALE, 321.0])
                * 0.35)
            .abs();
        if continental < -0.5 {
            BiomeType::Ocean
        } else if river_band < 0.07 && continental < 0.08 {
            BiomeType::River
        } else if temp > 0.45 && moist > 0.22 {
            BiomeType::Jungle
        } else if moist > 0.45 && temp > -0.15 {
            BiomeType::Swamp
        } else if continental > 0.45 && temp < 0.3 {
            BiomeType::ExtremeHills
        } else if temp < -0.45 {
            BiomeType::Tundra
        } else if temp < -0.1 && moist > 0.15 {
            BiomeType::Taiga
        } else if temp > 0.35 && moist < -0.2 {
            BiomeType::Desert
        } else if moist > 0.25 {
            BiomeType::Forest
        } else {
            BiomeType::Plains
        }
    }

    pub fn get_biome(&self, world_x: i32) -> BiomeType {
        Self::biome_for_x(&self.temp_perlin, &self.moist_perlin, world_x)
    }

    pub fn is_nether_fortress_zone(&self, world_x: i32, world_y: i32) -> bool {
        if self.dimension != Dimension::Nether {
            return false;
        }
        if world_y <= 1 || world_y >= CHUNK_HEIGHT as i32 - 1 {
            return false;
        }
        let chunk_x = world_x.div_euclid(CHUNK_WIDTH as i32);
        let region_chunk = chunk_x.div_euclid(NETHER_FORTRESS_REGION_CHUNKS);
        let layout = Self::nether_fortress_layout(&self.perlin, region_chunk);
        if world_x < layout.left_x || world_x > layout.right_x {
            return false;
        }
        world_y >= layout.roof_y - 5 && world_y <= layout.base_y + 2
    }

    fn nether_fortress_layout(perlin: &Perlin, region_chunk: i32) -> NetherFortressLayout {
        let region_start_chunk = region_chunk * NETHER_FORTRESS_REGION_CHUNKS;
        let center_noise = perlin.get([region_chunk as f64 * 0.77, 601.0]);
        let center_offset = NETHER_FORTRESS_CENTER_MIN_OFFSET_CHUNKS
            + ((((center_noise + 1.0) * 0.5) * NETHER_FORTRESS_CENTER_OFFSET_SPAN_CHUNKS as f64)
                .floor() as i32)
                .clamp(0, NETHER_FORTRESS_CENTER_OFFSET_SPAN_CHUNKS - 1);
        let center_chunk = region_start_chunk + center_offset;
        let center_x = center_chunk * CHUNK_WIDTH as i32 + (CHUNK_WIDTH as i32 / 2);

        let base_noise = perlin.get([region_chunk as f64 * 0.61, 907.0]);
        let base_y = NETHER_FORTRESS_BASE_Y_MIN
            + ((((base_noise + 1.0) * 0.5) * NETHER_FORTRESS_BASE_Y_SPAN as f64).floor() as i32)
                .clamp(0, NETHER_FORTRESS_BASE_Y_SPAN - 1);
        let left_x = center_x - NETHER_FORTRESS_HALF_SPAN_BLOCKS;
        let right_x = center_x + NETHER_FORTRESS_HALF_SPAN_BLOCKS;

        NetherFortressLayout {
            center_x,
            left_x,
            right_x,
            base_y,
            roof_y: base_y - NETHER_FORTRESS_ROOF_HEIGHT,
            chest_left_x: center_x - NETHER_FORTRESS_CHEST_OFFSET_BLOCKS,
            chest_right_x: center_x + NETHER_FORTRESS_CHEST_OFFSET_BLOCKS,
            seed: (region_chunk as u32)
                .wrapping_mul(1_103_515_245)
                .wrapping_add(12_345),
        }
    }

    fn overworld_surface_y(perlin: &Perlin, world_x: i32) -> i32 {
        let n1 = perlin.get([world_x as f64 * 0.015, 0.0]);
        let n2 = perlin.get([world_x as f64 * 0.08, 100.0]) * 0.2;
        (32.0 + (n1 + n2) * 12.0) as i32
    }

    fn nearest_biome_in_radius<F>(
        temp_perlin: &Perlin,
        moist_perlin: &Perlin,
        world_x: i32,
        radius: i32,
        predicate: F,
    ) -> Option<(BiomeType, i32)>
    where
        F: Fn(BiomeType) -> bool,
    {
        for distance in 1..=radius {
            for sample_x in [world_x - distance, world_x + distance] {
                let biome = Self::biome_for_x(temp_perlin, moist_perlin, sample_x);
                if predicate(biome) {
                    return Some((biome, distance));
                }
            }
        }
        None
    }

    fn adjusted_overworld_surface_y(perlin: &Perlin, world_x: i32, biome: BiomeType) -> i32 {
        let base = Self::overworld_surface_y(perlin, world_x);
        let jagged = perlin.get([world_x as f64 * 0.06, 540.0]).abs();
        let wetland_noise = perlin.get([world_x as f64 * 0.028, 620.0]).abs();
        let channel_noise = perlin.get([world_x as f64 * 0.041, 880.0]).abs();
        let adjusted = match biome {
            // Water biomes should stay anchored near sea level instead of inheriting
            // full land-height noise, otherwise rivers and swamps become implausibly deep.
            BiomeType::Ocean => {
                OVERWORLD_SEA_LEVEL
                    + 4
                    + (wetland_noise * 5.0) as i32
                    + (channel_noise * 2.0) as i32
            }
            BiomeType::River => OVERWORLD_SEA_LEVEL + 2 + (channel_noise * 2.0) as i32,
            BiomeType::Swamp => OVERWORLD_SEA_LEVEL + 2 + (wetland_noise * 2.0) as i32,
            BiomeType::Jungle => base - 2 + (jagged * 2.0) as i32,
            BiomeType::ExtremeHills => base - 8 - (jagged * 10.0) as i32,
            _ => base,
        };
        adjusted.clamp(18, CHUNK_HEIGHT as i32 - 14)
    }

    fn blended_overworld_surface_y(
        perlin: &Perlin,
        temp_perlin: &Perlin,
        moist_perlin: &Perlin,
        world_x: i32,
        biome: BiomeType,
    ) -> i32 {
        let mut adjusted = Self::adjusted_overworld_surface_y(perlin, world_x, biome);
        let shore_noise = perlin.get([world_x as f64 * 0.09, 760.0]).abs();
        let shoreline_lip_noise = perlin.get([world_x as f64 * 0.034, 812.0]);

        if biome == BiomeType::Ocean {
            if let Some((_land_biome, distance)) = Self::nearest_biome_in_radius(
                temp_perlin,
                moist_perlin,
                world_x,
                SHORELINE_SHELF_RADIUS,
                |candidate| {
                    !matches!(
                        candidate,
                        BiomeType::Ocean | BiomeType::River | BiomeType::Swamp
                    )
                },
            ) {
                let strength = match distance {
                    1 => 0.85,
                    2 => 0.6,
                    3 => 0.35,
                    _ => 0.18,
                };
                let shelf_y = OVERWORLD_SEA_LEVEL + 2 + (shore_noise * 3.0) as i32;
                let blended = ((adjusted as f64) * (1.0 - strength) + (shelf_y as f64) * strength)
                    .round() as i32;
                adjusted = adjusted.min(blended);
            }
        } else if !matches!(biome, BiomeType::River | BiomeType::Swamp)
            && let Some((water_biome, distance)) = Self::nearest_biome_in_radius(
                temp_perlin,
                moist_perlin,
                world_x,
                SHORELINE_BLEND_RADIUS,
                Self::is_surface_water_biome,
            )
        {
            let strength = match distance {
                1 => 0.82,
                2 => 0.58,
                _ => 0.32,
            };
            let shore_y = match water_biome {
                BiomeType::Ocean => OVERWORLD_SEA_LEVEL - 2 + (shore_noise * 2.0) as i32,
                BiomeType::River => OVERWORLD_SEA_LEVEL - 1 + (shore_noise * 2.0) as i32,
                BiomeType::Swamp => OVERWORLD_SEA_LEVEL - 1 + shore_noise.round() as i32,
                _ => OVERWORLD_SEA_LEVEL - 1,
            };
            let blended =
                ((adjusted as f64) * (1.0 - strength) + (shore_y as f64) * strength).round() as i32;
            adjusted = adjusted.max(blended);
            let accessible_bank_y = match distance {
                1 => {
                    if shoreline_lip_noise > 0.12 || water_biome == BiomeType::Swamp {
                        OVERWORLD_SEA_LEVEL
                    } else {
                        OVERWORLD_SEA_LEVEL - 1
                    }
                }
                2 => OVERWORLD_SEA_LEVEL - 1,
                _ => OVERWORLD_SEA_LEVEL - 2,
            };
            adjusted = adjusted.max(accessible_bank_y);
            adjusted = adjusted.min(Self::shoreline_land_water_depth_limit(
                water_biome,
                distance,
            ));
        }

        adjusted.clamp(18, CHUNK_HEIGHT as i32 - 14)
    }

    fn is_surface_water_biome(biome: BiomeType) -> bool {
        matches!(
            biome,
            BiomeType::Ocean | BiomeType::River | BiomeType::Swamp
        )
    }

    fn shoreline_land_water_depth_limit(water_biome: BiomeType, distance: i32) -> i32 {
        match water_biome {
            BiomeType::Ocean => {
                if distance <= 1 {
                    OVERWORLD_SEA_LEVEL + 2
                } else {
                    OVERWORLD_SEA_LEVEL
                }
            }
            BiomeType::River | BiomeType::Swamp => {
                if distance <= 1 {
                    OVERWORLD_SEA_LEVEL + 1
                } else {
                    OVERWORLD_SEA_LEVEL
                }
            }
            _ => OVERWORLD_SEA_LEVEL,
        }
    }

    fn is_beach_column(
        temp_perlin: &Perlin,
        moist_perlin: &Perlin,
        world_x: i32,
        biome: BiomeType,
        surface_y: i32,
    ) -> bool {
        if Self::is_surface_water_biome(biome) {
            return false;
        }
        if !(OVERWORLD_SEA_LEVEL - 3..=OVERWORLD_SEA_LEVEL + 4).contains(&surface_y) {
            return false;
        }
        Self::nearest_biome_in_radius(temp_perlin, moist_perlin, world_x, 2, |candidate| {
            matches!(candidate, BiomeType::Ocean | BiomeType::River)
        })
        .is_some()
    }

    fn column_has_surface_water(
        _perlin: &Perlin,
        temp_perlin: &Perlin,
        moist_perlin: &Perlin,
        world_x: i32,
        biome: BiomeType,
        surface_y: i32,
    ) -> bool {
        if surface_y <= OVERWORLD_SEA_LEVEL {
            return false;
        }
        if Self::is_surface_water_biome(biome) {
            return true;
        }
        let Some((water_biome, distance)) = Self::nearest_biome_in_radius(
            temp_perlin,
            moist_perlin,
            world_x,
            SHORELINE_BLEND_RADIUS,
            Self::is_surface_water_biome,
        ) else {
            return false;
        };

        if surface_y > Self::shoreline_land_water_depth_limit(water_biome, distance) {
            return false;
        }

        distance <= 1
    }

    fn column_touches_surface_water(
        perlin: &Perlin,
        temp_perlin: &Perlin,
        moist_perlin: &Perlin,
        world_x: i32,
        biome: BiomeType,
        surface_y: i32,
    ) -> bool {
        if Self::column_has_surface_water(
            perlin,
            temp_perlin,
            moist_perlin,
            world_x,
            biome,
            surface_y,
        ) {
            return true;
        }

        for sample_x in [world_x - 1, world_x + 1] {
            let sample_biome = Self::biome_for_x(temp_perlin, moist_perlin, sample_x);
            let sample_surface_y = Self::blended_overworld_surface_y(
                perlin,
                temp_perlin,
                moist_perlin,
                sample_x,
                sample_biome,
            );
            if Self::column_has_surface_water(
                perlin,
                temp_perlin,
                moist_perlin,
                sample_x,
                sample_biome,
                sample_surface_y,
            ) {
                return true;
            }
        }

        false
    }

    fn overworld_biome_cave_bias(biome: BiomeType) -> f64 {
        match biome {
            BiomeType::ExtremeHills => 0.030,
            BiomeType::Jungle => 0.022,
            BiomeType::Forest => 0.014,
            BiomeType::Taiga => 0.010,
            BiomeType::Ocean => 0.006,
            BiomeType::River => 0.002,
            BiomeType::Tundra => -0.004,
            BiomeType::Swamp => -0.008,
            BiomeType::Plains => -0.012,
            BiomeType::Desert => -0.016,
        }
    }

    fn overworld_chamber_carves(
        cave_perlin: &Perlin,
        world_x: i32,
        current_y: i32,
        biome: BiomeType,
        surface_y: i32,
    ) -> bool {
        let cave_depth = current_y - surface_y;
        if cave_depth < OVERWORLD_CHAMBER_MIN_SURFACE_DEPTH {
            return false;
        }

        let biome_bias = Self::overworld_biome_cave_bias(biome);
        let depth_bonus =
            ((cave_depth - OVERWORLD_CHAMBER_MIN_SURFACE_DEPTH) as f64 * 0.0035).min(0.08);
        let activation_threshold = 0.68 - biome_bias * 2.8 - depth_bonus;
        let roughness = cave_perlin.get([world_x as f64 * 0.085, current_y as f64 * 0.085, 660.0])
            * OVERWORLD_CHAMBER_EDGE_ROUGHNESS;

        let chamber_cell_x = world_x.div_euclid(OVERWORLD_CHAMBER_CELL_WIDTH);
        let chamber_cell_y = current_y.div_euclid(OVERWORLD_CHAMBER_CELL_HEIGHT);
        for sample_cell_x in (chamber_cell_x - 1)..=(chamber_cell_x + 1) {
            for sample_cell_y in (chamber_cell_y - 1)..=(chamber_cell_y + 1) {
                let activation = cave_perlin.get([
                    sample_cell_x as f64 * 0.73,
                    sample_cell_y as f64 * 0.79,
                    610.0,
                ]);
                if activation < activation_threshold {
                    continue;
                }

                let center_x_offset = ((((cave_perlin.get([
                    sample_cell_x as f64 * 0.69,
                    sample_cell_y as f64 * 0.83,
                    620.0,
                ]) + 1.0)
                    * 0.5)
                    * (OVERWORLD_CHAMBER_CELL_WIDTH as f64 - 1.0))
                    .round() as i32)
                    .clamp(0, OVERWORLD_CHAMBER_CELL_WIDTH - 1);
                let center_y_offset = ((((cave_perlin.get([
                    sample_cell_x as f64 * 0.77,
                    sample_cell_y as f64 * 0.71,
                    630.0,
                ]) + 1.0)
                    * 0.5)
                    * (OVERWORLD_CHAMBER_CELL_HEIGHT as f64 - 1.0))
                    .round() as i32)
                    .clamp(0, OVERWORLD_CHAMBER_CELL_HEIGHT - 1);
                let radius_x = OVERWORLD_CHAMBER_MIN_RADIUS_X
                    + ((((cave_perlin.get([
                        sample_cell_x as f64 * 0.81,
                        sample_cell_y as f64 * 0.67,
                        640.0,
                    ]) + 1.0)
                        * 0.5)
                        * OVERWORLD_CHAMBER_RADIUS_X_SPAN as f64)
                        .round() as i32)
                        .clamp(0, OVERWORLD_CHAMBER_RADIUS_X_SPAN);
                let radius_y = OVERWORLD_CHAMBER_MIN_RADIUS_Y
                    + ((((cave_perlin.get([
                        sample_cell_x as f64 * 0.75,
                        sample_cell_y as f64 * 0.87,
                        650.0,
                    ]) + 1.0)
                        * 0.5)
                        * OVERWORLD_CHAMBER_RADIUS_Y_SPAN as f64)
                        .round() as i32)
                        .clamp(0, OVERWORLD_CHAMBER_RADIUS_Y_SPAN);

                let center_x = sample_cell_x * OVERWORLD_CHAMBER_CELL_WIDTH + center_x_offset;
                let center_y = sample_cell_y * OVERWORLD_CHAMBER_CELL_HEIGHT + center_y_offset;
                let dx = (world_x - center_x) as f64 / radius_x as f64;
                let dy = (current_y - center_y) as f64 / radius_y as f64;
                if dx * dx + dy * dy <= 1.0 + roughness {
                    return true;
                }
            }
        }

        false
    }

    fn overworld_ravine_carves(
        cave_perlin: &Perlin,
        world_x: i32,
        current_y: i32,
        biome: BiomeType,
        surface_y: i32,
    ) -> bool {
        let cave_depth = current_y - surface_y;
        if cave_depth < OVERWORLD_RAVINE_MIN_SURFACE_DEPTH {
            return false;
        }

        let biome_bias = Self::overworld_biome_cave_bias(biome);
        let ravine_cell_x = world_x.div_euclid(OVERWORLD_RAVINE_CELL_WIDTH);
        for sample_cell_x in (ravine_cell_x - 1)..=(ravine_cell_x + 1) {
            let activation = cave_perlin.get([sample_cell_x as f64 * 0.47, 700.0]);
            if activation < 0.58 - biome_bias * 1.8 {
                continue;
            }

            let center_offset = ((((cave_perlin.get([sample_cell_x as f64 * 0.63, 710.0]) + 1.0)
                * 0.5)
                * (OVERWORLD_RAVINE_CELL_WIDTH as f64 - 1.0))
                .round() as i32)
                .clamp(0, OVERWORLD_RAVINE_CELL_WIDTH - 1);
            let top_depth = OVERWORLD_RAVINE_MIN_SURFACE_DEPTH
                + ((((cave_perlin.get([sample_cell_x as f64 * 0.71, 715.0]) + 1.0) * 0.5) * 4.0)
                    .round() as i32)
                    .clamp(0, 4);
            if cave_depth < top_depth {
                continue;
            }

            let depth_factor = ((cave_depth - top_depth) as f64 / 40.0).clamp(0.0, 1.0);
            let meander =
                cave_perlin.get([sample_cell_x as f64 * 0.59, current_y as f64 * 0.028, 720.0])
                    * OVERWORLD_RAVINE_MEANDER_AMPLITUDE
                    * (0.55 + depth_factor * 0.85);
            let width_noise =
                ((cave_perlin.get([sample_cell_x as f64 * 0.53, current_y as f64 * 0.024, 730.0])
                    + 1.0)
                    * 0.5)
                    .clamp(0.0, 1.0);
            let half_width = (OVERWORLD_RAVINE_MIN_HALF_WIDTH
                + width_noise * OVERWORLD_RAVINE_HALF_WIDTH_SPAN
                + depth_factor * 2.1
                + biome_bias * 16.0)
                .max(1.2);

            let center_x = sample_cell_x * OVERWORLD_RAVINE_CELL_WIDTH + center_offset;
            let dx_norm = (world_x as f64 - (center_x as f64 + meander)).abs() / half_width;
            if dx_norm > 1.0 {
                continue;
            }

            let ledge_noise =
                cave_perlin.get([world_x as f64 * 0.11, current_y as f64 * 0.18, 740.0]);
            if dx_norm > OVERWORLD_RAVINE_LEDGE_EDGE_START
                && ledge_noise > OVERWORLD_RAVINE_LEDGE_NOISE_THRESHOLD - depth_factor * 0.08
            {
                continue;
            }

            let rib_noise =
                cave_perlin.get([world_x as f64 * 0.08, current_y as f64 * 0.10, 750.0]);
            if depth_factor > 0.15
                && dx_norm > 0.35
                && rib_noise > OVERWORLD_RAVINE_RIB_NOISE_THRESHOLD
            {
                continue;
            }

            return true;
        }

        false
    }

    fn overworld_ore_blob_score(
        perlin: &Perlin,
        world_x: i32,
        current_y: i32,
        coarse_scale: f64,
        detail_scale: f64,
        coarse_seed: f64,
        detail_seed: f64,
    ) -> f64 {
        let coarse = perlin.get([
            world_x as f64 * coarse_scale,
            current_y as f64 * coarse_scale,
            coarse_seed,
        ]);
        let detail = perlin.get([
            world_x as f64 * detail_scale,
            current_y as f64 * detail_scale,
            detail_seed,
        ]);
        coarse * 0.72 + detail * 0.28
    }

    fn overworld_ore_breakup_score(
        perlin: &Perlin,
        world_x: i32,
        current_y: i32,
        scale: f64,
        seed: f64,
    ) -> f64 {
        perlin
            .get([world_x as f64 * scale, current_y as f64 * scale, seed])
            .abs()
    }

    fn overworld_ore_density_bonus(
        current_y: i32,
        start_y: i32,
        rich_y: i32,
        max_bonus: f64,
    ) -> f64 {
        if rich_y <= start_y {
            return 0.0;
        }
        (((current_y - start_y) as f64) / (rich_y - start_y) as f64).clamp(0.0, 1.0) * max_bonus
    }

    fn overworld_ore_block(
        perlin: &Perlin,
        world_x: i32,
        current_y: i32,
        exposed_to_cave: bool,
    ) -> Option<BlockType> {
        let iron_depth_t = ((current_y - OVERWORLD_IRON_ORE_MIN_Y) as f64
            / (OVERWORLD_IRON_ORE_RICH_Y - OVERWORLD_IRON_ORE_MIN_Y) as f64)
            .clamp(0.0, 1.0);
        let redstone_depth_t = ((current_y - REDSTONE_ORE_MIN_Y) as f64
            / (REDSTONE_ORE_RICH_Y - REDSTONE_ORE_MIN_Y) as f64)
            .clamp(0.0, 1.0);
        let exposure_bonus = if exposed_to_cave { 0.06 } else { 0.0 };
        let common_exposure_bonus = if exposed_to_cave { 0.08 } else { 0.0 };
        let iron_exposure_bonus = if exposed_to_cave { 0.08 } else { 0.02 };
        let diamond_exposure_bonus = if exposed_to_cave { 0.13 } else { 0.0 };
        let diamond_score =
            Self::overworld_ore_blob_score(perlin, world_x, current_y, 0.048, 0.16, 30.0, 31.0)
                + Self::overworld_ore_density_bonus(current_y, 108, 150, 0.085)
                + diamond_exposure_bonus
                - Self::overworld_ore_breakup_score(perlin, world_x, current_y, 0.13, 130.0) * 0.07;
        let gold_score =
            Self::overworld_ore_blob_score(perlin, world_x, current_y, 0.047, 0.15, 20.0, 21.0)
                + Self::overworld_ore_density_bonus(current_y, 86, 144, 0.06)
                + exposure_bonus
                - Self::overworld_ore_breakup_score(perlin, world_x, current_y, 0.12, 120.0) * 0.06;
        let redstone_score =
            Self::overworld_ore_blob_score(perlin, world_x, current_y, 0.058, 0.19, 15.0, 16.0)
                + exposure_bonus
                - Self::overworld_ore_breakup_score(perlin, world_x, current_y, 0.14, 115.0) * 0.07;
        let iron_score =
            Self::overworld_ore_blob_score(perlin, world_x, current_y, 0.045, 0.15, 10.0, 11.0)
                + Self::overworld_ore_density_bonus(
                    current_y,
                    OVERWORLD_IRON_ORE_MIN_Y,
                    OVERWORLD_IRON_ORE_RICH_Y,
                    0.14,
                )
                + iron_exposure_bonus
                - Self::overworld_ore_breakup_score(perlin, world_x, current_y, 0.16, 110.0) * 0.16;
        let iron_gate = perlin.get([world_x as f64 * 0.31, current_y as f64 * 0.31, 112.0]);
        let coal_score =
            Self::overworld_ore_blob_score(perlin, world_x, current_y, 0.05, 0.17, 0.0, 1.0)
                + Self::overworld_ore_density_bonus(current_y, 36, 98, 0.08)
                + common_exposure_bonus
                - Self::overworld_ore_breakup_score(perlin, world_x, current_y, 0.12, 100.0) * 0.05;

        if current_y > 104 && diamond_score > 0.65 {
            Some(BlockType::DiamondOre)
        } else if current_y > 82 && gold_score > 0.65 {
            Some(BlockType::GoldOre)
        } else if current_y >= REDSTONE_ORE_MIN_Y
            && redstone_score > (0.68 - redstone_depth_t * 0.12)
        {
            Some(BlockType::RedstoneOre)
        } else if current_y >= OVERWORLD_IRON_ORE_MIN_Y
            && iron_score > (0.53 - iron_depth_t * 0.05)
            && iron_gate > 0.0
        {
            Some(BlockType::IronOre)
        } else if coal_score > 0.49 {
            Some(BlockType::CoalOre)
        } else {
            None
        }
    }

    fn refine_overworld_exposed_ores(
        chunk: &mut Chunk,
        chunk_x: i32,
        perlin: &Perlin,
        surface_y_by_x: &[i32; CHUNK_WIDTH],
        surface_water_by_x: &[bool; CHUNK_WIDTH],
    ) {
        let c_start = chunk_x * CHUNK_WIDTH as i32;

        for x in 1..(CHUNK_WIDTH - 1) {
            if surface_water_by_x[x] {
                continue;
            }

            let min_depth_y = (surface_y_by_x[x] + 7).max(OVERWORLD_VISIBLE_ORE_MIN_Y) as usize;
            for y in min_depth_y..(CHUNK_HEIGHT - 8) {
                if chunk.get_block(x, y) != BlockType::Stone {
                    continue;
                }

                let cave_exposed = [(x - 1, y), (x + 1, y), (x, y - 1), (x, y + 1)]
                    .into_iter()
                    .any(|(nx, ny)| chunk.get_block(nx, ny) == BlockType::Air);
                if !cave_exposed {
                    continue;
                }

                let world_x = c_start + x as i32;
                let current_y = y as i32;
                if let Some(ore_block) = Self::overworld_ore_block(perlin, world_x, current_y, true)
                {
                    chunk.set_block(x, y, ore_block);
                }
            }
        }
    }

    fn refine_overworld_ore_runs(chunk: &mut Chunk) {
        const MAX_IRON_RUN: usize = 6;

        for y in OVERWORLD_VISIBLE_ORE_MIN_Y as usize..(CHUNK_HEIGHT - 8) {
            let mut x = 0usize;
            while x < CHUNK_WIDTH {
                if chunk.get_block(x, y) != BlockType::IronOre {
                    x += 1;
                    continue;
                }

                let run_start = x;
                while x < CHUNK_WIDTH && chunk.get_block(x, y) == BlockType::IronOre {
                    x += 1;
                }

                let run_len = x - run_start;
                if run_len <= MAX_IRON_RUN {
                    continue;
                }

                let mut carve_x = run_start + (MAX_IRON_RUN / 2);
                while carve_x < x {
                    chunk.set_block(carve_x, y, BlockType::Stone);
                    carve_x += MAX_IRON_RUN;
                }
            }
        }
    }

    fn is_overworld_dry_cave_air(
        chunk: &Chunk,
        surface_y_by_x: &[i32; CHUNK_WIDTH],
        surface_water_by_x: &[bool; CHUNK_WIDTH],
        x: usize,
        y: usize,
    ) -> bool {
        if x >= CHUNK_WIDTH || y >= CHUNK_HEIGHT || surface_water_by_x[x] {
            return false;
        }

        y as i32 > surface_y_by_x[x] + OVERWORLD_CAVE_MIN_SURFACE_DEPTH
            && chunk.get_block(x, y) == BlockType::Air
    }

    fn is_overworld_walkable_cave_cell(
        chunk: &Chunk,
        surface_y_by_x: &[i32; CHUNK_WIDTH],
        surface_water_by_x: &[bool; CHUNK_WIDTH],
        x: usize,
        y: usize,
    ) -> bool {
        y > 0
            && y + 1 < CHUNK_HEIGHT
            && Self::is_overworld_dry_cave_air(chunk, surface_y_by_x, surface_water_by_x, x, y)
            && Self::is_overworld_dry_cave_air(chunk, surface_y_by_x, surface_water_by_x, x, y - 1)
            && chunk.get_block(x, y + 1).is_solid()
    }

    fn overworld_walkable_cave_local_score(
        chunk: &Chunk,
        surface_y_by_x: &[i32; CHUNK_WIDTH],
        surface_water_by_x: &[bool; CHUNK_WIDTH],
        x: usize,
        y: usize,
    ) -> usize {
        let min_x = x.saturating_sub(2);
        let max_x = (x + 2).min(CHUNK_WIDTH - 1);
        let min_y = y.saturating_sub(1);
        let max_y = (y + 1).min(CHUNK_HEIGHT - 2);
        let mut score = 0usize;

        for sample_x in min_x..=max_x {
            for sample_y in min_y..=max_y {
                if Self::is_overworld_walkable_cave_cell(
                    chunk,
                    surface_y_by_x,
                    surface_water_by_x,
                    sample_x,
                    sample_y,
                ) {
                    score += 1;
                }
            }
        }

        score
    }

    fn overworld_dry_cave_local_air_score(
        chunk: &Chunk,
        surface_y_by_x: &[i32; CHUNK_WIDTH],
        surface_water_by_x: &[bool; CHUNK_WIDTH],
        x: usize,
        y: usize,
    ) -> usize {
        let min_x = x.saturating_sub(2);
        let max_x = (x + 2).min(CHUNK_WIDTH - 1);
        let min_y = y.saturating_sub(2);
        let max_y = (y + 1).min(CHUNK_HEIGHT - 1);
        let mut score = 0usize;

        for sample_x in min_x..=max_x {
            for sample_y in min_y..=max_y {
                if Self::is_overworld_dry_cave_air(
                    chunk,
                    surface_y_by_x,
                    surface_water_by_x,
                    sample_x,
                    sample_y,
                ) {
                    score += 1;
                }
            }
        }

        score
    }

    fn refine_overworld_cave_pacing(
        chunk: &mut Chunk,
        surface_y_by_x: &[i32; CHUNK_WIDTH],
        surface_water_by_x: &[bool; CHUNK_WIDTH],
    ) {
        for _ in 0..OVERWORLD_CAVE_DEAD_END_TRIM_PASSES {
            let mut fill_columns = Vec::new();
            for x in 2..(CHUNK_WIDTH - 2) {
                for y in 2..(CHUNK_HEIGHT - 2) {
                    if !Self::is_overworld_walkable_cave_cell(
                        chunk,
                        surface_y_by_x,
                        surface_water_by_x,
                        x,
                        y,
                    ) {
                        continue;
                    }

                    let horizontal_neighbors =
                        usize::from(Self::is_overworld_walkable_cave_cell(
                            chunk,
                            surface_y_by_x,
                            surface_water_by_x,
                            x - 1,
                            y,
                        )) + usize::from(Self::is_overworld_walkable_cave_cell(
                            chunk,
                            surface_y_by_x,
                            surface_water_by_x,
                            x + 1,
                            y,
                        ));
                    let local_score = Self::overworld_dry_cave_local_air_score(
                        chunk,
                        surface_y_by_x,
                        surface_water_by_x,
                        x,
                        y,
                    );
                    if horizontal_neighbors <= 1
                        && local_score <= OVERWORLD_CAVE_DEAD_END_MAX_LOCAL_AIR_SCORE
                    {
                        fill_columns.push((x, y));
                    }
                }
            }

            if fill_columns.is_empty() {
                break;
            }

            for (x, y) in fill_columns {
                if Self::is_overworld_walkable_cave_cell(
                    chunk,
                    surface_y_by_x,
                    surface_water_by_x,
                    x,
                    y,
                ) {
                    chunk.set_block(x, y, BlockType::Stone);
                    chunk.set_block(x, y - 1, BlockType::Stone);
                }
            }
        }

        let mut carve_columns = HashSet::new();

        for y in 2..(CHUNK_HEIGHT - 2) {
            for left_x in 2..(CHUNK_WIDTH - 2) {
                if !Self::is_overworld_walkable_cave_cell(
                    chunk,
                    surface_y_by_x,
                    surface_water_by_x,
                    left_x,
                    y,
                ) {
                    continue;
                }
                if Self::overworld_walkable_cave_local_score(
                    chunk,
                    surface_y_by_x,
                    surface_water_by_x,
                    left_x,
                    y,
                ) < OVERWORLD_CAVE_CONNECTOR_MIN_OPEN_SCORE
                {
                    continue;
                }

                for wall_width in 1..=OVERWORLD_CAVE_BRIDGE_MAX_WALL_WIDTH {
                    let right_x = left_x + wall_width + 1;
                    if right_x >= CHUNK_WIDTH - 1 {
                        break;
                    }
                    if !Self::is_overworld_walkable_cave_cell(
                        chunk,
                        surface_y_by_x,
                        surface_water_by_x,
                        right_x,
                        y,
                    ) {
                        continue;
                    }
                    if Self::overworld_walkable_cave_local_score(
                        chunk,
                        surface_y_by_x,
                        surface_water_by_x,
                        right_x,
                        y,
                    ) < OVERWORLD_CAVE_CONNECTOR_MIN_OPEN_SCORE
                    {
                        continue;
                    }

                    let mut valid = true;
                    let mut introduces_new_carve = false;
                    for fill_x in (left_x + 1)..right_x {
                        if surface_water_by_x[fill_x]
                            || (y as i32)
                                <= surface_y_by_x[fill_x] + OVERWORLD_CAVE_MIN_SURFACE_DEPTH
                            || !chunk.get_block(fill_x, y + 1).is_solid()
                        {
                            valid = false;
                            break;
                        }
                        for carve_y in [y - 1, y] {
                            let block = chunk.get_block(fill_x, carve_y);
                            if block.is_fluid() {
                                valid = false;
                                break;
                            }
                            if block != BlockType::Air {
                                introduces_new_carve = true;
                            }
                        }
                        if !valid {
                            break;
                        }
                    }

                    if valid && introduces_new_carve {
                        for fill_x in (left_x + 1)..right_x {
                            carve_columns.insert((fill_x, y));
                        }
                        break;
                    }
                }
            }
        }

        for x in 2..(CHUNK_WIDTH - 2) {
            for y in 2..(CHUNK_HEIGHT - OVERWORLD_CAVE_STAIR_MAX_DROP - 1) {
                if !Self::is_overworld_walkable_cave_cell(
                    chunk,
                    surface_y_by_x,
                    surface_water_by_x,
                    x,
                    y,
                ) {
                    continue;
                }
                if Self::overworld_walkable_cave_local_score(
                    chunk,
                    surface_y_by_x,
                    surface_water_by_x,
                    x,
                    y,
                ) < OVERWORLD_CAVE_CONNECTOR_MIN_OPEN_SCORE
                {
                    continue;
                }

                let mut carved_link = false;
                for dir in [-1isize, 1] {
                    if carved_link {
                        break;
                    }
                    for drop in OVERWORLD_CAVE_STAIR_MIN_DROP..=OVERWORLD_CAVE_STAIR_MAX_DROP {
                        let end_x = x as isize + dir * drop as isize;
                        if end_x < 2 || end_x >= (CHUNK_WIDTH - 2) as isize {
                            continue;
                        }

                        let end_x = end_x as usize;
                        let end_y = y + drop;
                        if !Self::is_overworld_walkable_cave_cell(
                            chunk,
                            surface_y_by_x,
                            surface_water_by_x,
                            end_x,
                            end_y,
                        ) {
                            continue;
                        }
                        if Self::overworld_walkable_cave_local_score(
                            chunk,
                            surface_y_by_x,
                            surface_water_by_x,
                            end_x,
                            end_y,
                        ) < OVERWORLD_CAVE_CONNECTOR_MIN_OPEN_SCORE
                        {
                            continue;
                        }

                        let mut valid = true;
                        let mut introduces_new_carve = false;
                        for step in 1..drop {
                            let path_x = (x as isize + dir * step as isize) as usize;
                            let path_y = y + step;
                            if surface_water_by_x[path_x]
                                || (path_y as i32)
                                    <= surface_y_by_x[path_x] + OVERWORLD_CAVE_MIN_SURFACE_DEPTH
                                || !chunk.get_block(path_x, path_y + 1).is_solid()
                            {
                                valid = false;
                                break;
                            }

                            for carve_y in [path_y - 1, path_y] {
                                let block = chunk.get_block(path_x, carve_y);
                                if block.is_fluid() {
                                    valid = false;
                                    break;
                                }
                                if block != BlockType::Air {
                                    introduces_new_carve = true;
                                }
                            }
                            if !valid {
                                break;
                            }
                        }

                        if valid && introduces_new_carve {
                            for step in 1..drop {
                                let path_x = (x as isize + dir * step as isize) as usize;
                                let path_y = y + step;
                                carve_columns.insert((path_x, path_y));
                            }
                            carved_link = true;
                            break;
                        }
                    }
                }
            }
        }

        for (x, y) in carve_columns {
            if !chunk.get_block(x, y).is_fluid() {
                chunk.set_block(x, y, BlockType::Air);
            }
            if !chunk.get_block(x, y - 1).is_fluid() {
                chunk.set_block(x, y - 1, BlockType::Air);
            }
        }
    }

    fn dress_overworld_caves(
        chunk: &mut Chunk,
        chunk_x: i32,
        perlin: &Perlin,
        cave_perlin: &Perlin,
        surface_y_by_x: &[i32; CHUNK_WIDTH],
        surface_water_by_x: &[bool; CHUNK_WIDTH],
    ) {
        let c_start = chunk_x * CHUNK_WIDTH as i32;
        let mut floor_updates = Vec::new();
        let mut pool_updates = Vec::new();

        for x in 2..(CHUNK_WIDTH - 2) {
            let world_x = c_start + x as i32;
            let surface_y = surface_y_by_x[x];
            if surface_water_by_x[x] {
                continue;
            }

            let min_y = (surface_y + OVERWORLD_CAVE_MIN_SURFACE_DEPTH + 1)
                .clamp(2, CHUNK_HEIGHT as i32 - 4);
            for current_y in min_y..=(CHUNK_HEIGHT as i32 - 4) {
                let y = current_y as usize;
                if chunk.get_block(x, y) != BlockType::Air {
                    continue;
                }
                if chunk.get_block(x, y + 1) == BlockType::Air {
                    continue;
                }

                let cave_depth = current_y - surface_y;
                let floor_block = chunk.get_block(x, y + 1);
                if cave_depth >= OVERWORLD_CAVE_FLOOR_VARIATION_MIN_DEPTH
                    && floor_block == BlockType::Stone
                {
                    let floor_noise =
                        perlin.get([world_x as f64 * 0.12, (current_y + 1) as f64 * 0.12, 520.0])
                            + cave_perlin.get([
                                world_x as f64 * 0.08,
                                current_y as f64 * 0.09,
                                530.0,
                            ]) * 0.35;
                    if floor_noise > 0.76 {
                        floor_updates.push((x, y + 1, BlockType::Gravel));
                    } else if floor_noise > 0.48 {
                        floor_updates.push((x, y + 1, BlockType::Dirt));
                    }
                }

                if cave_depth < OVERWORLD_CAVE_POOL_MIN_DEPTH
                    || !(4..(CHUNK_WIDTH - 4)).contains(&x)
                    || chunk.get_block(x - 1, y) != BlockType::Air
                    || chunk.get_block(x + 1, y) != BlockType::Air
                {
                    continue;
                }
                if !(chunk.get_block(x - 1, y + 1).is_solid()
                    && chunk.get_block(x, y + 1).is_solid()
                    && chunk.get_block(x + 1, y + 1).is_solid())
                {
                    continue;
                }

                let left_wall = (2..=3).any(|offset| {
                    chunk.get_block(x - offset, y).is_solid()
                        || chunk.get_block(x - offset, y + 1).is_solid()
                });
                let right_wall = (2..=3).any(|offset| {
                    chunk.get_block(x + offset, y).is_solid()
                        || chunk.get_block(x + offset, y + 1).is_solid()
                });
                if !left_wall || !right_wall {
                    continue;
                }

                let pool_gate =
                    cave_perlin.get([world_x as f64 * 0.09, current_y as f64 * 0.07, 770.0]);
                if pool_gate <= 0.74 {
                    continue;
                }

                let fluid = if cave_depth >= OVERWORLD_CAVE_POOL_LAVA_DEPTH || current_y > 102 {
                    BlockType::Lava(8)
                } else {
                    BlockType::Water(8)
                };
                for pool_x in (x - 1)..=(x + 1) {
                    pool_updates.push((pool_x, y, fluid));
                }
            }
        }

        for (x, y, block) in floor_updates {
            if chunk.get_block(x, y) == BlockType::Stone {
                chunk.set_block(x, y, block);
            }
        }
        for (x, y, block) in pool_updates {
            if chunk.get_block(x, y) == BlockType::Air {
                chunk.set_block(x, y, block);
            }
        }
    }

    fn fill_dungeon_chest_loot(chunk: &mut Chunk, local_x: usize, y: usize, seed: u32) {
        let Some(chest) = chunk.ensure_chest_inventory(local_x, y, 27) else {
            return;
        };

        let mut state = seed;
        let mut next = || {
            state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            state
        };

        let coal = 2 + (next() % 4);
        chest.add_item(ItemType::Coal, coal);
        chest.add_item(ItemType::Torch, 3 + (next() % 5));

        if next() % 100 < 78 {
            chest.add_item(ItemType::Bread, 1 + (next() % 3));
        }
        if next() % 100 < 62 {
            chest.add_item(ItemType::IronIngot, 1 + (next() % 3));
        }
        if next() % 100 < 20 {
            chest.add_item(ItemType::Bucket, 1);
        }
        if next() % 100 < 18 {
            chest.add_item(ItemType::IronPickaxe, 1);
        }
        if next() % 100 < 56 {
            chest.add_item(ItemType::RedstoneDust, 2 + (next() % 4));
        }
        if next() % 100 < 40 {
            chest.add_item(ItemType::Bone, 1 + (next() % 4));
        }
        if next() % 100 < 40 {
            chest.add_item(ItemType::String, 1 + (next() % 4));
        }
        if next() % 100 < 32 {
            chest.add_item(ItemType::Gunpowder, 1 + (next() % 3));
        }
        if next() % 100 < 20 {
            chest.add_item(ItemType::Arrow, 3 + (next() % 5));
        }
        if next() % 100 < 7 {
            chest.add_item(ItemType::Diamond, 1);
        }
    }

    fn fill_stronghold_chest_loot(
        chunk: &mut Chunk,
        local_x: usize,
        y: usize,
        seed: u32,
        utility_bias: bool,
    ) {
        let Some(chest) = chunk.ensure_chest_inventory(local_x, y, 27) else {
            return;
        };

        let mut state = seed;
        let mut next = || {
            state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            state
        };

        chest.add_item(ItemType::Torch, 4 + (next() % 5));
        chest.add_item(ItemType::Bread, 1 + (next() % 3));
        chest.add_item(ItemType::IronIngot, 2 + (next() % 3));

        if next() % 100 < 65 {
            chest.add_item(ItemType::RedstoneDust, 2 + (next() % 5));
        }
        if next() % 100 < 52 {
            chest.add_item(ItemType::Arrow, 4 + (next() % 6));
        }
        if next() % 100 < 26 {
            chest.add_item(ItemType::Bucket, 1);
        }
        if next() % 100 < 24 {
            chest.add_item(ItemType::IronPickaxe, 1);
        }
        if next() % 100 < 18 {
            chest.add_item(ItemType::Bow, 1);
        }
        if next() % 100 < 16 {
            chest.add_item(ItemType::IronSword, 1);
        }

        if utility_bias {
            if next() % 100 < 42 {
                chest.add_item(ItemType::Bone, 1 + (next() % 3));
            }
            if next() % 100 < 34 {
                chest.add_item(ItemType::String, 1 + (next() % 3));
            }
            if next() % 100 < 30 {
                chest.add_item(ItemType::Coal, 1 + (next() % 3));
            }
        } else {
            if next() % 100 < 40 {
                chest.add_item(ItemType::Coal, 2 + (next() % 4));
            }
            if next() % 100 < 24 {
                chest.add_item(ItemType::GoldIngot, 1 + (next() % 2));
            }
        }

        if next() % 100 < 10 {
            chest.add_item(ItemType::Diamond, 1);
        }
    }

    fn build_overworld_dungeon(chunk: &mut Chunk, chunk_x: i32, perlin: &Perlin) {
        // A deterministic chunk cadence keeps generation stable and testable.
        if chunk_x.rem_euclid(OVERWORLD_DUNGEON_CHUNK_CADENCE) != OVERWORLD_DUNGEON_CHUNK_PHASE {
            return;
        }

        let c_start = chunk_x * CHUNK_WIDTH as i32;
        let hash = (chunk_x as u32)
            .wrapping_mul(1_103_515_245)
            .wrapping_add(12_345);
        let center_lx = 8 + (hash as usize % (CHUNK_WIDTH - 16));
        let center_wx = c_start + center_lx as i32;
        let surface_y = Self::overworld_surface_y(perlin, center_wx);

        let room_top = (surface_y + 12 + ((hash >> 8) % 12) as i32).clamp(40, 110);
        let room_bottom = (room_top + 6).min(CHUNK_HEIGHT as i32 - 3);
        if room_bottom - room_top < 5 {
            return;
        }

        let left = center_lx - 3;
        let right = center_lx + 3;
        for lx in left..=right {
            for wy in room_top..=room_bottom {
                let is_shell = lx == left || lx == right || wy == room_top || wy == room_bottom;
                if is_shell {
                    let cracked = ((chunk_x * 31 + lx as i32 * 17 + wy * 13).rem_euclid(9)) == 0;
                    chunk.set_block(
                        lx,
                        wy as usize,
                        if cracked {
                            BlockType::StoneBricks
                        } else {
                            BlockType::Cobblestone
                        },
                    );
                } else {
                    chunk.set_block(lx, wy as usize, BlockType::Air);
                }
            }
        }

        let floor_y = room_bottom - 1;
        let dungeon_spawner = if (hash >> 1) & 1 == 0 {
            BlockType::ZombieSpawner
        } else {
            BlockType::SkeletonSpawner
        };
        chunk.set_block(center_lx, floor_y as usize, dungeon_spawner);

        let opening_y = room_top + 3;
        chunk.set_block(left, opening_y as usize, BlockType::Air);
        chunk.set_block(right, opening_y as usize, BlockType::Air);

        let chest_lx = if hash & 1 == 0 { left + 1 } else { right - 1 };
        if chunk.get_block(chest_lx, floor_y as usize) == BlockType::Air {
            chunk.set_block(chest_lx, floor_y as usize, BlockType::Chest);
            Self::fill_dungeon_chest_loot(chunk, chest_lx, floor_y as usize, hash ^ 0xA5A5_5A5A);
        }
    }

    fn fill_village_chest_loot(chunk: &mut Chunk, local_x: usize, y: usize, seed: u32) {
        let Some(chest) = chunk.ensure_chest_inventory(local_x, y, 27) else {
            return;
        };

        let mut state = seed;
        let mut next = || {
            state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            state
        };

        chest.add_item(ItemType::Bread, 1 + (next() % 3));
        if next() % 100 < 70 {
            chest.add_item(ItemType::Wheat, 1 + (next() % 3));
        }
        if next() % 100 < 65 {
            chest.add_item(ItemType::WheatSeeds, 1 + (next() % 4));
        }
        if next() % 100 < 52 {
            chest.add_item(ItemType::Coal, 1 + (next() % 3));
        }
        if next() % 100 < 42 {
            chest.add_item(ItemType::Arrow, 2 + (next() % 5));
        }
        if next() % 100 < 32 {
            chest.add_item(ItemType::Feather, 1 + (next() % 2));
        }
        if next() % 100 < 18 {
            chest.add_item(ItemType::IronIngot, 1 + (next() % 2));
        }
        if next() % 100 < 6 {
            chest.add_item(ItemType::Diamond, 1);
        }
    }

    fn build_overworld_village_hut(
        chunk: &mut Chunk,
        chunk_x: i32,
        perlin: &Perlin,
        temp_perlin: &Perlin,
        moist_perlin: &Perlin,
    ) {
        // Sparse deterministic cadence keeps worldgen stable while adding exploration rewards.
        if chunk_x.rem_euclid(11) != 5 {
            return;
        }

        let c_start = chunk_x * CHUNK_WIDTH as i32;
        let hash = (chunk_x as u32)
            .wrapping_mul(2_654_435_761)
            .wrapping_add(1_013_904_223);
        if ((hash >> 7) & 1) != 0 {
            return;
        }

        let half_span = 3 + (((hash >> 11) & 1) as usize);
        let edge_margin = half_span + 2;
        let center_lx = edge_margin + (hash as usize % (CHUNK_WIDTH - edge_margin * 2));
        let center_wx = c_start + center_lx as i32;
        let biome = Self::biome_for_x(temp_perlin, moist_perlin, center_wx);
        if matches!(
            biome,
            BiomeType::Ocean
                | BiomeType::River
                | BiomeType::Swamp
                | BiomeType::Jungle
                | BiomeType::ExtremeHills
        ) {
            return;
        }

        let surface_y =
            Self::blended_overworld_surface_y(perlin, temp_perlin, moist_perlin, center_wx, biome);
        if surface_y > OVERWORLD_SEA_LEVEL {
            return;
        }
        let left = center_lx.saturating_sub(half_span);
        let right = (center_lx + half_span).min(CHUNK_WIDTH - 2);
        if right - left < 5 {
            return;
        }

        let mut min_surface_y = surface_y;
        let mut max_surface_y = surface_y;
        for sample_lx in left.saturating_sub(1)..=(right + 1).min(CHUNK_WIDTH - 1) {
            let sample_wx = c_start + sample_lx as i32;
            let sample_biome = Self::biome_for_x(temp_perlin, moist_perlin, sample_wx);
            let sample_surface_y = Self::blended_overworld_surface_y(
                perlin,
                temp_perlin,
                moist_perlin,
                sample_wx,
                sample_biome,
            );
            if sample_surface_y > OVERWORLD_SEA_LEVEL
                || Self::column_has_surface_water(
                    perlin,
                    temp_perlin,
                    moist_perlin,
                    sample_wx,
                    sample_biome,
                    sample_surface_y,
                )
            {
                return;
            }
            min_surface_y = min_surface_y.min(sample_surface_y);
            max_surface_y = max_surface_y.max(sample_surface_y);
        }
        if max_surface_y - min_surface_y > OVERWORLD_VILLAGE_HUT_MAX_SURFACE_DELTA {
            return;
        }

        let floor_y = (max_surface_y - 1).clamp(12, CHUNK_HEIGHT as i32 - 10);
        let roof_y = floor_y - 4;
        if roof_y < 3 {
            return;
        }

        let wall_block = match biome {
            BiomeType::Desert => BlockType::StoneBricks,
            BiomeType::Taiga | BiomeType::Tundra => BlockType::BirchWood,
            _ => BlockType::Wood,
        };
        let trim_block = match biome {
            BiomeType::Desert => BlockType::Stone,
            BiomeType::Taiga | BiomeType::Tundra => BlockType::Planks,
            _ => BlockType::Cobblestone,
        };
        let floor_block = if biome == BiomeType::Desert {
            BlockType::Sand
        } else {
            BlockType::Planks
        };
        let foundation_block = if biome == BiomeType::Desert {
            BlockType::StoneBricks
        } else {
            BlockType::Cobblestone
        };
        let roof_edge_block = if biome == BiomeType::Desert {
            BlockType::StoneStairs
        } else {
            BlockType::StoneSlab
        };
        let roof_cap_block = if biome == BiomeType::Desert {
            BlockType::StoneSlab
        } else {
            BlockType::Planks
        };

        for lx in left..=right {
            for wy in roof_y..=floor_y {
                let is_wall = lx == left || lx == right;
                let block = if wy == roof_y {
                    if is_wall { trim_block } else { roof_cap_block }
                } else if wy == floor_y {
                    floor_block
                } else if is_wall {
                    if wy == floor_y - 1 || wy == roof_y + 1 {
                        trim_block
                    } else {
                        wall_block
                    }
                } else {
                    BlockType::Air
                };
                chunk.set_block(lx, wy as usize, block);
            }

            let support_wx = c_start + lx as i32;
            let support_biome = Self::biome_for_x(temp_perlin, moist_perlin, support_wx);
            let support_surface_y = Self::blended_overworld_surface_y(
                perlin,
                temp_perlin,
                moist_perlin,
                support_wx,
                support_biome,
            );
            for support_y in (floor_y + 1)..=(support_surface_y + 1).min(CHUNK_HEIGHT as i32 - 2) {
                chunk.set_block(lx, support_y as usize, foundation_block);
            }
        }

        if roof_y > 1 {
            for lx in left.saturating_sub(1)..=(right + 1).min(CHUNK_WIDTH - 1) {
                chunk.set_block(lx, (roof_y - 1) as usize, roof_edge_block);
            }
        }

        let left_entry_wx = c_start + left as i32 - 1;
        let left_entry_biome = Self::biome_for_x(temp_perlin, moist_perlin, left_entry_wx);
        let left_entry_surface_y = Self::blended_overworld_surface_y(
            perlin,
            temp_perlin,
            moist_perlin,
            left_entry_wx,
            left_entry_biome,
        );
        let right_entry_wx = c_start + right as i32 + 1;
        let right_entry_biome = Self::biome_for_x(temp_perlin, moist_perlin, right_entry_wx);
        let right_entry_surface_y = Self::blended_overworld_surface_y(
            perlin,
            temp_perlin,
            moist_perlin,
            right_entry_wx,
            right_entry_biome,
        );
        let left_entry_step = (left_entry_surface_y - (floor_y + 1)).abs();
        let right_entry_step = (right_entry_surface_y - (floor_y + 1)).abs();
        let door_on_left = if left_entry_step == right_entry_step {
            (hash & 1) == 0
        } else {
            left_entry_step <= right_entry_step
        };
        let door_lx = if door_on_left { left } else { right };
        let door_outer_lx = if door_on_left {
            left.saturating_sub(1)
        } else {
            (right + 1).min(CHUNK_WIDTH - 1)
        };
        chunk.set_block(door_lx, floor_y as usize, BlockType::WoodDoor(false));
        chunk.set_block(door_lx, (floor_y - 1) as usize, BlockType::WoodDoor(false));
        if floor_y - 2 > roof_y {
            chunk.set_block(door_lx, (floor_y - 2) as usize, trim_block);
        }
        for clear_y in (floor_y - 1)..=floor_y {
            chunk.set_block(door_outer_lx, clear_y as usize, BlockType::Air);
        }
        let door_outer_wx = c_start + door_outer_lx as i32;
        let door_outer_biome = Self::biome_for_x(temp_perlin, moist_perlin, door_outer_wx);
        let door_outer_surface_y = Self::blended_overworld_surface_y(
            perlin,
            temp_perlin,
            moist_perlin,
            door_outer_wx,
            door_outer_biome,
        );
        for support_y in (floor_y + 1)..=(door_outer_surface_y + 1).min(CHUNK_HEIGHT as i32 - 2) {
            chunk.set_block(door_outer_lx, support_y as usize, foundation_block);
        }

        let window_top_y = floor_y - 3;
        let window_bottom_y = floor_y - 2;
        if window_top_y > roof_y {
            for window_y in window_top_y..=window_bottom_y {
                chunk.set_block(left, window_y as usize, BlockType::Glass);
                chunk.set_block(right, window_y as usize, BlockType::Glass);
            }
        }

        let furnishing_cols: Vec<usize> = if door_on_left {
            ((left + 2)..=right.saturating_sub(1)).rev().collect()
        } else {
            (left + 1..=right.saturating_sub(2)).collect()
        };
        let chest_lx = furnishing_cols.first().copied().unwrap_or(left + 1);
        chunk.set_block(chest_lx, floor_y as usize, BlockType::Chest);
        Self::fill_village_chest_loot(chunk, chest_lx, floor_y as usize, hash ^ 0x5A5A_A5A5);

        let station_lx = furnishing_cols
            .iter()
            .copied()
            .find(|&lx| lx != chest_lx)
            .unwrap_or(right - 1);
        chunk.set_block(station_lx, floor_y as usize, BlockType::CraftingTable);
        if right - left >= 6
            && let Some(bed_lx) = furnishing_cols
                .iter()
                .copied()
                .find(|&lx| lx != chest_lx && lx != station_lx)
            && bed_lx != chest_lx
            && bed_lx != station_lx
        {
            chunk.set_block(bed_lx, floor_y as usize, BlockType::Bed);
        }
    }

    fn enqueue_chunk_request(&mut self, chunk_x: i32, center_chunk: i32) {
        if self.chunks.contains_key(&chunk_x) || self.pending_chunks.contains(&chunk_x) {
            return;
        }
        if self.pending_chunks.len() >= CHUNK_PIPELINE_MAX_PENDING {
            return;
        }
        let request = ChunkRequest {
            chunk_x,
            distance: (chunk_x - center_chunk).abs(),
            seq: self.chunk_request_seq,
        };
        self.chunk_request_seq = self.chunk_request_seq.wrapping_add(1);
        self.pending_chunks.insert(chunk_x);
        self.queued_chunks.insert(chunk_x);
        self.chunk_request_queue.push(request);
    }

    fn trim_stale_chunk_requests(&mut self, center_chunk: i32) {
        let mut stale = Vec::new();
        for &chunk_x in &self.queued_chunks {
            if (chunk_x - center_chunk).abs() > CHUNK_REQUEST_RETENTION_RADIUS {
                stale.push(chunk_x);
            }
        }
        for chunk_x in stale {
            self.queued_chunks.remove(&chunk_x);
            self.pending_chunks.remove(&chunk_x);
        }

        // Keep the heap bounded by dropping entries for chunks no longer in the queued set,
        // and collapse duplicate entries per chunk that can appear after trim/re-enqueue cycles.
        if self.chunk_request_queue.len() > self.queued_chunks.len() {
            let mut compacted = BinaryHeap::with_capacity(self.queued_chunks.len());
            let mut seen_chunks = HashSet::with_capacity(self.queued_chunks.len());
            while let Some(request) = self.chunk_request_queue.pop() {
                if self.queued_chunks.contains(&request.chunk_x)
                    && seen_chunks.insert(request.chunk_x)
                {
                    compacted.push(request);
                }
            }
            self.chunk_request_queue = compacted;
        }
    }

    fn dispatch_chunk_requests(&mut self) {
        while self.in_flight_chunks.len() < CHUNK_PIPELINE_MAX_IN_FLIGHT {
            let Some(request) = self.chunk_request_queue.pop() else {
                break;
            };
            let chunk_x = request.chunk_x;
            if !self.queued_chunks.remove(&chunk_x) {
                continue;
            }
            if self.chunks.contains_key(&chunk_x) {
                self.pending_chunks.remove(&chunk_x);
                continue;
            }
            if self
                .chunk_request_tx
                .send(ChunkWorkItem { chunk_x })
                .is_ok()
            {
                self.in_flight_chunks.insert(chunk_x);
            } else {
                self.pending_chunks.remove(&chunk_x);
                self.ensure_chunk_loaded_now(chunk_x);
            }
        }
    }

    fn collect_ready_chunks(&mut self) {
        while let Ok((chunk_x, chunk, was_generated)) = self.chunk_result_rx.try_recv() {
            self.pending_chunks.remove(&chunk_x);
            self.queued_chunks.remove(&chunk_x);
            self.in_flight_chunks.remove(&chunk_x);
            if self.chunks.contains_key(&chunk_x) {
                continue;
            }
            if chunk.has_fluids() {
                self.activate_fluid_chunk_neighbors(chunk_x);
            }
            if was_generated {
                self.newly_generated_chunks.push(chunk_x);
                self.chunk_metrics.async_generated += 1;
            } else {
                self.chunk_metrics.async_loaded += 1;
            }
            self.chunks.insert(chunk_x, chunk);
        }
    }

    fn ensure_chunk_loaded_now(&mut self, chunk_x: i32) {
        if self.chunks.contains_key(&chunk_x) {
            return;
        }
        self.pending_chunks.remove(&chunk_x);
        self.queued_chunks.remove(&chunk_x);
        self.in_flight_chunks.remove(&chunk_x);
        if let Some(loaded_chunk) = Chunk::load_from_disk(chunk_x, &self.save_key) {
            if loaded_chunk.has_fluids() {
                self.activate_fluid_chunk_neighbors(chunk_x);
            }
            self.chunks.insert(chunk_x, loaded_chunk);
            self.chunk_metrics.sync_loaded += 1;
            return;
        }
        let generated_chunk = Self::build_chunk_with_noise(
            chunk_x,
            self.dimension,
            &self.save_key,
            &self.perlin,
            &self.temp_perlin,
            &self.moist_perlin,
            &self.cave_perlin,
        );
        if generated_chunk.has_fluids() {
            self.activate_fluid_chunk_neighbors(chunk_x);
        }
        self.chunks.insert(chunk_x, generated_chunk);
        self.newly_generated_chunks.push(chunk_x);
        self.chunk_metrics.sync_generated += 1;
    }

    fn build_chunk_with_noise(
        chunk_x: i32,
        dimension: Dimension,
        save_key: &str,
        perlin: &Perlin,
        temp_perlin: &Perlin,
        moist_perlin: &Perlin,
        cave_perlin: &Perlin,
    ) -> Chunk {
        let mut chunk = Chunk::new(chunk_x, save_key);
        match dimension {
            Dimension::Nether => {
                return Self::build_nether_chunk_with_noise(chunk, chunk_x, perlin, cave_perlin);
            }
            Dimension::End => {
                return Self::build_end_chunk_with_noise(chunk, chunk_x, perlin);
            }
            Dimension::Overworld => {}
        }
        let c_start = chunk_x * CHUNK_WIDTH as i32;
        let mut biome_by_x = [BiomeType::Plains; CHUNK_WIDTH];
        let mut surface_y_by_x = [0; CHUNK_WIDTH];
        let mut surface_water_by_x = [false; CHUNK_WIDTH];
        let mut surface_block_by_x = [BlockType::Grass; CHUNK_WIDTH];
        let mut subsurface_block_by_x = [BlockType::Dirt; CHUNK_WIDTH];
        let mut biome_cave_bias_by_x = [0.0; CHUNK_WIDTH];

        for x in 0..CHUNK_WIDTH {
            let world_x = c_start + x as i32;
            let biome = Self::biome_for_x(temp_perlin, moist_perlin, world_x);
            let surface_y = Self::blended_overworld_surface_y(
                perlin,
                temp_perlin,
                moist_perlin,
                world_x,
                biome,
            );
            let beach_column =
                Self::is_beach_column(temp_perlin, moist_perlin, world_x, biome, surface_y);
            let surface_water = Self::column_has_surface_water(
                perlin,
                temp_perlin,
                moist_perlin,
                world_x,
                biome,
                surface_y,
            );
            let shoreline_water_column = surface_water
                && !matches!(
                    biome,
                    BiomeType::Ocean | BiomeType::River | BiomeType::Swamp
                );
            let biome_cave_bias = Self::overworld_biome_cave_bias(biome);
            let surface_block = if beach_column || shoreline_water_column {
                BlockType::Sand
            } else {
                match biome {
                    BiomeType::Desert => BlockType::Sand,
                    BiomeType::Ocean | BiomeType::River => {
                        if perlin.get([world_x as f64 * 0.11, surface_y as f64 * 0.09, 42.0]) > 0.2
                        {
                            BlockType::Sand
                        } else {
                            BlockType::Gravel
                        }
                    }
                    BiomeType::ExtremeHills => {
                        if perlin.get([world_x as f64 * 0.13, surface_y as f64 * 0.13, 84.0]) > 0.5
                        {
                            BlockType::Stone
                        } else {
                            BlockType::Grass
                        }
                    }
                    BiomeType::Tundra | BiomeType::Taiga => BlockType::Snow,
                    _ => BlockType::Grass,
                }
            };
            let subsurface_block = if surface_block.obeys_gravity() {
                // The game does not have sandstone yet; stone support avoids
                // chunk-load sand/gravel collapses that are far too dramatic in 2D.
                BlockType::Stone
            } else {
                BlockType::Dirt
            };

            biome_by_x[x] = biome;
            surface_y_by_x[x] = surface_y;
            surface_water_by_x[x] = surface_water;
            surface_block_by_x[x] = surface_block;
            subsurface_block_by_x[x] = subsurface_block;
            biome_cave_bias_by_x[x] = biome_cave_bias;
        }

        for x in 0..CHUNK_WIDTH {
            let world_x = c_start + x as i32;
            let biome = biome_by_x[x];
            let surface_y = surface_y_by_x[x];
            let surface_water = surface_water_by_x[x];
            let surface_block = surface_block_by_x[x];
            let subsurface_block = subsurface_block_by_x[x];
            let biome_cave_bias = biome_cave_bias_by_x[x];

            for y in 0..CHUNK_HEIGHT {
                let current_y = y as i32;
                if current_y == (CHUNK_HEIGHT - 1) as i32 {
                    chunk.set_block(x, y, BlockType::Bedrock);
                    continue;
                }
                // Worm-like caves (two intersecting noise fields for spaghetti-like tunnels)
                let n1 = cave_perlin
                    .get([world_x as f64 * 0.05, current_y as f64 * 0.05, 0.0])
                    .abs();
                let n2 = cave_perlin
                    .get([world_x as f64 * 0.05, current_y as f64 * 0.05, 100.0])
                    .abs();
                let cave_depth = current_y - surface_y;
                let worm_threshold = if cave_depth <= OVERWORLD_NEAR_SURFACE_CAVE_BAND {
                    OVERWORLD_NEAR_SURFACE_WORM_CAVE_THRESHOLD + biome_cave_bias * 0.35
                } else {
                    OVERWORLD_WORM_CAVE_THRESHOLD + biome_cave_bias * 0.45
                };
                let is_worm_cave = n1 < worm_threshold && n2 < worm_threshold;
                let is_chamber = Self::overworld_chamber_carves(
                    cave_perlin,
                    world_x,
                    current_y,
                    biome,
                    surface_y,
                );

                let is_ravine = Self::overworld_ravine_carves(
                    cave_perlin,
                    world_x,
                    current_y,
                    biome,
                    surface_y,
                );

                let is_cave = (is_worm_cave || is_chamber || is_ravine)
                    && current_y > surface_y + OVERWORLD_CAVE_MIN_SURFACE_DEPTH;
                if is_cave {
                    if current_y > (CHUNK_HEIGHT - 5) as i32 {
                        chunk.set_block(x, y, BlockType::Lava(8));
                    } else {
                        chunk.set_block(x, y, BlockType::Air);
                    }
                    continue;
                }
                if current_y > surface_y + 4 {
                    let mut block = BlockType::Stone;
                    if let Some(ore_block) =
                        Self::overworld_ore_block(perlin, world_x, current_y, false)
                    {
                        block = ore_block;
                    } else if cave_depth >= OVERWORLD_GRAVEL_MIN_SURFACE_DEPTH
                        && perlin.get([world_x as f64 * 0.15, current_y as f64 * 0.15, 40.0]) > 0.75
                    {
                        block = BlockType::Gravel;
                    }
                    chunk.set_block(x, y, block);
                } else if current_y > surface_y {
                    chunk.set_block(x, y, subsurface_block);
                } else if current_y == surface_y {
                    chunk.set_block(x, y, surface_block);
                    if matches!(biome, BiomeType::Tundra | BiomeType::Taiga)
                        && perlin.get([world_x as f64 * 0.1, surface_y as f64 * 0.1])
                            > if biome == BiomeType::Tundra {
                                0.5
                            } else {
                                0.72
                            }
                    {
                        chunk.set_block(x, y, BlockType::Ice);
                    }
                } else if current_y >= OVERWORLD_SEA_LEVEL && surface_water {
                    chunk.set_block(x, y, BlockType::Water(8));
                } else {
                    chunk.set_block(x, y, BlockType::Air);
                }
            }
        }

        Self::dress_overworld_caves(
            &mut chunk,
            chunk_x,
            perlin,
            cave_perlin,
            &surface_y_by_x,
            &surface_water_by_x,
        );
        Self::refine_overworld_cave_pacing(&mut chunk, &surface_y_by_x, &surface_water_by_x);
        Self::refine_overworld_exposed_ores(
            &mut chunk,
            chunk_x,
            perlin,
            &surface_y_by_x,
            &surface_water_by_x,
        );
        Self::refine_overworld_ore_runs(&mut chunk);

        let c_end = c_start + CHUNK_WIDTH as i32;
        for wx in (c_start - 2)..(c_end + 2) {
            let b = Self::biome_for_x(temp_perlin, moist_perlin, wx);
            let sy = Self::blended_overworld_surface_y(perlin, temp_perlin, moist_perlin, wx, b);
            let beach_column = Self::is_beach_column(temp_perlin, moist_perlin, wx, b, sy);
            let water_edge_column =
                Self::column_touches_surface_water(perlin, temp_perlin, moist_perlin, wx, b, sy);
            let h = (wx as u32).wrapping_mul(134775813).wrapping_add(9821471) % 100;
            if water_edge_column
                && !matches!(b, BiomeType::Ocean)
                && h < match b {
                    BiomeType::Swamp => 30,
                    BiomeType::River => 24,
                    BiomeType::Desert => 18,
                    _ => 12,
                }
                && wx >= c_start
                && wx < c_end
            {
                let lx = (wx - c_start) as usize;
                let ground = chunk.get_block(lx, sy as usize);
                if matches!(ground, BlockType::Grass | BlockType::Dirt | BlockType::Sand)
                    && sy > 0
                    && chunk.get_block(lx, (sy - 1) as usize) == BlockType::Air
                {
                    let cane_height = 1 + (h % 3) as usize;
                    for ty in 1..=cane_height {
                        chunk.set_block(lx, (sy as usize).saturating_sub(ty), BlockType::SugarCane);
                    }
                }
            }
            match b {
                BiomeType::Desert => {
                    if h < 3 && wx >= c_start && wx < c_end {
                        let lx = (wx - c_start) as usize;
                        if sy == 0 || chunk.get_block(lx, (sy - 1) as usize) != BlockType::Air {
                            continue;
                        }
                        for ty in 1..=3 {
                            chunk.set_block(
                                lx,
                                (sy as usize).saturating_sub(ty),
                                BlockType::Cactus,
                            );
                        }
                    } else if h < 6 && wx >= c_start && wx < c_end {
                        let lx = (wx - c_start) as usize;
                        if sy == 0 || chunk.get_block(lx, (sy - 1) as usize) != BlockType::Air {
                            continue;
                        }
                        chunk.set_block(lx, (sy as usize).saturating_sub(1), BlockType::DeadBush);
                    }
                }
                BiomeType::Ocean | BiomeType::River => {}
                _ => {
                    if beach_column || water_edge_column {
                        continue;
                    }
                    let t_c = match b {
                        BiomeType::Forest => 15,
                        BiomeType::Taiga => 12,
                        BiomeType::Swamp => 10,
                        BiomeType::Jungle => 24,
                        BiomeType::ExtremeHills => 6,
                        BiomeType::Tundra => 4,
                        BiomeType::Plains => 2,
                        _ => 2,
                    };
                    if h < t_c {
                        let t_h = match b {
                            BiomeType::Jungle => 5 + (h % 4) as usize,
                            BiomeType::Taiga => 4 + (h % 3) as usize,
                            BiomeType::Swamp => 3 + (h % 2) as usize,
                            BiomeType::ExtremeHills => 4 + (h % 2) as usize,
                            BiomeType::Tundra => 3 + (h % 2) as usize,
                            _ => 3 + (h % 3) as usize,
                        };
                        let w_t = if matches!(b, BiomeType::Tundra | BiomeType::ExtremeHills) {
                            BlockType::BirchWood
                        } else {
                            BlockType::Wood
                        };
                        let l_t = if matches!(b, BiomeType::Tundra | BiomeType::ExtremeHills) {
                            BlockType::BirchLeaves
                        } else {
                            BlockType::Leaves
                        };
                        if wx >= c_start && wx < c_end {
                            let lx = (wx - c_start) as usize;
                            let ground = chunk.get_block(lx, sy as usize);
                            if matches!(ground, BlockType::Water(_) | BlockType::Lava(_))
                                || sy == 0
                                || chunk.get_block(lx, (sy - 1) as usize) != BlockType::Air
                            {
                                continue;
                            }
                            if ground == BlockType::Snow {
                                chunk.set_block(lx, sy as usize, BlockType::Dirt);
                            }
                            for ty in 1..=t_h {
                                chunk.set_block(lx, (sy as usize).saturating_sub(ty), w_t);
                            }
                        }
                        let b_ly = (sy as usize).saturating_sub(t_h);
                        for ly in (b_ly as i32 - 2)..=(b_ly as i32) {
                            if ly < 0 || ly >= CHUNK_HEIGHT as i32 {
                                continue;
                            }
                            let u_ly = ly as usize;
                            let wd = if b == BiomeType::Taiga {
                                if ly == (b_ly as i32 - 2) { 0 } else { 1 }
                            } else if ly == (b_ly as i32 - 2) {
                                1
                            } else {
                                2
                            };
                            for lx in (wx - wd)..=(wx + wd) {
                                if lx >= c_start && lx < c_end {
                                    let llx = (lx - c_start) as usize;
                                    if chunk.get_block(llx, u_ly) == BlockType::Air {
                                        chunk.set_block(llx, u_ly, l_t);
                                    }
                                }
                            }
                        }
                        if matches!(b, BiomeType::Taiga | BiomeType::Jungle) {
                            let cap_y = (b_ly as i32 - 3).clamp(0, CHUNK_HEIGHT as i32 - 1);
                            let lx = (wx - c_start) as usize;
                            if chunk.get_block(lx, cap_y as usize) == BlockType::Air {
                                chunk.set_block(lx, cap_y as usize, l_t);
                            }
                        }
                    } else if h < match b {
                        BiomeType::Taiga => 12,
                        BiomeType::Swamp => 26,
                        BiomeType::Jungle => 16,
                        BiomeType::ExtremeHills => 12,
                        _ => 20,
                    } && wx >= c_start
                        && wx < c_end
                    {
                        let lx = (wx - c_start) as usize;
                        if sy == 0 || chunk.get_block(lx, (sy - 1) as usize) != BlockType::Air {
                            continue;
                        }
                        let fl = match b {
                            BiomeType::Taiga => {
                                if h < 2 {
                                    BlockType::RedFlower
                                } else if h < 4 {
                                    BlockType::YellowFlower
                                } else {
                                    BlockType::TallGrass
                                }
                            }
                            BiomeType::Swamp => {
                                if h < 5 {
                                    BlockType::DeadBush
                                } else {
                                    BlockType::TallGrass
                                }
                            }
                            BiomeType::Jungle => {
                                if h < 4 {
                                    BlockType::RedFlower
                                } else {
                                    BlockType::TallGrass
                                }
                            }
                            BiomeType::ExtremeHills => {
                                if h < 3 {
                                    BlockType::YellowFlower
                                } else {
                                    BlockType::TallGrass
                                }
                            }
                            _ => {
                                if h < 5 {
                                    BlockType::RedFlower
                                } else if h < 10 {
                                    BlockType::YellowFlower
                                } else {
                                    BlockType::TallGrass
                                }
                            }
                        };
                        let ground = chunk.get_block(lx, sy as usize);
                        if !matches!(ground, BlockType::Water(_) | BlockType::Lava(_)) {
                            chunk.set_block(lx, (sy as usize).saturating_sub(1), fl);
                        }
                    }
                }
            }
        }
        Self::build_overworld_dungeon(&mut chunk, chunk_x, perlin);
        Self::build_overworld_village_hut(&mut chunk, chunk_x, perlin, temp_perlin, moist_perlin);
        Self::build_overworld_stronghold(&mut chunk, chunk_x);
        chunk
    }

    fn set_stronghold_block(
        chunk: &mut Chunk,
        c_start: i32,
        c_end: i32,
        wx: i32,
        wy: i32,
        block: BlockType,
    ) {
        if wx < c_start || wx >= c_end || wy <= 0 || wy >= CHUNK_HEIGHT as i32 - 1 {
            return;
        }
        let lx = (wx - c_start) as usize;
        chunk.set_block(lx, wy as usize, block);
    }

    fn place_stronghold_shell(chunk: &mut Chunk, c_start: i32, c_end: i32, wx: i32, wy: i32) {
        let cracked = (wx * 31 + wy * 17).rem_euclid(9) == 0;
        Self::set_stronghold_block(
            chunk,
            c_start,
            c_end,
            wx,
            wy,
            if cracked {
                BlockType::Cobblestone
            } else {
                BlockType::StoneBricks
            },
        );
    }

    #[allow(clippy::too_many_arguments)]
    fn carve_stronghold_room(
        chunk: &mut Chunk,
        c_start: i32,
        c_end: i32,
        left: i32,
        right: i32,
        top: i32,
        bottom: i32,
        floor: i32,
    ) {
        for wx in left..=right {
            for wy in top..=bottom {
                let is_shell =
                    wx == left || wx == right || wy == top || wy == bottom || wy == floor;
                if is_shell {
                    Self::place_stronghold_shell(chunk, c_start, c_end, wx, wy);
                } else {
                    Self::set_stronghold_block(chunk, c_start, c_end, wx, wy, BlockType::Air);
                }
            }
        }
    }

    fn carve_stronghold_hall(
        chunk: &mut Chunk,
        c_start: i32,
        c_end: i32,
        left: i32,
        right: i32,
        top: i32,
        bottom: i32,
    ) {
        let min_x = left.min(right);
        let max_x = left.max(right);
        for wx in min_x..=max_x {
            for wy in top..=bottom {
                let is_shell = wx == min_x || wx == max_x || wy == top || wy == bottom;
                if is_shell {
                    Self::place_stronghold_shell(chunk, c_start, c_end, wx, wy);
                } else {
                    Self::set_stronghold_block(chunk, c_start, c_end, wx, wy, BlockType::Air);
                }
            }
        }
    }

    fn build_overworld_stronghold(chunk: &mut Chunk, chunk_x: i32) {
        let c_start = chunk_x * CHUNK_WIDTH as i32;
        let c_end = c_start + CHUNK_WIDTH as i32;

        let room_left = STRONGHOLD_CENTER_X - 18;
        let room_right = STRONGHOLD_CENTER_X + 18;
        let hall_floor = STRONGHOLD_ROOM_BOTTOM_Y - 1;
        let hall_top = hall_floor - 3;
        let west_hall_left = room_left - 18;
        let east_hall_right = room_right + 22;

        let upper_floor = hall_floor - 5;
        let upper_top = upper_floor - 3;
        let upper_left = room_left - 12;
        let upper_right = room_right + 18;

        let library_room_left = room_left - 30;
        let library_room_right = room_left - 10;
        let library_room_top = upper_top - 5;
        let library_room_bottom = upper_floor + 4;
        let library_room_floor = upper_floor + 2;
        let library_shaft_x = room_left - 11;

        let silver_room_left = room_right + 10;
        let silver_room_right = room_right + 22;
        let silver_room_top = upper_top - 4;
        let silver_room_bottom = upper_floor + 3;
        let silver_room_floor = upper_floor + 2;

        let staging_room_left = room_right + 12;
        let staging_room_right = room_right + 30;
        let staging_room_top = hall_top - 4;
        let staging_room_bottom = hall_floor + 1;
        let staging_room_floor = hall_floor;

        let stronghold_left = library_room_left.min(west_hall_left);
        let stronghold_right = silver_room_right.max(staging_room_right);
        if c_end <= stronghold_left || c_start > stronghold_right {
            return;
        }

        // Main portal room.
        Self::carve_stronghold_room(
            chunk,
            c_start,
            c_end,
            room_left,
            room_right,
            STRONGHOLD_ROOM_TOP_Y,
            STRONGHOLD_ROOM_BOTTOM_Y,
            hall_floor,
        );

        // Ground-level halls that branch out from the main chamber.
        Self::carve_stronghold_hall(
            chunk,
            c_start,
            c_end,
            west_hall_left,
            room_left + 1,
            hall_top,
            hall_floor,
        );
        Self::carve_stronghold_hall(
            chunk,
            c_start,
            c_end,
            room_right - 1,
            east_hall_right,
            hall_top,
            hall_floor,
        );

        // Upper branch network.
        Self::carve_stronghold_hall(
            chunk,
            c_start,
            c_end,
            upper_left,
            upper_right,
            upper_top,
            upper_floor,
        );
        Self::carve_stronghold_room(
            chunk,
            c_start,
            c_end,
            library_room_left,
            library_room_right,
            library_room_top,
            library_room_bottom,
            library_room_floor,
        );
        Self::carve_stronghold_room(
            chunk,
            c_start,
            c_end,
            silver_room_left,
            silver_room_right,
            silver_room_top,
            silver_room_bottom,
            silver_room_floor,
        );
        Self::carve_stronghold_room(
            chunk,
            c_start,
            c_end,
            staging_room_left,
            staging_room_right,
            staging_room_top,
            staging_room_bottom,
            staging_room_floor,
        );

        // Vertical connectors between levels.
        for connector_x in [room_left + 4, room_right - 4] {
            for wy in upper_top..=hall_floor - 1 {
                Self::set_stronghold_block(chunk, c_start, c_end, connector_x, wy, BlockType::Air);
                Self::set_stronghold_block(
                    chunk,
                    c_start,
                    c_end,
                    connector_x + 1,
                    wy,
                    BlockType::Air,
                );
            }
        }
        for wy in library_room_top + 1..=hall_floor - 1 {
            Self::set_stronghold_block(
                chunk,
                c_start,
                c_end,
                library_shaft_x,
                wy,
                BlockType::Ladder,
            );
            Self::set_stronghold_block(
                chunk,
                c_start,
                c_end,
                library_shaft_x + 1,
                wy,
                BlockType::Air,
            );
        }

        // Guidance lights and pillars make the approach to the portal room read clearly in 2D.
        let mut hall_light_x = west_hall_left + 3;
        while hall_light_x <= east_hall_right - 3 {
            Self::set_stronghold_block(
                chunk,
                c_start,
                c_end,
                hall_light_x,
                hall_top + 1,
                BlockType::Torch,
            );
            hall_light_x += 5;
        }
        let mut upper_light_x = upper_left + 3;
        while upper_light_x <= upper_right - 3 {
            Self::set_stronghold_block(
                chunk,
                c_start,
                c_end,
                upper_light_x,
                upper_top + 1,
                BlockType::Torch,
            );
            upper_light_x += 6;
        }
        for pillar_x in [room_left + 3, room_right - 3] {
            for wy in (STRONGHOLD_ROOM_TOP_Y + 1)..=hall_floor - 1 {
                Self::set_stronghold_block(
                    chunk,
                    c_start,
                    c_end,
                    pillar_x,
                    wy,
                    BlockType::StoneBricks,
                );
            }
        }
        for marker_x in [
            library_shaft_x - 1,
            library_shaft_x + 2,
            staging_room_left + 2,
        ] {
            Self::set_stronghold_block(
                chunk,
                c_start,
                c_end,
                marker_x,
                hall_top,
                BlockType::Glowstone,
            );
        }

        // The library wing reads as a real side branch rather than a bare box.
        for shelf_x in [
            library_room_left + 2,
            library_room_left + 3,
            library_room_right - 3,
            library_room_right - 2,
        ] {
            for wy in (library_room_top + 2)..=(library_room_floor - 2) {
                Self::set_stronghold_block(chunk, c_start, c_end, shelf_x, wy, BlockType::Planks);
            }
        }
        for loft_x in (library_room_left + 5)..=(library_room_right - 5) {
            if (library_room_left + 8..=library_room_left + 10).contains(&loft_x) {
                continue;
            }
            Self::set_stronghold_block(
                chunk,
                c_start,
                c_end,
                loft_x,
                library_room_floor - 4,
                BlockType::Planks,
            );
        }
        for ladder_x in [library_room_left + 5, library_room_right - 5] {
            for wy in (library_room_top + 2)..=(library_room_floor - 1) {
                Self::set_stronghold_block(chunk, c_start, c_end, ladder_x, wy, BlockType::Ladder);
            }
        }
        for torch_x in [
            library_room_left + 5,
            (library_room_left + library_room_right) / 2,
            library_room_right - 5,
        ] {
            Self::set_stronghold_block(
                chunk,
                c_start,
                c_end,
                torch_x,
                library_room_top + 1,
                BlockType::Torch,
            );
        }

        // The staging room gives a clear pre-portal supply stop on the main route.
        for utility_x in [staging_room_left + 4, staging_room_right - 4] {
            Self::set_stronghold_block(
                chunk,
                c_start,
                c_end,
                utility_x,
                staging_room_floor - 1,
                BlockType::StoneSlab,
            );
        }
        Self::set_stronghold_block(
            chunk,
            c_start,
            c_end,
            staging_room_left + 3,
            staging_room_floor,
            BlockType::CraftingTable,
        );
        Self::set_stronghold_block(
            chunk,
            c_start,
            c_end,
            staging_room_left + 5,
            staging_room_floor,
            BlockType::Furnace,
        );
        for torch_x in [staging_room_left + 2, staging_room_right - 2] {
            Self::set_stronghold_block(
                chunk,
                c_start,
                c_end,
                torch_x,
                staging_room_top + 1,
                BlockType::Torch,
            );
        }

        // Iron doors on branch transitions.
        for (door_x, door_y, open) in [
            (room_left + 1, hall_floor - 1, true),
            (room_right - 1, hall_floor - 1, true),
            (room_left + 4, upper_floor - 1, true),
            (library_room_right, upper_floor - 1, true),
            (silver_room_left + 1, upper_floor - 1, true),
            (staging_room_left + 1, hall_floor - 1, true),
        ] {
            Self::set_stronghold_block(
                chunk,
                c_start,
                c_end,
                door_x,
                door_y,
                BlockType::IronDoor(open),
            );
            Self::set_stronghold_block(
                chunk,
                c_start,
                c_end,
                door_x,
                door_y - 1,
                BlockType::IronDoor(open),
            );
        }

        // Silverfish room feature.
        let spawner_x = (silver_room_left + silver_room_right) / 2;
        let spawner_y = silver_room_floor - 1;
        Self::set_stronghold_block(
            chunk,
            c_start,
            c_end,
            spawner_x,
            spawner_y,
            BlockType::SilverfishSpawner,
        );

        // Portal frame remains anchored in the main room.
        let inner_x = STRONGHOLD_PORTAL_INNER_X;
        let inner_y = STRONGHOLD_PORTAL_INNER_Y;
        let left = inner_x - 1;
        let right = inner_x + 2;
        let top = inner_y - 1;
        let bottom = inner_y + 2;
        for wx in left..=right {
            for wy in top..=bottom {
                let is_frame = wx == left || wx == right || wy == top || wy == bottom;
                if is_frame {
                    Self::set_stronghold_block(
                        chunk,
                        c_start,
                        c_end,
                        wx,
                        wy,
                        BlockType::EndPortalFrame { filled: false },
                    );
                } else {
                    Self::set_stronghold_block(chunk, c_start, c_end, wx, wy, BlockType::Air);
                }
            }
        }

        // Build a compact portal dais with glow markers so the room communicates purpose quickly.
        for wx in (inner_x - 3)..=(inner_x + 4) {
            Self::set_stronghold_block(
                chunk,
                c_start,
                c_end,
                wx,
                hall_floor,
                BlockType::StoneBricks,
            );
            if wx == inner_x - 3 || wx == inner_x + 4 {
                Self::set_stronghold_block(
                    chunk,
                    c_start,
                    c_end,
                    wx,
                    hall_floor - 1,
                    BlockType::StoneSlab,
                );
            }
        }
        for wx in [inner_x - 3, inner_x + 4] {
            for wy in [top - 1, bottom + 1] {
                Self::set_stronghold_block(chunk, c_start, c_end, wx, wy, BlockType::Glowstone);
            }
        }
        for runway_x in (room_left + 6)..=(inner_x - 5) {
            if runway_x.rem_euclid(5) == 0 {
                Self::set_stronghold_block(
                    chunk,
                    c_start,
                    c_end,
                    runway_x,
                    hall_floor - 1,
                    BlockType::StoneSlab,
                );
            }
        }

        // Branch-room chests make underground progression feel less starved in the 2D slice.
        for (chest_x, chest_y, utility_bias, salt) in [
            (library_room_left + 3, library_room_floor, true, 0x1357_2468),
            (silver_room_right - 2, silver_room_floor, false, 0x2468_1357),
            (
                staging_room_right - 2,
                staging_room_floor,
                false,
                0x3579_1357,
            ),
        ] {
            Self::set_stronghold_block(chunk, c_start, c_end, chest_x, chest_y, BlockType::Chest);
            if chest_x >= c_start && chest_x < c_end {
                let local_x = (chest_x - c_start) as usize;
                Self::fill_stronghold_chest_loot(
                    chunk,
                    local_x,
                    chest_y as usize,
                    ((chest_x * 31 + chest_y * 17) as u32) ^ salt,
                    utility_bias,
                );
                if let Some(inv) = chunk.chest_inventory_mut(local_x, chest_y as usize) {
                    if chest_x == library_room_left + 3 {
                        inv.add_item(ItemType::Arrow, 6);
                        inv.add_item(ItemType::String, 2);
                    } else if chest_x == staging_room_right - 2 {
                        inv.add_item(ItemType::Bread, 2);
                        inv.add_item(ItemType::Torch, 4);
                    }
                }
            }
        }
    }

    fn build_end_chunk_with_noise(mut chunk: Chunk, chunk_x: i32, perlin: &Perlin) -> Chunk {
        let c_start = chunk_x * CHUNK_WIDTH as i32;

        for x in 0..CHUNK_WIDTH {
            let world_x = c_start + x as i32;
            for y in 0..CHUNK_HEIGHT {
                let current_y = y as i32;
                if current_y == (CHUNK_HEIGHT - 1) as i32 {
                    chunk.set_block(x, y, BlockType::Bedrock);
                } else {
                    chunk.set_block(x, y, BlockType::Air);
                }
            }

            let dist = world_x.abs() as f64;
            if dist <= 44.0 {
                let surface_y = (34.0 + perlin.get([world_x as f64 * 0.08, 700.0]) * 2.0) as i32;
                let thickness = (5.0 + (1.0 - dist / 44.0).max(0.0) * 12.0) as i32;
                for wy in surface_y..=(surface_y + thickness).min(CHUNK_HEIGHT as i32 - 2) {
                    if wy > 0 {
                        chunk.set_block(x, wy as usize, BlockType::EndStone);
                    }
                }
            } else {
                // Sparse outer islands as a scaffold for later End expansion.
                let island_noise = perlin.get([world_x as f64 * 0.045, 810.0]).abs()
                    + perlin.get([0.0, dist * 0.03]).abs();
                if dist < 140.0 && island_noise > 1.18 {
                    let top = 44 + ((island_noise * 2.0) as i32 % 4);
                    let depth = 2 + ((island_noise * 10.0) as i32 % 4);
                    for wy in top..=(top + depth).min(CHUNK_HEIGHT as i32 - 2) {
                        chunk.set_block(x, wy as usize, BlockType::EndStone);
                    }
                }
            }
        }

        for (idx, tower_x) in END_TOWER_XS.iter().enumerate() {
            let tower_x = *tower_x;
            if tower_x < c_start || tower_x >= c_start + CHUNK_WIDTH as i32 {
                continue;
            }
            let lx = (tower_x - c_start) as usize;
            let mut base_y = 34;
            for y in 2..(CHUNK_HEIGHT as i32 - 2) {
                if chunk.get_block(lx, y as usize) == BlockType::EndStone {
                    base_y = y;
                    break;
                }
            }
            let height = 8 + ((idx as i32 * 5 + tower_x.abs()) % 5);
            let top_y = (base_y - height).max(2);
            let arch_roof_y = (base_y - 4).max(top_y + 1);

            for wy in top_y..arch_roof_y {
                if wy > 1 {
                    chunk.set_block(lx, wy as usize, BlockType::Obsidian);
                }
            }

            for wx in (tower_x - 2)..=(tower_x + 2) {
                if wx >= c_start && wx < c_start + CHUNK_WIDTH as i32 && arch_roof_y > 1 {
                    let local_x = (wx - c_start) as usize;
                    chunk.set_block(local_x, arch_roof_y as usize, BlockType::Obsidian);
                }
            }

            for support_x in [tower_x - 2, tower_x + 2] {
                if support_x < c_start || support_x >= c_start + CHUNK_WIDTH as i32 {
                    continue;
                }
                let support_lx = (support_x - c_start) as usize;
                for wy in (arch_roof_y + 1)..=base_y {
                    if wy > 1 {
                        chunk.set_block(support_lx, wy as usize, BlockType::Obsidian);
                    }
                }
            }

            for wx in (tower_x - 1)..=(tower_x + 1) {
                if wx >= c_start && wx < c_start + CHUNK_WIDTH as i32 {
                    let local_x = (wx - c_start) as usize;
                    for passage_y in (arch_roof_y + 1)..=(base_y - 1) {
                        if passage_y > 1 {
                            chunk.set_block(local_x, passage_y as usize, BlockType::Air);
                        }
                    }
                }
            }

            for wx in (tower_x - 3)..=(tower_x + 3) {
                if wx >= c_start && wx < c_start + CHUNK_WIDTH as i32 {
                    let local_x = (wx - c_start) as usize;
                    if base_y > 1 && chunk.get_block(local_x, base_y as usize) == BlockType::Air {
                        chunk.set_block(local_x, base_y as usize, BlockType::EndStone);
                    }
                }
            }
            let glow_y = top_y - 1;
            if glow_y > 1 {
                chunk.set_block(lx, glow_y as usize, BlockType::Glowstone);
            }
            if base_y - 2 > 1 {
                for support_x in [tower_x - 2, tower_x + 2] {
                    if support_x >= c_start && support_x < c_start + CHUNK_WIDTH as i32 {
                        let support_lx = (support_x - c_start) as usize;
                        chunk.set_block(support_lx, (base_y - 2) as usize, BlockType::Glowstone);
                    }
                }
            }
        }

        // End return portal scaffold at the island center.
        let inner_x = -1;
        let inner_y = 32;
        let left = inner_x - 1;
        let right = inner_x + 2;
        let top = inner_y - 1;
        let bottom = inner_y + 2;
        for wx in left..=right {
            if wx < c_start || wx >= c_start + CHUNK_WIDTH as i32 {
                continue;
            }
            let lx = (wx - c_start) as usize;
            for wy in top..=bottom {
                if wy <= 1 || wy >= CHUNK_HEIGHT as i32 - 2 {
                    continue;
                }
                let is_frame = wx == left || wx == right || wy == top || wy == bottom;
                if is_frame {
                    chunk.set_block(lx, wy as usize, BlockType::Bedrock);
                } else {
                    chunk.set_block(lx, wy as usize, BlockType::EndPortal);
                }
            }
        }

        // Add a low obsidian dais under the return portal to improve readability in 2D.
        let dais_y = bottom + 1;
        if dais_y > 1 && dais_y < CHUNK_HEIGHT as i32 - 2 {
            for wx in (left - 3)..=(right + 3) {
                if wx < c_start || wx >= c_start + CHUNK_WIDTH as i32 {
                    continue;
                }
                let lx = (wx - c_start) as usize;
                if chunk.get_block(lx, dais_y as usize) == BlockType::Air {
                    chunk.set_block(lx, dais_y as usize, BlockType::Obsidian);
                }
            }
        }

        chunk
    }

    fn set_nether_fortress_block(
        chunk: &mut Chunk,
        c_start: i32,
        c_end: i32,
        wx: i32,
        wy: i32,
        block: BlockType,
    ) {
        if wx < c_start || wx >= c_end || wy <= 1 || wy >= CHUNK_HEIGHT as i32 - 1 {
            return;
        }
        let lx = (wx - c_start) as usize;
        chunk.set_block(lx, wy as usize, block);
    }

    fn nether_fortress_floor_block(wx: i32, center_x: i32) -> BlockType {
        match (wx - center_x).rem_euclid(13) {
            0 => BlockType::StoneStairs,
            1 | 2 => BlockType::StoneSlab,
            _ => BlockType::StoneBricks,
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn carve_nether_fortress_landmark(
        chunk: &mut Chunk,
        c_start: i32,
        c_end: i32,
        center_x: i32,
        top: i32,
        base_y: i32,
        half_width: i32,
        arch_half_width: i32,
        arch_height: i32,
    ) {
        let left = center_x - half_width;
        let right = center_x + half_width;
        for wx in left..=right {
            for wy in top..=base_y {
                let is_shell = wx == left || wx == right || wy == top || wy == base_y;
                Self::set_nether_fortress_block(
                    chunk,
                    c_start,
                    c_end,
                    wx,
                    wy,
                    if is_shell {
                        BlockType::StoneBricks
                    } else {
                        BlockType::Air
                    },
                );
            }
        }

        for wx in (center_x - arch_half_width)..=(center_x + arch_half_width) {
            for wy in (base_y - arch_height + 1)..=(base_y - 1) {
                Self::set_nether_fortress_block(chunk, c_start, c_end, wx, wy, BlockType::Air);
            }
        }

        for buttress_x in [left + 1, right - 1] {
            Self::set_nether_fortress_block(
                chunk,
                c_start,
                c_end,
                buttress_x,
                base_y - 1,
                BlockType::StoneStairs,
            );
            Self::set_nether_fortress_block(
                chunk,
                c_start,
                c_end,
                buttress_x,
                base_y - 2,
                BlockType::StoneBricks,
            );
        }

        for glow_x in [center_x - 2, center_x + 2] {
            Self::set_nether_fortress_block(
                chunk,
                c_start,
                c_end,
                glow_x,
                top + 1,
                BlockType::Glowstone,
            );
        }
        Self::set_nether_fortress_block(
            chunk,
            c_start,
            c_end,
            center_x,
            top + 2,
            BlockType::Glowstone,
        );
    }

    fn carve_nether_fortress_watchtower(
        chunk: &mut Chunk,
        c_start: i32,
        c_end: i32,
        center_x: i32,
        top: i32,
        base_y: i32,
    ) {
        Self::carve_nether_fortress_landmark(chunk, c_start, c_end, center_x, top, base_y, 4, 2, 4);

        for tower_x in [center_x - 1, center_x + 1] {
            for wy in (top - 3).max(4)..=(top - 1).max(4) {
                Self::set_nether_fortress_block(
                    chunk,
                    c_start,
                    c_end,
                    tower_x,
                    wy,
                    BlockType::StoneBricks,
                );
            }
        }

        for roof_x in (center_x - 2)..=(center_x + 2) {
            Self::set_nether_fortress_block(
                chunk,
                c_start,
                c_end,
                roof_x,
                (top - 4).max(3),
                BlockType::StoneSlab,
            );
        }

        for glow_x in [center_x - 1, center_x + 1] {
            Self::set_nether_fortress_block(
                chunk,
                c_start,
                c_end,
                glow_x,
                (top - 2).max(3),
                BlockType::Glowstone,
            );
        }
    }

    fn carve_nether_lava_brazier(
        chunk: &mut Chunk,
        c_start: i32,
        c_end: i32,
        center_x: i32,
        lip_y: i32,
        depth: i32,
    ) {
        if lip_y <= 2 || lip_y + depth >= CHUNK_HEIGHT as i32 - 2 {
            return;
        }

        for wx in (center_x - 1)..=(center_x + 1) {
            Self::set_nether_fortress_block(chunk, c_start, c_end, wx, lip_y, BlockType::Air);
            for wy in (lip_y + 1)..=(lip_y + depth) {
                Self::set_nether_fortress_block(chunk, c_start, c_end, wx, wy, BlockType::Lava(8));
            }
        }

        for wx in (center_x - 2)..=(center_x + 2) {
            Self::set_nether_fortress_block(
                chunk,
                c_start,
                c_end,
                wx,
                lip_y + depth,
                BlockType::StoneBricks,
            );
        }
        for wall_x in [center_x - 2, center_x + 2] {
            for wy in lip_y..=(lip_y + depth) {
                Self::set_nether_fortress_block(
                    chunk,
                    c_start,
                    c_end,
                    wall_x,
                    wy,
                    BlockType::StoneBricks,
                );
            }
        }
        for cap_x in [center_x - 2, center_x + 2] {
            Self::set_nether_fortress_block(
                chunk,
                c_start,
                c_end,
                cap_x,
                lip_y - 1,
                BlockType::StoneSlab,
            );
        }
    }

    fn fill_nether_fortress_chest_loot(
        chunk: &mut Chunk,
        local_x: usize,
        y: usize,
        seed: u32,
        guaranteed_wart: bool,
    ) {
        let Some(chest) = chunk.ensure_chest_inventory(local_x, y, 27) else {
            return;
        };

        let mut state = seed;
        let mut next = || {
            state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            state
        };

        if guaranteed_wart {
            chest.add_item(ItemType::NetherWart, 2 + (next() % 3));
        } else if next() % 100 < 58 {
            chest.add_item(ItemType::NetherWart, 1 + (next() % 2));
        }
        if next() % 100 < 62 {
            chest.add_item(ItemType::Coal, 2 + (next() % 4));
        }
        if next() % 100 < 56 {
            chest.add_item(ItemType::GoldIngot, 1 + (next() % 3));
        }
        if next() % 100 < 48 {
            chest.add_item(ItemType::IronIngot, 1 + (next() % 3));
        }
        if next() % 100 < 34 {
            chest.add_item(ItemType::BlazePowder, 1 + (next() % 2));
        }
        if next() % 100 < 24 {
            chest.add_item(ItemType::GhastTear, 1);
        }
        if next() % 100 < 18 {
            chest.add_item(ItemType::MagmaCream, 1);
        }
        if next() % 100 < 7 {
            chest.add_item(ItemType::Diamond, 1);
        }
    }

    fn fill_nether_landmark_chest_loot(chunk: &mut Chunk, local_x: usize, y: usize, seed: u32) {
        let Some(chest) = chunk.ensure_chest_inventory(local_x, y, 27) else {
            return;
        };

        let mut state = seed;
        let mut next = || {
            state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            state
        };

        if next() & 1 == 0 {
            chest.add_item(ItemType::Bread, 1 + (next() % 2));
        } else {
            chest.add_item(ItemType::CookedBeef, 1 + (next() % 2));
        }
        if next() % 100 < 70 {
            chest.add_item(ItemType::GoldIngot, 1 + (next() % 3));
        }
        if next() % 100 < 58 {
            chest.add_item(ItemType::IronIngot, 1 + (next() % 3));
        }
        if next() % 100 < 52 {
            chest.add_item(ItemType::Arrow, 3 + (next() % 5));
        }
        if next() % 100 < 18 {
            chest.add_item(ItemType::Bow, 1);
        }
        if next() % 100 < 24 {
            chest.add_item(ItemType::PotionFireResistance, 1);
        }
        if next() % 100 < 20 {
            chest.add_item(ItemType::BlazePowder, 1 + (next() % 2));
        }
        if next() % 100 < 14 {
            chest.add_item(ItemType::MagmaCream, 1);
        }
        if next() % 100 < 10 {
            chest.add_item(ItemType::GhastTear, 1);
        }
    }

    fn nether_chamber_room_shape(perlin: &Perlin, world_x: i32) -> (i32, i32) {
        let mut best_lift = 0;
        let mut best_drop = 0;
        let cell = world_x.div_euclid(NETHER_CHAMBER_CELL_WIDTH);

        for candidate in (cell - 1)..=(cell + 1) {
            let active = perlin.get([candidate as f64 * 0.59, 410.0]);
            if active < 0.12 {
                continue;
            }

            let offset_noise = perlin.get([candidate as f64 * 0.83, 411.0]);
            let radius_noise = perlin.get([candidate as f64 * 1.07, 412.0]);
            let lift_noise = perlin.get([candidate as f64 * 1.29, 413.0]);
            let drop_noise = perlin.get([candidate as f64 * 1.51, 414.0]);
            let center_x = candidate * NETHER_CHAMBER_CELL_WIDTH
                + (NETHER_CHAMBER_CELL_WIDTH / 2)
                + (offset_noise * 7.0).round() as i32;
            let region_chunk = center_x
                .div_euclid(CHUNK_WIDTH as i32)
                .div_euclid(NETHER_FORTRESS_REGION_CHUNKS);
            let room_fortress_layout = Self::nether_fortress_layout(perlin, region_chunk);
            if (center_x - room_fortress_layout.center_x).abs()
                <= NETHER_FORTRESS_HALF_SPAN_BLOCKS + 20
            {
                continue;
            }

            let radius = NETHER_CHAMBER_MIN_RADIUS
                + ((((radius_noise + 1.0) * 0.5) * NETHER_CHAMBER_RADIUS_SPAN as f64).round()
                    as i32)
                    .clamp(0, NETHER_CHAMBER_RADIUS_SPAN);
            let dx = (world_x - center_x).abs();
            if dx > radius {
                continue;
            }

            let profile = 1.0 - dx as f64 / (radius.max(1) as f64 + 0.35);
            let ceiling_lift = (profile
                * profile
                * (2.0 + ((lift_noise + 1.0) * 0.5) * NETHER_CHAMBER_MAX_CEILING_LIFT as f64))
                .round() as i32;
            let floor_drop = (profile
                * (1.5 + ((drop_noise + 1.0) * 0.5) * NETHER_CHAMBER_MAX_FLOOR_DROP as f64))
                .round() as i32;
            best_lift = best_lift.max(ceiling_lift);
            best_drop = best_drop.max(floor_drop);
        }

        (best_lift, best_drop)
    }

    fn carve_nether_route_landmark(
        chunk: &mut Chunk,
        c_start: i32,
        c_end: i32,
        center_x: i32,
        ceiling_y: i32,
        floor_y: i32,
        seed: u32,
    ) {
        match Self::nether_route_landmark_variant(seed) {
            NetherRouteLandmarkVariant::Shrine => Self::carve_nether_route_shrine(
                chunk, c_start, c_end, center_x, ceiling_y, floor_y, seed,
            ),
            NetherRouteLandmarkVariant::Bridge => Self::carve_nether_route_bridge(
                chunk, c_start, c_end, center_x, ceiling_y, floor_y, seed,
            ),
            NetherRouteLandmarkVariant::Reliquary => Self::carve_nether_route_reliquary(
                chunk, c_start, c_end, center_x, ceiling_y, floor_y, seed,
            ),
        }
    }

    fn nether_route_landmark_variant(seed: u32) -> NetherRouteLandmarkVariant {
        match seed % 3 {
            0 => NetherRouteLandmarkVariant::Shrine,
            1 => NetherRouteLandmarkVariant::Bridge,
            _ => NetherRouteLandmarkVariant::Reliquary,
        }
    }

    fn carve_nether_route_shrine(
        chunk: &mut Chunk,
        c_start: i32,
        c_end: i32,
        center_x: i32,
        ceiling_y: i32,
        floor_y: i32,
        seed: u32,
    ) {
        let top_y = (floor_y - 7).max(ceiling_y + 3);
        if top_y >= floor_y - 2 {
            return;
        }

        let width_noise =
            ((seed.wrapping_mul(1_103_515_245).wrapping_add(12_345) >> 16) & 0xFF) as i32;
        let half_width = if width_noise.rem_euclid(3) == 0 { 6 } else { 5 };
        let left = center_x - half_width;
        let right = center_x + half_width;
        if left < c_start || right >= c_end {
            return;
        }

        let chest_on_right = (seed & 1) == 0;
        let chest_x = center_x
            + if chest_on_right {
                half_width - 2
            } else {
                -(half_width - 2)
            };
        let chest_y = floor_y - 1;

        for wx in left..=right {
            for wy in top_y..=chest_y {
                let is_shell = wx == left || wx == right || wy == top_y;
                Self::set_nether_fortress_block(
                    chunk,
                    c_start,
                    c_end,
                    wx,
                    wy,
                    if is_shell {
                        BlockType::StoneBricks
                    } else {
                        BlockType::Air
                    },
                );
            }
        }

        for wx in (center_x - 2)..=(center_x + 2) {
            for wy in (floor_y - 4)..=chest_y {
                Self::set_nether_fortress_block(chunk, c_start, c_end, wx, wy, BlockType::Air);
            }
        }

        for glow_x in [left + 1, center_x, right - 1] {
            Self::set_nether_fortress_block(
                chunk,
                c_start,
                c_end,
                glow_x,
                top_y + 1,
                BlockType::Glowstone,
            );
        }

        let chest_local_x = (chest_x - c_start) as usize;
        chunk.set_block(chest_local_x, chest_y as usize, BlockType::Chest);
        Self::fill_nether_landmark_chest_loot(
            chunk,
            chest_local_x,
            chest_y as usize,
            seed ^ center_x as u32,
        );

        for wx in [chest_x - 1, chest_x + 1] {
            Self::set_nether_fortress_block(
                chunk,
                c_start,
                c_end,
                wx,
                chest_y,
                BlockType::StoneSlab,
            );
            Self::set_nether_fortress_block(
                chunk,
                c_start,
                c_end,
                wx,
                floor_y,
                BlockType::StoneBricks,
            );
        }

        for arch_x in [center_x - 3, center_x + 3] {
            Self::set_nether_fortress_block(
                chunk,
                c_start,
                c_end,
                arch_x,
                chest_y,
                BlockType::StoneStairs,
            );
        }

        for brazier_x in [left + 2, right - 2] {
            Self::carve_nether_lava_brazier(chunk, c_start, c_end, brazier_x, floor_y, 2);
        }
    }

    fn carve_nether_route_bridge(
        chunk: &mut Chunk,
        c_start: i32,
        c_end: i32,
        center_x: i32,
        ceiling_y: i32,
        floor_y: i32,
        seed: u32,
    ) {
        let top_y = (floor_y - 8).max(ceiling_y + 3);
        if top_y >= floor_y - 3 {
            return;
        }

        let half_width = 6;
        let left = center_x - half_width;
        let right = center_x + half_width;
        if left < c_start || right >= c_end {
            return;
        }

        let beam_y = top_y + 2;
        let bridge_y = floor_y - 1;
        let chest_y = bridge_y - 1;
        for wx in left..=right {
            for wy in top_y..=bridge_y {
                let is_outer_pylon =
                    matches!(wx, x if x == left || x == left + 1 || x == right - 1 || x == right);
                let is_beam = wy == beam_y && wx > left + 1 && wx < right - 1;
                let is_bridge_deck = wy == bridge_y && (center_x - 3..=center_x + 3).contains(&wx);
                let block = if is_outer_pylon || is_beam || is_bridge_deck {
                    BlockType::StoneBricks
                } else {
                    BlockType::Air
                };
                Self::set_nether_fortress_block(chunk, c_start, c_end, wx, wy, block);
            }
        }

        for wx in (center_x - 2)..=(center_x + 2) {
            Self::set_nether_fortress_block(chunk, c_start, c_end, wx, floor_y, BlockType::Air);
            Self::set_nether_fortress_block(
                chunk,
                c_start,
                c_end,
                wx,
                floor_y + 1,
                BlockType::Lava(8),
            );
            Self::set_nether_fortress_block(
                chunk,
                c_start,
                c_end,
                wx,
                floor_y + 2,
                BlockType::StoneBricks,
            );
        }

        for glow_x in [left + 1, center_x, right - 1] {
            Self::set_nether_fortress_block(
                chunk,
                c_start,
                c_end,
                glow_x,
                beam_y + 1,
                BlockType::Glowstone,
            );
        }

        for stair_x in [center_x - 2, center_x + 2] {
            Self::set_nether_fortress_block(
                chunk,
                c_start,
                c_end,
                stair_x,
                bridge_y,
                BlockType::StoneStairs,
            );
        }

        let chest_local_x = (center_x - c_start) as usize;
        chunk.set_block(chest_local_x, chest_y as usize, BlockType::Chest);
        Self::fill_nether_landmark_chest_loot(
            chunk,
            chest_local_x,
            chest_y as usize,
            seed ^ 0x41A7_6D53,
        );
    }

    fn carve_nether_route_reliquary(
        chunk: &mut Chunk,
        c_start: i32,
        c_end: i32,
        center_x: i32,
        ceiling_y: i32,
        floor_y: i32,
        seed: u32,
    ) {
        let top_y = (floor_y - 6).max(ceiling_y + 3);
        if top_y >= floor_y - 2 {
            return;
        }

        let half_width = 5;
        let left = center_x - half_width;
        let right = center_x + half_width;
        if left < c_start || right >= c_end {
            return;
        }

        let chest_side = if (seed & 1) == 0 { 2 } else { -2 };
        let chest_x = center_x + chest_side;
        let chest_y = floor_y - 2;

        for wx in left..=right {
            let dx = (wx - center_x).abs();
            let roof_y = top_y + (dx / 2).min(2);
            for wy in roof_y..=chest_y {
                let is_shell = wx == left || wx == right || wy == roof_y;
                let block = if is_shell {
                    BlockType::StoneBricks
                } else {
                    BlockType::Air
                };
                Self::set_nether_fortress_block(chunk, c_start, c_end, wx, wy, block);
            }
        }

        for wx in (center_x - 1)..=(center_x + 1) {
            Self::set_nether_fortress_block(
                chunk,
                c_start,
                c_end,
                wx,
                floor_y,
                BlockType::StoneBricks,
            );
            for wy in (floor_y - 4)..=chest_y {
                Self::set_nether_fortress_block(chunk, c_start, c_end, wx, wy, BlockType::Air);
            }
        }

        for brazier_x in [center_x - 3, center_x + 3] {
            Self::set_nether_fortress_block(
                chunk,
                c_start,
                c_end,
                brazier_x,
                chest_y,
                BlockType::StoneBricks,
            );
            Self::set_nether_fortress_block(
                chunk,
                c_start,
                c_end,
                brazier_x,
                chest_y - 1,
                BlockType::Glowstone,
            );
        }

        for stair_x in [center_x - 2, center_x + 2] {
            Self::set_nether_fortress_block(
                chunk,
                c_start,
                c_end,
                stair_x,
                chest_y,
                BlockType::StoneStairs,
            );
        }

        let chest_local_x = (chest_x - c_start) as usize;
        chunk.set_block(chest_local_x, chest_y as usize, BlockType::Chest);
        Self::fill_nether_landmark_chest_loot(
            chunk,
            chest_local_x,
            chest_y as usize,
            seed ^ 0x9E37_79B9,
        );

        for brazier_x in [left + 2, right - 2] {
            Self::carve_nether_lava_brazier(chunk, c_start, c_end, brazier_x, floor_y, 2);
        }
    }

    fn add_nether_route_landmarks(
        chunk: &mut Chunk,
        chunk_x: i32,
        perlin: &Perlin,
        ceiling_profile: &[i32],
        floor_profile: &[i32],
    ) {
        let c_start = chunk_x * CHUNK_WIDTH as i32;
        let c_end = c_start + CHUNK_WIDTH as i32;
        let start_cell =
            (c_start - NETHER_LANDMARK_CELL_WIDTH).div_euclid(NETHER_LANDMARK_CELL_WIDTH);
        let end_cell = (c_end + NETHER_LANDMARK_CELL_WIDTH).div_euclid(NETHER_LANDMARK_CELL_WIDTH);

        for cell in start_cell..=end_cell {
            let active = perlin.get([cell as f64 * 0.81, 430.0]);
            if active < 0.12 {
                continue;
            }

            let offset_noise = perlin.get([cell as f64 * 1.03, 431.0]);
            let center_x = cell * NETHER_LANDMARK_CELL_WIDTH
                + (NETHER_LANDMARK_CELL_WIDTH / 2)
                + (offset_noise * 8.0).round() as i32;
            if center_x <= c_start + 7 || center_x >= c_end - 7 {
                continue;
            }
            let region_chunk = center_x
                .div_euclid(CHUNK_WIDTH as i32)
                .div_euclid(NETHER_FORTRESS_REGION_CHUNKS);
            let landmark_fortress_layout = Self::nether_fortress_layout(perlin, region_chunk);
            if (center_x - landmark_fortress_layout.center_x).abs()
                <= NETHER_FORTRESS_HALF_SPAN_BLOCKS + 18
            {
                continue;
            }

            let local_x = (center_x - c_start) as usize;
            let ceiling_y = ceiling_profile[local_x];
            let floor_y = floor_profile[local_x];
            if floor_y - ceiling_y < NETHER_LANDMARK_MIN_CLEARANCE {
                continue;
            }

            let marker_seed = ((cell as i64)
                .wrapping_mul(1_103_515_245)
                .wrapping_add(12_345)
                .rem_euclid(u32::MAX as i64)) as u32;
            Self::carve_nether_route_landmark(
                chunk,
                c_start,
                c_end,
                center_x,
                ceiling_y,
                floor_y,
                marker_seed,
            );
        }
    }

    fn nether_ceiling_vault_lift(
        perlin: &Perlin,
        world_x: i32,
        fortress_layout: &NetherFortressLayout,
    ) -> i32 {
        let mut best_lift = 0;
        let cell = world_x.div_euclid(NETHER_CEILING_VAULT_CELL_WIDTH);
        for candidate in (cell - 1)..=(cell + 1) {
            let active = perlin.get([candidate as f64 * 0.63, 395.0]);
            if active < 0.04 {
                continue;
            }
            let offset_noise = perlin.get([candidate as f64 * 0.87, 396.0]);
            let radius_noise = perlin.get([candidate as f64 * 1.13, 397.0]);
            let depth_noise = perlin.get([candidate as f64 * 1.41, 398.0]);
            let center_x = candidate * NETHER_CEILING_VAULT_CELL_WIDTH
                + (NETHER_CEILING_VAULT_CELL_WIDTH / 2)
                + (offset_noise * 6.0).round() as i32;
            let radius = NETHER_CEILING_VAULT_MIN_RADIUS
                + ((((radius_noise + 1.0) * 0.5) * NETHER_CEILING_VAULT_RADIUS_SPAN as f64).round()
                    as i32)
                    .clamp(0, NETHER_CEILING_VAULT_RADIUS_SPAN);
            let dx = (world_x - center_x).abs();
            if dx > radius {
                continue;
            }

            let profile = 1.0 - dx as f64 / (radius.max(1) as f64 + 0.25);
            let mut lift = (profile
                * profile
                * (3.0 + ((depth_noise + 1.0) * 0.5) * NETHER_CEILING_VAULT_MAX_LIFT as f64))
                .round() as i32;
            if (world_x - fortress_layout.center_x).abs() <= NETHER_FORTRESS_HALF_SPAN_BLOCKS + 10 {
                lift = (lift * 2) / 3;
            }
            best_lift = best_lift.max(lift);
        }
        best_lift
    }

    fn nether_ceiling_intrusion_drop(
        perlin: &Perlin,
        world_x: i32,
        fortress_layout: &NetherFortressLayout,
    ) -> i32 {
        let mut best_drop = 0;
        let cell = world_x.div_euclid(NETHER_CEILING_INTRUSION_CELL_WIDTH);
        for candidate in (cell - 1)..=(cell + 1) {
            let active = perlin.get([candidate as f64 * 0.71, 399.0]);
            if active < 0.1 {
                continue;
            }
            let offset_noise = perlin.get([candidate as f64 * 0.97, 400.0]);
            let radius_noise = perlin.get([candidate as f64 * 1.27, 401.0]);
            let depth_noise = perlin.get([candidate as f64 * 1.51, 402.0]);
            let center_x = candidate * NETHER_CEILING_INTRUSION_CELL_WIDTH
                + (NETHER_CEILING_INTRUSION_CELL_WIDTH / 2)
                + (offset_noise * 4.0).round() as i32;
            let radius = NETHER_CEILING_INTRUSION_MIN_RADIUS
                + ((((radius_noise + 1.0) * 0.5) * NETHER_CEILING_INTRUSION_RADIUS_SPAN as f64)
                    .round() as i32)
                    .clamp(0, NETHER_CEILING_INTRUSION_RADIUS_SPAN);
            let dx = (world_x - center_x).abs();
            if dx > radius {
                continue;
            }

            let profile = 1.0 - dx as f64 / (radius.max(1) as f64 + 0.35);
            let mut drop = (profile
                * (2.0 + ((depth_noise + 1.0) * 0.5) * NETHER_CEILING_INTRUSION_MAX_DROP as f64))
                .round() as i32;
            if (world_x - fortress_layout.center_x).abs() <= NETHER_FORTRESS_HALF_SPAN_BLOCKS + 8 {
                drop = (drop * 2) / 3;
            }
            best_drop = best_drop.max(drop);
        }
        best_drop
    }

    fn add_nether_hanging_formations(
        chunk: &mut Chunk,
        chunk_x: i32,
        perlin: &Perlin,
        ceiling_profile: &[i32],
        floor_profile: &[i32],
        fortress_layout: &NetherFortressLayout,
    ) {
        let c_start = chunk_x * CHUNK_WIDTH as i32;
        for x in 2..(CHUNK_WIDTH - 2) {
            let world_x = c_start + x as i32;
            let fortress_dx = (world_x - fortress_layout.center_x).abs();
            if fortress_dx <= NETHER_FORTRESS_HALF_SPAN_BLOCKS + 28 {
                continue;
            }

            let ceiling_y = ceiling_profile[x];
            let floor_y = floor_profile[x];
            let open_height = floor_y - ceiling_y;
            if open_height < NETHER_HANGING_FORMATION_MIN_CLEARANCE + 6 {
                continue;
            }

            let density_noise = perlin.get([world_x as f64 * 0.051, 403.0]);
            if density_noise < 0.44 {
                continue;
            }

            let length_noise = perlin.get([world_x as f64 * 0.083, 404.0]);
            let width_noise = perlin.get([world_x as f64 * 0.067, 405.0]);
            let mut length = 3 + ((((length_noise + 1.0) * 0.5) * 5.0).round() as i32);
            length = length.min(open_height - NETHER_HANGING_FORMATION_MIN_CLEARANCE);
            if length < 3 {
                continue;
            }
            let half_width = if width_noise > 0.52 { 1 } else { 0 };
            let tip_y = ceiling_y + length;

            for dx in -half_width..=half_width {
                let nx = (x as i32 + dx) as usize;
                let taper = dx.abs();
                for wy in (ceiling_y + 1)..=(tip_y - taper) {
                    if chunk.get_block(nx, wy as usize) == BlockType::Air {
                        chunk.set_block(nx, wy as usize, BlockType::Netherrack);
                    }
                }
            }
            if density_noise > 0.78 && chunk.get_block(x, tip_y as usize) == BlockType::Netherrack {
                chunk.set_block(x, tip_y as usize, BlockType::Glowstone);
            }
        }
    }

    fn add_nether_perch_shelves(
        chunk: &mut Chunk,
        chunk_x: i32,
        perlin: &Perlin,
        ceiling_profile: &[i32],
        floor_profile: &[i32],
        fortress_layout: &NetherFortressLayout,
    ) {
        let c_start = chunk_x * CHUNK_WIDTH as i32;
        let left_world_x = c_start - NETHER_SHELF_CELL_WIDTH;
        let right_world_x = c_start + CHUNK_WIDTH as i32 + NETHER_SHELF_CELL_WIDTH;
        let start_cell = left_world_x.div_euclid(NETHER_SHELF_CELL_WIDTH);
        let end_cell = right_world_x.div_euclid(NETHER_SHELF_CELL_WIDTH);

        for cell in start_cell..=end_cell {
            let active = perlin.get([cell as f64 * 0.73, 406.0]);
            if active < 0.05 {
                continue;
            }

            let offset_noise = perlin.get([cell as f64 * 0.91, 407.0]);
            let span_noise = perlin.get([cell as f64 * 1.11, 408.0]);
            let height_noise = perlin.get([cell as f64 * 1.29, 409.0]);
            let center_x = cell * NETHER_SHELF_CELL_WIDTH
                + (NETHER_SHELF_CELL_WIDTH / 2)
                + (offset_noise * 7.0).round() as i32;
            if (center_x - fortress_layout.center_x).abs() <= NETHER_FORTRESS_HALF_SPAN_BLOCKS + 24
            {
                continue;
            }

            let half_span = 4 + ((((span_noise + 1.0) * 0.5) * 4.0).round() as i32);
            for world_x in (center_x - half_span)..=(center_x + half_span) {
                if world_x < c_start || world_x >= c_start + CHUNK_WIDTH as i32 {
                    continue;
                }
                let local_x = (world_x - c_start) as usize;
                let ceiling_y = ceiling_profile[local_x];
                let floor_y = floor_profile[local_x];
                let open_height = floor_y - ceiling_y;
                if open_height
                    < NETHER_SHELF_MIN_CLEARANCE_ABOVE + NETHER_SHELF_MIN_CLEARANCE_BELOW + 4
                {
                    continue;
                }

                let shelf_y = floor_y - (7 + ((((height_noise + 1.0) * 0.5) * 5.0).round() as i32));
                if shelf_y - ceiling_y < NETHER_SHELF_MIN_CLEARANCE_ABOVE
                    || floor_y - shelf_y < NETHER_SHELF_MIN_CLEARANCE_BELOW
                {
                    continue;
                }

                chunk.set_block(local_x, shelf_y as usize, BlockType::Netherrack);
                if (world_x - center_x).abs() <= 1 && active > 0.58 {
                    let under_y = shelf_y + 1;
                    if under_y < floor_y - 2
                        && chunk.get_block(local_x, under_y as usize) == BlockType::Air
                    {
                        chunk.set_block(local_x, under_y as usize, BlockType::Glowstone);
                    }
                }
            }
        }
    }

    fn add_nether_lava_vents(
        chunk: &mut Chunk,
        chunk_x: i32,
        perlin: &Perlin,
        ceiling_profile: &[i32],
        floor_profile: &[i32],
        fortress_layout: &NetherFortressLayout,
    ) {
        let c_start = chunk_x * CHUNK_WIDTH as i32;
        for x in 2..(CHUNK_WIDTH - 2) {
            let world_x = c_start + x as i32;
            let fortress_dx = (world_x - fortress_layout.center_x).abs();
            if fortress_dx <= NETHER_FORTRESS_HALF_SPAN_BLOCKS + 22 {
                continue;
            }

            let ceiling_y = ceiling_profile[x];
            let floor_y = floor_profile[x];
            if floor_y - ceiling_y < 18 {
                continue;
            }

            let active = perlin.get([world_x as f64 * 0.047, 416.0]);
            if active < 0.54 {
                continue;
            }

            let length_noise = perlin.get([world_x as f64 * 0.071, 417.0]);
            let vent_len = (2 + ((((length_noise + 1.0) * 0.5) * 3.0).round() as i32)).min(4);
            let max_tip_y = floor_y - 8;
            let tip_y = (ceiling_y + vent_len).min(max_tip_y);
            if tip_y <= ceiling_y + 1 {
                continue;
            }

            for wy in (ceiling_y + 1)..=tip_y {
                chunk.set_block(x, wy as usize, BlockType::Lava(8));
            }

            if active > 0.82 {
                for side_x in [x - 1, x + 1] {
                    if chunk.get_block(side_x, (ceiling_y + 1) as usize) == BlockType::Netherrack {
                        chunk.set_block(side_x, (ceiling_y + 1) as usize, BlockType::Lava(8));
                    }
                }
            }
        }
    }

    fn build_nether_fortress(chunk: &mut Chunk, chunk_x: i32, perlin: &Perlin) {
        let c_start = chunk_x * CHUNK_WIDTH as i32;
        let c_end = c_start + CHUNK_WIDTH as i32;
        let region_chunk = chunk_x.div_euclid(NETHER_FORTRESS_REGION_CHUNKS);
        let layout = Self::nether_fortress_layout(perlin, region_chunk);
        if c_end <= layout.left_x || c_start > layout.right_x {
            return;
        }

        for wx in layout.left_x..=layout.right_x {
            for wy in layout.roof_y..=layout.base_y {
                if wy == layout.base_y {
                    Self::set_nether_fortress_block(
                        chunk,
                        c_start,
                        c_end,
                        wx,
                        wy,
                        Self::nether_fortress_floor_block(wx, layout.center_x),
                    );
                } else if wy == layout.roof_y {
                    Self::set_nether_fortress_block(
                        chunk,
                        c_start,
                        c_end,
                        wx,
                        wy,
                        BlockType::StoneBricks,
                    );
                } else {
                    Self::set_nether_fortress_block(chunk, c_start, c_end, wx, wy, BlockType::Air);
                }
            }

            if (wx - layout.center_x).rem_euclid(11) == 0 {
                for support_y in
                    (layout.base_y + 1)..=(layout.base_y + 14).min(CHUNK_HEIGHT as i32 - 4)
                {
                    Self::set_nether_fortress_block(
                        chunk,
                        c_start,
                        c_end,
                        wx,
                        support_y,
                        BlockType::StoneBricks,
                    );
                }
            }
        }

        // Gatehouses and bastions make the fortress legible in a 2D slice while
        // keeping the main walking lane open.
        for gate_center_x in [
            layout.center_x - NETHER_FORTRESS_GATE_OFFSET_BLOCKS,
            layout.center_x,
            layout.center_x + NETHER_FORTRESS_GATE_OFFSET_BLOCKS,
        ] {
            let gate_half_width = if gate_center_x == layout.center_x {
                8
            } else {
                6
            };
            let gate_top = if gate_center_x == layout.center_x {
                layout.roof_y - 6
            } else {
                layout.roof_y - 5
            };
            let gate_arch_half_width = if gate_center_x == layout.center_x {
                5
            } else {
                4
            };
            Self::carve_nether_fortress_landmark(
                chunk,
                c_start,
                c_end,
                gate_center_x,
                gate_top,
                layout.base_y,
                gate_half_width,
                gate_arch_half_width,
                4,
            );

            for brazier_x in [
                gate_center_x - gate_half_width + 2,
                gate_center_x + gate_half_width - 2,
            ] {
                Self::carve_nether_lava_brazier(chunk, c_start, c_end, brazier_x, layout.base_y, 2);
            }
        }

        for tower_center_x in [
            layout.left_x + NETHER_FORTRESS_WATCHTOWER_INSET_BLOCKS,
            layout.right_x - NETHER_FORTRESS_WATCHTOWER_INSET_BLOCKS,
        ] {
            Self::carve_nether_fortress_watchtower(
                chunk,
                c_start,
                c_end,
                tower_center_x,
                layout.roof_y - 6,
                layout.base_y,
            );
        }

        // Side bastions hold blaze pressure off the main corridor instead of
        // spawning directly in the travel lane.
        for bastion_center_x in [
            layout.center_x - NETHER_FORTRESS_BASTION_OFFSET_BLOCKS,
            layout.center_x + NETHER_FORTRESS_BASTION_OFFSET_BLOCKS,
        ] {
            Self::carve_nether_fortress_landmark(
                chunk,
                c_start,
                c_end,
                bastion_center_x,
                layout.roof_y - 5,
                layout.base_y,
                6,
                3,
                4,
            );
            Self::set_nether_fortress_block(
                chunk,
                c_start,
                c_end,
                bastion_center_x,
                layout.base_y - 4,
                BlockType::BlazeSpawner,
            );
            for platform_x in [bastion_center_x - 2, bastion_center_x + 2] {
                Self::set_nether_fortress_block(
                    chunk,
                    c_start,
                    c_end,
                    platform_x,
                    layout.base_y - 3,
                    BlockType::StoneSlab,
                );
            }
        }

        // Side chapels break up long horizontal runs and make loot landmarks
        // clearer without choking the route.
        for chapel_center_x in [layout.chest_left_x, layout.chest_right_x] {
            Self::carve_nether_fortress_landmark(
                chunk,
                c_start,
                c_end,
                chapel_center_x,
                layout.roof_y - 4,
                layout.base_y,
                5,
                3,
                3,
            );

            for brazier_x in [chapel_center_x - 3, chapel_center_x + 3] {
                Self::carve_nether_lava_brazier(chunk, c_start, c_end, brazier_x, layout.base_y, 2);
            }
        }

        for edge_x in [layout.left_x + 6, layout.right_x - 6] {
            for wy in (layout.base_y - 2)..=(layout.base_y - 1) {
                Self::set_nether_fortress_block(chunk, c_start, c_end, edge_x, wy, BlockType::Air);
            }
            Self::set_nether_fortress_block(
                chunk,
                c_start,
                c_end,
                edge_x,
                layout.roof_y - 1,
                BlockType::Glowstone,
            );
        }

        for (chest_x, guaranteed_wart, salt) in [
            (layout.chest_left_x, true, 0xA5A5_5A5A),
            (layout.chest_right_x, false, 0x5A5A_A5A5),
        ] {
            let chest_y = layout.base_y - 3;
            Self::set_nether_fortress_block(
                chunk,
                c_start,
                c_end,
                chest_x,
                chest_y,
                BlockType::Chest,
            );
            for altar_x in [chest_x - 2, chest_x + 2] {
                Self::set_nether_fortress_block(
                    chunk,
                    c_start,
                    c_end,
                    altar_x,
                    chest_y,
                    BlockType::StoneSlab,
                );
            }
            if chest_x >= c_start && chest_x < c_end {
                let local_x = (chest_x - c_start) as usize;
                Self::fill_nether_fortress_chest_loot(
                    chunk,
                    local_x,
                    chest_y as usize,
                    layout.seed ^ salt ^ chest_x as u32,
                    guaranteed_wart,
                );
            }
        }
    }

    fn build_nether_chunk_with_noise(
        mut chunk: Chunk,
        chunk_x: i32,
        perlin: &Perlin,
        cave_perlin: &Perlin,
    ) -> Chunk {
        let c_start = chunk_x * CHUNK_WIDTH as i32;
        let fortress_layout =
            Self::nether_fortress_layout(perlin, chunk_x.div_euclid(NETHER_FORTRESS_REGION_CHUNKS));
        let mut previous_ceiling_y: Option<i32> = None;
        let mut previous_floor_y: Option<i32> = None;
        let mut ceiling_profile = vec![0i32; CHUNK_WIDTH];
        let mut floor_profile = vec![0i32; CHUNK_WIDTH];

        for x in 0..CHUNK_WIDTH {
            let world_x = c_start + x as i32;
            let mut ceiling_y = (NETHER_CAVERN_CEILING_BASE_Y as f64
                + perlin.get([world_x as f64 * 0.018, 390.0]) * NETHER_CAVERN_CEILING_VARIATION
                + perlin.get([world_x as f64 * 0.072, 391.0])
                    * NETHER_CAVERN_CEILING_DETAIL_VARIATION)
                .round() as i32;
            ceiling_y -= Self::nether_ceiling_vault_lift(perlin, world_x, &fortress_layout);
            ceiling_y += Self::nether_ceiling_intrusion_drop(perlin, world_x, &fortress_layout);
            let mut floor_y = (NETHER_CAVERN_FLOOR_BASE_Y as f64
                + perlin.get([world_x as f64 * 0.016, 392.0]) * NETHER_CAVERN_FLOOR_VARIATION
                + cave_perlin.get([world_x as f64 * 0.055, 393.0])
                    * NETHER_CAVERN_FLOOR_DETAIL_VARIATION)
                .round() as i32;
            let (chamber_lift, chamber_drop) = Self::nether_chamber_room_shape(perlin, world_x);
            ceiling_y -= chamber_lift;
            floor_y += chamber_drop;
            let fortress_dx = (world_x - fortress_layout.center_x).abs();
            if fortress_dx <= NETHER_FORTRESS_HALF_SPAN_BLOCKS + 48 {
                let fortress_floor_target = fortress_layout.base_y
                    + (((fortress_dx.saturating_sub(10)) as f64) / 28.0).round() as i32;
                floor_y = floor_y.min(
                    fortress_floor_target.clamp(fortress_layout.base_y, fortress_layout.base_y + 5),
                );
                let fortress_ceiling_target = fortress_layout.roof_y - 7
                    + (((fortress_dx.saturating_sub(16)) as f64) / 38.0).round() as i32;
                ceiling_y = ceiling_y
                    .min(fortress_ceiling_target.clamp(6, (fortress_layout.roof_y - 1).max(6)));
            }

            ceiling_y = ceiling_y.clamp(10, 30);
            floor_y = floor_y.clamp(42, 86);
            if let Some(previous) = previous_ceiling_y {
                ceiling_y = ceiling_y.clamp(previous - 3, previous + 3);
            }
            if let Some(previous) = previous_floor_y {
                floor_y = floor_y.clamp(previous - 2, previous + 2);
            }
            if floor_y - ceiling_y < NETHER_CAVERN_MIN_OPEN_HEIGHT {
                floor_y = (ceiling_y + NETHER_CAVERN_MIN_OPEN_HEIGHT).min(86);
            }
            previous_ceiling_y = Some(ceiling_y);
            previous_floor_y = Some(floor_y);
            ceiling_profile[x] = ceiling_y;
            floor_profile[x] = floor_y;

            let lava_sea_y = ((NETHER_LAVA_SEA_BASE_Y as f64
                + perlin.get([world_x as f64 * 0.019, 394.0]) * NETHER_LAVA_SEA_VARIATION)
                .round() as i32)
                .clamp(100, 118);
            let fortress_clear_band = fortress_dx <= NETHER_FORTRESS_HALF_SPAN_BLOCKS + 44;

            for y in 0..CHUNK_HEIGHT {
                let current_y = y as i32;
                if current_y <= 1 || current_y >= (CHUNK_HEIGHT as i32 - 2) {
                    chunk.set_block(x, y, BlockType::Bedrock);
                    continue;
                }

                if current_y >= lava_sea_y {
                    chunk.set_block(x, y, BlockType::Lava(8));
                    continue;
                }

                if current_y > ceiling_y && current_y < floor_y {
                    chunk.set_block(x, y, BlockType::Air);
                    continue;
                }

                let gravel_noise =
                    perlin.get([world_x as f64 * 0.09, current_y as f64 * 0.06, 350.0]);
                let soul_sand_noise =
                    perlin.get([world_x as f64 * 0.08, current_y as f64 * 0.05, 360.0]);
                let gravel_patch = perlin.get([world_x as f64 * 0.03, 351.0]);
                let soul_sand_patch = perlin.get([world_x as f64 * 0.028, 361.0]);
                let is_floor_band = current_y >= floor_y && current_y <= floor_y + 1;
                if is_floor_band
                    && !fortress_clear_band
                    && soul_sand_patch > 0.18
                    && soul_sand_noise > -0.05
                {
                    chunk.set_block(x, y, BlockType::SoulSand);
                } else if is_floor_band
                    && !fortress_clear_band
                    && gravel_patch > 0.24
                    && gravel_noise > -0.08
                {
                    chunk.set_block(x, y, BlockType::Gravel);
                } else {
                    chunk.set_block(x, y, BlockType::Netherrack);
                }
            }
        }

        Self::add_nether_hanging_formations(
            &mut chunk,
            chunk_x,
            perlin,
            &ceiling_profile,
            &floor_profile,
            &fortress_layout,
        );
        Self::add_nether_perch_shelves(
            &mut chunk,
            chunk_x,
            perlin,
            &ceiling_profile,
            &floor_profile,
            &fortress_layout,
        );
        Self::add_nether_lava_vents(
            &mut chunk,
            chunk_x,
            perlin,
            &ceiling_profile,
            &floor_profile,
            &fortress_layout,
        );

        // Deterministic glowstone clusters embedded in upper netherrack bands.
        for x in 1..(CHUNK_WIDTH - 1) {
            let world_x = chunk_x * CHUNK_WIDTH as i32 + x as i32;
            let hash = (world_x as i64)
                .wrapping_mul(1103515245)
                .wrapping_add(12345)
                .rem_euclid(97) as i32;
            if hash >= 9 {
                continue;
            }

            let anchor_y = 8 + ((hash * 13 + chunk_x.rem_euclid(17)) % 20);
            let ay = anchor_y as usize;
            if chunk.get_block(x, ay) != BlockType::Netherrack {
                continue;
            }
            chunk.set_block(x, ay, BlockType::Glowstone);

            for (dx, dy) in [(-1, 0), (1, 0), (0, 1), (0, -1)] {
                let nx = (x as i32 + dx) as usize;
                let ny = (anchor_y + dy) as usize;
                if chunk.get_block(nx, ny) == BlockType::Netherrack && ((hash + dx + dy) & 1) == 0 {
                    chunk.set_block(nx, ny, BlockType::Glowstone);
                }
            }
        }
        Self::add_nether_route_landmarks(
            &mut chunk,
            chunk_x,
            perlin,
            &ceiling_profile,
            &floor_profile,
        );
        Self::build_nether_fortress(&mut chunk, chunk_x, perlin);
        chunk
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::block::BlockType;
    use crate::world::item::ItemType;

    fn deterministic_generated_world(
        dimension: Dimension,
        save_key: &str,
        chunk_start: i32,
        chunk_end: i32,
    ) -> World {
        let mut world = World::new_for_dimension(dimension);
        world.chunks.clear();
        world.newly_generated_chunks.clear();
        world.pending_chunks.clear();
        world.queued_chunks.clear();
        world.in_flight_chunks.clear();
        world.chunk_request_queue.clear();
        world.active_fluid_chunks.clear();

        world.perlin = Perlin::new(1337);
        world.temp_perlin = Perlin::new(7331);
        world.moist_perlin = Perlin::new(9917);
        world.cave_perlin = Perlin::new(17_711);

        for chunk_x in chunk_start..=chunk_end {
            let chunk = World::build_chunk_with_noise(
                chunk_x,
                dimension,
                save_key,
                &world.perlin,
                &world.temp_perlin,
                &world.moist_perlin,
                &world.cave_perlin,
            );
            world.chunks.insert(chunk_x, chunk);
        }

        world
    }

    fn nether_walkable_surface_y(world: &World, x: i32) -> Option<i32> {
        for y in (2..(CHUNK_HEIGHT as i32 - 2)).rev() {
            let ground = world.get_block(x, y);
            if matches!(
                ground,
                BlockType::Netherrack
                    | BlockType::SoulSand
                    | BlockType::Gravel
                    | BlockType::Glowstone
                    | BlockType::StoneBricks
                    | BlockType::StoneSlab
                    | BlockType::StoneStairs
            ) && world.get_block(x, y - 1) == BlockType::Air
                && world.get_block(x, y - 2) == BlockType::Air
            {
                return Some(y);
            }
        }
        None
    }

    fn nether_highest_walkable_surface_y(world: &World, x: i32) -> Option<i32> {
        for y in 2..(CHUNK_HEIGHT as i32 - 2) {
            let ground = world.get_block(x, y);
            if matches!(
                ground,
                BlockType::Netherrack
                    | BlockType::SoulSand
                    | BlockType::Gravel
                    | BlockType::Glowstone
                    | BlockType::StoneBricks
                    | BlockType::StoneSlab
                    | BlockType::StoneStairs
            ) && world.get_block(x, y - 1) == BlockType::Air
                && world.get_block(x, y - 2) == BlockType::Air
            {
                return Some(y);
            }
        }
        None
    }

    fn nether_ceiling_surface_y(world: &World, x: i32) -> Option<i32> {
        for y in 2..(CHUNK_HEIGHT as i32 - 3) {
            let block = world.get_block(x, y);
            if block.is_solid() && world.get_block(x, y + 1) == BlockType::Air {
                return Some(y);
            }
        }
        None
    }

    fn sampled_overworld_cave_air_ratio(
        target_biome: BiomeType,
        perlin: &Perlin,
        temp_perlin: &Perlin,
        moist_perlin: &Perlin,
        cave_perlin: &Perlin,
    ) -> f64 {
        let mut sampled_chunks = 0;
        let mut air_blocks = 0usize;
        let mut sampled_blocks = 0usize;

        for chunk_x in -220i32..=220 {
            if chunk_x.rem_euclid(OVERWORLD_DUNGEON_CHUNK_CADENCE) == OVERWORLD_DUNGEON_CHUNK_PHASE
                || chunk_x.rem_euclid(11) == 5
            {
                continue;
            }

            let center_x = chunk_x * CHUNK_WIDTH as i32 + CHUNK_WIDTH as i32 / 2;
            if World::biome_for_x(temp_perlin, moist_perlin, center_x) != target_biome {
                continue;
            }

            let chunk = World::build_chunk_with_noise(
                chunk_x,
                Dimension::Overworld,
                "test_sampled_overworld_cave_air_ratio",
                perlin,
                temp_perlin,
                moist_perlin,
                cave_perlin,
            );
            sampled_chunks += 1;

            let c_start = chunk_x * CHUNK_WIDTH as i32;
            for lx in 0..CHUNK_WIDTH {
                let wx = c_start + lx as i32;
                let biome = World::biome_for_x(temp_perlin, moist_perlin, wx);
                if biome != target_biome {
                    continue;
                }

                let surface_y = World::blended_overworld_surface_y(
                    perlin,
                    temp_perlin,
                    moist_perlin,
                    wx,
                    biome,
                );
                if World::column_has_surface_water(
                    perlin,
                    temp_perlin,
                    moist_perlin,
                    wx,
                    biome,
                    surface_y,
                ) {
                    continue;
                }

                let min_y = surface_y + OVERWORLD_CHAMBER_MIN_SURFACE_DEPTH;
                let max_y = (surface_y + 36).min(CHUNK_HEIGHT as i32 - 12);
                if min_y >= max_y {
                    continue;
                }

                for y in min_y..=max_y {
                    sampled_blocks += 1;
                    if chunk.get_block(lx, y as usize) == BlockType::Air {
                        air_blocks += 1;
                    }
                }
            }

            if sampled_chunks >= 6 {
                break;
            }
        }

        assert!(sampled_chunks >= 4);
        assert!(sampled_blocks > 0);
        air_blocks as f64 / sampled_blocks as f64
    }

    fn solid_test_chunk(save_key: &str) -> Chunk {
        let mut chunk = Chunk::new(0, save_key);
        for x in 0..CHUNK_WIDTH {
            for y in 0..CHUNK_HEIGHT {
                chunk.set_block(x, y, BlockType::Stone);
            }
        }
        chunk
    }

    fn flat_cave_surface_profile(surface_y: i32) -> ([i32; CHUNK_WIDTH], [bool; CHUNK_WIDTH]) {
        ([surface_y; CHUNK_WIDTH], [false; CHUNK_WIDTH])
    }

    #[test]
    fn test_chunk_request_priority_prefers_nearer_chunks() {
        let mut queue = BinaryHeap::new();
        queue.push(ChunkRequest {
            chunk_x: 20,
            distance: 6,
            seq: 0,
        });
        queue.push(ChunkRequest {
            chunk_x: 4,
            distance: 2,
            seq: 1,
        });
        queue.push(ChunkRequest {
            chunk_x: -2,
            distance: 3,
            seq: 2,
        });

        let first = queue.pop().expect("priority queue should not be empty");
        assert_eq!(first.chunk_x, 4);
    }

    #[test]
    fn test_chunk_request_backpressure_caps_pending_queue() {
        let mut world = World::new();
        for x in 0..(CHUNK_PIPELINE_MAX_PENDING as i32 + 16) {
            world.enqueue_chunk_request(3000 + x, 0);
        }
        assert_eq!(world.pending_chunks.len(), CHUNK_PIPELINE_MAX_PENDING);
        assert_eq!(world.queued_chunks.len(), CHUNK_PIPELINE_MAX_PENDING);
    }

    #[test]
    fn test_trim_stale_chunk_requests_compacts_heap() {
        let mut world = World::new();
        world.enqueue_chunk_request(1, 0);
        world.enqueue_chunk_request(2, 0);
        world.enqueue_chunk_request(100, 0);
        assert_eq!(world.chunk_request_queue.len(), 3);

        world.trim_stale_chunk_requests(0);

        assert!(world.queued_chunks.contains(&1));
        assert!(world.queued_chunks.contains(&2));
        assert!(!world.queued_chunks.contains(&100));
        assert_eq!(world.pending_chunks.len(), 2);
        assert_eq!(world.chunk_request_queue.len(), 2);
        assert!(world.chunk_request_queue.iter().all(|r| r.chunk_x != 100));
    }

    #[test]
    fn test_trim_stale_chunk_requests_deduplicates_chunk_entries() {
        let mut world = World::new();
        world.enqueue_chunk_request(1, 0);
        world.chunk_request_queue.push(ChunkRequest {
            chunk_x: 1,
            distance: 1,
            seq: 0,
        });
        assert_eq!(world.chunk_request_queue.len(), 2);
        assert_eq!(world.queued_chunks.len(), 1);

        world.trim_stale_chunk_requests(0);

        assert_eq!(world.chunk_request_queue.len(), 1);
        assert_eq!(
            world
                .chunk_request_queue
                .iter()
                .filter(|request| request.chunk_x == 1)
                .count(),
            1
        );
    }

    #[test]
    fn test_world_seed_data_persists_for_save_key() {
        let save_key = "seedpersist_test";
        let path = World::world_seed_path(save_key);
        let _ = std::fs::remove_file(&path);

        let first = World::load_or_create_world_seed_data(save_key);
        let second = World::load_or_create_world_seed_data(save_key);
        assert_eq!(first.terrain_seed, second.terrain_seed);
        assert_eq!(first.temp_seed, second.temp_seed);
        assert_eq!(first.moist_seed, second.moist_seed);
        assert_eq!(first.cave_seed, second.cave_seed);

        let loaded = World::load_world_seed_data(save_key).expect("seed file should load");
        assert_eq!(loaded.terrain_seed, first.terrain_seed);
        assert_eq!(loaded.temp_seed, first.temp_seed);
        assert_eq!(loaded.moist_seed, first.moist_seed);
        assert_eq!(loaded.cave_seed, first.cave_seed);

        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn test_fluid_flow_down() {
        let mut world = World::new();
        world.load_chunks_around(0);

        // Clear space
        for x in 0..5 {
            for y in 0..10 {
                world.set_block(x, y, BlockType::Air);
            }
        }

        // Place water source at (2, 2)
        world.set_block(2, 2, BlockType::Water(8));

        world.update_fluids(0);

        // Should flow down to (2, 3) as level 7
        assert_eq!(world.get_block(2, 3), BlockType::Water(7));
        // Source should remain
        assert_eq!(world.get_block(2, 2), BlockType::Water(8));
    }

    #[test]
    fn test_fluid_flow_horizontal() {
        let mut world = World::new();
        world.load_chunks_around(0);

        // Clear space and place solid floor
        for x in 0..5 {
            for y in 0..5 {
                world.set_block(x, y, BlockType::Air);
            }
            world.set_block(x, 3, BlockType::Stone);
        }

        // Place water source at (2, 2) on floor
        world.set_block(2, 2, BlockType::Water(8));

        world.update_fluids(0);

        // Should flow sideways to (1, 2) and (3, 2)
        assert_eq!(world.get_block(1, 2), BlockType::Water(7));
        assert_eq!(world.get_block(3, 2), BlockType::Water(7));
        assert_eq!(world.get_block(2, 2), BlockType::Water(8));
    }

    #[test]
    fn test_water_source_regenerates_between_two_sources() {
        let mut world = World::new();
        world.load_chunks_around(0);

        for x in 0..5 {
            for y in 0..6 {
                world.set_block(x, y, BlockType::Air);
            }
            world.set_block(x, 3, BlockType::Stone);
        }
        world.set_block(1, 2, BlockType::Water(8));
        world.set_block(3, 2, BlockType::Water(8));

        world.update_fluids(0);

        assert_eq!(world.get_block(2, 2), BlockType::Water(8));
    }

    #[test]
    fn test_water_contact_with_lava_source_creates_obsidian() {
        let mut world = World::new();
        world.load_chunks_around(0);

        for x in 0..5 {
            for y in 0..6 {
                world.set_block(x, y, BlockType::Air);
            }
        }

        world.set_block(2, 2, BlockType::Water(8));
        world.set_block(2, 3, BlockType::Lava(8));

        world.update_fluids(0);

        assert_eq!(world.get_block(2, 3), BlockType::Obsidian);
    }

    #[test]
    fn test_water_contact_with_flowing_lava_creates_cobblestone() {
        let mut world = World::new();
        world.load_chunks_around(0);

        for x in 0..5 {
            for y in 0..6 {
                world.set_block(x, y, BlockType::Air);
            }
        }

        world.set_block(2, 2, BlockType::Water(8));
        world.set_block(2, 3, BlockType::Lava(7));

        world.update_fluids(0);

        assert_eq!(world.get_block(2, 3), BlockType::Cobblestone);
    }

    #[test]
    fn test_lava_falling_into_water_creates_stone() {
        let mut world = World::new();
        world.load_chunks_around(0);

        for x in 0..5 {
            for y in 0..6 {
                world.set_block(x, y, BlockType::Air);
            }
        }

        world.set_block(2, 2, BlockType::Lava(8));
        world.set_block(2, 3, BlockType::Water(8));

        world.update_fluids(0);

        assert_eq!(world.get_block(2, 3), BlockType::Stone);
    }

    #[test]
    fn test_overworld_lava_horizontal_spread_stops_after_three_blocks() {
        let mut world = World::new();
        world.load_chunks_around(0);

        for x in 0..9 {
            for y in 0..5 {
                world.set_block(x, y, BlockType::Air);
            }
            world.set_block(x, 3, BlockType::Stone);
        }

        world.set_block(3, 2, BlockType::Lava(8));

        for _ in 0..4 {
            world.update_fluids(0);
        }

        assert_eq!(world.get_block(0, 2), BlockType::Lava(5));
        assert_eq!(world.get_block(6, 2), BlockType::Lava(5));
        assert_eq!(world.get_block(7, 2), BlockType::Air);
    }

    #[test]
    fn test_world_update_uses_slow_overworld_lava_cadence() {
        let mut world = World::new();
        world.load_chunks_around(0);

        for x in 0..6 {
            for y in 0..5 {
                world.set_block(x, y, BlockType::Air);
            }
            world.set_block(x, 3, BlockType::Stone);
        }

        world.set_block(2, 2, BlockType::Lava(8));

        for _ in 0..5 {
            world.update(0);
        }
        assert_eq!(world.get_block(1, 2), BlockType::Air);
        assert_eq!(world.get_block(3, 2), BlockType::Air);

        for _ in 5..30 {
            world.update(0);
        }
        assert_eq!(world.get_block(1, 2), BlockType::Lava(7));
        assert_eq!(world.get_block(3, 2), BlockType::Lava(7));
    }

    #[test]
    fn test_stable_fluid_chunk_drops_out_of_active_frontier() {
        let mut world = World::new();
        world.load_chunks_around(0);

        for x in -64..=64 {
            for y in 0..CHUNK_HEIGHT as i32 {
                world.set_block(x, y, BlockType::Air);
            }
        }
        world.active_fluid_chunks.clear();

        world.set_block(1, 2, BlockType::Stone);
        world.set_block(3, 2, BlockType::Stone);
        world.set_block(2, 3, BlockType::Stone);
        world.set_block(2, 2, BlockType::Water(8));

        assert!(world.active_fluid_chunks.contains(&0));

        world.update_fluids(0);

        assert!(!world.active_fluid_chunks.contains(&0));
        assert_eq!(world.get_block(2, 2), BlockType::Water(8));
    }

    #[test]
    fn test_block_change_adjacent_to_static_water_reactivates_frontier() {
        let mut world = World::new();
        world.load_chunks_around(0);

        for x in -64..=64 {
            for y in 0..CHUNK_HEIGHT as i32 {
                world.set_block(x, y, BlockType::Air);
            }
        }
        world.active_fluid_chunks.clear();

        world.set_block(1, 2, BlockType::Stone);
        world.set_block(3, 2, BlockType::Stone);
        world.set_block(2, 3, BlockType::Stone);
        world.set_block(2, 2, BlockType::Water(8));
        world.update_fluids(0);
        assert!(!world.active_fluid_chunks.contains(&0));

        world.set_block(3, 2, BlockType::Air);
        assert!(world.active_fluid_chunks.contains(&0));

        world.update_fluids(0);

        assert_eq!(world.get_block(3, 2), BlockType::Water(7));
    }

    #[test]
    fn test_fluid_decay() {
        let mut world = World::new();
        world.load_chunks_around(0);

        // Setup flowing water
        world.set_block(2, 2, BlockType::Water(7)); // No source block above or beside

        world.update_fluids(0);

        // Should decay to 6
        assert_eq!(world.get_block(2, 2), BlockType::Water(6));

        for _ in 0..6 {
            world.update_fluids(0);
        }

        // Should be completely gone
        assert_eq!(world.get_block(2, 2), BlockType::Air);
    }

    #[test]
    fn test_nether_generation_contains_netherrack() {
        let mut world = World::new_for_dimension(Dimension::Nether);
        world.load_chunks_around(0);

        let mut found_netherrack = false;
        for x in -16..=16 {
            for y in 2..120 {
                if world.get_block(x, y) == BlockType::Netherrack {
                    found_netherrack = true;
                    break;
                }
            }
            if found_netherrack {
                break;
            }
        }
        assert!(found_netherrack);
    }

    #[test]
    fn test_nether_generation_contains_glowstone() {
        let mut world = World::new_for_dimension(Dimension::Nether);
        world.load_chunks_around(0);

        let mut found_glowstone = false;
        for x in -64..=64 {
            for y in 2..80 {
                if world.get_block(x, y) == BlockType::Glowstone {
                    found_glowstone = true;
                    break;
                }
            }
            if found_glowstone {
                break;
            }
        }
        assert!(found_glowstone);
    }

    #[test]
    fn test_nether_generation_contains_soul_sand() {
        let mut world = World::new_for_dimension(Dimension::Nether);
        for center_x in (-128..=128).step_by(32) {
            world.load_chunks_around(center_x);
        }

        let mut found_soul_sand = false;
        for x in -160..=160 {
            for y in 2..120 {
                if world.get_block(x, y) == BlockType::SoulSand {
                    found_soul_sand = true;
                    break;
                }
            }
            if found_soul_sand {
                break;
            }
        }
        assert!(found_soul_sand);
    }

    #[test]
    fn test_nether_generation_builds_fortress_blocks_and_wart_chest() {
        let mut world = World::new_for_dimension(Dimension::Nether);

        let mut fortress_left = None;
        let mut fortress_right = None;
        for x in 0..(NETHER_FORTRESS_REGION_CHUNKS * CHUNK_WIDTH as i32) {
            let mut in_zone = false;
            for y in 16..96 {
                if world.is_nether_fortress_zone(x, y) {
                    in_zone = true;
                    break;
                }
            }
            if in_zone {
                fortress_left.get_or_insert(x);
                fortress_right = Some(x);
            }
        }

        let fortress_left = fortress_left.expect("nether fortress zone should exist");
        let fortress_right = fortress_right.expect("nether fortress zone should exist");
        let fortress_center_x = (fortress_left + fortress_right) / 2;
        for sample_x in [fortress_left, fortress_center_x, fortress_right] {
            world.load_chunks_around(sample_x);
        }

        let mut found_fortress_block = false;
        let mut found_wart_chest = false;
        let mut found_blaze_spawner = false;
        for x in fortress_left..=fortress_right {
            for y in 8..112 {
                match world.get_block(x, y) {
                    BlockType::StoneBricks | BlockType::StoneSlab | BlockType::StoneStairs => {
                        found_fortress_block = true;
                    }
                    BlockType::Chest => {
                        if world
                            .chest_inventory(x, y)
                            .is_some_and(|inv| inv.has_item(ItemType::NetherWart, 1))
                        {
                            found_wart_chest = true;
                        }
                    }
                    BlockType::BlazeSpawner => found_blaze_spawner = true,
                    _ => {}
                }
                if found_fortress_block && found_wart_chest && found_blaze_spawner {
                    break;
                }
            }
            if found_fortress_block && found_wart_chest && found_blaze_spawner {
                break;
            }
        }

        assert!(found_fortress_block);
        assert!(found_wart_chest);
        assert!(found_blaze_spawner);
    }

    #[test]
    fn test_nether_fortress_chests_do_not_block_main_corridor() {
        let mut world = World::new_for_dimension(Dimension::Nether);
        let layout = World::nether_fortress_layout(&world.perlin, 0);
        for sample_x in [layout.left_x, layout.center_x, layout.right_x] {
            world.load_chunks_around(sample_x);
        }

        for chest_x in [layout.chest_left_x, layout.chest_right_x] {
            assert_eq!(
                world.get_block(chest_x, layout.base_y),
                BlockType::StoneBricks
            );
            assert_eq!(
                world.get_block(chest_x, layout.base_y - 3),
                BlockType::Chest
            );
            assert_eq!(world.get_block(chest_x, layout.base_y - 1), BlockType::Air);
            assert_eq!(world.get_block(chest_x, layout.base_y - 2), BlockType::Air);
        }
    }

    #[test]
    fn test_nether_fortress_bastion_spawners_are_raised_off_the_main_lane() {
        let mut world = World::new_for_dimension(Dimension::Nether);
        let layout = World::nether_fortress_layout(&world.perlin, 0);
        for sample_x in [layout.center_x - 48, layout.center_x + 48] {
            world.load_chunks_around(sample_x);
        }

        for spawner_x in [
            layout.center_x - NETHER_FORTRESS_BASTION_OFFSET_BLOCKS,
            layout.center_x + NETHER_FORTRESS_BASTION_OFFSET_BLOCKS,
        ] {
            assert_eq!(
                world.get_block(spawner_x, layout.base_y - 4),
                BlockType::BlazeSpawner
            );
            assert_eq!(
                world.get_block(spawner_x, layout.base_y - 1),
                BlockType::Air
            );
            assert_eq!(
                world.get_block(spawner_x - 2, layout.base_y - 3),
                BlockType::StoneSlab
            );
            assert_eq!(
                world.get_block(spawner_x + 2, layout.base_y - 3),
                BlockType::StoneSlab
            );
        }
    }

    #[test]
    fn test_nether_fortress_gatehouses_place_glowstone_landmarks() {
        let mut world = World::new_for_dimension(Dimension::Nether);
        let layout = World::nether_fortress_layout(&world.perlin, 0);
        for sample_x in [
            layout.center_x - NETHER_FORTRESS_GATE_OFFSET_BLOCKS,
            layout.center_x,
            layout.center_x + NETHER_FORTRESS_GATE_OFFSET_BLOCKS,
        ] {
            world.load_chunks_around(sample_x);
        }

        for gate_x in [
            layout.center_x - NETHER_FORTRESS_GATE_OFFSET_BLOCKS,
            layout.center_x,
            layout.center_x + NETHER_FORTRESS_GATE_OFFSET_BLOCKS,
        ] {
            let mut found_glowstone = false;
            for x in (gate_x - 2)..=(gate_x + 2) {
                for y in (layout.roof_y - 7)..=(layout.roof_y - 2) {
                    if world.get_block(x, y) == BlockType::Glowstone {
                        found_glowstone = true;
                        break;
                    }
                }
                if found_glowstone {
                    break;
                }
            }
            assert!(
                found_glowstone,
                "expected glowstone landmark near gate {gate_x}"
            );
        }
    }

    #[test]
    fn test_nether_fortress_gatehouses_place_lava_braziers_off_the_main_lane() {
        let mut world = World::new_for_dimension(Dimension::Nether);
        let layout = World::nether_fortress_layout(&world.perlin, 0);
        for sample_x in [
            layout.center_x - NETHER_FORTRESS_GATE_OFFSET_BLOCKS,
            layout.center_x,
            layout.center_x + NETHER_FORTRESS_GATE_OFFSET_BLOCKS,
        ] {
            world.load_chunks_around(sample_x);
        }

        let mut brazier_pairs = 0usize;
        for (gate_x, gate_half_width) in [
            (layout.center_x - NETHER_FORTRESS_GATE_OFFSET_BLOCKS, 6),
            (layout.center_x, 8),
            (layout.center_x + NETHER_FORTRESS_GATE_OFFSET_BLOCKS, 6),
        ] {
            if world.get_block(gate_x - gate_half_width + 2, layout.base_y + 1)
                == BlockType::Lava(8)
                && world.get_block(gate_x + gate_half_width - 2, layout.base_y + 1)
                    == BlockType::Lava(8)
            {
                brazier_pairs += 1;
            }
            assert_eq!(world.get_block(gate_x, layout.base_y - 1), BlockType::Air);
        }

        assert!(
            brazier_pairs >= 1,
            "expected at least one lava brazier pair"
        );
    }

    #[test]
    fn test_nether_generation_places_route_landmark_chests_outside_fortress() {
        let perlin = Perlin::new(1337);
        let temp_perlin = Perlin::new(7331);
        let moist_perlin = Perlin::new(9917);
        let cave_perlin = Perlin::new(17_711);

        let mut found_landmark = false;
        'chunks: for chunk_x in -18..=18 {
            let chunk = World::build_chunk_with_noise(
                chunk_x,
                Dimension::Nether,
                "test_nether_generation_places_route_landmark_chests_outside_fortress",
                &perlin,
                &temp_perlin,
                &moist_perlin,
                &cave_perlin,
            );
            let c_start = chunk_x * CHUNK_WIDTH as i32;

            for lx in 0..CHUNK_WIDTH {
                for y in 8..100usize {
                    if chunk.get_block(lx, y) != BlockType::Chest {
                        continue;
                    }

                    let world_x = c_start + lx as i32;
                    let world_y = y as i32;
                    let region_chunk = world_x
                        .div_euclid(CHUNK_WIDTH as i32)
                        .div_euclid(NETHER_FORTRESS_REGION_CHUNKS);
                    let layout = World::nether_fortress_layout(&perlin, region_chunk);
                    let in_fortress = world_x >= layout.left_x
                        && world_x <= layout.right_x
                        && world_y >= layout.roof_y - 5
                        && world_y <= layout.base_y + 2;
                    if in_fortress {
                        continue;
                    }

                    let mut near_fortress_blocks = false;
                    let mut near_glowstone = false;
                    for nx in lx.saturating_sub(6)..=(lx + 6).min(CHUNK_WIDTH - 1) {
                        for ny in y.saturating_sub(6)..=(y + 2).min(CHUNK_HEIGHT - 1) {
                            match chunk.get_block(nx, ny) {
                                BlockType::StoneBricks
                                | BlockType::StoneSlab
                                | BlockType::StoneStairs => {
                                    near_fortress_blocks = true;
                                }
                                BlockType::Glowstone => near_glowstone = true,
                                _ => {}
                            }
                        }
                    }

                    if near_fortress_blocks && near_glowstone {
                        found_landmark = true;
                        break 'chunks;
                    }
                }
            }
        }

        assert!(found_landmark);
    }

    #[test]
    fn test_nether_generation_contains_lava_features_above_the_lava_sea() {
        let world = deterministic_generated_world(
            Dimension::Nether,
            "test_nether_generation_contains_lava_features_above_the_lava_sea",
            -10,
            10,
        );

        let mut found_route_lava = false;
        for x in -160..=160 {
            for y in 20..(NETHER_LAVA_SEA_BASE_Y - 6) {
                if world.get_block(x, y) == BlockType::Lava(8) {
                    found_route_lava = true;
                    break;
                }
            }
            if found_route_lava {
                break;
            }
        }

        assert!(found_route_lava);
    }

    #[test]
    fn test_nether_route_landmark_variants_leave_distinct_signatures() {
        let center_x = (CHUNK_WIDTH / 2) as i32;
        let floor_y = 72;
        let ceiling_y = 46;

        let mut shrine_chunk = Chunk::new(0, "nether_route_shrine");
        World::carve_nether_route_landmark(
            &mut shrine_chunk,
            0,
            CHUNK_WIDTH as i32,
            center_x,
            ceiling_y,
            floor_y,
            0,
        );
        assert_eq!(
            shrine_chunk.get_block((center_x + 4) as usize, (floor_y - 1) as usize),
            BlockType::Chest
        );

        let mut bridge_chunk = Chunk::new(0, "nether_route_bridge");
        World::carve_nether_route_landmark(
            &mut bridge_chunk,
            0,
            CHUNK_WIDTH as i32,
            center_x,
            ceiling_y,
            floor_y,
            1,
        );
        assert_eq!(
            bridge_chunk.get_block(center_x as usize, (floor_y - 2) as usize),
            BlockType::Chest
        );
        assert_eq!(
            bridge_chunk.get_block(center_x as usize, (floor_y - 1) as usize),
            BlockType::StoneBricks
        );
        assert_eq!(
            bridge_chunk.get_block(center_x as usize, (floor_y + 1) as usize),
            BlockType::Lava(8)
        );

        let mut reliquary_chunk = Chunk::new(0, "nether_route_reliquary");
        World::carve_nether_route_landmark(
            &mut reliquary_chunk,
            0,
            CHUNK_WIDTH as i32,
            center_x,
            ceiling_y,
            floor_y,
            2,
        );
        assert_eq!(
            reliquary_chunk.get_block((center_x + 2) as usize, (floor_y - 2) as usize),
            BlockType::Chest
        );
        assert_eq!(
            reliquary_chunk.get_block((center_x - 3) as usize, (floor_y - 3) as usize),
            BlockType::Glowstone
        );
        assert_eq!(
            reliquary_chunk.get_block((center_x + 3) as usize, (floor_y - 3) as usize),
            BlockType::Glowstone
        );
        assert_eq!(
            reliquary_chunk.get_block((center_x - 3) as usize, (floor_y + 1) as usize),
            BlockType::Lava(8)
        );
    }

    #[test]
    fn test_nether_fortress_watchtowers_add_edge_crowns() {
        let mut world = World::new_for_dimension(Dimension::Nether);
        let layout = World::nether_fortress_layout(&world.perlin, 0);
        for sample_x in [
            layout.left_x + NETHER_FORTRESS_WATCHTOWER_INSET_BLOCKS,
            layout.right_x - NETHER_FORTRESS_WATCHTOWER_INSET_BLOCKS,
        ] {
            world.load_chunks_around(sample_x);
        }

        let glow_y = (layout.roof_y - 8).max(3);
        let roof_y = (layout.roof_y - 10).max(3);
        for tower_x in [
            layout.left_x + NETHER_FORTRESS_WATCHTOWER_INSET_BLOCKS,
            layout.right_x - NETHER_FORTRESS_WATCHTOWER_INSET_BLOCKS,
        ] {
            assert_eq!(world.get_block(tower_x - 1, glow_y), BlockType::Glowstone);
            assert_eq!(world.get_block(tower_x + 1, glow_y), BlockType::Glowstone);
            assert_eq!(world.get_block(tower_x, roof_y), BlockType::StoneSlab);
        }
    }

    #[test]
    fn test_nether_ceiling_has_strong_2d_silhouette_variation() {
        let mut world = World::new_for_dimension(Dimension::Nether);
        for center_x in (-160..=160).step_by(32) {
            world.load_chunks_around(center_x);
        }

        let mut min_ceiling = i32::MAX;
        let mut max_ceiling = i32::MIN;
        for x in -160..=160 {
            let Some(y) = nether_ceiling_surface_y(&world, x) else {
                continue;
            };
            min_ceiling = min_ceiling.min(y);
            max_ceiling = max_ceiling.max(y);
        }

        assert!(min_ceiling < i32::MAX);
        assert!(max_ceiling - min_ceiling >= 8);
    }

    #[test]
    fn test_nether_generation_contains_upper_perches_or_overhang_routes() {
        let mut world = World::new_for_dimension(Dimension::Nether);
        for center_x in (-160..=160).step_by(32) {
            world.load_chunks_around(center_x);
        }

        let mut upper_route_columns = 0;
        for x in -160..=160 {
            let Some(low_y) = nether_walkable_surface_y(&world, x) else {
                continue;
            };
            let Some(high_y) = nether_highest_walkable_surface_y(&world, x) else {
                continue;
            };
            if low_y - high_y >= 6 {
                upper_route_columns += 1;
            }
        }

        assert!(upper_route_columns >= 8);
    }

    #[test]
    fn test_nether_generation_contains_hanging_ceiling_formations() {
        let mut world = World::new_for_dimension(Dimension::Nether);
        for center_x in (-160..=160).step_by(32) {
            world.load_chunks_around(center_x);
        }

        let mut found_hanging_tip = false;
        for x in -160..=160 {
            for y in 8..72 {
                let block = world.get_block(x, y);
                if !block.is_solid() {
                    continue;
                }
                if world.get_block(x, y - 1).is_solid()
                    && world.get_block(x, y + 1) == BlockType::Air
                    && world.get_block(x, y + 2) == BlockType::Air
                {
                    found_hanging_tip = true;
                    break;
                }
            }
            if found_hanging_tip {
                break;
            }
        }

        assert!(found_hanging_tip);
    }

    #[test]
    fn test_nether_generation_has_long_traversable_route() {
        let world = deterministic_generated_world(
            Dimension::Nether,
            "test_nether_generation_has_long_traversable_route",
            -8,
            8,
        );

        let mut best_run = 0;
        let mut current_run = 0;
        let mut prev_y: Option<i32> = None;
        for x in -64..=64 {
            let Some(y) = nether_walkable_surface_y(&world, x) else {
                current_run = 0;
                prev_y = None;
                continue;
            };

            if prev_y.is_some_and(|last_y| (last_y - y).abs() <= 1) {
                current_run += 1;
            } else {
                current_run = 1;
            }
            prev_y = Some(y);
            best_run = best_run.max(current_run);
        }

        assert!(best_run >= 32);
    }

    #[test]
    fn test_nether_fortress_terrain_blends_to_walkable_height() {
        let mut world = World::new_for_dimension(Dimension::Nether);
        let layout = World::nether_fortress_layout(&world.perlin, 0);
        let mut best_run = 0;
        let mut current_run = 0;
        let mut prev_y: Option<i32> = None;
        for x in (layout.left_x - 12)..=(layout.right_x + 12) {
            world.load_chunks_around(x);
            let Some(y) = nether_walkable_surface_y(&world, x) else {
                current_run = 0;
                prev_y = None;
                continue;
            };
            if (y - layout.base_y).abs() > 6 {
                current_run = 0;
                prev_y = None;
                continue;
            }

            if prev_y.is_some_and(|last_y| (last_y - y).abs() <= 1) {
                current_run += 1;
            } else {
                current_run = 1;
            }
            prev_y = Some(y);
            best_run = best_run.max(current_run);
        }

        assert!(best_run >= 12);
    }

    #[test]
    fn test_biome_sampling_finds_taiga_regions() {
        let temp_perlin = Perlin::new(13_579);
        let moist_perlin = Perlin::new(24_680);

        let mut found_taiga = false;
        for x in (-60_000..=60_000).step_by(3) {
            if World::biome_for_x(&temp_perlin, &moist_perlin, x) == BiomeType::Taiga {
                found_taiga = true;
                break;
            }
        }

        assert!(found_taiga);
    }

    #[test]
    fn test_biome_sampling_finds_ocean_hills_swamp_and_jungle_regions() {
        let temp_perlin = Perlin::new(13_579);
        let moist_perlin = Perlin::new(24_680);

        let mut found_ocean = false;
        let mut found_hills = false;
        let mut found_swamp = false;
        let mut found_jungle = false;
        for x in (-120_000..=120_000).step_by(5) {
            match World::biome_for_x(&temp_perlin, &moist_perlin, x) {
                BiomeType::Ocean => found_ocean = true,
                BiomeType::ExtremeHills => found_hills = true,
                BiomeType::Swamp => found_swamp = true,
                BiomeType::Jungle => found_jungle = true,
                _ => {}
            }
            if found_ocean && found_hills && found_swamp && found_jungle {
                break;
            }
        }

        assert!(found_ocean);
        assert!(found_hills);
        assert!(found_swamp);
        assert!(found_jungle);
    }

    #[test]
    fn test_biome_sampling_prefers_large_regions_and_limits_tiny_water_runs() {
        let temp_perlin = Perlin::new(13_579);
        let moist_perlin = Perlin::new(24_680);

        let mut previous = World::biome_for_x(&temp_perlin, &moist_perlin, -40_000);
        let mut run_len = 1usize;
        let mut max_run = 1usize;
        let mut long_runs = 0usize;
        let mut water_runs = 0usize;
        let mut tiny_water_runs = 0usize;
        let mut long_water_run = false;

        let mut finish_run = |biome: BiomeType, len: usize| {
            max_run = max_run.max(len);
            if len >= 96 {
                long_runs += 1;
            }
            if matches!(
                biome,
                BiomeType::Ocean | BiomeType::River | BiomeType::Swamp
            ) {
                water_runs += 1;
                if len <= 2 {
                    tiny_water_runs += 1;
                }
                if len >= 20 {
                    long_water_run = true;
                }
            }
        };

        for x in (-39_999..=40_000).step_by(1) {
            let biome = World::biome_for_x(&temp_perlin, &moist_perlin, x);
            if biome == previous {
                run_len += 1;
                continue;
            }

            finish_run(previous, run_len);
            previous = biome;
            run_len = 1;
        }
        finish_run(previous, run_len);

        assert!(
            max_run >= 160,
            "expected at least one broad biome region, max_run={max_run}"
        );
        assert!(
            long_runs >= 32,
            "expected repeated large biome regions, long_runs={long_runs}"
        );
        assert!(
            long_water_run,
            "expected at least one substantial water-biome run"
        );
        assert!(
            tiny_water_runs * 4 <= water_runs.max(1),
            "too many tiny water-biome runs: tiny={tiny_water_runs}, total_water_runs={water_runs}"
        );
    }

    #[test]
    fn test_water_biome_surface_levels_stay_near_sea_level() {
        let perlin = Perlin::new(1337);
        let temp_perlin = Perlin::new(7331);
        let moist_perlin = Perlin::new(9917);

        let mut found_ocean = false;
        let mut found_river = false;
        let mut found_swamp = false;

        for x in (-120_000..=120_000).step_by(5) {
            let biome = World::biome_for_x(&temp_perlin, &moist_perlin, x);
            let surface_y = World::adjusted_overworld_surface_y(&perlin, x, biome);
            match biome {
                BiomeType::Ocean => {
                    assert!(
                        (OVERWORLD_SEA_LEVEL + 4..=OVERWORLD_SEA_LEVEL + 11).contains(&surface_y)
                    );
                    found_ocean = true;
                }
                BiomeType::River => {
                    assert!(
                        (OVERWORLD_SEA_LEVEL + 2..=OVERWORLD_SEA_LEVEL + 4).contains(&surface_y)
                    );
                    found_river = true;
                }
                BiomeType::Swamp => {
                    assert!(
                        (OVERWORLD_SEA_LEVEL + 2..=OVERWORLD_SEA_LEVEL + 4).contains(&surface_y)
                    );
                    found_swamp = true;
                }
                _ => {}
            }

            if found_ocean && found_river && found_swamp {
                break;
            }
        }

        assert!(found_ocean);
        assert!(found_river);
        assert!(found_swamp);
    }

    #[test]
    fn test_submerged_swamp_columns_keep_surface_water_clear_of_decor() {
        let perlin = Perlin::new(1337);
        let temp_perlin = Perlin::new(7331);
        let moist_perlin = Perlin::new(9917);
        let cave_perlin = Perlin::new(17_711);

        let mut sample = None;
        for x in (-120_000..=120_000).step_by(1) {
            let biome = World::biome_for_x(&temp_perlin, &moist_perlin, x);
            if biome != BiomeType::Swamp {
                continue;
            }
            let surface_y =
                World::blended_overworld_surface_y(&perlin, &temp_perlin, &moist_perlin, x, biome);
            let decor_roll = (x as u32).wrapping_mul(134775813).wrapping_add(9821471) % 100;
            if surface_y > OVERWORLD_SEA_LEVEL && decor_roll < 26 {
                sample = Some((x, surface_y));
                break;
            }
        }

        let (x, surface_y) = sample.expect("expected a submerged swamp decor sample");
        let chunk_x = x.div_euclid(CHUNK_WIDTH as i32);
        let chunk = World::build_chunk_with_noise(
            chunk_x,
            Dimension::Overworld,
            "test_submerged_swamp_columns_keep_surface_water_clear_of_decor",
            &perlin,
            &temp_perlin,
            &moist_perlin,
            &cave_perlin,
        );
        let lx = x.rem_euclid(CHUNK_WIDTH as i32) as usize;
        assert_eq!(
            chunk.get_block(lx, (surface_y - 1) as usize),
            BlockType::Water(8)
        );
    }

    #[test]
    fn test_submerged_shoreline_land_columns_fill_with_water_and_skip_decor() {
        let perlin = Perlin::new(1337);
        let temp_perlin = Perlin::new(7331);
        let moist_perlin = Perlin::new(9917);
        let cave_perlin = Perlin::new(17_711);

        let mut sample = None;
        for x in (-120_000..=120_000).step_by(1) {
            let biome = World::biome_for_x(&temp_perlin, &moist_perlin, x);
            if matches!(
                biome,
                BiomeType::Ocean | BiomeType::River | BiomeType::Swamp | BiomeType::Desert
            ) {
                continue;
            }

            let surface_y =
                World::blended_overworld_surface_y(&perlin, &temp_perlin, &moist_perlin, x, biome);
            if surface_y <= OVERWORLD_SEA_LEVEL {
                continue;
            }
            if World::nearest_biome_in_radius(
                &temp_perlin,
                &moist_perlin,
                x,
                SHORELINE_BLEND_RADIUS,
                |candidate| matches!(candidate, BiomeType::Ocean | BiomeType::River),
            )
            .is_none()
            {
                continue;
            }

            let decor_roll = (x as u32).wrapping_mul(134775813).wrapping_add(9821471) % 100;
            let decor_limit = match biome {
                BiomeType::Forest => 20,
                BiomeType::Taiga => 12,
                BiomeType::Jungle => 24,
                BiomeType::ExtremeHills => 12,
                BiomeType::Tundra => 12,
                BiomeType::Plains => 20,
                _ => 0,
            };
            if decor_roll >= decor_limit {
                continue;
            }

            sample = Some((x, surface_y));
            break;
        }

        let (x, surface_y) =
            sample.expect("expected a submerged shoreline land sample near ocean or river");
        let chunk_x = x.div_euclid(CHUNK_WIDTH as i32);
        let chunk = World::build_chunk_with_noise(
            chunk_x,
            Dimension::Overworld,
            "test_submerged_shoreline_land_columns_fill_with_water_and_skip_decor",
            &perlin,
            &temp_perlin,
            &moist_perlin,
            &cave_perlin,
        );
        let lx = x.rem_euclid(CHUNK_WIDTH as i32) as usize;
        assert_eq!(
            chunk.get_block(lx, OVERWORLD_SEA_LEVEL as usize),
            BlockType::Water(8)
        );
        assert_eq!(
            chunk.get_block(lx, (surface_y - 1) as usize),
            BlockType::Water(8)
        );
        assert_eq!(chunk.get_block(lx, surface_y as usize), BlockType::Sand);
    }

    #[test]
    fn test_submerged_land_columns_stay_tied_to_immediate_water_biome_edge() {
        let perlin = Perlin::new(1337);
        let temp_perlin = Perlin::new(7331);
        let moist_perlin = Perlin::new(9917);

        let mut checked = 0usize;
        for x in (-160_000..=160_000).step_by(1) {
            let biome = World::biome_for_x(&temp_perlin, &moist_perlin, x);
            if World::is_surface_water_biome(biome) {
                continue;
            }

            let surface_y =
                World::blended_overworld_surface_y(&perlin, &temp_perlin, &moist_perlin, x, biome);
            if !World::column_has_surface_water(
                &perlin,
                &temp_perlin,
                &moist_perlin,
                x,
                biome,
                surface_y,
            ) {
                continue;
            }

            let touches_water_biome = [x - 1, x + 1].into_iter().any(|sample_x| {
                let sample_biome = World::biome_for_x(&temp_perlin, &moist_perlin, sample_x);
                let sample_surface_y = World::blended_overworld_surface_y(
                    &perlin,
                    &temp_perlin,
                    &moist_perlin,
                    sample_x,
                    sample_biome,
                );
                World::is_surface_water_biome(sample_biome)
                    && sample_surface_y > OVERWORLD_SEA_LEVEL
            });
            assert!(
                touches_water_biome,
                "submerged land at x={x} was not attached to an immediate water-biome edge"
            );
            checked += 1;
            if checked >= 24 {
                break;
            }
        }

        assert!(
            checked > 0,
            "expected at least one shoreline submersion sample"
        );
    }

    #[test]
    fn test_motion_aware_chunk_loading_leads_player_direction() {
        let mut world = World::new();
        world.load_chunks_for_motion(CHUNK_WIDTH as f64 * 0.9, 0.3);
        assert!(world.chunks.contains_key(&2));

        let mut left_world = World::new();
        left_world.load_chunks_for_motion(CHUNK_WIDTH as f64 + 0.8, -0.3);
        assert!(left_world.chunks.contains_key(&-1));
    }

    #[test]
    fn test_spawn_search_chunk_loading_covers_requested_radius() {
        let mut world = World::new();
        world.load_chunks_for_spawn_search(0, 56);

        let min_chunk = (-56i32).div_euclid(CHUNK_WIDTH as i32);
        let max_chunk = 56i32.div_euclid(CHUNK_WIDTH as i32);
        for chunk_x in min_chunk..=max_chunk {
            assert!(world.chunks.contains_key(&chunk_x));
        }
    }

    #[test]
    fn test_shoreline_generation_blends_shelves_and_builds_beaches() {
        let perlin = Perlin::new(1337);
        let temp_perlin = Perlin::new(7331);
        let moist_perlin = Perlin::new(9917);
        let cave_perlin = Perlin::new(17_711);

        let mut shelf_sample = None;
        let mut beach_sample = None;
        for x in (-120_000..=120_000).step_by(5) {
            let biome = World::biome_for_x(&temp_perlin, &moist_perlin, x);
            let adjusted = World::adjusted_overworld_surface_y(&perlin, x, biome);
            let blended =
                World::blended_overworld_surface_y(&perlin, &temp_perlin, &moist_perlin, x, biome);

            if shelf_sample.is_none()
                && biome == BiomeType::Ocean
                && blended < adjusted
                && World::nearest_biome_in_radius(
                    &temp_perlin,
                    &moist_perlin,
                    x,
                    SHORELINE_SHELF_RADIUS,
                    |candidate| {
                        !matches!(
                            candidate,
                            BiomeType::Ocean | BiomeType::River | BiomeType::Swamp
                        )
                    },
                )
                .is_some()
            {
                shelf_sample = Some((x, adjusted, blended));
            }

            if beach_sample.is_none()
                && x.div_euclid(CHUNK_WIDTH as i32).rem_euclid(11) != 5
                && World::is_beach_column(&temp_perlin, &moist_perlin, x, biome, blended)
            {
                beach_sample = Some((x, adjusted, blended));
            }

            if shelf_sample.is_some() && beach_sample.is_some() {
                break;
            }
        }

        let (_shelf_x, shelf_adjusted, shelf_blended) =
            shelf_sample.expect("expected an ocean shelf sample near land");
        assert!(shelf_blended < shelf_adjusted);

        let (beach_x, beach_adjusted, beach_blended) =
            beach_sample.expect("expected a shoreline beach sample");
        assert!(beach_blended >= beach_adjusted);
        assert!((OVERWORLD_SEA_LEVEL - 3..=OVERWORLD_SEA_LEVEL + 4).contains(&beach_blended));

        let beach_chunk_x = beach_x.div_euclid(CHUNK_WIDTH as i32);
        let beach_chunk = World::build_chunk_with_noise(
            beach_chunk_x,
            Dimension::Overworld,
            "test_shoreline_generation_blends_shelves_and_builds_beaches",
            &perlin,
            &temp_perlin,
            &moist_perlin,
            &cave_perlin,
        );
        let beach_lx = beach_x.rem_euclid(CHUNK_WIDTH as i32) as usize;
        assert_eq!(
            beach_chunk.get_block(beach_lx, beach_blended as usize),
            BlockType::Sand
        );
    }

    #[test]
    fn test_land_banks_adjacent_to_surface_water_stay_climbable() {
        let perlin = Perlin::new(1337);
        let temp_perlin = Perlin::new(7331);
        let moist_perlin = Perlin::new(9917);

        let mut checked_banks = 0usize;
        let mut checked_swamp_banks = 0usize;
        let mut checked_level_banks = 0usize;
        for x in (-160_000..=160_000).step_by(1) {
            let biome = World::biome_for_x(&temp_perlin, &moist_perlin, x);
            if matches!(
                biome,
                BiomeType::Ocean | BiomeType::River | BiomeType::Swamp
            ) {
                continue;
            }

            let surface_y =
                World::blended_overworld_surface_y(&perlin, &temp_perlin, &moist_perlin, x, biome);
            if World::column_has_surface_water(
                &perlin,
                &temp_perlin,
                &moist_perlin,
                x,
                biome,
                surface_y,
            ) {
                continue;
            }

            let mut adjacent_water_biome = None;
            for sample_x in [x - 1, x + 1] {
                let sample_biome = World::biome_for_x(&temp_perlin, &moist_perlin, sample_x);
                let sample_surface_y = World::blended_overworld_surface_y(
                    &perlin,
                    &temp_perlin,
                    &moist_perlin,
                    sample_x,
                    sample_biome,
                );
                if World::column_has_surface_water(
                    &perlin,
                    &temp_perlin,
                    &moist_perlin,
                    sample_x,
                    sample_biome,
                    sample_surface_y,
                ) {
                    adjacent_water_biome = Some(sample_biome);
                    break;
                }
            }

            if let Some(water_biome) = adjacent_water_biome {
                assert!(
                    surface_y >= OVERWORLD_SEA_LEVEL - 1,
                    "adjacent bank too high at x={x}: biome={biome:?}, water_biome={water_biome:?}, surface_y={surface_y}"
                );
                checked_banks += 1;
                if surface_y == OVERWORLD_SEA_LEVEL {
                    checked_level_banks += 1;
                }
                if water_biome == BiomeType::Swamp {
                    checked_swamp_banks += 1;
                }
                if checked_banks >= 64 && checked_swamp_banks >= 8 && checked_level_banks >= 8 {
                    break;
                }
            }
        }

        assert!(checked_banks >= 32);
        assert!(checked_swamp_banks >= 4);
        assert!(checked_level_banks >= 4);
    }

    #[test]
    fn test_surface_water_edges_skip_tree_trunks() {
        let perlin = Perlin::new(1337);
        let temp_perlin = Perlin::new(7331);
        let moist_perlin = Perlin::new(9917);
        let cave_perlin = Perlin::new(17_711);

        let mut sample = None;
        for x in (-160_000..=160_000).step_by(1) {
            let biome = World::biome_for_x(&temp_perlin, &moist_perlin, x);
            if matches!(
                biome,
                BiomeType::Ocean | BiomeType::River | BiomeType::Swamp | BiomeType::Desert
            ) {
                continue;
            }

            let surface_y =
                World::blended_overworld_surface_y(&perlin, &temp_perlin, &moist_perlin, x, biome);
            if World::column_has_surface_water(
                &perlin,
                &temp_perlin,
                &moist_perlin,
                x,
                biome,
                surface_y,
            ) || !World::column_touches_surface_water(
                &perlin,
                &temp_perlin,
                &moist_perlin,
                x,
                biome,
                surface_y,
            ) {
                continue;
            }

            let decor_roll = (x as u32).wrapping_mul(134775813).wrapping_add(9821471) % 100;
            let tree_limit = match biome {
                BiomeType::Forest => 15,
                BiomeType::Taiga => 12,
                BiomeType::Jungle => 24,
                BiomeType::ExtremeHills => 6,
                BiomeType::Tundra => 4,
                BiomeType::Plains => 2,
                _ => 2,
            };
            if decor_roll < tree_limit {
                sample = Some((x, surface_y));
                break;
            }
        }

        let (x, surface_y) = sample.expect("expected a shoreline-edge tree decor sample");
        let chunk_x = x.div_euclid(CHUNK_WIDTH as i32);
        let chunk = World::build_chunk_with_noise(
            chunk_x,
            Dimension::Overworld,
            "test_surface_water_edges_skip_tree_trunks",
            &perlin,
            &temp_perlin,
            &moist_perlin,
            &cave_perlin,
        );
        let lx = x.rem_euclid(CHUNK_WIDTH as i32) as usize;
        assert!(
            !matches!(
                chunk.get_block(lx, (surface_y - 1) as usize),
                BlockType::Wood | BlockType::BirchWood
            ),
            "shoreline-edge bank at x={x} still grew a trunk"
        );
    }

    #[test]
    fn test_overworld_generation_supports_surface_gravity_blocks() {
        let perlin = Perlin::new(1337);
        let temp_perlin = Perlin::new(7331);
        let moist_perlin = Perlin::new(9917);
        let cave_perlin = Perlin::new(17_711);

        let mut found_gravity_surface = false;
        for chunk_x in -24..=24 {
            let chunk = World::build_chunk_with_noise(
                chunk_x,
                Dimension::Overworld,
                "test_overworld_generation_supports_surface_gravity_blocks",
                &perlin,
                &temp_perlin,
                &moist_perlin,
                &cave_perlin,
            );
            let c_start = chunk_x * CHUNK_WIDTH as i32;
            for lx in 0..CHUNK_WIDTH {
                let wx = c_start + lx as i32;
                let biome = World::biome_for_x(&temp_perlin, &moist_perlin, wx);
                let surface_y = World::blended_overworld_surface_y(
                    &perlin,
                    &temp_perlin,
                    &moist_perlin,
                    wx,
                    biome,
                );
                let surface = chunk.get_block(lx, surface_y as usize);
                if !surface.obeys_gravity() || surface_y + 1 >= CHUNK_HEIGHT as i32 {
                    continue;
                }

                found_gravity_surface = true;
                let below = chunk.get_block(lx, (surface_y + 1) as usize);
                assert_ne!(
                    below,
                    BlockType::Air,
                    "unsupported gravity surface at x={wx}"
                );
                assert!(
                    !below.obeys_gravity(),
                    "stacked gravity support at x={wx}, surface={surface:?}, below={below:?}"
                );
            }
        }

        assert!(found_gravity_surface);
    }

    #[test]
    fn test_overworld_generation_keeps_dry_surface_cap_solid() {
        let perlin = Perlin::new(1337);
        let temp_perlin = Perlin::new(7331);
        let moist_perlin = Perlin::new(9917);
        let cave_perlin = Perlin::new(17_711);

        let mut sampled_dry_column = false;
        for chunk_x in -24..=24 {
            let chunk = World::build_chunk_with_noise(
                chunk_x,
                Dimension::Overworld,
                "test_overworld_generation_keeps_dry_surface_cap_solid",
                &perlin,
                &temp_perlin,
                &moist_perlin,
                &cave_perlin,
            );
            let c_start = chunk_x * CHUNK_WIDTH as i32;
            for lx in 0..CHUNK_WIDTH {
                let wx = c_start + lx as i32;
                let biome = World::biome_for_x(&temp_perlin, &moist_perlin, wx);
                let surface_y = World::blended_overworld_surface_y(
                    &perlin,
                    &temp_perlin,
                    &moist_perlin,
                    wx,
                    biome,
                );
                if surface_y + OVERWORLD_CAVE_MIN_SURFACE_DEPTH >= CHUNK_HEIGHT as i32
                    || World::column_has_surface_water(
                        &perlin,
                        &temp_perlin,
                        &moist_perlin,
                        wx,
                        biome,
                        surface_y,
                    )
                {
                    continue;
                }

                sampled_dry_column = true;
                for depth in 1..=OVERWORLD_CAVE_MIN_SURFACE_DEPTH {
                    let block = chunk.get_block(lx, (surface_y + depth) as usize);
                    assert_ne!(
                        block,
                        BlockType::Air,
                        "near-surface cave opening at x={wx}, depth={depth}"
                    );
                    assert!(
                        !block.obeys_gravity(),
                        "near-surface gravity pocket at x={wx}, depth={depth}, block={block:?}"
                    );
                }
            }
        }

        assert!(sampled_dry_column);
    }

    #[test]
    fn test_overworld_generation_carves_readable_cave_chambers() {
        let perlin = Perlin::new(1337);
        let temp_perlin = Perlin::new(7331);
        let moist_perlin = Perlin::new(9917);
        let cave_perlin = Perlin::new(17_711);

        let mut found_wide_chamber = false;
        'chunks: for chunk_x in -96i32..=96 {
            if chunk_x.rem_euclid(OVERWORLD_DUNGEON_CHUNK_CADENCE) == OVERWORLD_DUNGEON_CHUNK_PHASE
                || chunk_x.rem_euclid(11) == 5
            {
                continue;
            }

            let chunk = World::build_chunk_with_noise(
                chunk_x,
                Dimension::Overworld,
                "test_overworld_generation_carves_readable_cave_chambers",
                &perlin,
                &temp_perlin,
                &moist_perlin,
                &cave_perlin,
            );
            let c_start = chunk_x * CHUNK_WIDTH as i32;

            for y in 0..CHUNK_HEIGHT {
                let mut run = 0usize;
                for lx in 0..CHUNK_WIDTH {
                    let wx = c_start + lx as i32;
                    let biome = World::biome_for_x(&temp_perlin, &moist_perlin, wx);
                    let surface_y = World::blended_overworld_surface_y(
                        &perlin,
                        &temp_perlin,
                        &moist_perlin,
                        wx,
                        biome,
                    );
                    let deep_dry_cave_band = !World::column_has_surface_water(
                        &perlin,
                        &temp_perlin,
                        &moist_perlin,
                        wx,
                        biome,
                        surface_y,
                    ) && (y as i32)
                        > surface_y + OVERWORLD_CHAMBER_MIN_SURFACE_DEPTH;

                    if deep_dry_cave_band && chunk.get_block(lx, y) == BlockType::Air {
                        run += 1;
                        if run >= 6 {
                            found_wide_chamber = true;
                            break 'chunks;
                        }
                    } else {
                        run = 0;
                    }
                }
            }
        }

        assert!(found_wide_chamber);
    }

    #[test]
    fn test_overworld_cave_pacing_trims_short_dead_end_stubs() {
        let mut chunk = solid_test_chunk("test_overworld_cave_pacing_trims_short_dead_end_stubs");
        let (surface_y_by_x, surface_water_by_x) = flat_cave_surface_profile(20);

        for x in 5..=8 {
            for y in 49..=50 {
                chunk.set_block(x, y, BlockType::Air);
            }
        }
        for x in 9..=10 {
            for y in 49..=50 {
                chunk.set_block(x, y, BlockType::Air);
            }
        }

        World::refine_overworld_cave_pacing(&mut chunk, &surface_y_by_x, &surface_water_by_x);

        assert_eq!(chunk.get_block(8, 50), BlockType::Air);
        assert_eq!(chunk.get_block(8, 49), BlockType::Air);
        assert_eq!(chunk.get_block(9, 50), BlockType::Stone);
        assert_eq!(chunk.get_block(9, 49), BlockType::Stone);
        assert_eq!(chunk.get_block(10, 50), BlockType::Stone);
        assert_eq!(chunk.get_block(10, 49), BlockType::Stone);
    }

    #[test]
    fn test_overworld_cave_pacing_bridges_thin_walls() {
        let mut chunk = solid_test_chunk("test_overworld_cave_pacing_bridges_thin_walls");
        let (surface_y_by_x, surface_water_by_x) = flat_cave_surface_profile(20);

        for x in 4..=8 {
            for y in 48..=50 {
                chunk.set_block(x, y, BlockType::Air);
            }
        }
        for x in 12..=16 {
            for y in 48..=50 {
                chunk.set_block(x, y, BlockType::Air);
            }
        }

        World::refine_overworld_cave_pacing(&mut chunk, &surface_y_by_x, &surface_water_by_x);

        for x in 9..=11 {
            assert_eq!(chunk.get_block(x, 50), BlockType::Air);
            assert_eq!(chunk.get_block(x, 49), BlockType::Air);
        }
    }

    #[test]
    fn test_overworld_cave_pacing_carves_stair_connectors() {
        let mut chunk = solid_test_chunk("test_overworld_cave_pacing_carves_stair_connectors");
        let (surface_y_by_x, surface_water_by_x) = flat_cave_surface_profile(20);

        for x in 4..=7 {
            for y in 38..=40 {
                chunk.set_block(x, y, BlockType::Air);
            }
        }
        for x in 11..=14 {
            for y in 42..=44 {
                chunk.set_block(x, y, BlockType::Air);
            }
        }

        World::refine_overworld_cave_pacing(&mut chunk, &surface_y_by_x, &surface_water_by_x);

        assert_eq!(chunk.get_block(8, 41), BlockType::Air);
        assert_eq!(chunk.get_block(8, 40), BlockType::Air);
        assert_eq!(chunk.get_block(9, 42), BlockType::Air);
        assert_eq!(chunk.get_block(9, 41), BlockType::Air);
        assert_eq!(chunk.get_block(10, 43), BlockType::Air);
        assert_eq!(chunk.get_block(10, 42), BlockType::Air);
    }

    #[test]
    fn test_overworld_ravines_widen_with_depth() {
        let perlin = Perlin::new(1337);
        let temp_perlin = Perlin::new(7331);
        let moist_perlin = Perlin::new(9917);
        let cave_perlin = Perlin::new(17_711);

        let mut found_profile = false;
        for center_x in (-20_000..=20_000).step_by(3) {
            let biome = World::biome_for_x(&temp_perlin, &moist_perlin, center_x);
            let surface_y = World::blended_overworld_surface_y(
                &perlin,
                &temp_perlin,
                &moist_perlin,
                center_x,
                biome,
            );
            if World::column_has_surface_water(
                &perlin,
                &temp_perlin,
                &moist_perlin,
                center_x,
                biome,
                surface_y,
            ) {
                continue;
            }

            let top_y = surface_y + OVERWORLD_RAVINE_MIN_SURFACE_DEPTH + 4;
            let deep_y = top_y + 18;
            if deep_y >= CHUNK_HEIGHT as i32 - 6 {
                continue;
            }

            let row_width = |sample_y: i32| -> usize {
                ((center_x - 12)..=(center_x + 12))
                    .filter(|&sample_x| {
                        let sample_biome =
                            World::biome_for_x(&temp_perlin, &moist_perlin, sample_x);
                        let sample_surface_y = World::blended_overworld_surface_y(
                            &perlin,
                            &temp_perlin,
                            &moist_perlin,
                            sample_x,
                            sample_biome,
                        );
                        World::overworld_ravine_carves(
                            &cave_perlin,
                            sample_x,
                            sample_y,
                            sample_biome,
                            sample_surface_y,
                        )
                    })
                    .count()
            };

            let top_width = row_width(top_y);
            let deep_width = row_width(deep_y);
            if top_width >= 2 && deep_width >= top_width + 2 {
                found_profile = true;
                break;
            }
        }

        assert!(found_profile);
    }

    #[test]
    fn test_extreme_hills_have_more_cave_space_than_plains() {
        let perlin = Perlin::new(1337);
        let temp_perlin = Perlin::new(7331);
        let moist_perlin = Perlin::new(9917);
        let cave_perlin = Perlin::new(17_711);

        let plains_ratio = sampled_overworld_cave_air_ratio(
            BiomeType::Plains,
            &perlin,
            &temp_perlin,
            &moist_perlin,
            &cave_perlin,
        );
        let hills_ratio = sampled_overworld_cave_air_ratio(
            BiomeType::ExtremeHills,
            &perlin,
            &temp_perlin,
            &moist_perlin,
            &cave_perlin,
        );

        assert!(
            hills_ratio > plains_ratio + 0.01,
            "expected more carved cave space in hills: plains={plains_ratio:.3}, hills={hills_ratio:.3}"
        );
    }

    #[test]
    fn test_overworld_generation_dresses_cave_floors() {
        let perlin = Perlin::new(1337);
        let temp_perlin = Perlin::new(7331);
        let moist_perlin = Perlin::new(9917);
        let cave_perlin = Perlin::new(17_711);

        let mut found_dressed_floor = false;
        'chunks: for chunk_x in -80i32..=80 {
            if chunk_x.rem_euclid(OVERWORLD_DUNGEON_CHUNK_CADENCE) == OVERWORLD_DUNGEON_CHUNK_PHASE
                || chunk_x.rem_euclid(11) == 5
            {
                continue;
            }

            let chunk = World::build_chunk_with_noise(
                chunk_x,
                Dimension::Overworld,
                "test_overworld_generation_dresses_cave_floors",
                &perlin,
                &temp_perlin,
                &moist_perlin,
                &cave_perlin,
            );
            let c_start = chunk_x * CHUNK_WIDTH as i32;
            for lx in 0..CHUNK_WIDTH {
                let wx = c_start + lx as i32;
                let biome = World::biome_for_x(&temp_perlin, &moist_perlin, wx);
                let surface_y = World::blended_overworld_surface_y(
                    &perlin,
                    &temp_perlin,
                    &moist_perlin,
                    wx,
                    biome,
                );
                if World::column_has_surface_water(
                    &perlin,
                    &temp_perlin,
                    &moist_perlin,
                    wx,
                    biome,
                    surface_y,
                ) {
                    continue;
                }

                for y in (surface_y + OVERWORLD_CAVE_FLOOR_VARIATION_MIN_DEPTH)
                    ..=(CHUNK_HEIGHT as i32 - 4)
                {
                    if chunk.get_block(lx, y as usize) == BlockType::Air
                        && matches!(
                            chunk.get_block(lx, (y + 1) as usize),
                            BlockType::Dirt | BlockType::Gravel
                        )
                    {
                        found_dressed_floor = true;
                        break 'chunks;
                    }
                }
            }
        }

        assert!(found_dressed_floor);
    }

    #[test]
    fn test_overworld_generation_adds_cave_pools() {
        let perlin = Perlin::new(1337);
        let temp_perlin = Perlin::new(7331);
        let moist_perlin = Perlin::new(9917);
        let cave_perlin = Perlin::new(17_711);

        let mut found_pool = false;
        'chunks: for chunk_x in -220i32..=220 {
            if chunk_x.rem_euclid(OVERWORLD_DUNGEON_CHUNK_CADENCE) == OVERWORLD_DUNGEON_CHUNK_PHASE
                || chunk_x.rem_euclid(11) == 5
            {
                continue;
            }

            let chunk = World::build_chunk_with_noise(
                chunk_x,
                Dimension::Overworld,
                "test_overworld_generation_adds_cave_pools",
                &perlin,
                &temp_perlin,
                &moist_perlin,
                &cave_perlin,
            );
            let c_start = chunk_x * CHUNK_WIDTH as i32;
            for lx in 0..CHUNK_WIDTH {
                let wx = c_start + lx as i32;
                let biome = World::biome_for_x(&temp_perlin, &moist_perlin, wx);
                let surface_y = World::blended_overworld_surface_y(
                    &perlin,
                    &temp_perlin,
                    &moist_perlin,
                    wx,
                    biome,
                );
                if World::column_has_surface_water(
                    &perlin,
                    &temp_perlin,
                    &moist_perlin,
                    wx,
                    biome,
                    surface_y,
                ) {
                    continue;
                }

                for y in (surface_y + OVERWORLD_CAVE_POOL_MIN_DEPTH)..=(CHUNK_HEIGHT as i32 - 10) {
                    if matches!(
                        chunk.get_block(lx, y as usize),
                        BlockType::Water(8) | BlockType::Lava(8)
                    ) {
                        found_pool = true;
                        break 'chunks;
                    }
                }
            }
        }

        assert!(found_pool);
    }

    #[test]
    fn test_overworld_generation_clusters_ores_into_pockets() {
        let perlin = Perlin::new(1337);
        let temp_perlin = Perlin::new(7331);
        let moist_perlin = Perlin::new(9917);
        let cave_perlin = Perlin::new(17_711);

        let mut found_cluster = false;
        'chunks: for chunk_x in -120i32..=120 {
            let chunk = World::build_chunk_with_noise(
                chunk_x,
                Dimension::Overworld,
                "test_overworld_generation_clusters_ores_into_pockets",
                &perlin,
                &temp_perlin,
                &moist_perlin,
                &cave_perlin,
            );

            for y in 48..(CHUNK_HEIGHT - 10) {
                let mut run_block = None;
                let mut run_len = 0usize;
                for x in 0..CHUNK_WIDTH {
                    let block = chunk.get_block(x, y);
                    if matches!(
                        block,
                        BlockType::CoalOre
                            | BlockType::IronOre
                            | BlockType::RedstoneOre
                            | BlockType::GoldOre
                            | BlockType::DiamondOre
                    ) {
                        if Some(block) == run_block {
                            run_len += 1;
                        } else {
                            run_block = Some(block);
                            run_len = 1;
                        }

                        if run_len >= 3 {
                            found_cluster = true;
                            break 'chunks;
                        }
                    } else {
                        run_block = None;
                        run_len = 0;
                    }
                }
            }
        }

        assert!(found_cluster);
    }

    #[test]
    fn test_overworld_generation_maintains_readable_ore_density() {
        let perlin = Perlin::new(1337);
        let temp_perlin = Perlin::new(7331);
        let moist_perlin = Perlin::new(9917);
        let cave_perlin = Perlin::new(17_711);

        let mut coal = 0usize;
        let mut iron = 0usize;
        let mut redstone = 0usize;
        let mut gold = 0usize;
        let mut diamond = 0usize;

        for chunk_x in -96i32..=96 {
            let chunk = World::build_chunk_with_noise(
                chunk_x,
                Dimension::Overworld,
                "test_overworld_generation_maintains_readable_ore_density",
                &perlin,
                &temp_perlin,
                &moist_perlin,
                &cave_perlin,
            );

            for y in 44..(CHUNK_HEIGHT - 8) {
                for x in 0..CHUNK_WIDTH {
                    match chunk.get_block(x, y) {
                        BlockType::CoalOre => coal += 1,
                        BlockType::IronOre => iron += 1,
                        BlockType::RedstoneOre => redstone += 1,
                        BlockType::GoldOre => gold += 1,
                        BlockType::DiamondOre => diamond += 1,
                        _ => {}
                    }
                }
            }
        }

        assert!(coal >= 1000, "coal too sparse: {coal}");
        assert!(iron >= 900, "iron too sparse: {iron}");
        assert!(redstone >= 220, "redstone too sparse: {redstone}");
        assert!(gold >= 140, "gold too sparse: {gold}");
        assert!(diamond >= 90, "diamond too sparse: {diamond}");
    }

    #[test]
    fn test_overworld_generation_caps_long_iron_seams() {
        let perlin = Perlin::new(1337);
        let temp_perlin = Perlin::new(7331);
        let moist_perlin = Perlin::new(9917);
        let cave_perlin = Perlin::new(17_711);

        let mut max_run = 0usize;
        for chunk_x in -96i32..=96 {
            let chunk = World::build_chunk_with_noise(
                chunk_x,
                Dimension::Overworld,
                "test_overworld_generation_caps_long_iron_seams",
                &perlin,
                &temp_perlin,
                &moist_perlin,
                &cave_perlin,
            );

            for y in OVERWORLD_VISIBLE_ORE_MIN_Y as usize..(CHUNK_HEIGHT - 8) {
                let mut run = 0usize;
                for x in 0..CHUNK_WIDTH {
                    if chunk.get_block(x, y) == BlockType::IronOre {
                        run += 1;
                        max_run = max_run.max(run);
                    } else {
                        run = 0;
                    }
                }
            }
        }

        assert!(max_run <= 6, "iron seam too long: {max_run}");
    }

    #[test]
    fn test_overworld_generation_places_diamond_in_deep_bands() {
        let perlin = Perlin::new(1337);
        let temp_perlin = Perlin::new(7331);
        let moist_perlin = Perlin::new(9917);
        let cave_perlin = Perlin::new(17_711);

        let mut deep_diamond = 0usize;
        for chunk_x in -96i32..=96 {
            let chunk = World::build_chunk_with_noise(
                chunk_x,
                Dimension::Overworld,
                "test_overworld_generation_places_diamond_in_deep_bands",
                &perlin,
                &temp_perlin,
                &moist_perlin,
                &cave_perlin,
            );

            for y in 104usize..(CHUNK_HEIGHT - 8) {
                for x in 0..CHUNK_WIDTH {
                    if chunk.get_block(x, y) == BlockType::DiamondOre {
                        deep_diamond += 1;
                    }
                }
            }
        }

        assert!(
            deep_diamond >= 80,
            "deep diamond too sparse: {deep_diamond}"
        );
    }

    #[test]
    fn test_overworld_generation_keeps_iron_visible_in_mid_depth_caves() {
        let perlin = Perlin::new(1337);
        let temp_perlin = Perlin::new(7331);
        let moist_perlin = Perlin::new(9917);
        let cave_perlin = Perlin::new(17_711);

        let mut exposed_iron = 0usize;
        for chunk_x in -96i32..=96 {
            let chunk = World::build_chunk_with_noise(
                chunk_x,
                Dimension::Overworld,
                "test_overworld_generation_keeps_iron_visible_in_mid_depth_caves",
                &perlin,
                &temp_perlin,
                &moist_perlin,
                &cave_perlin,
            );

            for y in OVERWORLD_VISIBLE_ORE_MIN_Y as usize..(CHUNK_HEIGHT - 8) {
                for x in 1..(CHUNK_WIDTH - 1) {
                    if chunk.get_block(x, y) != BlockType::IronOre {
                        continue;
                    }

                    if [
                        chunk.get_block(x - 1, y),
                        chunk.get_block(x + 1, y),
                        chunk.get_block(x, y - 1),
                        chunk.get_block(x, y + 1),
                    ]
                    .into_iter()
                    .any(|block| block == BlockType::Air)
                    {
                        exposed_iron += 1;
                    }
                }
            }
        }

        assert!(
            exposed_iron >= 80,
            "exposed iron too sparse in normal cave bands: {exposed_iron}"
        );
    }

    #[test]
    fn test_overworld_generation_exposes_ore_on_cave_walls() {
        let perlin = Perlin::new(1337);
        let temp_perlin = Perlin::new(7331);
        let moist_perlin = Perlin::new(9917);
        let cave_perlin = Perlin::new(17_711);

        let mut exposed_ore = 0usize;
        for chunk_x in -96i32..=96 {
            let chunk = World::build_chunk_with_noise(
                chunk_x,
                Dimension::Overworld,
                "test_overworld_generation_exposes_ore_on_cave_walls",
                &perlin,
                &temp_perlin,
                &moist_perlin,
                &cave_perlin,
            );

            for y in 48..(CHUNK_HEIGHT - 8) {
                for x in 1..(CHUNK_WIDTH - 1) {
                    if !matches!(
                        chunk.get_block(x, y),
                        BlockType::CoalOre
                            | BlockType::IronOre
                            | BlockType::RedstoneOre
                            | BlockType::GoldOre
                            | BlockType::DiamondOre
                    ) {
                        continue;
                    }

                    if [
                        chunk.get_block(x - 1, y),
                        chunk.get_block(x + 1, y),
                        chunk.get_block(x, y - 1),
                        chunk.get_block(x, y + 1),
                    ]
                    .into_iter()
                    .any(|block| block == BlockType::Air)
                    {
                        exposed_ore += 1;
                    }
                }
            }
        }

        assert!(exposed_ore >= 140, "exposed ore too sparse: {exposed_ore}");
    }

    #[test]
    fn test_overworld_generation_exposes_diamond_on_deep_cave_walls() {
        let perlin = Perlin::new(1337);
        let temp_perlin = Perlin::new(7331);
        let moist_perlin = Perlin::new(9917);
        let cave_perlin = Perlin::new(17_711);

        let mut exposed_diamond = 0usize;
        for chunk_x in -96i32..=96 {
            let chunk = World::build_chunk_with_noise(
                chunk_x,
                Dimension::Overworld,
                "test_overworld_generation_exposes_diamond_on_deep_cave_walls",
                &perlin,
                &temp_perlin,
                &moist_perlin,
                &cave_perlin,
            );

            for y in 104usize..(CHUNK_HEIGHT - 8) {
                for x in 1..(CHUNK_WIDTH - 1) {
                    if chunk.get_block(x, y) != BlockType::DiamondOre {
                        continue;
                    }

                    if [
                        chunk.get_block(x - 1, y),
                        chunk.get_block(x + 1, y),
                        chunk.get_block(x, y - 1),
                        chunk.get_block(x, y + 1),
                    ]
                    .into_iter()
                    .any(|block| block == BlockType::Air)
                    {
                        exposed_diamond += 1;
                    }
                }
            }
        }

        assert!(
            exposed_diamond >= 30,
            "exposed diamond too sparse in deep caves: {exposed_diamond}"
        );
    }

    #[test]
    fn test_overworld_generation_builds_dungeon_spawner_and_chest() {
        let perlin = Perlin::new(1337);
        let temp_perlin = Perlin::new(7331);
        let moist_perlin = Perlin::new(9917);
        let cave_perlin = Perlin::new(4444);
        let chunk_x = 2; // Matches deterministic dungeon cadence.
        let chunk = World::build_chunk_with_noise(
            chunk_x,
            Dimension::Overworld,
            "test_dungeon_chunk",
            &perlin,
            &temp_perlin,
            &moist_perlin,
            &cave_perlin,
        );

        let mut found_spawner = false;
        let mut chest_pos = None;
        for y in 0..CHUNK_HEIGHT {
            for x in 0..CHUNK_WIDTH {
                match chunk.get_block(x, y) {
                    BlockType::ZombieSpawner | BlockType::SkeletonSpawner => found_spawner = true,
                    BlockType::Chest => chest_pos = Some((x, y)),
                    _ => {}
                }
            }
        }

        assert!(found_spawner);
        let (chest_x, chest_y) = chest_pos.expect("dungeon chest should exist");
        let chest_inv = chunk
            .chest_inventory(chest_x, chest_y)
            .expect("dungeon chest inventory should be initialized");
        assert!(chest_inv.has_item(crate::world::item::ItemType::Coal, 1));
        assert!(chest_inv.has_item(crate::world::item::ItemType::Torch, 1));
    }

    #[test]
    fn test_overworld_generation_dungeon_cadence_is_not_too_sparse() {
        let perlin = Perlin::new(1337);
        let temp_perlin = Perlin::new(7331);
        let moist_perlin = Perlin::new(9917);
        let cave_perlin = Perlin::new(4444);

        let mut dungeon_chunks = 0usize;
        for chunk_x in -20i32..=20 {
            let chunk = World::build_chunk_with_noise(
                chunk_x,
                Dimension::Overworld,
                "test_overworld_generation_dungeon_cadence_is_not_too_sparse",
                &perlin,
                &temp_perlin,
                &moist_perlin,
                &cave_perlin,
            );
            if (0..CHUNK_HEIGHT).any(|y| {
                (0..CHUNK_WIDTH).any(|x| {
                    matches!(
                        chunk.get_block(x, y),
                        BlockType::ZombieSpawner | BlockType::SkeletonSpawner
                    )
                })
            }) {
                dungeon_chunks += 1;
            }
        }

        assert!(
            dungeon_chunks >= 8,
            "expected denser dungeon cadence, got {dungeon_chunks}"
        );
    }

    #[test]
    fn test_overworld_dungeons_do_not_use_silverfish_spawners() {
        let perlin = Perlin::new(1337);
        let temp_perlin = Perlin::new(7331);
        let moist_perlin = Perlin::new(9917);
        let cave_perlin = Perlin::new(4444);

        for chunk_x in -20i32..=20 {
            let chunk = World::build_chunk_with_noise(
                chunk_x,
                Dimension::Overworld,
                "test_overworld_dungeons_do_not_use_silverfish_spawners",
                &perlin,
                &temp_perlin,
                &moist_perlin,
                &cave_perlin,
            );
            for y in 0..CHUNK_HEIGHT {
                for x in 0..CHUNK_WIDTH {
                    assert_ne!(
                        chunk.get_block(x, y),
                        BlockType::SilverfishSpawner,
                        "overworld dungeon in chunk {chunk_x} used a silverfish spawner"
                    );
                }
            }
        }
    }

    #[test]
    fn test_overworld_generation_builds_village_hut_features() {
        let perlin = Perlin::new(1337);
        let temp_perlin = Perlin::new(7331);
        let moist_perlin = Perlin::new(9917);
        let cave_perlin = Perlin::new(4444);

        let mut found_hut = false;
        for chunk_x in -220i32..=220 {
            if chunk_x.rem_euclid(11) != 5 {
                continue;
            }
            let chunk = World::build_chunk_with_noise(
                chunk_x,
                Dimension::Overworld,
                "test_village_chunk",
                &perlin,
                &temp_perlin,
                &moist_perlin,
                &cave_perlin,
            );

            let mut chest_pos = None;
            let mut has_wood_door = false;
            let mut has_glass = false;
            let mut has_station = false;
            let mut has_bed = false;
            for y in 0..CHUNK_HEIGHT {
                for x in 0..CHUNK_WIDTH {
                    match chunk.get_block(x, y) {
                        BlockType::Chest => chest_pos = Some((x, y)),
                        BlockType::WoodDoor(_) => has_wood_door = true,
                        BlockType::Glass => has_glass = true,
                        BlockType::CraftingTable => has_station = true,
                        BlockType::Bed => has_bed = true,
                        _ => {}
                    }
                }
            }

            if let Some((chest_x, chest_y)) = chest_pos
                && has_wood_door
                && has_glass
                && has_station
                && has_bed
            {
                let chest_inv = chunk
                    .chest_inventory(chest_x, chest_y)
                    .expect("village chest inventory should be initialized");
                assert!(chest_inv.has_item(crate::world::item::ItemType::Bread, 1));
                found_hut = true;
                break;
            }
        }

        assert!(found_hut);
    }

    #[test]
    fn test_overworld_generation_places_closed_side_entry_village_door() {
        let perlin = Perlin::new(1337);
        let temp_perlin = Perlin::new(7331);
        let moist_perlin = Perlin::new(9917);
        let cave_perlin = Perlin::new(4444);

        let mut found_side_entry_hut = false;
        for chunk_x in -220i32..=220 {
            if chunk_x.rem_euclid(11) != 5 {
                continue;
            }
            let chunk = World::build_chunk_with_noise(
                chunk_x,
                Dimension::Overworld,
                "test_village_chunk",
                &perlin,
                &temp_perlin,
                &moist_perlin,
                &cave_perlin,
            );

            for y in 2..CHUNK_HEIGHT {
                for x in 1..(CHUNK_WIDTH - 1) {
                    if chunk.get_block(x, y) != BlockType::WoodDoor(false) {
                        continue;
                    }
                    let left_open = chunk.get_block(x - 1, y) == BlockType::Air
                        && chunk.get_block(x - 1, y - 1) == BlockType::Air;
                    let right_open = chunk.get_block(x + 1, y) == BlockType::Air
                        && chunk.get_block(x + 1, y - 1) == BlockType::Air;
                    let left_floored = chunk.get_block(x - 1, y).is_solid()
                        && chunk.get_block(x - 1, y - 1) == BlockType::Air;
                    let right_floored = chunk.get_block(x + 1, y).is_solid()
                        && chunk.get_block(x + 1, y - 1) == BlockType::Air;
                    if (left_open && right_floored) || (right_open && left_floored) {
                        found_side_entry_hut = true;
                        break;
                    }
                }
                if found_side_entry_hut {
                    break;
                }
            }
            if found_side_entry_hut {
                break;
            }
        }

        assert!(found_side_entry_hut);
    }

    #[test]
    fn test_overworld_generation_contains_stronghold_portal_frame() {
        let mut world = World::new_for_dimension(Dimension::Overworld);
        world.load_chunks_around(STRONGHOLD_CENTER_X);

        let mut found_frame = false;
        for x in (STRONGHOLD_CENTER_X - 6)..=(STRONGHOLD_CENTER_X + 6) {
            for y in (STRONGHOLD_PORTAL_INNER_Y - 4)..=(STRONGHOLD_PORTAL_INNER_Y + 4) {
                if matches!(world.get_block(x, y), BlockType::EndPortalFrame { .. }) {
                    found_frame = true;
                    break;
                }
            }
            if found_frame {
                break;
            }
        }
        assert!(found_frame);
    }

    #[test]
    fn test_overworld_generation_contains_stronghold_branch_doors_and_spawner() {
        let mut world = World::new_for_dimension(Dimension::Overworld);
        world.load_chunks_around(STRONGHOLD_CENTER_X);

        let room_right = STRONGHOLD_CENTER_X + 18;
        let mut found_iron_door = false;
        let mut found_spawner = false;
        let mut found_east_branch_air = false;

        for x in (STRONGHOLD_CENTER_X - 64)..=(STRONGHOLD_CENTER_X + 64) {
            for y in (STRONGHOLD_ROOM_TOP_Y - 8)..=(STRONGHOLD_ROOM_BOTTOM_Y + 8) {
                match world.get_block(x, y) {
                    BlockType::IronDoor(_) => found_iron_door = true,
                    BlockType::SilverfishSpawner => found_spawner = true,
                    BlockType::Air if x > room_right + 6 => found_east_branch_air = true,
                    _ => {}
                }
                if found_iron_door && found_spawner && found_east_branch_air {
                    break;
                }
            }
            if found_iron_door && found_spawner && found_east_branch_air {
                break;
            }
        }

        assert!(found_iron_door);
        assert!(found_spawner);
        assert!(found_east_branch_air);
    }

    #[test]
    fn test_stronghold_generation_places_progression_chests() {
        let mut world = World::new_for_dimension(Dimension::Overworld);
        for sample_x in [
            STRONGHOLD_CENTER_X - 48,
            STRONGHOLD_CENTER_X,
            STRONGHOLD_CENTER_X + 48,
        ] {
            world.load_chunks_around(sample_x);
        }

        let mut chest_count = 0usize;
        let mut found_utility_loot = false;
        let mut found_progression_loot = false;
        let mut found_pre_end_supplies = false;

        for x in (STRONGHOLD_CENTER_X - 64)..=(STRONGHOLD_CENTER_X + 64) {
            for y in (STRONGHOLD_ROOM_TOP_Y - 8)..=(STRONGHOLD_ROOM_BOTTOM_Y + 8) {
                if world.get_block(x, y) != BlockType::Chest {
                    continue;
                }
                let Some(inv) = world.chest_inventory(x, y) else {
                    continue;
                };
                chest_count += 1;
                if inv.has_item(crate::world::item::ItemType::Torch, 1)
                    || inv.has_item(crate::world::item::ItemType::Bread, 1)
                {
                    found_utility_loot = true;
                }
                if inv.has_item(crate::world::item::ItemType::IronPickaxe, 1)
                    || inv.has_item(crate::world::item::ItemType::Bucket, 1)
                    || inv.has_item(crate::world::item::ItemType::RedstoneDust, 1)
                {
                    found_progression_loot = true;
                }
                if inv.has_item(crate::world::item::ItemType::Bow, 1)
                    || inv.has_item(crate::world::item::ItemType::Arrow, 6)
                    || inv.has_item(crate::world::item::ItemType::IronSword, 1)
                {
                    found_pre_end_supplies = true;
                }
            }
        }

        assert!(chest_count >= 3);
        assert!(found_utility_loot);
        assert!(found_progression_loot);
        assert!(found_pre_end_supplies);
    }

    #[test]
    fn test_stronghold_library_branch_has_shelves_and_ladder_shaft() {
        let mut world = World::new_for_dimension(Dimension::Overworld);
        world.load_chunks_around(STRONGHOLD_CENTER_X - 32);
        world.load_chunks_around(STRONGHOLD_CENTER_X);

        let room_left = STRONGHOLD_CENTER_X - 18;
        let library_room_left = room_left - 30;
        let library_room_right = room_left - 10;
        let library_room_top = (STRONGHOLD_ROOM_BOTTOM_Y - 1 - 5 - 3) - 5;
        let library_room_bottom = (STRONGHOLD_ROOM_BOTTOM_Y - 1 - 5) + 4;
        let library_shaft_x = room_left - 11;
        let mut found_planks = false;
        let mut found_ladder = false;

        for x in library_room_left..=library_room_right {
            for y in library_room_top..=library_room_bottom {
                match world.get_block(x, y) {
                    BlockType::Planks => found_planks = true,
                    BlockType::Ladder if x == library_shaft_x || x == library_room_left + 5 => {
                        found_ladder = true;
                    }
                    _ => {}
                }
            }
        }

        assert!(found_planks);
        assert!(found_ladder);
    }

    #[test]
    fn test_stronghold_portal_dais_has_glow_markers() {
        let mut world = World::new_for_dimension(Dimension::Overworld);
        world.load_chunks_around(STRONGHOLD_CENTER_X);

        let mut glow_count = 0;
        for x in (STRONGHOLD_PORTAL_INNER_X - 4)..=(STRONGHOLD_PORTAL_INNER_X + 5) {
            for y in (STRONGHOLD_PORTAL_INNER_Y - 3)..=(STRONGHOLD_PORTAL_INNER_Y + 4) {
                if world.get_block(x, y) == BlockType::Glowstone {
                    glow_count += 1;
                }
            }
        }

        assert!(glow_count >= 2);
    }

    #[test]
    fn test_end_generation_contains_endstone_and_exit_portal() {
        let mut world = World::new_for_dimension(Dimension::End);
        world.load_chunks_around(0);

        let mut found_endstone = false;
        let mut found_portal = false;
        for x in -48..=48 {
            for y in 20..80 {
                let block = world.get_block(x, y);
                if block == BlockType::EndStone {
                    found_endstone = true;
                }
                if block == BlockType::EndPortal {
                    found_portal = true;
                }
                if found_endstone && found_portal {
                    break;
                }
            }
            if found_endstone && found_portal {
                break;
            }
        }
        assert!(found_endstone);
        assert!(found_portal);
    }

    #[test]
    fn test_end_towers_have_player_passage_in_2d() {
        let mut world = World::new_for_dimension(Dimension::End);

        for tower_x in END_TOWER_XS {
            world.load_chunks_around(tower_x);
            let mut passage_floor_y = None;
            for y in 4..(CHUNK_HEIGHT as i32 - 2) {
                let center_clear = (tower_x - 1..=tower_x + 1).all(|wx| {
                    world.get_block(wx, y - 1) == BlockType::Air
                        && world.get_block(wx, y - 2) == BlockType::Air
                });
                let has_tower_shell = (tower_x - 2..=tower_x + 2).any(|wx| {
                    world.get_block(wx, y) == BlockType::Obsidian
                        || world.get_block(wx, y - 1) == BlockType::Obsidian
                        || world.get_block(wx, y - 2) == BlockType::Obsidian
                        || world.get_block(wx, y - 3) == BlockType::Obsidian
                });
                if center_clear && has_tower_shell {
                    passage_floor_y = Some(y);
                }
            }
            assert!(passage_floor_y.is_some());
        }
    }

    #[test]
    fn test_redstone_propagates_from_redstone_torch() {
        let mut world = World::new();
        world.load_chunks_around(0);

        for x in 0..8 {
            for y in 0..8 {
                world.set_block(x, y, BlockType::Air);
            }
            world.set_block(x, 4, BlockType::Stone);
        }

        world.set_block(1, 3, BlockType::RedstoneTorch(true));
        world.set_block(2, 3, BlockType::RedstoneDust(0));
        world.set_block(3, 3, BlockType::RedstoneDust(0));
        world.set_block(4, 3, BlockType::RedstoneDust(0));

        for _ in 0..6 {
            world.update_redstone(0);
        }

        assert_eq!(world.get_block(2, 3), BlockType::RedstoneDust(15));
        assert_eq!(world.get_block(3, 3), BlockType::RedstoneDust(14));
        assert_eq!(world.get_block(4, 3), BlockType::RedstoneDust(13));
    }

    #[test]
    fn test_redstone_decays_after_source_removed() {
        let mut world = World::new();
        world.load_chunks_around(0);

        for x in 0..8 {
            for y in 0..8 {
                world.set_block(x, y, BlockType::Air);
            }
            world.set_block(x, 4, BlockType::Stone);
        }

        world.set_block(1, 3, BlockType::RedstoneTorch(true));
        world.set_block(2, 3, BlockType::RedstoneDust(0));
        world.set_block(3, 3, BlockType::RedstoneDust(0));
        world.set_block(4, 3, BlockType::RedstoneDust(0));
        for _ in 0..6 {
            world.update_redstone(0);
        }

        world.set_block(1, 3, BlockType::Air);
        for _ in 0..50 {
            world.update_redstone(0);
        }

        assert_eq!(world.get_block(2, 3), BlockType::RedstoneDust(0));
        assert_eq!(world.get_block(3, 3), BlockType::RedstoneDust(0));
        assert_eq!(world.get_block(4, 3), BlockType::RedstoneDust(0));
    }

    #[test]
    fn test_stone_button_pulse_expires() {
        let mut world = World::new();
        world.load_chunks_around(0);
        world.set_block(1, 1, BlockType::StoneButton(3));

        world.update_redstone(0);
        assert_eq!(world.get_block(1, 1), BlockType::StoneButton(2));
        world.update_redstone(0);
        assert_eq!(world.get_block(1, 1), BlockType::StoneButton(1));
        world.update_redstone(0);
        assert_eq!(world.get_block(1, 1), BlockType::StoneButton(0));
    }

    #[test]
    fn test_redstone_torch_inverter_turns_off_when_powered() {
        let mut world = World::new();
        world.load_chunks_around(0);

        for x in 0..6 {
            for y in 0..6 {
                world.set_block(x, y, BlockType::Air);
            }
            world.set_block(x, 4, BlockType::Stone);
        }

        world.set_block(2, 3, BlockType::Lever(true));
        world.set_block(3, 3, BlockType::RedstoneTorch(true));
        for _ in 0..3 {
            world.update_redstone(0);
        }
        assert_eq!(world.get_block(3, 3), BlockType::RedstoneTorch(false));

        world.set_block(2, 3, BlockType::Lever(false));
        for _ in 0..3 {
            world.update_redstone(0);
        }
        assert_eq!(world.get_block(3, 3), BlockType::RedstoneTorch(true));
    }

    #[test]
    fn test_repeater_input_to_torch_is_directional() {
        let mut world = World::new();
        world.load_chunks_around(0);

        for x in 0..8 {
            for y in 0..8 {
                world.set_block(x, y, BlockType::Air);
            }
            world.set_block(x, 4, BlockType::Stone);
        }

        world.set_block(3, 3, BlockType::RedstoneTorch(true));
        world.set_block(
            4,
            3,
            BlockType::RedstoneRepeater {
                powered: true,
                delay: 1,
                facing_right: true,
                timer: 0,
                target_powered: true,
            },
        );
        for _ in 0..3 {
            world.update_redstone(0);
        }
        assert_eq!(world.get_block(3, 3), BlockType::RedstoneTorch(true));

        world.set_block(5, 3, BlockType::Lever(true));
        world.set_block(
            4,
            3,
            BlockType::RedstoneRepeater {
                powered: true,
                delay: 1,
                facing_right: false,
                timer: 0,
                target_powered: true,
            },
        );
        for _ in 0..3 {
            world.update_redstone(0);
        }
        assert_eq!(world.get_block(3, 3), BlockType::RedstoneTorch(false));
    }

    #[test]
    fn test_redstone_branches_power_both_arms() {
        let mut world = World::new();
        world.load_chunks_around(0);

        for x in 0..10 {
            for y in 0..8 {
                world.set_block(x, y, BlockType::Air);
            }
            world.set_block(x, 4, BlockType::Stone);
        }

        world.set_block(5, 3, BlockType::RedstoneTorch(true));
        world.set_block(4, 3, BlockType::RedstoneDust(0));
        world.set_block(3, 3, BlockType::RedstoneDust(0));
        world.set_block(6, 3, BlockType::RedstoneDust(0));
        world.set_block(7, 3, BlockType::RedstoneDust(0));

        for _ in 0..8 {
            world.update_redstone(0);
        }

        assert_eq!(
            world.get_block(4, 3),
            BlockType::RedstoneDust(REDSTONE_MAX_POWER)
        );
        assert_eq!(
            world.get_block(3, 3),
            BlockType::RedstoneDust(REDSTONE_MAX_POWER - 1)
        );
        assert_eq!(
            world.get_block(6, 3),
            BlockType::RedstoneDust(REDSTONE_MAX_POWER)
        );
        assert_eq!(
            world.get_block(7, 3),
            BlockType::RedstoneDust(REDSTONE_MAX_POWER - 1)
        );
    }

    #[test]
    fn test_redstone_branch_decays_after_source_removed() {
        let mut world = World::new();
        world.load_chunks_around(0);

        for x in 0..10 {
            for y in 0..8 {
                world.set_block(x, y, BlockType::Air);
            }
            world.set_block(x, 4, BlockType::Stone);
        }

        world.set_block(5, 3, BlockType::RedstoneTorch(true));
        world.set_block(4, 3, BlockType::RedstoneDust(0));
        world.set_block(3, 3, BlockType::RedstoneDust(0));
        world.set_block(6, 3, BlockType::RedstoneDust(0));
        world.set_block(7, 3, BlockType::RedstoneDust(0));
        for _ in 0..8 {
            world.update_redstone(0);
        }

        world.set_block(5, 3, BlockType::Air);
        for _ in 0..60 {
            world.update_redstone(0);
        }

        assert_eq!(world.get_block(4, 3), BlockType::RedstoneDust(0));
        assert_eq!(world.get_block(3, 3), BlockType::RedstoneDust(0));
        assert_eq!(world.get_block(6, 3), BlockType::RedstoneDust(0));
        assert_eq!(world.get_block(7, 3), BlockType::RedstoneDust(0));
    }

    #[test]
    fn test_redstone_tick_cadence_applies_on_world_update() {
        let mut world = World::new();
        world.load_chunks_around(0);

        for x in 0..8 {
            for y in 0..8 {
                world.set_block(x, y, BlockType::Air);
            }
            world.set_block(x, 4, BlockType::Stone);
        }

        world.set_block(1, 3, BlockType::RedstoneTorch(true));
        world.set_block(2, 3, BlockType::RedstoneDust(0));

        world.update(0);
        assert_eq!(world.get_block(2, 3), BlockType::RedstoneDust(0));

        world.update(0);
        assert_eq!(
            world.get_block(2, 3),
            BlockType::RedstoneDust(REDSTONE_MAX_POWER)
        );
    }

    #[test]
    fn test_redstone_power_query_reports_neighbor_sources() {
        let mut world = World::new();
        world.load_chunks_around(0);

        world.set_block(2, 2, BlockType::Lever(true));
        assert!(world.is_redstone_powered(3, 2));
        assert_eq!(world.redstone_neighbor_power(3, 2), REDSTONE_MAX_POWER);
        assert_eq!(world.redstone_power_at(2, 2), REDSTONE_MAX_POWER);
    }

    #[test]
    fn test_redstone_power_query_reports_dust_output_strength() {
        let mut world = World::new();
        world.load_chunks_around(0);

        world.set_block(1, 1, BlockType::RedstoneDust(11));
        assert_eq!(world.redstone_power_at(1, 1), 10);
    }

    #[test]
    fn test_repeater_outputs_only_in_facing_direction() {
        let mut world = World::new();
        world.load_chunks_around(0);

        for x in 0..8 {
            for y in 0..8 {
                world.set_block(x, y, BlockType::Air);
            }
            world.set_block(x, 4, BlockType::Stone);
        }

        world.set_block(
            3,
            3,
            BlockType::RedstoneRepeater {
                powered: true,
                delay: 1,
                facing_right: true,
                timer: 0,
                target_powered: true,
            },
        );

        assert!(world.is_redstone_powered(4, 3));
        assert!(!world.is_redstone_powered(2, 3));
    }

    #[test]
    fn test_repeater_applies_delay_before_state_change() {
        let mut world = World::new();
        world.load_chunks_around(0);

        for x in 0..10 {
            for y in 0..8 {
                world.set_block(x, y, BlockType::Air);
            }
            world.set_block(x, 4, BlockType::Stone);
        }

        world.set_block(2, 3, BlockType::Lever(true));
        world.set_block(
            3,
            3,
            BlockType::RedstoneRepeater {
                powered: false,
                delay: 2,
                facing_right: true,
                timer: 0,
                target_powered: false,
            },
        );
        world.set_block(4, 3, BlockType::RedstoneDust(0));

        world.update_redstone(0);
        assert_eq!(
            world.get_block(3, 3),
            BlockType::RedstoneRepeater {
                powered: false,
                delay: 2,
                facing_right: true,
                timer: 2,
                target_powered: true
            }
        );
        assert_eq!(world.get_block(4, 3), BlockType::RedstoneDust(0));

        world.update_redstone(0);
        assert_eq!(
            world.get_block(3, 3),
            BlockType::RedstoneRepeater {
                powered: false,
                delay: 2,
                facing_right: true,
                timer: 1,
                target_powered: true
            }
        );

        world.update_redstone(0);
        assert_eq!(
            world.get_block(3, 3),
            BlockType::RedstoneRepeater {
                powered: true,
                delay: 2,
                facing_right: true,
                timer: 0,
                target_powered: true
            }
        );
        assert_eq!(world.get_block(4, 3), BlockType::RedstoneDust(15));

        world.set_block(2, 3, BlockType::Lever(false));
        world.update_redstone(0);
        assert_eq!(
            world.get_block(3, 3),
            BlockType::RedstoneRepeater {
                powered: true,
                delay: 2,
                facing_right: true,
                timer: 2,
                target_powered: false
            }
        );

        world.update_redstone(0);
        world.update_redstone(0);
        assert_eq!(
            world.get_block(3, 3),
            BlockType::RedstoneRepeater {
                powered: false,
                delay: 2,
                facing_right: true,
                timer: 0,
                target_powered: false
            }
        );
        assert_eq!(world.get_block(4, 3), BlockType::RedstoneDust(0));
    }

    #[test]
    fn test_trigger_chain_powers_tnt_and_piston_output() {
        let mut world = World::new();
        world.load_chunks_around(0);

        for x in 0..12 {
            for y in 0..10 {
                world.set_block(x, y, BlockType::Air);
            }
            world.set_block(x, 6, BlockType::Stone);
        }

        world.set_block(1, 5, BlockType::Lever(true));
        world.set_block(2, 5, BlockType::RedstoneDust(0));
        world.set_block(3, 5, BlockType::RedstoneDust(0));
        world.set_block(4, 5, BlockType::RedstoneDust(0));

        world.set_block(5, 5, BlockType::Tnt);
        world.set_block(
            3,
            4,
            BlockType::Piston {
                extended: false,
                facing_right: true,
            },
        );
        world.set_block(4, 4, BlockType::Stone);

        for _ in 0..6 {
            world.update_redstone(0);
        }
        world.update_redstone_outputs(0);

        assert_eq!(world.get_block(5, 5), BlockType::PrimedTnt(TNT_FUSE_TICKS));
        assert_eq!(
            world.get_block(3, 4),
            BlockType::Piston {
                extended: true,
                facing_right: true
            }
        );
        assert_eq!(world.get_block(4, 4), BlockType::Air);
        assert_eq!(world.get_block(5, 4), BlockType::Stone);
    }

    #[test]
    fn test_tnt_primes_when_powered() {
        let mut world = World::new();
        world.load_chunks_around(0);

        for x in 0..8 {
            for y in 0..8 {
                world.set_block(x, y, BlockType::Air);
            }
            world.set_block(x, 5, BlockType::Stone);
        }

        world.set_block(2, 4, BlockType::Lever(true));
        world.set_block(3, 4, BlockType::Tnt);

        world.update_redstone_outputs(0);
        assert_eq!(world.get_block(3, 4), BlockType::PrimedTnt(TNT_FUSE_TICKS));
    }

    #[test]
    fn test_tnt_explosion_breaks_blocks() {
        let mut world = World::new();
        world.load_chunks_around(0);

        for x in 0..10 {
            for y in 0..10 {
                world.set_block(x, y, BlockType::Stone);
            }
        }

        world.set_block(5, 5, BlockType::PrimedTnt(0));
        world.set_block(5, 6, BlockType::Bedrock);

        world.update_redstone_outputs(0);

        assert_eq!(world.get_block(5, 5), BlockType::Air);
        assert_eq!(world.get_block(7, 5), BlockType::Air);
        assert_eq!(world.get_block(5, 6), BlockType::Bedrock);
    }

    #[test]
    fn test_tnt_explosion_records_event_for_entity_damage() {
        let mut world = World::new();
        world.load_chunks_around(0);
        world.set_block(4, 4, BlockType::PrimedTnt(0));

        world.update_redstone_outputs(0);

        assert!(world.recent_explosions.contains(&(4, 4, TNT_BLAST_RADIUS)));
    }

    #[test]
    fn test_tnt_explosion_resistance_preserves_obsidian() {
        let mut world = World::new();
        world.load_chunks_around(0);
        for x in 0..10 {
            for y in 0..10 {
                world.set_block(x, y, BlockType::Stone);
            }
        }
        world.set_block(6, 5, BlockType::Obsidian);
        world.set_block(5, 5, BlockType::PrimedTnt(0));

        world.update_redstone_outputs(0);

        assert_eq!(world.get_block(6, 5), BlockType::Obsidian);
    }

    #[test]
    fn test_tnt_explosion_records_block_loss_events() {
        let mut world = World::new();
        world.load_chunks_around(0);
        for x in 0..10 {
            for y in 0..10 {
                world.set_block(x, y, BlockType::Stone);
            }
        }
        world.set_block(5, 5, BlockType::PrimedTnt(0));

        world.update_redstone_outputs(0);

        assert!(!world.recent_explosion_block_losses.is_empty());
        assert!(
            world
                .recent_explosion_block_losses
                .iter()
                .any(|(_, _, block, _)| *block == BlockType::Stone)
        );
    }

    #[test]
    fn test_piston_extends_and_pushes_once() {
        let mut world = World::new();
        world.load_chunks_around(0);

        for x in 0..10 {
            for y in 0..8 {
                world.set_block(x, y, BlockType::Air);
            }
        }

        world.set_block(
            2,
            4,
            BlockType::Piston {
                extended: false,
                facing_right: true,
            },
        );
        world.set_block(3, 4, BlockType::Stone);
        world.set_block(2, 3, BlockType::Lever(true));

        world.update_redstone_outputs(0);
        assert_eq!(
            world.get_block(2, 4),
            BlockType::Piston {
                extended: true,
                facing_right: true
            }
        );
        assert_eq!(world.get_block(3, 4), BlockType::Air);
        assert_eq!(world.get_block(4, 4), BlockType::Stone);

        world.update_redstone_outputs(0);
        assert_eq!(world.get_block(4, 4), BlockType::Stone);
        assert_eq!(world.get_block(5, 4), BlockType::Air);
    }

    #[test]
    fn test_piston_retracts_when_unpowered() {
        let mut world = World::new();
        world.load_chunks_around(0);

        for x in 0..8 {
            for y in 0..8 {
                world.set_block(x, y, BlockType::Air);
            }
        }

        world.set_block(
            2,
            4,
            BlockType::Piston {
                extended: false,
                facing_right: true,
            },
        );
        world.set_block(2, 3, BlockType::Lever(true));

        world.update_redstone_outputs(0);
        world.set_block(2, 3, BlockType::Lever(false));
        world.update_redstone_outputs(0);

        assert_eq!(
            world.get_block(2, 4),
            BlockType::Piston {
                extended: false,
                facing_right: true
            }
        );
    }

    #[test]
    fn test_piston_blocked_by_immovable_block_does_not_extend() {
        let mut world = World::new();
        world.load_chunks_around(0);

        for x in 0..8 {
            for y in 0..8 {
                world.set_block(x, y, BlockType::Air);
            }
        }

        world.set_block(
            2,
            4,
            BlockType::Piston {
                extended: false,
                facing_right: true,
            },
        );
        world.set_block(3, 4, BlockType::Obsidian);
        world.set_block(2, 3, BlockType::Lever(true));

        world.update_redstone_outputs(0);

        assert_eq!(
            world.get_block(2, 4),
            BlockType::Piston {
                extended: false,
                facing_right: true
            }
        );
        assert_eq!(world.get_block(3, 4), BlockType::Obsidian);
    }

    #[test]
    fn test_piston_push_limit_prevents_extension() {
        let mut world = World::new();
        world.load_chunks_around(0);

        for x in 0..32 {
            for y in 0..8 {
                world.set_block(x, y, BlockType::Air);
            }
        }

        world.set_block(
            2,
            4,
            BlockType::Piston {
                extended: false,
                facing_right: true,
            },
        );
        for x in 3..=(3 + PISTON_PUSH_LIMIT as i32) {
            world.set_block(x, 4, BlockType::Stone);
        }
        world.set_block(2, 3, BlockType::Lever(true));

        world.update_redstone_outputs(0);

        assert_eq!(
            world.get_block(2, 4),
            BlockType::Piston {
                extended: false,
                facing_right: true
            }
        );
        assert_eq!(world.get_block(3, 4), BlockType::Stone);
        assert_eq!(
            world.get_block(3 + PISTON_PUSH_LIMIT as i32, 4),
            BlockType::Stone
        );
    }

    #[test]
    fn test_sticky_piston_pulls_block_on_retract() {
        let mut world = World::new();
        world.load_chunks_around(0);

        for x in 0..10 {
            for y in 0..8 {
                world.set_block(x, y, BlockType::Air);
            }
        }

        world.set_block(
            2,
            4,
            BlockType::StickyPiston {
                extended: false,
                facing_right: true,
            },
        );
        world.set_block(3, 4, BlockType::Stone);
        world.set_block(2, 3, BlockType::Lever(true));

        world.update_redstone_outputs(0);
        assert_eq!(world.get_block(4, 4), BlockType::Stone);

        world.set_block(2, 3, BlockType::Lever(false));
        world.update_redstone_outputs(0);

        assert_eq!(
            world.get_block(2, 4),
            BlockType::StickyPiston {
                extended: false,
                facing_right: true
            }
        );
        assert_eq!(world.get_block(3, 4), BlockType::Stone);
        assert_eq!(world.get_block(4, 4), BlockType::Air);
    }

    #[test]
    fn test_regular_piston_does_not_pull_on_retract() {
        let mut world = World::new();
        world.load_chunks_around(0);

        for x in 0..10 {
            for y in 0..8 {
                world.set_block(x, y, BlockType::Air);
            }
        }

        world.set_block(
            2,
            4,
            BlockType::Piston {
                extended: false,
                facing_right: true,
            },
        );
        world.set_block(3, 4, BlockType::Stone);
        world.set_block(2, 3, BlockType::Lever(true));

        world.update_redstone_outputs(0);
        assert_eq!(world.get_block(4, 4), BlockType::Stone);

        world.set_block(2, 3, BlockType::Lever(false));
        world.update_redstone_outputs(0);

        assert_eq!(
            world.get_block(2, 4),
            BlockType::Piston {
                extended: false,
                facing_right: true
            }
        );
        assert_eq!(world.get_block(3, 4), BlockType::Air);
        assert_eq!(world.get_block(4, 4), BlockType::Stone);
    }

    #[test]
    fn test_set_block_breaks_unsupported_flower_and_queues_drop() {
        let mut world = World::new();
        world.load_chunks_around(0);

        for x in -2..=2 {
            for y in 0..24 {
                world.set_block(x, y, BlockType::Air);
            }
            world.set_block(x, 10, BlockType::Grass);
        }
        world.recent_environment_drops.clear();

        world.set_block(0, 9, BlockType::RedFlower);
        assert_eq!(world.get_block(0, 9), BlockType::RedFlower);

        world.set_block(0, 10, BlockType::Air);

        assert_eq!(world.get_block(0, 9), BlockType::Air);
        assert!(
            world
                .recent_environment_drops
                .contains(&(0, 9, ItemType::RedFlower))
        );
    }

    #[test]
    fn test_grass_spread_turns_exposed_dirt_green() {
        let mut world = World::new();
        world.load_chunks_around(0);

        for x in -2..=2 {
            for y in 0..24 {
                world.set_block(x, y, BlockType::Air);
            }
            world.set_block(x, 10, BlockType::Grass);
        }
        world.set_block(1, 10, BlockType::Dirt);

        let spread_phase = (0_i32..=4)
            .find(|phase| {
                ((1_i32.wrapping_mul(13) ^ 10_i32.wrapping_mul(7) ^ (*phase).wrapping_mul(5)) & 3)
                    == 0
            })
            .expect("expected a deterministic spread phase");
        world.tick_counter = spread_phase as u64 * 20;
        world.update_grass_spread(0);

        assert_eq!(world.get_block(1, 10), BlockType::Grass);
    }

    #[test]
    fn test_cactus_breaks_and_drops_when_support_is_removed() {
        let mut world = World::new();
        world.load_chunks_around(0);

        for x in -2..=2 {
            for y in 0..24 {
                world.set_block(x, y, BlockType::Air);
            }
        }
        world.recent_environment_drops.clear();

        world.set_block(0, 10, BlockType::Sand);
        world.set_block(0, 9, BlockType::Cactus);
        assert_eq!(world.get_block(0, 9), BlockType::Cactus);

        world.set_block(0, 10, BlockType::Air);

        assert_eq!(world.get_block(0, 9), BlockType::Air);
        assert!(
            world
                .recent_environment_drops
                .contains(&(0, 9, ItemType::Cactus))
        );
    }

    #[test]
    fn test_leaf_decay_can_drop_saplings_after_logs_are_removed() {
        let mut world = World::new();
        world.load_chunks_around(0);

        for x in -4..=4 {
            for y in 0..28 {
                world.set_block(x, y, BlockType::Air);
            }
            world.set_block(x, 12, BlockType::Grass);
        }
        world.recent_environment_drops.clear();

        let wy = 8_i32;
        let (wx, phase) = (-8_i32..=8)
            .find_map(|wx| {
                (0_i32..=8).find_map(|phase| {
                    let decay_due =
                        ((wx.wrapping_mul(31) ^ wy.wrapping_mul(17) ^ phase.wrapping_mul(13)) & 3)
                            == 0;
                    let sapling_due =
                        ((wx.wrapping_mul(19) ^ wy.wrapping_mul(23) ^ phase.wrapping_mul(7))
                            .rem_euclid(7))
                            == 0;
                    if decay_due && sapling_due {
                        Some((wx, phase))
                    } else {
                        None
                    }
                })
            })
            .expect("expected a deterministic leaf-decay position");

        world.set_block(wx, wy, BlockType::Leaves);
        world.tick_counter = phase as u64 * 20;
        world.update_leaf_decay(0);

        assert_eq!(world.get_block(wx, wy), BlockType::Air);
        assert!(
            world
                .recent_environment_drops
                .contains(&(wx, wy, ItemType::Sapling))
        );
    }

    #[test]
    fn test_birch_leaf_decay_drops_birch_saplings() {
        let mut world = World::new();
        world.load_chunks_around(0);

        for x in -4..=4 {
            for y in 0..28 {
                world.set_block(x, y, BlockType::Air);
            }
            world.set_block(x, 12, BlockType::Grass);
        }
        world.recent_environment_drops.clear();

        let wy = 8_i32;
        let (wx, phase) = (-8_i32..=8)
            .find_map(|wx| {
                (0_i32..=8).find_map(|phase| {
                    let decay_due =
                        ((wx.wrapping_mul(31) ^ wy.wrapping_mul(17) ^ phase.wrapping_mul(13)) & 3)
                            == 0;
                    let sapling_due =
                        ((wx.wrapping_mul(19) ^ wy.wrapping_mul(23) ^ phase.wrapping_mul(7))
                            .rem_euclid(7))
                            == 0;
                    if decay_due && sapling_due {
                        Some((wx, phase))
                    } else {
                        None
                    }
                })
            })
            .expect("expected a deterministic birch leaf-decay position");

        world.set_block(wx, wy, BlockType::BirchLeaves);
        world.tick_counter = phase as u64 * 20;
        world.update_leaf_decay(0);

        assert_eq!(world.get_block(wx, wy), BlockType::Air);
        assert!(
            world
                .recent_environment_drops
                .contains(&(wx, wy, ItemType::BirchSapling))
        );
    }

    #[test]
    fn test_sapling_grows_into_tree_during_farming_tick() {
        let mut world = World::new();
        world.load_chunks_around(0);

        for x in -6..=6 {
            for y in 0..28 {
                world.set_block(x, y, BlockType::Air);
            }
            world.set_block(x, 12, BlockType::Grass);
        }

        let wy = 11_i32;
        let (wx, phase) = (-4_i32..=4)
            .find_map(|wx| {
                (0_i32..=12)
                    .find_map(|phase| sapling_growth_due_at(wx, wy, phase).then_some((wx, phase)))
            })
            .expect("expected a deterministic sapling growth position");

        world.set_block(wx, wy, BlockType::Sapling);
        world.tick_counter = phase as u64 * 20;
        world.update_farming(0);

        assert!(matches!(
            world.get_block(wx, wy),
            BlockType::Wood | BlockType::BirchWood
        ));
        let has_leaves = ((wx - 2)..=(wx + 2)).any(|leaf_x| {
            ((wy - 5)..=wy).any(|leaf_y| {
                matches!(
                    world.get_block(leaf_x, leaf_y),
                    BlockType::Leaves | BlockType::BirchLeaves
                )
            })
        });
        assert!(has_leaves);
    }

    #[test]
    fn test_birch_sapling_grows_into_birch_tree_during_farming_tick() {
        let mut world = World::new();
        world.load_chunks_around(0);

        for x in -6..=6 {
            for y in 0..28 {
                world.set_block(x, y, BlockType::Air);
            }
            world.set_block(x, 12, BlockType::Grass);
        }

        let wy = 11_i32;
        let (wx, phase) = (-4_i32..=4)
            .find_map(|wx| {
                (0_i32..=12)
                    .find_map(|phase| sapling_growth_due_at(wx, wy, phase).then_some((wx, phase)))
            })
            .expect("expected a deterministic birch sapling growth position");

        world.set_block(wx, wy, BlockType::BirchSapling);
        world.tick_counter = phase as u64 * 20;
        world.update_farming(0);

        assert_eq!(world.get_block(wx, wy), BlockType::BirchWood);
        let has_birch_leaves = ((wx - 2)..=(wx + 2)).any(|leaf_x| {
            ((wy - 5)..=wy).any(|leaf_y| world.get_block(leaf_x, leaf_y) == BlockType::BirchLeaves)
        });
        assert!(has_birch_leaves);
    }

    #[test]
    fn test_snow_biome_trees_root_on_ground_not_snow() {
        let perlin = Perlin::new(1337);
        let temp_perlin = Perlin::new(7331);
        let moist_perlin = Perlin::new(9917);
        let cave_perlin = Perlin::new(17_711);

        for chunk_x in -32..=32 {
            let chunk = World::build_chunk_with_noise(
                chunk_x,
                Dimension::Overworld,
                "test_snow_biome_trees_root_on_ground_not_snow",
                &perlin,
                &temp_perlin,
                &moist_perlin,
                &cave_perlin,
            );
            let c_start = chunk_x * CHUNK_WIDTH as i32;
            for lx in 0..CHUNK_WIDTH {
                let wx = c_start + lx as i32;
                let biome = World::biome_for_x(&temp_perlin, &moist_perlin, wx);
                if !matches!(biome, BiomeType::Tundra | BiomeType::Taiga) {
                    continue;
                }
                for y in 1..(CHUNK_HEIGHT - 1) {
                    let block = chunk.get_block(lx, y);
                    if !matches!(block, BlockType::Wood | BlockType::BirchWood) {
                        continue;
                    }
                    let below = chunk.get_block(lx, y + 1);
                    if matches!(below, BlockType::Wood | BlockType::BirchWood) {
                        continue;
                    }
                    assert_ne!(
                        below,
                        BlockType::Snow,
                        "snowy biome tree at x={wx} still rooted directly on snow"
                    );
                    assert!(matches!(below, BlockType::Dirt | BlockType::Grass));
                    return;
                }
            }
        }

        panic!("expected to find at least one snowy-biome tree root");
    }

    #[test]
    fn test_nether_wart_grows_on_soul_sand_during_farming_tick() {
        let mut world = World::new();
        world.load_chunks_around(0);

        for x in -6..=6 {
            for y in 0..24 {
                world.set_block(x, y, BlockType::Air);
            }
            world.set_block(x, 12, BlockType::Netherrack);
        }

        let wy = 11_i32;
        let wx = 1_i32;
        world.set_block(wx, wy + 1, BlockType::SoulSand);
        world.set_block(wx, wy, BlockType::NetherWart(0));

        let phase = (0_i32..=24)
            .find(|phase| nether_wart_growth_due_at(wx, wy, *phase))
            .expect("expected deterministic nether wart growth phase");

        world.tick_counter = phase as u64 * 20;
        world.update_farming(0);

        assert_eq!(world.get_block(wx, wy), BlockType::NetherWart(1));
    }

    #[test]
    fn test_dry_farmland_does_not_revert_on_first_farming_tick() {
        let mut world = World::new();
        world.load_chunks_around(0);

        for x in -4..=4 {
            for y in 0..20 {
                world.set_block(x, y, BlockType::Air);
            }
        }

        let wx = 1;
        let wy = 10;
        world.set_block(wx, wy, BlockType::Farmland(1));
        world.tick_counter = 0;
        world.update_farming(0);

        assert_eq!(world.get_block(wx, wy), BlockType::Farmland(0));
    }

    #[test]
    fn test_dry_farmland_eventually_reverts_without_crop() {
        let mut world = World::new();
        world.load_chunks_around(0);

        for x in -4..=4 {
            for y in 0..20 {
                world.set_block(x, y, BlockType::Air);
            }
        }

        let wx = 1;
        let wy = 10;
        world.set_block(wx, wy, BlockType::Farmland(1));

        for phase in 0..32 {
            world.tick_counter = phase * 20;
            world.update_farming(0);
            if world.get_block(wx, wy) == BlockType::Dirt {
                return;
            }
        }

        panic!("expected dry farmland to revert to dirt after an extended delay");
    }

    #[test]
    fn test_dry_farmland_with_crop_does_not_revert() {
        let mut world = World::new();
        world.load_chunks_around(0);

        for x in -4..=4 {
            for y in 0..20 {
                world.set_block(x, y, BlockType::Air);
            }
        }

        let wx = 1;
        let wy = 10;
        world.set_block(wx, wy, BlockType::Farmland(0));
        world.set_block(wx, wy - 1, BlockType::Crops(0));

        for phase in 0..24 {
            world.tick_counter = phase * 20;
            world.update_farming(0);
        }

        assert!(
            matches!(world.get_block(wx, wy), BlockType::Farmland(_)),
            "expected farmland under crops to stay tilled"
        );
    }

    #[test]
    fn test_support_break_drops_multiple_mature_nether_wart_items() {
        let mut world = World::new();
        world.load_chunks_around(0);

        for x in -4..=4 {
            for y in 0..20 {
                world.set_block(x, y, BlockType::Air);
            }
        }

        let wx = 2;
        let wy = 9;
        world.set_block(wx, wy + 1, BlockType::SoulSand);
        world.set_block(wx, wy, BlockType::NetherWart(3));
        world.recent_environment_drops.clear();

        world.set_block(wx, wy + 1, BlockType::Air);

        let expected = nether_wart_drop_count_at(wx, wy, true) as usize;
        let actual = world
            .recent_environment_drops
            .iter()
            .filter(|drop| matches!(drop, (x, y, ItemType::NetherWart) if *x == wx && *y == wy))
            .count();
        assert_eq!(world.get_block(wx, wy), BlockType::Air);
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_apply_chunk_column_snapshot_replaces_chunk_blocks_without_dirty_save_state() {
        let mut world = World::new();
        world.load_chunks_around(0);

        let mut blocks = vec![BlockType::Air; CHUNK_WIDTH * CHUNK_HEIGHT];
        blocks[11 * CHUNK_WIDTH + 5] = BlockType::Stone;
        blocks[12 * CHUNK_WIDTH + 6] = BlockType::Chest;

        assert!(world.apply_chunk_column_snapshot(0, &blocks));
        assert_eq!(world.get_block(5, 11), BlockType::Stone);
        assert_eq!(world.get_block(6, 12), BlockType::Chest);
        let chunk = world
            .chunks
            .get(&0)
            .expect("chunk should remain loaded after snapshot apply");
        assert!(!chunk.dirty);
    }

    #[test]
    fn test_apply_chunk_column_snapshot_rejects_invalid_payload_length() {
        let mut world = World::new();
        world.load_chunks_around(0);
        let before = world.get_block(0, 0);

        assert!(!world.apply_chunk_column_snapshot(0, &[BlockType::Air; 8]));
        assert_eq!(world.get_block(0, 0), before);
    }
}
