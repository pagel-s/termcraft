//! Reproducible headless exploration/tick measurements. Never opens user saves.
use std::time::Instant;
use termcraft::engine::command::ClientCommand;
use termcraft::engine::state::GameState;
use termcraft::entities::zombie::Zombie;
use termcraft::world::recipe::WorldRecipe;
use termcraft::world::{Dimension, World};
use termcraft::worlds::WorldStore;

fn distribution(samples: &mut [f64]) -> serde_json::Value {
    samples.sort_by(f64::total_cmp);
    let percentile = |p: usize| samples[((samples.len() - 1) * p / 100).min(samples.len() - 1)];
    serde_json::json!({"p50_ms":percentile(50),"p95_ms":percentile(95),"p99_ms":percentile(99),"max_ms":samples[samples.len()-1]})
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let trim = std::env::args().any(|a| a == "--trim");
    let classic = std::env::args().any(|a| a == "--classic");
    let root = std::env::temp_dir().join(format!(
        "termcraft-profile-{}-{}",
        std::process::id(),
        rand::random::<u64>()
    ));
    let world_root = if classic {
        std::fs::create_dir(&root)?;
        std::fs::write(
            root.join("overworld_world_seed.bin"),
            bincode::serialize(&(1337u32, 7331u32, 9917u32, 17711u32))?,
        )?;
        root.clone()
    } else {
        let store = WorldStore::new(root.clone());
        store.create(
            "profile",
            "Fixed benchmark fixture",
            &WorldRecipe::example(),
        )?
    };
    let mut world = World::open_in_directory(Dimension::Overworld, &world_root)?;
    let mut loads = Vec::new();
    let mut peak_chunks = 0;
    for step in 0..240 {
        let x = step * 32;
        let started = Instant::now();
        world.load_chunks_for_motion(x as f64, 0.3);
        world.check_storage()?;
        if trim {
            world.trim_chunk_cache(&[x], 1)?;
        }
        loads.push(started.elapsed().as_secs_f64() * 1000.0);
        peak_chunks = peak_chunks.max(world.loaded_chunk_count());
    }
    let exploration = serde_json::json!({"chunk_centers":240,"trim":trim,"peak_resident_chunks":peak_chunks,
        "final_resident_chunks":world.loaded_chunk_count(),"evicted_chunks":world.chunk_metrics.evicted_chunks,"work":distribution(&mut loads)});
    drop(world);
    let opening_started = Instant::now();
    let mut state = GameState::open_in_directory(&world_root)?;
    let open_ms = opening_started.elapsed().as_secs_f64() * 1000.0;
    state.dismiss_startup_splash();
    state.time_of_day = 22000.0;
    state.apply_client_command(ClientCommand::ToggleRuleDaylightCycle);
    state.apply_client_command(ClientCommand::ToggleRuleMobSpawning);
    let mut ticks = Vec::new();
    // A fixed crowded scene exercises the real simulation, including collision.
    for i in 0..32 {
        state
            .zombies
            .push(Zombie::new((i - 16) as f64 + 0.5, state.player.y));
    }
    for _ in 0..600 {
        let started = Instant::now();
        state.update(0, 0);
        state.world.check_storage()?;
        if state.is_showing_death_screen() {
            return Err("profile entered death screen; tick fixture is invalid".into());
        }
        ticks.push(started.elapsed().as_secs_f64() * 1000.0);
        // Keep the workload active without letting a death screen replace it.
        state.player.health = state.player.max_health;
    }
    println!(
        "{}",
        serde_json::to_string_pretty(&serde_json::json!({
        "profile":"release, 240 explored chunk centers, 600 dense-mob ticks",
        "terrain":if classic {"classic seeds 1337/7331/9917/17711"} else {"recipe seed 42"},
        "exploration":exploration,"open_ms":open_ms,"dense_mob_ticks":distribution(&mut ticks),
        "final_zombie_count":state.zombies.len(),
        "limits":"Headless fixture, not a terminal FPS, whole-game or co-op benchmark. Trim includes budgeted disk writes. Terrain is fixed; runtime mob RNG is not."
        }))?
    );
    drop(state);
    std::fs::remove_dir_all(root)?;
    Ok(())
}
