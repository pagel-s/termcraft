use super::*;
use rand::{SeedableRng, rngs::StdRng};

#[test]
fn native_generated_nights_produce_surface_enemies_across_multiple_seeds() {
    for seed in [1337u32, 42, 73, 2026] {
        let root = std::env::temp_dir().join(format!(
            "termcraft-natural-night-{seed}-{}",
            rand::random::<u64>()
        ));
        std::fs::create_dir_all(&root).unwrap();
        let seeds = (
            seed,
            seed.wrapping_add(5994),
            seed.wrapping_add(8580),
            seed.wrapping_add(71),
        );
        std::fs::write(
            root.join("overworld_world_seed.bin"),
            bincode::serialize(&seeds).unwrap(),
        )
        .unwrap();
        let mut s = GameState::open_in_directory(&root).unwrap();
        s.dismiss_startup_splash();
        s.time_of_day = 23000.0;
        s.world.load_chunks_around(s.player.x.floor() as i32);
        let mut rng = StdRng::seed_from_u64(seed as u64);
        for _ in 0..40 {
            s.overworld_hostile_spawn_timer = 0;
            s.update_ambient_respawns(&mut rng, false);
        }
        let surface = s
            .zombies
            .iter()
            .map(|m| (m.x, m.y))
            .chain(s.creepers.iter().map(|m| (m.x, m.y)))
            .chain(s.skeletons.iter().map(|m| (m.x, m.y)))
            .chain(s.spiders.iter().map(|m| (m.x, m.y)))
            .filter(|(x, y)| {
                s.world
                    .has_open_sky(x.floor() as i32, (y - 1.0).floor() as i32)
            })
            .count();
        assert!(
            surface > 0,
            "seed {seed}: no surface enemies near spawn ({},{})",
            s.player.x,
            s.player.y
        );
        drop(s);
        std::fs::remove_dir_all(root).unwrap();
    }
}

fn surface_with_cave() -> GameState {
    let mut s = GameState::network_view();
    s.dismiss_startup_splash();
    s.time_of_day = 23000.0;
    for cx in -3..=3 {
        s.world
            .apply_chunk_column_snapshot(cx, &vec![BlockType::Air; CHUNK_WIDTH * CHUNK_HEIGHT]);
    }
    for x in -96..=96 {
        s.world.set_block(x, 32, BlockType::Grass);
        for y in 33..=55 {
            s.world.set_block(x, y, BlockType::Stone);
        }
        s.world.set_block(x, 60, BlockType::Stone);
    }
    s.player.x = 0.5;
    s.player.y = 31.9;
    s
}

#[test]
fn co_op_surface_spawns_continue_when_the_first_player_is_underground() {
    let mut s = surface_with_cave();
    s.join_peer(1);
    s.player.x = 0.5;
    s.player.y = 60.0;
    s.join_peer(2);
    s.activate_peer(2);
    s.player.x = 40.5;
    s.player.y = 32.0;
    s.activate_peer(1);
    let mut sim = crate::engine::net::ServerSimulation::with_state(s);
    for _ in 0..120 {
        sim.step(0, 0);
    }
    let s = sim.state();
    assert!(
        s.zombies.iter().any(|m| m.y < 40.0)
            || s.creepers.iter().any(|m| m.y < 40.0)
            || s.skeletons.iter().any(|m| m.y < 40.0)
            || s.spiders.iter().any(|m| m.y < 40.0)
    );
}

