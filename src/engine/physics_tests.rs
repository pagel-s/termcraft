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
