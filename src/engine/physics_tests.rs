use super::*;

fn scene() -> GameState {
    let mut state = GameState::new();
    state.world.load_chunks_around(0);
    state.world.check_storage().unwrap();
    for x in -8..=8 {
        for y in 0..32 {
            state.world.set_block(x, y, BlockType::Air);
        }
    }
    state.player = Player::new(0.5, 20.0);
    state
}

#[test]
fn rays_and_fast_projectiles_respect_empty_and_solid_halves() {
    let mut s = scene();
    s.world.set_block(1, 19, BlockType::StoneSlab);
    assert!(
        s.terrain_segment_hit(0.2, 19.25, 3.8, 19.25, false)
            .is_none()
    );
    let impact = s
        .terrain_segment_hit(0.2, 19.75, 3.8, 19.75, false)
        .unwrap();
    assert!((impact.0 - 1.0).abs() < 0.00001);
    assert!(s.has_line_of_sight(0.2, 19.25, 3.8, 19.25));
    s.world.set_block(1, 19, BlockType::StoneStairs);
    let impact = s
        .terrain_segment_hit(0.2, 19.25, 3.8, 19.25, false)
        .unwrap();
    assert!((impact.0 - 1.5).abs() < 0.00001);
    assert!(!s.has_line_of_sight(0.2, 19.25, 3.8, 19.25));
    assert!(
        s.terrain_segment_hit(1.45, 18.9, 1.05, 19.4, false)
            .is_none()
    );
}

#[test]
fn dropped_items_settle_on_the_visible_slab_top() {
    let mut s = scene();
    s.game_rules.do_mob_spawning = false;
    s.player.x = -6.0;
    s.world.set_block(3, 19, BlockType::StoneSlab);
    let mut item = ItemEntity::new(3.2, 18.8, ItemType::Diamond);
    item.vx = 0.0;
    item.vy = 0.1;
    s.item_entities.push(item);
    for _ in 0..15 {
        s.update(0, 0);
    }
    assert_eq!(s.item_entities.len(), 1);
    assert_eq!(s.item_entities[0].y, 19.5);
    assert!(s.item_entities[0].grounded);
}

#[test]
fn visible_dragon_head_takes_hits_on_both_facings_and_wing_air_does_not() {
    let mut s = scene();
    s.current_dimension = Dimension::End;
    for right in [false, true] {
        let mut dragon = EnderDragon::new(0.5, 18.0);
        dragon.facing_right = right;
        let head_x = if right { 3.5 } else { -2.5 };
        s.ender_dragon = Some(dragon);
        assert!(s.try_apply_player_arrow_hit(head_x, 15.9, 0.3, 0.0, 5.0));
        assert_eq!(s.ender_dragon.as_ref().unwrap().health, 196.0);
        assert!(!s.try_apply_player_arrow_hit(0.5, 13.5, 0.3, 0.0, 5.0));
    }
}

#[test]
fn slabs_and_stairs_match_visible_surfaces_in_both_directions() {
    let mut s = scene();
    for x in -16..=16 {
        for y in 0..20 {
            s.world.set_block(x, y, BlockType::Air);
        }
        s.world.set_block(x, 20, BlockType::Stone);
    }
    for x in [0, 1, 6, 7] {
        s.world.set_block(x, 19, BlockType::StoneSlab);
    }
    s.world.set_block(2, 19, BlockType::StoneStairs);
    for x in 3..=5 {
        s.world.set_block(x, 19, BlockType::Stone);
    }
    for direction in [-1.0, 1.0] {
        let mut x = if direction > 0.0 { -1.5 } else { 8.5 };
        let mut y = 20.0;
        let mut grounded = true;
        for _ in 0..56 {
            let (nx, ny, _, vy, gr, hit) = s.calculate_movement_with_jump_held(
                x,
                y,
                direction * 0.2,
                0.0,
                grounded,
                PLAYER_HALF_WIDTH,
                PLAYER_HEIGHT,
                true,
                false,
            );
            assert!(!hit, "shaped course blocked at {x},{y}");
            assert!(
                gr && vy == 0.0,
                "stair traversal must stay grounded at {nx},{ny}"
            );
            assert!(
                (ny - y).abs() <= 0.501,
                "stairs must change height by half a block"
            );
            assert!(
                !s.is_colliding(nx, ny - 0.01, CollisionType::Horizontal),
                "feet penetrated drawn surface"
            );
            x = nx;
            y = ny;
            grounded = gr;
        }
        assert!(if direction > 0.0 { x > 8.0 } else { x < -1.0 });
    }
}