#[test]
fn outdoor_mobs_accept_vegetation_and_slopes_without_a_player_spawn_runway() {
    let mut s = surface_with_cave();
    for x in -96i32..=96 {
        let ground = 30 + x.rem_euclid(3);
        for y in 25..=35 {
            s.world.set_block(
                x,
                y,
                if y >= ground {
                    BlockType::Stone
                } else {
                    BlockType::Air
                },
            );
        }
        s.world.set_block(x, ground, BlockType::Grass);
        s.world.set_block(
            x,
            ground - 1,
            if x % 2 == 0 {
                BlockType::TallGrass
            } else {
                BlockType::YellowFlower
            },
        );
    }
    assert!(s.find_dark_overworld_spawn(40).is_some());
    let mut rng = StdRng::seed_from_u64(99);
    for _ in 0..12 {
        s.overworld_hostile_spawn_timer = 0;
        s.update_ambient_respawns(&mut rng, false);
    }
    assert!(
        !s.zombies.is_empty()
            || !s.creepers.is_empty()
            || !s.skeletons.is_empty()
            || !s.spiders.is_empty()
    );
}

#[test]
fn night_surface_spawns_are_not_starved_by_daytime_cave_population() {
    for cave_floor in [43, 60] {
        let mut s = surface_with_cave();
        for x in -96..=96 {
            for y in cave_floor - 4..cave_floor {
                s.world.set_block(x, y, BlockType::Air);
            }
            s.world.set_block(x, cave_floor, BlockType::Stone);
        }
        let cave_count =
            GameState::spawn_refill_threshold(s.scaled_hostile_cap(OVERWORLD_HOSTILE_CAP), 2, 3);
        s.zombies = (0..cave_count)
            .map(|i| Zombie::new(40.5 + i as f64, cave_floor as f64 - 0.1))
            .collect();
        let mut rng = StdRng::seed_from_u64(42);
        for _ in 0..8 {
            s.overworld_hostile_spawn_timer = 0;
            s.update_ambient_respawns(&mut rng, false);
        }
        let surface_mobs = s.zombies.iter().filter(|m| m.y < 40.0).count()
            + s.creepers.iter().filter(|m| m.y < 40.0).count()
            + s.skeletons.iter().filter(|m| m.y < 40.0).count()
            + s.spiders.iter().filter(|m| m.y < 40.0).count();
        assert!(
            surface_mobs > 0,
            "the night surface stayed empty despite dark eligible land"
        );
        assert!(
            s.zombies.len() + s.creepers.len() + s.skeletons.len() + s.spiders.len()
                <= OVERWORLD_HOSTILE_CAP
        );
    }
}

#[test]
fn surface_floor_obeys_night_day_light_and_peaceful_rules() {
    let mut s = surface_with_cave();
    assert_eq!(s.find_dark_overworld_spawn(40), Some(31.9));
    s.world.set_block(40, 31, BlockType::Torch);
    assert_ne!(s.find_dark_overworld_spawn(40), Some(31.9));
    s.world.set_block(40, 31, BlockType::Air);
    s.time_of_day = 12000.0;
    assert_ne!(s.find_dark_overworld_spawn(40), Some(31.9));
    s.time_of_day = 23000.0;
    s.difficulty = Difficulty::Peaceful;
    s.overworld_hostile_spawn_timer = 0;
    s.update_ambient_respawns(&mut StdRng::seed_from_u64(42), false);
    assert!(
        s.zombies.is_empty()
            && s.creepers.is_empty()
            && s.skeletons.is_empty()
            && s.spiders.is_empty()
    );
}

#[test]
fn outdoor_night_attempts_do_not_substitute_caves_under_water() {
    let mut s = surface_with_cave();
    for y in 28..32 {
        s.world.set_block(30, y, BlockType::Water(8));
    }
    assert_eq!(s.find_dark_overworld_spawn(30), None);
    s.player.y = 59.9;
    assert_eq!(s.find_dark_overworld_spawn(30), Some(59.9));
}

