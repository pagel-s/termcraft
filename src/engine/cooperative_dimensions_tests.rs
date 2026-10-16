use super::cooperative_tests::{command, count, fixture};
use super::*;
use crate::world::item::ItemStack;

fn light_portal(sim: &mut ServerSimulation, id: u16) {
    sim.state.activate_peer(id);
    sim.state.player.x = 0.5;
    sim.state.player.y = 80.0;
    for x in -1..=2 {
        for y in 76..=80 {
            sim.state.world.set_block(
                x,
                y,
                if x == -1 || x == 2 || y == 76 || y == 80 {
                    BlockType::Obsidian
                } else {
                    BlockType::Air
                },
            );
        }
    }
    sim.state.inventory.slots[0] = Some(ItemStack {
        item_type: ItemType::FlintAndSteel,
        count: 1,
        durability: Some(65),
    });
    command(sim, id, ClientCommand::SelectHotbarSlot(0));
    command(sim, id, ClientCommand::UseAt(-1, 78));
    assert_eq!(
        sim.state.world.nether_portal_anchor_at(0, 78),
        Some((0, 80))
    );
}

#[test]
fn portal_travel_keeps_other_players_terrain_and_entities_in_their_dimension() {
    let (_root, mut sim) = fixture();
    light_portal(&mut sim, 1);
    sim.state.world.set_block(6, 79, BlockType::Glowstone);
    sim.state
        .cows
        .push(crate::entities::cow::Cow::new(12.5, 80.0));
    sim.state.inventory.add_item(ItemType::Diamond, 3);
    command(&mut sim, 1, ClientCommand::UseAt(0, 78));
    assert_eq!(sim.state.peer_dimension(1), Dimension::Nether);
    assert_eq!(sim.state.peer_dimension(2), Dimension::Overworld);
    let nether = sim.snapshot_for_client(1).unwrap();
    assert_eq!(nether.dimension, SnapshotDimension::Nether);
    assert!(nether.remote_players.is_empty());
    assert!(sim.state.cows.is_empty());
    let mut view = sim.client_view(2).unwrap();
    assert_eq!(
        sim.state.current_dimension,
        Dimension::Nether,
        "observing another player changed the selected simulation"
    );
    assert_eq!(view.current_dimension, Dimension::Overworld);
    assert_eq!(view.world.get_block(6, 79), BlockType::Glowstone);
    view.world.set_block(6, 79, BlockType::Air);
    assert_eq!(
        sim.client_view(2).unwrap().world.get_block(6, 79),
        BlockType::Glowstone,
        "replica edits leaked into the server"
    );
    let overworld = sim.snapshot_for_client(2).unwrap();
    assert_eq!(overworld.dimension, SnapshotDimension::Overworld);
    assert!(overworld.remote_players.is_empty());
    sim.state.activate_peer(2);
    assert_eq!(sim.state.world.get_block(6, 79), BlockType::Glowstone);
    assert_eq!(sim.state.cows.len(), 1);
    assert_eq!(count(&sim, 1, ItemType::Diamond), 3);
    let cow_age = sim.state.cows[0].age;
    let ages: Vec<_> = [1, 2]
        .map(|id| sim.state.peer_session(id).unwrap().player().age)
        .to_vec();
    sim.step(0, 0);
    for (i, id) in [1, 2].into_iter().enumerate() {
        assert_eq!(
            sim.state.peer_session(id).unwrap().player().age,
            ages[i] + 1
        );
    }
    sim.state.activate_peer(2);
    assert_eq!(sim.state.cows[0].age, cow_age + 1);
    command(&mut sim, 2, ClientCommand::UseAt(0, 78));
    assert_eq!(sim.state.peer_dimension(2), Dimension::Nether);
    let together = sim.snapshot_for_client(2).unwrap();
    assert_eq!(together.remote_players.len(), 1);
    assert_eq!(together.remote_players[0].client_id, 1);
    sim.register_client(3);
    assert_eq!(
        sim.state.peer_dimension(3),
        Dimension::Overworld,
        "new players should join the Overworld even while the host is in the Nether"
    );
}

