//! Whole-entity visual audit through the real renderer; no user saves or model.
use std::{fs, path::PathBuf};
use termcraft::{
    engine::state::GameState,
    entities::{
        arrow::Arrow, blaze::Blaze, boat::Boat, chicken::Chicken, cow::Cow, creeper::Creeper,
        enderman::Enderman, experience_orb::ExperienceOrb, fireball::Fireball, ghast::Ghast,
        item_entity::ItemEntity, ocelot::Ocelot, pig::Pig, sheep::Sheep, silverfish::Silverfish,
        skeleton::Skeleton, slime::Slime, spider::Spider, squid::Squid, villager::Villager,
        wolf::Wolf, zombie::Zombie, zombie_pigman::ZombiePigman,
    },
    renderer::Renderer,
    world::{Dimension, World, block::BlockType, item::ItemType},
};

fn scene(kind: &str, root: &std::path::Path) -> std::io::Result<GameState> {
    let mut s = GameState::open_in_directory(root)?;
    s.dismiss_startup_splash();
    s.current_dimension = if kind == "nether" {
        Dimension::Nether
    } else {
        Dimension::Overworld
    };
    s.world = World::open_in_directory(s.current_dimension, root)?;
    s.world.load_chunks_for_spawn_search(0, 64);
    for x in -48..=48 {
        for y in 0..48 {
            s.world.set_block(x, y, BlockType::Air);
        }
    }
    for x in -48..=48 {
        for y in 34..42 {
            s.world.set_block(
                x,
                y,
                if kind == "nether" {
                    BlockType::Netherrack
                } else if y == 34 {
                    BlockType::Grass
                } else {
                    BlockType::Dirt
                },
            );
        }
    }
    s.player.x = 0.5;
    s.player.y = 34.0;
    s.player.grounded = true;
    s.time_of_day = if kind == "night" { 23000.0 } else { 12000.0 };
    match kind {
        "farm" | "farm_states" => {
            s.cows.push(Cow::new(-6.0, 34.0));
            s.sheep.push(Sheep::new(-3.0, 34.0));
            s.pigs.push(Pig::new(3.0, 34.0));
            s.chickens.push(Chicken::new(6.0, 34.0));
            s.wolves.push(Wolf::new(8.5, 34.0));
            s.ocelots.push(Ocelot::new(-9.0, 34.0));
        }
        "small" => {
            s.spiders.push(Spider::new(-6.0, 34.0));
            s.silverfish.push(Silverfish::new(-3.0, 34.0));
            s.slimes.push(Slime::new(3.0, 34.0, 1));
            s.slimes.push(Slime::new(6.0, 34.0, 2));
            s.squids.push(Squid::new(9.0, 34.0));
        }
        "nether" => {
            s.pigmen.push(ZombiePigman::new(-4.0, 34.0));
            s.blazes.push(Blaze::new(3.0, 33.0));
            s.ghasts.push(Ghast::new(7.0, 31.0));
            for x in -8..=-6 {
                s.world.set_block(x, 34, BlockType::Lava(0));
            }
        }
        "props" => {
            s.boats.push(Boat::new(-6.0, 34.0));
            s.item_entities
                .push(ItemEntity::new(-3.0, 33.0, ItemType::DiamondSword));
            s.experience_orbs.push(ExperienceOrb::new(3.0, 33.0, 5));
            s.fireballs.push(Fireball::new(6.0, 32.0, 0.0, 0.0));
            s.arrows.push(Arrow::new(9.0, 32.0, 0.3, 0.0));
        }
        _ => {
            s.zombies.push(Zombie::new(-6.0, 34.0));
            s.skeletons.push(Skeleton::new(-3.0, 34.0));
            s.creepers.push(Creeper::new(3.0, 34.0));
            s.endermen.push(Enderman::new(6.0, 34.0));
            s.villagers.push(Villager::new(9.0, 34.0, 9, 34));
        }
    }
    if kind == "poses" {
        s.player.sneaking = true;
        s.inventory.slots[0] = Some(termcraft::world::item::ItemStack {
            item_type: ItemType::DiamondSword,
            count: 1,
            durability: Some(100),
        });
        s.zombies[0].hit_timer = 6;
        s.skeletons[0].facing_right = false;
        s.creepers[0].charged = true;
        s.creepers[0].fuse_timer = 10;
        s.villagers[0].facing_right = false;
    }
    if kind == "farm_states" {
        s.sheep[0].sheared = true;
        s.wolves[0].aggressive_timer = 20;
    }
    if kind == "cave" {
        for x in -48..=48 {
            for y in 0..28 {
                s.world.set_block(x, y, BlockType::Stone);
            }
        }
        s.world.set_block(-7, 33, BlockType::Torch);
        for y in 28..34 {
            s.world.set_block(10, y, BlockType::Stone);
        }
    }
    Ok(s)
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output = PathBuf::from(std::env::args().nth(1).ok_or("output directory required")?);
    fs::create_dir_all(&output)?;
    let root = std::env::temp_dir().join(format!(
        "termcraft-entity-review-{}-{}",
        std::process::id(),
        rand::random::<u64>()
    ));
    fs::create_dir_all(&root)?;
    for name in ["overworld", "nether", "end"] {
        fs::write(
            root.join(format!("{name}_world_seed.bin")),
            bincode::serialize(&[42u32, 73, 99, 44])?,
        )?;
    }
    for kind in [
        "humanoids",
        "farm",
        "small",
        "nether",
        "props",
        "night",
        "cave",
        "poses",
        "farm_states",
    ] {
        let s = scene(kind, &root)?;
        for (w, h) in [(120, 40), (160, 48), (200, 60)] {
            let mut renderer = Renderer::with_size(w, h);
            let mut ansi = Vec::new();
            renderer.render_to(&s, 0.0, &mut ansi)?;
            fs::write(output.join(format!("{kind}-{w}x{h}.ansi")), ansi)?;
        }
    }
    fs::remove_dir_all(root)?;
    Ok(())
}