#[test]
fn night_spawns_keep_a_safe_gap_but_can_enter_normal_awareness() {
    let mut s = surface_with_cave();
    let mut rng = StdRng::seed_from_u64(99);
    for _ in 0..16 {
        s.overworld_hostile_spawn_timer = 0;
        s.update_ambient_respawns(&mut rng, false);
    }
    assert!(s.zombies.iter().any(|m| (m.x - s.player.x).abs() < 20.0));
    for (x, y) in s
        .zombies
        .iter()
        .map(|m| (m.x, m.y))
        .chain(s.creepers.iter().map(|m| (m.x, m.y)))
        .chain(s.skeletons.iter().map(|m| (m.x, m.y)))
        .chain(s.spiders.iter().map(|m| (m.x, m.y)))
    {
        assert!(!s.is_spawn_too_close_to_player(x, y, OVERWORLD_HOSTILE_SPAWN_MIN_DIST_SQ));
    }
}

#[test]
fn a_distant_players_surface_mobs_do_not_suppress_local_night_refill() {
    let mut s = surface_with_cave();
    // The distant player's mobs are still resident, but outside this player's
    // 40-block spawning neighbourhood. Keep the existing total hard cap.
    let soft_cap = GameState::spawn_refill_threshold(OVERWORLD_HOSTILE_CAP, 2, 3);
    s.zombies = (0..soft_cap)
        .map(|i| Zombie::new(65.5 + i as f64, 31.9))
        .collect();
    assert!(s.can_refill_overworld_hostiles());
    s.zombies
        .extend((soft_cap..OVERWORLD_HOSTILE_CAP).map(|i| Zombie::new(65.5 + i as f64, 31.9)));
    assert!(
        !s.can_refill_overworld_hostiles(),
        "the global safety cap must stay in force"
    );
}

#[test]
fn thirty_seconds_of_unforced_night_spawning_remain_bounded() {
    for seed in [42, 73, 1337, 2026] {
        let mut s = surface_with_cave();
        let mut rng = StdRng::seed_from_u64(seed);
        for _ in 0..600 {
            s.update_ambient_respawns(&mut rng, false);
        }
        let mobs: Vec<_> = s
            .zombies
            .iter()
            .map(|m| (m.x, m.y))
            .chain(s.creepers.iter().map(|m| (m.x, m.y)))
            .chain(s.skeletons.iter().map(|m| (m.x, m.y)))
            .chain(s.spiders.iter().map(|m| (m.x, m.y)))
            .collect();
        let aware = mobs
            .iter()
            .filter(|(x, _)| (x - s.player.x).abs() < 20.0)
            .count();
        eprintln!(
            "night seed {seed}: {} surface mobs after 30s; {aware} inside 20 blocks",
            mobs.len()
        );
        assert!(mobs.len() >= 4 && mobs.len() <= 9);
        assert!(mobs.iter().all(|&(x, y)| !s.is_spawn_too_close_to_player(
            x,
            y,
            OVERWORLD_HOSTILE_SPAWN_MIN_DIST_SQ
        )));
    }
}

#[test]
fn multiplayer_night_recovers_from_a_full_persisted_cave_population() {
    let mut s = surface_with_cave();
    s.zombies = (0..OVERWORLD_HOSTILE_CAP)
        .map(|i| {
            let mut m = Zombie::new(-12.0 + i as f64, 59.9);
            m.age = 600;
            m
        })
        .collect();
    s.respawn_grace_ticks = 1200;
    let mut sim = crate::engine::net::ServerSimulation::with_state(s);
    sim.register_client(1);
    sim.register_client(2);
    let mut surface_peak = 0;
    for _ in 0..600 {
        sim.step(0, 0);
        let s = sim.state();
        let mobs: Vec<_> = s
            .zombies
            .iter()
            .map(|m| (m.x, m.y))
            .chain(s.creepers.iter().map(|m| (m.x, m.y)))
            .chain(s.skeletons.iter().map(|m| (m.x, m.y)))
            .chain(s.spiders.iter().map(|m| (m.x, m.y)))
            .chain(s.slimes.iter().map(|m| (m.x, m.y)))
            .collect();
        assert!(mobs.len() <= OVERWORLD_HOSTILE_CAP);
        surface_peak = surface_peak.max(
            mobs.iter()
                .filter(|(x, y)| (x - s.player.x).abs() <= 40.0 && *y < 40.0)
                .count(),
        );
    }
    assert!(
        surface_peak >= 3,
        "a full cave population left both surface players without night enemies"
    );
}

