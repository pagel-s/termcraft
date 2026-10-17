//! Lifecycle tests use isolated worlds and the same checkpoint entry points as play.
use super::*;
use crate::world::recipe::WorldRecipe;
use crate::worlds::WorldStore;

struct Fixture(PathBuf);
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
fn fixture() -> (Fixture, GameState) {
    let root = std::env::temp_dir().join(format!(
        "termcraft-persistence-{}",
        super::super::server_save::token()
    ));
    let mut recipe = WorldRecipe::example();
    recipe.surface_y = 64;
    recipe.relief = 0;
    recipe.regions.clear();
    recipe.landmarks.clear();
    let path = WorldStore::new(root.clone())
        .create("test", "Persistence fixture", &recipe)
        .unwrap();
    let mut state = GameState::open_in_directory(&path).unwrap();
    state.dismiss_startup_splash();
    state.game_rules.do_mob_spawning = false;
    state.game_rules.do_daylight_cycle = false;
    state.game_rules.do_weather_cycle = false;
    for x in -16..48 {
        for y in 0..64 {
            state.world.set_block(x, y, BlockType::Air);
        }
        state.world.set_block(x, 64, BlockType::Stone);
    }
    state.world.newly_generated_chunks.clear();
    state.player.x = 0.5;
    state.player.y = 64.0;
    (Fixture(root), state)
}
fn populate(s: &mut GameState) {
    s.zombies.push(Zombie::new(12.5, 64.0));
    s.creepers.push(Creeper::new(13.5, 64.0));
    s.skeletons.push(Skeleton::new(14.5, 64.0));
    s.spiders.push(Spider::new(15.5, 64.0));
    s.silverfish.push(Silverfish::new(16.5, 64.0));
    s.slimes.push(Slime::new(17.5, 64.0, 2));
    s.endermen.push(Enderman::new(18.5, 64.0));
    s.blazes.push(Blaze::new(19.5, 64.0));
    s.pigmen.push(ZombiePigman::new(20.5, 64.0));
    s.ghasts.push(Ghast::new(21.5, 44.0));
    s.cows.push(Cow::new(22.5, 64.0));
    s.sheep.push(Sheep::new(23.5, 64.0));
    s.sheep[0].sheared = true;
    s.sheep[0].health = 5.0;
    s.pigs.push(Pig::new(24.5, 64.0));
    s.chickens.push(Chicken::new(25.5, 64.0));
    s.squids.push(Squid::new(26.5, 64.0));
    s.wolves.push(Wolf::new(27.5, 64.0));
    s.ocelots.push(Ocelot::new(28.5, 64.0));
    s.villagers.push(Villager::new(29.5, 64.0, 29, 63));
    s.boats.push(Boat::new(30.5, 64.0));
    s.item_entities
        .push(ItemEntity::new(31.5, 64.0, ItemType::Diamond));
    s.item_entities[0].age = 1234;
    s.experience_orbs.push(ExperienceOrb::new(32.5, 64.0, 7));
    s.experience_orbs[0].age = 432;
    s.arrows.push(Arrow::new_player(33.5, 62.0, 0.8, -0.1, 5.0));
    s.fireballs.push(Fireball::new(34.5, 60.0, -0.2, 0.1));
    s.end_crystals.push(EndCrystal::new(35.5, 44.0));
    s.ender_dragon = Some(EnderDragon::new(36.5, 32.0));
    s.lightning_bolts.push(LightningBolt {
        x: 37,
        y_top: 0,
        y_bottom: 64,
        ttl: 2,
    });
    s.world_tick = 5678;
    s.villager_open_doors.insert((29, 63), 8);
}

