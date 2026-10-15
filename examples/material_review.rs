//! Full block/state and item catalog through the implemented renderer.
use std::{fs, path::PathBuf};
use termcraft::{
    engine::state::GameState,
    entities::item_entity::ItemEntity,
    renderer::Renderer,
    world::{
        block::BlockType,
        item::{ItemStack, ItemType},
    },
};
fn record_bounds(
    records: &mut [serde_json::Value],
    renderer: &Renderer,
    state: &GameState,
    page: usize,
    width: u16,
    height: u16,
    file: &str,
) {
    for record in records.iter_mut().filter(|r| r["page"] == page) {
        let bx = record["x"].as_i64().unwrap() as i32;
        let by = record["y"].as_i64().unwrap() as i32;
        let xs: Vec<_> = (0..width)
            .filter(|&x| renderer.screen_to_world(state, x, 0).0 == bx)
            .collect();
        let ys: Vec<_> = (1..height - 4)
            .filter(|&y| renderer.screen_to_world(state, 0, y).1 == by)
            .collect();
        assert!(
            !xs.is_empty() && !ys.is_empty(),
            "catalog block outside view"
        );
        if record.get("views").is_none() {
            record["views"] = serde_json::json!({});
        }
        record["views"][file] =
            serde_json::json!([xs[0], ys[0], xs[xs.len() - 1] + 1, ys[ys.len() - 1] + 1]);
    }
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    use BlockType::*;
    let output = PathBuf::from(std::env::args().nth(1).ok_or("output directory required")?);
    fs::create_dir_all(&output)?;
    let root = std::env::temp_dir().join(format!(
        "termcraft-materials-{}-{}",
        std::process::id(),
        rand::random::<u64>()
    ));
    fs::create_dir_all(&root)?;
    let mut state = GameState::open_in_directory(&root)?;
    state.dismiss_startup_splash();
    state.time_of_day = 12000.0;
    state.world.load_chunks_for_spawn_search(0, 32);
    state.player.x = 0.5;
    state.player.y = 34.0;
    state.player.grounded = true;
    let blocks = [
        Dirt,
        Grass,
        Stone,
        Wood,
        Leaves,
        IronOre,
        GoldOre,
        DiamondOre,
        CoalOre,
        Sand,
        Gravel,
        Bedrock,
        Planks,
        CraftingTable,
        Torch,
        Lever(false),
        Lever(true),
        StoneButton(0),
        StoneButton(4),
        RedstoneTorch(false),
        RedstoneTorch(true),
        Furnace,
        Snow,
        Ice,
        Cactus,
        DeadBush,
        BirchWood,
        BirchLeaves,
        RedFlower,
        YellowFlower,
        TallGrass,
        Water(1),
        Water(4),
        Water(8),
        Lava(1),
        Lava(4),
        Lava(8),
        Cobblestone,
        Obsidian,
        Farmland(0),
        Farmland(7),
        Crops(0),
        Crops(3),
        Crops(7),
        RedstoneOre,
        RedstoneDust(0),
        RedstoneDust(7),
        RedstoneDust(15),
        Netherrack,
        SoulSand,
        Glowstone,
        NetherPortal,
        Tnt,
        PrimedTnt(1),
        PrimedTnt(4),
        Piston {
            extended: false,
            facing_right: false,
        },
        Piston {
            extended: false,
            facing_right: true,
        },
        Piston {
            extended: true,
            facing_right: false,
        },
        Piston {
            extended: true,
            facing_right: true,
        },
        StickyPiston {
            extended: false,
            facing_right: false,
        },
        StickyPiston {
            extended: false,
            facing_right: true,
        },
        StickyPiston {
            extended: true,
            facing_right: false,
        },
        StickyPiston {
            extended: true,
            facing_right: true,
        },
        StoneBricks,
        EndPortalFrame { filled: false },
        EndPortalFrame { filled: true },
        EndPortal,
        EndStone,
        IronDoor(false),
        IronDoor(true),
        SilverfishSpawner,
        Bed,
        Chest,
        Glass,
        WoodDoor(false),
        WoodDoor(true),
        Ladder,
        StoneSlab,
        StoneStairs,
        EnchantingTable,
        Anvil,
        BrewingStand,
        BlazeSpawner,
        RedstoneRepeater {
            powered: false,
            delay: 2,
            facing_right: false,
            timer: 0,
            target_powered: false,
        },
        RedstoneRepeater {
            powered: false,
            delay: 2,
            facing_right: true,
            timer: 0,
            target_powered: false,
        },
        RedstoneRepeater {
            powered: true,
            delay: 2,
            facing_right: false,
            timer: 0,
            target_powered: true,
        },
        RedstoneRepeater {
            powered: true,
            delay: 2,
            facing_right: true,
            timer: 0,
            target_powered: true,
        },
        Wool,
        Sapling,
        SugarCane,
        Bookshelf,
        NetherWart(0),
        NetherWart(1),
        NetherWart(3),
        BirchSapling,
        ZombieSpawner,
        SkeletonSpawner,
    ];
    let mut records = Vec::new();
    for (page, batch) in blocks.chunks(18).enumerate() {
        for x in -24..=24 {
            for y in 0..42 {
                state
                    .world
                    .set_block(x, y, if y >= 37 { Stone } else { Air });
            }
        }
        for (i, &block) in batch.iter().enumerate() {
            let x = [-9, -6, -3, 0, 3, 6][i % 6];
            let y = 28 + (i / 6) as i32 * 3;
            let support = match block {
                Torch => Some(Stone),
                Cactus | DeadBush | SugarCane => Some(Sand),
                Crops(_) => Some(Farmland(7)),
                NetherWart(_) => Some(SoulSand),
                RedFlower | YellowFlower | TallGrass | Sapling | BirchSapling => Some(Grass),
                _ => None,
            };
            if let Some(support) = support {
                state.world.set_block(x, y + 1, support);
                if block == SugarCane {
                    state.world.set_block(x + 1, y + 1, Water(0));
                }
            }
            state.world.set_block(x, y, block);
            assert_eq!(state.world.get_block(x, y), block, "missing catalog sample");
            records.push(
                serde_json::json!({"page":page,"index":i,"name":format!("{block:?}"),"x":x,"y":y}),
            );
        }
        for (w, h) in [(120, 40), (160, 48)] {
            let mut renderer = Renderer::with_size(w, h);
            let mut ansi = Vec::new();
            renderer.render_to(&state, 0.0, &mut ansi)?;
            let file = format!("blocks-{page:02}-{w}x{h}.ansi");
            fs::write(output.join(&file), ansi)?;
            record_bounds(&mut records, &renderer, &state, page, w, h, &file);
        }
        let mut renderer = Renderer::with_size(200, 72);
        renderer.toggle_zoom();
        renderer.toggle_zoom();
        let mut ansi = Vec::new();
        renderer.render_to(&state, 0.0, &mut ansi)?;
        let file = format!("blocks-{page:02}-detail-200x72.ansi");
        fs::write(output.join(&file), ansi)?;
        record_bounds(&mut records, &renderer, &state, page, 200, 72, &file);
    }
    for x in -24..=24 {
        for y in 0..42 {
            state
                .world
                .set_block(x, y, if y >= 37 { Stone } else { Air });
        }
    }
    let mut items = Vec::new();
    for (page, batch) in ItemType::ALL.chunks(27).enumerate() {
        state.inventory.slots.fill(None);
        for (i, &item) in batch.iter().enumerate() {
            state.inventory.slots[i] = Some(ItemStack {
                item_type: item,
                count: 1,
                durability: None,
            });
            items.push(serde_json::json!({"page":page,"index":i,"name":item.name()}));
        }
        state.inventory_open = true;
        for (w, h) in [(120, 40), (160, 48)] {
            let mut renderer = Renderer::with_size(w, h);
            let mut ansi = Vec::new();
            renderer.render_to(&state, 0.0, &mut ansi)?;
            fs::write(
                output.join(format!("inventory-{page:02}-{w}x{h}.ansi")),
                ansi,
            )?;
        }
    }
    state.inventory_open = false;
    for x in -24..=24 {
        for y in 0..42 {
            state
                .world
                .set_block(x, y, if y >= 37 { Stone } else { Air });
        }
    }
    state.player.y = 37.0;
    for (page, batch) in ItemType::ALL.chunks(9).enumerate() {
        state.item_entities = batch
            .iter()
            .enumerate()
            .map(|(i, &item)| ItemEntity::new(i as f64 * 2.0 - 7.5, 33.0, item))
            .collect();
        for (w, h) in [(120, 40), (160, 48)] {
            let mut renderer = Renderer::with_size(w, h);
            let mut ansi = Vec::new();
            renderer.render_to(&state, 0.0, &mut ansi)?;
            fs::write(output.join(format!("drops-{page:02}-{w}x{h}.ansi")), ansi)?;
        }
    }
    fs::write(
        output.join("catalog.json"),
        serde_json::to_vec_pretty(&serde_json::json!({"blocks":records,"items":items}))?,
    )?;
    drop(state);
    fs::remove_dir_all(root)?;
    Ok(())
}
