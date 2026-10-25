use super::block::BlockType;
use super::item::Inventory;
use crate::save_path;
use flate2::Compression;
use flate2::read::DeflateDecoder;
use flate2::write::DeflateEncoder;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, hash_map::Entry};
use std::fs::File;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

pub const CHUNK_WIDTH: usize = 32;
pub const CHUNK_HEIGHT: usize = 128;
const CHUNK_SAVE_MAGIC: &[u8; 4] = b"MCCF";
const CHUNK_SAVE_CODEC_DEFLATE: u8 = 1;
const CHUNK_SAVE_VERSION: u8 = 3;
const CHUNK_SAVE_TMP_SUFFIX: &str = ".tmp";
const MAX_CHUNK_SAVE_BYTES: u64 = 8 * 1024 * 1024;

#[derive(Serialize, Deserialize)]
struct ChestSaveData {
    local_x: usize,
    local_y: usize,
    inventory: Inventory,
}

#[derive(Serialize, Deserialize)]
struct FurnaceSaveData {
    local_x: usize,
    local_y: usize,
    storage: super::furnace::FurnaceStorage,
}

#[derive(Serialize, Deserialize)]
struct ChunkSaveDataV2 {
    version: u8,
    blocks: Vec<BlockType>,
    chests: Vec<ChestSaveData>,
}

#[derive(Serialize, Deserialize)]
struct ChunkSaveData {
    version: u8,
    blocks: Vec<BlockType>,
    chests: Vec<ChestSaveData>,
    furnaces: Vec<FurnaceSaveData>,
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
    heat_blocks: u16,
    timed_heat_blocks: u16,
    flammable_blocks: u16,
}

impl ChunkActivityCounts {
    fn add_block(&mut self, block: BlockType) {
        if block.is_flammable() {
            self.flammable_blocks += 1;
        }
        if matches!(block, BlockType::Fire(_) | BlockType::LitRedstoneOre(_)) {
            self.timed_heat_blocks += 1;
        }
        if block.has_heat_tick() {
            self.heat_blocks += 1;
        }
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
        if block.is_flammable() {
            self.flammable_blocks = self.flammable_blocks.saturating_sub(1);
        }
        if matches!(block, BlockType::Fire(_) | BlockType::LitRedstoneOre(_)) {
            self.timed_heat_blocks = self.timed_heat_blocks.saturating_sub(1);
        }
        if block.has_heat_tick() {
            self.heat_blocks = self.heat_blocks.saturating_sub(1);
        }
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
    pub(crate) furnaces: HashMap<(usize, usize), super::furnace::FurnaceStorage>,
    pub dirty: bool,
    net_revision: u64,
    save_key: String,
    save_root: PathBuf,
    activity_counts: ChunkActivityCounts,
}

impl Chunk {
    #[cfg(test)]
    fn namespaced_chunk_path(save_key: &str, x: i32) -> PathBuf {
        save_path::save_file(&format!("{save_key}_chunk_{x}.bin"))
    }

    #[cfg(test)]
    fn legacy_chunk_path(x: i32) -> PathBuf {
        save_path::save_file(&format!("chunk_{x}.bin"))
    }

    fn temp_chunk_path(path: &Path) -> PathBuf {
        let mut temp_path = path.as_os_str().to_os_string();
        temp_path.push(CHUNK_SAVE_TMP_SUFFIX);
        PathBuf::from(temp_path)
    }

    fn write_payload_atomically(path: &Path, payload: &[u8]) -> std::io::Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        crate::checkpoint::protect(path)?;
        let temp_path = Self::temp_chunk_path(path);
        {
            let mut file = File::create(&temp_path)?;
            file.write_all(payload)?;
            file.sync_all()?;
        }
        if let Err(err) = std::fs::rename(&temp_path, path) {
            let _ = std::fs::remove_file(&temp_path);
            return Err(err);
        }
        crate::checkpoint::sync_directory(path.parent().unwrap())
    }

