//! Measure moving ANSI output, rather than qualifying a renderer with a still camera.
use std::io::{self, Write};
use std::time::Instant;
use termcraft::{engine::state::GameState, renderer::Renderer, world::block::BlockType};
#[derive(Default)]
struct Counter {
    bytes: usize,
}
impl Write for Counter {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.bytes += bytes.len();
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let root = std::env::temp_dir().join(format!(
        "termcraft-motion-profile-{}",
        rand::random::<u64>()
    ));
    let mut state = GameState::open_in_directory(&root)?;
    state.dismiss_startup_splash();
    state.world.load_chunks_for_spawn_search(0, 110);
    for x in -110..=110 {
        for y in 0..90 {
            state.world.set_block(
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
    state.player.y = 64.0;
    state.player.grounded = true;
    state.player.vx = 0.0;
    state.player.vy = 0.0;
    state.time_of_day = 12000.0;
    let mut results = Vec::new();
    for (width, height) in [(120, 40), (160, 48), (200, 60)] {
        for (mode, toggles) in [("overview", 0), ("wide", 1), ("detail", 2)] {
            for moving in [false, true] {
                state.player.x = 0.5;
                state.player.age = 0;
                let mut renderer = Renderer::with_size(width, height);
                for _ in 0..toggles {
                    renderer.toggle_zoom();
                }
                let mut sink = Counter::default();
                renderer.render_to(&state, 0.0, &mut sink)?;
                sink.bytes = 0;
                let mut timings = Vec::new();
                for frame in 0..180 {
                    state.player.x = if moving {
                        0.5 + frame as f64 * 0.32 / 3.0
                    } else {
                        0.5
                    };
                    state.player.age = frame / 3;
                    let start = Instant::now();
                    renderer.render_to(&state, 0.0, &mut sink)?;
                    timings.push(start.elapsed().as_secs_f64() * 1000.0);
                }
                timings.sort_by(f64::total_cmp);
                results.push(serde_json::json!({"viewport":[width,height],"view":mode,"moving":moving,"frames":180,"bytes_per_frame":sink.bytes as f64/180.0,"estimated_bytes_per_second_at_60fps":sink.bytes as f64/3.0,"p50_ms":timings[89],"p95_ms":timings[170]}));
            }
        }
    }
    println!(
        "{}",
        serde_json::to_string_pretty(
            &serde_json::json!({"color_mode":if std::env::var_os("NO_COLOR").is_some(){"no_color"}else{"color"},"results":results,"limits":"Synthetic camera pan at 6.4 blocks/s over fixed flat geometry, real renderer and ANSI counting writer. Excludes emulator, kernel I/O, actual input, simulation and network. Bandwidth at 60 fps is an estimate, not achieved frame rate."})
        )?
    );
    drop(state);
    std::fs::remove_dir_all(root)?;
    Ok(())
}