#[test]
fn players_reconnect_and_restart_in_their_own_dimensions_with_portal_links() {
    let (_root, mut sim) = fixture();
    sim.unregister_client(1);
    sim.unregister_client(2);
    sim.load_persistent().unwrap();
    let a = sim.authenticate(3, None).unwrap();
    let b = sim.authenticate(4, None).unwrap();
    light_portal(&mut sim, 3);
    sim.state.world.set_block(6, 79, BlockType::Glowstone);
    command(&mut sim, 3, ClientCommand::UseAt(0, 78));
    sim.state.inventory.add_item(ItemType::Diamond, 3);
    let path = sim.state.world.save_root().to_path_buf();
    sim.save().unwrap();
    sim.unregister_client(3);
    sim.authenticate(30, Some(&a.token)).unwrap();
    assert_eq!(
        sim.snapshot_for_client(30).unwrap().dimension,
        SnapshotDimension::Nether
    );
    sim.save().unwrap();
    drop(sim);
    let mut restarted = ServerSimulation::with_state(GameState::open_in_directory(&path).unwrap());
    restarted.load_persistent().unwrap();
    restarted.authenticate(31, Some(&a.token)).unwrap();
    restarted.authenticate(41, Some(&b.token)).unwrap();
    assert_eq!(
        restarted.snapshot_for_client(31).unwrap().dimension,
        SnapshotDimension::Nether
    );
    assert_eq!(
        restarted.snapshot_for_client(41).unwrap().dimension,
        SnapshotDimension::Overworld
    );
    assert_eq!(count(&restarted, 31, ItemType::Diamond), 3);
    for _ in 0..42 {
        restarted.step(0, 0);
    }
    restarted.state.activate_peer(31);
    let mut anchor = None;
    for x in -16..=16 {
        for y in 0..128 {
            if let Some(p) = restarted.state.world.nether_portal_anchor_at(x, y) {
                anchor = Some(p);
            }
        }
    }
    let (x, y) = anchor.expect("saved destination frame");
    restarted.state.player.x = x as f64 + 0.5;
    restarted.state.player.y = y as f64;
    command(&mut restarted, 31, ClientCommand::UseAt(x, y - 2));
    assert_eq!(restarted.state.peer_dimension(31), Dimension::Overworld);
    assert!((restarted.state.peer_session(31).unwrap().player().y - 79.9).abs() < 0.2);
    assert_eq!(restarted.state.world.get_block(6, 79), BlockType::Glowstone);
    assert_eq!(
        restarted
            .snapshot_for_client(31)
            .unwrap()
            .remote_players
            .len(),
        1
    );
}

#[test]
fn dimension_local_explosions_do_not_damage_an_actor_at_matching_coordinates_elsewhere() {
    let (_root, mut sim) = fixture();
    light_portal(&mut sim, 1);
    command(&mut sim, 1, ClientCommand::UseAt(0, 78));
    let nether = sim.snapshot_for_client(1).unwrap().player;
    sim.state.activate_peer(2);
    sim.state.player.x = nether.x;
    sim.state.player.y = nether.y;
    sim.state.world.set_block(
        nether.x.floor() as i32,
        nether.y.floor() as i32 - 1,
        BlockType::PrimedTnt(1),
    );
    let before = sim.snapshot_for_client(1).unwrap().player.health;
    for _ in 0..4 {
        sim.step(0, 0);
    }
    assert_eq!(sim.snapshot_for_client(1).unwrap().player.health, before);
    assert!(sim.snapshot_for_client(2).unwrap().player.health < 20.0);
}

#[test]
fn stale_overworld_actions_are_rejected_after_travel_but_releases_still_apply() {
    let (_root, mut sim) = fixture();
    light_portal(&mut sim, 1);
    command(&mut sim, 1, ClientCommand::UseAt(0, 78));
    sim.state.activate_peer(1);
    let (x, y) = (
        sim.state.player.x.floor() as i32 + 1,
        sim.state.player.y.ceil() as i32 - 1,
    );
    sim.state.world.set_block(x, y, BlockType::Air);
    sim.state.world.set_block(x, y + 1, BlockType::Stone);
    sim.state.inventory.slots[0] = Some(ItemStack {
        item_type: ItemType::Glowstone,
        count: 2,
        durability: None,
    });
    let sequence = sim.last_applied_sequence_by_client[&1] + 1;
    sim.enqueue_input(ClientInputFrame::new(
        1,
        sequence,
        sim.tick() + 1,
        ClientCommand::UseAt(x, y),
    ));
    sim.step(0, 0);
    assert_eq!(
        count(&sim, 1, ItemType::Glowstone),
        2,
        "stale source-world click placed a block"
    );
    command(&mut sim, 1, ClientCommand::UseAt(x, y));
    assert_eq!(count(&sim, 1, ItemType::Glowstone), 1);
    command(&mut sim, 1, ClientCommand::SetMoveRight(true));
    let sequence = sim.last_applied_sequence_by_client[&1] + 1;
    sim.enqueue_input(ClientInputFrame::new(
        1,
        sequence,
        sim.tick() + 1,
        ClientCommand::SetMoveRight(false),
    ));
    sim.step(0, 0);
    assert!(sim.snapshot_for_client(1).unwrap().player.vx.abs() < 0.1);
}

