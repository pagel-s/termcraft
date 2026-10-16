//! Owned dimension state: switching the selected actor never clones or clears a world.
use super::*;
use std::collections::BTreeMap;

macro_rules! dimension_fields {
    ($world:ident; $( $(#[$attr:meta])* $field:ident: $ty:ty = $fresh:expr; )*) => {
        pub(super) struct DimensionRuntime { $world: World, $( $field: $ty, )* }
        #[derive(Clone, Debug, Serialize, Deserialize)]
        #[serde(deny_unknown_fields)]
        pub(crate) struct DimensionCheckpoint { $( $(#[$attr])* $field: $ty, )* }
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
                if self.$field.iter().any(|e| !valid_position(e.x, e.y) || !valid_velocity(e.vx, e.vy)) {
                    return Err(concat!("invalid saved ", stringify!($field), "; save preserved").into());
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