fn full_old_cave_population() -> GameState {
    let mut s = surface_with_cave();
    s.world_tick = 1000;
    s.zombies = (0..s.scaled_hostile_cap(OVERWORLD_HOSTILE_CAP))
        .map(|i| {
            let mut m = Zombie::new(-12.0 + i as f64, 59.9);
            m.age = 600;
            m
        })
        .collect();
    s
}

#[test]
fn ambient_slot_replacement_preserves_every_players_visible_and_nearby_mobs() {
    let mut s = full_old_cave_population();
    assert!(s.inactive_overworld_hostile().is_some());
    // A second player in the cave protects those occupants from surface refill.
    s.join_peer(1);
    s.join_peer(2);
    s.activate_peer(2);
    s.player.y = 59.9;
    s.activate_peer(1);
    let before = s.zombies.clone();
    assert!(!s.reserve_overworld_hostile_slot());
    assert_eq!(s.zombies, before);

    // Keep a clear sight line to the mobs, beyond the unconditional 24-block gap.
    s.activate_peer(2);
    s.player.x = -50.5;
    s.activate_peer(1);
    assert!(!s.reserve_overworld_hostile_slot());
    assert_eq!(s.zombies, before);
}

#[test]
fn ambient_slot_replacement_preserves_new_recently_fought_and_managed_mobs() {
    for protection in 0..5 {
        let mut s = full_old_cave_population();
        for m in &mut s.zombies {
            match protection {
                0 => m.age = 199,
                1 => m.hit_timer = 1,
                2 => m.last_player_damage_tick = s.world_tick - 199,
                3 => m.director_event = 1,
                _ => m.attack_windup = 1,
            }
        }
        let before = s.zombies.clone();
        assert!(!s.reserve_overworld_hostile_slot());
        assert_eq!(s.zombies, before);
        assert!(s.item_entities.is_empty() && s.experience_orbs.is_empty());
    }
}

#[test]
fn invalid_surface_sites_do_not_evict_cave_mobs() {
    let mut s = full_old_cave_population();
    for x in -96..=96 {
        for y in 28..32 {
            s.world.set_block(x, y, BlockType::Water(8));
        }
    }
    let before = s.zombies.clone();
    let mut rng = StdRng::seed_from_u64(42);
    for _ in 0..12 {
        s.overworld_hostile_spawn_timer = 0;
        s.update_ambient_respawns(&mut rng, false);
    }
    assert_eq!(s.zombies, before);
    assert!(s.creepers.is_empty() && s.skeletons.is_empty() && s.spiders.is_empty());
}

#[test]
fn replacing_an_inactive_slot_respects_difficulty_caps_without_drops() {
    for difficulty in [Difficulty::Easy, Difficulty::Normal, Difficulty::Hard] {
        let mut s = full_old_cave_population();
        s.difficulty = difficulty;
        let cap = s.scaled_hostile_cap(OVERWORLD_HOSTILE_CAP);
        s.zombies.resize_with(cap, || {
            let mut m = Zombie::new(0.5, 59.9);
            m.age = 600;
            m
        });
        s.overworld_hostile_spawn_timer = 0;
        s.update_ambient_respawns(&mut StdRng::seed_from_u64(42), false);
        assert_eq!(
            s.zombies.len()
                + s.creepers.len()
                + s.skeletons.len()
                + s.spiders.len()
                + s.slimes.len(),
            cap
        );
        assert!(
            s.zombies.iter().any(|m| m.y < 40.0)
                || s.creepers.iter().any(|m| m.y < 40.0)
                || s.skeletons.iter().any(|m| m.y < 40.0)
                || s.spiders.iter().any(|m| m.y < 40.0)
        );
        assert!(s.item_entities.is_empty() && s.experience_orbs.is_empty());
    }
}