#[test]
fn shaped_blocks_land_at_half_height_and_leave_real_head_clearance() {
    let mut s = scene();
    s.world.set_block(0, 20, BlockType::StoneSlab);
    let (_, y, _, vy, grounded, _) =
        s.calculate_movement(0.25, 20.2, 0.0, 0.6, false, 0.2, 1.8, false);
    assert_eq!(y, 20.5);
    assert_eq!(vy, 0.0);
    assert!(grounded);
    s.world.set_block(0, 20, BlockType::StoneStairs);
    for (x, expected_y) in [(0.2, 20.5), (0.8, 20.0)] {
        let (_, y, _, _, gr, _) = s.calculate_movement(x, 19.8, 0.0, 0.8, false, 0.1, 1.8, false);
        assert_eq!(y, expected_y);
        assert!(gr);
    }
    assert!(!s.is_colliding(0.2, 20.25, CollisionType::Horizontal));
    assert!(s.is_colliding(0.8, 20.25, CollisionType::Horizontal));
}

#[test]
fn half_step_cannot_push_player_through_a_low_ceiling_or_up_a_full_wall() {
    let mut s = scene();
    for x in -2..=3 {
        s.world.set_block(x, 20, BlockType::Stone);
    }
    s.world.set_block(1, 19, BlockType::StoneSlab);
    s.world.set_block(1, 17, BlockType::Stone);
    let (x, y, _, _, _, hit) = s.calculate_movement(
        0.5,
        20.0,
        0.6,
        0.0,
        true,
        PLAYER_HALF_WIDTH,
        PLAYER_HEIGHT,
        true,
    );
    assert!(hit && x <= 0.752);
    assert_eq!(y, 20.0);
    s.world.set_block(1, 17, BlockType::Air);
    s.world.set_block(1, 19, BlockType::Stone);
    let (_, y, _, _, _, hit) = s.calculate_movement(
        0.5,
        20.0,
        0.6,
        0.0,
        true,
        PLAYER_HALF_WIDTH,
        PLAYER_HEIGHT,
        true,
    );
    assert!(hit);
    assert_eq!(y, 20.0);
}

#[test]
fn held_jump_climbs_ladder_continuously_and_sneak_holds_position() {
    let mut s = scene();
    for y in 0..30 {
        s.world.set_block(0, y, BlockType::Ladder);
    }
    s.apply_client_command(ClientCommand::SetJumpHeld(true));
    s.apply_client_command(ClientCommand::QueueJump);
    for _ in 0..12 {
        s.tick_player_actions(5, 10);
    }
    assert!(
        s.player.y < 17.5,
        "holding climb must not fall back: {}",
        s.player.y
    );
    s.apply_client_command(ClientCommand::SetJumpHeld(false));
    s.apply_client_command(ClientCommand::SetSneakHeld(true));
    let y = s.player.y;
    for _ in 0..20 {
        s.tick_player_actions(5, 10);
    }
    assert!(
        (s.player.y - y).abs() < 0.001,
        "sneaking must hold on the ladder"
    );
    assert_eq!(s.player.fall_distance, 0.0);
}