#[test]
fn persistence_villagers_sleep_without_terrain_and_recover_old_void_falls() {
    let (_fixture, mut s) = fixture();
    let root = s.world.save_root().to_path_buf();
    let mut sleeping = Villager::new(8191.8, 64.0, 8191, 63);
    sleeping.health = 7.0;
    sleeping.vx = 0.13;
    sleeping.vy = 0.9;
    s.villagers.push(sleeping.clone());
    let mut fallen = Villager::new(8192.5, 1400.0, 8192, 63);
    fallen.health = 13.0;
    fallen.age = 12345;
    s.villagers.push(fallen);
    for _ in 0..40 {
        s.update(0, 0);
    }
    assert_eq!(s.villagers[0], sleeping);
    assert_eq!((s.villagers[1].x, s.villagers[1].y), (8192.5, 64.0));
    assert_eq!(s.villagers[1].health, 13.0);
    assert_eq!(s.villagers[1].age, 12345);
    s.try_persist_world_and_progress().unwrap();
    drop(s);
    let s = GameState::open_in_directory(&root).unwrap();
    assert_eq!(s.villagers.len(), 2);
    assert_eq!(s.villagers[0], sleeping);
    assert_eq!(s.villagers[1].health, 13.0);
}

#[test]
fn persistence_thrown_stack_and_reflected_projectile_roundtrip() {
    let (_fixture, mut s) = fixture();
    let root = s.world.save_root().to_path_buf();
    s.inventory.slots[0] = Some(ItemStack {
        item_type: ItemType::DiamondSword,
        count: 1,
        durability: Some(37),
    });
    s.inventory_enchant_levels[0] = 3;
    s.hotbar_index = 0;
    assert!(s.drop_selected_item(true));
    let mut f = Fireball::new(12.0, 60.0, 0.48, 0.0);
    f.reflected = true;
    s.fireballs.push(f.clone());
    let item = s.item_entities[0].clone();
    s.try_persist_world_and_progress().unwrap();
    drop(s);
    let mut s = GameState::open_in_directory(&root).unwrap();
    assert_eq!(s.fireballs, [f]);
    assert_eq!(s.item_entities.as_slice(), std::slice::from_ref(&item));
    let mut item = item;
    assert!(s.collect_loose_item(&mut item));
    assert_eq!(s.inventory.slots[0].as_ref().unwrap().durability, Some(37));
    assert_eq!(s.inventory_enchant_levels[0], 3);
}

#[test]
fn thrown_items_wait_two_seconds_before_any_player_can_collect_them() {
    let (_fixture, mut s) = fixture();
    s.inventory.slots.fill(None);
    s.inventory_enchant_levels.fill(0);
    s.inventory.add_item(ItemType::Wood, 1);
    s.hotbar_index = 0;
    assert!(s.drop_selected_item(false));
    for _ in 0..40 {
        let item = &s.item_entities[0];
        s.player.x = item.x;
        s.player.y = item.y + 1.0;
        s.update(0, 0);
        assert!(!s.inventory.has_item(ItemType::Wood, 1));
    }
    assert_eq!(s.item_entities[0].pickup_delay, 0);
    s.player.x = s.item_entities[0].x;
    s.player.y = s.item_entities[0].y + 1.0;
    s.update(0, 0);
    assert!(s.inventory.has_item(ItemType::Wood, 1));
    assert!(s.item_entities.is_empty());
}

#[test]
fn persistence_fire_and_ore_timers_resume_from_saved_chunk_activity() {
    let (_fixture, mut s) = fixture();
    let root = s.world.save_root().to_path_buf();
    s.world.set_block(3, 63, BlockType::LitRedstoneOre(6));
    s.world.set_block(6, 63, BlockType::Fire(6));
    s.world.set_block(6, 64, BlockType::Planks);
    s.try_persist_world_and_progress().unwrap();
    drop(s);
    let mut s = GameState::open_in_directory(&root).unwrap();
    assert_eq!(s.world.get_block(3, 63), BlockType::LitRedstoneOre(6));
    assert_eq!(s.world.get_block(6, 63), BlockType::Fire(6));
    for _ in 0..10 {
        s.world.update(0);
    }
    assert_eq!(s.world.get_block(3, 63), BlockType::LitRedstoneOre(5));
    assert_eq!(s.world.get_block(6, 63), BlockType::Fire(5));
}