#[test]
fn shared_authoring_saves_do_not_replace_the_native_player_checkpoint() {
    let (_root, mut sim) = fixture();
    let path = sim.state.world.save_root().join("player_progression.bin");
    let original = std::fs::read(&path).ok();
    sim.state.inventory.add_item(ItemType::Diamond, 7);
    sim.state.try_persist_world_and_progress().unwrap();
    assert_eq!(std::fs::read(path).ok(), original);
}

#[test]
fn death_in_the_nether_respawns_only_that_player_in_the_overworld() {
    let (_root, mut sim) = fixture();
    light_portal(&mut sim, 1);
    command(&mut sim, 1, ClientCommand::UseAt(0, 78));
    command(&mut sim, 2, ClientCommand::UseAt(0, 78));
    sim.state.activate_peer(1);
    sim.state.player.health = 0.0;
    for _ in 0..60 {
        sim.step(0, 0);
    }
    assert!(sim.snapshot_for_client(1).unwrap().death_screen_active);
    command(&mut sim, 1, ClientCommand::RespawnFromDeathScreen);
    let respawned = sim.snapshot_for_client(1).unwrap();
    assert_eq!(respawned.dimension, SnapshotDimension::Overworld);
    assert!(!respawned.death_screen_active);
    assert_eq!(respawned.player.health, respawned.player.max_health);
    assert_eq!(
        sim.snapshot_for_client(2).unwrap().dimension,
        SnapshotDimension::Nether
    );
}

#[test]
fn end_encounter_is_shared_and_credits_do_not_freeze_or_teleport_the_other_player() {
    let (_root, mut sim) = fixture();
    sim.state.activate_peer(1);
    for x in -1..=2 {
        for y in 76..=79 {
            sim.state.world.set_block(
                x,
                y,
                if x == -1 || x == 2 || y == 76 || y == 79 {
                    BlockType::EndPortalFrame { filled: false }
                } else {
                    BlockType::Air
                },
            );
        }
    }
    sim.state.player.x = 0.5;
    sim.state.player.y = 79.0;
    sim.state.inventory.slots[0] = Some(ItemStack {
        item_type: ItemType::EyeOfEnder,
        count: 12,
        durability: None,
    });
    for x in -1..=2 {
        for y in 76..=79 {
            if x == -1 || x == 2 || y == 76 || y == 79 {
                command(&mut sim, 1, ClientCommand::UseAt(x, y));
            }
        }
    }
    assert_eq!(count(&sim, 1, ItemType::EyeOfEnder), 0);
    command(&mut sim, 1, ClientCommand::UseAt(0, 78));
    assert_eq!(sim.state.peer_dimension(1), Dimension::End);
    sim.state.ender_dragon.as_mut().unwrap().health = 150.0;
    command(&mut sim, 2, ClientCommand::UseAt(0, 78));
    assert_eq!(sim.state.peer_dimension(2), Dimension::End);
    assert!(
        sim.state.ender_dragon.as_ref().unwrap().health < 160.0,
        "second arrival respawned the dragon"
    );
    sim.state.ender_dragon.as_mut().unwrap().health = 0.0;
    sim.step(0, 0);
    assert!(sim.state.ender_dragon.is_none());
    for _ in 0..42 {
        sim.step(0, 0);
    }
    sim.state.activate_peer(1);
    sim.state.player.x = 0.5;
    sim.state.player.y = 34.0;
    command(&mut sim, 1, ClientCommand::UseAt(0, 32));
    assert!(sim.snapshot_for_client(1).unwrap().credits_active);
    let age = sim.state.peer_session(2).unwrap().player().age;
    sim.step(0, 0);
    assert_eq!(sim.state.peer_session(2).unwrap().player().age, age + 1);
    assert!(!sim.snapshot_for_client(2).unwrap().credits_active);
    command(&mut sim, 1, ClientCommand::SkipCompletionCredits);
    assert_eq!(sim.state.peer_dimension(1), Dimension::Overworld);
    assert_eq!(sim.state.peer_dimension(2), Dimension::End);
    assert!(
        !sim.state
            .world
            .get_block(
                sim.state.player.x.floor() as i32,
                (sim.state.player.y - 1.0).floor() as i32
            )
            .is_solid(),
        "End return left the player buried in the recipe terrain"
    );
    sim.save().unwrap();
    let path = sim.state.world.save_root().to_path_buf();
    let data: serde_json::Value =
        serde_json::from_slice(&std::fs::read(path.join("server-state.json")).unwrap()).unwrap();
    assert_eq!(data["progression"]["dragon_defeated"], true);
    drop(sim);
    let mut restarted = ServerSimulation::with_state(GameState::open_in_directory(&path).unwrap());
    restarted.load_persistent().unwrap();
    assert_eq!(
        serde_json::to_value(super::super::state::SharedProgress::capture(
            &restarted.state
        ))
        .unwrap()["dragon_defeated"],
        true
    );
}

