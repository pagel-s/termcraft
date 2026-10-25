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
use crate::entities::ender_pearl::EnderPearl;
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
    end_victory: Option<(u16, f64, f64)>,
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
    ender_pearls: Vec<EnderPearl>,
    fireballs: Vec<Fireball>,
    end_crystals: Vec<EndCrystal>,
    ender_dragon: Option<EnderDragon>,
    lightning_bolts: Vec<LightningBolt>,
}
impl RuntimeEntities {
    pub fn capture(state: &GameState, center: f64) -> Self {
        let mut remaining = 512usize;
        Self {
            end_victory: state.end_victory_sequence_state(),
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
            ender_pearls: state
                .ender_pearls
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
        state.set_replicated_end_victory(self.end_victory);
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
        state.ender_pearls = self.ender_pearls.clone();
        state.fireballs = self.fireballs.clone();
        state.end_crystals = self.end_crystals.clone();
        state.ender_dragon = self.ender_dragon.clone();
        state.lightning_bolts = self.lightning_bolts.clone();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn flying_pearls_and_impact_effects_reach_other_players_without_connection_ownership() {
        let mut state = GameState::network_view();
        state
            .ender_pearls
            .push(EnderPearl::new(8.0, 20.0, (1.0, -0.2), Some(2)));
        let mut impact = EnderPearl::new(12.0, 22.0, (0.0, 0.0), Some(1));
        impact.impact_ticks = 6;
        state.ender_pearls.push(impact);
        let bytes = bincode::serialize(&RuntimeEntities::capture(&state, 0.0)).unwrap();
        let packet: RuntimeEntities = bincode::deserialize(&bytes).unwrap();
        let mut view = GameState::network_view();
        packet.apply(&mut view);
        assert_eq!(view.ender_pearls.len(), 2);
        assert_eq!(view.ender_pearls[0].vx, EnderPearl::SPEED);
        assert_eq!(view.ender_pearls[1].impact_ticks, 6);
        assert!(view.ender_pearls.iter().all(|p| p.owner.is_none()));
    }
    #[test]
    fn victory_and_reflected_fireballs_survive_the_binary_snapshot() {
        let mut s = GameState::new();
        s.set_replicated_end_victory(Some((90, 2.5, 27.0)));
        let mut f = Fireball::new(1.0, 20.0, 0.48, 0.0);
        f.reflected = true;
        s.fireballs.push(f.clone());
        let packet = RuntimeEntities::capture(&s, 0.0);
        let bytes = bincode::serialize(&packet).unwrap();
        let packet: RuntimeEntities = bincode::deserialize(&bytes).unwrap();
        let mut view = GameState::network_view();
        packet.apply(&mut view);
        assert_eq!(view.end_victory_sequence_state(), Some((90, 2.5, 27.0)));
        assert_eq!(view.fireballs, [f]);
        s.set_replicated_end_victory(None);
        RuntimeEntities::capture(&s, 0.0).apply(&mut view);
        assert_eq!(view.end_victory_sequence_state(), None);
    }
}