#[test]
fn sneaking_catches_existing_momentum_at_both_edges() {
    for direction in [-1.0, 1.0] {
        let mut s = scene();
        s.world.set_block(0, 20, BlockType::Stone);
        s.player.grounded = true;
        s.player.vx = direction * 0.6;
        s.apply_client_command(ClientCommand::SetSneakHeld(true));
        for _ in 0..15 {
            s.tick_player_actions(5, 10);
        }
        assert!(
            s.player.grounded && s.player.y == 20.0,
            "must stay on ledge"
        );
        assert!((0.25..=0.75).contains(&s.player.x));
    }
}

#[test]
fn landing_includes_final_partial_tick_in_fall_damage() {
    let mut s = scene();
    for x in -8..=8 {
        s.world.set_block(x, 20, BlockType::Stone);
    }
    s.player.y = 19.7;
    s.player.vy = 0.5;
    s.player.fall_distance = 2.8;
    s.tick_player_actions(5, 10);
    assert!(s.player.grounded);
    assert_eq!(s.player.health, 19.0);
    assert_eq!(s.player.fall_distance, 0.0);
}

#[test]
fn directional_release_brakes_once_without_erasing_later_knockback() {
    let mut s = scene();
    s.player.grounded = true;
    s.apply_client_command(ClientCommand::SetMoveRight(true));
    s.player.vx = 0.6;
    s.apply_client_command(ClientCommand::SetMoveRight(false));
    assert!(s.player.vx < 0.12);
    s.player.vx = -0.4;
    s.apply_client_command(ClientCommand::SetMoveRight(false));
    assert_eq!(
        s.player.vx, -0.4,
        "duplicate releases must preserve external impulses"
    );
}

#[test]
fn precise_controls_reverse_on_the_next_tick_and_stop_within_a_fraction_of_a_tile() {
    let mut s = scene();
    for x in -8..=8 {
        s.world.set_block(x, 20, BlockType::Stone);
    }
    s.player.grounded = true;
    s.apply_client_command(ClientCommand::SetMoveRight(true));
    for _ in 0..10 {
        s.tick_player_actions(5, 10);
    }
    assert!((0.30..=0.32).contains(&s.player.vx));
    s.apply_client_command(ClientCommand::SetMoveLeft(true));
    s.tick_player_actions(5, 10);
    assert!(
        s.player.vx < 0.0,
        "reversal should not spend ticks sliding the wrong way"
    );
    s.apply_client_command(ClientCommand::SetMoveLeft(false));
    let x = s.player.x;
    for _ in 0..10 {
        s.tick_player_actions(5, 10);
    }
    assert!((s.player.x - x).abs() < 0.15);
}

#[test]
fn releasing_jump_shortens_the_arc_without_cancelling_falls_or_external_impulses() {
    fn arc(release: bool) -> f64 {
        let mut s = scene();
        for x in -8..=8 {
            s.world.set_block(x, 20, BlockType::Stone);
        }
        s.player.grounded = true;
        s.apply_client_command(ClientCommand::SetJumpHeld(true));
        s.apply_client_command(ClientCommand::QueueJump);
        s.tick_player_actions(5, 10);
        if release {
            s.apply_client_command(ClientCommand::SetJumpHeld(false));
        }
        let mut top = s.player.y;
        for _ in 0..20 {
            s.tick_player_actions(5, 10);
            top = top.min(s.player.y);
        }
        assert!(s.player.grounded);
        20.0 - top
    }
    let hop = arc(true);
    let jump = arc(false);
    assert!(jump > 1.0 && hop < jump * 0.7);
    let mut s = scene();
    s.player.vy = 0.4;
    s.apply_client_command(ClientCommand::SetJumpHeld(false));
    assert_eq!(s.player.vy, 0.4);
    s.player.vy = -0.5;
    s.apply_client_command(ClientCommand::SetJumpHeld(false));
    assert_eq!(
        s.player.vy, -0.5,
        "duplicate releases must preserve knockback"
    );
}

