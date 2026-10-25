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
    records.push(json!({"id":format!("action-{id}"),"title":title,"dimension":format!("{:?}",s.current_dimension),"category":"Actions & tools","origin":"Scripted gameplay","source":"tests/support/interaction_review.rs","caption":"Actual normal action commands, pixel renderer at three zooms. Scene and inventory staged in a disposable save.","images":images}));
    Ok(())
}
fn pty_fixture(
    root: &Path,
    address: &str,
    inventory: bool,
    furnace: bool,
) -> Result<(), Box<dyn std::error::Error>> {
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
    state.player.hunger = 16.0;
    if inventory {
        state.inventory.slots.fill(None);
        for (slot, item, count) in [
            (0, ItemType::Wood, 16),
            (1, ItemType::Cobblestone, 60),
            (2, ItemType::Cobblestone, 20),
            (3, ItemType::CookedBeef, 4),
            (9, ItemType::Coal, 4),
            (10, ItemType::Planks, 16),
            (11, ItemType::Stick, 4),
        ] {
            state.inventory.slots[slot] = Some(ItemStack {
                item_type: item,
                count,
                durability: None,
            });
        }
        state.player.hunger = 19.0;
    }
    if furnace {
        state.inventory.slots.fill(None);
        state.inventory.slots[0] = Some(ItemStack {
            item_type: ItemType::RawIron,
            count: 3,
            durability: None,
        });
        state.inventory.slots[1] = Some(ItemStack {
            item_type: ItemType::Coal,
            count: 2,
            durability: None,
        });
        state.player.x = 0.5;
        state.player.y = 40.0;
        state.player.grounded = true;
        state.world.load_chunks_for_spawn_search(0, 24);
        for x in -16..32 {
            for y in 0..=40 {
                state.world.set_block(
                    x,
                    y,
                    if y == 40 {
                        BlockType::Stone
                    } else {
                        BlockType::Air
                    },
                );
            }
        }
        state.world.set_block(2, 39, BlockType::Furnace);
    }
    state.try_persist_world_and_progress()?;
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
    if args
        .get(1)
        .is_some_and(|s| s == "--session-inspect" || s == "--furnace-inspect")
    {
        let mut state =
            GameState::open_in_directory(Path::new(args.get(2).ok_or("save root required")?))?;
        if args[1] == "--furnace-inspect" {
            state.player.x = 0.5;
            state.player.y = 40.0;
            state.inventory_open = false;
            state.apply_client_command(ClientCommand::UseAt(2, 39));
            assert!(state.at_furnace, "expected the fixture's placed furnace");
        }
        println!(
            "{}",
            serde_json::to_string(&termcraft::engine::state::PlayerSession::capture(&state))?
        );
        return Ok(());
    }
    if args.get(1).is_some_and(|s| {
        s == "--pty-fixture" || s == "--inventory-fixture" || s == "--furnace-fixture"
    }) {
        return pty_fixture(
            Path::new(args.get(2).ok_or("save root required")?),
            args.get(3).ok_or("address required")?,
            args[1] == "--inventory-fixture",
            args[1] == "--furnace-fixture",
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
    // Real contact and combat feedback, after startup invulnerability expires.
    for _ in 0..140 {
        s.update(0, 0);
    }
    hold(&mut s, ItemType::IronSword);
    s.player.x = 0.75;
    s.world.set_block(1, 40, BlockType::Sand);
    s.world.set_block(1, 39, BlockType::Cactus);
    capture(
        &s,
        &out,
        "cactus-before",
        "Cactus · before contact",
        &mut scenes,
    )?;
    let health = s.player.health;
    s.update(0, 0);
    assert!(s.player.health < health && s.player_hurt_ticks() > 5);
    capture(
        &s,
        &out,
        "cactus-hurt",
        "Cactus · actual contact damage and hurt flash",
        &mut scenes,
    )?;
    s.world.set_block(1, 39, BlockType::Air);
    s.world.set_block(1, 40, BlockType::Grass);
    s.player.x = 0.5;
    for _ in 0..12 {
        s.update(0, 0);
    }
    let mut ghast = termcraft::entities::ghast::Ghast::new(6.0, 40.0);
    ghast.shoot_cooldown = 100;
    s.ghasts.push(ghast);
    s.fireballs
        .push(termcraft::entities::fireball::Fireball::new(
            1.7, 39.1, -0.3, 0.0,
        ));
    capture(
        &s,
        &out,
        "fireball-incoming",
        "Fireball · incoming",
        &mut scenes,
    )?;
    s.apply_client_command(ClientCommand::SetPrimaryAction(true));
    s.update(1, 39);
    s.apply_client_command(ClientCommand::SetPrimaryAction(false));
    assert!(s.fireballs[0].reflected && s.fireballs[0].vx > 0.0);
    capture(
        &s,
        &out,
        "fireball-return",
        "Fireball · returned with normal melee",
        &mut scenes,
    )?;
    for _ in 0..8 {
        s.update(1, 39);
    }
    assert!(s.ghasts[0].health <= 10.0);
    capture(
        &s,
        &out,
        "fireball-hit",
        "Ghast · damaged by its returned projectile",
        &mut scenes,
    )?;
    s.ghasts.clear();
    s.fireballs.clear();
    for _ in 0..12 {
        s.update(0, 0);
    }
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
    for (item, id) in [
        (ItemType::IronSword, "sword"),
        (ItemType::IronAxe, "axe"),
        (ItemType::IronPickaxe, "pickaxe"),
    ] {
        for (right, face) in [(true, "right"), (false, "left")] {
            hold(&mut s, item);
            let tx = if right { 3 } else { -3 };
            s.apply_client_command(ClientCommand::SetPrimaryAction(true));
            s.update(tx, 38);
            s.apply_client_command(ClientCommand::SetPrimaryAction(false));
            let mut renderer = Renderer::with_size(160, 48);
            renderer.toggle_zoom();
            renderer.toggle_zoom();
            for tick in 0..=12 {
                let mut bytes = Vec::new();
                renderer.render_to(&s, 0.0, &mut bytes)?;
                fs::write(out.join(format!("rigid-{id}-{face}-{tick:02}.ansi")), bytes)?;
                s.update(tx, 38);
            }
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
    s.inventory.slots.fill(None);
    s.inventory.slots[0] = Some(ItemStack {
        item_type: ItemType::RawIron,
        count: 3,
        durability: None,
    });
    s.inventory.slots[1] = Some(ItemStack {
        item_type: ItemType::Coal,
        count: 2,
        durability: None,
    });
    s.hotbar_index = 2;
    s.world.set_block(2, 39, BlockType::Furnace);
    s.interact_block(2, 39, false);
    assert!(s.at_furnace);
    s.apply_client_command(ClientCommand::InventoryClick(0, 0));
    s.apply_client_command(ClientCommand::InventoryClick(60, 0));
    s.apply_client_command(ClientCommand::InventoryClick(1, 0));
    s.apply_client_command(ClientCommand::InventoryClick(61, 0));
    for _ in 0..40 {
        s.update(0, 0);
    }
    capture(
        &s,
        &out,
        "furnace-cooking",
        "Furnace · explicit input, fuel and output",
        &mut scenes,
    )?;
    let mut compact = Renderer::with_size(80, 24);
    let mut bytes = Vec::new();
    compact.render_to(&s, 0.0, &mut bytes)?;
    fs::write(out.join("furnace-compact-80x24.ansi"), bytes)?;
    for _ in 0..150 {
        s.update(0, 0);
    }
    capture(
        &s,
        &out,
        "furnace-result",
        "Furnace · completed output remains in its slot",
        &mut scenes,
    )?;
    s.apply_client_command(ClientCommand::ToggleInventory);
    // Environmental feedback uses actual fluid contact; breath is primed to
    // show the warning without a fifteen-second static capture lead-in.
    s.player.drowning_timer = 60;
    for x in -2..=2 {
        for y in 36..40 {
            s.world.set_block(x, y, BlockType::Water(8));
        }
    }
    s.update(0, 0);
    capture(
        &s,
        &out,
        "air-warning",
        "Air · actual underwater status (breath timer primed)",
        &mut scenes,
    )?;
    for x in -2..=2 {
        for y in 36..40 {
            s.world.set_block(x, y, BlockType::Air);
        }
    }
    s.player.drowning_timer = 300;
    s.cows.push(termcraft::entities::cow::Cow::new(8.5, 40.0));
    for x in 6..=10 {
        s.world.set_block(x, 39, BlockType::Lava(8));
    }
    for _ in 0..20 {
        s.update(0, 0);
    }
    assert!(
        s.cows
            .iter()
            .any(|e| e.hazards.burning > 0 && e.health < 10.0)
    );
    capture(
        &s,
        &out,
        "creature-lava",
        "Creature · actual lava contact and burning feedback",
        &mut scenes,
    )?;
    s.cows.clear();
    for x in 6..=10 {
        s.world.set_block(x, 39, BlockType::Air);
    }
    // Confirm held gear uses the same readable cave fill as the body.
    for x in -20..=20 {
        for y in 0..40 {
            s.world.set_block(
                x,
                y,
                if y <= 34 {
                    BlockType::Stone
                } else {
                    BlockType::Air
                },
            );
        }
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
