//! Aim-driven flight and local, collision-checked landings for thrown pearls.
use super::*;

const THROW_COOLDOWN: u64 = 20;
const TELEPORT_DAMAGE: f32 = 5.0;

impl GameState {
    pub(super) fn throw_ender_pearl(&mut self, target_x: i32, target_y: i32) {
        let owner = self.selected_peer_id();
        let key = owner.unwrap_or(0);
        if self.player.health <= 0.0
            || self.death_screen_active
            || self.last_pearl_throw_age.get(&key).is_some_and(|&last| {
                self.player.age >= last && self.player.age - last < THROW_COOLDOWN
            })
        {
            return;
        }
        let origin = (self.player.x, self.player.y - 1.35);
        let delta = (
            f64::from(target_x) + 0.5 - origin.0,
            f64::from(target_y) + 0.5 - origin.1,
        );
        let distance = delta.0.hypot(delta.1);
        let direction = if distance < 0.1 {
            (if self.player.facing_right { 1.0 } else { -1.0 }, 0.0)
        } else {
            (delta.0 / distance, delta.1 / distance)
        };
        if !self.consume_held_item(ItemType::EnderPearl) {
            return;
        }
        self.last_pearl_throw_age.insert(key, self.player.age);
        self.player.facing_right = direction.0 >= 0.0;
        self.ender_pearls
            .push(EnderPearl::new(origin.0, origin.1, direction, owner));
    }

    pub(super) fn cancel_ender_pearls(&mut self) {
        let owner = self.selected_peer_id();
        self.ender_pearls.retain(|p| p.owner != owner);
        self.last_pearl_throw_age.remove(&owner.unwrap_or(0));
    }

    fn pearl_landing(&self, impact: (f64, f64), previous: (f64, f64)) -> Option<(f64, f64)> {
        let delta = (impact.0 - previous.0, impact.1 - previous.1);
        let length = delta.0.hypot(delta.1).max(0.0001);
        // Start the visibility test on the incoming side of the collision. A
        // nearby empty cave on the other side of a wall is not a valid landing.
        let anchor = (
            impact.0 - delta.0 / length * 0.01,
            impact.1 - delta.1 / length * 0.01,
        );
        let base_x = impact.0 - delta.0 / length * (PLAYER_HALF_WIDTH + 0.02);
        let mut best = None;
        let mut best_distance = f64::INFINITY;
        // Only make room for the player's body immediately at impact. Never
        // search a whole vertical column or generate remote terrain to land.
        for dx in -4..=4 {
            for dy in -8..=8 {
                let x = base_x + f64::from(dx) * 0.25;
                let y = impact.1 - 0.01 + f64::from(dy) * 0.25;
                let distance = (x - impact.0).powi(2) + (y - impact.1).powi(2);
                if distance >= best_distance
                    || y < PLAYER_HEIGHT
                    || y >= CHUNK_HEIGHT as f64 - 1.0
                    || !self
                        .world
                        .is_chunk_loaded_at((x - PLAYER_HALF_WIDTH).floor() as i32)
                    || !self
                        .world
                        .is_chunk_loaded_at((x + PLAYER_HALF_WIDTH).floor() as i32)
                    || self.pearl_body_blocked(x, y)
                    || self
                        .terrain_segment_hit(anchor.0, anchor.1, x, y - PLAYER_HEIGHT / 2.0, false)
                        .is_some()
                {
                    continue;
                }
                best = Some((x, y));
                best_distance = distance;
            }
        }
        best
    }

    fn pearl_body_blocked(&self, x: f64, y: f64) -> bool {
        let body = [
            x - PLAYER_HALF_WIDTH,
            y - PLAYER_HEIGHT,
            x + PLAYER_HALF_WIDTH,
            y,
        ];
        for by in body[1].floor() as i32..=(body[3] - 0.001).floor() as i32 {
            for bx in body[0].floor() as i32..=(body[2] - 0.001).floor() as i32 {
                if self.collision_bounds_at(bx, by).any(|b| {
                    body[0] < b[2] - 0.001
                        && body[2] > b[0] + 0.001
                        && body[1] < b[3] - 0.001
                        && body[3] > b[1] + 0.001
                }) {
                    return true;
                }
            }
        }
        false
    }

