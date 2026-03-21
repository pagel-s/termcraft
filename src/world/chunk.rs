use super::block::BlockType;
use super::item::Inventory;
use flate2::Compression;
use flate2::read::DeflateDecoder;
use flate2::write::DeflateEncoder;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, hash_map::Entry};
use std::fs::File;
use std::io::{Read, Write};
use std::path::Path;

pub const CHUNK_WIDTH: usize = 32;
pub const CHUNK_HEIGHT: usize = 128;
const CHUNK_SAVE_MAGIC: &[u8; 4] = b"MCCF";
const CHUNK_SAVE_CODEC_DEFLATE: u8 = 1;
const CHUNK_SAVE_VERSION: u8 = 2;
const CHUNK_SAVE_DIR: &str = "saves";
const CHUNK_SAVE_TMP_SUFFIX: &str = ".tmp";

#[derive(Serialize, Deserialize)]
struct ChestSaveData {
    local_x: usize,
    local_y: usize,
    inventory: Inventory,
}

#[derive(Serialize, Deserialize)]
struct ChunkSaveData {
    version: u8,
    blocks: Vec<BlockType>,
    chests: Vec<ChestSaveData>,
}

#[derive(Serialize, Deserialize)]
struct ChunkSaveDataV1 {
    blocks: Vec<BlockType>,
}

#[derive(Clone, Copy, Default)]
struct ChunkActivityCounts {
    fluid_blocks: u16,
    falling_blocks: u16,
    leaf_blocks: u16,
    farming_blocks: u16,
    redstone_blocks: u16,
}

impl ChunkActivityCounts {
    fn add_block(&mut self, block: BlockType) {
        if block.is_fluid() {
            self.fluid_blocks = self.fluid_blocks.saturating_add(1);
        }
        if block.obeys_gravity() {
            self.falling_blocks = self.falling_blocks.saturating_add(1);
        }
        if block.is_leaf_block() {
            self.leaf_blocks = self.leaf_blocks.saturating_add(1);
        }
        if block.participates_in_farming_tick() {
            self.farming_blocks = self.farming_blocks.saturating_add(1);
        }
        if block.participates_in_redstone_tick() {
            self.redstone_blocks = self.redstone_blocks.saturating_add(1);
        }
    }

    fn remove_block(&mut self, block: BlockType) {
        if block.is_fluid() {
            self.fluid_blocks = self.fluid_blocks.saturating_sub(1);
        }
        if block.obeys_gravity() {
            self.falling_blocks = self.falling_blocks.saturating_sub(1);
        }
        if block.is_leaf_block() {
            self.leaf_blocks = self.leaf_blocks.saturating_sub(1);
        }
        if block.participates_in_farming_tick() {
            self.farming_blocks = self.farming_blocks.saturating_sub(1);
        }
        if block.participates_in_redstone_tick() {
            self.redstone_blocks = self.redstone_blocks.saturating_sub(1);
        }
    }
}

pub struct Chunk {
    pub x: i32,
    pub blocks: Vec<BlockType>,
    pub chests: HashMap<(usize, usize), Inventory>,
    pub dirty: bool,
    net_revision: u64,
    save_key: String,
    activity_counts: ChunkActivityCounts,
}

impl Chunk {
    fn namespaced_chunk_path(save_key: &str, x: i32) -> String {
        format!("{CHUNK_SAVE_DIR}/{}_chunk_{}.bin", save_key, x)
    }

    fn legacy_chunk_path(x: i32) -> String {
        format!("{CHUNK_SAVE_DIR}/chunk_{}.bin", x)
    }

    fn temp_chunk_path(path: &str) -> String {
        format!("{path}{CHUNK_SAVE_TMP_SUFFIX}")
    }

    fn write_payload_atomically(path: &str, payload: &[u8]) -> std::io::Result<()> {
        if let Some(parent) = Path::new(path).parent() {
            std::fs::create_dir_all(parent)?;
        }
        let temp_path = Self::temp_chunk_path(path);
        {
            let mut file = File::create(&temp_path)?;
            file.write_all(payload)?;
            file.flush()?;
        }
        if let Err(err) = std::fs::rename(&temp_path, path) {
            let _ = std::fs::remove_file(&temp_path);
            return Err(err);
        }
        Ok(())
    }