#[test]
fn recovery_preserves_dirty_memory_without_replacing_an_unreadable_source_chunk() {
    let mut state = scene();
    state.world.set_block(1, 19, BlockType::Planks);
    assert_eq!(state.world.get_block(1, 19), BlockType::Planks);
    state.inventory.add_item(ItemType::Diamond, 9);
    let source = state.world.save_root().join("overworld_chunk_0.bin");
    std::fs::write(&source, b"corrupt original").unwrap();
    let root =
        std::env::temp_dir().join(format!("termcraft-recovery-test-{}", rand::random::<u64>()));
    state.write_recovery_directory(&root).unwrap();
    assert_eq!(std::fs::read(&source).unwrap(), b"corrupt original");
    let recovered = GameState::open_in_directory(&root).unwrap();
    assert_eq!(recovered.world.get_block(1, 19), BlockType::Planks);
    assert!(recovered.inventory.has_item(ItemType::Diamond, 9));
    assert!(root.join("RECOVERY.txt").exists());
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            std::fs::metadata(&root).unwrap().permissions().mode() & 0o777,
            0o700
        );
    }
    drop(recovered);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn native_checkpoint_restores_crafting_enchants_effects_and_releases_held_input() {
    let mut state = scene();
    state.crafting_grid[4] = Some(ItemStack {
        item_type: ItemType::Planks,
        count: 3,
        durability: None,
    });
    state.inventory.slots[0] = Some(ItemStack {
        item_type: ItemType::DiamondSword,
        count: 1,
        durability: Some(123),
    });
    state.inventory_enchant_levels[0] = 3;
    state.potion_strength_timer = 125;
    state.moving_right = true;
    state.jump_held = true;
    state.left_click_down = true;
    state.player.vx = 0.5;
    state.try_persist_world_and_progress().unwrap();
    let loaded = GameState::open_in_directory(state.world.save_root()).unwrap();
    assert_eq!(loaded.crafting_grid, state.crafting_grid);
    assert_eq!(loaded.inventory, state.inventory);
    assert_eq!(loaded.inventory_enchant_levels[0], 3);
    assert_eq!(loaded.potion_strength_timer, 125);
    assert!(!loaded.moving_right && !loaded.jump_held && !loaded.left_click_down);
    assert_eq!(loaded.player.vx, 0.0);
}

#[test]
fn legacy_v6_upgrades_and_invalid_full_session_preserves_original_file() {
    let state = scene();
    let path = state.world.save_root().join(PLAYER_PROGRESS_FILE);
    let mut legacy = state.to_progression_save_data();
    legacy.version = PLAYER_PROGRESS_VERSION_V6;
    legacy.complete_session = None;
    legacy.inventory.add_item(ItemType::Diamond, 4);
    GameState::save_progression_data_to_path(&path, &legacy).unwrap();
    let upgraded = GameState::open_in_directory(state.world.save_root()).unwrap();
    assert!(upgraded.inventory.has_item(ItemType::Diamond, 4));
    upgraded.try_save_progression().unwrap();
    assert_eq!(
        &std::fs::read(&path).unwrap()[..2],
        &PLAYER_PROGRESS_VERSION.to_le_bytes()
    );
    assert!(GameState::open_in_directory(state.world.save_root()).is_ok());

    let mut invalid = state.to_progression_save_data();
    invalid.complete_session.as_mut().unwrap().player.x = f64::NAN;
    let encoded = GameState::encode_progression(&invalid).unwrap();
    std::fs::write(&path, &encoded).unwrap();
    assert!(GameState::open_in_directory(state.world.save_root()).is_err());
    assert_eq!(std::fs::read(&path).unwrap(), encoded);
}