#[test]
fn persistence_completed_furnace_waits_for_bag_space_without_losing_output_or_fuel() {
    let (_fixture, mut s) = fixture();
    let root = s.world.save_root().to_path_buf();
    let stone = Some(ItemStack {
        item_type: ItemType::Stone,
        count: 64,
        durability: None,
    });
    s.inventory.slots.fill(stone.clone());
    s.inventory.slots[0] = Some(ItemStack {
        item_type: ItemType::RawIron,
        count: 1,
        durability: None,
    });
    s.inventory.slots[1] = Some(ItemStack {
        item_type: ItemType::Coal,
        count: 1,
        durability: None,
    });
    let job = GameState::furnace_job_for_input_item(ItemType::RawIron).unwrap();
    assert!(s.start_furnace_job(job));
    s.inventory.slots[0] = stone.clone();
    s.inventory.slots[1] = stone;
    for _ in 0..FURNACE_COOK_TICKS {
        s.update_furnace();
    }
    let fuel = s.furnace_burn_ticks;
    for _ in 0..50 {
        s.update_furnace();
    }
    assert_eq!(s.furnace_burn_ticks, fuel);
    assert_eq!(s.furnace_progress_ticks, FURNACE_COOK_TICKS);
    assert!(!s.inventory.has_item(ItemType::IronIngot, 1));
    assert!(s.item_entities.is_empty());
    s.try_persist_world_and_progress().unwrap();
    drop(s);
    let mut s = GameState::open_in_directory(&root).unwrap();
    s.inventory.slots[26] = None;
    s.update_furnace();
    assert!(s.inventory.has_item(ItemType::IronIngot, 1));
    assert!(s.furnace_job.is_none());
    s.try_persist_world_and_progress().unwrap();
    drop(s);
    let s = GameState::open_in_directory(&root).unwrap();
    assert!(!s.inventory.has_item(ItemType::IronIngot, 2));
}

#[test]
fn persistence_failed_checkpoint_restores_chest_player_and_build_together() {
    let (_fixture, mut s) = fixture();
    let root = s.world.save_root().to_path_buf();
    s.world.set_block(5, 63, BlockType::Chest);
    s.world
        .chest_inventory_mut(5, 63)
        .unwrap()
        .add_item(ItemType::Diamond, 4);
    s.try_persist_world_and_progress().unwrap();
    s.world
        .chest_inventory_mut(5, 63)
        .unwrap()
        .remove_item(ItemType::Diamond, 1);
    s.inventory.add_item(ItemType::Diamond, 1);
    s.world.load_chunks_around(-40);
    s.world.set_block(-40, 63, BlockType::Planks);
    assert_eq!(s.world.get_block(-40, 63), BlockType::Planks);
    // Eviction writes are part of the unfinished checkpoint too, even though
    // these chunks have left memory before the player save fails.
    s.world.trim_chunk_cache(&[4096], 64).unwrap();
    let obstruction = root.join("player_progression.bin.tmp");
    std::fs::create_dir(&obstruction).unwrap();
    assert!(s.try_persist_world_and_progress().is_err());
    std::fs::remove_dir(obstruction).unwrap();
    drop(s);
    for _ in 0..2 {
        let mut restored = GameState::open_in_directory(&root).unwrap();
        restored.world.load_chunks_around(-40);
        assert!(!restored.inventory.has_item(ItemType::Diamond, 1));
        assert!(
            restored
                .world
                .chest_inventory(5, 63)
                .unwrap()
                .has_item(ItemType::Diamond, 4),
            "chest and inventory came from different checkpoints"
        );
        assert_ne!(restored.world.get_block(-40, 63), BlockType::Planks);
        restored.try_persist_world_and_progress().unwrap();
    }
}

