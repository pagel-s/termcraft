use super::*;
use rand::{SeedableRng, rngs::StdRng};

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
