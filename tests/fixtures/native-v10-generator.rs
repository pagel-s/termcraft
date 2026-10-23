// Previous-build save fixture generator, run with unmodified Termcraft at a371169.
// Use a disposable world and a separate target directory; never a player save.
use termcraft::{
    entities::{creeper::Creeper, skeleton::Skeleton, spider::Spider, silverfish::Silverfish, slime::Slime, enderman::Enderman, zombie_pigman::ZombiePigman, ghast::Ghast, cow::Cow, sheep::Sheep, pig::Pig, chicken::Chicken, squid::Squid, wolf::Wolf, ocelot::Ocelot, boat::Boat, experience_orb::ExperienceOrb, arrow::Arrow, end_crystal::EndCrystal, ender_dragon::EnderDragon},
    engine::state::LightningBolt,
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
    s.zombies.push(Zombie::new(12.5, 64.0));
    s.creepers.push(Creeper::new(13.5, 64.0));
    s.skeletons.push(Skeleton::new(14.5, 64.0));
    s.spiders.push(Spider::new(15.5, 64.0));
    s.silverfish.push(Silverfish::new(16.5, 64.0));
    s.slimes.push(Slime::new(17.5, 64.0, 2));
    s.endermen.push(Enderman::new(18.5, 64.0));
    s.blazes.push(Blaze::new(19.5, 64.0));
    s.pigmen.push(ZombiePigman::new(20.5, 64.0));
    s.ghasts.push(Ghast::new(21.5, 44.0));
    s.cows.push(Cow::new(22.5, 64.0));
    s.sheep.push(Sheep::new(23.5, 64.0));
    s.sheep[0].sheared = true;
    s.sheep[0].health = 5.0;
    s.pigs.push(Pig::new(24.5, 64.0));
    s.chickens.push(Chicken::new(25.5, 64.0));
    s.squids.push(Squid::new(26.5, 64.0));
    s.wolves.push(Wolf::new(27.5, 64.0));
    s.ocelots.push(Ocelot::new(28.5, 64.0));
    s.villagers.push(Villager::new(29.5, 64.0, 29, 63));
    s.boats.push(Boat::new(30.5, 64.0));
    s.item_entities
        .push(ItemEntity::new(31.5, 64.0, ItemType::Diamond));
    s.item_entities[0].age = 1234;
    s.experience_orbs.push(ExperienceOrb::new(32.5, 64.0, 7));
    s.experience_orbs[0].age = 432;
    s.arrows.push(Arrow::new_player(33.5, 62.0, 0.8, -0.1, 5.0));
    s.fireballs.push(Fireball::new(34.5, 60.0, -0.2, 0.1));
    s.end_crystals.push(EndCrystal::new(35.5, 44.0));
    s.ender_dragon = Some(EnderDragon::new(36.5, 32.0));
    s.lightning_bolts.push(LightningBolt {
        x: 37,
        y_top: 0,
        y_bottom: 64,
        ttl: 2,
    });
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
    s.inventory.slots[0] = Some(termcraft::world::item::ItemStack { item_type: ItemType::DiamondSword, count:1, durability:Some(37) });
    s.hotbar_index=0;
    s.apply_client_command(ClientCommand::DropItem(true));
    s.fireballs.last_mut().unwrap().reflected=true;
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