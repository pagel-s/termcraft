use super::*;

impl GameState {
    pub(super) fn hostile_target(&mut self, x: f64, y: f64, eye: f64) -> (f64, f64, bool) {
        let visible = |p: &Player| {
            p.health > 0.0
                && (p.x - x).hypot(p.y - y) < 48.0
                && self.has_line_of_sight(x, y - eye, p.x, p.y - 1.5)
        };
        if !self.cooperative() {
            return if visible(&self.player) {
                (self.player.x, self.player.y, true)
            } else {
                (x, y + 1000.0, false)
            };
        }
        let best = self
            .peer_ids_here()
            .into_iter()
            .filter_map(|id| {
                let p = self.peer_session(id)?;
                visible(p.player()).then_some((id, (p.player().x - x).hypot(p.player().y - y)))
            })
            .min_by(|a, b| a.1.total_cmp(&b.1));
        if let Some((id, _)) = best {
            self.activate_peer(id);
            (self.player.x, self.player.y, true)
        } else {
            (x, y + 1000.0, false)
        }
    }

    /// Older builds let loose entities fall through missing chunks. Recover
    /// their value at the same X above real terrain; never regenerate the End void.
    pub(super) fn recover_loose_void_fall(&mut self, x: f64, y: f64) -> Option<f64> {
        if self.current_dimension == Dimension::End
            || !x.is_finite()
            || x.abs() > 1_000_000.0
            || !y.is_finite()
            || y < 128.0
        {
            return None;
        }
        let bx = x.floor() as i32;
        self.world.load_chunks_for_spawn_search(bx, 1);
        (3..128)
            .find(|&by| {
                self.world.get_block(bx, by).is_solid()
                    && !self.world.get_block(bx, by - 1).is_solid()
                    && !self.world.get_block(bx, by - 2).is_solid()
            })
            .map(|y| y as f64 - 0.1)
    }

    pub(super) fn body_terrain_loaded(&self, x: f64, y: f64, vx: f64, half: f64) -> bool {
        y.is_finite()
            && self
                .world
                .is_chunk_loaded_at((x.min(x + vx) - half).floor() as i32)
            && self
                .world
                .is_chunk_loaded_at((x.max(x + vx) + half).floor() as i32)
    }

