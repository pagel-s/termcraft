//! Real placement, ignition, save/reload and portal travel in disposable saves.
use serde_json::{Value, json};
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

fn capture(
    out: &Path,
    s: &GameState,
    id: &str,
    title: &str,
    caption: &str,
) -> Result<Value, Box<dyn std::error::Error>> {
    let mut images = Vec::new();
    for (zoom, view) in [(0, "overview"), (1, "wide"), (2, "detail")] {
        let mut r = Renderer::with_size(160, 48);
        for _ in 0..zoom {
            r.toggle_zoom();
        }
        let mut ansi = Vec::new();
        r.render_to(s, 0.0, &mut ansi)?;
        let name = format!("{id}-{view}-160x48.ansi");
        fs::write(out.join(&name), ansi)?;
        images.push(json!({"view":view,"ansi":name,"width":160,"height":48}));
    }
    Ok(
        json!({"id":id,"title":title,"caption":caption,"category":"Play scenarios",
        "dimension":format!("{:?}",s.current_dimension),"origin":"Scripted gameplay",
        "source":"examples/portal_review.rs","x":s.player.x,"y":s.player.y,"images":images}),
    )
}

fn room(root: &Path) -> Result<GameState, Box<dyn std::error::Error>> {
    fs::create_dir_all(root)?;
    let seeds = bincode::serialize(&[1337u32, 7331, 9917, 4444])?;
    for d in ["overworld", "nether", "end"] {
        fs::write(root.join(format!("{d}_world_seed.bin")), &seeds)?;
    }
    let mut s = GameState::open_in_directory(root)?;
    s.dismiss_startup_splash();
    s.apply_client_command(ClientCommand::ToggleRuleMobSpawning);
    s.apply_client_command(ClientCommand::ToggleRuleDaylightCycle);
    s.apply_client_command(ClientCommand::ToggleRuleWeatherCycle);
    s.time_of_day = 0.0;
    s.world.load_chunks_for_spawn_search(0, 64);
    for x in -18..=18 {
        for y in 24..44 {
            s.world.set_block(
                x,
                y,
                if y >= 36 || y == 30 || ((x == -12 || x == 12) && y > 30) {
                    BlockType::StoneBricks
                } else {
                    BlockType::Air
                },
            );
        }
    }
    s.player.x = 0.5;
    s.player.y = 36.0;
    s.player.grounded = true;
    s.inventory.slots.fill(None);
    s.hotbar_index = 0;
    Ok(s)
}

fn hold(s: &mut GameState, item_type: ItemType, count: u32) {
    s.inventory.slots[0] = Some(ItemStack {
        item_type,
        count,
        durability: item_type.max_durability(),
    });
    s.hotbar_index = 0;
}

