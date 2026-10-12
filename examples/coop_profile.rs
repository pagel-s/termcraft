//! Accelerated simulated-hour exploration, maintenance and snapshot encoding.
use std::time::Instant;
use termcraft::engine::{
    command::ClientCommand,
    net::{ClientInputFrame, ServerSimulation, encode_server_snapshot},
    state::GameState,
};
use termcraft::world::{
    item::ItemType,
    recipe::{Terrain, WorldRecipe},
};
use termcraft::worlds::WorldStore;
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let ticks = std::env::args()
        .nth(1)
        .map(|n| n.parse::<u32>())
        .transpose()?
        .unwrap_or(72000)
        .max(1);
    let root =
        std::env::temp_dir().join(format!("termcraft-coop-profile-{}", rand::random::<u64>()));
    let mut recipe = WorldRecipe::example();
    recipe.terrain = Terrain::Plains;
    recipe.relief = 0;
    recipe.regions.clear();
    recipe.landmarks.clear();
    let world = WorldStore::new(root.clone()).create(
        "profile",
        "Accelerated co-op exploration",
        &recipe,
    )?;
    let mut state = GameState::open_in_directory(&world)?;
    state.dismiss_startup_splash();
    state.apply_client_command(ClientCommand::ToggleRuleMobSpawning);
    let mut sim = ServerSimulation::with_state(state);
    for id in 1..=8 {
        sim.register_client(id);
    }
    let mut samples = Vec::new();
    let mut bytes = 0u64;
    let mut peak_chunks = 0;
    let mut peak_rss_kib = 0;
    let mut sequence = 0u32;
    let started = Instant::now();
    for tick in 0..ticks {
        let frame = Instant::now();
        if tick % 10 == 0 {
            sequence += 1;
            for id in 1..=8 {
                sim.enqueue_input(ClientInputFrame::new(
                    id,
                    sequence,
                    0,
                    if id % 2 == 0 {
                        ClientCommand::SetMoveLeft(true)
                    } else {
                        ClientCommand::SetMoveRight(true)
                    },
                ));
            }
        }
        sim.step(0, 0);
        sim.maintain_chunk_cache()?;
        for id in 1..=8 {
            let snapshot = sim.snapshot_for_client(id).unwrap();
            if snapshot.death_screen_active {
                return Err("actor died; workload invalid".into());
            }
            if snapshot.session.inventory().has_item(ItemType::Diamond, 1) {
                return Err("unexpected inventory mutation".into());
            }
            bytes += encode_server_snapshot(&snapshot)?.len() as u64;
        }
        if tick % 900 == 899 {
            sim.save()?;
        }
        samples.push(frame.elapsed().as_secs_f64() * 1000.0);
        peak_chunks = peak_chunks.max(sim.state().world.loaded_chunk_count());
        if tick % 600 == 0
            && let Ok(status) = std::fs::read_to_string("/proc/self/status")
            && let Some(rss) = status.lines().find_map(|l| {
                l.strip_prefix("VmRSS:")
                    .and_then(|s| s.split_whitespace().next())
                    .and_then(|s| s.parse::<u64>().ok())
            })
        {
            peak_rss_kib = peak_rss_kib.max(rss);
        }
    }
    sim.save()?;
    samples.sort_by(f64::total_cmp);
    println!(
        "{}",
        serde_json::to_string_pretty(
            &serde_json::json!({"actors":8,"ticks":ticks,"simulated_minutes":ticks as f64/1200.0,
        "elapsed_seconds":started.elapsed().as_secs_f64(),"p95_ms":samples[(samples.len()-1)*95/100],"p99_ms":samples[(samples.len()-1)*99/100],"max_ms":samples[samples.len()-1],
        "peak_resident_chunks":peak_chunks,"peak_sampled_rss_kib":peak_rss_kib,"snapshot_bytes":bytes,
        "limits":"Accelerated fixed-terrain exploration with eight normal actors in two opposing groups, spawning disabled, budgeted cache writes and snapshot encoding. Not a wall-clock soak, network latency measurement or full survival campaign."})
        )?
    );
    drop(sim);
    std::fs::remove_dir_all(root)?;
    Ok(())
}