    pub(super) fn tick_creature_environment(&mut self) {
        macro_rules! hazards {
            ($field:ident,$half:expr,$height:expr,$fireproof:expr,$aquatic:expr,$air:expr) => {
                hazards!(
                    $field,
                    $half,
                    $height,
                    $fireproof,
                    $aquatic,
                    $air,
                    |_: &_| 1.0
                );
            };
            ($field:ident,$half:expr,$height:expr,$fireproof:expr,$aquatic:expr,$air:expr,$scale:expr) => {
                for i in 0..self.$field.len() {
                    let e = &self.$field[i];
                    if e.health <= 0.0 || !self.body_terrain_loaded(e.x, e.y, e.vx, $half) {
                        continue;
                    }
                    let scale = $scale(e);
                    let (water, lava) =
                        self.entity_fluid_submersion(e.x, e.y, $half * scale, $height * scale);
                    let head = self.world.fluid_at_point(e.x, e.y - $height * scale + 0.25);
                    let submerged = matches!(head, BlockType::Water(_));
                    let fire = matches!(
                        self.world
                            .get_block(e.x.floor() as i32, (e.y - 0.5).floor() as i32),
                        BlockType::Fire(_)
                    );
                    let wet = water > 0.0 || self.is_weather_wet_at(e.x, e.y - $height + 0.25);
                    let e = &mut self.$field[i];
                    let damage = e.hazards.tick(
                        submerged,
                        wet,
                        lava > 0.0,
                        fire,
                        $fireproof,
                        $aquatic,
                        $air,
                    );
                    if damage > 0.0 {
                        e.health -= damage;
                        e.hit_timer = e.hit_timer.max(8);
                    }
                }
            };
        }
        hazards!(zombies, 0.3, 1.8, false, false, false);
        hazards!(skeletons, 0.3, 1.8, false, false, false);
        hazards!(pigmen, 0.3, 1.8, true, false, false);
        hazards!(creepers, 0.3, 1.8, false, false, true);
        hazards!(spiders, 0.8, 0.75, false, false, true);
        hazards!(silverfish, 0.4, 0.5, false, false, true);
        for i in 0..self.slimes.len() {
            let e = &self.slimes[i];
            if e.health <= 0.0 || !self.body_terrain_loaded(e.x, e.y, e.vx, e.half_width()) {
                continue;
            }
            let (water, lava) = self.entity_fluid_submersion(e.x, e.y, e.half_width(), e.height());
            let submerged = matches!(
                self.world.fluid_at_point(e.x, e.y - e.height() + 0.15),
                BlockType::Water(_)
            );
            let e = &mut self.slimes[i];
            let damage = e.hazards.tick(
                submerged,
                water > 0.0,
                lava > 0.0,
                false,
                false,
                false,
                true,
            );
            if damage > 0.0 {
                e.health -= damage;
                e.hit_timer = 8;
            }
        }
        hazards!(endermen, 0.3, 2.75, false, false, true);
        hazards!(ghasts, 0.875, 1.875, true, false, true);
        hazards!(blazes, 0.5, 1.75, true, false, true);
        hazards!(cows, 0.65, 1.375, false, false, true, |e: &Cow| e
            .husbandry
            .scale());
        hazards!(sheep, 0.65, 1.125, false, false, true, |e: &Sheep| e
            .husbandry
            .scale());
        hazards!(pigs, 0.65, 1.0, false, false, true, |e: &Pig| e
            .husbandry
            .scale());
        hazards!(chickens, 0.3, 1.0, false, false, true, |e: &Chicken| e
            .husbandry
            .scale());
        hazards!(squids, 0.6, 1.0, false, true, true);
        hazards!(wolves, 0.6, 0.875, false, false, true);
        hazards!(ocelots, 0.6, 0.75, false, false, true);
        hazards!(villagers, 0.3, 1.8, false, false, true);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn room() -> GameState {
        let mut s = GameState::network_view();
        for c in -1..=1 {
            s.world
                .apply_chunk_column_snapshot(c, &vec![BlockType::Air; CHUNK_WIDTH * CHUNK_HEIGHT]);
        }
        for x in -16..32 {
            s.world.set_block(x, 20, BlockType::Stone);
        }
        s.player = Player::new(0.5, 20.0);
        s.game_rules.do_mob_spawning = false;
        s.game_rules.do_daylight_cycle = false;
        s.game_rules.do_weather_cycle = false;
        s
    }
    #[test]
    fn solid_corner_blocks_rays_and_mining_but_exposed_faces_remain_reachable() {
        let mut s = room();
        s.world.set_block(1, 18, BlockType::Stone);
        s.world.set_block(0, 19, BlockType::Stone);
        s.world.set_block(1, 19, BlockType::DiamondOre);
        assert!(!s.can_see_block(0.5, 18.5, 1, 19));
        assert!(s.terrain_segment_hit(0.5, 18.5, 1.5, 19.5, false).is_some());
        s.world.set_block(0, 19, BlockType::Air);
        assert!(s.can_see_block(0.5, 18.5, 1, 19));
        s.world.set_block(1, 18, BlockType::StoneSlab);
        assert!(s.has_line_of_sight(0.5, 18.25, 2.5, 18.25));
        assert!(!s.has_line_of_sight(0.5, 18.75, 2.5, 18.75));
    }
    #[test]
    fn hidden_players_do_not_trigger_fuse_bow_teleport_or_fireballs() {
        let mut s = room();
        // Wall covers all creature eyes and prevents jumping over it during the test.
        for y in 0..20 {
            s.world.set_block(2, y, BlockType::Stone);
        }
        s.zombies.push(Zombie::new(3.5, 20.0));
        s.creepers.push(Creeper::new(3.5, 20.0));
        s.skeletons.push(Skeleton::new(5.5, 20.0));
        s.skeletons[0].bow_windup = 1;
        s.endermen.push(Enderman::new(5.5, 20.0));
        s.blazes.push(Blaze::new(5.5, 19.0));
        s.ghasts.push(Ghast::new(6.5, 18.0));
        for _ in 0..12 {
            s.update(0, 0);
        }
        assert_eq!(s.zombies[0].vx, 0.0);
        assert_eq!(s.creepers[0].fuse_timer, 0);
        assert_eq!(s.skeletons[0].bow_windup, 0);
        assert_eq!(s.endermen[0].aggressive_timer, 0);
        assert!(s.arrows.is_empty());
        assert!(s.fireballs.is_empty());
        for y in 0..20 {
            s.world.set_block(2, y, BlockType::Air);
        }
        s.blazes[0].shoot_cooldown = 0;
        s.ghasts[0].shoot_cooldown = 0;
        s.update(0, 0);
        assert!(s.endermen[0].aggressive_timer > 0);
        assert!(!s.fireballs.is_empty());
    }
    #[test]
    fn coop_hostile_selects_visible_player_instead_of_nearest_hidden_player() {
        let mut s = room();
        s.active_peer = Some(1);
        s.player = Player::new(1.5, 20.0);
        let original = s.player.clone();
        s.player.x = 7.5;
        let peer = PlayerSession::capture(&s);
        s.player = original;
        s.cooperative_players.insert(2, peer);
        for y in 0..20 {
            s.world.set_block(2, y, BlockType::Stone);
        }
        let (x, _, visible) = s.hostile_target(3.5, 20.0, 1.5);
        assert!(visible);
        assert_eq!(x, 7.5);
        assert_eq!(s.active_peer, Some(2));
    }
    #[test]
    fn actual_fluid_contacts_damage_creatures_and_players_with_expected_immunities() {
        let mut s = room();
        for x in 3..=8 {
            for y in 17..20 {
                s.world.set_block(x, y, BlockType::Lava(8));
            }
        }
        s.cows.push(Cow::new(3.5, 20.0));
        s.zombies.push(Zombie::new(4.5, 20.0));
        s.blazes.push(Blaze::new(5.5, 20.0));
        s.pigmen.push(ZombiePigman::new(6.5, 20.0));
        s.ghasts.push(Ghast::new(7.5, 20.0));
        for _ in 0..20 {
            s.tick_creature_environment();
        }
        assert_eq!(s.cows[0].health, 2.0);
        assert_eq!(s.zombies[0].health, 12.0);
        assert_eq!(s.blazes[0].health, 20.0);
        assert_eq!(s.pigmen[0].health, 20.0);
        assert_eq!(s.ghasts[0].health, 20.0);
        s.cows.clear();
        s.cows.push(Cow::new(10.5, 20.0));
        for y in 17..20 {
            s.world.set_block(10, y, BlockType::Water(8));
        }
        for _ in 0..320 {
            s.tick_creature_environment();
        }
        assert_eq!(s.cows[0].health, 8.0);
        s.player.x = 10.5;
        s.player.drowning_timer = 0;
        s.player.hunger = 0.0;
        for (slot, item) in [
            ItemType::DiamondHelmet,
            ItemType::DiamondChestplate,
            ItemType::DiamondLeggings,
            ItemType::DiamondBoots,
        ]
        .into_iter()
        .enumerate()
        {
            s.armor_slots[slot] = Some(ItemStack {
                item_type: item,
                count: 1,
                durability: item.max_durability(),
            });
        }
        let mut rng = rand::thread_rng();
        for _ in 0..20 {
            s.tick_player_survival(&mut rng);
        }
        assert_eq!(s.player.health, 18.0, "armor cannot cancel drowning");
    }
}