fn portal(s: &GameState) -> (i32, i32) {
    for x in -24..=24 {
        for y in 0..128 {
            if let Some(anchor) = s.world.nether_portal_anchor_at(x, y) {
                return anchor;
            }
        }
    }
    panic!("no complete portal in the destination");
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    use BlockType::*;
    let out = std::path::PathBuf::from(std::env::args().nth(1).ok_or("output directory required")?);
    fs::create_dir_all(&out)?;
    let root = std::env::temp_dir().join(format!(
        "termcraft-portal-review-{}-{}",
        std::process::id(),
        rand::random::<u64>()
    ));
    let nether_root = root.join("nether-trip");
    let mut s = room(&nether_root)?;
    let mut scenes = Vec::new();
    scenes.push(capture(
        &out,
        &s,
        "glowstone-dark",
        "Glowstone · before placement",
        "A roofed, unlit review room at night. Compare the next scene at the same zoom.",
    )?);
    hold(&mut s, ItemType::Glowstone, 1);
    s.apply_client_command(ClientCommand::UseAt(2, 31));
    assert_eq!(s.world.get_block(2, 31), Glowstone);
    assert!(s.inventory.slots[0].is_none());
    scenes.push(capture(&out, &s, "glowstone-lit", "Glowstone · placed and lighting the room", "Glowstone placed through the ordinary use command under a supported ceiling. The warm light comes from the live renderer.")?);
    for x in 6..=9 {
        for y in 32..=36 {
            if x == 6 || x == 9 || y == 32 || y == 36 {
                s.world.set_block(x, y, Obsidian);
            }
        }
    }
    s.world.set_block(3, 35, Chest);
    s.world
        .ensure_chest_inventory(3, 35, 27)
        .unwrap()
        .add_item(ItemType::Diamond, 3);
    s.player.x = 5.5;
    hold(&mut s, ItemType::FlintAndSteel, 1);
    s.apply_client_command(ClientCommand::UseAt(6, 35));
    assert_eq!(s.world.nether_portal_anchor_at(8, 34), Some((7, 36)));
    scenes.push(capture(&out, &s, "portal-ignition", "Nether portal · actual ignition", "A staged obsidian frame ignited with Flint and Steel through the real use command; six active tiles, not a painted portal.")?);
    s.player.fall_distance = 100.0;
    s.apply_client_command(ClientCommand::UseAt(7, 34));
    assert_eq!(s.current_dimension, Dimension::Nether);
    assert_eq!(s.player.fall_distance, 0.0);
    scenes.push(capture(&out, &s, "portal-nether-arrival", "Nether portal · destination", "Actual portal travel generated and linked this arrival portal in the Nether. No travel shortcut was used.")?);
    s.world.try_save_all()?;
    s.save_progression();
    drop(s);
    let mut s = GameState::open_in_directory(&nether_root)?;
    s.dismiss_startup_splash();
    assert_eq!(s.current_dimension, Dimension::Nether);
    let (x, y) = portal(&s);
    for _ in 0..42 {
        s.update(x, y - 2);
    }
    s.player.x = x as f64 + 0.5;
    s.player.y = y as f64;
    s.apply_client_command(ClientCommand::UseAt(x, y - 2));
    assert_eq!(s.current_dimension, Dimension::Overworld);
    assert!(
        (s.player.y - 35.9).abs() < 0.01,
        "returned to a different portal height"
    );
    let mut anchors = std::collections::BTreeSet::new();
    for x in -18..=18 {
        for y in 0..128 {
            if let Some(anchor) = s.world.nether_portal_anchor_at(x, y) {
                anchors.insert(anchor);
            }
        }
    }
    assert_eq!(
        anchors.into_iter().collect::<Vec<_>>(),
        vec![(7, 36)],
        "return created a duplicate portal"
    );
    assert_eq!(s.world.get_block(2, 31), Glowstone);
    assert_eq!(s.world.get_block(3, 35), Chest);
    assert_eq!(
        s.world.chest_inventory(3, 35).unwrap().slots[0]
            .as_ref()
            .unwrap()
            .count,
        3
    );
    scenes.push(capture(&out, &s, "portal-return-workshop", "Nether portal · workshop preserved", "Return through the linked portal after saving and reopening in the Nether. Glowstone, the roof, the chest and its three diamonds survive the round trip.")?);
    s.world.set_block(6, 34, Air);
    for x in 7..=8 {
        for y in 33..=35 {
            assert_eq!(s.world.get_block(x, y), Air);
        }
    }
    scenes.push(capture(&out, &s, "portal-extinguished", "Nether portal · broken frame extinguished", "One side of the frame was removed through the world-edit path. All six portal tiles extinguished together; the glowstone still lights the room.")?);
    drop(s);

    let mut s = room(&root.join("end-trip"))?;
    for x in -1..=2 {
        for y in 32..=35 {
            if x == -1 || x == 2 || y == 32 || y == 35 {
                s.world.set_block(x, y, EndPortalFrame { filled: false });
            }
        }
    }
    s.player.x = 0.5;
    s.player.y = 35.0;
    hold(&mut s, ItemType::EyeOfEnder, 12);
    for x in -1..=2 {
        for y in 32..=35 {
            if x == -1 || x == 2 || y == 32 || y == 35 {
                s.apply_client_command(ClientCommand::UseAt(x, y));
            }
        }
    }
    assert!(s.inventory.slots[0].is_none());
    for x in 0..=1 {
        for y in 33..=34 {
            assert_eq!(s.world.get_block(x, y), EndPortal);
        }
    }
    scenes.push(capture(&out, &s, "portal-end-activation", "End portal · twelve Eyes of Ender", "A staged empty frame filled through twelve real use commands. The game consumed the Eyes and activated the complete opening.")?);
    s.apply_client_command(ClientCommand::UseAt(0, 34));
    assert_eq!(s.current_dimension, Dimension::End);
    scenes.push(capture(&out, &s, "portal-end-arrival", "End portal · safe arrival", "Actual travel through the activated End portal, landing on the generated island with cleared headroom. This capture precedes the dragon encounter update.")?);
    // Wait for the normal cooldown on the arrival pad, then use the generated return portal.
    for _ in 0..42 {
        s.update(0, 32);
    }
    s.player.x = 0.5;
    s.player.y = 34.0;
    s.apply_client_command(ClientCommand::UseAt(0, 32));
    assert_eq!(s.current_dimension, Dimension::Overworld);
    scenes.push(capture(&out, &s, "portal-end-return", "End portal · stronghold return", "Actual interaction with the generated bedrock return portal, returning to the Overworld stronghold. The boss was not defeated in this scripted travel check.")?);
    fs::write(
        out.join("catalog.json"),
        serde_json::to_vec_pretty(&json!({"scenes":scenes,"checks":{
        "glowstone_placed":true,"flint_and_steel_ignition":true,"nether_travel":true,
            "save_reload_roundtrip":true,"original_portal_reused":true,"workshop_and_chest_contents_preserved":true,
        "fall_distance_reset":true,"broken_frame_extinguished":true,"twelve_eyes_consumed":true,
        "end_travel_and_return":true}}))?,
    )?;
    drop(s);
    fs::remove_dir_all(root)?;
    println!(
        "Portal and glowstone checks passed; {} native scenes",
        scenes.len()
    );
    Ok(())
}
