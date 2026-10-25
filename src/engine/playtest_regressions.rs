use super::*;

fn flat() -> GameState {
    let mut s = GameState::network_view();
    s.dismiss_startup_splash();
    s.game_rules.do_mob_spawning = false;
    s.game_rules.do_weather_cycle = false;
    s.game_rules.do_daylight_cycle = false;
    for cx in -2..=2 {
        s.world
            .apply_chunk_column_snapshot(cx, &vec![BlockType::Air; CHUNK_WIDTH * CHUNK_HEIGHT]);
    }
    for x in -64..96 {
        s.world.set_block(x, 20, BlockType::Stone);
    }
    s.player = Player::new(-8.5, 20.0);
    s.player.grounded = true;
    s
}

#[test]
fn sustained_walking_on_a_submerged_floor_stays_slower_than_dry_land() {
    for profile in [
        MovementProfile::Classic,
        MovementProfile::Smooth,
        MovementProfile::Agile,
    ] {
        let distance = |wet: bool| {
            let mut s = flat();
            s.movement_profile = profile;
            if wet {
                for x in -64..96 {
                    for y in 16..20 {
                        s.world.set_block(x, y, BlockType::Water(8));
                    }
                }
            }
            s.apply_client_command(ClientCommand::SetMoveRight(true));
            let start = s.player.x;
            for _ in 0..90 {
                s.update(0, 0);
                assert!(s.player.health > 0.0);
            }
            s.player.x - start
        };
        let land = distance(false);
        let water = distance(true);
        assert!(
            water > 1.0 && water < land * 0.6,
            "{profile:?}: water={water}, land={land}"
        );
    }
}

#[test]
fn land_animals_clear_one_block_but_cannot_step_mid_jump_over_two() {
    for door in [false, true] {
        for height in [1, 2] {
            let mut s = flat();
            for y in 20 - height..20 {
                s.world.set_block(
                    2,
                    y,
                    if door {
                        BlockType::WoodDoor(false)
                    } else {
                        BlockType::Stone
                    },
                );
            }
            let mut cow = Cow::new(0.5, 20.0);
            cow.grounded = true;
            let mut furthest = cow.x;
            for _ in 0..120 {
                cow.jump_cooldown = cow.jump_cooldown.saturating_sub(1);
                let (x, y, vx, vy, grounded, wall) = s.calculate_animal_movement(
                    cow.x,
                    cow.y,
                    0.1,
                    cow.vy,
                    cow.grounded,
                    0.6,
                    1.375,
                    false,
                );
                cow.x = x;
                cow.y = y;
                cow.vx = vx;
                cow.vy = vy;
                cow.grounded = grounded;
                if wall {
                    cow.jump();
                }
                furthest = furthest.max(cow.x);
            }
            if height == 1 {
                assert!(
                    furthest > 3.5,
                    "one-block obstacle was impassable: door={door}, x={furthest}"
                );
            } else {
                assert!(
                    furthest < 1.41,
                    "cow escaped over two-block obstacle: door={door}, x={furthest}"
                );
            }
        }
    }
}

#[test]
fn a_villager_door_stays_open_for_the_queue_and_closes_after_everyone_passes() {
    let mut s = flat();
    for y in [18, 19] {
        s.world.set_block(2, y, BlockType::WoodDoor(true));
    }
    s.villagers.push(Villager::new(3.6, 20.0, 0, 19));
    s.villagers.push(Villager::new(1.3, 20.0, 0, 19));
    s.mark_villager_door_active(2, 19);
    s.try_close_wood_door_behind_entity(3.6, 20.0);
    assert_eq!(s.world.get_block(2, 19), BlockType::WoodDoor(true));
    for _ in 0..4 {
        s.tick_villager_door_hold_timers();
    }
    s.villagers[1].x = 1.8;
    assert!(
        s.doorway_is_occupied(2, 19),
        "the trailing villager's body overlaps the doorway"
    );
    for _ in 0..40 {
        s.tick_villager_door_hold_timers();
    }
    assert_eq!(s.world.get_block(2, 19), BlockType::WoodDoor(true));
    s.villagers[1].x = 4.5;
    for _ in 0..VILLAGER_DOOR_HOLD_TICKS {
        s.tick_villager_door_hold_timers();
    }
    assert_eq!(s.world.get_block(2, 19), BlockType::WoodDoor(false));
}

