//! Actual generated settlements at several sizes, plus a disposable native review save.
use std::{fs, path::Path};
use termcraft::{
    engine::{command::ClientCommand, state::GameState},
    entities::villager::Villager,
    renderer::Renderer,
    world::village::VillageSite,
};
fn capture(s: &GameState, out: &Path, name: &str, w: u16, h: u16) -> std::io::Result<()> {
    let mut r = Renderer::with_size(w, h);
    let mut bytes = Vec::new();
    r.render_to(s, 0.0, &mut bytes)?;
    fs::write(out.join(format!("{name}-{w}x{h}.ansi")), bytes)
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let out = std::path::PathBuf::from(std::env::args().nth(1).ok_or("output directory required")?);
    fs::create_dir_all(&out)?;
    let root = std::env::temp_dir().join(format!("tc-village-review-{}", rand::random::<u64>()));
    fs::create_dir_all(&root)?;
    let seeds = bincode::serialize(&[1337u32, 7331, 9917, 4444])?;
    for name in ["overworld", "nether", "end"] {
        fs::write(root.join(format!("{name}_world_seed.bin")), &seeds)?;
    }
    let mut s = GameState::open_in_directory(&root)?;
    s.dismiss_startup_splash();
    s.apply_client_command(ClientCommand::ToggleRuleMobSpawning);
    s.apply_client_command(ClientCommand::ToggleRuleDaylightCycle);
    let sites = s.world.village_sites_near(0, 10000);
    eprintln!(
        "accepted sites: {:?}",
        sites
            .iter()
            .map(|v| (v.center_x, v.ground_y, v.biome))
            .collect::<Vec<_>>()
    );
    let site: VillageSite = sites
        .iter()
        .filter(|v| v.center_x < 0)
        .min_by_key(|v| v.center_x.abs())
        .ok_or("no seed fixture village")?
        .clone();
    s.world.load_chunks_for_spawn_search(site.center_x, 100);
    let mut metadata = Vec::new();
    for (left, right, role) in site.buildings() {
        s.player.x = (left + right) as f64 / 2.0 + 0.5;
        s.player.y = site.ground_y as f64;
        s.player.grounded = true;
        s.villagers = vec![Villager::new(
            (left + 9) as f64 + 0.5,
            site.ground_y as f64,
            left,
            site.ground_y - 1,
        )];
        s.time_of_day = 8500.0;
        for (w, h) in [(80, 24), (120, 40), (160, 48)] {
            capture(&s, &out, &format!("{role:?}").to_lowercase(), w, h)?;
        }
        s.time_of_day = 23000.0;
        capture(&s, &out, &format!("{role:?}-night").to_lowercase(), 160, 48)?;
        metadata.push(serde_json::json!({"role":format!("{role:?}"),"left":left,"right":right,"ground_y":site.ground_y,"door_base_y":site.ground_y-1}));
    }
    s.time_of_day = 8500.0;
    s.player.x = site.center_x as f64;
    s.player.y = site.ground_y as f64;
    s.villagers = site
        .buildings()
        .iter()
        .map(|(l, _, _)| {
            Villager::new(
                (*l + 5) as f64 + 0.5,
                site.ground_y as f64,
                *l,
                site.ground_y - 1,
            )
        })
        .collect();
    capture(&s, &out, "settlement", 480, 32)?;
    capture(&s, &out, "square", 200, 40)?;
    s.player.x = (site.center_x + 39) as f64 + 0.5;
    s.player.y = site.ground_y as f64 - 0.5;
    capture(&s, &out, "garden", 160, 48)?;
    let mut detail = Renderer::with_size(160, 48);
    detail.toggle_zoom();
    detail.toggle_zoom();
    let mut bytes = Vec::new();
    detail.render_to(&s, 0.0, &mut bytes)?;
    fs::write(out.join("garden-detail-160x48.ansi"), bytes)?;
    if let Some(desert) = sites
        .iter()
        .find(|v| v.biome == termcraft::world::BiomeType::Desert)
    {
        s.world.load_chunks_for_spawn_search(desert.center_x, 100);
        s.player.y = desert.ground_y as f64;
        s.player.x = desert.center_x as f64;
        s.villagers.clear();
        capture(&s, &out, "desert-settlement", 480, 32)?;
        let (left, _, _) = desert.buildings()[0];
        s.player.x = (left + 5) as f64 + 0.5;
        capture(&s, &out, "desert-cottage", 160, 48)?;
    }
    s.player.y = site.ground_y as f64;
    s.player.x = (site.center_x - 45) as f64 + 0.5;
    s.villagers.clear();
    s.try_persist_world_and_progress()?;
    fs::write(
        out.join("village.json"),
        serde_json::to_vec_pretty(
            &serde_json::json!({"seed":[1337,7331,9917,4444],"center_x":site.center_x,"ground_y":site.ground_y,"biome":format!("{:?}",site.biome),"buildings":metadata,"irrigation":{"channel_x":site.center_x+39,"water_y":site.ground_y,"crossing_surface_y":site.ground_y as f64-0.5},"review_root":root,"staged_player_positions":true,"play":"Set TERMCRAFT_SAVE_DIR to review_root and run termcraft"}),
        )?,
    )?;
    println!("Native review save: {}", root.display());
    Ok(())
}
