//! Actual seeded End terrain and a shape/precision course in disposable saves.
use std::{fs, path::Path};
use termcraft::{
    engine::{command::ClientCommand, state::GameState},
    entities::{end_crystal::EndCrystal, ender_dragon::EnderDragon, enderman::Enderman},
    renderer::Renderer,
    world::{Dimension, END_TOWER_XS, World, block::BlockType},
};

fn capture(
    state: &GameState,
    output: &Path,
    name: &str,
    width: u16,
    height: u16,
) -> std::io::Result<()> {
    let mut renderer = Renderer::with_size(width, height);
    let mut ansi = Vec::new();
    renderer.render_to(state, 0.0, &mut ansi)?;
    fs::write(output.join(format!("{name}-{width}x{height}.ansi")), ansi)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output =
        std::path::PathBuf::from(std::env::args().nth(1).ok_or("output directory required")?);
    fs::create_dir_all(&output)?;
    let root = std::env::temp_dir().join(format!(
        "termcraft-translation-{}-{}",
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
    state.current_dimension = Dimension::End;
    state.world = World::open_in_directory(Dimension::End, &root)?;
    state.world.load_chunks_for_spawn_search(0, 64);
    // Crystal anchors come from generated geometry, never a four-block placeholder.
    state.end_crystals = END_TOWER_XS
        .iter()
        .filter_map(|&x| {
            (2..64)
                .find(|&y| state.world.get_block(x, y) == BlockType::Glowstone)
                .map(|y| EndCrystal::new(x as f64 + 0.5, y as f64 - 1.0))
        })
        .collect();
    state.player.x = 7.5;
    state.player.y = (2..64)
        .find(|&y| state.world.get_block(7, y) == BlockType::EndStone)
        .unwrap() as f64;
    state.player.grounded = true;
    state.endermen = vec![Enderman::new(9.5, state.player.y)];
    state.ender_dragon = Some(EnderDragon::new(5.5, 25.0));
    for (w, h) in [(120, 40), (160, 48), (200, 60)] {
        capture(&state, &output, "end-arena", w, h)?;
    }
    state.current_dimension = Dimension::Overworld;
    state.world = World::open_in_directory(Dimension::Overworld, &root)?;
    state.world.load_chunks_for_spawn_search(0, 32);
    state.endermen.clear();
    state.end_crystals.clear();
    state.ender_dragon = None;
    state.apply_client_command(ClientCommand::ToggleRuleMobSpawning);
    for x in -30..=30 {
        for y in 0..50 {
            state.world.set_block(
                x,
                y,
                if y >= 34 {
                    BlockType::Stone
                } else {
                    BlockType::Air
                },
            );
        }
    }
    for x in [0, 1] {
        state.world.set_block(x, 33, BlockType::StoneSlab);
    }
    state.world.set_block(2, 33, BlockType::StoneStairs);
    for x in 3..=5 {
        state.world.set_block(x, 33, BlockType::Stone);
    }
    for x in 6..=8 {
        state.world.set_block(x, 33, BlockType::StoneSlab);
    }
    state.player.x = -3.5;
    state.player.y = 34.0;
    state.player.vx = 0.0;
    state.player.vy = 0.0;
    state.player.grounded = true;
    state.apply_client_command(ClientCommand::SetMoveRight(true));
    let mut renderer = Renderer::with_size(160, 48);
    let mut trace = Vec::new();
    for tick in 0..56 {
        if tick == 42 {
            state.apply_client_command(ClientCommand::ClearDirectionalInput);
        }
        renderer.capture_motion_step(&state);
        state.update(0, 0);
        trace.push(serde_json::json!({"tick":tick,"x":state.player.x,"y":state.player.y,"vx":state.player.vx,"vy":state.player.vy,"grounded":state.player.grounded}));
        for frame in 0..3 {
            let mut ansi = Vec::new();
            renderer.render_to(&state, frame as f64 / 3.0, &mut ansi)?;
            fs::write(
                output.join(format!("course-{:03}.ansi", tick * 3 + frame)),
                ansi,
            )?;
        }
    }
    fs::write(
        output.join("course.json"),
        serde_json::to_vec_pretty(&trace)?,
    )?;
    drop(state);
    fs::remove_dir_all(root)?;
    Ok(())
}
