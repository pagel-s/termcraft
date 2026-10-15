//! Actual falling weather and sheltered room captures in a disposable staged world.
use std::{
    fs,
    path::{Path, PathBuf},
};
use termcraft::{
    engine::state::{GameState, WeatherType},
    renderer::Renderer,
    world::{
        block::BlockType,
        environment,
        recipe::{Region, Terrain},
    },
};
fn fixture(root: &Path, terrain: Terrain) -> Result<GameState, Box<dyn std::error::Error>> {
    fs::create_dir_all(root)?;
    environment::write(
        root,
        &[Region {
            start_x: -64,
            end_x: 63,
            terrain,
        }],
    )?;
    let mut s = GameState::open_in_directory(root)?;
    s.dismiss_startup_splash();
    s.world.load_chunks_for_spawn_search(0, 80);
    for x in -80..=80 {
        for y in 0..100 {
            s.world.set_block(
                x,
                y,
                if y >= 64 {
                    BlockType::Stone
                } else {
                    BlockType::Air
                },
            );
        }
    }
    for x in 1..=13 {
        s.world.set_block(x, 58, BlockType::Planks);
    }
    for x in [1, 13] {
        for y in 59..64 {
            s.world.set_block(x, y, BlockType::Wood);
        }
    }
    s.world.set_block(4, 63, BlockType::Torch);
    s.player.x = -2.5;
    s.player.y = 64.0;
    s.player.vx = 0.0;
    s.player.vy = 0.0;
    s.player.grounded = true;
    s.time_of_day = 12000.0;
    s.weather = WeatherType::Rain;
    s.weather_rain_intensity = 1.0;
    s.inventory.slots.fill(None);
    Ok(s)
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let out = PathBuf::from(std::env::args().nth(1).ok_or("output directory required")?);
    fs::create_dir_all(&out)?;
    let root = std::env::temp_dir().join(format!(
        "termcraft-weather-review-{}",
        rand::random::<u64>()
    ));
    for (name, terrain, surface) in [
        ("rain", Terrain::Plains, BlockType::Grass),
        ("snow", Terrain::Tundra, BlockType::Snow),
    ] {
        let mut s = fixture(&root.join(name), terrain)?;
        for x in -80..=80 {
            if !(1..=13).contains(&x) {
                s.world.set_block(x, 64, surface);
            }
        }
        s.player.age = 40;
        for (view, toggles) in [("overview", 0), ("wide", 1), ("detail", 2)] {
            let mut r = Renderer::with_size(160, 48);
            for _ in 0..toggles {
                r.toggle_zoom();
            }
            let mut bytes = Vec::new();
            r.render_to(&s, 0.0, &mut bytes)?;
            fs::write(out.join(format!("{name}-{view}-160x48.ansi")), bytes)?;
        }
        let motion = out.join(format!("{name}-motion"));
        fs::create_dir_all(&motion)?;
        let mut r = Renderer::with_size(160, 48);
        for age in 0..48 {
            s.player.age = age;
            let mut bytes = Vec::new();
            r.render_to(&s, 0.0, &mut bytes)?;
            fs::write(motion.join(format!("frame-{age:03}.ansi")), bytes)?;
        }
    }
    fs::remove_dir_all(root)?;
    Ok(())
}
