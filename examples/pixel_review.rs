//! Real renderer captures: generated village, all dimensions and a physics course.
//! Uses disposable saves. No user worlds, model, screenshots or mock art as input.
use std::{fs, path::Path};
use termcraft::{
    engine::{command::ClientCommand, state::GameState},
    entities::{
        blaze::Blaze, end_crystal::EndCrystal, ender_dragon::EnderDragon, enderman::Enderman,
        ghast::Ghast, villager::Villager, zombie_pigman::ZombiePigman,
    },
    renderer::Renderer,
    world::{Dimension, World, block::BlockType},
};

fn capture(state: &GameState, output: &Path, name: &str) -> std::io::Result<()> {
    let mut renderer = Renderer::with_size(160, 48);
    let mut ansi = Vec::new();
    renderer.render_to(state, 1.0, &mut ansi)?;
    fs::write(output.join(format!("{name}.ansi")), ansi)
}

fn stage(state: &mut GameState, dimension: Dimension, floor: BlockType) -> std::io::Result<()> {
    state.current_dimension = dimension;
    state.world = World::open_in_directory(dimension, state.world.save_root())?;
    state.world.load_chunks_for_spawn_search(0, 40);
    for x in -40..=40 {
        for y in 0..50 {
            state
                .world
                .set_block(x, y, if y >= 34 { floor } else { BlockType::Air });
        }
    }
    state.player.x = 0.5;
    state.player.y = 34.0;
    state.player.vx = 0.0;
    state.player.vy = 0.0;
    state.player.grounded = true;
    state.villagers.clear();
    state.pigmen.clear();
    state.blazes.clear();
    state.ghasts.clear();
    state.endermen.clear();
    state.end_crystals.clear();
    state.ender_dragon = None;
    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output =
        std::path::PathBuf::from(std::env::args().nth(1).ok_or("output directory required")?);
    fs::create_dir_all(&output)?;
    let root = std::env::temp_dir().join(format!(
        "termcraft-pixel-review-{}-{}",
        std::process::id(),
        rand::random::<u64>()
    ));
    fs::create_dir_all(&root)?;
    let seeds = bincode::serialize(&[1337u32, 7331, 9917, 4444])?;
    for name in ["overworld", "nether", "end"] {
        fs::write(root.join(format!("{name}_world_seed.bin")), &seeds)?;
    }
    let mut state = GameState::open_in_directory(&root)?;
    state.dismiss_startup_splash();
    state.time_of_day = 8500.0;
    let site = state
        .world
        .village_sites_near(0, 10000)
        .into_iter()
        .min_by_key(|site| site.center_x.abs())
        .ok_or("seed fixture produced no village")?;
    state.world.load_chunks_for_spawn_search(site.center_x, 100);
    let (left, _, _) = site.buildings()[0];
    state.player.x = (left + 5) as f64 + 0.5;
    state.player.y = site.ground_y as f64;
    state.player.grounded = true;
    state.villagers = vec![Villager::new(
        (left + 8) as f64 + 0.5,
        site.ground_y as f64,
        left,
        site.ground_y - 1,
    )];
    capture(&state, &output, "village")?;

    stage(&mut state, Dimension::Nether, BlockType::Netherrack)?;
    for x in -8..=-4 {
        for y in 34..38 {
            state.world.set_block(x, y, BlockType::Lava(0));
        }
    }
    for x in [-10, 10] {
        for y in 22..34 {
            state.world.set_block(x, y, BlockType::StoneBricks);
        }
    }
    for x in -10..=10 {
        state.world.set_block(x, 22, BlockType::StoneBricks);
    }
    for x in [-7, 7] {
        state.world.set_block(x, 23, BlockType::Glowstone);
    }
    state.pigmen = vec![ZombiePigman::new(-2.5, 34.0)];
    state.blazes = vec![Blaze::new(3.5, 32.0)];
    state.ghasts = vec![Ghast::new(7.0, 30.0)];
    capture(&state, &output, "nether")?;

    stage(&mut state, Dimension::End, BlockType::EndStone)?;
    for y in 30..34 {
        state.world.set_block(-8, y, BlockType::Obsidian);
    }
    state.end_crystals = vec![EndCrystal::new(-7.5, 30.0)];
    state.endermen = vec![Enderman::new(-3.0, 34.0)];
    state.ender_dragon = Some(EnderDragon::new(6.0, 31.0));
    capture(&state, &output, "end")?;

    stage(&mut state, Dimension::Overworld, BlockType::Stone)?;
    for x in -40..=40 {
        for y in 0..24 {
            state.world.set_block(x, y, BlockType::Stone);
        }
    }
    for (x, ore) in [
        (-8, BlockType::IronOre),
        (-5, BlockType::CoalOre),
        (4, BlockType::DiamondOre),
        (7, BlockType::GoldOre),
    ] {
        // Keep deposits inside the actual close-view camera, rather than above
        // the captured ceiling where the atlas could not be reviewed.
        state.world.set_block(x, 35, ore);
        state.world.set_block(x + 1, 36, ore);
    }
    for y in 31..34 {
        state.world.set_block(-9, y, BlockType::Stone);
        state.world.set_block(9, y, BlockType::Stone);
    }
    for x in [-7, 6] {
        state.world.set_block(x, 33, BlockType::Torch);
    }
    for x in 2..=5 {
        state.world.set_block(x, 34, BlockType::Water(0));
    }
    capture(&state, &output, "cave")?;

    stage(&mut state, Dimension::Overworld, BlockType::Grass)?;
    state.apply_client_command(ClientCommand::ToggleRuleMobSpawning);
    state.player.x = -4.5;
    state.world.set_block(4, 33, BlockType::Stone);
    for y in 31..34 {
        state.world.set_block(9, y, BlockType::Stone);
    }
    let mut renderer = Renderer::with_size(160, 48);
    state.apply_client_command(ClientCommand::SetMoveRight(true));
    let mut trace = Vec::new();
    for tick in 0..48 {
        if tick == 21 {
            state.apply_client_command(ClientCommand::QueueJump);
            state.apply_client_command(ClientCommand::SetJumpHeld(true));
        }
        if tick == 26 {
            state.apply_client_command(ClientCommand::SetJumpHeld(false));
        }
        if tick == 35 {
            state.apply_client_command(ClientCommand::SetMoveLeft(true));
        }
        if tick == 43 {
            state.apply_client_command(ClientCommand::ClearDirectionalInput);
        }
        renderer.capture_motion_step(&state);
        state.update(0, 0);
        trace.push(serde_json::json!({"tick":tick,"x":state.player.x,"y":state.player.y,"vx":state.player.vx,"vy":state.player.vy,"grounded":state.player.grounded}));
        for frame in 0..3 {
            let mut ansi = Vec::new();
            renderer.render_to(&state, frame as f64 / 3.0, &mut ansi)?;
            fs::write(
                output.join(format!("motion-{:03}.ansi", tick * 3 + frame)),
                ansi,
            )?;
        }
    }
    fs::write(
        output.join("motion.json"),
        serde_json::to_vec_pretty(&trace)?,
    )?;
    drop(state);
    fs::remove_dir_all(root)?;
    Ok(())
}