#[test]
fn persistence_failed_world_checkpoint_restores_visited_runtime_and_portal_history() {
    let (_fixture, mut s) = fixture();
    let root = s.world.save_root().to_path_buf();
    let dimensions = [Dimension::Overworld, Dimension::Nether, Dimension::End];
    for dimension in dimensions {
        s.select_dimension(dimension);
        s.world.load_chunks_around(0);
        populate(&mut s);
        s.world.set_block(5, 60, BlockType::Planks);
    }
    s.select_dimension(Dimension::Overworld);
    s.portal_links.insert((0, 0, 64), (2, 80));
    s.try_persist_world_and_progress().unwrap();
    let expected = serde_json::to_value(RuntimeCheckpoint::capture(&s)).unwrap();
    for dimension in dimensions {
        s.select_dimension(dimension);
        s.cows.clear();
        s.item_entities.clear();
        s.world_tick += 100;
        s.ender_dragon.as_mut().unwrap().health = 13.0;
        s.world.set_block(5, 60, BlockType::Cobblestone);
    }
    s.select_dimension(Dimension::Overworld);
    s.portal_links.clear();
    let obstruction = root.join("player_progression.bin.tmp");
    std::fs::create_dir(&obstruction).unwrap();
    assert!(s.try_persist_world_and_progress().is_err());
    drop(s);
    // Recovery must also tolerate the failed writer's obstructed .tmp path.
    let mut restored = GameState::open_in_directory(&root).unwrap();
    std::fs::remove_dir(obstruction).unwrap();
    let actual = serde_json::to_value(RuntimeCheckpoint::capture(&restored)).unwrap();
    assert_eq!(actual["dimensions"], expected["dimensions"]);
    assert_eq!(restored.portal_links.get(&(0, 0, 64)), Some(&(2, 80)));
    for dimension in dimensions {
        restored.select_dimension(dimension);
        restored.world.load_chunks_around(0);
        assert_eq!(restored.world.get_block(5, 60), BlockType::Planks);
    }
}

#[test]
fn persistence_restores_every_entity_family_and_frozen_dimension_clock() {
    let (_fixture, mut s) = fixture();
    populate(&mut s);
    let root = s.world.save_root().to_path_buf();
    let expected = serde_json::to_value(RuntimeCheckpoint::capture(&s)).unwrap();
    s.try_persist_world_and_progress().unwrap();
    drop(s);
    let restored = GameState::open_in_directory(&root).unwrap();
    let actual = serde_json::to_value(RuntimeCheckpoint::capture(&restored)).unwrap();
    assert_eq!(actual["dimensions"], expected["dimensions"]);
    assert_eq!(restored.world_tick, 5678);
    assert_eq!(restored.item_entities[0].age, 1234);
    assert_eq!(restored.experience_orbs[0].age, 432);
    assert!(restored.sheep[0].sheared);
}

#[test]
fn persistence_uses_full_world_entities_instead_of_the_nearby_512_entity_projection() {
    let (_fixture, mut s) = fixture();
    for i in 0..600 {
        s.item_entities
            .push(ItemEntity::new(5000.5 + i as f64, 64.0, ItemType::Coal));
    }
    let root = s.world.save_root().to_path_buf();
    s.try_persist_world_and_progress().unwrap();
    let restored = GameState::open_in_directory(&root).unwrap();
    assert_eq!(restored.item_entities, s.item_entities);
}

#[test]
fn persistence_pickups_and_inventory_commit_together_without_resurrection_on_repeated_reloads() {
    let (_fixture, mut s) = fixture();
    let root = s.world.save_root().to_path_buf();
    s.item_entities.push(ItemEntity::new(
        s.player.x,
        s.player.y - 0.2,
        ItemType::Diamond,
    ));
    s.experience_orbs
        .push(ExperienceOrb::new(s.player.x, s.player.y - 1.0, 7));
    s.try_persist_world_and_progress().unwrap();
    drop(s);
    let mut s = GameState::open_in_directory(&root).unwrap();
    s.dismiss_startup_splash();
    s.update(0, 0);
    assert!(s.item_entities.is_empty());
    assert!(s.experience_orbs.is_empty());
    assert!(s.inventory.has_item(ItemType::Diamond, 1));
    assert!(!s.inventory.has_item(ItemType::Diamond, 2));
    let experience = s.player.experience_total;
    assert_eq!(experience, 7);
    for _ in 0..3 {
        s.try_persist_world_and_progress().unwrap();
        s = GameState::open_in_directory(&root).unwrap();
        assert!(s.item_entities.is_empty());
        assert!(s.experience_orbs.is_empty());
        assert!(!s.inventory.has_item(ItemType::Diamond, 2));
        assert_eq!(s.player.experience_total, experience);
    }
}