    fn migrate_legacy_chunk_path(legacy_path: &Path, namespaced_path: &Path) {
        if !legacy_path.exists() || namespaced_path.exists() {
            return;
        }
        if let Some(parent) = namespaced_path.parent() {
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
            decoder
                .by_ref()
                .take(MAX_CHUNK_SAVE_BYTES + 1)
                .read_to_end(&mut decoded)
                .ok()?;
            if decoded.len() as u64 > MAX_CHUNK_SAVE_BYTES {
                return None;
            }
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
            && save_data.furnaces.len() <= CHUNK_WIDTH * CHUNK_HEIGHT
            && save_data
                .furnaces
                .iter()
                .all(|f| f.local_x < CHUNK_WIDTH && f.local_y < CHUNK_HEIGHT && f.storage.valid())
        {
            return Some(save_data);
        }

        if let Ok(v2) = bincode::deserialize::<ChunkSaveDataV2>(buffer)
            && v2.version == 2
            && v2.blocks.len() == CHUNK_WIDTH * CHUNK_HEIGHT
        {
            return Some(ChunkSaveData {
                version: CHUNK_SAVE_VERSION,
                blocks: v2.blocks,
                chests: v2.chests,
                furnaces: Vec::new(),
            });
        }
        let v1 = bincode::deserialize::<ChunkSaveDataV1>(buffer).ok()?;
        if v1.blocks.len() != CHUNK_WIDTH * CHUNK_HEIGHT {
            return None;
        }
        Some(ChunkSaveData {
            version: CHUNK_SAVE_VERSION,
            blocks: v1.blocks,
            chests: Vec::new(),
            furnaces: Vec::new(),
        })
    }

    pub fn new(x: i32, save_key: &str) -> Self {
        Self {
            x,
            blocks: vec![BlockType::Air; CHUNK_WIDTH * CHUNK_HEIGHT],
            chests: HashMap::new(),
            furnaces: HashMap::new(),
            dirty: false,
            net_revision: 1,
            save_key: save_key.to_string(),
            save_root: save_path::save_dir(),
            activity_counts: ChunkActivityCounts::default(),
        }
    }

    pub(crate) fn set_save_root(&mut self, root: &Path) {
        self.save_root = root.to_path_buf();
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
            if previous == BlockType::Furnace && block != BlockType::Furnace {
                self.furnaces.remove(&(x, y));
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

    pub(crate) fn has_heat_blocks(&self) -> bool {
        self.activity_counts.heat_blocks > 0
    }

    pub(crate) fn has_timed_heat(&self) -> bool {
        self.activity_counts.timed_heat_blocks > 0
    }
    pub(crate) fn has_flammable_blocks(&self) -> bool {
        self.activity_counts.flammable_blocks > 0
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
        self.furnaces
            .retain(|&(x, y), _| self.blocks[y * CHUNK_WIDTH + x] == BlockType::Furnace);
        // Remote snapshot hydration should not mark local saves dirty.
        self.dirty = false;
        self.bump_net_revision();
        true
    }

    pub fn chest_inventory(&self, x: usize, y: usize) -> Option<&Inventory> {
        self.chests.get(&(x, y))
    }

    pub fn chest_inventory_mut(&mut self, x: usize, y: usize) -> Option<&mut Inventory> {
        if self.chests.contains_key(&(x, y)) {
            self.dirty = true;
        }
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
        self.dirty = true;
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
        let _ = self.try_save_to_disk();
    }

    pub fn try_save_to_disk(&mut self) -> std::io::Result<()> {
        if !self.dirty {
            return Ok(());
        }

        let path = self
            .save_root
            .join(format!("{}_chunk_{}.bin", self.save_key, self.x));
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
            furnaces: {
                let mut entries: Vec<_> = self
                    .furnaces
                    .iter()
                    .filter(|&(&(x, y), _)| self.get_block(x, y) == BlockType::Furnace)
                    .map(|(&(local_x, local_y), storage)| FurnaceSaveData {
                        local_x,
                        local_y,
                        storage: storage.clone(),
                    })
                    .collect();
                entries.sort_by_key(|f| (f.local_y, f.local_x));
                entries
            },
        };

        let encoded = Self::encode_save_data(&save_data)
            .ok_or_else(|| std::io::Error::other("could not encode chunk"))?;
        Self::write_payload_atomically(&path, &encoded)?;
        self.dirty = false;
        Ok(())
    }

    pub(crate) fn write_recovery_copy(&self, root: &Path) -> std::io::Result<()> {
        let mut copy = Self::new(self.x, &self.save_key);
        copy.blocks = self.blocks.clone();
        copy.chests = self.chests.clone();
        copy.furnaces = self.furnaces.clone();
        copy.dirty = true;
        copy.set_save_root(root);
        copy.try_save_to_disk()
    }

    pub fn load_from_disk(x: i32, save_key: &str) -> Option<Self> {
        Self::load_from_directory(x, save_key, &save_path::save_dir())
    }

    pub(crate) fn load_from_directory(x: i32, save_key: &str, root: &Path) -> Option<Self> {
        Self::try_load_from_directory(x, save_key, root)
            .ok()
            .flatten()
    }

    /// Only an absent file means new terrain. Corruption and I/O errors must
    /// reach the world owner so existing chunks cannot be regenerated over them.
    pub(crate) fn try_load_from_directory(
        x: i32,
        save_key: &str,
        root: &Path,
    ) -> std::io::Result<Option<Self>> {
        let namespaced_path = root.join(format!("{save_key}_chunk_{x}.bin"));
        let legacy_path = root.join(format!("chunk_{x}.bin"));
        let (path, file, loaded_from_legacy) = match File::open(&namespaced_path) {
            Ok(file) => (namespaced_path.clone(), file, false),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                if save_key != "overworld" {
                    return Ok(None);
                }
                match File::open(&legacy_path) {
                    Ok(file) => (legacy_path.clone(), file, true),
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
                    Err(error) => return Err(error),
                }
            }
            Err(error) => return Err(error),
        };
        let mut buffer = Vec::new();
        file.take(MAX_CHUNK_SAVE_BYTES + 1)
            .read_to_end(&mut buffer)?;
        let save_data = (buffer.len() as u64 <= MAX_CHUNK_SAVE_BYTES)
            .then(|| Self::decode_save_data(&buffer))
            .flatten()
            .ok_or_else(|| {
                std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    format!(
                        "cannot decode {}; existing terrain was preserved",
                        path.display()
                    ),
                )
            })?;
        {
            let ChunkSaveData {
                blocks,
                chests,
                furnaces,
                ..
            } = save_data;
            let mut activity_counts = ChunkActivityCounts::default();
            for &block in &blocks {
                activity_counts.add_block(block);
            }
            let chest_map: HashMap<(usize, usize), Inventory> = chests
                .into_iter()
                .filter(|entry| {
                    entry.local_x < CHUNK_WIDTH
                        && entry.local_y < CHUNK_HEIGHT
                        && blocks[entry.local_y * CHUNK_WIDTH + entry.local_x] == BlockType::Chest
                })
                .map(|entry| ((entry.local_x, entry.local_y), entry.inventory))
                .collect();
            let furnace_map = furnaces
                .into_iter()
                .filter(|f| blocks[f.local_y * CHUNK_WIDTH + f.local_x] == BlockType::Furnace)
                .map(|f| ((f.local_x, f.local_y), f.storage))
                .collect();
            if loaded_from_legacy {
                Self::migrate_legacy_chunk_path(&legacy_path, &namespaced_path);
            }
            Ok(Some(Self {
                x,
                blocks,
                furnaces: furnace_map,
                chests: chest_map,
                dirty: false,
                net_revision: 1,
                save_key: save_key.to_string(),
                save_root: root.to_path_buf(),
                activity_counts,
            }))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn v2_chunks_import_and_furnace_queues_roundtrip_without_losing_chests() {
        use super::super::item::{ItemStack, ItemType};
        let mut blocks = vec![BlockType::Air; CHUNK_WIDTH * CHUNK_HEIGHT];
        blocks[3 * CHUNK_WIDTH + 2] = BlockType::Furnace;
        let v2 = bincode::serialize(&ChunkSaveDataV2 {
            version: 2,
            blocks: blocks.clone(),
            chests: Vec::new(),
        })
        .unwrap();
        let mut current = Chunk::decode_save_data(&v2).expect("existing v2 save imports");
        assert_eq!(current.blocks, blocks);
        assert!(current.furnaces.is_empty());
        let storage = super::super::furnace::FurnaceStorage {
            slots: [
                Some(ItemStack {
                    item_type: ItemType::RawIron,
                    count: 3,
                    durability: None,
                }),
                None,
                None,
            ],
            ..Default::default()
        };
        current.furnaces.push(FurnaceSaveData {
            local_x: 2,
            local_y: 3,
            storage: storage.clone(),
        });
        let payload = Chunk::encode_save_data(&current).unwrap();
        let decoded = Chunk::decode_save_data(&payload).unwrap();
        assert_eq!(decoded.furnaces[0].storage, storage);
        current.furnaces[0].storage.slots[0].as_mut().unwrap().count = 10000;
        assert!(
            Chunk::decode_save_data(&Chunk::encode_save_data(&current).unwrap()).is_none(),
            "invalid queue must be preserved/rejected, not loaded"
        );
    }

    fn chunk_path(save_key: &str, x: i32) -> PathBuf {
        Chunk::namespaced_chunk_path(save_key, x)
    }

    fn legacy_chunk_path(x: i32) -> PathBuf {
        Chunk::legacy_chunk_path(x)
    }

    fn temp_path(save_key: &str, x: i32) -> PathBuf {
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

        let _ = std::fs::create_dir_all(save_path::save_dir());
        let mut blocks = vec![BlockType::Air; CHUNK_WIDTH * CHUNK_HEIGHT];
        blocks[5 * CHUNK_WIDTH + 4] = BlockType::GoldOre;
        let payload = bincode::serialize(&ChunkSaveDataV1 { blocks })
            .expect("legacy payload should serialize");
        std::fs::write(&path, payload).expect("legacy payload should write");

        assert!(path.exists());
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
    fn chest_inventory_only_edits_save_after_a_clean_checkpoint() {
        let root = std::env::temp_dir().join(format!("tc-chest-dirty-{}", rand::random::<u64>()));
        std::fs::create_dir_all(&root).unwrap();
        let mut chunk = Chunk::new(0, "overworld");
        chunk.set_save_root(&root);
        chunk.set_block(4, 6, BlockType::Chest);
        chunk
            .ensure_chest_inventory(4, 6, 27)
            .unwrap()
            .add_item(super::super::item::ItemType::Diamond, 3);
        chunk.try_save_to_disk().unwrap();
        assert!(!chunk.dirty);
        chunk.chest_inventory_mut(4, 6).unwrap().slots.fill(None);
        assert!(chunk.dirty);
        chunk.try_save_to_disk().unwrap();
        let loaded = Chunk::try_load_from_directory(0, "overworld", &root)
            .unwrap()
            .unwrap();
        assert!(
            loaded
                .chest_inventory(4, 6)
                .unwrap()
                .slots
                .iter()
                .all(Option::is_none)
        );
        chunk
            .ensure_chest_inventory(4, 6, 27)
            .unwrap()
            .add_item(super::super::item::ItemType::Bread, 1);
        assert!(chunk.dirty);
        std::fs::remove_dir_all(root).unwrap();
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

        assert!(path.exists());
        assert!(!temp.exists());

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

        let _ = std::fs::create_dir_all(save_path::save_dir());
        let mut blocks = vec![BlockType::Air; CHUNK_WIDTH * CHUNK_HEIGHT];
        blocks[3 * CHUNK_WIDTH + 2] = BlockType::DiamondOre;
        let payload = bincode::serialize(&ChunkSaveDataV1 { blocks })
            .expect("legacy payload should serialize");
        std::fs::write(&legacy, payload).expect("legacy payload should write");

        let loaded =
            Chunk::load_from_disk(chunk_x, save_key).expect("legacy overworld chunk should load");
        assert_eq!(loaded.get_block(2, 3), BlockType::DiamondOre);
        assert!(namespaced.exists());
        assert!(!legacy.exists());

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

        let _ = std::fs::create_dir_all(save_path::save_dir());
        let mut blocks = vec![BlockType::Air; CHUNK_WIDTH * CHUNK_HEIGHT];
        blocks[2 * CHUNK_WIDTH + 1] = BlockType::GoldOre;
        let payload = bincode::serialize(&ChunkSaveDataV1 { blocks })
            .expect("legacy payload should serialize");
        std::fs::write(&legacy, payload).expect("legacy payload should write");

        let loaded = Chunk::load_from_disk(chunk_x, save_key);
        assert!(loaded.is_none());
        assert!(legacy.exists());
        assert!(!namespaced.exists());

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

        let _ = std::fs::create_dir_all(save_path::save_dir());

        let mut namespaced_blocks = vec![BlockType::Air; CHUNK_WIDTH * CHUNK_HEIGHT];
        namespaced_blocks[4 * CHUNK_WIDTH + 3] = BlockType::DiamondOre;
        let namespaced_payload = Chunk::encode_save_data(&ChunkSaveData {
            version: CHUNK_SAVE_VERSION,
            blocks: namespaced_blocks,
            chests: Vec::new(),
            furnaces: Vec::new(),
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
        assert!(namespaced.exists());
        assert!(legacy.exists());

        let _ = std::fs::remove_file(namespaced);
        let _ = std::fs::remove_file(legacy);
    }
}
