//! Real renderer/ANSI work with a counting writer. Never opens user saves.
use std::io::{self, Write};
use std::time::Instant;
use termcraft::engine::state::{GameState, WeatherType};
use termcraft::entities::zombie::Zombie;
use termcraft::entities::{end_crystal::EndCrystal, ender_dragon::EnderDragon};
use termcraft::renderer::Renderer;
use termcraft::world::{Dimension, World, block::BlockType, recipe::WorldRecipe};
use termcraft::worlds::WorldStore;

#[derive(Default)]
struct Counter {
    bytes: usize,
    writes: usize,
    capture: Option<std::fs::File>,
}
impl Write for Counter {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if let Some(file) = &mut self.capture {
            file.write_all(bytes)?;
        }
        self.bytes += bytes.len();
        self.writes += 1;
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let capture = std::env::args().nth(1).map(std::path::PathBuf::from);
    if let Some(path) = &capture {
        std::fs::create_dir_all(path)?;
    }
    let root = std::env::temp_dir().join(format!(
        "termcraft-render-{}-{}",
        std::process::id(),
        rand::random::<u64>()
    ));
    let mut recipe = WorldRecipe::example();
    recipe.relief = 0;
    recipe.landmarks.clear();
    let path = WorldStore::new(root.clone()).create("render", "Renderer fixture", &recipe)?;
    let mut results = Vec::new();
    for scene in [
        "daylight",
        "storm",
        "torch_cave",
        "dense_mobs",
        "nether",
        "end",
        "end_dragon",
    ] {
        let mut state = GameState::open_in_directory(&path)?;
        state.dismiss_startup_splash();
        state.current_dimension = match scene {
            "nether" => Dimension::Nether,
            "end" | "end_dragon" => Dimension::End,
            _ => Dimension::Overworld,
        };
        state.world = World::open_in_directory(state.current_dimension, &path)?;
        state.world.load_chunks_for_spawn_search(0, 110);
        // Fixed geometry avoids terrain RNG and scene-dependent camera placement.
        for x in -110..=110 {
            for y in 0..105 {
                let block = if y >= 64 || (scene == "torch_cave" && y < 40) {
                    BlockType::Stone
                } else {
                    BlockType::Air
                };
                state.world.set_block(x, y, block);
            }
        }
        if scene == "torch_cave" {
            for x in (-96..=96).step_by(8) {
                state.world.set_block(x, 62, BlockType::Torch);
            }
        }
        if scene == "nether" {
            for x in (-96..=96).step_by(8) {
                state.world.set_block(x, 63, BlockType::Lava(0));
            }
        }
        if scene == "dense_mobs" {
            for x in -24..24 {
                state.zombies.push(Zombie::new(x as f64, 64.0));
            }
        }
        state.time_of_day = if scene == "torch_cave" {
            22000.0
        } else {
            12000.0
        };
        state.weather = if scene == "storm" {
            WeatherType::Thunderstorm
        } else {
            WeatherType::Clear
        };
        state.weather_rain_intensity = if scene == "storm" { 1.0 } else { 0.0 };
        state.player.x = 0.5;
        state.player.y = 64.0;
        state.player.vx = 0.0;
        state.player.vy = 0.0;
        if scene == "end_dragon" {
            state.ender_dragon = Some(EnderDragon::new(0.5, 62.0));
            state.end_crystals = vec![EndCrystal::new(-7.5, 60.0)];
        }
        for (width, height) in [(120, 40), (200, 60)] {
            let mut renderer = Renderer::with_size(width, height);
            let mut samples = Vec::new();
            let mut sink = Counter::default();
            if let Some(path) = &capture {
                sink.capture = Some(std::fs::File::create(
                    path.join(format!("{scene}-{width}x{height}.ansi")),
                )?);
            }

            for frame in 0..210 {
                state.player.age = frame;
                if let Some(dragon) = &mut state.ender_dragon {
                    dragon.age = frame / 3;
                    dragon.phase = if frame >= 105 {
                        termcraft::entities::ender_dragon::DragonPhase::Swoop
                    } else {
                        termcraft::entities::ender_dragon::DragonPhase::Patrol
                    };
                }
                state.time_of_day += 0.1;
                let started = Instant::now();
                renderer.render_to(
                    &state,
                    if scene == "end_dragon" {
                        (frame % 3) as f64 / 3.0
                    } else {
                        0.0
                    },
                    &mut sink,
                )?;
                if frame >= 30 {
                    samples.push(started.elapsed().as_secs_f64() * 1000.0);
                }
            }
            samples.sort_by(f64::total_cmp);
            results.push(
                serde_json::json!({"scene":scene,"viewport":[width,height],"frames":210,
                "p50_ms":samples[89],"p95_ms":samples[170],"p99_ms":samples[177],
                "bytes":sink.bytes,"write_calls":sink.writes}),
            );
        }
    }
    println!(
        "{}",
        serde_json::to_string_pretty(
            &serde_json::json!({"results":results, "color_mode": if std::env::var_os("NO_COLOR").is_some() {"NO_COLOR"} else {"color"},
        "limits":"Real renderer and ANSI encoding to a counting writer; excludes emulator presentation, kernel I/O, simulation, model and networking."})
        )?
    );
    std::fs::remove_dir_all(root)?;
    Ok(())
}
