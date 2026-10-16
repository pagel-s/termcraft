//! Declarative, bounded world designs. Models supply data; the engine builds blocks.
use super::block::BlockType;
use super::chunk::{CHUNK_HEIGHT, CHUNK_WIDTH, Chunk};
use noise::{NoiseFn, Perlin};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::io::Read;
use std::path::Path;

pub const RECIPE_VERSION: u32 = 1;
pub const RECIPE_FILE: &str = "world.json";
pub const MAX_RECIPE_BYTES: usize = 256 * 1024;

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Terrain {
    Plains,
    Forest,
    Desert,
    Tundra,
    Mountains,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BlueprintBlock {
    Air,
    Stone,
    Dirt,
    Grass,
    Sand,
    Snow,
    Ice,
    Wood,
    Leaves,
    Planks,
    Cobblestone,
    StoneBricks,
    Glass,
    Ladder,
    Torch,
    CraftingTable,
    Furnace,
    Chest,
    CoalOre,
    IronOre,
    GoldOre,
    DiamondOre,
    Glowstone,
}

impl BlueprintBlock {
    pub(crate) fn block(self) -> BlockType {
        match self {
            Self::Air => BlockType::Air,
            Self::Stone => BlockType::Stone,
            Self::Dirt => BlockType::Dirt,
            Self::Grass => BlockType::Grass,
            Self::Sand => BlockType::Sand,
            Self::Snow => BlockType::Snow,
            Self::Ice => BlockType::Ice,
            Self::Wood => BlockType::Wood,
            Self::Leaves => BlockType::Leaves,
            Self::Planks => BlockType::Planks,
            Self::Cobblestone => BlockType::Cobblestone,
            Self::StoneBricks => BlockType::StoneBricks,
            Self::Glass => BlockType::Glass,
            Self::Ladder => BlockType::Ladder,
            Self::Torch => BlockType::Torch,
            Self::CraftingTable => BlockType::CraftingTable,
            Self::Furnace => BlockType::Furnace,
            Self::Chest => BlockType::Chest,
            Self::CoalOre => BlockType::CoalOre,
            Self::IronOre => BlockType::IronOre,
            Self::GoldOre => BlockType::GoldOre,
            Self::DiamondOre => BlockType::DiamondOre,
            Self::Glowstone => BlockType::Glowstone,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Region {
    pub start_x: i32,
    pub end_x: i32,
    pub terrain: Terrain,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct BlockRect {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    pub block: BlueprintBlock,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Landmark {
    pub name: String,
    pub x: i32,
    /// Rectangles are relative to this landmark's x and its surface height.
    /// Negative y builds upwards; positive y digs into the ground.
    pub blocks: Vec<BlockRect>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct WorldRecipe {
    pub version: u32,
    pub name: String,
    pub description: String,
    pub seed: u32,
    pub terrain: Terrain,
    pub surface_y: i32,
    pub relief: u32,
    pub regions: Vec<Region>,
    pub landmarks: Vec<Landmark>,
}

impl WorldRecipe {
    pub fn example() -> Self {
        Self {
            version: RECIPE_VERSION,
            name: "Frostbound ruins".into(),
            description: "Snowy forests surrounding an abandoned stone tower.".into(),
            seed: 42,
            terrain: Terrain::Tundra,
            surface_y: 64,
            relief: 4,
            regions: vec![Region {
                start_x: -160,
                end_x: -32,
                terrain: Terrain::Forest,
            }],
            landmarks: vec![Landmark {
                name: "Abandoned tower".into(),
                x: 24,
                blocks: vec![
                    BlockRect {
                        x: 0,
                        y: -9,
                        width: 10,
                        height: 9,
                        block: BlueprintBlock::StoneBricks,
                    },
                    BlockRect {
                        x: 1,
                        y: -8,
                        width: 8,
                        height: 8,
                        block: BlueprintBlock::Air,
                    },
                    BlockRect {
                        x: 0,
                        y: -3,
                        width: 1,
                        height: 3,
                        block: BlueprintBlock::Air,
                    },
                    BlockRect {
                        x: 2,
                        y: -1,
                        width: 1,
                        height: 1,
                        block: BlueprintBlock::CraftingTable,
                    },
                ],
            }],
        }
    }

    pub fn read(root: &Path) -> Result<Option<Self>, String> {
        let path = root.join(RECIPE_FILE);
        let file = match std::fs::File::open(path) {
            Ok(file) => file,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(format!("cannot read world recipe: {e}")),
        };
        let mut bytes = Vec::new();
        file.take(MAX_RECIPE_BYTES as u64 + 1)
            .read_to_end(&mut bytes)
            .map_err(|e| format!("cannot read world recipe: {e}"))?;
        if bytes.len() > MAX_RECIPE_BYTES {
            return Err("world recipe exceeds 256 KiB".into());
        }
        let recipe: Self = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
        recipe.validate()?;
        Ok(Some(recipe))
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.version != RECIPE_VERSION {
            return Err(format!("unsupported world recipe version {}", self.version));
        }
        if self.name.trim().is_empty()
            || self.name.len() > 80
            || self.name.chars().any(char::is_control)
        {
            return Err("name must contain 1–80 bytes and no control characters".into());
        }
        if self.description.len() > 2000 || self.description.chars().any(char::is_control) {
            return Err(
                "description must contain at most 2000 bytes and no control characters".into(),
            );
        }
        if !(48..=88).contains(&self.surface_y) || self.relief > 12 {
            return Err("surface_y must be 48–88 and relief must be 0–12".into());
        }
        if self.regions.len() > 32 || self.landmarks.len() > 32 {
            return Err("at most 32 regions and 32 landmarks are allowed".into());
        }
        for (i, region) in self.regions.iter().enumerate() {
            if region.start_x < -100_000 || region.end_x > 100_000 || region.start_x >= region.end_x
            {
                return Err(format!(
                    "region {i} needs -100000 <= start_x < end_x <= 100000"
                ));
            }
            if self.regions[..i]
                .iter()
                .any(|other| region.start_x < other.end_x && region.end_x > other.start_x)
            {
                return Err(format!("region {i} overlaps another region"));
            }
        }
        let mut total_area = 0_u64;
        for landmark in &self.landmarks {
            if landmark.name.trim().is_empty()
                || landmark.name.len() > 80
                || landmark.name.chars().any(char::is_control)
            {
                return Err(
                    "landmark names must contain 1–80 bytes and no control characters".into(),
                );
            }
            if !(-100_000..=100_000).contains(&landmark.x) || landmark.blocks.len() > 64 {
                return Err(format!(
                    "landmark {} exceeds coordinate or rectangle limits",
                    landmark.name
                ));
            }
            let surface = self.surface_at(landmark.x);
            for rect in &landmark.blocks {
                if !(-64..=64).contains(&rect.x)
                    || !(-48..=48).contains(&rect.y)
                    || !(1..=64).contains(&rect.width)
                    || !(1..=48).contains(&rect.height)
                {
                    return Err(format!(
                        "landmark {} has an invalid rectangle",
                        landmark.name
                    ));
                }
                let left = landmark.x + rect.x;
                let right = left + rect.width as i32;
                let top = surface + rect.y;
                let bottom = top + rect.height as i32;
                if left < -100_000
                    || right > 100_000
                    || top < 2
                    || bottom >= CHUNK_HEIGHT as i32 - 1
                {
                    return Err(format!(
                        "landmark {} extends outside world limits",
                        landmark.name
                    ));
                }
                // The starting platform must remain clear regardless of model output.
                if left <= 7 && right > -7 {
                    return Err(format!(
                        "landmark {} intersects the reserved spawn area (-7..7)",
                        landmark.name
                    ));
                }
                let tree_surface = self.surface_at(10);
                if left < 13 && right > 8 && top < tree_surface && bottom > tree_surface - 7 {
                    return Err(format!(
                        "landmark {} intersects the starter tree (x=8..12)",
                        landmark.name
                    ));
                }
                total_area += u64::from(rect.width) * u64::from(rect.height);
            }
        }
        if total_area > 100_000 {
            return Err("landmark rectangles exceed the 100000-block budget".into());
        }
        if serde_json::to_vec_pretty(self)
            .map_err(|e| e.to_string())?
            .len()
            > MAX_RECIPE_BYTES
        {
            return Err("encoded world recipe exceeds 256 KiB; use fewer rectangles".into());
        }
        Ok(())
    }

    pub fn terrain_at(&self, x: i32) -> Terrain {
        self.regions
            .iter()
            .find(|r| x >= r.start_x && x < r.end_x)
            .map_or(self.terrain, |r| r.terrain)
    }

    pub fn surface_at(&self, x: i32) -> i32 {
        self.surface_with_noise(x, &Perlin::new(self.seed))
    }

    fn surface_with_noise(&self, x: i32, perlin: &Perlin) -> i32 {
        if (-7..=7).contains(&x) {
            return self.surface_y;
        }
        let noise = perlin.get([x as f64 * 0.035, 0.0]);
        self.surface_y + (noise * self.relief as f64).round() as i32
    }

    pub(crate) fn build_chunk(&self, chunk_x: i32, save_key: &str) -> Chunk {
        let mut chunk = Chunk::new(chunk_x, save_key);
        let cave = Perlin::new(self.seed.wrapping_add(1));
        let height_noise = Perlin::new(self.seed);
        for lx in 0..CHUNK_WIDTH {
            let x = chunk_x * CHUNK_WIDTH as i32 + lx as i32;
            let surface = self.surface_with_noise(x, &height_noise);
            let terrain = self.terrain_at(x);
            let trees: Vec<(i32, i32)> = ((x - 2)..=(x + 2))
                .filter(|tree_x| {
                    *tree_x == 10
                        || (tree_x.abs() > 7
                            && self.terrain_at(*tree_x) == Terrain::Forest
                            && tree_x.rem_euclid(13) == 9)
                })
                .map(|tree_x| (tree_x, self.surface_with_noise(tree_x, &height_noise)))
                .collect();
            for y in 0..CHUNK_HEIGHT {
                let wy = y as i32;
                let mut block = if y == CHUNK_HEIGHT - 1 {
                    BlockType::Bedrock
                } else if wy < surface {
                    BlockType::Air
                } else if wy == surface {
                    match terrain {
                        Terrain::Desert => BlockType::Sand,
                        Terrain::Tundra => BlockType::Snow,
                        Terrain::Mountains => BlockType::Stone,
                        _ => BlockType::Grass,
                    }
                } else if wy < surface + 4 {
                    if terrain == Terrain::Desert {
                        BlockType::Sand
                    } else {
                        BlockType::Dirt
                    }
                } else {
                    let n = cave.get([x as f64 * 0.12, wy as f64 * 0.13]);
                    if x.abs() > 7 && wy > surface + 7 && n > 0.48 {
                        BlockType::Air
                    } else {
                        super::ores::at(self.seed, x, wy, false).unwrap_or(BlockType::Stone)
                    }
                };
                // A deterministic nearby tree makes the survival start viable in every theme.
                for &(tree_x, base) in &trees {
                    if x.abs() <= 7 {
                        continue;
                    }
                    if x == tree_x && (base - 5..base).contains(&wy) {
                        block = BlockType::Wood;
                    } else if (base - 7..=base - 4).contains(&wy) && (x - tree_x).abs() <= 2 {
                        block = BlockType::Leaves;
                    }
                }
                chunk.set_block(lx, y, block);
            }
        }
        let surfaces = std::array::from_fn(|x| {
            self.surface_with_noise(chunk_x * CHUNK_WIDTH as i32 + x as i32, &height_noise)
        });
        super::ores::expose_cave_walls(&mut chunk, self.seed, &surfaces, &[false; CHUNK_WIDTH]);
        let chunk_start = chunk_x * CHUNK_WIDTH as i32;
        for landmark in &self.landmarks {
            let surface = self.surface_at(landmark.x);
            for rect in &landmark.blocks {
                let left = (landmark.x + rect.x).max(chunk_start);
                let right =
                    (landmark.x + rect.x + rect.width as i32).min(chunk_start + CHUNK_WIDTH as i32);
                for x in left..right {
                    for y in (surface + rect.y)..(surface + rect.y + rect.height as i32) {
                        chunk.set_block((x - chunk_start) as usize, y as usize, rect.block.block());
                    }
                }
            }
        }
        chunk
    }

    pub fn schema() -> Value {
        let terrain =
            json!({"type":"string","enum":["plains","forest","desert","tundra","mountains"]});
        let block = json!({"type":"string","enum":["air","stone","dirt","grass","sand","snow","ice","wood","leaves","planks","cobblestone","stone_bricks","glass","ladder","torch","crafting_table","furnace","chest","coal_ore","iron_ore","gold_ore","diamond_ore","glowstone"]});
        json!({
            "type":"object","additionalProperties":false,
            "required":["version","name","description","seed","terrain","surface_y","relief","regions","landmarks"],
            "properties":{
                "version":{"type":"integer","const":1},
                "name":{"type":"string","minLength":1,"maxLength":80},
                "description":{"type":"string","maxLength":2000},
                "seed":{"type":"integer","minimum":0,"maximum":4294967295_u64},
                "terrain":terrain,
                "surface_y":{"type":"integer","minimum":48,"maximum":88},
                "relief":{"type":"integer","minimum":0,"maximum":12},
                "regions":{"type":"array","maxItems":32,"items":{
                    "type":"object","additionalProperties":false,"required":["start_x","end_x","terrain"],
                    "properties":{"start_x":{"type":"integer","minimum":-100000},"end_x":{"type":"integer","maximum":100000},"terrain":terrain}
                }},
                "landmarks":{"type":"array","maxItems":32,"items":{
                    "type":"object","additionalProperties":false,"required":["name","x","blocks"],
                    "properties":{
                        "name":{"type":"string","minLength":1,"maxLength":80},
                        "x":{"type":"integer","minimum":-100000,"maximum":100000},
                        "blocks":{"type":"array","maxItems":64,"items":{
                            "type":"object","additionalProperties":false,"required":["x","y","width","height","block"],
                            "properties":{
                                "x":{"type":"integer","minimum":-64,"maximum":64},"y":{"type":"integer","minimum":-48,"maximum":48},
                                "width":{"type":"integer","minimum":1,"maximum":64},"height":{"type":"integer","minimum":1,"maximum":48},"block":block
                            }
                        }}
                    }
                }}
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generation_is_deterministic_and_landmarks_cross_chunk_boundaries() {
        let recipe = WorldRecipe::example();
        recipe.validate().unwrap();
        let a = recipe.build_chunk(0, "test");
        let b = recipe.build_chunk(0, "test");
        assert_eq!(a.blocks, b.blocks);
        let surface = recipe.surface_at(24);
        assert_eq!(
            a.get_block(24, (surface - 9) as usize),
            BlockType::StoneBricks
        );
        let next = recipe.build_chunk(1, "test");
        assert_eq!(
            next.get_block(1, (surface - 9) as usize),
            BlockType::StoneBricks
        );
        assert_eq!(a.get_block(0, recipe.surface_y as usize), BlockType::Snow);
        assert_eq!(
            a.get_block(0, recipe.surface_y as usize - 1),
            BlockType::Air
        );
        assert_eq!(
            a.get_block(10, (recipe.surface_at(10) - 1) as usize),
            BlockType::Wood
        );
    }

    #[test]
    fn rejects_spawn_edits_overlaps_and_out_of_bounds_geometry() {
        let mut recipe = WorldRecipe::example();
        recipe.landmarks[0].x = 0;
        assert!(recipe.validate().unwrap_err().contains("spawn"));
        recipe.landmarks[0].x = 24;
        recipe.landmarks[0].blocks[0].y = -48;
        recipe.surface_y = 48;
        assert!(recipe.validate().is_err());
        recipe = WorldRecipe::example();
        recipe.regions.push(recipe.regions[0].clone());
        assert!(recipe.validate().unwrap_err().contains("overlaps"));
    }
}
