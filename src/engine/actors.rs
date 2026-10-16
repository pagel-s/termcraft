//! Per-player state for the shared authoritative simulation. World state is never cloned.
use super::*;
macro_rules! player_fields {
    ($( $(#[$attr:meta])* $field:ident: $ty:ty = $fresh:expr; )*) => {
        #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
        pub struct PlayerSession { $( $(#[$attr])* pub(super) $field: $ty, )* }
        impl PlayerSession {
            #[allow(clippy::clone_on_copy)]
            pub fn capture(state: &GameState) -> Self { Self { $($field: state.$field.clone(),)* } }
            #[allow(clippy::clone_on_copy)]
            pub fn apply(&self, state: &mut GameState) { $(state.$field = self.$field.clone();)* }
            fn swap(&mut self, state: &mut GameState) { $(std::mem::swap(&mut self.$field, &mut state.$field);)* }
            fn fresh(x: f64, y: f64) -> Self { let mut actor=Self { $($field: $fresh,)* }; actor.player=Player::new(x,y); actor }
            pub fn player(&self) -> &Player { &self.player }
            pub fn inventory(&self) -> &Inventory { &self.inventory }
            pub fn hotbar_index(&self) -> u8 { self.hotbar_index }
            pub fn inventory_open(&self) -> bool { self.inventory_open }
            pub fn dead(&self) -> bool { self.death_screen_active }
            pub fn credits(&self) -> bool { self.credits_active }
        }
    }
}
player_fields! {
        player: Player = Player::new(0.0, 0.0);
        inventory: Inventory = Inventory::new(PLAYER_INVENTORY_CAPACITY);
        armor_slots: [Option<ItemStack>; ARMOR_SLOT_COUNT] = [None, None, None, None];
        crafting_grid: [Option<ItemStack>; CRAFT_GRID_UI_SLOTS] = std::array::from_fn(|_| None);
        #[serde(with = "array_serde")]
        inventory_enchant_levels: [u8; PLAYER_INVENTORY_CAPACITY] = [0; PLAYER_INVENTORY_CAPACITY];
        armor_enchant_levels: [u8; ARMOR_SLOT_COUNT] = [0; ARMOR_SLOT_COUNT];
        hotbar_index: u8 = 0;
        left_click_down: bool = false;
        eye_guidance_timer: u16 = 0;
        eye_guidance_dir: i8 = 0;
        eye_guidance_distance: i32 = 0;
        moving_left: bool = false;
        moving_right: bool = false;
        inventory_open: bool = false;
        at_crafting_table: bool = false;
        at_furnace: bool = false;
        at_chest: bool = false;
        at_enchanting_table: bool = false;
        at_anvil: bool = false;
        at_brewing_stand: bool = false;
        selected_inventory_slot: Option<usize> = None;
        open_chest_pos: Option<(i32, i32)> = None;
        spawn_point: Option<(i32, i32)> = None;
        portal_timer: u16 = 0;
        portal_cooldown: u16 = 0;
        movement_profile: MovementProfile = MovementProfile::Classic;
        sneak_toggled: bool = false;
        sneak_held: bool = false;
        jump_held: bool = false;
        sprinting: bool = false;
        sprint_direction: i8 = 0;
        sprint_left_tap_ticks: u8 = 0;
        sprint_right_tap_ticks: u8 = 0;
        jump_buffer_ticks: u8 = 0;
        coyote_ticks: u8 = 0;
        completion_credits_seen: bool = false;
        credits_active: bool = false;
        credits_tick: u32 = 0;
        death_screen_active: bool = false;
        death_screen_ticks: u16 = 0;
        respawn_grace_ticks: u16 = 0;
        player_combat_hurt_cooldown: u8 = 0;
        bow_draw_ticks: u8 = 0;
        bow_draw_active: bool = false;
        fishing_bobber_active: bool = false;
        fishing_bobber_x: f64 = 0.0;
        fishing_bobber_y: f64 = 0.0;
        fishing_wait_ticks: u16 = 0;
        fishing_bite_window_ticks: u8 = 0;
        mounted_boat: Option<usize> = None;
        furnace_job: Option<FurnaceJob> = None;
        furnace_progress_ticks: u16 = 0;
        furnace_burn_ticks: u16 = 0;
        potion_strength_timer: u16 = 0;
        potion_regeneration_timer: u16 = 0;
        potion_fire_resistance_timer: u16 = 0;
}
mod array_serde {
    use serde::{Deserialize, Deserializer, Serialize, Serializer};
    pub fn serialize<S: Serializer, const N: usize>(v: &[u8; N], s: S) -> Result<S::Ok, S::Error> {
        v.as_slice().serialize(s)
    }
    pub fn deserialize<'de, D: Deserializer<'de>, const N: usize>(
        d: D,
    ) -> Result<[u8; N], D::Error> {
        Vec::<u8>::deserialize(d)?
            .try_into()
            .map_err(|_| serde::de::Error::custom("invalid enchantment array length"))
    }
}
impl GameState {
    pub(crate) fn director_near_player(&self, x: f64, distance: f64) -> bool {
        (self.player.x - x).abs() < distance
            || self
                .other_players_here()
                .any(|s| (s.player.x - x).abs() < distance)
    }
    pub(crate) fn cooperative(&self) -> bool {
        self.active_peer.is_some()
    }
    pub(crate) fn peer_ids(&self) -> Vec<u16> {
        let mut ids: Vec<_> = self.cooperative_players.keys().copied().collect();
        if let Some(id) = self.active_peer {
            ids.push(id);
        }
        ids.sort_unstable();
        ids
    }
    pub(crate) fn peer_session(&self, id: u16) -> Option<PlayerSession> {
        if self.active_peer == Some(id) {
            Some(PlayerSession::capture(self))
        } else {
            self.cooperative_players.get(&id).cloned()
        }
    }
    pub(crate) fn activate_peer(&mut self, id: u16) -> bool {
        if self.active_peer == Some(id) {
            self.select_dimension(self.peer_dimension(id));
            if !self.world.is_replica()
                && (!self
                    .world
                    .is_chunk_loaded_at((self.player.x - 1.0).floor() as i32)
                    || !self
                        .world
                        .is_chunk_loaded_at((self.player.x + 1.0).floor() as i32))
            {
                self.world
                    .load_chunks_for_motion(self.player.x, self.player.vx);
            }
            return true;
        }
        let dimension = self.peer_dimension(id);
        let Some(mut next) = self.cooperative_players.remove(&id) else {
            return false;
        };
        next.swap(self);
        if let Some(old) = self.active_peer.replace(id) {
            self.cooperative_players.insert(old, next);
        }
        self.select_dimension(dimension);
        if !self.world.is_replica()
            && (!self
                .world
                .is_chunk_loaded_at((self.player.x - 1.0).floor() as i32)
                || !self
                    .world
                    .is_chunk_loaded_at((self.player.x + 1.0).floor() as i32))
        {
            self.world
                .load_chunks_for_motion(self.player.x, self.player.vx);
        }
        true
    }
    pub(crate) fn join_peer(&mut self, id: u16) {
        if self.peer_ids().contains(&id) {
            return;
        }
        let selected = self.current_dimension;
        self.select_dimension(Dimension::Overworld);
        let center = if selected == Dimension::Overworld {
            self.player.x.floor() as i32
        } else {
            0
        };
        let (x, y) = self.multiplayer_join_spawn_near(center);
        self.select_dimension(selected);
        self.cooperative_dimensions.insert(id, Dimension::Overworld);
        let mut actor = PlayerSession::fresh(x, y);
        if self.trial.is_some() {
            for (item, n) in [
                (ItemType::StoneSword, 1),
                (ItemType::WoodAxe, 1),
                (ItemType::WoodPickaxe, 1),
                (ItemType::Planks, 6),
                (ItemType::Torch, 4),
                (ItemType::Bread, 2),
            ] {
                actor.inventory.add_item(item, n);
            }
        }
        if self.active_peer.is_none() {
            self.select_dimension(Dimension::Overworld);
            actor.apply(self);
            self.active_peer = Some(id);
        } else {
            self.cooperative_players.insert(id, actor);
        }
        self.dismiss_startup_splash();
    }
    pub(crate) fn leave_peer(&mut self, id: u16) {
        if self.active_peer == Some(id) {
            if let Some(next) = self.cooperative_players.keys().copied().next() {
                self.activate_peer(next);
            } else {
                self.active_peer = None;
            }
        }
        self.cooperative_players.remove(&id);
        self.cooperative_dimensions.remove(&id);
    }
    pub(crate) fn select_nearest_peer(&mut self, x: f64, y: f64) {
        if !self.cooperative() {
            return;
        }
        let distance = |p: &Player| (p.x - x).powi(2) + (p.y - y).powi(2);
        let mut best = if self.player.health > 0.0 && !self.death_screen_active {
            self.active_peer.map(|id| (id, distance(&self.player)))
        } else {
            None
        };
        for (&id, actor) in &self.cooperative_players {
            if self.peer_dimension(id) != self.current_dimension {
                continue;
            }
            if actor.player.health <= 0.0 || actor.death_screen_active {
                continue;
            }
            let d = distance(&actor.player);
            if best.is_none_or(|(old, dist)| d < dist || (d == dist && id < old)) {
                best = Some((id, d));
            }
        }
        if let Some((id, _)) = best {
            self.activate_peer(id);
        }
    }
    pub(crate) fn finish_cooperative_damage(&mut self) {
        if self.player.health <= 0.0 && !self.death_screen_active {
            self.start_death_screen();
        }
    }
    pub(crate) fn tick_cooperative_player(&mut self, x: i32, y: i32) {
        self.handle_portal_transition();
        if self.credits_active {
            self.update_completion_credits();
            return;
        }
        self.update_furnace();
        if self.death_screen_active {
            self.death_screen_ticks = self.death_screen_ticks.saturating_add(1);
            return;
        }
        if self.player.health <= 0.0 {
            self.start_death_screen();
            return;
        }
        self.respawn_grace_ticks = self.respawn_grace_ticks.saturating_sub(1);
        self.player_combat_hurt_cooldown = self.player_combat_hurt_cooldown.saturating_sub(1);
        self.world
            .load_chunks_for_motion(self.player.x, self.player.vx);
        self.close_chest_if_out_of_range();
        self.sanitize_enchant_levels();
        let mut rng = rand::thread_rng();
        self.tick_player_survival(&mut rng);
        self.tick_player_actions(x, y);
        if self.player.health <= 0.0 {
            self.start_death_screen();
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SharedSettings {
    time: f32,
    weather: u8,
    weather_timer: u32,
    tick: u64,
    difficulty: u8,
    rules_preset: u8,
    mob_spawning: bool,
    daylight: bool,
    weather_cycle: bool,
    keep_inventory: bool,
}
impl SharedSettings {
    pub fn capture(s: &GameState) -> Self {
        Self {
            time: s.time_of_day,
            weather: GameState::encode_weather(s.weather),
            weather_timer: s.weather_timer,
            tick: s.world_tick,
            difficulty: GameState::encode_difficulty(s.difficulty),
            rules_preset: GameState::encode_game_rules_preset(s.game_rules_preset),
            mob_spawning: s.game_rules.do_mob_spawning,
            daylight: s.game_rules.do_daylight_cycle,
            weather_cycle: s.game_rules.do_weather_cycle,
            keep_inventory: s.game_rules.keep_inventory,
        }
    }
    pub fn apply(&self, s: &mut GameState) -> Result<(), String> {
        if !self.time.is_finite() || !(0.0..24000.0).contains(&self.time) {
            return Err("invalid server clock".into());
        }
        s.weather =
            GameState::decode_weather(self.weather).ok_or("invalid saved server weather")?;
        s.difficulty = GameState::decode_difficulty(self.difficulty)
            .ok_or("invalid saved server difficulty")?;
        s.game_rules_preset = GameState::decode_game_rules_preset(self.rules_preset)
            .ok_or("invalid saved server rules")?;
        s.time_of_day = self.time;
        s.weather_timer = self.weather_timer;
        s.world_tick = self.tick;
        s.game_rules = GameRules {
            do_mob_spawning: self.mob_spawning,
            do_daylight_cycle: self.daylight,
            do_weather_cycle: self.weather_cycle,
            keep_inventory: self.keep_inventory,
        };
        Ok(())
    }
}
impl PlayerSession {
    pub(crate) fn chest(&self, state: &GameState) -> Option<(i32, i32, Inventory)> {
        let (x, y) = self.open_chest_pos?;
        Some((x, y, state.world.chest_inventory(x, y)?.clone()))
    }
    pub fn validate(&self) -> Result<(), String> {
        let p = &self.player;
        if !p.x.is_finite()
            || !p.y.is_finite()
            || p.x.abs() > 100_000.0
            || !(-128.0..256.0).contains(&p.y)
            || !p.vx.is_finite()
            || !p.vy.is_finite()
            || !p.health.is_finite()
            || !p.hunger.is_finite()
            || !(0.0..=100.0).contains(&p.max_health)
            || !(0.0..=100.0).contains(&p.max_hunger)
            || self.inventory.capacity != PLAYER_INVENTORY_CAPACITY
            || self.inventory.slots.len() != PLAYER_INVENTORY_CAPACITY
            || self.hotbar_index >= 9
            || self
                .inventory
                .slots
                .iter()
                .chain(self.armor_slots.iter())
                .chain(self.crafting_grid.iter())
                .flatten()
                .any(|s| s.count == 0 || s.count > 64)
        {
            return Err("invalid multiplayer player record; save preserved".into());
        }
        Ok(())
    }
    pub fn release_inputs(&mut self) {
        self.moving_left = false;
        self.moving_right = false;
        self.jump_held = false;
        self.sneak_held = false;
        self.left_click_down = false;
        self.jump_buffer_ticks = 0;
        self.sprinting = false;
        self.sprint_direction = 0;
        self.player.mining_timer = 0.0;
        self.player.vx = 0.0;
        self.bow_draw_active = false;
        self.bow_draw_ticks = 0;
        self.fishing_bobber_active = false;
        self.mounted_boat = None;
    }
}
impl GameState {
    pub(crate) fn restore_peer(&mut self, id: u16, mut session: PlayerSession) {
        session.release_inputs();
        if self.active_peer == Some(id) {
            session.apply(self);
            if !self.world.is_replica() {
                self.world.load_chunks_around(self.player.x.floor() as i32);
            }
        } else {
            self.cooperative_players.insert(id, session);
        }
    }
    pub(crate) fn disconnect_peer_session(&mut self, id: u16) -> Option<PlayerSession> {
        if !self.activate_peer(id) {
            return None;
        }
        self.clear_container_interaction_state();
        self.action_visuals.remove(&id);
        let mut session = PlayerSession::capture(self);
        session.release_inputs();
        Some(session)
    }
}

impl GameState {
    // Include the active actor exactly once, or the local player outside co-op.
    pub(super) fn each_player(&mut self, mut f: impl FnMut(&mut Self)) {
        let active = self.active_peer;
        let ids = self.peer_ids_here();
        if ids.is_empty() {
            f(self);
            return;
        }
        for id in ids {
            self.activate_peer(id);
            f(self);
        }
        if let Some(id) = active {
            self.activate_peer(id);
        }
    }
    pub(crate) fn tick_cooperative_boats(&mut self) {
        self.update_boats();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn state() -> GameState {
        let mut s = GameState::network_view();
        let blocks = vec![BlockType::Air; CHUNK_WIDTH * crate::world::chunk::CHUNK_HEIGHT];
        s.world.apply_chunk_column_snapshot(0, &blocks);
        s.active_peer = Some(1);
        s.player = Player::new(2.5, 10.0);
        s.cooperative_players
            .insert(2, PlayerSession::fresh(3.5, 10.0));
        s
    }
    #[test]
    fn explosion_and_lightning_damage_every_nearby_actor_once() {
        let mut s = state();
        s.world.recent_explosions.push((2, 9, 3));
        s.apply_world_explosion_impacts();
        assert!(s.peer_session(1).unwrap().player.health < 20.0);
        assert!(s.peer_session(2).unwrap().player.health < 20.0);
        let a = s.peer_session(1).unwrap().player.health;
        let b = s.peer_session(2).unwrap().player.health;
        s.apply_world_explosion_impacts();
        assert_eq!(s.peer_session(1).unwrap().player.health, a);
        assert_eq!(s.peer_session(2).unwrap().player.health, b);
        s.player.health = 20.0;
        s.activate_peer(2);
        s.player.health = 20.0;
        s.apply_lightning_strike(2, 10);
        assert_eq!(s.peer_session(1).unwrap().player.health, 15.0);
        assert_eq!(s.peer_session(2).unwrap().player.health, 15.0);
    }
    #[test]
    fn placement_and_spawn_exclusion_consider_other_players() {
        let s = state();
        assert!(s.block_intersects_player_bounds(3, 9));
        assert!(!s.block_intersects_player_bounds(7, 9));
        assert!(s.is_spawn_too_close_to_player(4.0, 10.0, 1.0));
    }
    #[test]
    fn boat_ownership_and_swap_removal_preserve_the_other_rider() {
        let mut s = state();
        s.boats.push(Boat::new(2.0, 10.0));
        s.boats.push(Boat::new(3.0, 10.0));
        assert!(s.try_mount_boat(1));
        s.activate_peer(2);
        assert!(!s.try_mount_boat(1));
        assert!(s.try_mount_boat(0));
        s.remove_boat_with_drop(0);
        assert_eq!(s.mounted_boat, None);
        assert_eq!(s.peer_session(1).unwrap().mounted_boat, Some(0));
    }
    #[test]
    fn full_actor_roundtrip_keeps_armor_crafting_effects_and_durability() {
        let mut s = state();
        s.armor_slots[0] = Some(ItemStack {
            item_type: ItemType::DiamondHelmet,
            count: 1,
            durability: Some(100),
        });
        s.crafting_grid[0] = Some(ItemStack {
            item_type: ItemType::Wood,
            count: 12,
            durability: None,
        });
        s.potion_strength_timer = 450;
        s.inventory_enchant_levels[0] = 3;
        let saved = PlayerSession::capture(&s);
        saved.validate().unwrap();
        let restored: PlayerSession =
            serde_json::from_slice(&serde_json::to_vec(&saved).unwrap()).unwrap();
        assert_eq!(saved, restored);
        s.activate_peer(2);
        assert!(s.armor_slots[0].is_none());
        s.activate_peer(1);
        assert_eq!(PlayerSession::capture(&s), saved);
    }
}