#[test]
fn cooperative_walk_in_portal_and_action_poses_replicate_without_affecting_other_actor() {
    let (_root, mut sim) = fixture();
    light_portal(&mut sim, 1);
    sim.state.player.x = -3.5;
    sim.state.player.y = 80.0;
    sim.state.player.grounded = true;
    for x in -6..=5 {
        if !(0..=1).contains(&x) {
            sim.state.world.set_block(x, 80, BlockType::Stone);
        }
    }
    command(&mut sim, 1, ClientCommand::SetPrimaryAction(true));
    sim.step(-4, 78);
    let local = sim.snapshot_for_client(1).unwrap();
    assert!(local.action.is_some());
    let other = sim.snapshot_for_client(2).unwrap();
    assert!(
        other
            .remote_players
            .iter()
            .find(|p| p.client_id == 1)
            .unwrap()
            .action
            .is_some()
    );
    command(&mut sim, 1, ClientCommand::SetPrimaryAction(false));
    command(&mut sim, 1, ClientCommand::SetMoveRight(true));
    for _ in 0..40 {
        sim.step(0, 0);
        if sim.state.peer_dimension(1) == Dimension::Nether {
            break;
        }
    }
    assert_eq!(sim.state.peer_dimension(1), Dimension::Nether);
    assert_eq!(sim.state.peer_dimension(2), Dimension::Overworld);
    sim.state.activate_peer(1);
    assert!(!sim.state.moving_right);
}

#[test]
fn persistence_shared_entities_and_encounters_restore_in_all_dimensions_with_player_records() {
    let (_root, mut sim) = fixture();
    sim.unregister_client(1);
    sim.unregister_client(2);
    sim.load_persistent().unwrap();
    let a = sim.authenticate(3, None).unwrap();
    let b = sim.authenticate(4, None).unwrap();
    light_portal(&mut sim, 3);
    sim.state
        .cows
        .push(crate::entities::cow::Cow::new(12.5, 80.0));
    sim.state
        .item_entities
        .push(crate::entities::item_entity::ItemEntity::new(
            13.5,
            80.0,
            ItemType::Diamond,
        ));
    command(&mut sim, 3, ClientCommand::UseAt(0, 78));
    sim.state.blazes.push(crate::entities::blaze::Blaze::new(
        sim.state.player.x + 8.0,
        sim.state.player.y - 4.0,
    ));
    sim.state.inventory.add_item(ItemType::Diamond, 3);
    sim.state.select_dimension(Dimension::End);
    sim.state.world.load_chunks_around(0);
    sim.state.ender_dragon = Some(crate::entities::ender_dragon::EnderDragon::new(12.5, 32.0));
    sim.state.ender_dragon.as_mut().unwrap().health = 51.0;
    // Restore the selected player's real dimension before checkpointing.
    sim.state.select_dimension(Dimension::Nether);
    sim.state.activate_peer(4);
    let mut z = crate::entities::zombie::Zombie::new(14.5, 80.0);
    z.director_event = 77;
    sim.state.zombies.push(z);
    sim.state.director_encounter = Some(77);
    sim.save().unwrap();
    let expected =
        serde_json::to_value(crate::engine::state::RuntimeCheckpoint::capture(&sim.state)).unwrap();
    let path = sim.state.world.save_root().to_path_buf();
    drop(sim);
    let mut restarted = ServerSimulation::with_state(GameState::open_in_directory(&path).unwrap());
    restarted.load_persistent().unwrap();
    restarted.authenticate(30, Some(&a.token)).unwrap();
    restarted.authenticate(40, Some(&b.token)).unwrap();
    assert_eq!(count(&restarted, 30, ItemType::Diamond), 3);
    assert_eq!(restarted.state.peer_dimension(30), Dimension::Nether);
    let actual = serde_json::to_value(crate::engine::state::RuntimeCheckpoint::capture(
        &restarted.state,
    ))
    .unwrap();
    assert_eq!(actual["dimensions"], expected["dimensions"]);
    assert_eq!(actual["encounter"], expected["encounter"]);
    restarted.state.activate_peer(40);
    assert_eq!(restarted.state.item_entities.len(), 1);
    assert_eq!(restarted.state.cows.len(), 1);
    restarted.state.activate_peer(30);
    assert_eq!(restarted.state.blazes.len(), 1);
}

