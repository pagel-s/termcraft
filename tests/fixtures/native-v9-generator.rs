use termcraft::{
    engine::{command::ClientCommand, state::GameState},
    entities::{
        blaze::Blaze, fireball::Fireball, item_entity::ItemEntity, villager::Villager,
        zombie::Zombie,
    },
    world::item::ItemType,
};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let root = std::path::PathBuf::from(std::env::args().nth(1).unwrap());
    std::fs::create_dir_all(&root)?;
    let mut s = GameState::open_in_directory(&root)?;
    s.dismiss_startup_splash();
    if std::env::args().nth(2).as_deref() == Some("--export-server") {
        termcraft::engine::server_save::ServerSave::read(&s)?.write(&root)?;
        return Ok(());
    }

    s.apply_client_command(ClientCommand::ToggleRuleMobSpawning);
    let mut v = Villager::new(12.5, 64.0, 12, 63);
    v.health = 7.0;
    v.age = 1234;
    s.villagers.push(v);
    let mut z = Zombie::new(16.5, 64.0);
    z.health = 8.0;
    s.zombies.push(z);
    let mut i = ItemEntity::new(13.5, 62.0, ItemType::Coal);
    i.age = 432;
    i.vx = 0.03;
    s.item_entities.push(i);
    s.item_entities
        .push(ItemEntity::new(14.5, 62.0, ItemType::Diamond));
    let mut f = Fireball::new(15.5, 61.0, -0.3, 0.1);
    f.age = 33;
    s.fireballs.push(f);
    s.fireballs.push(Fireball::new(16.5, 60.0, 0.34, 0.0));
    s.try_persist_world_and_progress()?;
    s.apply_client_command(ClientCommand::TravelToNether);
    let mut b = Blaze::new(20.5, 60.0);
    b.health = 9.0;
    s.blazes.push(b);
    s.fireballs.push(Fireball::new(21.5, 59.0, -0.34, 0.0));
    s.item_entities
        .push(ItemEntity::new(22.5, 59.0, ItemType::BlazeRod));
    s.apply_client_command(ClientCommand::TravelToEnd);
    s.apply_client_command(ClientCommand::TravelToOverworld);
    s.try_persist_world_and_progress()?;
    let saved = serde_json::to_value(termcraft::engine::server_save::ServerSave::read(&s)?)?;
    std::fs::write(
        root.join("expected-runtime.json"),
        serde_json::to_vec_pretty(&saved["runtime"]["dimensions"])?,
    )?;
    Ok(())
}
