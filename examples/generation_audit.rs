//! Reproducible ore census over fresh, disposable worlds (including chunk seams).
use serde_json::json;
use std::{
    collections::{BTreeMap, VecDeque},
    fs,
};
use termcraft::world::{
    Dimension, World,
    block::BlockType,
    recipe::{Terrain, WorldRecipe},
};

fn ore(b: BlockType) -> Option<usize> {
    use BlockType::*;
    [CoalOre, IronOre, RedstoneOre, GoldOre, DiamondOre]
        .iter()
        .position(|&v| v == b)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut reports = Vec::new();
    for (case, seeds) in [
        [1337u32, 7331, 9917, 4444],
        [42, 91, 27, 18],
        [9182, 461, 7803, 59],
        [7, 11, 13, 17],
        [2026, 133, 777, 909],
        [65535, 65536, 8191, 97],
    ]
    .into_iter()
    .enumerate()
    {
        for recipe_mode in [false, true] {
            let root = std::env::temp_dir().join(format!(
                "termcraft-ore-audit-{}-{}-{}",
                std::process::id(),
                seeds[0],
                recipe_mode
            ));
            fs::create_dir_all(&root)?;
            fs::write(
                root.join("overworld_world_seed.bin"),
                bincode::serialize(&seeds)?,
            )?;
            if recipe_mode {
                let mut recipe = WorldRecipe::example();
                recipe.seed = seeds[0];
                recipe.terrain = [
                    Terrain::Plains,
                    Terrain::Forest,
                    Terrain::Desert,
                    Terrain::Tundra,
                    Terrain::Mountains,
                    Terrain::Plains,
                ][case];
                recipe.regions.clear();
                recipe.landmarks.clear();
                fs::write(root.join("world.json"), serde_json::to_vec(&recipe)?)?;
            }
            let mut world = World::open_in_directory(Dimension::Overworld, &root)?;
            world.load_chunks_for_spawn_search(0, 2048);
            let mut counts = [[0usize; 5]; 4];
            let mut rock = [0usize; 4];
            let mut exposed = [0usize; 5];
            let mut min_y = [128; 5];
            let mut bands8_counts = [[0usize; 5]; 16];
            let mut bands8_rock = [0usize; 16];
            let mut bands8_exposed = [[0usize; 5]; 16];
            let mut sectors = [[[false; 5]; 16]; 64];
            let mut bottom_windows = [[0usize; 5]; 128];
            let mut bottom_rock = [0usize; 128];
            let mut biomes: BTreeMap<String, (usize, [usize; 5])> = BTreeMap::new();
            let mut grid = vec![None; 4096 * 128];
            for x in -2048..2048 {
                let biome = biomes
                    .entry(format!("{:?}", world.get_biome(x)))
                    .or_default();
                for y in 0..128 {
                    let b = world.get_block(x, y);
                    let band = (y / 32) as usize;
                    if b == BlockType::Stone || ore(b).is_some() {
                        rock[band] += 1;
                        bands8_rock[y as usize / 8] += 1;
                        biome.0 += 1;
                        if y >= 112 {
                            bottom_rock[(x + 2048) as usize / 32] += 1;
                        }
                    }
                    if let Some(o) = ore(b) {
                        counts[band][o] += 1;
                        bands8_counts[y as usize / 8][o] += 1;
                        sectors[(x + 2048) as usize / 64][y as usize / 8][o] = true;
                        biome.1[o] += 1;
                        if y >= 112 {
                            bottom_windows[(x + 2048) as usize / 32][o] += 1;
                        }
                        min_y[o] = min_y[o].min(y);
                        grid[(x + 2048) as usize * 128 + y as usize] = Some(o);
                        if [(-1, 0), (1, 0), (0, -1), (0, 1)]
                            .iter()
                            .any(|&(dx, dy)| world.get_block(x + dx, y + dy) == BlockType::Air)
                        {
                            exposed[o] += 1;
                            bands8_exposed[y as usize / 8][o] += 1;
                        }
                    }
                }
            }
            let mut veins: [Vec<usize>; 5] = std::array::from_fn(|_| Vec::new());
            for i in 0..grid.len() {
                let Some(o) = grid[i].take() else {
                    continue;
                };
                let mut queue = VecDeque::from([i]);
                let mut size = 0;
                while let Some(p) = queue.pop_front() {
                    size += 1;
                    let (x, y) = (p / 128, p % 128);
                    for (nx, ny) in [
                        (x as i32 - 1, y as i32),
                        (x as i32 + 1, y as i32),
                        (x as i32, y as i32 - 1),
                        (x as i32, y as i32 + 1),
                    ] {
                        if !(0..4096).contains(&nx) || !(0..128).contains(&ny) {
                            continue;
                        }
                        let n = nx as usize * 128 + ny as usize;
                        if grid[n] == Some(o) {
                            grid[n] = None;
                            queue.push_back(n);
                        }
                    }
                }
                veins[o].push(size);
            }
            let vein_stats: Vec<_> = veins.iter_mut().map(|v| {
            v.sort_unstable();
            json!({"count":v.len(),"median":v.get(v.len()/2),"p95":v.get(v.len()*95/100),"max":v.last()})
        }).collect();
            let fine_bands: Vec<_> = (0..16).map(|b| json!({
            "y":[b*8,b*8+7], "host_rock":bands8_rock[b], "counts":bands8_counts[b],
            "cave_exposed":bands8_exposed[b],
            "occupied_64_column_regions":(0..5).map(|o| sectors.iter().filter(|s|s[b][o]).count()).collect::<Vec<_>>()
        })).collect();
            let bottom_stats: Vec<_> = (0..5).map(|o| {
            let mut v: Vec<_> = bottom_windows.iter().map(|w|w[o]).collect();
            v.sort_unstable();
            json!({"empty_regions":v.iter().filter(|&&n|n==0).count(),"median":v[64],"p95":v[121],"max":v[127]})
        }).collect();
            let bottom_peak_fraction = bottom_windows
                .iter()
                .zip(bottom_rock)
                .filter(|(_, r)| *r > 0)
                .map(|(w, r)| w.iter().sum::<usize>() as f64 / r as f64)
                .fold(0.0, f64::max);
            reports.push(json!({"generator":if recipe_mode {"recipe"} else {"classic"},"seeds":seeds,"ore_order":["coal","iron","redstone","gold","diamond"],"bands":"world Y 0–31 / 32–63 / 64–95 / 96–127 (downward)","host_rock":rock,"counts":counts,"cave_exposed":exposed,"minimum_y":min_y,"connected_veins":vein_stats,
          "bands8":fine_bands,"biomes":biomes,
          "bottom_32_column_regions":{"y":[112,126],"regions":128,"ore_stats":bottom_stats,"maximum_total_ore_fraction_of_host_rock":bottom_peak_fraction}}));
            drop(world);
            fs::remove_dir_all(root)?;
        }
    }
    println!("{}", serde_json::to_string_pretty(&reports)?);
    Ok(())
}
