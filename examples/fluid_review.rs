//! Actual bucket commands and world ticks in isolated native saves, captured at all
//! three pixel zooms. Geometry/player poses are staged, never a survival claim.
use serde_json::json;
use std::{fs, path::Path, time::Instant};
use termcraft::{
    engine::{command::ClientCommand, state::GameState},
    renderer::Renderer,
    world::{
        Dimension, World,
        block::BlockType,
        chunk::{CHUNK_HEIGHT, CHUNK_WIDTH},
        item::{ItemStack, ItemType},
    },
};

fn room(dimension: Dimension, root: &Path) -> Result<GameState, Box<dyn std::error::Error>> {
    fs::create_dir_all(root)?;
    for name in ["overworld", "nether", "end"] {
        fs::write(
            root.join(format!("{name}_world_seed.bin")),
            bincode::serialize(&[1337u32, 7331, 9917, 4444])?,
        )?;
    }
    let mut s = GameState::open_in_directory(root)?;
    s.current_dimension = dimension;
    s.world = World::open_in_directory(dimension, root)?;
    for x in -2..=2 {
        s.world
            .apply_chunk_column_snapshot(x, &vec![BlockType::Air; CHUNK_WIDTH * CHUNK_HEIGHT]);
    }
    for x in -50..=50 {
        for y in 30..128 {
            s.world.set_block(
                x,
                y,
                if dimension == Dimension::Nether {
                    BlockType::Netherrack
                } else {
                    BlockType::Stone
                },
            );
        }
    }
    for command in [
        ClientCommand::ToggleRuleMobSpawning,
        ClientCommand::ToggleRuleDaylightCycle,
        ClientCommand::ToggleRuleWeatherCycle,
    ] {
        s.apply_client_command(command);
    }
    s.time_of_day = 6000.0;
    s.dismiss_startup_splash();
    s.inventory.slots.fill(None);
    Ok(s)
}

fn use_bucket(s: &mut GameState, item: ItemType, x: i32, y: i32) {
    if matches!(item, ItemType::WaterBucket | ItemType::LavaBucket)
        && ![(x - 1, y), (x + 1, y), (x, y - 1), (x, y + 1)]
            .into_iter()
            .any(|(bx, by)| s.world.get_block(bx, by).is_solid())
    {
        // Ordinary bucket placement needs an adjacent anchor. Connect its
        // ceiling beam to the floor rather than staging a floating support.
        for bx in x..=x + 2 {
            s.world.set_block(bx, y - 1, BlockType::StoneBricks);
        }
        for by in y - 1..30 {
            s.world.set_block(x + 2, by, BlockType::StoneBricks);
        }
    }
    s.inventory.slots[0] = Some(ItemStack {
        item_type: item,
        count: 1,
        durability: None,
    });
    s.hotbar_index = 0;
    s.player.x = if s.world.get_block(x - 1, y).is_solid() {
        x as f64 + 2.5
    } else {
        x as f64 - 1.5
    };
    s.player.y = y as f64 + 1.0;
    s.apply_client_command(ClientCommand::UseAt(x, y));
}

fn advance(s: &mut GameState, ticks: usize) {
    for _ in 0..ticks {
        s.world.update(0);
        s.player.age += 1;
    }
}

