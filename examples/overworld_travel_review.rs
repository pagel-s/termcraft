//! Native terrain/spawning review; only creates disposable seeded saves.
use serde_json::json;
use std::{fs, path::PathBuf, time::Instant};
use termcraft::{
    engine::{command::ClientCommand, state::GameState},
    renderer::Renderer,
    world::block::BlockType,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let out = PathBuf::from(std::env::args().nth(1).ok_or("output directory required")?);
    fs::create_dir_all(&out)?;
    let root = std::env::temp_dir().join(format!("termcraft-travel-review-{}", std::process::id()));
    fs::create_dir(&root)?;
    let seeds = [1337u32, 7331, 9917, 17711];
    fs::write(
        root.join("overworld_world_seed.bin"),
        bincode::serialize(&seeds)?,
    )?;
    let mut s = GameState::open_in_directory(&root)?;
    s.dismiss_startup_splash();
    s.apply_client_command(ClientCommand::ToggleRuleDaylightCycle);
    s.apply_client_command(ClientCommand::ToggleRuleWeatherCycle);
    let mut records = Vec::new();
    let mut night_position = None;
    for (id, center) in [
        ("route-0", 0),
        ("route-450", 450),
        ("route-900", 900),
        ("route-1350", 1350),
    ] {
        let t = Instant::now();
        s.world.load_chunks_for_spawn_search(center, 160);
        let load_ms = t.elapsed().as_secs_f64() * 1000.0;
        let (x, y) = (-96i32..=96)
            .filter_map(|dx| {
                let x = center + dx;
                let y = (3..110).find(|&y| {
                    matches!(
                        s.world.get_block(x, y),
                        BlockType::Grass | BlockType::Snow | BlockType::Sand | BlockType::Stone
                    )
                })?;
                ((1..=2).all(|dy| s.world.get_block(x, y - dy) == BlockType::Air)
                    && s.world.has_open_sky(x, y - 1))
                .then_some((x, y, dx.abs()))
            })
            .min_by_key(|p| p.2)
            .map(|(x, y, _)| (x, y))
            .ok_or("no dry surface camera position")?;
        night_position.get_or_insert((x, y));
        if center == 900 {
            night_position = Some((x, y));
        }
        s.player.x = x as f64 + 0.5;
        s.player.y = y as f64;
        s.player.vx = 0.0;
        s.player.vy = 0.0;
        s.player.grounded = true;
        s.time_of_day = 10000.0;
        for (view, width, height, zoom) in [("panorama", 400, 80, 0), ("detail", 200, 64, 2)] {
            let mut r = Renderer::with_size(width, height);
            for _ in 0..zoom {
                r.toggle_zoom();
            }
            let mut ansi = Vec::new();
            r.render_to(&s, 0.0, &mut ansi)?;
            fs::write(out.join(format!("{id}-{view}-{width}x{height}.ansi")), ansi)?;
        }
        records.push(json!({"id":id,"x":x,"y":y,"biome":format!("{:?}",s.world.get_biome(x)),"load_ms":load_ms}));
    }
    // Leave the already explored ground loaded and wait through real simulation,
    // rather than placing showcase monsters. Keep the review player alive.
    let (x, y) = night_position.ok_or("no night observation floor")?;
    s.world.load_chunks_for_spawn_search(x, 96);
    s.player.x = x as f64 + 0.5;
    s.player.y = y as f64;
    s.player.vx = 0.0;
    s.player.vy = 0.0;
    s.player.grounded = true;
    s.time_of_day = 23000.0;
    for _ in 0..1200 {
        s.update(0, 0);
        s.player.health = s.player.max_health;
    }
    let surface = |x: f64, y: f64| {
        s.world
            .has_open_sky(x.floor() as i32, (y - 1.0).floor() as i32)
    };
    let surface_mobs = s.zombies.iter().filter(|m| surface(m.x, m.y)).count()
        + s.creepers.iter().filter(|m| surface(m.x, m.y)).count()
        + s.skeletons.iter().filter(|m| surface(m.x, m.y)).count()
        + s.spiders.iter().filter(|m| surface(m.x, m.y)).count();
    let mut r = Renderer::with_size(200, 64);
    r.toggle_zoom();
    r.toggle_zoom();
    let mut ansi = Vec::new();
    r.render_to(&s, 0.0, &mut ansi)?;
    fs::write(out.join("night-detail-200x64.ansi"), ansi)?;
    fs::write(
        out.join("evidence.json"),
        serde_json::to_vec_pretty(&json!({
            "seeds":seeds,"scenes":records,"night_simulation_ticks":1200,"night_surface_mobs":surface_mobs,
            "player_health_held_for_observation":true,
            "limits":"Native generated terrain; camera/player positions and time staged. Night mobs are natural random spawns, not deterministic counts. No user saves opened."
        }))?,
    )?;
    drop(s);
    fs::remove_dir_all(root)?;
    Ok(())
}
