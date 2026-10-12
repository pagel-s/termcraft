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
