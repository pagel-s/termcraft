//! Focused full-body flight captures on disposable terrain; no player saves.
use std::{fs, path::Path};
use termcraft::{
    engine::{command::ClientCommand, state::GameState},
    entities::{blaze::Blaze, ghast::Ghast},
    renderer::Renderer,
    world::{Dimension, World, block::BlockType},
};
fn capture(s: &GameState, out: &Path, name: &str) -> std::io::Result<()> {
    for (w, h) in [(80, 24), (120, 40), (160, 48)] {
        let mut renderer = Renderer::with_size(w, h);
        let mut bytes = Vec::new();
        renderer.render_to(s, 0.0, &mut bytes)?;
        fs::write(out.join(format!("{name}-{w}x{h}.ansi")), bytes)?;
    }
    Ok(())
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let out = std::path::PathBuf::from(std::env::args().nth(1).ok_or("output directory required")?);
    fs::create_dir_all(&out)?;
    let root = std::env::temp_dir().join(format!("tc-flight-review-{}", rand::random::<u64>()));
    let mut s = GameState::open_in_directory(&root)?;
    s.dismiss_startup_splash();
    s.current_dimension = Dimension::Nether;
    s.world = World::open_in_directory(Dimension::Nether, &root)?;
    s.world.load_chunks_for_spawn_search(0, 48);
    s.apply_client_command(ClientCommand::ToggleRuleMobSpawning);
    s.apply_client_command(ClientCommand::ToggleRuleDaylightCycle);
    for x in -40..=40 {
        for y in 0..45 {
            s.world.set_block(
                x,
                y,
                if y >= 34 {
                    BlockType::Netherrack
                } else {
                    BlockType::Air
                },
            );
        }
    }
    for y in 27..34 {
        s.world.set_block(4, y, BlockType::StoneBricks);
    }
    for x in -4..=0 {
        s.world.set_block(x, 27, BlockType::StoneBricks);
    }
    for x in [-6, 0, 6] {
        s.world.set_block(x, 33, BlockType::Torch);
    }
    s.zombies.clear();
    s.skeletons.clear();
    s.creepers.clear();
    s.spiders.clear();
    s.slimes.clear();
    s.endermen.clear();
    s.player.x = -5.5;
    s.player.y = 34.0;
    s.player.grounded = true;
    let mut ghast = Ghast::new(2.0, 32.0);
    ghast.vx = 2.0;
    ghast.hit_timer = 5;
    let mut blaze = Blaze::new(-2.5, 30.5);
    blaze.vy = -1.5;
    blaze.hit_timer = 5;
    s.ghasts = vec![ghast];
    s.blazes = vec![blaze];
    capture(&s, &out, "flight-before")?;
    s.update(0, 0);
    capture(&s, &out, "flight-impact")?;
    fs::write(
        out.join("flight.json"),
        serde_json::to_vec_pretty(&serde_json::json!({
            "staged_fixture":true,"real_engine_ticks":1,"ghast":{"x":s.ghasts[0].x,"y":s.ghasts[0].y,"half_width":0.875,"wall_left":4},
            "blaze":{"x":s.blazes[0].x,"y":s.blazes[0].y,"height":1.75,"ceiling_bottom":28}
        }))?,
    )?;
    drop(s);
    fs::remove_dir_all(root)?;
    Ok(())
}
