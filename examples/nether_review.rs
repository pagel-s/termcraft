//! Generated Nether terrain captures in disposable seeded worlds.
//! Only player/camera positions are staged by default; --creatures adds an
//! explicitly staged encounter and --animate a fixed-camera presentation loop.
use std::{fs, path::Path};
use termcraft::{
    engine::state::GameState,
    entities::{blaze::Blaze, ghast::Ghast, zombie_pigman::ZombiePigman},
    renderer::Renderer,
    world::{Dimension, World, block::BlockType},
};

fn capture(state: &GameState, output: &Path, name: &str) -> std::io::Result<()> {
    for (width, height, detail) in [
        (120, 40, false),
        (160, 48, false),
        (200, 60, false),
        (160, 48, true),
    ] {
        let mut renderer = Renderer::with_size(width, height);
        if detail {
            renderer.toggle_zoom();
            renderer.toggle_zoom();
        }
        let mut ansi = Vec::new();
        renderer.render_to(state, 0.0, &mut ansi)?;
        let suffix = if detail { "-detail" } else { "" };
        fs::write(
            output.join(format!("{name}{suffix}-{width}x{height}.ansi")),
            ansi,
        )?;
    }
    Ok(())
}

fn stand_y(world: &World, x: i32) -> Option<i32> {
    (30..96)
        .filter(|&y| {
            world.get_block(x, y).is_solid()
                && world.get_block(x, y - 1) == BlockType::Air
                && world.get_block(x, y - 2) == BlockType::Air
        })
        .min_by_key(|y| (y - 60i32).abs())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output =
        std::path::PathBuf::from(std::env::args().nth(1).ok_or("output directory required")?);
    fs::create_dir_all(&output)?;
    let creatures = std::env::args().any(|s| s == "--creatures");
    let animate = std::env::args().any(|s| s == "--animate");
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
        let y = stand_y(&state.world, x).ok_or("no clear player position")?;
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
    if creatures {
        state.world.load_chunks_for_spawn_search(0, 64);
        let y = stand_y(&state.world, 0).ok_or("no cavern position")?;
        state.player.x = 0.5;
        state.player.y = y as f64;
        state.pigmen = vec![ZombiePigman::new(
            -4.5,
            stand_y(&state.world, -5).ok_or("no pigman support")? as f64,
        )];
        state.blazes = vec![Blaze::new(4.5, y as f64 - 2.5)];
        state.blazes[0].shoot_cooldown = 35;
        state.ghasts = vec![Ghast::new(9.5, y as f64 - 5.0)];
        state.ghasts[0].shoot_cooldown = 50;
        // Reject obstructed fixture positions instead of carving a scenic arena.
        for (x, feet, half, height) in [
            (4.5_f64, y as f64 - 2.5, 0.5, 1.75),
            (9.5_f64, y as f64 - 5.0, 0.875, 1.875),
        ] {
            for bx in (x - half).floor() as i32..=(x + half).floor() as i32 {
                for by in (feet - height).floor() as i32..feet.ceil() as i32 {
                    if state.world.get_block(bx, by) != BlockType::Air {
                        return Err("staged flyer obstructed".into());
                    }
                }
            }
        }
        capture(&state, &output, "encounter")?;
        if animate {
            let mut renderer = Renderer::with_size(160, 48);
            for frame in 0..72u64 {
                state.player.age = frame / 3;
                state.blazes[0].age = frame / 3;
                state.ghasts[0].age = frame / 3;
                state.blazes[0].shoot_cooldown = if frame >= 36 { 5 } else { 35 };
                state.ghasts[0].shoot_cooldown = if frame >= 36 { 5 } else { 50 };
                let mut ansi = Vec::new();
                renderer.render_to(&state, (frame % 3) as f64 / 3.0, &mut ansi)?;
                fs::write(output.join(format!("motion-{frame:03}-160x48.ansi")), ansi)?;
            }
        }
    }
    fs::write(
        output.join("scenes.json"),
        serde_json::to_vec_pretty(&serde_json::json!({
            "seed":[1337,7331,9917,4444],"terrain_modified":false,
            "staged_player_positions":true,"staged_creature_positions":creatures,"scenes":metadata
        }))?,
    )?;
    drop(state);
    fs::remove_dir_all(root)?;
    Ok(())
}
