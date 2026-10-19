//! Owned dimension state: switching the selected actor never clones or clears a world.
use super::*;
use std::collections::BTreeMap;

macro_rules! v9_type {
    (zombies, $ty:ty) => { Vec<legacy_entities::ZombieV10> };
    (pigmen, $ty:ty) => { Vec<legacy_entities::ZombiePigmanV10> };
    (skeletons, $ty:ty) => { Vec<legacy_entities::SkeletonV10> };
    (creepers, $ty:ty) => { Vec<legacy_entities::CreeperV10> };
    (spiders, $ty:ty) => { Vec<legacy_entities::SpiderV10> };
    (silverfish, $ty:ty) => { Vec<legacy_entities::SilverfishV10> };
    (slimes, $ty:ty) => { Vec<legacy_entities::SlimeV10> };
    (endermen, $ty:ty) => { Vec<legacy_entities::EndermanV10> };
    (ghasts, $ty:ty) => { Vec<legacy_entities::GhastV10> };
    (blazes, $ty:ty) => { Vec<legacy_entities::BlazeV10> };
    (cows, $ty:ty) => { Vec<legacy_entities::CowV10> };
    (sheep, $ty:ty) => { Vec<legacy_entities::SheepV10> };
    (pigs, $ty:ty) => { Vec<legacy_entities::PigV10> };
    (chickens, $ty:ty) => { Vec<legacy_entities::ChickenV10> };
    (squids, $ty:ty) => { Vec<legacy_entities::SquidV10> };
    (wolves, $ty:ty) => { Vec<legacy_entities::WolfV10> };
    (ocelots, $ty:ty) => { Vec<legacy_entities::OcelotV10> };
    (villagers, $ty:ty) => { Vec<legacy_entities::VillagerV10> };
    (item_entities, $ty:ty) => { Vec<legacy_entities::ItemEntityV9> };
    (fireballs, $ty:ty) => { Vec<legacy_entities::FireballV9> };
    ($field:ident, $ty:ty) => { $ty };
}
macro_rules! v9_value {
    (zombies, $value:expr) => {
        $value.into_iter().map(Into::into).collect()
    };
    (pigmen, $value:expr) => {
        $value.into_iter().map(Into::into).collect()
    };
    (skeletons, $value:expr) => {
        $value.into_iter().map(Into::into).collect()
    };
    (creepers, $value:expr) => {
        $value.into_iter().map(Into::into).collect()
    };
    (spiders, $value:expr) => {
        $value.into_iter().map(Into::into).collect()
    };
    (silverfish, $value:expr) => {
        $value.into_iter().map(Into::into).collect()
    };
    (slimes, $value:expr) => {
        $value.into_iter().map(Into::into).collect()
    };
    (endermen, $value:expr) => {
        $value.into_iter().map(Into::into).collect()
    };
    (ghasts, $value:expr) => {
        $value.into_iter().map(Into::into).collect()
    };
    (blazes, $value:expr) => {
        $value.into_iter().map(Into::into).collect()
    };
    (cows, $value:expr) => {
        $value.into_iter().map(Into::into).collect()
    };
    (sheep, $value:expr) => {
        $value.into_iter().map(Into::into).collect()
    };
    (pigs, $value:expr) => {
        $value.into_iter().map(Into::into).collect()
    };
    (chickens, $value:expr) => {
        $value.into_iter().map(Into::into).collect()
    };
    (squids, $value:expr) => {
        $value.into_iter().map(Into::into).collect()
    };
    (wolves, $value:expr) => {
        $value.into_iter().map(Into::into).collect()
    };
    (ocelots, $value:expr) => {
        $value.into_iter().map(Into::into).collect()
    };
    (villagers, $value:expr) => {
        $value.into_iter().map(Into::into).collect()
    };
    (item_entities, $value:expr) => {
        $value.into_iter().map(Into::into).collect()
    };
    (fireballs, $value:expr) => {
        $value.into_iter().map(Into::into).collect()
    };
    ($field:ident, $value:expr) => {
        $value
    };
}

