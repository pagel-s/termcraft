//! Generated Nether terrain captures in disposable seeded worlds.
//! Only the player/camera are staged; no terrain or creature art is modified.
use std::{fs, path::Path};
use termcraft::{
    engine::state::GameState,
    renderer::Renderer,
    world::{Dimension, World, block::BlockType},
};

fn capture(state: &GameState, output: &Path, name: &str) -> std::io::Result<()> {
    for (width, height) in [(160, 48), (200, 60)] {
        let mut renderer = Renderer::with_size(width, height);
        let mut ansi = Vec::new();
        renderer.render_to(state, 0.0, &mut ansi)?;
        fs::write(output.join(format!("{name}-{width}x{height}.ansi")), ansi)?;
    }
    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output =
        std::path::PathBuf::from(std::env::args().nth(1).ok_or("output directory required")?);
    fs::create_dir_all(&output)?;
    let root = std::env::temp_dir().join(format!(
        "termcraft-nether-review-{}-{}",
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
    state.current_dimension = Dimension::Nether;
    state.world = World::open_in_directory(Dimension::Nether, &root)?;
    let fortress_columns: Vec<_> = (0..768)
        .filter(|&x| (40..60).any(|y| state.world.is_nether_fortress_zone(x, y)))
        .collect();
    let fortress_x = (fortress_columns.first().ok_or("fortress missing")?
        + fortress_columns.last().ok_or("fortress missing")?)
        / 2;
    let mut metadata = Vec::new();
    for (name, x) in [("cavern", 0), ("fortress", fortress_x)] {
        state.world.load_chunks_for_spawn_search(x, 64);
        let y = (30..96)
            .filter(|&y| {
                state.world.get_block(x, y).is_solid()
                    && state.world.get_block(x, y - 1) == BlockType::Air
                    && state.world.get_block(x, y - 2) == BlockType::Air
            })
            .min_by_key(|y| (y - 60i32).abs())
            .ok_or("no clear player position")?;
        state.player.x = x as f64 + 0.5;
        state.player.y = y as f64;
        state.player.grounded = true;
        state.player.vx = 0.0;
        state.player.vy = 0.0;
        capture(&state, &output, name)?;
        metadata.push(
            serde_json::json!({"scene":name,"player_x":state.player.x,"player_y":state.player.y}),
        );
    }
    fs::write(
        output.join("scenes.json"),
        serde_json::to_vec_pretty(&serde_json::json!({
            "seed":[1337,7331,9917,4444],"terrain_modified":false,
            "staged_player_positions":true,"scenes":metadata
        }))?,
    )?;
    drop(state);
    fs::remove_dir_all(root)?;
    Ok(())
}