#[test]
fn persistence_real_portal_roundtrip_preserves_both_dimensions_and_survives_restart() {
    let (_fixture, mut s) = fixture();
    let root = s.world.save_root().to_path_buf();
    for x in -1..=2 {
        for y in 60..=64 {
            s.world.set_block(
                x,
                y,
                if x == -1 || x == 2 || y == 60 || y == 64 {
                    BlockType::Obsidian
                } else {
                    BlockType::Air
                },
            );
        }
    }
    s.inventory.slots[0] = Some(GameState::fresh_item_stack(ItemType::FlintAndSteel, 1));
    s.apply_client_command(ClientCommand::UseAt(-1, 62));
    s.cows.push(Cow::new(12.5, 64.0));
    s.item_entities
        .push(ItemEntity::new(11.5, 64.0, ItemType::Diamond));
    s.world_tick = 333;
    s.apply_client_command(ClientCommand::UseAt(0, 62));
    assert_eq!(s.current_dimension, Dimension::Nether);
    assert!(s.cows.is_empty());
    s.blazes
        .push(Blaze::new(s.player.x + 8.0, s.player.y - 3.0));
    s.try_persist_world_and_progress().unwrap();
    drop(s);
    let mut s = GameState::open_in_directory(&root).unwrap();
    assert_eq!(s.current_dimension, Dimension::Nether);
    assert_eq!(s.blazes.len(), 1);
    let (x, y) = s.nearby_player_nether_portal_anchor(12).unwrap();
    s.dismiss_startup_splash();
    for _ in 0..60 {
        s.apply_client_command(ClientCommand::SetMoveLeft(s.player.x > x as f64 + 3.0));
        s.update(x, y - 2);
    }
    s.apply_client_command(ClientCommand::SetMoveLeft(false));
    s.apply_client_command(ClientCommand::UseAt(x, y - 2));
    assert_eq!(s.current_dimension, Dimension::Overworld);
    assert_eq!(s.cows.len(), 1);
    assert_eq!(s.item_entities.len(), 1);
    assert_eq!(s.world_tick, 333);
    assert!(s.blazes.is_empty());
    s.travel_dimension(Dimension::Nether);
    assert_eq!(s.blazes.len(), 1);
    assert!(s.cows.is_empty());
}

#[test]
fn persistence_end_damage_and_destroyed_crystals_are_not_regenerated_after_reload() {
    let (_fixture, mut s) = fixture();
    let root = s.world.save_root().to_path_buf();
    s.travel_dimension(Dimension::End);
    s.world.load_chunks_around(0);
    s.end_boss_initialized = true;
    s.ender_dragon = Some(EnderDragon::new(12.5, 32.0));
    s.ender_dragon.as_mut().unwrap().health = 73.0;
    s.ender_dragon.as_mut().unwrap().phase = crate::entities::ender_dragon::DragonPhase::Swoop;
    s.end_crystals.clear();
    s.try_persist_world_and_progress().unwrap();
    drop(s);
    let mut s = GameState::open_in_directory(&root).unwrap();
    assert_eq!(s.ender_dragon.as_ref().unwrap().health, 73.0);
    assert!(s.end_crystals.is_empty());
    s.dismiss_startup_splash();
    s.update(0, 0);
    assert_eq!(s.ender_dragon.as_ref().unwrap().health, 73.0);
    assert!(s.end_crystals.is_empty());
}

#[test]
fn persistence_encounter_resumes_with_surviving_members_and_ends_once() {
    let (_fixture, mut s) = fixture();
    let root = s.world.save_root().to_path_buf();
    let mut z = Zombie::new(12.5, 64.0);
    z.director_event = 71;
    z.health = 3.0;
    s.zombies.push(z);
    s.director_encounter = Some(71);
    s.try_persist_world_and_progress().unwrap();
    drop(s);
    let mut s = GameState::open_in_directory(&root).unwrap();
    let mut director = super::super::director::Director::new(&mut s);
    director.poll(&mut s, std::time::Instant::now());
    assert_eq!(s.director_encounter, Some(71));
    assert_eq!(s.zombies.len(), 1);
    assert_eq!(s.zombies[0].health, 3.0);
    s.zombies.clear();
    director.poll(&mut s, std::time::Instant::now());
    assert!(s.director_encounter.is_none());
    s.try_persist_world_and_progress().unwrap();
    let restored = GameState::open_in_directory(&root).unwrap();
    assert!(restored.director_encounter.is_none());
    assert!(restored.zombies.is_empty());
}

