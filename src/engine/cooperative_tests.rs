use super::*;
use crate::world::{
    block::BlockType,
    item::{ItemType, Recipe},
    recipe::{Terrain, WorldRecipe},
};
use crate::worlds::WorldStore;

pub(super) struct Fixture(std::path::PathBuf);
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
pub(super) fn fixture() -> (Fixture, ServerSimulation) {
    let root = std::env::temp_dir().join(format!("termcraft-coop-{}", token()));
    let mut recipe = WorldRecipe::example();
    recipe.terrain = Terrain::Plains;
    recipe.relief = 0;
    recipe.landmarks.clear();
    recipe.regions.clear();
    let path = WorldStore::new(root.clone())
        .create("test", "co-op fixture", &recipe)
        .unwrap();
    let mut state = GameState::open_in_directory(&path).unwrap();
    state.apply_client_command(ClientCommand::ToggleRuleMobSpawning);
    for x in -16..=48 {
        for y in 0..80 {
            state.world.set_block(x, y, BlockType::Air);
        }
        state.world.set_block(x, 80, BlockType::Stone);
    }
    state.world.newly_generated_chunks.clear();
    let mut sim = ServerSimulation::with_state(state);
    for (id, x) in [(1, 0.5), (2, 4.5)] {
        sim.register_client(id);
        sim.state.activate_peer(id);
        sim.state.player.x = x;
        sim.state.player.y = 80.0;
        sim.state.player.grounded = true;
    }
    sim.state.activate_peer(1);
    (Fixture(root), sim)
}
pub(super) fn command(sim: &mut ServerSimulation, id: u16, cmd: ClientCommand) {
    let sequence = sim
        .last_applied_sequence_by_client
        .get(&id)
        .copied()
        .unwrap_or(0)
        + 1;
    let mut input = ClientInputFrame::new(id, sequence, sim.tick() + 1, cmd);
    input.dimension = sim.state.peer_dimension(id).into();
    sim.enqueue_input(input);
    sim.step(0, 0);
}
pub(super) fn count(sim: &ServerSimulation, id: u16, item: ItemType) -> u32 {
    sim.state
        .peer_session(id)
        .unwrap()
        .inventory()
        .slots
        .iter()
        .flatten()
        .filter(|s| s.item_type == item)
        .map(|s| s.count)
        .sum()
}
#[test]
fn joining_and_promotion_never_copy_another_inventory() {
    let (_root, mut sim) = fixture();
    sim.state.inventory.add_item(ItemType::Diamond, 7);
    sim.register_client(3);
    assert_eq!(count(&sim, 3, ItemType::Diamond), 0);
    sim.state.activate_peer(2);
    sim.state.inventory.add_item(ItemType::Wood, 5);
    sim.state.player.health = 12.0;
    sim.unregister_client(1);
    assert_eq!(sim.primary_client_id, Some(2));
    assert_eq!(count(&sim, 2, ItemType::Wood), 5);
    assert_eq!(count(&sim, 2, ItemType::Diamond), 0);
    assert_eq!(sim.snapshot_for_client(2).unwrap().player.health, 12.0);
}
#[test]
fn shared_clock_entities_and_each_player_advance_once() {
    let (_root, mut sim) = fixture();
    sim.state
        .zombies
        .push(crate::entities::zombie::Zombie::new(15.5, 80.0));
    let time = sim.state.time_of_day;
    let a = sim.snapshot_for_client(1).unwrap().session.player().age;
    let b = sim.snapshot_for_client(2).unwrap().session.player().age;
    command(&mut sim, 2, ClientCommand::ToggleInventory);
    assert!(sim.snapshot_for_client(2).unwrap().inventory_open);
    assert_eq!(
        sim.snapshot_for_client(1).unwrap().session.player().age,
        a + 1
    );
    assert_eq!(
        sim.snapshot_for_client(2).unwrap().session.player().age,
        b + 1
    );
    assert_eq!(sim.state.zombies[0].age, 1);
    let step = sim.state.time_of_day - time;
    assert!(step > 0.0 && step < 40.0);
}
#[test]
fn contested_mine_pickup_craft_and_place_conserve_items() {
    let (_root, mut sim) = fixture();
    sim.state.world.set_block(2, 79, BlockType::Wood);
    for id in [1, 2] {
        command(&mut sim, id, ClientCommand::AimAt(2, 79));
        command(&mut sim, id, ClientCommand::SetPrimaryAction(true));
    }
    for tick in 0..150 {
        sim.step(2, 79);
        if tick % 10 == 0 {
            for id in [1, 2] {
                command(&mut sim, id, ClientCommand::AimAt(2, 79));
            }
        }
    }
    for id in [1, 2] {
        command(&mut sim, id, ClientCommand::SetPrimaryAction(false));
    }
    for _ in 0..30 {
        sim.step(2, 79);
    }
    assert_eq!(sim.state.world.get_block(2, 79), BlockType::Air);
    assert_eq!(
        count(&sim, 1, ItemType::Wood) + count(&sim, 2, ItemType::Wood),
        1
    );
    assert!(
        sim.state
            .item_entities
            .iter()
            .all(|e| e.item_type != ItemType::Wood)
    );
    let owner = if count(&sim, 1, ItemType::Wood) == 1 {
        1
    } else {
        2
    };
    let recipe = Recipe::all()
        .iter()
        .position(|r| r.result == ItemType::Planks)
        .unwrap();
    command(&mut sim, owner, ClientCommand::CraftRecipe(recipe as u16));
    assert_eq!(count(&sim, owner, ItemType::Wood), 0);
    assert_eq!(count(&sim, owner, ItemType::Planks), 4);
    command(&mut sim, owner, ClientCommand::UseAt(2, 79));
    assert_eq!(sim.state.world.get_block(2, 79), BlockType::Planks);
    assert_eq!(count(&sim, owner, ItemType::Planks), 3);
    let observer = if owner == 1 { 2 } else { 1 };
    assert_eq!(count(&sim, observer, ItemType::Planks), 0);
}
#[test]
fn contested_chest_transfer_is_shared_and_not_duplicated() {
    let (_root, mut sim) = fixture();
    sim.state.world.set_block(2, 79, BlockType::Chest);
    sim.state
        .world
        .ensure_chest_inventory(2, 79, 27)
        .unwrap()
        .add_item(ItemType::Diamond, 9);
    for id in [1, 2] {
        command(&mut sim, id, ClientCommand::UseAt(2, 79));
    }
    assert_eq!(
        sim.snapshot_for_client(2).unwrap().chest.unwrap().2.slots[0]
            .as_ref()
            .unwrap()
            .count,
        9
    );
    for id in [1, 2] {
        command(&mut sim, id, ClientCommand::InventoryClick(0, 2));
    }
    assert_eq!(
        count(&sim, 1, ItemType::Diamond) + count(&sim, 2, ItemType::Diamond),
        9
    );
    assert!(
        sim.state
            .world
            .chest_inventory(2, 79)
            .unwrap()
            .slots
            .iter()
            .all(Option::is_none)
    );
    assert!(
        sim.snapshot_for_client(2)
            .unwrap()
            .chest
            .unwrap()
            .2
            .slots
            .iter()
            .all(Option::is_none)
    );
}
#[test]
fn nearest_player_takes_mob_damage_and_respawn_keeps_shared_world() {
    let (_root, mut sim) = fixture();
    sim.state.activate_peer(2);
    sim.state.player.hunger = 0.0;
    sim.state
        .zombies
        .push(crate::entities::zombie::Zombie::new(4.6, 80.0));
    sim.step(0, 0);
    assert_eq!(sim.snapshot_for_client(1).unwrap().player.health, 20.0);
    assert!(sim.snapshot_for_client(2).unwrap().player.health < 20.0);
    sim.state.activate_peer(2);
    sim.state.player.health = 0.0;
    sim.state.world.set_block(12, 79, BlockType::Planks);
    sim.state.activate_peer(1);
    sim.state.inventory.add_item(ItemType::Diamond, 3);
    for _ in 0..45 {
        sim.step(0, 0);
    }
    assert!(sim.snapshot_for_client(2).unwrap().death_screen_active);
    command(&mut sim, 2, ClientCommand::RespawnFromDeathScreen);
    assert!(!sim.snapshot_for_client(2).unwrap().death_screen_active);
    assert_eq!(count(&sim, 1, ItemType::Diamond), 3);
    assert_eq!(sim.state.world.get_block(12, 79), BlockType::Planks);
    assert_eq!(sim.state.zombies.len(), 1);
}
#[test]
fn disconnect_purges_queued_actions_and_stale_input_releases_holds() {
    let (_root, mut sim) = fixture();
    command(&mut sim, 2, ClientCommand::SetMoveRight(true));
    for _ in 0..45 {
        sim.step(0, 0);
    }
    assert_eq!(sim.snapshot_for_client(2).unwrap().player.vx, 0.0);
    sim.enqueue_input(ClientInputFrame::new(
        2,
        99,
        sim.tick() + 10,
        ClientCommand::SetPrimaryAction(true),
    ));
    sim.unregister_client(2);
    for _ in 0..15 {
        sim.step(0, 0);
    }
    assert_eq!(sim.state.peer_ids(), vec![1]);
    assert!(
        sim.pending_inputs
            .values()
            .flatten()
            .all(|f| f.client_id != 2)
    );
}
#[test]
fn complete_records_reconnect_and_survive_restart() {
    let (_root, mut sim) = fixture();
    sim.unregister_client(1);
    sim.unregister_client(2);
    sim.load_persistent().unwrap();
    let welcome = sim.authenticate(11, None).unwrap();
    let other = sim.authenticate(12, None).unwrap();
    sim.state.activate_peer(11);
    sim.state.inventory.add_item(ItemType::Wood, 17);
    sim.state.inventory.slots[5] = Some(crate::world::item::ItemStack {
        item_type: ItemType::Bow,
        count: 1,
        durability: Some(123),
    });
    sim.state.player.health = 13.0;
    sim.state.activate_peer(12);
    sim.state.inventory.add_item(ItemType::Diamond, 4);
    assert!(sim.authenticate(13, Some(&welcome.token)).is_err());
    command(&mut sim, 11, ClientCommand::SetMoveRight(true));
    sim.unregister_client(11);
    sim.save().unwrap();
    sim.authenticate(15, Some(&welcome.token)).unwrap();
    assert_eq!(count(&sim, 15, ItemType::Wood), 17);
    assert_eq!(sim.snapshot_for_client(15).unwrap().player.vx, 0.0);
    sim.save().unwrap();
    let path = sim.state.world.save_root().to_path_buf();
    drop(sim);
    let mut restarted = ServerSimulation::with_state(GameState::open_in_directory(&path).unwrap());
    restarted.load_persistent().unwrap();
    assert_eq!(
        restarted
            .authenticate(1, Some(&welcome.token))
            .unwrap()
            .server_id,
        welcome.server_id
    );
    restarted.authenticate(2, Some(&other.token)).unwrap();
    assert_eq!(count(&restarted, 1, ItemType::Wood), 17);
    assert_eq!(count(&restarted, 2, ItemType::Diamond), 4);
    assert_eq!(
        restarted
            .snapshot_for_client(1)
            .unwrap()
            .session
            .inventory()
            .slots[5]
            .as_ref()
            .unwrap()
            .durability,
        Some(123)
    );
}
#[test]
fn corrupt_save_is_preserved_and_failed_writes_retain_player_data() {
    let (_root, mut sim) = fixture();
    sim.load_persistent().unwrap();
    sim.authenticate(1, None).unwrap();
    sim.state.activate_peer(1);
    sim.state.inventory.add_item(ItemType::Diamond, 8);
    let path = sim.state.world.save_root().join("server-state.json");
    std::fs::remove_file(&path).unwrap();
    std::fs::create_dir(&path).unwrap();
    assert!(sim.save().is_err());
    assert_eq!(count(&sim, 1, ItemType::Diamond), 8);
    std::fs::remove_dir(&path).unwrap();
    sim.save().unwrap();
    std::fs::write(&path, b"broken").unwrap();
    assert!(sim.load_persistent().is_err());
    assert_eq!(std::fs::read(path).unwrap(), b"broken");
}
#[test]
fn remote_commands_cannot_change_global_rules_or_dimensions() {
    let (_root, mut sim) = fixture();
    let difficulty = sim.state.difficulty_name();
    for cmd in [
        ClientCommand::TravelToNether,
        ClientCommand::TravelToEnd,
        ClientCommand::EquipDiamondLoadout,
        ClientCommand::CycleDifficulty,
    ] {
        command(&mut sim, 2, cmd);
    }
    assert_eq!(
        sim.state.current_dimension,
        crate::world::Dimension::Overworld
    );
    assert_eq!(sim.state.difficulty_name(), difficulty);
    assert_eq!(count(&sim, 2, ItemType::DiamondSword), 0);
}

#[test]
fn world_physics_runs_near_distant_peers_without_double_ticks() {
    let (_root, mut sim) = fixture();
    sim.state.activate_peer(2);
    sim.state.player.x = 640.5;
    sim.state.player.y = 40.0;
    sim.state.world.load_chunks_around(640);
    for x in [10, 650] {
        for y in 0..80 {
            sim.state.world.set_block(x, y, BlockType::Air);
        }
        sim.state.world.set_block(x, 60, BlockType::Sand);
    }
    sim.step(0, 0);
    sim.step(0, 0);
    for x in [10, 650] {
        assert_eq!(sim.state.world.get_block(x, 60), BlockType::Air);
        assert_eq!(sim.state.world.get_block(x, 61), BlockType::Sand);
    }
}
