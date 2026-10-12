//! Deterministic art-review frames, through the same renderer used by the game.
use termcraft::engine::state::GameState;
use termcraft::entities::{
    cow::Cow, creeper::Creeper, pig::Pig, sheep::Sheep, skeleton::Skeleton, zombie::Zombie,
};
use termcraft::renderer::Renderer;
use termcraft::world::{block::BlockType, recipe::WorldRecipe};
use termcraft::worlds::WorldStore;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output =
        std::path::PathBuf::from(std::env::args().nth(1).ok_or("output directory required")?);
    std::fs::create_dir_all(&output)?;
    let root = std::env::temp_dir().join(format!(
        "termcraft-art-{}-{}",
        std::process::id(),
        rand::random::<u64>()
    ));
    let path =
        WorldStore::new(root.clone()).create("art", "Art review", &WorldRecipe::example())?;
    let mut state = GameState::open_in_directory(&path)?;
    state.dismiss_startup_splash();
    state.world.load_chunks_for_spawn_search(0, 65);
    for x in -65..=65 {
        for y in 0..80 {
            state.world.set_block(
                x,
                y,
                if y == 34 {
                    BlockType::Grass
                } else if y > 38 {
                    BlockType::Stone
                } else if y > 34 {
                    BlockType::Dirt
                } else {
                    BlockType::Air
                },
            );
        }
    }
    for x in [-12, 13] {
        for y in 27..34 {
            state.world.set_block(x, y, BlockType::Wood);
        }
        for dx in -3..=3 {
            for y in 24..29 {
                if dx * dx + (y - 26) * (y - 26) < 13 {
                    state.world.set_block(x + dx, y, BlockType::Leaves);
                }
            }
        }
    }
    for x in -22..=-17 {
        for y in 29..34 {
            if x == -22 || x == -17 || y == 29 {
                state.world.set_block(x, y, BlockType::Planks);
            }
        }
    }
    state.world.set_block(-19, 33, BlockType::Torch);
    state.player.x = 0.5;
    state.player.y = 34.0;
    state.player.grounded = true;
    state.player.vx = 0.3;
    state.player.vy = 0.0;
    state.time_of_day = 7000.0;
    state.zombies = vec![Zombie::new(5.5, 34.0)];
    state.skeletons = vec![Skeleton::new(8.5, 34.0)];
    state.creepers = vec![Creeper::new(11.0, 34.0)];
    state.cows = vec![Cow::new(-3.5, 34.0)];
    state.sheep = vec![Sheep::new(-6.5, 34.0)];
    state.pigs = vec![Pig::new(-9.5, 34.0)];
    for (name, wide) in [("close", false), ("wide", true)] {
        let mut renderer = Renderer::with_size(160, 48);
        if wide {
            renderer.toggle_zoom();
        }
        for frame in 0..12 {
            state.player.age = frame;
            let mut ansi = Vec::new();
            renderer.render_to(&state, 0.0, &mut ansi)?;
            std::fs::write(output.join(format!("{name}-{frame:02}.ansi")), ansi)?;
        }
    }
    drop(state);
    std::fs::remove_dir_all(root)?;
    Ok(())
}
