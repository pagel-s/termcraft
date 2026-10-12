//! Release simulation timing in a generated village; disposable save, no user data.
use std::time::Instant;
use termcraft::engine::command::ClientCommand;
use termcraft::engine::state::GameState;
use termcraft::entities::villager::Villager;
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let root = std::env::temp_dir().join(format!("tc-review-prof-{}", std::process::id()));
    std::fs::create_dir_all(&root)?;
    let bytes = [1337u32, 7331, 9917, 4444]
        .iter()
        .flat_map(|x| x.to_le_bytes())
        .collect::<Vec<_>>();
    std::fs::write(root.join("overworld_world_seed.bin"), bytes)?;
    let mut s = GameState::open_in_directory(&root)?;
    s.dismiss_startup_splash();
    s.apply_client_command(ClientCommand::ToggleRuleMobSpawning);
    s.apply_client_command(ClientCommand::ToggleRuleDaylightCycle);
    s.apply_client_command(ClientCommand::ToggleRuleWeatherCycle);
    s.time_of_day = 6000.;
    let sites = s.world.village_sites_near(0, 10000);
    let site = sites
        .iter()
        .filter(|v| v.center_x < 0)
        .min_by_key(|v| v.center_x.abs())
        .unwrap();
    let cx = site.center_x;
    let g = site.ground_y;
    s.world.load_chunks_for_spawn_search(cx, 100);
    s.player.x = cx as f64 + 12.5;
    s.player.y = g as f64;
    s.player.grounded = true;
    for (l, _, _) in site.buildings() {
        for d in 0..2 {
            let mut v = Villager::new(l as f64 - 3.5 + d as f64, g as f64, l, g - 1);
            v.grounded = true;
            s.villagers.push(v);
        }
    }
    for _ in 0..100 {
        s.update(0, 0);
    }
    let mut times = Vec::new();
    for _ in 0..1000 {
        let t = Instant::now();
        s.update(0, 0);
        times.push(t.elapsed().as_secs_f64() * 1000.);
    }
    times.sort_by(f64::total_cmp);
    println!(
        "{}",
        serde_json::json!({"center_x":cx,"villagers":s.villagers.len(),"warmup_ticks":100,"measured_ticks":1000,"p50_ms":times[500],"p95_ms":times[950],"p99_ms":times[990],"max_ms":times[999],"limits":"Local real GameState::update CPU time; excludes render, emulator, model and network time. Fixed seeds, six staged villagers."})
    );
    drop(s);
    std::fs::remove_dir_all(root)?;
    Ok(())
}
