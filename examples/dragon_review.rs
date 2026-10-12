//! Dragon poses and a fixed-position wing loop through the real renderer.
//! Disposable staging fixture; this does not alter boss AI or player saves.
use std::{fs, path::Path};
use termcraft::{
    engine::state::GameState,
    entities::{
        end_crystal::EndCrystal,
        ender_dragon::{DragonPhase, EnderDragon},
        enderman::Enderman,
    },
    renderer::Renderer,
    world::{Dimension, World, block::BlockType},
};

fn capture(
    state: &GameState,
    output: &Path,
    name: &str,
    width: u16,
    height: u16,
    alpha: f64,
) -> std::io::Result<()> {
    let mut renderer = Renderer::with_size(width, height);
    let mut ansi = Vec::new();
    renderer.render_to(state, alpha, &mut ansi)?;
    fs::write(output.join(format!("{name}-{width}x{height}.ansi")), ansi)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output =
        std::path::PathBuf::from(std::env::args().nth(1).ok_or("output directory required")?);
    fs::create_dir_all(&output)?;
    let root = std::env::temp_dir().join(format!(
        "termcraft-dragon-review-{}-{}",
        std::process::id(),
        rand::random::<u64>()
    ));
    fs::create_dir_all(&root)?;
    let seeds = bincode::serialize(&[1337u32, 7331, 9917, 4444])?;
    for dimension in ["overworld", "nether", "end"] {
        fs::write(root.join(format!("{dimension}_world_seed.bin")), &seeds)?;
    }
    let mut state = GameState::open_in_directory(&root)?;
    state.dismiss_startup_splash();
    state.current_dimension = Dimension::End;
    state.world = World::open_in_directory(Dimension::End, &root)?;
    state.world.load_chunks_for_spawn_search(0, 40);
    for x in -40..=40 {
        for y in 0..50 {
            state.world.set_block(
                x,
                y,
                if y >= 34 {
                    BlockType::EndStone
                } else {
                    BlockType::Air
                },
            );
        }
    }
    state.player.x = 0.5;
    state.player.y = 34.0;
    state.player.grounded = true;
    state.endermen = vec![Enderman::new(-3.0, 34.0)];
    for y in 30..34 {
        state.world.set_block(-8, y, BlockType::Obsidian);
    }
    state.end_crystals = vec![EndCrystal::new(-7.5, 30.0)];
    for pose in ["raised", "lowered", "left", "swoop", "hit"] {
        let mut dragon = EnderDragon::new(0.5, 32.0);
        dragon.age = if pose == "lowered" { 11 } else { 0 };
        dragon.facing_right = pose != "left";
        if pose == "swoop" {
            dragon.phase = DragonPhase::Swoop;
            dragon.age = 5;
        }
        if pose == "hit" {
            dragon.hit_timer = 6;
        }
        state.ender_dragon = Some(dragon);
        for (width, height) in [(120, 40), (160, 48), (200, 60)] {
            capture(&state, &output, pose, width, height, 0.0)?;
        }
    }
    for frame in 0..66 {
        let mut dragon = EnderDragon::new(0.5, 32.0);
        dragon.age = frame / 3;
        state.ender_dragon = Some(dragon);
        capture(
            &state,
            &output,
            &format!("flight-{frame:03}"),
            160,
            48,
            (frame % 3) as f64 / 3.0,
        )?;
    }
    drop(state);
    fs::remove_dir_all(root)?;
    Ok(())
}
