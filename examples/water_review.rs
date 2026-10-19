//! Capture actual animal swimming and normal UseAt water placement in a staged pond.
use std::{fs, path::Path};
use termcraft::{
    engine::{command::ClientCommand, state::GameState},
    entities::cow::Cow,
    renderer::Renderer,
    world::{
        block::BlockType,
        chunk::{CHUNK_HEIGHT, CHUNK_WIDTH},
        item::{ItemStack, ItemType},
    },
};

fn pond(root: &Path) -> Result<GameState, Box<dyn std::error::Error>> {
    let mut s = GameState::open_in_directory(root)?;
    s.dismiss_startup_splash();
    for cx in -2..=2 {
        s.world
            .apply_chunk_column_snapshot(cx, &vec![BlockType::Air; CHUNK_WIDTH * CHUNK_HEIGHT]);
    }
    for x in -24..24 {
        for y in 0..24 {
            s.world.set_block(
                x,
                y,
                if y >= 20 || (y >= 15 && !(-4..=4).contains(&x)) {
                    if y == 15 {
                        BlockType::Grass
                    } else {
                        BlockType::Dirt
                    }
                } else if y >= 15 {
                    BlockType::Water(8)
                } else {
                    BlockType::Air
                },
            );
        }
    }
    for command in [
        ClientCommand::ToggleRuleMobSpawning,
        ClientCommand::ToggleRuleWeatherCycle,
        ClientCommand::ToggleRuleDaylightCycle,
    ] {
        s.apply_client_command(command);
    }
    s.world.newly_generated_chunks.clear();
    s.player.x = -6.5;
    s.player.y = 15.0;
    s.player.grounded = true;
    s.time_of_day = 12000.0;
    Ok(s)
}

fn draw(s: &GameState, r: &mut Renderer, out: &Path, name: &str) -> std::io::Result<()> {
    let mut bytes = Vec::new();
    r.render_to(s, 0.0, &mut bytes)?;
    fs::write(out.join(name), bytes)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let out = std::path::PathBuf::from(std::env::args().nth(1).ok_or("output directory required")?);
    fs::create_dir_all(&out)?;
    let root = std::env::temp_dir().join(format!("tc-water-review-{}", rand::random::<u64>()));
    fs::create_dir_all(&root)?;
    for dimension in ["overworld", "nether", "end"] {
        fs::write(
            root.join(format!("{dimension}_world_seed.bin")),
            bincode::serialize(&[1337_u32, 7331, 9917, 4444])?,
        )?;
    }
    let mut s = pond(&root)?;
    s.cows.push(Cow::new(0.5, 19.9));
    let initial_health = s.cows[0].health;
    let mut r = Renderer::with_size(160, 48);
    let mut history = Vec::new();
    let mut reached_air = false;
    let mut reached_bank = false;
    for tick in 0..=240 {
        let cow = s.cows.first().ok_or("cow died")?;
        reached_air |= !matches!(
            s.world.fluid_at_point(cow.x, cow.y - 1.375 + 0.25),
            BlockType::Water(_)
        );
        reached_bank |= cow.x.abs() > 5.6 && cow.y <= 15.01;
        assert_eq!(cow.health, initial_health);
        history.push(serde_json::json!({"tick":tick,"x":cow.x,"y":cow.y,"health":cow.health,"air":cow.hazards.air}));
        if tick % 2 == 0 {
            draw(&s, &mut r, &out, &format!("swim-{tick:03}.ansi"))?;
        }
        s.update(0, 0);
    }
    assert!(reached_air && reached_bank);
    let mut s = pond(&root)?;
    s.player.x = 1.5;
    s.player.y = 17.0;
    s.player.grounded = false;
    s.inventory.slots.fill(None);
    s.inventory.slots[0] = Some(ItemStack {
        item_type: ItemType::Cobblestone,
        count: 3,
        durability: None,
    });
    s.hotbar_index = 0;
    let mut r = Renderer::with_size(160, 48);
    r.toggle_zoom();
    r.toggle_zoom();
    draw(&s, &mut r, &out, "placement-before.ansi")?;
    s.apply_client_command(ClientCommand::UseAt(3, 16));
    assert_eq!(s.world.get_block(4, 16), BlockType::Cobblestone);
    assert_eq!(s.inventory.slots[0].as_ref().unwrap().count, 2);
    draw(&s, &mut r, &out, "placement-after.ansi")?;
    fs::write(
        out.join("water.json"),
        serde_json::to_vec_pretty(&serde_json::json!({
            "swimming":history, "reached_air":reached_air,"reached_bank":reached_bank,
            "placement":{"command":{"UseAt":[3,16]},"placed":[4,16],"remaining":2},
            "columns":160,"rows":48,"frame_interval_ms":100,
            "limits":"Staged pond and cow; actual normal UseAt command, engine ticks and terminal renderer. Not a manual Warp session."
        }))?,
    )?;
    drop(s);
    fs::remove_dir_all(root)?;
    Ok(())
}