#[test]
fn flying_bodies_sweep_side_edges_and_fast_motion_in_both_directions() {
    for dimensions in [(0.875, 1.875), (0.5, 1.75)] {
        for direction in [-1.0, 1.0] {
            let mut s = scene();
            let wall = if direction > 0.0 { 3 } else { -3 };
            for y in 7..11 {
                s.world.set_block(wall, y, BlockType::Stone);
            }
            let (x, y, vx, vy) =
                s.move_flying_body((0.5, 10.0), (8.0 * direction, 0.0), dimensions, 0.6);
            assert!(!s.entity_collides_with_world(x, y, dimensions.0, dimensions.1));
            assert!(vx * direction < 0.0, "side impact did not reflect");
            assert_eq!(y, 10.0, "flying must not acquire gravity");
            assert_eq!(vy, 0.0);
            let boundary = if direction > 0.0 {
                3.0 - dimensions.0
            } else {
                -2.0 + dimensions.0
            };
            assert!((x - boundary).abs() < 0.0001);
        }
    }
}

#[test]
fn flying_heads_stop_at_ceilings_and_slab_air_stays_passable() {
    let mut s = scene();
    s.world.set_block(0, 7, BlockType::Stone);
    for dimensions in [(0.875, 1.875), (0.5, 1.75)] {
        let (_, y, _, vy) = s.move_flying_body((0.5, 10.0), (0.0, -3.0), dimensions, 0.6);
        assert!(y - dimensions.1 >= 8.0 && y - dimensions.1 < 8.0001);
        assert!(vy > 0.0);
    }
    s.world.set_block(1, 10, BlockType::StoneSlab);
    let (x, _, vx, _) = s.move_flying_body((0.0, 10.25), (3.0, 0.0), (0.5, 1.75), 0.6);
    assert_eq!((x, vx), (3.0, 3.0));
    let (x, _, vx, _) = s.move_flying_body((0.0, 10.75), (3.0, 0.0), (0.5, 1.75), 0.6);
    assert!(x < 0.501 && vx < 0.0);
    s.world.set_block(1, 10, BlockType::Wood);
    s.world.set_block(1, 8, BlockType::Leaves);
    let (x, _, vx, _) = s.move_flying_body((0.0, 10.75), (3.0, 0.0), (0.5, 1.75), 0.6);
    assert_eq!(
        (x, vx),
        (3.0, 3.0),
        "natural tree trunks use the shared pass-through policy"
    );
}

#[test]
fn complete_flying_ai_tick_respects_head_only_terrain() {
    let mut s = scene();
    s.dismiss_startup_splash();
    s.game_rules.do_mob_spawning = false;
    s.player.x = -7.0;
    s.world.set_block(2, 7, BlockType::Stone);
    let mut ghast = Ghast::new(2.5, 10.0);
    ghast.hit_timer = 8;
    ghast.vy = -2.0;
    let mut blaze = Blaze::new(-2.5, 10.0);
    blaze.hit_timer = 8;
    blaze.vy = -2.0;
    s.world.set_block(-3, 7, BlockType::Stone);
    s.ghasts.push(ghast);
    s.blazes.push(blaze);
    s.update(0, 0);
    assert!(s.ghasts[0].y - 1.875 >= 8.0);
    assert!(s.blazes[0].y - 1.75 >= 8.0);
}

#[test]
fn flying_spawns_reject_pockets_that_only_fit_the_center() {
    let mut s = scene();
    for x in -2..=2 {
        for y in 0..100 {
            s.world.set_block(x, y, BlockType::Stone);
        }
    }
    for x in -1..=1 {
        for y in 18..=21 {
            s.world.set_block(x, y, BlockType::Air);
        }
    }
    assert_eq!(s.find_nether_air_spawn_for_ghast(0), None);
    for x in -1..=1 {
        s.world.set_block(x, 17, BlockType::Air);
    }
    assert!(s.find_nether_air_spawn_for_ghast(0).is_some());
    for x in -1..=1 {
        for y in 17..20 {
            s.world.set_block(x, y, BlockType::Stone);
        }
    }
    assert_eq!(s.find_nether_air_spawn_for_blaze(0), None);
    for x in -1..=1 {
        for y in 18..20 {
            s.world.set_block(x, y, BlockType::Air);
        }
    }
    assert!(s.find_nether_air_spawn_for_blaze(0).is_some());
}