#[test]
fn persistence_failed_write_keeps_old_inventory_and_loot_together_and_can_retry() {
    let (_fixture, mut s) = fixture();
    s.item_entities
        .push(ItemEntity::new(12.5, 64.0, ItemType::Diamond));
    s.try_persist_world_and_progress().unwrap();
    let path = s.world.save_root().join(PLAYER_PROGRESS_FILE);
    let old = std::fs::read(&path).unwrap();
    let revision = s.runtime_revision.get();
    s.item_entities.clear();
    s.inventory.add_item(ItemType::Diamond, 1);
    let temp = GameState::progression_temp_path(&path);
    std::fs::create_dir(&temp).unwrap();
    assert!(s.try_save_progression().is_err());
    assert_eq!(std::fs::read(&path).unwrap(), old);
    assert_eq!(s.runtime_revision.get(), revision);
    let old_state = GameState::open_in_directory(s.world.save_root()).unwrap();
    assert_eq!(old_state.item_entities.len(), 1);
    assert!(!old_state.inventory.has_item(ItemType::Diamond, 1));
    std::fs::remove_dir(temp).unwrap();
    s.try_save_progression().unwrap();
    let restored = GameState::open_in_directory(s.world.save_root()).unwrap();
    assert!(restored.item_entities.is_empty());
    assert!(restored.inventory.has_item(ItemType::Diamond, 1));
}

#[test]
fn persistence_invalid_or_oversized_runtime_is_rejected_without_replacing_the_save() {
    let (_fixture, mut s) = fixture();
    s.try_persist_world_and_progress().unwrap();
    let path = s.world.save_root().join(PLAYER_PROGRESS_FILE);
    let previous = std::fs::read(&path).unwrap();
    s.cows.push(Cow::new(f64::NAN, 64.0));
    assert!(s.try_save_progression().is_err());
    assert_eq!(std::fs::read(&path).unwrap(), previous);
    // Deliberately bypass the writer to test rejection of corrupt input as well.
    let invalid = GameState::encode_progression(&s.to_progression_save_data()).unwrap();
    std::fs::write(&path, &invalid).unwrap();
    assert!(GameState::open_in_directory(s.world.save_root()).is_err());
    assert_eq!(std::fs::read(&path).unwrap(), invalid);
    s.cows.clear();
    s.item_entities = vec![ItemEntity::new(10.5, 64.0, ItemType::Coal); 32_769];
    assert!(s.try_save_progression().is_err());
    assert_eq!(std::fs::read(&path).unwrap(), invalid);
}

#[test]
fn persistence_legacy_v8_upgrades_without_inventing_entities_or_losing_portal_links() {
    let (_fixture, s) = fixture();
    let path = s.world.save_root().join(PLAYER_PROGRESS_FILE);
    let mut progress = s.to_progression_save_data();
    progress.version = 8;
    let links = vec![((0, 1, 64), (2, 80))];
    let encoded = bincode::serialize(&PlayerCheckpointV8 {
        version: 8,
        progress,
        session: PlayerSession::capture(&s),
        portal_links: links.clone(),
    })
    .unwrap();
    std::fs::write(&path, encoded).unwrap();
    let mut restored = GameState::open_in_directory(s.world.save_root()).unwrap();
    assert!(restored.cows.is_empty());
    assert_eq!(restored.portal_links.into_iter().collect::<Vec<_>>(), links);
    restored.portal_links = links.into_iter().collect();
    restored.try_persist_world_and_progress().unwrap();
    assert_eq!(&std::fs::read(&path).unwrap()[..2], &9u16.to_le_bytes());
}

