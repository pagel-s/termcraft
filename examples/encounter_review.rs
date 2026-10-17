//! Actual generated terrain and engine-driven dragon flight in disposable saves.
//! Player positions and invulnerability are staged; dragon motion is simulated.
use std::{fs, path::Path};
use termcraft::{
    engine::{command::ClientCommand, state::GameState},
    renderer::Renderer,
    world::{Dimension, World, block::BlockType},
};

fn pty_fixture(
    root: &Path,
    address: &str,
    dimension: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    use termcraft::{
        engine::{
            server_save::{ServerSave, token},
            state::PlayerSession,
        },
        world::recipe::WorldRecipe,
        worlds::WorldStore,
    };
    let mut recipe = WorldRecipe::example();
    recipe.regions.clear();
    recipe.landmarks.clear();
    let path = WorldStore::new(root.join("worlds")).create(
        "terminal-input",
        "Dimension terminal check",
        &recipe,
    )?;
    for name in ["overworld", "nether", "end"] {
        fs::write(
            path.join(format!("{name}_world_seed.bin")),
            bincode::serialize(&[1337u32, 7331, 9917, 4444])?,
        )?;
    }
    let mut s = GameState::open_in_directory(&path)?;
    s.dismiss_startup_splash();
    s.apply_client_command(ClientCommand::EquipDiamondLoadout);
    s.apply_client_command(match dimension {
        "Nether" => ClientCommand::TravelToNether,
        "End" => ClientCommand::TravelToEnd,
        _ => return Err("fixture dimension must be Nether or End".into()),
    });
    s.try_persist_world_and_progress()?;
    let mut saved = ServerSave::read(&s)?;
    let identity = token();
    saved
        .players
        .insert(identity.clone(), PlayerSession::capture(&s));
    saved
        .dimensions
        .insert(identity.clone(), s.current_dimension);
    saved.write(&path)?;
    let hash = address.bytes().fold(0xcbf29ce484222325u64, |h, c| {
        (h ^ u64::from(c)).wrapping_mul(0x100000001b3)
    });
    let clients = root.join("clients");
    fs::create_dir_all(&clients)?;
    fs::write(
        clients.join(format!("{hash:016x}-terminal-input-test.json")),
        serde_json::to_vec(
            &serde_json::json!({"address":address,"profile":"terminal-input-test","server_id":saved.id,"token":identity}),
        )?,
    )?;
    Ok(())
}

