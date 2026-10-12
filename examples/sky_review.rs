//! Staged sky and swing poses in disposable fixed geometry, through the real renderer.
use termcraft::{engine::state::GameState, renderer::Renderer, world::block::BlockType};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let root = std::env::temp_dir().join(format!("tc-sky-art-review-{}", std::process::id()));
    let out = std::path::PathBuf::from(std::env::args().nth(1).ok_or("output directory required")?);
    std::fs::create_dir_all(&out)?;
    let mut s = GameState::open_in_directory(&root)?;
    s.dismiss_startup_splash();
    s.world.load_chunks_for_spawn_search(0, 100);
    for x in -100..=100 {
        for y in 0..90 {
            s.world.set_block(
                x,
                y,
                if y > 32 {
                    BlockType::Dirt
                } else if y == 32 {
                    BlockType::Grass
                } else {
                    BlockType::Air
                },
            );
        }
    }
    s.player.x = 0.5;
    s.player.y = 32.0;
    s.player.vx = 0.0;
    s.player.vy = 0.0;
    s.player.grounded = true;
    s.inventory.slots[0] = None;
    for (name, time, mining, age) in [
        ("daylight", 12000.0, 0.0, 0),
        ("night", 22000.0, 0.0, 0),
        ("empty-hand", 12000.0, 0.5, 4),
    ] {
        s.time_of_day = time;
        s.player.mining_timer = mining;
        s.player.age = age;
        for (view, toggles) in [("overview", 0), ("wide", 1), ("detail", 2)] {
            let mut r = Renderer::with_size(160, 48);
            for _ in 0..toggles {
                r.toggle_zoom();
            }
            let mut bytes = Vec::new();
            r.render_to(&s, 0.0, &mut bytes)?;
            std::fs::write(out.join(format!("{name}-{view}-160x48.ansi")), bytes)?;
        }
    }
    std::fs::remove_dir_all(root)?;
    Ok(())
}