macro_rules! v10_type {
    (item_entities, $ty:ty) => {
        $ty
    };
    (fireballs, $ty:ty) => {
        $ty
    };
    ($field:ident, $ty:ty) => {
        v9_type!($field, $ty)
    };
}
macro_rules! v10_value {
    (item_entities, $value:expr) => {
        $value
    };
    (fireballs, $value:expr) => {
        $value
    };
    ($field:ident, $value:expr) => {
        v9_value!($field, $value)
    };
}
macro_rules! dimension_fields {
    ($world:ident; $( $(#[$attr:meta])* $field:ident: $ty:ty = $fresh:expr; )*) => {
        pub(super) struct DimensionRuntime { $world: World, $( $field: $ty, )* }
        #[derive(Clone, Debug, Serialize, Deserialize)]
        #[serde(deny_unknown_fields)]
        pub(crate) struct DimensionCheckpoint { $( $(#[$attr])* $field: $ty, )* }
        #[derive(Clone, Debug, Deserialize)]
        pub(crate) struct DimensionCheckpointV9 { $( $(#[$attr])* $field: v9_type!($field, $ty), )* }
        #[derive(Clone, Debug, Deserialize)]
        pub(crate) struct DimensionCheckpointV10 { $( $(#[$attr])* $field: v10_type!($field, $ty), )* }
        impl From<DimensionCheckpointV10> for DimensionCheckpoint {
            fn from(old: DimensionCheckpointV10)->Self { Self { $($field:v10_value!($field,old.$field),)* } }
        }
        impl From<DimensionCheckpointV9> for DimensionCheckpoint {
            fn from(old: DimensionCheckpointV9) -> Self { Self { $($field: v9_value!($field, old.$field),)* } }
        }
        impl DimensionCheckpoint {
            #[allow(clippy::clone_on_copy)]
            fn capture(state: &GameState) -> Self { Self { $($field: state.$field.clone(),)* } }
            #[allow(clippy::clone_on_copy)]
            fn capture_runtime(state: &DimensionRuntime) -> Self { Self { $($field: state.$field.clone(),)* } }
            #[allow(clippy::clone_on_copy)]
            fn apply(&self, state: &mut GameState) { $(state.$field = self.$field.clone();)* }
        }
        impl DimensionRuntime {
            fn fresh($world: World) -> Self { Self { $world, $( $field: $fresh, )* } }
            fn swap(&mut self, state: &mut GameState) {
                std::mem::swap(&mut self.$world, &mut state.$world);
                $( std::mem::swap(&mut self.$field, &mut state.$field); )*
            }
        }
    }
}
dimension_fields! { world;
    zombies: Vec<Zombie> = Vec::new();
    creepers: Vec<Creeper> = Vec::new();
    skeletons: Vec<Skeleton> = Vec::new();
    spiders: Vec<Spider> = Vec::new();
    silverfish: Vec<Silverfish> = Vec::new();
    slimes: Vec<Slime> = Vec::new();
    endermen: Vec<Enderman> = Vec::new();
    blazes: Vec<Blaze> = Vec::new();
    pigmen: Vec<ZombiePigman> = Vec::new();
    ghasts: Vec<Ghast> = Vec::new();
    cows: Vec<Cow> = Vec::new();
    sheep: Vec<Sheep> = Vec::new();
    pigs: Vec<Pig> = Vec::new();
    chickens: Vec<Chicken> = Vec::new();
    squids: Vec<Squid> = Vec::new();
    wolves: Vec<Wolf> = Vec::new();
    ocelots: Vec<Ocelot> = Vec::new();
    villagers: Vec<Villager> = Vec::new();
    boats: Vec<Boat> = Vec::new();
    item_entities: Vec<ItemEntity> = Vec::new();
    experience_orbs: Vec<ExperienceOrb> = Vec::new();
    arrows: Vec<Arrow> = Vec::new();
    fireballs: Vec<Fireball> = Vec::new();
    end_crystals: Vec<EndCrystal> = Vec::new();
    ender_dragon: Option<EnderDragon> = None;
    lightning_bolts: Vec<LightningBolt> = Vec::new();
    time_of_day: f32 = 4000.0;
    weather: WeatherType = WeatherType::Clear;
    weather_timer: u32 = 6000;
    thunder_flash_timer: u8 = 0;
    weather_rain_intensity: f32 = 0.0;
    weather_wind_intensity: f32 = 0.0;
    weather_thunder_intensity: f32 = 0.0;
    world_tick: u64 = 0;
    dungeon_spawner_timer: u16 = 0;
    silverfish_spawner_timer: u16 = 0;
    blaze_spawner_timer: u16 = 0;
    overworld_hostile_spawn_timer: u16 = 0;
    overworld_passive_spawn_timer: u16 = 0;
    overworld_villager_spawn_timer: u16 = 0;
    overworld_squid_spawn_timer: u16 = 0;
    overworld_wolf_spawn_timer: u16 = 0;
    overworld_ocelot_spawn_timer: u16 = 0;
    #[serde(with = "door_map")]
    villager_open_doors: HashMap<(i32, i32), u8> = HashMap::new();
    nether_spawn_timer: u16 = 0;
    end_spawn_timer: u16 = 0;
    end_boss_initialized: bool = false;
    end_victory_ticks: u16 = 0;
    end_victory_origin: Option<(f64, f64)> = None;
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct SharedProgress {
    portal_links: Vec<PortalLink>,
    dragon_defeated: bool,
}
impl SharedProgress {
    pub(crate) fn capture(state: &GameState) -> Self {
        let mut portal_links: Vec<_> = state.portal_links.iter().map(|(&s, &t)| (s, t)).collect();
        portal_links.sort_unstable();
        Self {
            portal_links,
            dragon_defeated: state.dragon_defeated,
        }
    }
    fn validate(&self) -> Result<(), String> {
        if self.portal_links.iter().any(|&((d, x, y), (tx, ty))| {
            d > 1
                || !(3..CHUNK_HEIGHT as i32).contains(&y)
                || !(3..CHUNK_HEIGHT as i32).contains(&ty)
                || x.checked_mul(8).is_none()
                || tx.checked_mul(8).is_none()
        }) {
            return Err("invalid shared portal links; save preserved".into());
        }
        Ok(())
    }
    pub(crate) fn apply(&self, state: &mut GameState) -> Result<(), String> {
        self.validate()?;
        state.portal_links = self.portal_links.iter().copied().collect();
        state.dragon_defeated = self.dragon_defeated;
        Ok(())
    }
}

impl GameState {
    pub(crate) fn selected_peer_id(&self) -> Option<u16> {
        self.active_peer
    }
    pub(crate) fn world_in_dimension(&self, dimension: Dimension) -> Option<&World> {
        if self.current_dimension == dimension {
            Some(&self.world)
        } else {
            self.cooperative_worlds
                .get(&dimension)
                .map(|runtime| &runtime.world)
        }
    }
    pub(crate) fn check_dimension_storage(&self) -> std::io::Result<()> {
        self.world.check_storage()?;
        for runtime in self.cooperative_worlds.values() {
            runtime.world.check_storage()?;
        }
        Ok(())
    }
    pub(crate) fn peer_dimension(&self, id: u16) -> Dimension {
        self.cooperative_dimensions
            .get(&id)
            .copied()
            .unwrap_or(self.current_dimension)
    }
    pub(crate) fn peer_ids_here(&self) -> Vec<u16> {
        self.peer_ids()
            .into_iter()
            .filter(|&id| self.peer_dimension(id) == self.current_dimension)
            .collect()
    }
    pub(super) fn other_players_here(&self) -> impl Iterator<Item = &PlayerSession> {
        self.cooperative_players
            .iter()
            .filter_map(|(&id, p)| (self.peer_dimension(id) == self.current_dimension).then_some(p))
    }
    pub(crate) fn select_dimension(&mut self, dimension: Dimension) {
        if dimension == self.current_dimension {
            return;
        }
        let mut next = self
            .cooperative_worlds
            .remove(&dimension)
            .unwrap_or_else(|| DimensionRuntime::fresh(self.world.for_dimension(dimension)));
        next.swap(self);
        self.cooperative_worlds.insert(self.current_dimension, next);
        self.current_dimension = dimension;
    }
    pub(super) fn travel_dimension(&mut self, dimension: Dimension) {
        if dimension != self.current_dimension {
            // Player references index the current world's vectors/coordinates.
            // Keep the boat in its source world, never mount a destination boat
            // that happens to occupy the same vector index.
            self.mounted_boat = None;
            self.clear_fishing_line();
        }
        self.select_dimension(dimension);
        if let Some(id) = self.active_peer {
            self.cooperative_dimensions.insert(id, dimension);
        }
    }
    pub(crate) fn restore_peer_dimension(&mut self, id: u16, dimension: Dimension) {
        self.cooperative_dimensions.insert(id, dimension);
        if self.active_peer == Some(id) {
            self.select_dimension(dimension);
        }
    }
    pub(crate) fn save_dimension_worlds(
        &mut self,
    ) -> std::io::Result<BTreeMap<Dimension, SharedSettings>> {
        let selected = self.current_dimension;
        let mut dimensions: Vec<_> = self.cooperative_worlds.keys().copied().collect();
        dimensions.push(selected);
        let mut settings = BTreeMap::new();
        let result = (|| {
            for dimension in dimensions {
                self.select_dimension(dimension);
                self.world.try_save_all()?;
                settings.insert(dimension, SharedSettings::capture(self));
            }
            Ok(settings)
        })();
        self.select_dimension(selected);
        result
    }
    pub(crate) fn restore_dimension_settings(
        &mut self,
        settings: &BTreeMap<Dimension, SharedSettings>,
    ) -> Result<(), String> {
        let selected = self.current_dimension;
        let result = (|| {
            for (&dimension, settings) in settings {
                self.select_dimension(dimension);
                settings.apply(self)?;
            }
            Ok(())
        })();
        self.select_dimension(selected);
        result
    }
    pub(crate) fn maintain_dimension_caches(&mut self) -> std::io::Result<()> {
        let selected = self.current_dimension;
        let mut dimensions: Vec<_> = self.cooperative_worlds.keys().copied().collect();
        dimensions.push(selected);
        let result = (|| {
            for dimension in dimensions {
                self.select_dimension(dimension);
                self.world.check_storage()?;
                let viewers: Vec<_> = self
                    .peer_ids_here()
                    .into_iter()
                    .filter_map(|id| self.peer_session(id).map(|s| s.player().x.floor() as i32))
                    .collect();
                self.world.trim_chunk_cache(&viewers, 1)?;
            }
            Ok(())
        })();
        self.select_dimension(selected);
        result
    }
}

pub(crate) const MAX_CHECKPOINT_BYTES: u64 = 16 * 1024 * 1024;
const MAX_ENTITIES: usize = 32_768;

/// Saved together with inventories, never the range-limited network projection.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RuntimeCheckpoint {
    version: u8,
    selected: Dimension,
    pub(crate) revision: u64,
    progression: SharedProgress,
    dimensions: BTreeMap<Dimension, DimensionCheckpoint>,
    encounter: Option<u64>,
    trial: Option<Trial>,
}
#[derive(Clone, Debug, Deserialize)]
pub(crate) struct RuntimeCheckpointV9 {
    version: u8,
    selected: Dimension,
    revision: u64,
    progression: SharedProgress,
    dimensions: BTreeMap<Dimension, DimensionCheckpointV9>,
    encounter: Option<u64>,
    trial: Option<Trial>,
}
impl From<RuntimeCheckpointV9> for RuntimeCheckpoint {
    fn from(old: RuntimeCheckpointV9) -> Self {
        Self {
            version: old.version,
            selected: old.selected,
            revision: old.revision,
            progression: old.progression,
            dimensions: old
                .dimensions
                .into_iter()
                .map(|(d, c)| (d, c.into()))
                .collect(),
            encounter: old.encounter,
            trial: old.trial,
        }
    }
}
#[derive(Clone, Debug, Deserialize)]
pub(crate) struct RuntimeCheckpointV10 {
    version: u8,
    selected: Dimension,
    revision: u64,
    progression: SharedProgress,
    dimensions: BTreeMap<Dimension, DimensionCheckpointV10>,
    encounter: Option<u64>,
    trial: Option<Trial>,
}
impl From<RuntimeCheckpointV10> for RuntimeCheckpoint {
    fn from(old: RuntimeCheckpointV10) -> Self {
        Self {
            version: old.version,
            selected: old.selected,
            revision: old.revision,
            progression: old.progression,
            dimensions: old
                .dimensions
                .into_iter()
                .map(|(d, c)| (d, c.into()))
                .collect(),
            encounter: old.encounter,
            trial: old.trial,
        }
    }
}
impl RuntimeCheckpoint {
    pub(crate) fn capture(state: &GameState) -> Self {
        let mut dimensions: BTreeMap<_, _> = state
            .cooperative_worlds
            .iter()
            .map(|(&d, runtime)| (d, DimensionCheckpoint::capture_runtime(runtime)))
            .collect();
        dimensions.insert(state.current_dimension, DimensionCheckpoint::capture(state));
        Self {
            version: 1,
            selected: state.current_dimension,
            revision: state.runtime_revision.get().saturating_add(1),
            progression: SharedProgress::capture(state),
            dimensions,
            encounter: state.director_encounter,
            trial: state.trial.clone(),
        }
    }
    pub(crate) fn contains_dimension(&self, dimension: Dimension) -> bool {
        self.dimensions.contains_key(&dimension)
    }
    pub(crate) fn validate(&self) -> Result<(), String> {
        if self.version != 1
            || self.revision == 0
            || self.revision == u64::MAX
            || self.dimensions.is_empty()
            || self.dimensions.len() > 3
            || !self.dimensions.contains_key(&self.selected)
            || self.encounter == Some(0)
            || self
                .trial
                .as_ref()
                .is_some_and(|t| t.version != 1 || t.kills > 3)
        {
            return Err("invalid runtime checkpoint; save preserved".into());
        }
        self.progression.validate()?;
        let mut count = 0;
        for snapshot in self.dimensions.values() {
            count += snapshot.validate()?;
        }
        if count > MAX_ENTITIES {
            return Err("runtime checkpoint exceeds 32768 entities; save preserved".into());
        }
        Ok(())
    }
    pub(crate) fn apply(&self, state: &mut GameState) {
        // Validation is completed before changing any live state.
        state.cooperative_worlds.clear();
        for (&dimension, checkpoint) in &self.dimensions {
            state.select_dimension(dimension);
            checkpoint.apply(state);
        }
        state.select_dimension(self.selected);
        self.progression
            .apply(state)
            .expect("validated shared progression");
        state.runtime_revision.set(self.revision);
        state.director_encounter = self.encounter;
        state.trial = self.trial.clone();
        // Player references into an old boat vector must never survive restoration.
        state.mounted_boat = None;
    }
    pub(crate) fn apply_for_native(&self, state: &mut GameState, dimension: Dimension) {
        self.apply(state);
        state.select_dimension(dimension);
    }
}

fn valid_position(x: f64, y: f64) -> bool {
    x.is_finite() && x.abs() <= 1_000_000.0 && y.is_finite() && (-1024.0..=1024.0).contains(&y)
}
fn valid_velocity(x: f64, y: f64) -> bool {
    x.is_finite() && y.is_finite() && x.abs() <= 128.0 && y.abs() <= 128.0
}
impl DimensionCheckpoint {
    fn validate(&self) -> Result<usize, String> {
        let mut count = 0;
        macro_rules! moving {
            ($($field:ident),*) => { $(
                count += self.$field.len();
                if let Some(e) = self.$field.iter().find(|e| !valid_position(e.x, e.y) || !valid_velocity(e.vx, e.vy)) {
                    return Err(format!("invalid saved {} at ({:.3}, {:.3}), velocity ({:.3}, {:.3}); save preserved", stringify!($field), e.x, e.y, e.vx, e.vy));
                }
            )* };
        }
        moving!(
            zombies,
            creepers,
            skeletons,
            spiders,
            silverfish,
            slimes,
            endermen,
            blazes,
            pigmen,
            ghasts,
            cows,
            sheep,
            pigs,
            chickens,
            squids,
            wolves,
            ocelots,
            villagers,
            boats,
            item_entities,
            experience_orbs,
            arrows,
            fireballs
        );
        macro_rules! health {
            ($($field:ident),*) => { $(
                if self.$field.iter().any(|e| !e.health.is_finite() || e.health.abs() > 10000.0 || e.age > u64::MAX / 2) {
                    return Err("invalid saved entity health; save preserved".into());
                }
            )* };
        }
        health!(
            zombies,
            creepers,
            skeletons,
            spiders,
            silverfish,
            slimes,
            endermen,
            blazes,
            pigmen,
            ghasts,
            cows,
            sheep,
            pigs,
            chickens,
            squids,
            wolves,
            ocelots,
            villagers,
            end_crystals
        );
        macro_rules! hazard_state {
            ($($field:ident),*) => { $(
                if self.$field.iter().any(|e| !e.hazards.valid()) {
                    return Err(format!("invalid saved {} hazard state; save preserved", stringify!($field)));
                }
            )* };
        }
        hazard_state!(
            zombies, creepers, skeletons, spiders, silverfish, slimes, endermen, blazes, pigmen,
            ghasts, cows, sheep, pigs, chickens, squids, wolves, ocelots, villagers
        );
        count += self.end_crystals.len()
            + self.lightning_bolts.len()
            + usize::from(self.ender_dragon.is_some());
        if self.end_crystals.iter().any(|e| !valid_position(e.x, e.y))
            || self.ender_dragon.as_ref().is_some_and(|e| {
                !valid_position(e.x, e.y)
                    || !valid_velocity(e.vx, e.vy)
                    || !e.health.is_finite()
                    || e.health.abs() > 10000.0
                    || e.age > u64::MAX / 2
                    || !e.max_health.is_finite()
                    || !(1.0..=10000.0).contains(&e.max_health)
            })
            || self
                .arrows
                .iter()
                .any(|e| !e.damage.is_finite() || !(0.0..=10000.0).contains(&e.damage))
            || self
                .skeletons
                .iter()
                .any(|e| !valid_position(e.aim_x, e.aim_y))
            || self
                .squids
                .iter()
                .any(|e| !valid_velocity(e.swim_dir_x, e.swim_dir_y))
            || self.slimes.iter().any(|e| ![1, 2, 4].contains(&e.size))
            || self
                .villagers
                .iter()
                .any(|e| e.home_x.abs_diff(0) > 1_000_000 || !(0..128).contains(&e.home_y))
            || self.lightning_bolts.iter().any(|e| {
                !valid_position(e.x as f64, e.y_top as f64)
                    || !valid_position(e.x as f64, e.y_bottom as f64)
            })
            || self.item_entities.iter().any(|e| e.age > u64::MAX / 2)
            || self.item_entities.iter().any(|e| {
                e.payload.as_ref().is_some_and(|p| {
                    p.stack.item_type != e.item_type
                        || p.stack.count == 0
                        || p.stack.count
                            > if e.item_type.max_durability().is_some() {
                                1
                            } else {
                                e.item_type.max_stack_size()
                            }
                        || p.enchant_level > ENCHANT_MAX_LEVEL
                        || p.stack.durability.is_some_and(|v| {
                            e.item_type
                                .max_durability()
                                .is_none_or(|max| v == 0 || v > max)
                        })
                })
            })
            || self.experience_orbs.iter().any(|e| e.age > u64::MAX / 2)
            || self.arrows.iter().any(|e| e.age > u64::MAX / 2)
            || self.fireballs.iter().any(|e| e.age > u64::MAX / 2)
            || self.villager_open_doors.len() > 4096
            || !self.time_of_day.is_finite()
            || !(0.0..24000.0).contains(&self.time_of_day)
            || [
                self.weather_rain_intensity,
                self.weather_wind_intensity,
                self.weather_thunder_intensity,
            ]
            .iter()
            .any(|v| !v.is_finite() || !(0.0..=1.0).contains(v))
            || self
                .end_victory_origin
                .is_some_and(|(x, y)| !valid_position(x, y))
            || count > MAX_ENTITIES
        {
            return Err("invalid saved dimension state; save preserved".into());
        }
        Ok(count)
    }
}

// JSON object keys cannot represent a block coordinate. Keep both checkpoint
// encodings deterministic and reject duplicate/invalid positions on read.
mod door_map {
    use super::*;
    pub fn serialize<S: serde::Serializer>(
        map: &HashMap<(i32, i32), u8>,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        let mut entries: Vec<_> = map.iter().map(|(&p, &t)| (p, t)).collect();
        entries.sort_unstable();
        entries.serialize(serializer)
    }
    pub fn deserialize<'de, D: serde::Deserializer<'de>>(
        deserializer: D,
    ) -> Result<HashMap<(i32, i32), u8>, D::Error> {
        let entries = Vec::<((i32, i32), u8)>::deserialize(deserializer)?;
        if entries.len() > 4096
            || entries
                .iter()
                .any(|&((x, y), _)| x.abs_diff(0) > 1_000_000 || !(0..128).contains(&y))
        {
            return Err(serde::de::Error::custom("invalid saved door positions"));
        }
        let len = entries.len();
        let map: HashMap<_, _> = entries.into_iter().collect();
        if map.len() != len {
            return Err(serde::de::Error::custom("duplicate saved door position"));
        }
        Ok(map)
    }
}