fn capture(s: &GameState, out: &Path, name: &str) -> std::io::Result<()> {
    for (w, h, zoom) in [(120, 40, 0), (160, 48, 0), (200, 60, 1), (160, 48, 2)] {
        let mut r = Renderer::with_size(w, h);
        for _ in 0..zoom {
            r.toggle_zoom();
        }
        let mut bytes = Vec::new();
        r.render_to(s, 0.0, &mut bytes)?;
        fs::write(out.join(format!("{name}-z{zoom}-{w}x{h}.ansi")), bytes)?;
    }
    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().collect();
    if args.get(1).is_some_and(|v| v == "--pty-fixture") {
        return pty_fixture(
            Path::new(args.get(2).ok_or("root required")?),
            args.get(3).ok_or("address required")?,
            args.get(4).ok_or("dimension required")?,
        );
    }
    let covered = args.iter().any(|arg| arg == "--covered");
    let out = std::path::PathBuf::from(std::env::args().nth(1).ok_or("output directory required")?);
    fs::create_dir_all(&out)?;
    let root = std::env::temp_dir().join(format!("tc-encounter-{}", rand::random::<u64>()));
    fs::create_dir(&root)?;
    let seeds = bincode::serialize(&[1337u32, 7331, 9917, 4444])?;
    for name in ["overworld", "nether", "end"] {
        fs::write(root.join(format!("{name}_world_seed.bin")), &seeds)?;
    }
    let mut s = GameState::open_in_directory(&root)?;
    s.dismiss_startup_splash();
    s.apply_client_command(ClientCommand::ToggleRuleMobSpawning);
    s.current_dimension = Dimension::End;
    s.world = World::open_in_directory(Dimension::End, &root)?;
    s.world.load_chunks_for_spawn_search(0, 100);
    let anchor_x: f64 = if covered { 14.5 } else { 6.5 };
    s.player.x = anchor_x;
    s.player.y = (29..40)
        .find(|&y| {
            s.world.get_block(anchor_x.floor() as i32, y).is_solid()
                && s.world.get_block(anchor_x.floor() as i32, y - 1) == BlockType::Air
                && s.world.get_block(anchor_x.floor() as i32, y - 2) == BlockType::Air
        })
        .unwrap() as f64;
    s.player.grounded = true;
    let anchor_y = s.player.y;
    if args.iter().any(|arg| arg == "--victory") {
        s.update(0, 0);
        s.end_crystals.clear();
        let dragon = s.ender_dragon.as_mut().ok_or("dragon missing")?;
        dragon.x = 2.5;
        dragon.y = 27.0;
        dragon.health = 0.0;
        s.update(0, 0); // Actual boss-death path starts the shared sequence/rewards.
        let mut renderer = Renderer::with_size(160, 48);
        for tick in 0..=140 {
            let mut bytes = Vec::new();
            renderer.render_to(&s, 0.0, &mut bytes)?;
            fs::write(out.join(format!("victory-{tick:03}-160x48.ansi")), bytes)?;
            if [0, 18, 36, 60, 90, 120, 140].contains(&tick) {
                capture(&s, &out, &format!("finale-{tick:03}"))?;
            }
            s.player.health = 20.0;
            s.update(0, 0);
        }
        fs::write(
            out.join("victory.json"),
            serde_json::to_vec_pretty(&serde_json::json!({
                "frames":141,"tick_ms":50,"dragon_defeated":s.has_defeated_dragon(),
                "sequence_finished":s.end_victory_sequence_state().is_none(),
                "limits":"Actual engine boss-death path and unmodified terminal renderer. Kill and player position staged in a disposable seed-1337 world; excludes live human combat/emulator timing."
            }))?,
        )?;
        fs::remove_dir_all(root)?;
        return Ok(());
    }
    let mut trajectory = Vec::new();
    let mut tick_us = Vec::new();
    let mut contact_ticks = 0;
    let mut renderer = Renderer::with_size(200, 60);
    renderer.toggle_zoom(); // Wide view holds the whole generated arena.
    for tick in 0..2400 {
        s.player.x = anchor_x;
        s.player.y = anchor_y;
        s.player.vx = 0.0;
        s.player.vy = 0.0;
        s.player.grounded = true;
        s.player.health = 20.0;
        s.player.hunger = 20.0;
        let start = std::time::Instant::now();
        s.update(0, 0);
        tick_us.push(start.elapsed().as_secs_f64() * 1_000_000.0);
        contact_ticks += usize::from(s.player.health < 20.0);
        if tick % 10 == 0 {
            let d = s.ender_dragon.as_ref().ok_or("dragon missing")?;
            trajectory.push(serde_json::json!({"tick":tick,"x":d.x,"y":d.y,"vx":d.vx,"vy":d.vy,"phase":format!("{:?}",d.phase)}));
        }
        if [0, 240, 480, 960, 1440, 1920].contains(&tick) {
            capture(&s, &out, &format!("end-{tick:04}"))?;
        }
        if tick % 4 == 0 && tick < 800 {
            let mut bytes = Vec::new();
            renderer.render_to(&s, 0.0, &mut bytes)?;
            fs::write(
                out.join(format!("flight-{:03}-200x60.ansi", tick / 4)),
                bytes,
            )?;
        }
    }
    fs::write(
        out.join("trajectory.json"),
        serde_json::to_vec_pretty(&trajectory)?,
    )?;
    tick_us.sort_by(f64::total_cmp);
    fs::write(
        out.join("simulation.json"),
        serde_json::to_vec_pretty(&serde_json::json!({
            "ticks":tick_us.len(),"contact_ticks":contact_ticks,"player_anchor":[anchor_x,anchor_y],"covered_target":covered,"p50_tick_us":tick_us[tick_us.len()/2],"p95_tick_us":tick_us[tick_us.len()*95/100],"max_tick_us":tick_us.last(),
            "limits":"Real generated arena and engine ticks. Fixed invulnerable player; no terminal emulator, input latency or human fight qualification."
        }))?,
    )?;

    s.current_dimension = Dimension::Nether;
    s.ender_dragon = None;
    s.end_crystals.clear();
    s.world = World::open_in_directory(Dimension::Nether, &root)?;
    let mut positions = Vec::new();
    for target_x in [0, 48, 96, 176, 256, -48, -144] {
        let x = target_x;
        s.world.load_chunks_for_spawn_search(x, 64);
        let (x, y) = (x - 8..=x + 8)
            .flat_map(|x| (35..92).map(move |y| (x, y)))
            .filter(|&(x, y)| {
                s.world.get_block(x, y).is_solid()
                    && s.world.get_block(x, y - 1) == BlockType::Air
                    && s.world.get_block(x, y - 2) == BlockType::Air
            })
            .min_by_key(|&(x, y)| (x - target_x).abs() * 100 + (y - 60).abs())
            .ok_or("no nether floor")?;
        s.player.x = x as f64 + 0.5;
        s.player.y = y as f64;
        s.player.vx = 0.0;
        s.player.vy = 0.0;
        s.player.grounded = true;
        capture(&s, &out, &format!("nether-{target_x}"))?;
        positions.push(serde_json::json!({"scene":target_x,"x":s.player.x,"y":s.player.y}));
    }
    fs::write(
        out.join("nether-positions.json"),
        serde_json::to_vec_pretty(&positions)?,
    )?;
    drop(s);
    fs::remove_dir_all(root)?;
    Ok(())
}