#[test]
fn villager_home_and_shelter_steering_wait_for_hit_recovery() {
    for time in [6000.0, 23000.0] {
        let mut s = scene();
        s.dismiss_startup_splash();
        s.game_rules.do_mob_spawning = false;
        s.game_rules.do_daylight_cycle = false;
        s.time_of_day = time;
        for x in -8..=8 {
            s.world.set_block(x, 20, BlockType::Stone);
        }
        s.world.set_block(-6, 17, BlockType::Planks);
        let mut v = Villager::new(4.5, 20.0, -6, 20);
        v.grounded = true;
        v.hit_timer = 8;
        v.vx = 0.4;
        s.villagers.push(v);
        s.update(0, 0);
        assert!(
            s.villagers[0].x > 4.75 && s.villagers[0].vx > 0.0,
            "home steering erased recoil at {time}"
        );
    }
}

#[test]
fn npc_fluid_motion_is_independent_of_player_jump_hold() {
    let mut s = scene();
    for x in -2..=2 {
        for y in 17..=21 {
            s.world.set_block(x, y, BlockType::Water(0));
        }
    }
    s.world.set_block(1, 19, BlockType::Stone);
    s.jump_held = false;
    let released = s.calculate_mob_movement(0.5, 20.0, 0.3, -0.05, false, 0.3, 1.8, true);
    s.jump_held = true;
    let held = s.calculate_mob_movement(0.5, 20.0, 0.3, -0.05, false, 0.3, 1.8, true);
    assert_eq!(released, held);
}

