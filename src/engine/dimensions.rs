//! Owned dimension state: switching the selected actor never clones or clears a world.
use super::*;
use std::collections::BTreeMap;

macro_rules! dimension_fields {
    ($world:ident; $( $field:ident: $ty:ty = $fresh:expr; )*) => {
        pub(super) struct DimensionRuntime { $world: World, $( $field: $ty, )* }
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
    villager_open_doors: HashMap<(i32, i32), u8> = HashMap::new();
    nether_spawn_timer: u16 = 0;
    end_spawn_timer: u16 = 0;
    end_boss_initialized: bool = false;
    end_victory_ticks: u16 = 0;
    end_victory_origin: Option<(f64, f64)> = None;
}

#[derive(Clone, Default, Serialize, Deserialize)]
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
    pub(crate) fn apply(&self, state: &mut GameState) -> Result<(), String> {
        if self.portal_links.iter().any(|&((d, x, y), (tx, ty))| {
            d > 1
                || !(3..CHUNK_HEIGHT as i32).contains(&y)
                || !(3..CHUNK_HEIGHT as i32).contains(&ty)
                || x.checked_mul(8).is_none()
                || tx.checked_mul(8).is_none()
        }) {
            return Err("invalid shared portal links; save preserved".into());
        }
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
        if self.cooperative() {
            self.select_dimension(dimension);
            if let Some(id) = self.active_peer {
                self.cooperative_dimensions.insert(id, dimension);
            }
        } else {
            self.current_dimension = dimension;
            self.world = self.world.for_dimension(dimension);
            self.reset_weather_for_dimension();
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
