//! Server-owned entities for terminal clients.
use super::state::{GameState, LightningBolt};
use crate::entities::arrow::Arrow;
use crate::entities::blaze::Blaze;
use crate::entities::boat::Boat;
use crate::entities::chicken::Chicken;
use crate::entities::cow::Cow;
use crate::entities::creeper::Creeper;
use crate::entities::end_crystal::EndCrystal;
use crate::entities::ender_dragon::EnderDragon;
use crate::entities::enderman::Enderman;
use crate::entities::experience_orb::ExperienceOrb;
use crate::entities::fireball::Fireball;
use crate::entities::ghast::Ghast;
use crate::entities::item_entity::ItemEntity;
use crate::entities::ocelot::Ocelot;
use crate::entities::pig::Pig;
use crate::entities::sheep::Sheep;
use crate::entities::silverfish::Silverfish;
use crate::entities::skeleton::Skeleton;
use crate::entities::slime::Slime;
use crate::entities::spider::Spider;
use crate::entities::squid::Squid;
use crate::entities::villager::Villager;
use crate::entities::wolf::Wolf;
use crate::entities::zombie::Zombie;
use crate::entities::zombie_pigman::ZombiePigman;
use serde::{Deserialize, Serialize};
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RuntimeEntities {
    zombies: Vec<Zombie>,
    creepers: Vec<Creeper>,
    skeletons: Vec<Skeleton>,
    spiders: Vec<Spider>,
    silverfish: Vec<Silverfish>,
    slimes: Vec<Slime>,
    endermen: Vec<Enderman>,
    blazes: Vec<Blaze>,
    pigmen: Vec<ZombiePigman>,
    ghasts: Vec<Ghast>,
    cows: Vec<Cow>,
    sheep: Vec<Sheep>,
    pigs: Vec<Pig>,
    chickens: Vec<Chicken>,
    squids: Vec<Squid>,
    wolves: Vec<Wolf>,
    ocelots: Vec<Ocelot>,
    villagers: Vec<Villager>,
    boats: Vec<Boat>,
    item_entities: Vec<ItemEntity>,
    experience_orbs: Vec<ExperienceOrb>,
    arrows: Vec<Arrow>,
    fireballs: Vec<Fireball>,
    end_crystals: Vec<EndCrystal>,
    ender_dragon: Option<EnderDragon>,
    lightning_bolts: Vec<LightningBolt>,
}
impl RuntimeEntities {
    pub fn capture(state: &GameState, center: f64) -> Self {
        let mut remaining = 512usize;
        Self {
            zombies: state
                .zombies
                .iter()
                .filter(|e| (e.x - center).abs() <= 128.0)
                .take(remaining)
                .cloned()
                .inspect(|_| remaining -= 1)
                .collect(),
            creepers: state
                .creepers
                .iter()
                .filter(|e| (e.x - center).abs() <= 128.0)
                .take(remaining)
                .cloned()
                .inspect(|_| remaining -= 1)
                .collect(),
            skeletons: state
                .skeletons
                .iter()
                .filter(|e| (e.x - center).abs() <= 128.0)
                .take(remaining)
                .cloned()
                .inspect(|_| remaining -= 1)
                .collect(),
            spiders: state
                .spiders
                .iter()
                .filter(|e| (e.x - center).abs() <= 128.0)
                .take(remaining)
                .cloned()
                .inspect(|_| remaining -= 1)
                .collect(),
            silverfish: state
                .silverfish
                .iter()
                .filter(|e| (e.x - center).abs() <= 128.0)
                .take(remaining)
                .cloned()
                .inspect(|_| remaining -= 1)
                .collect(),
            slimes: state
                .slimes
                .iter()
                .filter(|e| (e.x - center).abs() <= 128.0)
                .take(remaining)
                .cloned()
                .inspect(|_| remaining -= 1)
                .collect(),
            endermen: state
                .endermen
                .iter()
                .filter(|e| (e.x - center).abs() <= 128.0)
                .take(remaining)
                .cloned()
                .inspect(|_| remaining -= 1)
                .collect(),
            blazes: state
                .blazes
                .iter()
                .filter(|e| (e.x - center).abs() <= 128.0)
                .take(remaining)
                .cloned()
                .inspect(|_| remaining -= 1)
                .collect(),
            pigmen: state
                .pigmen
                .iter()
                .filter(|e| (e.x - center).abs() <= 128.0)
                .take(remaining)
                .cloned()
                .inspect(|_| remaining -= 1)
                .collect(),
            ghasts: state
                .ghasts
                .iter()
                .filter(|e| (e.x - center).abs() <= 128.0)
                .take(remaining)
                .cloned()
                .inspect(|_| remaining -= 1)
                .collect(),
            cows: state
                .cows
                .iter()
                .filter(|e| (e.x - center).abs() <= 128.0)
                .take(remaining)
                .cloned()
                .inspect(|_| remaining -= 1)
                .collect(),
            sheep: state
                .sheep
                .iter()
                .filter(|e| (e.x - center).abs() <= 128.0)
                .take(remaining)
                .cloned()
                .inspect(|_| remaining -= 1)
                .collect(),
            pigs: state
                .pigs
                .iter()
                .filter(|e| (e.x - center).abs() <= 128.0)
                .take(remaining)
                .cloned()
                .inspect(|_| remaining -= 1)
                .collect(),
            chickens: state
                .chickens
                .iter()
                .filter(|e| (e.x - center).abs() <= 128.0)
                .take(remaining)
                .cloned()
                .inspect(|_| remaining -= 1)
                .collect(),
            squids: state
                .squids
                .iter()
                .filter(|e| (e.x - center).abs() <= 128.0)
                .take(remaining)
                .cloned()
                .inspect(|_| remaining -= 1)
                .collect(),
            wolves: state
                .wolves
                .iter()
                .filter(|e| (e.x - center).abs() <= 128.0)
                .take(remaining)
                .cloned()
                .inspect(|_| remaining -= 1)
                .collect(),
            ocelots: state
                .ocelots
                .iter()
                .filter(|e| (e.x - center).abs() <= 128.0)
                .take(remaining)
                .cloned()
                .inspect(|_| remaining -= 1)
                .collect(),
            villagers: state
                .villagers
                .iter()
                .filter(|e| (e.x - center).abs() <= 128.0)
                .take(remaining)
                .cloned()
                .inspect(|_| remaining -= 1)
                .collect(),
            boats: state
                .boats
                .iter()
                .filter(|e| (e.x - center).abs() <= 128.0)
                .take(remaining)
                .cloned()
                .inspect(|_| remaining -= 1)
                .collect(),
            item_entities: state
                .item_entities
                .iter()
                .filter(|e| (e.x - center).abs() <= 128.0)
                .take(remaining)
                .cloned()
                .inspect(|_| remaining -= 1)
                .collect(),
            experience_orbs: state
                .experience_orbs
                .iter()
                .filter(|e| (e.x - center).abs() <= 128.0)
                .take(remaining)
                .cloned()
                .inspect(|_| remaining -= 1)
                .collect(),
            arrows: state
                .arrows
                .iter()
                .filter(|e| (e.x - center).abs() <= 128.0)
                .take(remaining)
                .cloned()
                .inspect(|_| remaining -= 1)
                .collect(),
            fireballs: state
                .fireballs
                .iter()
                .filter(|e| (e.x - center).abs() <= 128.0)
                .take(remaining)
                .cloned()
                .inspect(|_| remaining -= 1)
                .collect(),
            end_crystals: state
                .end_crystals
                .iter()
                .filter(|e| (e.x - center).abs() <= 128.0)
                .take(remaining)
                .cloned()
                .inspect(|_| remaining -= 1)
                .collect(),
            ender_dragon: state.ender_dragon.clone(),
            lightning_bolts: state
                .lightning_bolts
                .iter()
                .filter(|e| (e.x as f64 - center).abs() <= 128.0)
                .take(remaining)
                .cloned()
                .inspect(|_| remaining -= 1)
                .collect(),
        }
    }
    pub fn apply(&self, state: &mut GameState) {
        state.zombies = self.zombies.clone();
        state.creepers = self.creepers.clone();
        state.skeletons = self.skeletons.clone();
        state.spiders = self.spiders.clone();
        state.silverfish = self.silverfish.clone();
        state.slimes = self.slimes.clone();
        state.endermen = self.endermen.clone();
        state.blazes = self.blazes.clone();
        state.pigmen = self.pigmen.clone();
        state.ghasts = self.ghasts.clone();
        state.cows = self.cows.clone();
        state.sheep = self.sheep.clone();
        state.pigs = self.pigs.clone();
        state.chickens = self.chickens.clone();
        state.squids = self.squids.clone();
        state.wolves = self.wolves.clone();
        state.ocelots = self.ocelots.clone();
        state.villagers = self.villagers.clone();
        state.boats = self.boats.clone();
        state.item_entities = self.item_entities.clone();
        state.experience_orbs = self.experience_orbs.clone();
        state.arrows = self.arrows.clone();
        state.fireballs = self.fireballs.clone();
        state.end_crystals = self.end_crystals.clone();
        state.ender_dragon = self.ender_dragon.clone();
        state.lightning_bolts = self.lightning_bolts.clone();
    }
}