#[test]
fn a_remote_players_body_keeps_an_automatic_door_open() {
    let mut s = flat();
    s.join_peer(1);
    s.join_peer(2);
    s.activate_peer(2);
    s.player.x = 1.8;
    s.player.y = 20.0;
    s.activate_peer(1);
    for y in [18, 19] {
        s.world.set_block(2, y, BlockType::WoodDoor(true));
    }
    s.mark_villager_door_active(2, 19);
    for _ in 0..40 {
        s.tick_villager_door_hold_timers();
    }
    assert_eq!(s.world.get_block(2, 19), BlockType::WoodDoor(true));
}

#[test]
fn villagers_cross_the_door_and_settle_apart_for_the_whole_night() {
    for population in [2, 4, 6] {
        let mut s = flat();
        s.time_of_day = 23000.0;
        for x in 2..=12 {
            s.world.set_block(x, 16, BlockType::Planks);
        }
        for y in 17..20 {
            s.world.set_block(2, y, BlockType::Planks);
            s.world.set_block(12, y, BlockType::Planks);
        }
        for y in [18, 19] {
            s.world.set_block(2, y, BlockType::WoodDoor(false));
        }
        s.villagers = (0..population)
            .map(|i| Villager::new(0.5 - i as f64, 20.0, 2, 19))
            .collect();
        let mut changes = 0;
        let mut was_open = false;
        for _ in 0..700 {
            s.update(0, 0);
            let open = s.world.get_block(2, 19) == BlockType::WoodDoor(true);
            changes += usize::from(open != was_open);
            was_open = open;
        }
        assert!(
            s.villagers
                .iter()
                .all(|v| v.x > 3.5 && v.x < 11.5 && (v.y - 20.0).abs() < 0.05),
            "{:?}",
            s.villagers
        );
        assert!(
            s.villagers
                .iter()
                .enumerate()
                .all(|(i, v)| s.villagers[i + 1..]
                    .iter()
                    .all(|other| (v.x - other.x).abs() >= 0.7)),
            "villagers stacked in the entrance: {:?}",
            s.villagers
        );
        assert!(
            changes <= 2,
            "door toggled {changes} times during one entry"
        );
        assert!(!was_open, "door stayed occupied all night");
    }
}

#[test]
fn ripe_wheat_supplies_food_and_surplus_seeds_while_early_harvest_does_not_multiply() {
    for stage in [0, 3, 7] {
        let mut s = flat();
        s.player.x = 0.5;
        s.world.set_block(1, 20, BlockType::Farmland(7));
        s.world.set_block(1, 19, BlockType::Crops(stage));
        s.interact_block(1, 19, true);
        let seeds = s
            .item_entities
            .iter()
            .filter(|i| i.item_type == ItemType::WheatSeeds)
            .count();
        let wheat = s
            .item_entities
            .iter()
            .filter(|i| i.item_type == ItemType::Wheat)
            .count();
        if stage == 7 {
            assert!((2..=3).contains(&seeds));
            assert_eq!(wheat, 1);
        } else {
            assert_eq!(seeds, 1);
            assert_eq!(wheat, 0);
        }
        assert_eq!(s.world.get_block(1, 20), BlockType::Farmland(7));
    }
}

#[test]
fn food_costs_for_idle_walking_sprinting_jumping_and_swimming_are_bounded() {
    for activity in 0..4 {
        let mut s = flat();
        for cx in 3..=10 {
            s.world
                .apply_chunk_column_snapshot(cx, &vec![BlockType::Air; CHUNK_WIDTH * CHUNK_HEIGHT]);
        }
        for x in 96..350 {
            s.world.set_block(x, 20, BlockType::Stone);
        }
        if activity > 0 {
            s.moving_right = true;
        }
        if activity == 2 {
            s.sprinting = true;
            s.sprint_direction = 1;
        }
        for tick in 0..600 {
            if activity == 3 && tick % 20 == 0 {
                s.queue_jump();
            }
            s.update(0, 0);
        }
        let spent = 20.0 - s.player.hunger;
        assert!(
            spent > 0.0 && spent < [0.4, 0.85, 1.9, 1.9][activity],
            "activity {activity}: {spent} hunger in 30s"
        );
    }
    let mut p = Player::new(0.5, 20.0);
    for _ in 0..600 {
        p.jump(1.0, 0.0);
    }
    assert!(20.0 - p.hunger < 0.4, "held swimming spent a food bar");
}
