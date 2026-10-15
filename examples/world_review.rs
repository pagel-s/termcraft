//! A current-renderer world atlas. Only disposable saves are opened.
//! Generated terrain, staged lights/actors and structure studies are labelled.
use serde_json::{Value, json};
use std::{
    fs,
    path::{Path, PathBuf},
};
use termcraft::{
    engine::{
        command::ClientCommand,
        state::{GameState, RemotePlayerState, WeatherType},
    },
    entities::{
        boat::Boat, end_crystal::EndCrystal, ender_dragon::EnderDragon, enderman::Enderman,
    },
    renderer::Renderer,
    world::{BiomeType, Dimension, END_TOWER_XS, World, block::BlockType, recipe::WorldRecipe},
    worlds::WorldStore,
};

use termcraft::world::item::ItemType;

struct Atlas {
    out: PathBuf,
    records: Vec<Value>,
}
impl Atlas {
    fn capture(
        &mut self,
        s: &GameState,
        id: &str,
        title: &str,
        category: &str,
        origin: &str,
        caption: &str,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut views = vec![("overview", 200, 60, 0)];
        if id.contains("front-preview") {
            views.push(("detail", 200, 100, 2));
        } else if s.ender_dragon.is_none() {
            views.push(("detail", 160, 48, 2));
        }
        if id.contains("panorama") {
            views.push(("panorama", 400, 80, 0));
        }
        if id.contains("vault") || id.contains("roof") {
            views.push(("tall", 200, 128, 0));
        }
        let mut images = Vec::new();
        for (view, width, height, zoom) in views {
            let mut r = Renderer::with_size(width, height);
            if id.contains("front-preview") {
                r.set_dragon_front_preview(true);
            }
            for _ in 0..zoom {
                r.toggle_zoom();
            }
            let mut ansi = Vec::new();
            r.render_to(s, 0.0, &mut ansi)?;
            let file = format!("{id}-{view}-{width}x{height}.ansi");
            fs::write(self.out.join(&file), ansi)?;
            images.push(json!({"view":view,"ansi":file,"width":width,"height":height}));
        }
        self.records.push(json!({"id":id,"title":title,"dimension":format!("{:?}",s.current_dimension),"category":category,"origin":origin,"caption":caption,"x":s.player.x,"y":s.player.y,"images":images,"source":"examples/world_review.rs"}));
        Ok(())
    }
}
fn seeded(root: &Path) -> Result<GameState, Box<dyn std::error::Error>> {
    fs::create_dir_all(root)?;
    let bytes = bincode::serialize(&[1337u32, 7331, 9917, 4444])?;
    for d in ["overworld", "nether", "end"] {
        fs::write(root.join(format!("{d}_world_seed.bin")), &bytes)?;
    }
    let mut s = GameState::open_in_directory(root)?;
    s.dismiss_startup_splash();
    Ok(s)
}
fn stand(world: &World, x: i32, target: i32) -> Option<i32> {
    (3..125)
        .filter(|&y| {
            let floor = world.get_block(x, y);
            floor.is_solid()
                && !matches!(
                    floor,
                    BlockType::Wood | BlockType::BirchWood | BlockType::Cactus
                )
                && (1..=2).all(|dy| {
                    let b = world.get_block(x, y - dy);
                    !b.is_solid()
                        && !b.is_fluid()
                        && !matches!(b, BlockType::Leaves | BlockType::BirchLeaves)
                })
        })
        .min_by_key(|y| (y - target).abs())
}
fn position(s: &mut GameState, x: i32, target: i32) -> Result<(), Box<dyn std::error::Error>> {
    s.world.load_chunks_for_spawn_search(x, 64);
    let (x, y) = (-8i32..=8)
        .filter_map(|dx| {
            stand(&s.world, x + dx, target).map(|y| (x + dx, y, dx.abs() * 3 + (y - target).abs()))
        })
        .min_by_key(|v| v.2)
        .map(|(x, y, _)| (x, y))
        .ok_or("no clear camera/player position")?;
    s.player.x = x as f64 + 0.5;
    s.player.y = y as f64;
    s.player.vx = 0.0;
    s.player.vy = 0.0;
    s.player.grounded = true;
    Ok(())
}
fn light(s: &mut GameState) -> bool {
    let x = s.player.x.floor() as i32;
    for dx in [3, -3, 2, -2] {
        let bx = x + dx;
        if let Some(y) = stand(&s.world, bx, s.player.y as i32)
            && (y - s.player.y as i32).abs() < 4
            && s.world.get_block(bx, y - 1) == BlockType::Air
        {
            s.world.set_block(bx, y - 1, BlockType::Torch);
            return true;
        }
    }
    false
}
fn switch(
    s: &mut GameState,
    dim: Dimension,
    root: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    s.current_dimension = dim;
    s.world = World::open_in_directory(dim, root)?;
    s.endermen.clear();
    s.end_crystals.clear();
    s.ender_dragon = None;
    s.boats.clear();
    s.remote_players.clear();
    s.weather = WeatherType::Clear;
    s.weather_rain_intensity = 0.0;
    Ok(())
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    use BlockType::*;
    let out = PathBuf::from(std::env::args().nth(1).ok_or("output directory required")?);
    fs::create_dir_all(&out)?;
    let root = std::env::temp_dir().join(format!(
        "termcraft-world-atlas-{}-{}",
        std::process::id(),
        rand::random::<u64>()
    ));
    let mut s = seeded(&root)?;
    let mut atlas = Atlas {
        out,
        records: Vec::new(),
    };
    let mut missing = Vec::new();
    let biomes = [
        BiomeType::Plains,
        BiomeType::Forest,
        BiomeType::Desert,
        BiomeType::Tundra,
        BiomeType::Taiga,
        BiomeType::Swamp,
        BiomeType::Jungle,
        BiomeType::ExtremeHills,
        BiomeType::Ocean,
        BiomeType::River,
    ];
    let mut plains = (0, 34);
    let mut tundra = (0, 34);
    for biome in biomes {
        let candidate = (-60_000i32..=60_000)
            .step_by(16)
            .filter(|&x| s.world.get_biome(x) == biome)
            .max_by_key(|&x| {
                (
                    (-16..=16)
                        .step_by(4)
                        .filter(|dx| s.world.get_biome(x + dx) == biome)
                        .count(),
                    -x.abs(),
                )
            });
        let Some(x) = candidate else {
            missing.push(format!("Biome {biome:?}"));
            continue;
        };
        s.world.load_chunks_for_spawn_search(x, 64);
        s.time_of_day = 11000.0;
        if matches!(
            biome,
            BiomeType::Ocean | BiomeType::River | BiomeType::Swamp
        ) && let Some(y) = (3..90).find(|&y| matches!(s.world.get_block(x, y), Water(_)))
        {
            s.player.x = x as f64 + 0.5;
            s.player.y = y as f64;
            s.player.grounded = false;
            s.boats = vec![Boat::new(s.player.x, s.player.y + 0.4)];
        } else {
            position(&mut s, x, 36)?;
        }
        if biome == BiomeType::Plains {
            plains = (s.player.x.floor() as i32, s.player.y as i32);
        }
        if biome == BiomeType::Tundra {
            tundra = (s.player.x.floor() as i32, s.player.y as i32);
        }
        let id = format!("biome-{biome:?}").to_lowercase();
        atlas.capture(&s,&id,&format!("{biome:?}"),"Biomes","Generated", "Native seeded terrain. Player/camera position is staged; water scenes also stage a boat.")?;
        s.boats.clear();
    }
    position(&mut s, plains.0, plains.1)?;
    for (id, title, time) in [
        ("dawn", "Dawn", 5500.0),
        ("sunset", "Sunset", 17500.0),
        ("moonlight", "Moonlit exploration", 23000.0),
    ] {
        s.time_of_day = time;
        atlas.capture(
            &s,
            id,
            title,
            "Weather & time",
            "Generated",
            "The same native Plains location at a staged time of day.",
        )?;
    }
    s.time_of_day = 11000.0;
    s.weather = WeatherType::Thunderstorm;
    s.weather_rain_intensity = 1.0;
    atlas.capture(
        &s,
        "thunderstorm",
        "Thunderstorm",
        "Weather & time",
        "Staged weather",
        "Native terrain with a staged thunderstorm; no live weather event is claimed.",
    )?;
    s.weather = WeatherType::Rain;
    position(&mut s, tundra.0, tundra.1)?;
    atlas.capture(
        &s,
        "tundra-snow",
        "Snow in the tundra",
        "Weather & time",
        "Staged weather",
        "Native cold biome with rain intensity set for a snow presentation review.",
    )?;
    s.weather = WeatherType::Clear;
    s.weather_rain_intensity = 0.0;
    position(&mut s, plains.0, plains.1)?;
    let px = s.player.x.floor() as i32;
    s.remote_players = [(-4, 1u16), (4, 2)]
        .into_iter()
        .map(|(dx, id)| RemotePlayerState {
            client_id: id,
            x: (px + dx) as f64 + 0.5,
            y: stand(&s.world, px + dx, plains.1).unwrap_or(plains.1) as f64,
            vx: 0.0,
            vy: 0.0,
            grounded: true,
            facing_right: dx < 0,
            sneaking: false,
            health: 20.0,
            max_health: 20.0,
            hunger: 20.0,
            max_hunger: 20.0,
        })
        .collect();
    atlas.capture(&s,"coop","Building with friends","Play scenarios","Staged actors", "Native terrain with two remote-player render states. This is a composition review, not a network session recording.")?;
    s.remote_players.clear();
    for (item, count) in [
        (ItemType::DiamondSword, 1),
        (ItemType::DiamondPickaxe, 1),
        (ItemType::Torch, 24),
        (ItemType::Bread, 8),
        (ItemType::Cobblestone, 32),
    ] {
        s.inventory.add_item(item, count);
    }
    s.apply_client_command(ClientCommand::ToggleInventory);
    atlas.capture(
        &s,
        "inventory",
        "Inventory and equipment",
        "Play scenarios",
        "Staged UI",
        "Inventory opened with a staged equipment loadout; ordinary item icon rendering.",
    )?;
    s.apply_client_command(ClientCommand::ToggleInventory);
    s.chat.open = true;
    s.chat.draft = "Create a moonlit forest and a challenging battle nearby".into();
    atlas.capture(&s,"game-master-chat","Game-master chat","Play scenarios","Staged UI", "An unsent chat draft. No model request, response or generated world edit is being simulated.")?;
    s.chat.open = false;
    s.chat.draft.clear();
    s.inventory.slots.fill(None);
    let portal_root = root.join("portal-study");
    let mut portal = seeded(&portal_root)?;
    portal.world.load_chunks_for_spawn_search(0, 64);
    for x in -40..=40 {
        for y in 0..80 {
            portal
                .world
                .set_block(x, y, if y >= 60 { Grass } else { Air });
        }
    }
    for x in 2..=5 {
        for y in 55..=59 {
            portal.world.set_block(
                x,
                y,
                if x == 2 || x == 5 || y == 55 || y == 59 {
                    Obsidian
                } else {
                    NetherPortal
                },
            );
        }
    }
    for (x, block) in [(-4, CraftingTable), (-5, Furnace), (-6, Chest), (-7, Bed)] {
        portal.world.set_block(x, 59, block);
    }
    position(&mut portal, 0, 60)?;
    atlas.capture(&portal,"portal-workshop","Nether portal and player workshop","Play scenarios","Staged build","A hand-built portal block layout beside crafting, smelting, storage and a bed. This is a staged build, not a generated structure or travel recording.")?;
    drop(portal);
    // Find actual cave floors, including an ore-rich or vertically open location.
    let mut cave = None;
    let mut deep = None;
    let mut ravine = None;
    let mut best_ore = 0;
    for x in (-480..=480).step_by(8) {
        s.world.load_chunks_for_spawn_search(x, 24);
        for y in 52..119 {
            if !s.world.get_block(x, y).is_solid()
                || !(1..=4).all(|dy| s.world.get_block(x, y - dy) == Air)
            {
                continue;
            }
            let ore = (-7..=7)
                .flat_map(|dx| (-9..=3).map(move |dy| (dx, dy)))
                .filter(|&(dx, dy)| {
                    matches!(
                        s.world.get_block(x + dx, y + dy),
                        IronOre | GoldOre | CoalOre | DiamondOre | RedstoneOre
                    )
                })
                .count();
            if ore > best_ore {
                best_ore = ore;
                cave = Some((x, y));
            }
            if y > 95 && deep.is_none() {
                deep = Some((x, y));
            }
            if (1..=16).all(|dy| s.world.get_block(x, y - dy) == Air) && ravine.is_none() {
                ravine = Some((x, y));
            }
        }
    }
    for (id, title, location) in [
        ("ore-cave", "Ore-bearing cave", cave),
        ("deep-cave", "Deep underground", deep),
        ("ravine-cave", "Vertical cave / ravine", ravine),
    ] {
        if let Some((x, y)) = location {
            position(&mut s, x, y)?;
            let lit = light(&mut s);
            atlas.capture(&s,id,title,"Underground",if lit {"Generated + light"}else{"Generated"},"Actual seeded cave geometry; a temporary supported torch is added when possible for inspection.")?;
            if id == "ore-cave" {
                s.inventory.add_item(ItemType::IronPickaxe, 1);
                s.player.mining_timer = 0.5;
                s.player.last_mine_x = s.player.x.floor() as i32;
                s.player.last_mine_y = s.player.y.floor() as i32;
                atlas.capture(&s,"mining","Mining underground","Play scenarios","Staged action","Actual cave terrain with a supported inspection torch and a staged pickaxe/mining pose. This is an art inspection, not a recorded mining session.")?;
                s.player.mining_timer = 0.0;
                s.inventory.slots.fill(None);
            }
        } else {
            missing.push(title.into());
        }
    }
    let mut dungeon_types = std::collections::HashSet::new();
    for chunk in (2..42).step_by(5) {
        let hash = (chunk as u32)
            .wrapping_mul(1_103_515_245)
            .wrapping_add(12_345);
        let x = chunk * 32 + 8 + (hash as usize % 16) as i32;
        s.world.load_chunks_for_spawn_search(x, 32);
        if let Some((y, name)) = (40..124).find_map(|y| match s.world.get_block(x, y) {
            ZombieSpawner => Some((y, "Zombie")),
            SkeletonSpawner => Some((y, "Skeleton")),
            _ => None,
        }) && dungeon_types.insert(name)
        {
            position(&mut s, x - 1, y + 1)?;
            light(&mut s);
            atlas.capture(&s,&format!("dungeon-{}",name.to_lowercase()),&format!("{name} dungeon"),"Underground","Generated + light","Native spawner room, chest and openings; player position and a supported inspection torch are staged.")?;
        }
        if dungeon_types.len() == 2 {
            break;
        }
    }
    for (id, title, x, y) in [
        ("stronghold-panorama", "Stronghold complex", 640, 91),
        ("stronghold-portal", "End portal chamber", 637, 91),
        ("stronghold-library", "Stronghold library", 598, 88),
        ("stronghold-silverfish", "Silverfish room", 671, 88),
        ("stronghold-staging", "Stronghold staging room", 681, 91),
        ("stronghold-upper", "Upper halls and ladder", 613, 86),
    ] {
        position(&mut s, x, y)?;
        atlas.capture(&s,id,title,"Stronghold","Generated","Native stronghold rooms, lights, doors and contents. The panorama provides the complete complex for proportion review.")?;
    }
    position(&mut s, 637, 91)?;
    for x in 639..=642 {
        for y in 85..=88 {
            s.world.set_block(
                x,
                y,
                if x == 639 || x == 642 || y == 85 || y == 88 {
                    EndPortalFrame { filled: true }
                } else {
                    EndPortal
                },
            );
        }
    }
    atlas.capture(&s,"stronghold-active-portal","Activated End portal — progression study","Stronghold","Staged progression","Generated portal chamber with frame/portal blocks explicitly staged in their activated state. This is a visual progression study, not a recorded activation playthrough.")?;
    switch(&mut s, Dimension::Nether, &root)?;
    position(&mut s, 0, 60)?;
    atlas.capture(&s,"nether-vault","Full-height Nether cavern","Nether terrain","Generated","Tall context includes the generated ceiling, glowstone and floor. Recessed backdrop rock is visual-only.")?;
    for (id, title, x, y) in [
        ("fortress-panorama", "Nether fortress complex", 528, 50),
        ("fortress-gate", "Fortress gatehouse", 528, 50),
        ("fortress-watchtower", "Fortress watchtower", 450, 50),
        ("fortress-bastion", "Blaze-spawner bastion", 487, 50),
        ("fortress-chapel", "Loot chapel", 471, 50),
        (
            "fortress-corridor",
            "Fortress corridor and supports",
            514,
            50,
        ),
    ] {
        position(&mut s, x, y)?;
        atlas.capture(&s,id,title,"Nether fortress","Generated","Native fortress geometry and contents at the fixed review seed, with unchanged walking lanes and support proportions.")?;
    }
    let mut shrine = None;
    let mut soul = None;
    let mut vent = None;
    for chunk in -40..40 {
        let start = chunk * 32;
        s.world.load_chunks_for_spawn_search(start, 24);
        for x in start..start + 32 {
            for y in 15..90 {
                let b = s.world.get_block(x, y);
                if b == Chest && !s.world.is_nether_fortress_zone(x, y) && shrine.is_none() {
                    shrine = Some((x, y + 1));
                }
                if b == SoulSand && s.world.get_block(x, y - 1) == Air && soul.is_none() {
                    soul = Some((x, y));
                }
                if matches!(b, Lava(_))
                    && y < 34
                    && !s.world.is_nether_fortress_zone(x, 50)
                    && vent.is_none()
                {
                    vent = Some((x, 60));
                }
            }
        }
        if shrine.is_some() && soul.is_some() && vent.is_some() {
            break;
        }
    }
    for (id, title, group, point) in [
        ("nether-shrine", "Route shrine", "Nether landmarks", shrine),
        ("soul-sand", "Soul-sand floor", "Nether terrain", soul),
        (
            "lava-vent-roof",
            "Lava vent and cavern roof",
            "Nether terrain",
            vent,
        ),
    ] {
        if let Some((x, y)) = point {
            position(&mut s, x, y)?;
            atlas.capture(&s,id,title,group,"Generated","Located in actual seeded chunks, outside the fortress. Tall context is available for the roof and vent scene.")?;
        } else {
            missing.push(title.into());
        }
    }
    // Presentation-only studies: native seed selection currently reaches only Shrine.
    for bridge in [true, false] {
        let study_root = root.join(if bridge {
            "bridge-study"
        } else {
            "reliquary-study"
        });
        let mut study = seeded(&study_root)?;
        switch(&mut study, Dimension::Nether, &study_root)?;
        study.world.load_chunks_for_spawn_search(0, 64);
        for x in -35i32..=35 {
            for y in 5..80 {
                study
                    .world
                    .set_block(x, y, if y >= 60 { Netherrack } else { Air });
            }
        }
        if bridge {
            for x in -6i32..=6 {
                for y in 52..=59 {
                    study.world.set_block(
                        x,
                        y,
                        if x.abs() >= 5 || y == 54 && x.abs() < 5 || y == 59 && x.abs() <= 3 {
                            StoneBricks
                        } else {
                            Air
                        },
                    );
                }
            }
            for x in -2..=2 {
                study.world.set_block(x, 60, Air);
                study.world.set_block(x, 61, Lava(8));
                study.world.set_block(x, 62, StoneBricks);
            }
            for x in [-5, 0, 5] {
                study.world.set_block(x, 55, Glowstone);
            }
            study.world.set_block(0, 58, Chest);
        } else {
            for x in -5i32..=5 {
                let roof = 54 + (x.abs() / 2).min(2);
                for y in roof..=58 {
                    study.world.set_block(
                        x,
                        y,
                        if x.abs() == 5 || y == roof {
                            StoneBricks
                        } else {
                            Air
                        },
                    );
                }
            }
            for x in [-3, 3] {
                study.world.set_block(x, 58, StoneBricks);
                study.world.set_block(x, 57, Glowstone);
            }
            study.world.set_block(2, 58, Chest);
        }
        position(&mut study, -1, if bridge { 59 } else { 60 })?;
        atlas.capture(&study,if bridge{"bridge-study"}else{"reliquary-study"},if bridge{"Route bridge — structure study"}else{"Reliquary — structure study"},"Nether landmarks","Structure study","Source-derived staged block study, not a natural spawn. The current marker-seed formula always selects Shrine; see the coverage notes.")?;
    }
    // The generated lava sea is an underground layer, not an open surface coast.
    position(&mut s, 0, 60)?;
    s.player.y = 109.0;
    s.player.grounded = false;
    atlas.capture(&s,"nether-lava-cutaway","Deep lava layer — inspection","Nether terrain","Inspection","Camera/player placed inside the generated deep lava layer. This is a world cutaway, not a safe playable spawn or an open lava coastline.")?;
    switch(&mut s, Dimension::End, &root)?;
    s.world.load_chunks_for_spawn_search(0, 150);
    s.end_crystals = END_TOWER_XS
        .iter()
        .filter_map(|&x| {
            (2..50)
                .find(|&y| s.world.get_block(x, y) == Glowstone)
                .map(|y| EndCrystal::new(x as f64 + 0.5, y as f64 - 1.0))
        })
        .collect();
    for (id, title, x, y) in [
        ("end-panorama", "End island and four towers", 7, 34),
        ("end-tower", "Obsidian tower and walking cutaway", 10, 34),
        ("end-return", "End return portal", 4, 34),
        ("end-edge", "Island edge and void", 39, 34),
    ] {
        position(&mut s, x, y)?;
        atlas.capture(&s,id,title,"End","Generated","Actual island, pillars, crystals and return portal. Player/camera and crystal entities are staged at generated anchors; the terrain is untouched.")?;
    }
    if let Some((x, y)) = (48..140).find_map(|x| {
        (25..60)
            .find(|&y| s.world.get_block(x, y) == EndStone && s.world.get_block(x, y - 1) == Air)
            .map(|y| (x, y))
    }) {
        position(&mut s, x, y)?;
        atlas.capture(&s,"end-outer","Outer End island scaffold","End","Generated","Sparse outer-island terrain in the current generator; this is a scaffold, not an End-city feature.")?;
    } else {
        missing.push("Outer End island: no island found for this fixed seed".into());
    }
    position(&mut s, 7, 34)?;
    s.endermen = vec![Enderman::new(s.player.x + 3.0, s.player.y)];
    s.ender_dragon = Some(EnderDragon::new(5.5, 25.0));
    atlas.capture(&s,"end-dragon","Dragon encounter — current art","End","Staged actors","Current playable side-view dragon over the real generated arena; creature positions and phase are staged for inspection.")?;
    atlas.capture(&s,"end-front-preview","Dragon encounter — front art preview","End","Art preview","Opt-in front-facing candidate over the actual generated arena. This is not the playable default, and frontal combat is not qualified.")?;
    let store = WorldStore::new(root.join("recipe"));
    let path = store.create("gallery", "Frostbound ruins", &WorldRecipe::example())?;
    let mut custom = GameState::open_in_directory(&path)?;
    custom.dismiss_startup_splash();
    position(&mut custom, 28, 64)?;
    atlas.capture(&custom,"recipe-tower","Frostbound ruins / abandoned tower","Custom worlds","Recipe world","The built-in declarative world recipe, with its authored tower and regional terrain. No LLM was called to create this gallery.")?;
    fs::write(
        atlas.out.join("catalog.json"),
        serde_json::to_vec_pretty(
            &json!({"native_seed":[1337,7331,9917,4444],"recipe_seed":42,"scenes":atlas.records,"missing":missing,"generation_issue":"Nether marker multiplier, offset and modulus are all divisible by 3; generated route variants therefore select only Shrine. Bridge/reliquary cards are source-derived staged studies."}),
        )?,
    )?;
    eprintln!(
        "Captured {} atlas scenes; gaps: {:?}",
        atlas.records.len(),
        missing
    );
    drop(custom);
    drop(s);
    fs::remove_dir_all(root)?;
    Ok(())
}
