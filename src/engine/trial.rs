//! An opt-in, offline survival/building slice. Supplies are seeded once on creation.
use super::*;
use crate::world::{item::ItemType, recipe::WorldRecipe};
use crate::worlds::{WorldLease, WorldStore};
use std::io::{self, Read};
const FILE: &str = "lantern-trial.json";
pub(super) const EVENT: u64 = u64::MAX - 71;
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TrialStage {
    Approach,
    Encounter,
    Build,
    Complete,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Trial {
    pub version: u8,
    pub stage: TrialStage,
    pub kills: u8,
    #[serde(default)]
    attempt_live: bool,
}
impl Trial {
    pub fn objective(&self) -> String {
        match self.stage {
            TrialStage::Approach => "LANTERN RUN: Cross the broken trail to the ruins →".into(),
            TrialStage::Encounter => format!(
                "LANTERN RUN: Defeat the guards {}/3 · dodge ! warnings · break cover or build steps",
                self.kills
            ),
            TrialStage::Build => {
                "LANTERN RUN: Take supplies · bridge the ravine · light the signal →".into()
            }
            TrialStage::Complete => {
                "LANTERN RUN COMPLETE · signal restored · keep exploring/building!".into()
            }
        }
    }
    pub fn compact_objective(&self) -> String {
        match self.stage {
            TrialStage::Approach => "LANTERN RUN · Reach the ruins →".into(),
            TrialStage::Encounter => format!("LANTERN RUN · Guards {}/3", self.kills),
            TrialStage::Build => "LANTERN RUN · Bridge gap + light signal →".into(),
            TrialStage::Complete => "LANTERN RUN COMPLETE · Signal restored!".into(),
        }
    }
    fn load(root: &Path) -> io::Result<Option<Self>> {
        let file = match File::open(root.join(FILE)) {
            Ok(f) => f,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(e),
        };
        let mut bytes = Vec::new();
        file.take(8193).read_to_end(&mut bytes)?;
        if bytes.len() > 8192 {
            return Err(io::Error::other("trial record exceeds limit"));
        }
        let mut trial: Self = serde_json::from_slice(&bytes)?;
        if trial.version != 1 || trial.kills > 3 {
            return Err(io::Error::other("invalid trial state; preserved"));
        }
        // Native/shared world saves do not retain mobs. Restart a complete attempt.
        if trial.stage == TrialStage::Encounter {
            trial.kills = 0;
        }
        Ok(Some(trial))
    }
}
impl GameState {
    pub fn trial_controls_hint(&self) -> Option<&'static str> {
        let trial = self.trial.as_ref()?;
        if self.is_player_on_ladder() {
            return Some("Hold W/Space to climb the ladder");
        }
        Some(match trial.stage {
            TrialStage::Approach => "A/D: move · Space: jump · 1-9: select",
            TrialStage::Encounter => "L-click hit · dodge ! · mine/build cover",
            TrialStage::Build if self.player.x < 49.0 => "R-click chest · Shift+L-click transfer",
            TrialStage::Build if self.player.x < 61.5 => "Select planks · R-click beside deck",
            TrialStage::Build => "Select torch · R-click top of stone signal",
            TrialStage::Complete => "Keep exploring/building · Q: save and exit",
        })
    }

    pub fn create_lantern_trial(store: &WorldStore, id: &str) -> io::Result<()> {
        let mut recipe = WorldRecipe::example();
        recipe.name = "Lantern Run".into();
        recipe.description = "Reclaim the ruins and rebuild the signal route.".into();
        recipe.terrain = crate::world::recipe::Terrain::Forest;
        recipe.surface_y = 64;
        recipe.relief = 0;
        recipe.seed = 12026;
        recipe.regions.clear();
        recipe.landmarks.clear();
        let root=store.create(id,"Offline survival/building trial: traverse, fight, reclaim supplies, bridge, relight.",&recipe).map_err(io::Error::other)?;
        let _lease = WorldLease::acquire(&root).map_err(io::Error::other)?;
        let mut s = Self::open_in_directory(&root)?;
        s.world.load_chunks_for_spawn_search(40, 100);
        for x in -16..=90 {
            for y in 52..=87 {
                s.world.set_block(
                    x,
                    y,
                    if y == 64 {
                        BlockType::Grass
                    } else if y > 64 {
                        BlockType::Stone
                    } else {
                        BlockType::Air
                    },
                );
            }
        }
        // A sheltered starter camp and readable, recoverable traversal course.
        for x in -5..=3 {
            s.world.set_block(x, 59, BlockType::Planks);
        }
        for y in 60..64 {
            s.world.set_block(-5, y, BlockType::Wood);
        }
        s.world.set_block(-3, 63, BlockType::CraftingTable);
        for y in 60..64 {
            s.world.set_block(7, y, BlockType::Wood);
        }
        for x in 5..=9 {
            for y in 57..=59 {
                s.world.set_block(x, y, BlockType::Leaves);
            }
        }
        for (x, b) in [
            (12, BlockType::StoneSlab),
            (13, BlockType::StoneStairs),
            (14, BlockType::Stone),
            (15, BlockType::StoneSlab),
        ] {
            s.world.set_block(x, 63, b);
        }
        for x in 18..=21 {
            for y in 64..68 {
                s.world.set_block(
                    x,
                    y,
                    if y == 67 {
                        BlockType::Water(8)
                    } else {
                        BlockType::Air
                    },
                );
            }
        }
        for y in 64..68 {
            s.world.set_block(21, y, BlockType::Ladder);
        }
        // Keep the recovery shaft dry: flowing water can wash away ladders.
        s.world.set_block(20, 67, BlockType::Stone);
        for x in [18, 21] {
            s.world.set_block(x, 63, BlockType::Planks);
        }
        s.world.set_block(21, 63, BlockType::Ladder); // Open the shaft's exit; no solid cap above the climber.
        // Ruins: roof, high windows and broken cover, with two entries.
        for x in 28..=44 {
            s.world.set_block(x, 58, BlockType::StoneBricks);
        }
        for x in [28, 44] {
            for y in 59..=60 {
                s.world.set_block(x, y, BlockType::StoneBricks);
            }
        }
        for x in [31, 39] {
            for y in 62..64 {
                s.world.set_block(x, y, BlockType::StoneBricks);
            }
        }
        s.world.set_block(44, 61, BlockType::StoneBricks);
        s.world.set_block(44, 62, BlockType::WoodDoor(false));
        s.world.set_block(44, 63, BlockType::WoodDoor(false));
        for x in 34..=36 {
            s.world.set_block(x, 59, BlockType::Glass);
        }
        // Reward is an ordinary chest, seeded ONLY here. Resume never refills it.
        s.world.set_block(46, 63, BlockType::Chest);
        let chest = s
            .world
            .ensure_chest_inventory(46, 63, 27)
            .ok_or_else(|| io::Error::other("trial chest unavailable"))?;
        for (item, n) in [
            (ItemType::Planks, 24),
            (ItemType::Torch, 8),
            (ItemType::Bread, 3),
            (ItemType::IronPickaxe, 1),
        ] {
            chest.add_item(item, n);
        }
        for x in 52..=59 {
            for y in 64..84 {
                s.world.set_block(
                    x,
                    y,
                    if y == 83 {
                        BlockType::Water(8)
                    } else {
                        BlockType::Air
                    },
                );
            }
        }
        for x in [50, 60] {
            for y in 64..84 {
                s.world.set_block(x, y, BlockType::Ladder);
            }
        }
        s.world.set_block(51, 63, BlockType::Planks); // Solid abutment for the first player-placed deck block.
        for x in [52, 59] {
            s.world.set_block(x, 83, BlockType::Stone);
        }
        for y in 61..64 {
            s.world.set_block(65, y, BlockType::StoneBricks);
        }
        for x in 63..=67 {
            s.world.set_block(x, 64, BlockType::StoneBricks);
        }
        for x in [-4, 3, 11, 16, 23, 29, 35, 42, 47, 61, 68] {
            if s.world.torch_attachment(x, 63).is_some() {
                s.world.set_block(x, 63, BlockType::Torch);
            }
        }
        s.player.x = 0.5;
        s.player.y = 64.0;
        s.player.grounded = true;
        s.time_of_day = 22000.0;
        for command in [
            ClientCommand::ToggleRuleMobSpawning,
            ClientCommand::ToggleRuleDaylightCycle,
            ClientCommand::ToggleRuleWeatherCycle,
            ClientCommand::ToggleRuleKeepInventory,
        ] {
            s.apply_client_command(command);
        }
        s.inventory.slots.fill(None);
        for (item, n) in [
            (ItemType::StoneSword, 1),
            (ItemType::WoodAxe, 1),
            (ItemType::WoodPickaxe, 1),
            (ItemType::Planks, 6),
            (ItemType::Torch, 4),
            (ItemType::Bread, 2),
        ] {
            s.inventory.add_item(item, n);
        }
        s.zombies.clear();
        s.skeletons.clear();
        s.creepers.clear();
        s.spiders.clear();
        s.slimes.clear();
        s.endermen.clear();
        s.trial = Some(Trial {
            version: 1,
            stage: TrialStage::Approach,
            kills: 0,
            attempt_live: false,
        });
        s.try_persist_world_and_progress()
    }
    pub(super) fn load_trial(&mut self) -> io::Result<()> {
        self.trial = Trial::load(self.world.save_root())?;
        Ok(())
    }
    pub fn save_trial(&self) -> io::Result<()> {
        self.save_trial_in(self.world.save_root())
    }
    pub(super) fn save_trial_in(&self, root: &Path) -> io::Result<()> {
        if let Some(t) = &self.trial
            && !self.world.is_replica()
        {
            Self::write_progression_payload_atomically(&root.join(FILE), &serde_json::to_vec(t)?)?;
        }
        Ok(())
    }
    pub(super) fn credit_trial_kill(&mut self, event: u64, last_damage: u64) {
        if event == EVENT
            && self.mob_has_player_kill_credit(last_damage)
            && let Some(t) = self
                .trial
                .as_mut()
                .filter(|t| t.stage == TrialStage::Encounter && t.attempt_live)
        {
            t.kills = t.kills.saturating_add(1).min(3);
        }
    }
    pub(super) fn update_trial(&mut self) {
        if self.current_dimension != Dimension::Overworld {
            return;
        }
        let Some(mut t) = self.trial.take() else {
            return;
        };
        if t.stage == TrialStage::Approach && self.director_near_player(35.0, 9.0) {
            t.stage = TrialStage::Encounter;
        }
        if t.stage == TrialStage::Encounter {
            let remaining = self
                .zombies
                .iter()
                .filter(|e| e.director_event == EVENT)
                .count()
                + self
                    .skeletons
                    .iter()
                    .filter(|e| e.director_event == EVENT)
                    .count();
            if t.kills == 3 && remaining == 0 {
                t.stage = TrialStage::Build;
            } else if !t.attempt_live || remaining == 0 {
                self.zombies.retain(|e| e.director_event != EVENT);
                self.skeletons.retain(|e| e.director_event != EVENT);
                t.kills = 0;
                t.attempt_live = true;
                for x in [32.5, 37.5] {
                    let mut z = Zombie::new(x, 64.0);
                    z.director_event = EVENT;
                    self.zombies.push(z);
                }
                let mut skeleton = Skeleton::new(42.5, 64.0);
                skeleton.director_event = EVENT;
                self.skeletons.push(skeleton);
            }
        }
        if t.stage == TrialStage::Build
            && (52..=59).all(|x| self.world.get_block(x, 63).is_solid())
            && self.world.get_block(65, 60) == BlockType::Torch
        {
            t.stage = TrialStage::Complete;
        }
        self.trial = Some(t);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn trial_credit_restart_and_looted_chest_are_persistent_without_refills() {
        let root = std::env::temp_dir().join(format!("tc-trial-test-{}", rand::random::<u64>()));
        let store = WorldStore::new(root.clone());
        GameState::create_lantern_trial(&store, "trial").unwrap();
        let path = store.open("trial").unwrap();
        let mut s = GameState::open_in_directory(&path).unwrap();
        s.dismiss_startup_splash();
        s.player.x = 27.5;
        s.update(0, 0);
        assert_eq!(s.trial.as_ref().unwrap().stage, TrialStage::Encounter);
        s.zombies[0].health = 0.0;
        s.zombies[0].last_player_damage_tick = s.world_tick;
        s.update(0, 0);
        assert_eq!(s.trial.as_ref().unwrap().kills, 1);
        s.try_persist_world_and_progress().unwrap();
        drop(s);
        let mut s = GameState::open_in_directory(&path).unwrap();
        s.dismiss_startup_splash();
        assert_eq!(s.trial.as_ref().unwrap().kills, 1);
        s.update(0, 0);
        assert_eq!(s.zombies.len() + s.skeletons.len(), 2);
        // Despawn is not victory.
        s.zombies.clear();
        s.skeletons.clear();
        s.update(0, 0);
        assert_eq!(s.trial.as_ref().unwrap().stage, TrialStage::Encounter);
        for z in &mut s.zombies {
            z.health = 0.0;
            z.last_player_damage_tick = s.world_tick;
        }
        for z in &mut s.skeletons {
            z.health = 0.0;
            z.last_player_damage_tick = s.world_tick;
        }
        s.update(0, 0);
        s.update(0, 0);
        assert_eq!(s.trial.as_ref().unwrap().stage, TrialStage::Build);
        s.world
            .chest_inventory_mut(46, 63)
            .unwrap()
            .slots
            .fill(None);
        s.try_persist_world_and_progress().unwrap();
        drop(s);
        let mut s = GameState::open_in_directory(&path).unwrap();
        s.world.load_chunks_for_spawn_search(46, 32);
        assert!(
            s.world
                .chest_inventory(46, 63)
                .unwrap()
                .slots
                .iter()
                .all(Option::is_none)
        );
        for x in 52..=59 {
            s.world.set_block(x, 63, BlockType::Planks);
        }
        s.world.set_block(65, 60, BlockType::Torch);
        s.update_trial();
        assert_eq!(s.trial.as_ref().unwrap().stage, TrialStage::Complete);
        drop(s);
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn fresh_trial_peer_gets_one_kit_and_rejoin_does_not_refill() {
        let mut s = GameState::new();
        s.trial = Some(Trial {
            version: 1,
            stage: TrialStage::Approach,
            kills: 0,
            attempt_live: false,
        });
        s.join_peer(1);
        assert!(s.inventory.has_item(ItemType::StoneSword, 1));
        s.inventory.slots.fill(None);
        s.join_peer(1);
        assert!(!s.inventory.has_item(ItemType::StoneSword, 1));
    }
}
