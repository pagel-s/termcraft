//! Reproducible ore census over fresh, disposable worlds (including chunk seams).
use serde_json::json;
use std::{collections::VecDeque, fs};
use termcraft::world::{Dimension, World, block::BlockType};

fn ore(b: BlockType) -> Option<usize> {
    use BlockType::*;
    [CoalOre, IronOre, RedstoneOre, GoldOre, DiamondOre]
        .iter()
        .position(|&v| v == b)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut reports = Vec::new();
    for seeds in [
        [1337u32, 7331, 9917, 4444],
        [42, 91, 27, 18],
        [9182, 461, 7803, 59],
    ] {
        let root = std::env::temp_dir().join(format!(
            "termcraft-ore-audit-{}-{}",
            std::process::id(),
            seeds[0]
        ));
        fs::create_dir_all(&root)?;
        fs::write(
            root.join("overworld_world_seed.bin"),
            bincode::serialize(&seeds)?,
        )?;
        let mut world = World::open_in_directory(Dimension::Overworld, &root)?;
        world.load_chunks_for_spawn_search(0, 2048);
        let mut counts = [[0usize; 5]; 4];
        let mut rock = [0usize; 4];
        let mut exposed = [0usize; 5];
        let mut min_y = [128; 5];
        let mut grid = vec![None; 4096 * 128];
        for x in -2048..2048 {
            for y in 0..128 {
                let b = world.get_block(x, y);
                let band = (y / 32) as usize;
                if b == BlockType::Stone || ore(b).is_some() {
                    rock[band] += 1;
                }
                if let Some(o) = ore(b) {
                    counts[band][o] += 1;
                    min_y[o] = min_y[o].min(y);
                    grid[(x + 2048) as usize * 128 + y as usize] = Some(o);
                    if [(-1, 0), (1, 0), (0, -1), (0, 1)]
                        .iter()
                        .any(|&(dx, dy)| world.get_block(x + dx, y + dy) == BlockType::Air)
                    {
                        exposed[o] += 1;
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
        reports.push(json!({"seeds":seeds,"ore_order":["coal","iron","redstone","gold","diamond"],"bands":"world Y 0–31 / 32–63 / 64–95 / 96–127 (downward)","host_rock":rock,"counts":counts,"cave_exposed":exposed,"minimum_y":min_y,"connected_veins":vein_stats}));
        drop(world);
        fs::remove_dir_all(root)?;
    }
    println!("{}", serde_json::to_string_pretty(&reports)?);
    Ok(())
}