    fn migrate_legacy_chunk_path(legacy_path: &str, namespaced_path: &str) {
        if !Path::new(legacy_path).exists() || Path::new(namespaced_path).exists() {
            return;
        }
        if let Some(parent) = Path::new(namespaced_path).parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if std::fs::rename(legacy_path, namespaced_path).is_err()
            && std::fs::copy(legacy_path, namespaced_path).is_ok()
        {
            let _ = std::fs::remove_file(legacy_path);
        }
    }

    fn bump_net_revision(&mut self) {
        self.net_revision = self.net_revision.wrapping_add(1);
        if self.net_revision == 0 {
            self.net_revision = 1;
        }
    }

    fn encode_save_data(save_data: &ChunkSaveData) -> Option<Vec<u8>> {
        let encoded = bincode::serialize(save_data).ok()?;
        let mut deflater = DeflateEncoder::new(Vec::new(), Compression::default());
        deflater.write_all(&encoded).ok()?;
        let compressed = deflater.finish().ok()?;

        let mut payload = Vec::with_capacity(CHUNK_SAVE_MAGIC.len() + 1 + compressed.len());
        payload.extend_from_slice(CHUNK_SAVE_MAGIC);
        payload.push(CHUNK_SAVE_CODEC_DEFLATE);
        payload.extend_from_slice(&compressed);
        Some(payload)
    }

    fn decode_save_data(buffer: &[u8]) -> Option<ChunkSaveData> {
        if buffer.len() > CHUNK_SAVE_MAGIC.len()
            && &buffer[..CHUNK_SAVE_MAGIC.len()] == CHUNK_SAVE_MAGIC
        {
            let codec = buffer[CHUNK_SAVE_MAGIC.len()];
            if codec != CHUNK_SAVE_CODEC_DEFLATE {
                return None;
            }
            let mut decoder = DeflateDecoder::new(&buffer[(CHUNK_SAVE_MAGIC.len() + 1)..]);
            let mut decoded = Vec::new();
            decoder.read_to_end(&mut decoded).ok()?;
            if let Some(save_data) = Self::decode_save_data_payload(&decoded) {
                return Some(save_data);
            }
            return None;
        }

        Self::decode_save_data_payload(buffer)
    }

    fn decode_save_data_payload(buffer: &[u8]) -> Option<ChunkSaveData> {
        if let Ok(save_data) = bincode::deserialize::<ChunkSaveData>(buffer)
            && save_data.version == CHUNK_SAVE_VERSION
            && save_data.blocks.len() == CHUNK_WIDTH * CHUNK_HEIGHT
        {
            return Some(save_data);
        }

        let v1 = bincode::deserialize::<ChunkSaveDataV1>(buffer).ok()?;
        if v1.blocks.len() != CHUNK_WIDTH * CHUNK_HEIGHT {
            return None;
        }
        Some(ChunkSaveData {
            version: CHUNK_SAVE_VERSION,
            blocks: v1.blocks,
            chests: Vec::new(),
        })
    }

    pub fn new(x: i32, save_key: &str) -> Self {
        Self {
            x,
            blocks: vec![BlockType::Air; CHUNK_WIDTH * CHUNK_HEIGHT],
            chests: HashMap::new(),
            dirty: false,
            net_revision: 1,
            save_key: save_key.to_string(),
            activity_counts: ChunkActivityCounts::default(),
        }
    }

    fn rebuild_activity_counts(&mut self) {
        let mut counts = ChunkActivityCounts::default();
        for &block in &self.blocks {
            counts.add_block(block);
        }
        self.activity_counts = counts;
    }

    pub fn get_block(&self, x: usize, y: usize) -> BlockType {
        if x < CHUNK_WIDTH && y < CHUNK_HEIGHT {
            self.blocks[y * CHUNK_WIDTH + x]
        } else {
            BlockType::Air
        }
    }

    pub fn set_block(&mut self, x: usize, y: usize, block: BlockType) {
        if x < CHUNK_WIDTH && y < CHUNK_HEIGHT {
            let idx = y * CHUNK_WIDTH + x;
            let previous = self.blocks[idx];
            if previous == block {
                return;
            }
            self.activity_counts.remove_block(previous);
            self.blocks[idx] = block;
            self.activity_counts.add_block(block);
            if previous == BlockType::Chest && block != BlockType::Chest {
                self.chests.remove(&(x, y));
            } else if previous != BlockType::Chest && block == BlockType::Chest {
                self.chests
                    .entry((x, y))
                    .or_insert_with(|| Inventory::new(27));
            }
            self.dirty = true;
            self.bump_net_revision();
        }
    }