#[test]
fn persistence_checkpoint_failure_cancels_portal_travel_without_losing_entities() {
    let (_fixture, mut s) = fixture();
    for x in -1..=2 {
        for y in 60..=64 {
            s.world.set_block(
                x,
                y,
                if x == -1 || x == 2 || y == 60 || y == 64 {
                    BlockType::Obsidian
                } else {
                    BlockType::Air
                },
            );
        }
    }
    s.inventory.slots[0] = Some(GameState::fresh_item_stack(ItemType::FlintAndSteel, 1));
    s.apply_client_command(ClientCommand::UseAt(-1, 62));
    s.cows.push(Cow::new(12.5, 64.0));
    s.try_persist_world_and_progress().unwrap();
    let path = s.world.save_root().join(PLAYER_PROGRESS_FILE);
    let previous = std::fs::read(&path).unwrap();
    let temp = GameState::progression_temp_path(&path);
    std::fs::create_dir(&temp).unwrap();
    s.apply_client_command(ClientCommand::UseAt(0, 62));
    assert_eq!(s.current_dimension, Dimension::Overworld);
    assert_eq!(s.cows.len(), 1);
    assert!(
        s.save_error
            .borrow()
            .as_ref()
            .unwrap()
            .contains("Travel cancelled")
    );
    assert_eq!(std::fs::read(&path).unwrap(), previous);
    std::fs::remove_dir(temp).unwrap();
    s.apply_client_command(ClientCommand::UseAt(0, 62));
    assert_eq!(s.current_dimension, Dimension::Nether);
}

#[test]
fn persistence_nearby_loot_survives_death_respawn_and_reopen() {
    let (_fixture, mut s) = fixture();
    s.inventory.slots.fill(None);
    s.inventory.add_item(ItemType::IronSword, 1);
    for _ in 0..3 {
        s.item_entities
            .push(ItemEntity::new(12.5, 64.0, ItemType::Diamond));
    }
    s.game_rules.keep_inventory = false;
    s.player.health = -1.0;
    s.update(0, 0);
    assert!(s.death_screen_active);
    assert!(s.inventory.has_item(ItemType::IronSword, 1));
    assert_eq!(
        s.item_entities
            .iter()
            .filter(|e| e.item_type == ItemType::Diamond)
            .count(),
        3
    );
    s.death_screen_ticks = 200;
    s.respawn_from_death_screen();
    assert!(!s.death_screen_active);
    assert!(!s.inventory.has_item(ItemType::IronSword, 1));
    assert_eq!(
        s.item_entities
            .iter()
            .filter(|e| e.item_type == ItemType::Diamond)
            .count(),
        3
    );
    s.try_persist_world_and_progress().unwrap();
    let restored = GameState::open_in_directory(s.world.save_root()).unwrap();
    assert!(!restored.inventory.has_item(ItemType::Diamond, 1));
    assert_eq!(
        restored
            .item_entities
            .iter()
            .filter(|e| e.item_type == ItemType::Diamond)
            .count(),
        3
    );
}

#[test]
fn persistence_travel_dismounts_without_moving_boats_between_worlds() {
    let (_fixture, mut s) = fixture();
    s.boats.push(Boat::new(12.5, 64.0));
    s.mounted_boat = Some(0);
    s.fishing_bobber_active = true;
    s.travel_dimension(Dimension::Nether);
    assert!(s.mounted_boat.is_none());
    assert!(!s.fishing_bobber_active);
    assert!(s.boats.is_empty());
    s.boats.push(Boat::new(8.5, 60.0));
    s.mounted_boat = Some(0);
    s.travel_dimension(Dimension::Overworld);
    assert!(s.mounted_boat.is_none());
    assert_eq!(s.boats[0].x, 12.5);
    s.try_persist_world_and_progress().unwrap();
    let mut restored = GameState::open_in_directory(s.world.save_root()).unwrap();
    assert!(restored.mounted_boat.is_none());
    assert_eq!(restored.boats[0].x, 12.5);
    restored.travel_dimension(Dimension::Nether);
    assert_eq!(restored.boats[0].x, 8.5);
    assert!(restored.mounted_boat.is_none());
}
