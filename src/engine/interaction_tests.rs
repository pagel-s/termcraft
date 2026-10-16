use super::*;
use rand::{SeedableRng, rngs::StdRng};
fn cave() -> GameState {
    let mut s = GameState::network_view();
    s.dismiss_startup_splash();
    s.time_of_day = 12000.0;
    for cx in -2..=2 {
        s.world
            .apply_chunk_column_snapshot(cx, &vec![BlockType::Air; CHUNK_WIDTH * CHUNK_HEIGHT]);
    }
    for x in -24..=24 {
        s.world.set_block(x, 55, BlockType::Stone);
        s.world.set_block(x, 60, BlockType::Stone);
    }
    s.player.x = 18.5;
    s.player.y = 60.0;
    s
}
#[test]
fn cave_spawning_requires_darkness_and_checks_the_local_floor() {
    let mut s = cave();
    assert_eq!(s.dark_spawn_floor_near(0, 60, 4, true), Some(59.9));
    s.world.set_block(1, 59, BlockType::Torch);
    assert_eq!(s.dark_spawn_floor_near(0, 60, 4, true), None);
    s.world.set_block(1, 59, BlockType::Air);
    s.world.set_block(0, 55, BlockType::Air);
    assert_eq!(s.dark_spawn_floor_near(0, 60, 4, true), None);
    assert_eq!(s.dark_spawn_floor_near(0, 60, 4, false), Some(59.9));
}
#[test]
fn dungeon_light_suppresses_new_spawns_and_mixed_local_cap_is_shared() {
    let mut s = cave();
    s.world.set_block(0, 59, BlockType::ZombieSpawner);
    let mut rng = StdRng::seed_from_u64(42);
    s.world.set_block(1, 59, BlockType::Torch);
    s.update_dungeon_spawners(&mut rng);
    assert!(s.zombies.is_empty());
    s.world.set_block(1, 59, BlockType::Air);
    s.dungeon_spawner_timer = 0;
    s.update_dungeon_spawners(&mut rng);
    assert_eq!(s.zombies.len(), 1);
    assert!(s.dungeon_spawner_timer >= 90);
    s.zombies.clear();
    s.skeletons = (0..4).map(|x| Skeleton::new(x as f64, 59.9)).collect();
    s.dungeon_spawner_timer = 0;
    s.update_dungeon_spawners(&mut rng);
    assert!(s.zombies.is_empty());
}