    pub fn blocks_revision(&self) -> u64 {
        self.net_revision
    }

    pub fn has_fluids(&self) -> bool {
        self.activity_counts.fluid_blocks > 0
    }

    pub fn has_falling_blocks(&self) -> bool {
        self.activity_counts.falling_blocks > 0
    }

    pub fn has_leaf_blocks(&self) -> bool {
        self.activity_counts.leaf_blocks > 0
    }

    pub fn has_farming_blocks(&self) -> bool {
        self.activity_counts.farming_blocks > 0
    }

    pub fn has_redstone_blocks(&self) -> bool {
        self.activity_counts.redstone_blocks > 0
    }

    pub fn apply_block_snapshot(&mut self, blocks: &[BlockType]) -> bool {
        if blocks.len() != CHUNK_WIDTH * CHUNK_HEIGHT {
            return false;
        }
        if self.blocks.as_slice() == blocks {
            return true;
        }

        self.blocks.copy_from_slice(blocks);
        self.rebuild_activity_counts();
        let mut stale_chest_slots = Vec::new();
        for &(x, y) in self.chests.keys() {
            if self.blocks[y * CHUNK_WIDTH + x] != BlockType::Chest {
                stale_chest_slots.push((x, y));
            }
        }
        for slot in stale_chest_slots {
            self.chests.remove(&slot);
        }
        // Remote snapshot hydration should not mark local saves dirty.
        self.dirty = false;
        self.bump_net_revision();
        true
    }

    pub fn chest_inventory(&self, x: usize, y: usize) -> Option<&Inventory> {
        self.chests.get(&(x, y))
    }

    pub fn chest_inventory_mut(&mut self, x: usize, y: usize) -> Option<&mut Inventory> {
        self.chests.get_mut(&(x, y))
    }

    pub fn ensure_chest_inventory(
        &mut self,
        x: usize,
        y: usize,
        capacity: usize,
    ) -> Option<&mut Inventory> {
        if self.get_block(x, y) != BlockType::Chest {
            return None;
        }
        match self.chests.entry((x, y)) {
            Entry::Occupied(entry) => Some(entry.into_mut()),
            Entry::Vacant(entry) => {
                self.dirty = true;
                Some(entry.insert(Inventory::new(capacity)))
            }
        }
    }

    pub fn remove_chest_inventory(&mut self, x: usize, y: usize) -> Option<Inventory> {
        let removed = self.chests.remove(&(x, y));
        if removed.is_some() {
            self.dirty = true;
        }
        removed
    }

    pub fn save_to_disk(&mut self) {
        if !self.dirty {
            return;
        }

        let path = Self::namespaced_chunk_path(&self.save_key, self.x);
        let mut chests: Vec<ChestSaveData> = self
            .chests
            .iter()
            .filter_map(|(&(local_x, local_y), inventory)| {
                if self.get_block(local_x, local_y) == BlockType::Chest {
                    Some(ChestSaveData {
                        local_x,
                        local_y,
                        inventory: inventory.clone(),
                    })
                } else {
                    None
                }
            })
            .collect();
        chests.sort_by_key(|entry| (entry.local_y, entry.local_x));
        let save_data = ChunkSaveData {
            version: CHUNK_SAVE_VERSION,
            blocks: self.blocks.clone(),
            chests,
        };

        if let Some(encoded) = Self::encode_save_data(&save_data)
            && Self::write_payload_atomically(&path, &encoded).is_ok()
        {
            self.dirty = false;
        }
    }