fn capture(s: &GameState, out: &Path, id: &str) -> Result<(), Box<dyn std::error::Error>> {
    for zoom in 0..3 {
        let mut r = Renderer::with_size(160, 48);
        for _ in 0..zoom {
            r.toggle_zoom();
        }
        let mut bytes = Vec::new();
        r.render_to(s, 0.0, &mut bytes)?;
        fs::write(out.join(format!("{id}-{zoom}.ansi")), bytes)?;
    }
    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let out = std::path::PathBuf::from(std::env::args().nth(1).ok_or("output directory required")?);
    fs::create_dir_all(&out)?;
    let root = std::env::temp_dir().join(format!(
        "termcraft-fluid-review-{}-{}",
        std::process::id(),
        rand::random::<u64>()
    ));
    let mut water = room(Dimension::Overworld, &root.join("waterfall"))?;
    use_bucket(&mut water, ItemType::WaterBucket, 0, 22);
    assert_eq!(water.world.get_block(0, 22), BlockType::Water(8));
    assert_eq!(
        water.inventory.slots[0].as_ref().unwrap().item_type,
        ItemType::Bucket
    );
    advance(&mut water, 180);
    for x in -8..=-4 {
        water.world.set_block(x, 27, BlockType::Planks);
    }
    for y in 28..30 {
        water.world.set_block(-8, y, BlockType::StoneBricks);
    }
    water.player.x = -5.5;
    water.player.y = 27.0;
    water.player.grounded = true;
    capture(&water, &out, "waterfall")?;
    for y in 23..29 {
        assert_eq!(water.world.get_block(1, y), BlockType::Air);
    }
    use_bucket(&mut water, ItemType::Bucket, 0, 22);
    assert_eq!(water.world.get_block(0, 22), BlockType::Air);
    assert_eq!(
        water.inventory.slots[0].as_ref().unwrap().item_type,
        ItemType::WaterBucket
    );
    advance(&mut water, 300);
    for x in -12..=12 {
        for y in 21..30 {
            assert!(!water.world.get_block(x, y).is_fluid());
        }
    }
    water.player.x = -5.5;
    water.player.y = 27.0;
    capture(&water, &out, "drained")?;

    let mut shallow = room(Dimension::Overworld, &root.join("shallow"))?;
    use_bucket(&mut shallow, ItemType::WaterBucket, 0, 29);
    advance(&mut shallow, 100);
    shallow.player.x = -6.5;
    shallow.player.y = 30.0;
    shallow.player.grounded = true;
    capture(&shallow, &out, "shallow-flow")?;

    let mut routed = room(Dimension::Overworld, &root.join("routed"))?;
    for y in 30..128 {
        routed.world.set_block(3, y, BlockType::Air);
    }
    use_bucket(&mut routed, ItemType::WaterBucket, 0, 29);
    advance(&mut routed, 140);
    assert_eq!(routed.world.get_block(-1, 29), BlockType::Air);
    assert!(routed.world.get_block(3, 33).is_fluid());
    routed.player.x = -3.5;
    routed.player.y = 30.0;
    routed.player.grounded = true;
    capture(&routed, &out, "nearest-drop")?;

    let mut lava = room(Dimension::Nether, &root.join("lava"))?;
    use_bucket(&mut lava, ItemType::LavaBucket, 0, 22);
    assert_eq!(lava.world.get_block(0, 22), BlockType::Lava(8));
    advance(&mut lava, 180);
    for x in -8..=-4 {
        lava.world.set_block(x, 27, BlockType::StoneBricks);
    }
    for y in 28..30 {
        lava.world.set_block(-8, y, BlockType::StoneBricks);
    }
    lava.player.x = -5.5;
    lava.player.y = 27.0;
    lava.player.grounded = true;
    capture(&lava, &out, "lavafall")?;

    let mut mixing = room(Dimension::Overworld, &root.join("mixing"))?;
    use_bucket(&mut mixing, ItemType::LavaBucket, 1, 26);
    use_bucket(&mut mixing, ItemType::WaterBucket, 0, 26);
    advance(&mut mixing, 5);
    assert_eq!(mixing.world.get_block(1, 26), BlockType::Obsidian);
    mixing.player.x = -3.5;
    mixing.player.y = 30.0;
    mixing.player.grounded = true;
    capture(&mixing, &out, "mixing")?;

    let mut basin = room(Dimension::Overworld, &root.join("basin"))?;
    for y in 18..30 {
        for x in [-13, 13] {
            basin.world.set_block(x, y, BlockType::StoneBricks);
        }
    }
    for y in 18..28 {
        basin.world.set_block(0, y, BlockType::StoneBricks);
    }
    // The divider hangs from a roof beam; its low opening connects the vessels.
    for x in -13..=13 {
        basin.world.set_block(x, 18, BlockType::StoneBricks);
    }
    use_bucket(&mut basin, ItemType::WaterBucket, -12, 22);
    assert_eq!(
        basin.world.get_block(-12, 22),
        BlockType::Water(8),
        "basin bucket placement"
    );
    advance(&mut basin, 50); // Save a partially filled vessel, then finish after reopen.
    basin.world.try_save_all()?;
    basin.try_save_progression()?;
    drop(basin);
    let mut basin = GameState::open_in_directory(&root.join("basin"))?;
    basin.dismiss_startup_splash();
    let started = Instant::now();
    advance(&mut basin, 650);
    let filling_ms = started.elapsed().as_secs_f64() * 1000.0;
    for x in -12..13 {
        if x != 0 {
            assert_eq!(basin.world.fluid_surface_y(x, 22), Some(22.0));
            assert_eq!(basin.world.get_block(x, 21), BlockType::Air);
        }
    }
    assert_eq!(basin.world.get_block(12, 22), BlockType::Water(9));
    basin.player.x = 3.5;
    basin.player.y = 27.0;
    basin.player.grounded = false;
    capture(&basin, &out, "connected-vessels")?;
    basin.world.try_save_all()?;
    basin.try_save_progression()?;
    drop(basin);
    let mut basin = GameState::open_in_directory(&root.join("basin"))?;
    basin.dismiss_startup_splash();
    assert_eq!(basin.world.get_block(12, 22), BlockType::Water(9));
    basin.world.set_block(13, 27, BlockType::Air);
    advance(&mut basin, 650);
    assert_eq!(basin.world.fluid_surface_y(10, 28), Some(28.0));
    assert_eq!(basin.world.get_block(10, 22), BlockType::Air);
    basin.player.y = 30.0;
    basin.player.grounded = true;
    capture(&basin, &out, "basin-outlet")?;
    basin.world.set_block(13, 27, BlockType::StoneBricks);
    advance(&mut basin, 650);
    assert_eq!(basin.world.fluid_surface_y(12, 22), Some(22.0));
    basin.player.y = 27.0;
    basin.player.grounded = false;
    capture(&basin, &out, "basin-refilled")?;
    use_bucket(&mut basin, ItemType::Bucket, -12, 22);
    assert_eq!(basin.world.get_block(-12, 22), BlockType::Air);
    advance(&mut basin, 900);
    for x in -12..13 {
        for y in 22..30 {
            assert!(!basin.world.get_block(x, y).is_fluid());
        }
    }
    basin.player.x = 3.5;
    basin.player.y = 30.0;
    basin.player.grounded = true;
    capture(&basin, &out, "vessels-drained")?;

    let started = Instant::now();
    advance(&mut shallow, 1000);
    let idle_ms = started.elapsed().as_secs_f64() * 1000.0;
    let saved_flows: Vec<_> = (-7..=7).map(|x| shallow.world.get_block(x, 29)).collect();
    shallow.world.try_save_all()?;
    shallow.try_save_progression()?;
    drop(shallow);
    let shallow = GameState::open_in_directory(&root.join("shallow"))?;
    for (x, expected) in (-7..=7).zip(saved_flows) {
        assert_eq!(shallow.world.get_block(x, 29), expected);
    }
    let report = json!({"status":"passed", "actual_bucket_place_and_pickup":true,
        "vertical_waterfall":true,"complete_drain_after_pickup":true,"nearest_drop":true,
        "adjacent_lava_source_cooled":true,"save_reopen_preserves_flow_levels":true,"idle_world_1000_ticks_ms":idle_ms,
        "source_height_in_connected_vessels":true,"no_rise_above_source":true,
        "opening_outlet_lowers_pool_closing_it_refills":true,
        "partial_fill_continues_after_reopen":true,"full_supplied_water_survives_reopen":true,
        "supplied_pool_drains_after_actual_bucket_pickup":true,"filling_650_ticks_cpu_ms":filling_ms,
        "views_per_scene":3,"scenes":10,
        "limits":"Staged native geometry/player poses, actual bucket commands and world ticks. CPU work excludes terminal presentation, disk and WAN."});
    fs::write(out.join("checks.json"), serde_json::to_vec_pretty(&report)?)?;
    println!("{report}");
    drop((water, shallow, routed, lava, mixing, basin));
    fs::remove_dir_all(root)?;
    Ok(())
}