    pub(super) fn tick_ender_pearls(&mut self) {
        let active = self.selected_peer_id();
        for i in (0..self.ender_pearls.len()).rev() {
            if self.ender_pearls[i].impact_ticks > 0 {
                self.ender_pearls[i].impact_ticks -= 1;
                self.ender_pearls[i].age += 1;
                if self.ender_pearls[i].impact_ticks == 0 {
                    self.ender_pearls.swap_remove(i);
                }
                continue;
            }
            let owner = self.ender_pearls[i].owner;
            if let Some(id) = owner {
                if !self.peer_ids_here().contains(&id) || !self.activate_peer(id) {
                    self.ender_pearls.swap_remove(i);
                    continue;
                }
            } else if self.cooperative() {
                self.ender_pearls.swap_remove(i);
                continue;
            }
            if self.player.health <= 0.0 || self.death_screen_active {
                self.ender_pearls.swap_remove(i);
                continue;
            }
            let previous = (self.ender_pearls[i].x, self.ender_pearls[i].y);
            let underwater = self.world.fluid_at_point(previous.0, previous.1).is_fluid();
            self.ender_pearls[i].advance(underwater);
            let p = &self.ender_pearls[i];
            let next = (p.x, p.y);
            if p.age > EnderPearl::MAX_AGE
                || next.1 < -64.0
                || next.1 >= CHUNK_HEIGHT as f64
                || (next.0 - self.player.x).abs() > 128.0
            {
                self.ender_pearls.swap_remove(i);
                continue;
            }
            if !self.world.is_chunk_loaded_at(next.0.floor() as i32) {
                self.world
                    .load_chunks_for_motion(next.0, next.0 - previous.0);
            }
            let terrain = self.terrain_segment_hit(previous.0, previous.1, next.0, next.1, false);
            let travel = (next.0 - previous.0, next.1 - previous.1);
            let length2 = travel.0.powi(2) + travel.1.powi(2);
            let terrain_t = terrain.map_or(f64::INFINITY, |hit| {
                ((hit.0 - previous.0) * travel.0 + (hit.1 - previous.1) * travel.1)
                    / length2.max(0.000001)
            });
            let contact = self
                .projectile_body_contact_t(previous, next, owner)
                .filter(|&t| t < terrain_t)
                .map(|t| (previous.0 + travel.0 * t, previous.1 + travel.1 * t))
                .or(terrain);
            let Some(contact) = contact else { continue };
            self.ender_pearls[i].x = contact.0;
            self.ender_pearls[i].y = contact.1;
            self.ender_pearls[i].impact_ticks = 8;
            if let Some((x, y)) = self.pearl_landing(contact, previous) {
                self.clear_container_interaction_state();
                self.clear_fishing_line();
                self.mounted_boat = None;
                self.player.x = x;
                self.player.y = y;
                self.player.vx = 0.0;
                self.player.vy = 0.0;
                self.player.fall_distance = 0.0;
                self.player.grounded = false;
                self.apply_player_damage(TELEPORT_DAMAGE);
            } else if let Some(id) = owner {
                self.player_notices
                    .push((id, "Pearl impact: no room to land here.".into()));
            }
        }
        if let Some(id) = active {
            self.activate_peer(id);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn room() -> GameState {
        let mut s = GameState::network_view();
        s.dismiss_startup_splash();
        for cx in -4..=4 {
            s.world
                .apply_chunk_column_snapshot(cx, &vec![BlockType::Air; CHUNK_WIDTH * CHUNK_HEIGHT]);
        }
        for x in -60..=60 {
            s.world.set_block(x, 40, BlockType::Stone);
        }
        s.player.x = 0.5;
        s.player.y = 40.0;
        s.player.health = 20.0;
        s.player.grounded = true;
        s.inventory.slots.fill(None);
        s.inventory.slots[0] = Some(ItemStack {
            item_type: ItemType::EnderPearl,
            count: 3,
            durability: None,
        });
        s.hotbar_index = 0;
        s
    }
    fn impact(s: &mut GameState) {
        for _ in 0..=EnderPearl::MAX_AGE {
            s.tick_ender_pearls();
            if s.ender_pearls.iter().any(|p| p.impact_ticks > 0) {
                return;
            }
        }
        panic!("pearl did not hit the fixture: {:?}", s.ender_pearls);
    }
    #[test]
    fn throwing_is_visible_before_impact_and_only_impact_teleports() {
        let mut s = room();
        s.apply_client_command(ClientCommand::UseAt(24, 27));
        assert_eq!((s.player.x, s.player.y, s.player.health), (0.5, 40.0, 20.0));
        assert_eq!(s.ender_pearls.len(), 1);
        assert_eq!(s.inventory.slots[0].as_ref().unwrap().count, 2);
        assert_eq!(
            s.action_visual(0.0).unwrap().item,
            Some(ItemType::EnderPearl)
        );
        let launch = s.ender_pearls[0].clone();
        for _ in 0..8 {
            s.tick_ender_pearls();
        }
        assert!(s.ender_pearls[0].x > launch.x + 6.0);
        assert!(s.ender_pearls[0].y < launch.y);
        assert_eq!(s.player.x, 0.5);
        impact(&mut s);
        assert!(s.player.x > 15.0 && s.player.x < 55.0, "{}", s.player.x);
        assert!((s.player.y - 40.0).abs() < 0.05);
        assert_eq!(s.player.health, 15.0);
        assert!(!s.pearl_body_blocked(s.player.x, s.player.y));
        assert_eq!(
            (s.player.vx, s.player.vy, s.player.fall_distance),
            (0.0, 0.0, 0.0)
        );
        for _ in 0..8 {
            s.tick_ender_pearls();
        }
        assert!(s.ender_pearls.is_empty());
        assert_eq!(
            s.player.health, 15.0,
            "impact effect must not teleport/damage again"
        );
    }
    #[test]
    fn distant_cursor_controls_direction_without_a_reach_gate_and_click_spam_is_throttled() {
        let mut s = room();
        let loaded = s.world.loaded_chunk_count();
        s.interact_block(1000, 20, false);
        s.interact_block(1000, 20, false);
        assert_eq!(s.ender_pearls.len(), 1);
        assert_eq!(s.inventory.slots[0].as_ref().unwrap().count, 2);
        assert_eq!(
            s.world.loaded_chunk_count(),
            loaded,
            "aiming must not generate distant chunks"
        );
        s.player.age += THROW_COOLDOWN;
        s.interact_block(-1000, 20, false);
        assert_eq!(s.ender_pearls.len(), 2);
        assert!(s.ender_pearls[1].vx < 0.0);
    }
    #[test]
    fn a_wall_intercepts_a_throw_and_never_teleports_to_its_far_side() {
        let mut s = room();
        for y in 0..40 {
            s.world.set_block(6, y, BlockType::Stone);
        }
        s.interact_block(25, 32, false);
        assert_eq!(
            s.ender_pearls.len(),
            1,
            "aiming through a wall must still throw"
        );
        impact(&mut s);
        assert!(s.player.x > 5.0 && s.player.x + PLAYER_HALF_WIDTH < 6.001);
        assert!(!s.pearl_body_blocked(s.player.x, s.player.y));
        assert!((s.ender_pearls[0].x - 6.0).abs() < 0.001);
    }
    #[test]
    fn slab_and_ceiling_landings_check_the_actual_player_body() {
        let mut s = room();
        for x in 2..=8 {
            s.world.set_block(x, 37, BlockType::StoneSlab);
        }
        let landing = s.pearl_landing((4.5, 37.5), (4.5, 37.0)).unwrap();
        assert!((landing.1 - 37.5).abs() < 0.02);
        assert!(!s.pearl_body_blocked(landing.0, landing.1));
        for x in -3..=1 {
            s.world.set_block(x, 35, BlockType::Stone);
        }
        let landing = s.pearl_landing((0.5, 36.0), (0.5, 36.5)).unwrap();
        assert!(landing.1 - PLAYER_HEIGHT >= 35.999);
        assert!(!s.pearl_body_blocked(landing.0, landing.1));
    }
    #[test]
    fn a_one_block_crevice_does_not_warp_the_player_to_a_distant_cave() {
        let mut s = room();
        for x in -3..=3 {
            for y in 5..=38 {
                s.world.set_block(x, y, BlockType::Stone);
            }
        }
        s.world.set_block(0, 20, BlockType::Air);
        assert!(s.pearl_landing((0.9, 20.5), (0.5, 20.5)).is_none());
    }
    #[test]
    fn multiplayer_teleports_the_thrower_and_restores_the_selected_actor() {
        let mut s = room();
        s.join_peer(1);
        s.join_peer(2);
        s.activate_peer(1);
        s.player.x = -15.5;
        s.player.y = 40.0;
        s.player.health = 20.0;
        let first = (s.player.x, s.player.y, s.player.health);
        s.activate_peer(2);
        s.player.x = 0.5;
        s.player.y = 40.0;
        s.player.health = 20.0;
        s.inventory.slots[0] = Some(ItemStack {
            item_type: ItemType::EnderPearl,
            count: 2,
            durability: None,
        });
        s.interact_block(24, 27, false);
        s.activate_peer(1);
        impact(&mut s);
        assert_eq!(s.selected_peer_id(), Some(1));
        assert_eq!((s.player.x, s.player.y, s.player.health), first);
        let thrower = s.peer_session(2).unwrap();
        assert!(thrower.player().x > 15.0);
        assert_eq!(thrower.player().health, 15.0);
        assert_eq!(thrower.inventory().slots[0].as_ref().unwrap().count, 1);
        s.activate_peer(2);
        s.player.age += THROW_COOLDOWN;
        s.interact_block(-24, 27, false);
        assert!(s.ender_pearls.iter().any(|p| p.impact_ticks == 0));
        s.disconnect_peer_session(2).unwrap();
        assert!(
            s.ender_pearls.is_empty(),
            "disconnect must cancel that connection's pending teleport"
        );
        s.activate_peer(1);
        assert_eq!((s.player.x, s.player.y, s.player.health), first);
    }
    #[test]
    fn death_and_dimension_travel_cancel_pending_teleports() {
        let mut s = room();
        s.interact_block(24, 27, false);
        s.start_death_screen();
        assert!(s.ender_pearls.is_empty());
        s.death_screen_active = false;
        s.player.health = 20.0;
        s.interact_block(24, 27, false);
        assert_eq!(s.ender_pearls.len(), 1);
        s.travel_dimension(Dimension::Nether);
        s.travel_dimension(Dimension::Overworld);
        assert!(s.ender_pearls.is_empty());
    }
    #[test]
    fn pearls_are_transient_and_do_not_change_native_checkpoint_bytes() {
        let mut s = room();
        let before = bincode::serialize(&RuntimeCheckpoint::capture(&s)).unwrap();
        s.interact_block(24, 27, false);
        let during = bincode::serialize(&RuntimeCheckpoint::capture(&s)).unwrap();
        assert_eq!(before, during);
        let saved: RuntimeCheckpoint = bincode::deserialize(&during).unwrap();
        saved.validate().unwrap();
        saved.apply(&mut s);
        assert!(
            s.ender_pearls.is_empty(),
            "a saved connection must not resume a pending teleport"
        );
    }
}