#[test]
fn persistence_shared_and_solo_modes_choose_the_newest_entity_checkpoint() {
    let (_root, mut sim) = fixture();
    let path = sim.state.world.save_root().to_path_buf();
    sim.unregister_client(1);
    sim.unregister_client(2);
    sim.state
        .item_entities
        .push(crate::entities::item_entity::ItemEntity::new(
            13.5,
            80.0,
            ItemType::Diamond,
        ));
    sim.state.try_persist_world_and_progress().unwrap();
    sim.load_persistent().unwrap();
    let a = sim.authenticate(3, None).unwrap();
    sim.state.activate_peer(3);
    sim.state.item_entities.clear();
    sim.state.inventory.add_item(ItemType::Diamond, 1);
    sim.state
        .cows
        .push(crate::entities::cow::Cow::new(12.5, 80.0));
    sim.save().unwrap();
    drop(sim);
    let mut solo = GameState::open_in_directory(&path).unwrap();
    assert!(
        solo.item_entities.is_empty(),
        "hosting resurrected native loot"
    );
    assert_eq!(solo.cows.len(), 1);
    solo.cows.clear();
    solo.sheep
        .push(crate::entities::sheep::Sheep::new(14.5, 80.0));
    solo.try_persist_world_and_progress().unwrap();
    drop(solo);
    let mut restarted = ServerSimulation::with_state(GameState::open_in_directory(&path).unwrap());
    restarted.load_persistent().unwrap();
    restarted.authenticate(30, Some(&a.token)).unwrap();
    assert!(restarted.state.item_entities.is_empty());
    assert!(
        restarted.state.cows.is_empty(),
        "hosting restored an older native entity checkpoint"
    );
    assert_eq!(restarted.state.sheep.len(), 1);
    assert_eq!(count(&restarted, 30, ItemType::Diamond), 1);
}

#[test]
fn persistence_legacy_server_v2_upgrades_and_invalid_v3_entities_are_preserved() {
    let (_root, mut sim) = fixture();
    sim.load_persistent().unwrap();
    let path = sim.state.world.save_root().join("server-state.json");
    let mut legacy: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    legacy["version"] = 2.into();
    legacy.as_object_mut().unwrap().remove("runtime");
    std::fs::write(&path, serde_json::to_vec(&legacy).unwrap()).unwrap();
    sim.load_persistent().unwrap();
    let mut current: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    assert_eq!(current["version"], 3);
    assert!(current["runtime"].is_object());
    current["runtime"]["dimensions"]["Overworld"]["time_of_day"] = serde_json::json!(999999.0);
    let invalid = serde_json::to_vec(&current).unwrap();
    std::fs::write(&path, &invalid).unwrap();
    assert!(sim.load_persistent().is_err());
    assert_eq!(std::fs::read(&path).unwrap(), invalid);
    assert!(GameState::open_in_directory(sim.state.world.save_root()).is_err());
}
