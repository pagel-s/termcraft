//! Fixed attachment fixtures through the real renderer; never opens user saves.
use std::{fs, path::PathBuf};
use termcraft::{engine::state::GameState, renderer::Renderer, world::block::BlockType};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let out = PathBuf::from(std::env::args().nth(1).ok_or("output directory required")?);
    fs::create_dir_all(&out)?;
    let root = std::env::temp_dir().join(format!("termcraft-torch-art-{}", rand::random::<u64>()));
    let mut s = GameState::open_in_directory(&root)?;
    s.dismiss_startup_splash();
    s.world.load_chunks_for_spawn_search(0, 40);
    for x in -40..=40 {
        for y in 0..65 {
            s.world.set_block(
                x,
                y,
                if y >= 28 {
                    BlockType::Stone
                } else {
                    BlockType::Air
                },
            );
        }
    }
    for x in -8..=8 {
        s.world.set_block(x, 22, BlockType::Planks);
    }
    for x in [-8, 8] {
        for y in 23..28 {
            s.world.set_block(x, y, BlockType::Wood);
        }
    }
    s.world.set_block(-2, 28, BlockType::StoneSlab);
    let positions = [(-4, 27), (-2, 27), (-7, 24), (7, 24), (3, 25)];
    let mut mounts = Vec::new();
    for (x, y) in positions {
        s.world.set_block(x, y, BlockType::Torch);
        mounts.push(serde_json::json!({"x":x,"y":y,"attachment":format!("{:?}",s.world.torch_attachment(x,y).ok_or("fixture has no support")?)}));
    }
    s.player.x = 0.5;
    s.player.y = 28.0;
    s.player.grounded = true;
    for (name, time) in [("day", 8500.0), ("night", 23000.0)] {
        s.time_of_day = time;
        for (view, toggles) in [("overview", 0), ("wide", 1), ("detail", 2)] {
            let mut r = Renderer::with_size(160, 48);
            for _ in 0..toggles {
                r.toggle_zoom();
            }
            let mut bytes = Vec::new();
            r.render_to(&s, 0.0, &mut bytes)?;
            fs::write(out.join(format!("{name}-{view}-160x48.ansi")), bytes)?;
        }
    }
    fs::write(
        out.join("attachments.json"),
        serde_json::to_vec_pretty(&serde_json::json!({"fixtures":mounts,"staged":true}))?,
    )?;
    drop(s);
    fs::remove_dir_all(root)?;
    Ok(())
}
