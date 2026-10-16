//! Exercise real action commands in a disposable room and capture all pixel zooms.
use serde_json::json;
use std::{fs, path::Path};
use termcraft::{
    engine::{command::ClientCommand, state::GameState},
    renderer::Renderer,
    world::{
        Dimension,
        block::BlockType,
        item::{ItemStack, ItemType},
    },
};
fn hold(s: &mut GameState, item: ItemType) {
    s.inventory.slots[0] = Some(ItemStack {
        item_type: item,
        count: 1,
        durability: item.max_durability(),
    });
    s.hotbar_index = 0;
}
fn capture(
    s: &GameState,
    out: &Path,
    id: &str,
    title: &str,
    records: &mut Vec<serde_json::Value>,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut images = Vec::new();
    for (zoom, view) in [(0, "overview"), (1, "wide"), (2, "detail")] {
        let mut r = Renderer::with_size(160, 48);
        for _ in 0..zoom {
            r.toggle_zoom();
        }
        let name = format!("{id}-{view}-160x48.ansi");
        let mut bytes = Vec::new();
        r.render_to(s, 0.0, &mut bytes)?;
        fs::write(out.join(&name), bytes)?;
        images.push(json!({"view":view,"ansi":name,"width":160,"height":48}));
    }
    records.push(json!({"id":format!("action-{id}"),"title":title,"dimension":format!("{:?}",s.current_dimension),"category":"Actions & tools","origin":"Scripted gameplay","source":"examples/interaction_review.rs","caption":"Actual normal action commands, pixel renderer at three zooms. Scene and inventory staged in a disposable save.","images":images}));
    Ok(())
}
fn pty_fixture(root: &Path, address: &str) -> Result<(), Box<dyn std::error::Error>> {
    use termcraft::{
        engine::{
            server_save::{ServerSave, token},
            state::PlayerSession,
        },
        world::recipe::{Terrain, WorldRecipe},
        worlds::WorldStore,
    };
    let mut recipe = WorldRecipe::example();
    recipe.terrain = Terrain::Plains;
    recipe.relief = 0;
    recipe.regions.clear();
    recipe.landmarks.clear();
    let path = WorldStore::new(root.join("worlds")).create(
        "terminal-input",
        "Disposable action input test",
        &recipe,
    )?;
    let mut state = GameState::open_in_directory(&path)?;
    state.dismiss_startup_splash();
    state.apply_client_command(ClientCommand::EquipDiamondLoadout);
    let mut saved = ServerSave::read(&state)?;
    let identity = token();
    saved
        .players
        .insert(identity.clone(), PlayerSession::capture(&state));
    saved
        .dimensions
        .insert(identity.clone(), Dimension::Overworld);
    saved.write(&path)?;
    let hash = address.bytes().fold(0xcbf29ce484222325u64, |h, c| {
        (h ^ u64::from(c)).wrapping_mul(0x100000001b3)
    });
    let clients = root.join("clients");
    fs::create_dir_all(&clients)?;
    fs::write(
        clients.join(format!("{hash:016x}-terminal-input-test.json")),
        serde_json::to_vec(
            &json!({"address":address,"profile":"terminal-input-test","server_id":saved.id,"token":identity}),
        )?,
    )?;
    state.world.try_save_all()?;
    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().collect();
    if args.get(1).is_some_and(|s| s == "--pty-fixture") {
        return pty_fixture(
            Path::new(args.get(2).ok_or("save root required")?),
            args.get(3).ok_or("address required")?,
        );
    }
    let out = std::path::PathBuf::from(std::env::args().nth(1).ok_or("output required")?);
    fs::create_dir_all(&out)?;
    let root = std::env::temp_dir().join(format!("termcraft-actions-{}", std::process::id()));
    fs::create_dir_all(&root)?;
    for dimension in ["overworld", "nether", "end"] {
        fs::write(
            root.join(format!("{dimension}_world_seed.bin")),
            bincode::serialize(&[1337u32, 7331, 9917, 4444])?,
        )?;
    }
    let mut s = GameState::open_in_directory(&root)?;
    s.dismiss_startup_splash();
    for cmd in [
        ClientCommand::ToggleRuleMobSpawning,
        ClientCommand::ToggleRuleDaylightCycle,
        ClientCommand::ToggleRuleWeatherCycle,
    ] {
        s.apply_client_command(cmd);
    }
    s.world.load_chunks_for_spawn_search(0, 48);
    for x in -30..=30 {
        for y in 0..50 {
            s.world.set_block(
                x,
                y,
                if y >= 40 {
                    BlockType::Grass
                } else {
                    BlockType::Air
                },
            );
        }
    }
    s.player.x = 0.5;
    s.player.y = 40.0;
    s.player.grounded = true;
    s.time_of_day = 12000.0;
    s.inventory.slots.fill(None);
    let mut scenes = Vec::new();
    for (item, id, title) in [
        (ItemType::IronSword, "sword", "Sword · swing into empty air"),
        (ItemType::IronPickaxe, "pickaxe", "Pickaxe · mining"),
        (ItemType::IronAxe, "axe", "Axe · swing"),
        (ItemType::IronShovel, "shovel", "Shovel · swing"),
        (ItemType::IronHoe, "hoe", "Hoe · tilling"),
    ] {
        hold(&mut s, item);
        if item == ItemType::IronPickaxe {
            s.world.set_block(3, 38, BlockType::Stone);
        }
        s.apply_client_command(ClientCommand::SetPrimaryAction(true));
        for _ in 0..4 {
            s.update(3, 38);
        }
        capture(&s, &out, id, title, &mut scenes)?;
        s.apply_client_command(ClientCommand::SetPrimaryAction(false));
        s.world.set_block(3, 38, BlockType::Air);
        for _ in 0..12 {
            s.update(3, 38);
        }
    }
    hold(&mut s, ItemType::Bow);
    s.inventory.add_item(ItemType::Arrow, 8);
    for (right, id) in [(true, "bow-right"), (false, "bow-left")] {
        let tx = if right { 8 } else { -8 };
        s.apply_client_command(ClientCommand::SetPrimaryAction(true));
        for _ in 0..20 {
            s.update(tx, 36);
        }
        capture(
            &s,
            &out,
            id,
            "Bow · fully drawn, aimed and nocked",
            &mut scenes,
        )?;
        s.apply_client_command(ClientCommand::SetPrimaryAction(false));
        s.update(tx, 36);
        capture(
            &s,
            &out,
            &format!("{id}-release"),
            "Bow · release and arrow",
            &mut scenes,
        )?;
        for _ in 0..12 {
            s.update(tx, 36);
        }
    }
    hold(&mut s, ItemType::Bread);
    s.player.hunger = 10.0;
    s.interact_block(100, 0, false);
    for _ in 0..4 {
        s.update(3, 38);
    }
    capture(
        &s,
        &out,
        "eating",
        "Eating · last bread remains visible",
        &mut scenes,
    )?;
    hold(&mut s, ItemType::WaterBucket);
    s.interact_block(2, 39, false);
    capture(
        &s,
        &out,
        "pouring",
        "Bucket · water source placement",
        &mut scenes,
    )?;
    hold(&mut s, ItemType::IronHoe);
    s.interact_block(3, 40, false);
    capture(&s, &out, "tilling", "Hoe · irrigated farmland", &mut scenes)?;
    hold(&mut s, ItemType::WheatSeeds);
    s.interact_block(3, 40, false);
    capture(
        &s,
        &out,
        "planting",
        "Seeds · click farmland to plant",
        &mut scenes,
    )?;
    assert_eq!(s.world.get_block(3, 39), BlockType::Crops(0));
    // A continuous renderer sequence exercises draw, release, swing, eat and use.
    let mut views = Vec::new();
    for (zoom, name) in [(0, "overview"), (1, "wide"), (2, "detail")] {
        let mut r = Renderer::with_size(160, 48);
        for _ in 0..zoom {
            r.toggle_zoom();
        }
        views.push((name, r));
    }
    for tick in 0..64 {
        match tick {
            0 => {
                hold(&mut s, ItemType::Bow);
                s.apply_client_command(ClientCommand::SetPrimaryAction(true));
            }
            20 => {
                s.apply_client_command(ClientCommand::SetPrimaryAction(false));
            }
            28 => {
                hold(&mut s, ItemType::IronSword);
                s.apply_client_command(ClientCommand::SetPrimaryAction(true));
            }
            38 => {
                s.apply_client_command(ClientCommand::SetPrimaryAction(false));
                hold(&mut s, ItemType::Bread);
                s.player.hunger = 10.0;
                s.interact_block(8, 36, false);
            }
            54 => {
                hold(&mut s, ItemType::WaterBucket);
                s.interact_block(-2, 39, false);
            }
            _ => {}
        }
        s.update(8, 36);
        for (view, r) in &mut views {
            let mut bytes = Vec::new();
            r.render_to(&s, 0.0, &mut bytes)?;
            fs::write(out.join(format!("motion-{view}-{tick:03}.ansi")), bytes)?;
        }
    }
    // Confirm held gear uses the same readable cave fill as the body.
    for x in -20..=20 {
        s.world.set_block(x, 34, BlockType::Stone);
    }
    s.world.set_block(-2, 39, BlockType::Torch);
    s.time_of_day = 22000.0;
    hold(&mut s, ItemType::Bow);
    s.apply_client_command(ClientCommand::SetPrimaryAction(true));
    for _ in 0..20 {
        s.update(8, 38);
    }
    capture(&s, &out, "cave-bow", "Bow · torch-lit cave", &mut scenes)?;
    s.apply_client_command(ClientCommand::SetPrimaryAction(false));
    s.update(8, 38);
    for cmd in [
        ClientCommand::TravelToNether,
        ClientCommand::TravelToEnd,
        ClientCommand::TravelToOverworld,
    ] {
        s.apply_client_command(cmd);
        let id = format!("travel-{:?}", s.current_dimension).to_lowercase();
        capture(
            &s,
            &out,
            &id,
            "Quick travel · same pixel renderer",
            &mut scenes,
        )?;
    }
    assert_eq!(s.current_dimension, Dimension::Overworld);
    fs::write(
        out.join("catalog.json"),
        serde_json::to_vec_pretty(
            &json!({"scenes":scenes,"checks":{"bow_fired":s.inventory.has_item(ItemType::Arrow,1) && !s.inventory.has_item(ItemType::Arrow,8),"farming_commands":true,"quick_travel":true}}),
        )?,
    )?;
    drop(s);
    fs::remove_dir_all(root)?;
    Ok(())
}