    pub fn load_from_disk(x: i32, save_key: &str) -> Option<Self> {
        let namespaced_path = Self::namespaced_chunk_path(save_key, x);
        let legacy_path = Self::legacy_chunk_path(x);
        let (path, loaded_from_legacy) = if Path::new(&namespaced_path).exists() {
            (namespaced_path.clone(), false)
        } else if save_key == "overworld" && Path::new(&legacy_path).exists() {
            (legacy_path.clone(), true)
        } else {
            (namespaced_path.clone(), false)
        };
        if let Ok(mut file) = File::open(path) {
            let mut buffer = Vec::new();
            if file.read_to_end(&mut buffer).is_ok()
                && let Some(save_data) = Self::decode_save_data(&buffer)
            {
                let ChunkSaveData { blocks, chests, .. } = save_data;
                let mut activity_counts = ChunkActivityCounts::default();
                for &block in &blocks {
                    activity_counts.add_block(block);
                }
                let chest_map: HashMap<(usize, usize), Inventory> = chests
                    .into_iter()
                    .filter(|entry| {
                        entry.local_x < CHUNK_WIDTH
                            && entry.local_y < CHUNK_HEIGHT
                            && blocks[entry.local_y * CHUNK_WIDTH + entry.local_x]
                                == BlockType::Chest
                    })
                    .map(|entry| ((entry.local_x, entry.local_y), entry.inventory))
                    .collect();
                if loaded_from_legacy {
                    Self::migrate_legacy_chunk_path(&legacy_path, &namespaced_path);
                }
                return Some(Self {
                    x,
                    blocks,
                    chests: chest_map,
                    dirty: false,
                    net_revision: 1,
                    save_key: save_key.to_string(),
                    activity_counts,
                });
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn chunk_path(save_key: &str, x: i32) -> String {
        Chunk::namespaced_chunk_path(save_key, x)
    }

    fn legacy_chunk_path(x: i32) -> String {
        Chunk::legacy_chunk_path(x)
    }

    fn temp_path(save_key: &str, x: i32) -> String {
        Chunk::temp_chunk_path(&chunk_path(save_key, x))
    }

    #[test]
    fn test_chunk_save_uses_deflate_header_and_roundtrips() {
        let save_key = "chunkdeflate";
        let chunk_x = 710_001;
        let path = chunk_path(save_key, chunk_x);
        let _ = std::fs::remove_file(&path);

        let mut chunk = Chunk::new(chunk_x, save_key);
        chunk.set_block(2, 3, BlockType::Stone);
        chunk.set_block(7, 9, BlockType::RedFlower);
        chunk.save_to_disk();

        let bytes = std::fs::read(&path).expect("saved chunk should be readable");
        assert!(bytes.starts_with(CHUNK_SAVE_MAGIC));
        assert_eq!(bytes[CHUNK_SAVE_MAGIC.len()], CHUNK_SAVE_CODEC_DEFLATE);

        let loaded = Chunk::load_from_disk(chunk_x, save_key).expect("chunk should load");
        assert_eq!(loaded.get_block(2, 3), BlockType::Stone);
        assert_eq!(loaded.get_block(7, 9), BlockType::RedFlower);

        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn test_chunk_loader_accepts_legacy_uncompressed_format() {
        let save_key = "chunklegacy";
        let chunk_x = 710_002;
        let path = chunk_path(save_key, chunk_x);
        let _ = std::fs::remove_file(&path);

        let _ = std::fs::create_dir_all(CHUNK_SAVE_DIR);
        let mut blocks = vec![BlockType::Air; CHUNK_WIDTH * CHUNK_HEIGHT];
        blocks[5 * CHUNK_WIDTH + 4] = BlockType::GoldOre;
        let payload = bincode::serialize(&ChunkSaveDataV1 { blocks })
            .expect("legacy payload should serialize");
        std::fs::write(&path, payload).expect("legacy payload should write");

        assert!(Path::new(&path).exists());
        let loaded = Chunk::load_from_disk(chunk_x, save_key).expect("legacy chunk should load");
        assert_eq!(loaded.get_block(4, 5), BlockType::GoldOre);

        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn test_chunk_chest_inventory_roundtrips() {
        let save_key = "chunkchest";
        let chunk_x = 710_003;
        let path = chunk_path(save_key, chunk_x);
        let _ = std::fs::remove_file(&path);

        let mut chunk = Chunk::new(chunk_x, save_key);
        chunk.set_block(4, 6, BlockType::Chest);
        {
            let chest = chunk
                .ensure_chest_inventory(4, 6, 27)
                .expect("chest inventory should initialize");
            chest.add_item(super::super::item::ItemType::Diamond, 3);
        }
        chunk.save_to_disk();

        let loaded = Chunk::load_from_disk(chunk_x, save_key).expect("chunk should load");
        let chest = loaded
            .chest_inventory(4, 6)
            .expect("loaded chest inventory should be present");
        assert!(chest.has_item(super::super::item::ItemType::Diamond, 3));

        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn test_chunk_save_does_not_leave_temporary_file() {
        let save_key = "chunkatomic";
        let chunk_x = 710_004;
        let path = chunk_path(save_key, chunk_x);
        let temp = temp_path(save_key, chunk_x);
        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_file(&temp);

        let mut chunk = Chunk::new(chunk_x, save_key);
        chunk.set_block(1, 1, BlockType::Stone);
        chunk.save_to_disk();

        assert!(Path::new(&path).exists());
        assert!(!Path::new(&temp).exists());

        let _ = std::fs::remove_file(path);
        let _ = std::fs::remove_file(temp);
    }

    #[test]
    fn test_chunk_revision_increments_only_on_real_block_change() {
        let mut chunk = Chunk::new(710_099, "chunkrev");
        let baseline = chunk.blocks_revision();

        chunk.set_block(1, 1, BlockType::Air);
        assert_eq!(chunk.blocks_revision(), baseline);
        assert!(!chunk.dirty);

        chunk.set_block(1, 1, BlockType::Stone);
        assert!(chunk.blocks_revision() > baseline);
        assert!(chunk.dirty);
        let changed = chunk.blocks_revision();

        chunk.set_block(1, 1, BlockType::Stone);
        assert_eq!(chunk.blocks_revision(), changed);
    }

    #[test]
    fn test_chunk_activity_counts_track_dynamic_block_presence() {
        let mut chunk = Chunk::new(710_100, "chunkactivity");
        assert!(!chunk.has_fluids());
        assert!(!chunk.has_falling_blocks());
        assert!(!chunk.has_leaf_blocks());
        assert!(!chunk.has_farming_blocks());
        assert!(!chunk.has_redstone_blocks());

        chunk.set_block(1, 1, BlockType::Water(8));
        chunk.set_block(2, 1, BlockType::Sand);
        chunk.set_block(3, 1, BlockType::Leaves);
        chunk.set_block(4, 1, BlockType::Farmland(0));
        chunk.set_block(5, 1, BlockType::RedstoneDust(0));

        assert!(chunk.has_fluids());
        assert!(chunk.has_falling_blocks());
        assert!(chunk.has_leaf_blocks());
        assert!(chunk.has_farming_blocks());
        assert!(chunk.has_redstone_blocks());

        chunk.set_block(1, 1, BlockType::Air);
        chunk.set_block(2, 1, BlockType::Air);
        chunk.set_block(3, 1, BlockType::Air);
        chunk.set_block(4, 1, BlockType::Air);
        chunk.set_block(5, 1, BlockType::Air);

        assert!(!chunk.has_fluids());
        assert!(!chunk.has_falling_blocks());
        assert!(!chunk.has_leaf_blocks());
        assert!(!chunk.has_farming_blocks());
        assert!(!chunk.has_redstone_blocks());
    }

    #[test]
    fn test_apply_block_snapshot_rebuilds_activity_counts() {
        let mut chunk = Chunk::new(710_101, "chunksnapshotactivity");
        let mut blocks = vec![BlockType::Air; CHUNK_WIDTH * CHUNK_HEIGHT];
        blocks[CHUNK_WIDTH + 1] = BlockType::Lava(8);
        blocks[CHUNK_WIDTH + 2] = BlockType::Gravel;
        blocks[CHUNK_WIDTH + 3] = BlockType::BirchLeaves;
        blocks[CHUNK_WIDTH + 4] = BlockType::Crops(3);
        blocks[CHUNK_WIDTH + 5] = BlockType::Piston {
            extended: false,
            facing_right: true,
        };

        assert!(chunk.apply_block_snapshot(&blocks));
        assert!(chunk.has_fluids());
        assert!(chunk.has_falling_blocks());
        assert!(chunk.has_leaf_blocks());
        assert!(chunk.has_farming_blocks());
        assert!(chunk.has_redstone_blocks());
    }

    #[test]
    fn test_overworld_legacy_chunk_path_is_migrated() {
        let save_key = "overworld";
        let chunk_x = 710_005;
        let namespaced = chunk_path(save_key, chunk_x);
        let legacy = legacy_chunk_path(chunk_x);
        let _ = std::fs::remove_file(&namespaced);
        let _ = std::fs::remove_file(&legacy);

        let _ = std::fs::create_dir_all(CHUNK_SAVE_DIR);
        let mut blocks = vec![BlockType::Air; CHUNK_WIDTH * CHUNK_HEIGHT];
        blocks[3 * CHUNK_WIDTH + 2] = BlockType::DiamondOre;
        let payload = bincode::serialize(&ChunkSaveDataV1 { blocks })
            .expect("legacy payload should serialize");
        std::fs::write(&legacy, payload).expect("legacy payload should write");

        let loaded =
            Chunk::load_from_disk(chunk_x, save_key).expect("legacy overworld chunk should load");
        assert_eq!(loaded.get_block(2, 3), BlockType::DiamondOre);
        assert!(Path::new(&namespaced).exists());
        assert!(!Path::new(&legacy).exists());

        let _ = std::fs::remove_file(namespaced);
        let _ = std::fs::remove_file(legacy);
    }

    #[test]
    fn test_non_overworld_load_does_not_consume_legacy_path() {
        let save_key = "nether";
        let chunk_x = 710_006;
        let namespaced = chunk_path(save_key, chunk_x);
        let legacy = legacy_chunk_path(chunk_x);
        let _ = std::fs::remove_file(&namespaced);
        let _ = std::fs::remove_file(&legacy);

        let _ = std::fs::create_dir_all(CHUNK_SAVE_DIR);
        let mut blocks = vec![BlockType::Air; CHUNK_WIDTH * CHUNK_HEIGHT];
        blocks[2 * CHUNK_WIDTH + 1] = BlockType::GoldOre;
        let payload = bincode::serialize(&ChunkSaveDataV1 { blocks })
            .expect("legacy payload should serialize");
        std::fs::write(&legacy, payload).expect("legacy payload should write");

        let loaded = Chunk::load_from_disk(chunk_x, save_key);
        assert!(loaded.is_none());
        assert!(Path::new(&legacy).exists());
        assert!(!Path::new(&namespaced).exists());

        let _ = std::fs::remove_file(namespaced);
        let _ = std::fs::remove_file(legacy);
    }

    #[test]
    fn test_overworld_loader_prefers_namespaced_file_over_legacy() {
        let save_key = "overworld";
        let chunk_x = 710_007;
        let namespaced = chunk_path(save_key, chunk_x);
        let legacy = legacy_chunk_path(chunk_x);
        let _ = std::fs::remove_file(&namespaced);
        let _ = std::fs::remove_file(&legacy);

        let _ = std::fs::create_dir_all(CHUNK_SAVE_DIR);

        let mut namespaced_blocks = vec![BlockType::Air; CHUNK_WIDTH * CHUNK_HEIGHT];
        namespaced_blocks[4 * CHUNK_WIDTH + 3] = BlockType::DiamondOre;
        let namespaced_payload = Chunk::encode_save_data(&ChunkSaveData {
            version: CHUNK_SAVE_VERSION,
            blocks: namespaced_blocks,
            chests: Vec::new(),
        })
        .expect("namespaced payload should serialize");
        std::fs::write(&namespaced, namespaced_payload).expect("namespaced payload should write");

        let mut legacy_blocks = vec![BlockType::Air; CHUNK_WIDTH * CHUNK_HEIGHT];
        legacy_blocks[4 * CHUNK_WIDTH + 3] = BlockType::GoldOre;
        let legacy_payload = bincode::serialize(&ChunkSaveDataV1 {
            blocks: legacy_blocks,
        })
        .expect("legacy payload should serialize");
        std::fs::write(&legacy, legacy_payload).expect("legacy payload should write");

        let loaded = Chunk::load_from_disk(chunk_x, save_key).expect("chunk should load");
        assert_eq!(loaded.get_block(3, 4), BlockType::DiamondOre);
        assert!(Path::new(&namespaced).exists());
        assert!(Path::new(&legacy).exists());

        let _ = std::fs::remove_file(namespaced);
        let _ = std::fs::remove_file(legacy);
    }
}
