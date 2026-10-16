//! Disposable, staged conservation fixture for cross-file crash testing.
//! Real serializers/checkpoint recovery, not a survival or performance benchmark.
use serde_json::{Value, json};
use std::{
    fs,
    io::{self, Write},
    path::Path,
};
use termcraft::{
    engine::state::GameState,
    entities::item_entity::ItemEntity,
    world::{Dimension, World, block::BlockType, item::ItemType},
    worlds::WorldLease,
};

fn prepare(root: &Path) -> Result<(), Box<dyn std::error::Error>> {
    fs::create_dir_all(root)?;
    let _lease = WorldLease::acquire(root)?;
    for (name, dimension) in dimensions() {
        fs::write(
            root.join(format!("{name}_world_seed.bin")),
            bincode::serialize(&[1337u32, 7331, 9917, 4444])?,
        )?;
        let mut world = World::open_in_directory(dimension, root)?;
        world.load_chunks_around(0);
        world.set_block(3, 95, BlockType::Chest);
        world
            .ensure_chest_inventory(3, 95, 27)
            .unwrap()
            .add_item(ItemType::Diamond, 4);
        world.load_chunks_around(65);
        world.set_block(65, 94, BlockType::Planks);
        world.try_save_all()?;
    }
    let mut state = GameState::open_in_directory(root)?;
    state.inventory.slots.fill(None);
    state.inventory.add_item(ItemType::Diamond, 4);
    state.item_entities.clear();
    state
        .item_entities
        .push(ItemEntity::new(12.5, 64.0, ItemType::Diamond));
    state.player.x = 0.5;
    state.player.y = 64.0;
    state.try_persist_world_and_progress()?;
    Ok(())
}

fn dimensions() -> [(&'static str, Dimension); 3] {
    [
        ("overworld", Dimension::Overworld),
        ("nether", Dimension::Nether),
        ("end", Dimension::End),
    ]
}

fn mutate_world(world: &mut World) -> io::Result<()> {
    world.load_chunks_around(0);
    assert!(
        world
            .chest_inventory_mut(3, 95)
            .unwrap()
            .remove_item(ItemType::Diamond, 1)
    );
    world.load_chunks_around(65);
    world.set_block(65, 94, BlockType::Cobblestone);
    world.load_chunks_around(4096);
    world.set_block(4096, 95, BlockType::GoldOre);
    world.try_save_all()
}

fn mutate(root: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let _lease = WorldLease::acquire(root)?;
    let mut state = GameState::open_in_directory(root)?;
    println!("ready");
    io::stdout().flush()?;
    let mut start = String::new();
    io::stdin().read_line(&mut start)?;
    mutate_world(&mut state.world)?;
    for dimension in [Dimension::Nether, Dimension::End] {
        mutate_world(&mut World::open_in_directory(dimension, root)?)?;
    }
    state.inventory.add_item(ItemType::Diamond, 4);
    state.item_entities.clear();
    state.player.x = 7.5;
    state.player.health = 13.0;
    state.try_persist_world_and_progress()?;
    println!("committed");
    Ok(())
}

fn inspect(root: &Path) -> Result<Value, Box<dyn std::error::Error>> {
    let _lease = WorldLease::acquire(root)?;
    let state = GameState::open_in_directory(root)?;
    let mut terrain = Vec::new();
    for (name, dimension) in dimensions() {
        let mut world = World::open_in_directory(dimension, root)?;
        world.load_chunks_around(0);
        let chest: u32 = world
            .chest_inventory(3, 95)
            .unwrap()
            .slots
            .iter()
            .flatten()
            .filter(|s| s.item_type == ItemType::Diamond)
            .map(|s| s.count)
            .sum();
        world.load_chunks_around(65);
        terrain.push(
            json!({"dimension":name,"chest":chest,"build":format!("{:?}", world.get_block(65,94)),
            "new_chunk":root.join(format!("{name}_chunk_128.bin")).exists()}),
        );
    }
    let diamonds: u32 = state
        .inventory
        .slots
        .iter()
        .flatten()
        .filter(|s| s.item_type == ItemType::Diamond)
        .map(|s| s.count)
        .sum();
    let loot = state
        .item_entities
        .iter()
        .filter(|s| s.item_type == ItemType::Diamond)
        .count();
    Ok(
        json!({"inventory":diamonds,"loot":loot,"x":state.player.x,"health":state.player.health,"terrain":terrain}),
    )
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args_os().collect();
    let mode = args
        .get(1)
        .and_then(|s| s.to_str())
        .ok_or("mode required")?;
    let root = Path::new(args.get(2).ok_or("disposable world directory required")?);
    match mode {
        "prepare" => prepare(root)?,
        "mutate" => mutate(root)?,
        "inspect" => println!("{}", inspect(root)?),
        _ => return Err("expected prepare, mutate or inspect".into()),
    }
    Ok(())
}