fn generated_village_scene() -> (
    GameState,
    crate::world::village::VillageSite,
    std::path::PathBuf,
) {
    let root = std::env::temp_dir().join(format!("tc-village-engine-{}", rand::random::<u64>()));
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(
        root.join("overworld_world_seed.bin"),
        bincode::serialize(&[1337u32, 7331, 9917, 4444]).unwrap(),
    )
    .unwrap();
    let mut s = GameState::open_in_directory(&root).unwrap();
    s.dismiss_startup_splash();
    s.game_rules.do_mob_spawning = false;
    s.game_rules.do_daylight_cycle = false;
    s.game_rules.do_weather_cycle = false;
    let site = s
        .world
        .village_sites_near(0, 10000)
        .into_iter()
        .filter(|v| v.center_x < 0)
        .min_by_key(|v| v.center_x.abs())
        .unwrap();
    s.world.load_chunks_for_spawn_search(site.center_x, 100);
    s.villagers.clear();
    s.player = Player::new((site.center_x - 46) as f64 + 0.5, site.ground_y as f64);
    s.player.grounded = true;
    (s, site, root)
}
#[test]
fn generated_village_is_walkable_both_ways_using_normal_door_commands() {
    let (mut s, site, root) = generated_village_scene();
    let g = site.ground_y;
    for direction in [1.0, -1.0] {
        s.player = Player::new((site.center_x as f64) - direction * 46.0 + 0.5, g as f64);
        s.player.grounded = true;
        s.apply_client_command(ClientCommand::SetMoveRight(direction > 0.0));
        s.apply_client_command(ClientCommand::SetMoveLeft(direction < 0.0));
        let mut crossed = false;
        for _ in 0..420 {
            for (left, right, _) in site.buildings() {
                for door in [left, right] {
                    let dx = door as f64 + 0.5 - s.player.x;
                    if dx * direction > 0.0
                        && dx.abs() < 2.4
                        && s.world.get_block(door, g - 1) == BlockType::WoodDoor(false)
                    {
                        s.apply_client_command(ClientCommand::UseAt(door, g - 1));
                        assert_eq!(s.world.get_block(door, g - 2), BlockType::WoodDoor(true));
                    }
                }
            }
            s.update(0, 0);
            assert!(
                (s.player.y - g as f64).abs() < 0.01,
                "unexpected fall/step at {},{}",
                s.player.x,
                s.player.y
            );
            if (s.player.x - site.center_x as f64) * direction >= 46.0 {
                crossed = true;
                break;
            }
        }
        assert!(crossed, "settlement corridor blocked at {}", s.player.x);
        s.apply_client_command(ClientCommand::ClearDirectionalInput);
    }
    drop(s);
    std::fs::remove_dir_all(root).unwrap();
}
#[test]
fn generated_village_has_three_homes_and_villagers_use_real_entries() {
    let (mut s, site, root) = generated_village_scene();
    let g = site.ground_y;
    let anchors = s.collect_village_hut_anchors(site.center_x);
    assert_eq!(
        anchors.len(),
        3,
        "opposite doors or market gates became duplicate homes: {anchors:?}"
    );
    let (left, _, _) = site.buildings()[0];
    s.player.x = (site.center_x + 46) as f64;
    s.time_of_day = 23000.0;
    let mut v = Villager::new((left - 3) as f64 + 0.5, g as f64, left, g - 1);
    v.grounded = true;
    s.villagers.push(v);
    for _ in 0..100 {
        s.update(0, 0);
    }
    assert!(s.villagers[0].x > left as f64);
    assert!(!s.is_exposed_to_sky(s.villagers[0].x, s.villagers[0].y - 1.0));
    s.time_of_day = 6000.0;
    let mut exited = false;
    for _ in 0..140 {
        s.update(0, 0);
        exited |= s.is_exposed_to_sky(s.villagers[0].x, s.villagers[0].y - 1.0);
    }
    assert!(exited, "villager never returned outdoors in daylight");
    drop(s);
    std::fs::remove_dir_all(root).unwrap();
}
#[test]
fn generated_loft_storage_and_beds_are_reachable_after_normal_climbing() {
    use crate::world::village::BuildingRole;
    let (mut s, site, root) = generated_village_scene();
    let g = site.ground_y;
    for (left, right, role) in site.buildings() {
        let at = |offset| {
            if role == BuildingRole::Cottage {
                left + offset
            } else {
                right - offset
            }
        };
        s.player = Player::new(at(4) as f64 + 0.5, g as f64);
        s.player.grounded = true;
        s.apply_client_command(ClientCommand::SetJumpHeld(true));
        s.apply_client_command(ClientCommand::QueueJump);
        for _ in 0..25 {
            s.update(0, 0);
            if s.player.y <= (g - 4) as f64 {
                break;
            }
        }
        // All stations must be usable from the ladder. Do not qualify a room
        // by making the player jump across the tops of its furniture.
        s.apply_client_command(ClientCommand::SetJumpHeld(false));
        s.apply_client_command(ClientCommand::SetSneakHeld(true));
        s.apply_client_command(ClientCommand::UseAt(at(1), g - 4));
        assert!(s.at_chest, "loft chest was out of reach");
        s.apply_client_command(ClientCommand::ToggleInventory);
        if role == BuildingRole::Workshop {
            s.apply_client_command(ClientCommand::UseAt(at(3), g - 4));
            assert!(s.at_furnace);
            s.apply_client_command(ClientCommand::ToggleInventory);
        } else {
            s.time_of_day = 23000.0;
            s.apply_client_command(ClientCommand::UseAt(at(2), g - 4));
            assert_eq!(s.spawn_point, Some((at(2), g - 4)));
            s.apply_client_command(ClientCommand::TravelToSpawn);
            assert!(!s.entity_collides_with_world(
                s.player.x,
                s.player.y,
                PLAYER_HALF_WIDTH,
                PLAYER_HEIGHT
            ));
            assert!((s.player.x - (at(2) as f64 + 0.5)).abs() < 12.0);
        }
        s.apply_client_command(ClientCommand::SetSneakHeld(false));
    }
    drop(s);
    std::fs::remove_dir_all(root).unwrap();
}
