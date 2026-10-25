//! Shared cursor/body picking and readable, interruptible melee timing.
use super::*;

pub(super) fn melee_strike_ready(windup: &mut u8, cooldown: u8, contact: bool, hurt: bool) -> bool {
    if !contact || hurt || cooldown > 0 {
        *windup = 0;
        return false;
    }
    if *windup == 0 {
        *windup = 8;
        return false;
    }
    *windup -= 1;
    *windup == 0
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Target {
    Player(u16),
    Zombie(usize),
    Pigman(usize),
    Skeleton(usize),
    Creeper(usize),
    Spider(usize),
    Silverfish(usize),
    Slime(usize),
    Enderman(usize),
    Ghast(usize),
    Blaze(usize),
    Cow(usize),
    Sheep(usize),
    Pig(usize),
    Chicken(usize),
    Wolf(usize),
    Ocelot(usize),
    Squid(usize),
    Villager(usize),
    Crystal(usize),
    Dragon,
}
struct Body {
    target: Target,
    x: f64,
    y: f64,
    half: f64,
    height: f64,
    health: f32,
}
impl GameState {
    fn combat_bodies(&self) -> Vec<Body> {
        self.combat_bodies_for(self.active_peer.or(self.local_player_id))
    }
    fn combat_bodies_for(&self, attacker: Option<u16>) -> Vec<Body> {
        let mut bodies = Vec::new();
        macro_rules! bodies {
            ($field:ident, $kind:ident, $half:expr, $height:expr) => {
                for (i, e) in self
                    .$field
                    .iter()
                    .enumerate()
                    .filter(|(_, e)| e.health > 0.0)
                {
                    bodies.push(Body {
                        target: Target::$kind(i),
                        x: e.x,
                        y: e.y,
                        half: $half,
                        height: $height,
                        health: e.health,
                    });
                }
            };
        }
        bodies!(zombies, Zombie, 0.375, 1.75);
        bodies!(pigmen, Pigman, 0.375, 1.75);
        bodies!(skeletons, Skeleton, 0.375, 1.75);
        bodies!(creepers, Creeper, 0.375, 1.75);
        bodies!(spiders, Spider, 0.9, 0.75);
        bodies!(silverfish, Silverfish, 0.45, 0.5);
        for (i, e) in self
            .slimes
            .iter()
            .enumerate()
            .filter(|(_, e)| e.health > 0.0)
        {
            bodies.push(Body {
                target: Target::Slime(i),
                x: e.x,
                y: e.y,
                half: e.half_width(),
                height: e.height(),
                health: e.health,
            });
        }
        bodies!(endermen, Enderman, 0.375, 2.75);
        bodies!(ghasts, Ghast, 0.875, 1.875);
        bodies!(blazes, Blaze, 0.5, 1.75);
        bodies!(cows, Cow, 0.75, 1.375);
        bodies!(sheep, Sheep, 0.7, 1.125);
        bodies!(pigs, Pig, 0.7, 1.0);
        bodies!(chickens, Chicken, 0.375, 1.0);
        bodies!(wolves, Wolf, 0.7, 0.875);
        bodies!(ocelots, Ocelot, 0.75, 0.75);
        bodies!(squids, Squid, 0.625, 1.0);
        bodies!(villagers, Villager, 0.375, 1.75);
        bodies!(end_crystals, Crystal, 0.5, 1.0);
        if self.pvp_enabled && attacker.is_some() {
            let mut player_body = |id, x, y, health| {
                if Some(id) != attacker && health > 0.0 {
                    bodies.push(Body {
                        target: Target::Player(id),
                        x,
                        y,
                        half: PLAYER_HALF_WIDTH,
                        height: PLAYER_HEIGHT,
                        health,
                    });
                }
            };
            if let Some(id) = self.active_peer
                && !self.death_screen_active
            {
                player_body(id, self.player.x, self.player.y, self.player.health);
            }
            for (&id, actor) in &self.cooperative_players {
                if self.peer_dimension(id) == self.current_dimension && !actor.dead() {
                    let p = actor.player();
                    player_body(id, p.x, p.y, p.health);
                }
            }
            for remote in &self.remote_players {
                player_body(remote.client_id, remote.x, remote.y, remote.health);
            }
        }
        bodies
    }
    fn pick_combat_target(&self, mx: f64, my: f64) -> Option<Target> {
        let mut best: Option<(f64, Target)> = None;
        for b in self.combat_bodies() {
            let x = mx.clamp(b.x - b.half, b.x + b.half);
            let y = my.clamp(b.y - b.height, b.y);
            let distance = (mx - x).powi(2) + (my - y).powi(2);
            // A body may rest exactly on solid ground. Aim inside its visible
            // outline rather than ending the ray in that supporting block.
            let aim_x = x.clamp(b.x - b.half + 0.02, b.x + b.half - 0.02);
            let aim_y = y.clamp(b.y - b.height + 0.02, b.y - 0.02);
            let reach_x = self.player.x.clamp(b.x - b.half, b.x + b.half);
            let reach_y = (self.player.y - 0.9).clamp(b.y - b.height, b.y);
            let reachable = (self.player.x - reach_x).hypot(self.player.y - 0.9 - reach_y) <= 2.0;
            if distance > 0.36
                || !reachable
                || self
                    .terrain_segment_hit(self.player.x, self.player.y - 0.9, aim_x, aim_y, false)
                    .is_some()
            {
                continue;
            }
            let score =
                distance + 0.001 * ((mx - b.x).powi(2) + (my - (b.y - b.height * 0.5)).powi(2));
            if best.is_none_or(|(s, _)| score < s) {
                best = Some((score, b.target));
            }
        }
        if self
            .ender_dragon
            .as_ref()
            .is_some_and(|d| d.health > 0.0 && d.hit_test(mx, my))
            && self.can_melee_entity(mx, my, 4.4)
            && best.is_none_or(|(s, _)| s > 0.0)
        {
            return Some(Target::Dragon);
        }
        best.map(|(_, target)| target)
    }
    pub fn focused_entity(&self, x: i32, y: i32) -> Option<(&'static str, f32)> {
        let target = self.pick_combat_target(x as f64 + 0.5, y as f64 + 0.5)?;
        let name = match target {
            Target::Player(_) => "Player",
            Target::Zombie(_) => "Zombie",
            Target::Pigman(_) => "Pigman",
            Target::Skeleton(_) => "Skeleton",
            Target::Creeper(_) => "Creeper",
            Target::Spider(_) => "Spider",
            Target::Silverfish(_) => "Silverfish",
            Target::Slime(_) => "Slime",
            Target::Enderman(_) => "Enderman",
            Target::Ghast(_) => "Ghast",
            Target::Blaze(_) => "Blaze",
            Target::Cow(_) => "Cow",
            Target::Sheep(_) => "Sheep",
            Target::Pig(_) => "Pig",
            Target::Chicken(_) => "Chicken",
            Target::Wolf(_) => "Wolf",
            Target::Ocelot(_) => "Ocelot",
            Target::Squid(_) => "Squid",
            Target::Villager(_) => "Villager",
            Target::Crystal(_) => "Crystal",
            Target::Dragon => "Ender Dragon",
        };
        let health = if target == Target::Dragon {
            self.ender_dragon.as_ref()?.health
        } else {
            self.combat_bodies()
                .into_iter()
                .find(|b| b.target == target)?
                .health
        };
        Some((name, health))
    }
    fn first_arrow_body(
        &self,
        from: (f64, f64),
        to: (f64, f64),
        owner: Option<u16>,
    ) -> Option<(f64, Target)> {
        let mut best = None;
        for b in self.combat_bodies_for(owner) {
            if let Some(t) =
                segment_box_hit(from, to, [b.x - b.half, b.y - b.height, b.x + b.half, b.y])
                && best.is_none_or(|(old, _)| t < old)
            {
                best = Some((t, b.target));
            }
        }
        if let Some(d) = &self.ender_dragon {
            for r in d.hit_regions() {
                let (x, y) = ((from.0 - r[0]) / r[2], (from.1 - r[1]) / r[3]);
                let (dx, dy) = ((to.0 - from.0) / r[2], (to.1 - from.1) / r[3]);
                let (a, b, c) = (
                    dx * dx + dy * dy,
                    2.0 * (x * dx + y * dy),
                    x * x + y * y - 1.0,
                );
                let disc = b * b - 4.0 * a * c;
                let t = if c <= 0.0 {
                    Some(0.0)
                } else if a > 0.0 && disc >= 0.0 {
                    Some((-b - disc.sqrt()) / (2.0 * a))
                } else {
                    None
                };
                if let Some(t) = t.filter(|t| (0.0..=1.0).contains(t))
                    && best.is_none_or(|(old, _)| t < old)
                {
                    best = Some((t, Target::Dragon));
                }
            }
        }
        best
    }
    #[cfg(test)]
    pub(super) fn hit_player_arrow_segment(
        &mut self,
        from: (f64, f64),
        to: (f64, f64),
        damage: f32,
        terrain_t: f64,
    ) -> bool {
        self.hit_player_projectile_segment(from, to, damage, terrain_t)
            .is_some()
    }
    pub(super) fn hit_player_projectile_segment(
        &mut self,
        from: (f64, f64),
        to: (f64, f64),
        damage: f32,
        terrain_t: f64,
    ) -> Option<(f64, f64)> {
        self.hit_owned_projectile_segment(from, to, damage, terrain_t, None)
    }
    pub(super) fn hit_owned_player_arrow_segment(
        &mut self,
        from: (f64, f64),
        to: (f64, f64),
        damage: f32,
        terrain_t: f64,
        owner: Option<u16>,
    ) -> bool {
        self.hit_owned_projectile_segment(from, to, damage, terrain_t, owner)
            .is_some()
    }
    fn hit_owned_projectile_segment(
        &mut self,
        from: (f64, f64),
        to: (f64, f64),
        damage: f32,
        terrain_t: f64,
        owner: Option<u16>,
    ) -> Option<(f64, f64)> {
        if let Some((t, target)) = self
            .first_arrow_body(from, to, owner)
            .filter(|(t, _)| *t < terrain_t)
        {
            self.apply_target_hit(
                target,
                if target == Target::Dragon {
                    damage * 0.8
                } else {
                    damage
                },
                false,
                Some((
                    (to.0 - from.0).clamp(-0.6, 0.6),
                    (to.1 - from.1 - 0.15).clamp(-0.45, 0.2),
                )),
            );
            return Some((from.0 + (to.0 - from.0) * t, from.1 + (to.1 - from.1) * t));
        }
        None
    }
    pub(super) fn perform_melee(&mut self, mx: f64, my: f64) -> bool {
        let eye = (self.player.x, self.player.y - 0.9);
        let projectile = self
            .fireballs
            .iter()
            .enumerate()
            .filter(|(_, f)| {
                !f.dead
                    && (mx - f.x).hypot(my - f.y) <= 0.9
                    && (eye.0 - f.x).hypot(eye.1 - f.y) <= 2.6
                    && self
                        .terrain_segment_hit(eye.0, eye.1, f.x, f.y, false)
                        .is_none()
            })
            .min_by(|(_, a), (_, b)| {
                ((mx - a.x).powi(2) + (my - a.y).powi(2))
                    .total_cmp(&((mx - b.x).powi(2) + (my - b.y).powi(2)))
            })
            .map(|(i, _)| i);
        if let Some(i) = projectile {
            let f = &mut self.fireballs[i];
            // Reverse the incoming shot. This is predictable even when the
            // cursor has only one-block precision in an overview terminal.
            let speed = f.vx.hypot(f.vy);
            if speed > 0.001 {
                f.vx = -f.vx / speed * 0.48;
                f.vy = -f.vy / speed * 0.48;
            } else {
                f.vx = if self.player.facing_right {
                    0.48
                } else {
                    -0.48
                };
                f.vy = 0.0;
            }
            f.reflected = true;
            f.age = 0;
            return true;
        }
        let Some(target) = self.pick_combat_target(mx, my) else {
            return false;
        };
        let damage = self.effective_held_damage(self.current_hotbar_item_type());
        self.apply_target_hit(target, damage, true, None);
        true
    }
    fn apply_target_hit(
        &mut self,
        target: Target,
        damage: f32,
        allow_shear: bool,
        impulse: Option<(f64, f64)>,
    ) {
        let (px, tick) = (self.player.x, self.world_tick);
        macro_rules! hit {
            ($field:ident,$i:expr,$kb:expr,$lift:expr) => {{
                let e = &mut self.$field[$i];
                e.health -= damage;
                e.hit_timer = 10;
                e.last_player_damage_tick = tick;
                e.vx = if px < e.x { $kb } else { -$kb };
                e.vy = -$lift;
                if let Some((vx, vy)) = impulse {
                    e.vx = vx;
                    e.vy = vy;
                }
            }};
        }
        match target {
            Target::Player(id) => {
                let selected = self.active_peer;
                if self.activate_peer(id) {
                    if self.apply_player_combat_damage(damage) {
                        self.apply_player_contact_knockback(px, 0.32, 0.22);
                        if let Some((vx, vy)) = impulse {
                            self.player.vx = vx;
                            self.player.vy = vy;
                        }
                    }
                    if let Some(selected) = selected {
                        self.activate_peer(selected);
                    }
                }
            }
            Target::Zombie(i) => hit!(zombies, i, 0.4, 0.3),
            Target::Skeleton(i) => hit!(skeletons, i, 0.4, 0.3),
            Target::Creeper(i) => hit!(creepers, i, 0.4, 0.3),
            Target::Spider(i) => hit!(spiders, i, 0.4, 0.3),
            Target::Silverfish(i) => hit!(silverfish, i, 0.35, 0.22),
            Target::Slime(i) => {
                let kb = match self.slimes[i].size {
                    4 => 0.44,
                    2 => 0.38,
                    _ => 0.32,
                };
                hit!(slimes, i, kb, 0.2);
            }
            Target::Cow(i) => hit!(cows, i, 0.4, 0.3),
            Target::Sheep(i) => {
                if !allow_shear || !self.try_shear_sheep(i) {
                    hit!(sheep, i, 0.4, 0.3);
                }
            }
            Target::Pig(i) => hit!(pigs, i, 0.4, 0.3),
            Target::Chicken(i) => hit!(chickens, i, 0.35, 0.24),
            Target::Squid(i) => hit!(squids, i, 0.34, 0.24),
            Target::Villager(i) => hit!(villagers, i, 0.4, 0.3),
            Target::Ghast(i) => hit!(ghasts, i, 0.35, 0.15),
            Target::Blaze(i) => hit!(blazes, i, 0.32, 0.18),
            Target::Pigman(i) => {
                hit!(pigmen, i, 0.4, 0.3);
                let (x, y) = (self.pigmen[i].x, self.pigmen[i].y);
                for p in &mut self.pigmen {
                    if (p.x - x).hypot(p.y - y) < 12.0 {
                        p.provoke();
                    }
                }
            }
            Target::Enderman(i) => {
                hit!(endermen, i, 0.45, 0.35);
                self.endermen[i].provoke();
                self.endermen[i].teleport_cooldown = self.endermen[i].teleport_cooldown.min(8);
            }
            Target::Wolf(i) => {
                hit!(wolves, i, 0.36, 0.24);
                self.provoke_wolves_near(self.wolves[i].x, self.wolves[i].y, 12.0);
            }
            Target::Ocelot(i) => {
                hit!(ocelots, i, 0.34, 0.24);
                self.spook_ocelots_near(self.ocelots[i].x, self.ocelots[i].y, 12.0, px);
            }
            Target::Crystal(i) => {
                self.end_crystals[i].health -= damage * 2.0;
                self.end_crystals[i].hit_timer = 10;
            }
            Target::Dragon => {
                let d = self.ender_dragon.as_mut().unwrap();
                d.health -= damage;
                d.hit_timer = 10;
                d.vx += impulse.map_or_else(
                    || if px < d.x { 0.24 } else { -0.24 },
                    |(dx, _)| dx.signum() * 0.24,
                );
                d.vy = (d.vy - 0.12).clamp(-0.35, 0.35);
            }
        }
    }
}

pub(super) fn segment_box_hit(from: (f64, f64), to: (f64, f64), bounds: [f64; 4]) -> Option<f64> {
    let (mut enter, mut exit) = (0.0_f64, 1.0_f64);
    for (origin, delta, lo, hi) in [
        (from.0, to.0 - from.0, bounds[0], bounds[2]),
        (from.1, to.1 - from.1, bounds[1], bounds[3]),
    ] {
        if delta == 0.0 {
            if origin < lo || origin >= hi {
                return None;
            }
        } else {
            let (a, b) = ((lo - origin) / delta, (hi - origin) / delta);
            enter = enter.max(a.min(b));
            exit = exit.min(a.max(b));
        }
    }
    (enter <= exit && exit >= 0.0 && enter <= 1.0).then_some(enter)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn arena() -> GameState {
        let mut s = GameState::new();
        s.dismiss_startup_splash();
        s.game_rules.do_mob_spawning = false;
        s.game_rules.do_daylight_cycle = false;
        s.time_of_day = 23000.0;
        s.world.load_chunks_for_spawn_search(0, 16);
        for x in -8..=8 {
            for y in 0..24 {
                s.world.set_block(
                    x,
                    y,
                    if y >= 20 {
                        BlockType::Stone
                    } else {
                        BlockType::Air
                    },
                );
            }
        }
        s.zombies.clear();
        s.skeletons.clear();
        s.creepers.clear();
        s.player = Player::new(0.5, 20.0);
        s.player.grounded = true;
        s.world_tick = 1;
        s
    }
    #[test]
    fn nearest_species_body_wins_and_walls_and_distant_air_cannot_be_hit() {
        let mut s = arena();
        s.zombies.push(Zombie::new(1.3, 20.0));
        s.skeletons.push(Skeleton::new(2.0, 20.0));
        assert!(s.perform_melee(2.2, 19.1));
        assert_eq!(s.zombies[0].health, 20.0);
        assert!(s.skeletons[0].health < 20.0);
        assert!(!s.perform_melee(7.0, 19.0));
        s.zombies.clear();
        s.world.set_block(1, 19, BlockType::Stone);
        assert!(!s.perform_melee(2.2, 19.1));
    }
    #[test]
    fn cursor_cells_at_mob_feet_do_not_ray_cast_into_the_floor() {
        let mut s = arena();
        s.cows.push(Cow::new(1.8, 20.0));
        assert!(
            s.perform_melee(1.5, 20.5),
            "the visible feet cell selected the ground instead of the body"
        );
        assert!(s.cows[0].health < 10.0);
        s.cows[0].health = 10.0;
        s.world.set_block(1, 19, BlockType::Stone);
        assert!(
            !s.perform_melee(1.5, 20.5),
            "aim assistance must respect a blocking wall"
        );
    }
    #[test]
    fn cursor_cells_at_a_players_feet_use_the_same_pick_as_mobs() {
        let mut s = arena();
        s.join_peer(1);
        s.join_peer(2);
        s.activate_peer(2);
        s.player.x = 2.0;
        s.player.y = 20.0;
        s.activate_peer(1);
        s.player.x = 0.5;
        s.player.y = 20.0;
        assert!(s.perform_melee(2.5, 20.5));
        assert!(s.peer_session(2).unwrap().player().health < 20.0);
    }
    #[test]
    fn body_cells_of_small_tall_and_wide_mobs_are_consistently_pickable() {
        for kind in 0..12 {
            let mut s = arena();
            match kind {
                0 => s.zombies.push(Zombie::new(1.8, 20.0)),
                1 => s.skeletons.push(Skeleton::new(1.8, 20.0)),
                2 => s.creepers.push(Creeper::new(1.8, 20.0)),
                3 => s.spiders.push(Spider::new(1.8, 20.0)),
                4 => s.silverfish.push(Silverfish::new(1.8, 20.0)),
                5 => s.endermen.push(Enderman::new(1.8, 20.0)),
                6 => s.cows.push(Cow::new(1.8, 20.0)),
                7 => s.sheep.push(Sheep::new(1.8, 20.0)),
                8 => s.pigs.push(Pig::new(1.8, 20.0)),
                9 => s.chickens.push(Chicken::new(1.8, 20.0)),
                10 => s.villagers.push(Villager::new(1.8, 20.0, 0, 19)),
                _ => s.slimes.push(Slime::new(1.8, 20.0, 2)),
            }
            let b = s.combat_bodies().into_iter().next().unwrap();
            for x in (b.x - b.half).floor() as i32..=(b.x + b.half - 0.001).floor() as i32 {
                for y in (b.y - b.height).floor() as i32..=(b.y - 0.001).floor() as i32 {
                    assert_eq!(
                        s.pick_combat_target(x as f64 + 0.5, y as f64 + 0.5),
                        Some(b.target),
                        "visible body cell {x},{y} missed {:?}",
                        b.target
                    );
                }
            }
        }
    }
    #[test]
    fn fireballs_can_be_returned_to_ghasts_and_blazes_but_not_through_walls() {
        for blaze in [false, true] {
            let mut s = arena();
            s.player_combat_hurt_cooldown = 0;
            s.respawn_grace_ticks = 0;
            if blaze {
                s.blazes.push(Blaze::new(4.0, 20.0));
                s.blazes[0].shoot_cooldown = 100;
            } else {
                s.ghasts.push(Ghast::new(4.0, 20.0));
                s.ghasts[0].shoot_cooldown = 100;
            }
            s.fireballs.push(Fireball::new(1.8, 19.1, -0.3, 0.0));
            assert!(s.perform_melee(1.5, 19.5));
            assert!(s.fireballs[0].reflected && s.fireballs[0].vx > 0.4);
            let health = s.player.health;
            for _ in 0..5 {
                s.update(0, 0);
            }
            assert_eq!(s.player.health, health, "returned fireball hit its player");
            assert!(s.fireballs.is_empty());
            if blaze {
                assert!(s.blazes.is_empty() || s.blazes[0].health <= 10.0);
            } else {
                assert!(s.ghasts.is_empty() || s.ghasts[0].health <= 10.0);
            }
        }
        let mut s = arena();
        s.fireballs.push(Fireball::new(1.8, 19.1, -0.3, 0.0));
        s.world.set_block(1, 19, BlockType::Stone);
        assert!(!s.perform_melee(1.5, 19.5));
        assert!(!s.fireballs[0].reflected);
        s.world.set_block(1, 19, BlockType::Air);
        s.fireballs[0].x = 5.0;
        assert!(!s.perform_melee(5.0, 19.1));
    }
    #[test]
    fn lone_ocelot_can_jump_obstacles_without_wolves() {
        let mut s = arena();
        s.wolves.clear();
        s.ocelots.clear();
        s.chickens.clear();
        s.player.x = -7.0;
        s.world.set_block(1, 19, BlockType::Stone);
        let mut ocelot = Ocelot::new(0.4, 20.0);
        ocelot.grounded = true;
        s.ocelots.push(ocelot);
        let mut chicken = Chicken::new(3.5, 19.0);
        chicken.hit_timer = 80;
        s.chickens.push(chicken);
        let mut jumped = false;
        for _ in 0..24 {
            s.update(0, 0);
            jumped |= s.ocelots[0].y < 19.9;
        }
        assert!(s.wolves.is_empty());
        assert!(jumped, "ocelot never attempted the obstacle jump");
        assert!(s.ocelots[0].x.is_finite() && s.ocelots[0].y.is_finite());
    }
    #[test]
    fn hurt_impulse_survives_complete_ai_tick() {
        let mut s = arena();
        s.zombies.push(Zombie::new(1.5, 20.0));
        s.zombies[0].grounded = true;
        assert!(s.perform_melee(1.5, 19.1));
        let x = s.zombies[0].x;
        s.update(0, 0);
        assert!(s.zombies[0].x > x + 0.25, "AI cancelled the hit impulse");
    }
    #[test]
    fn cactus_sides_and_top_damage_and_flash_without_penetrating_the_block() {
        for (x, y) in [(0.75, 20.0), (2.25, 20.0), (1.5, 19.0)] {
            let mut s = arena();
            s.respawn_grace_ticks = 0;
            s.player.x = x;
            s.player.y = y;
            s.world.set_block(1, 20, BlockType::Sand);
            s.world.set_block(1, 19, BlockType::Cactus);
            s.update(0, 0);
            assert!(s.player.health < 20.0, "contact at {x},{y}");
            assert!(s.player_hurt_ticks() > 5);
            let health = s.player.health;
            for _ in 0..5 {
                s.update(0, 0);
            }
            assert_eq!(
                s.player.health, health,
                "contact damage must have a cooldown"
            );
        }
        let mut s = arena();
        s.respawn_grace_ticks = 0;
        s.player.x = 0.65;
        s.world.set_block(1, 20, BlockType::Sand);
        s.world.set_block(1, 19, BlockType::Cactus);
        s.update(0, 0);
        assert_eq!(s.player.health, 20.0, "damage across an air gap");
    }
    #[test]
    fn crossing_behind_a_committed_attack_dodges_it() {
        let mut s = arena();
        s.zombies.push(Zombie::new(1.4, 20.0));
        s.zombies[0].grounded = true;
        s.update(0, 0);
        assert!(s.zombies[0].attack_windup > 0);
        assert!(!s.zombies[0].facing_right);
        s.player.x = 2.3;
        for _ in 0..8 {
            s.update(0, 0);
        }
        assert_eq!(s.player.health, 20.0);
    }
    #[test]
    fn swept_arrow_hits_nearest_body_before_wall_and_knockback_follows_flight() {
        let mut s = arena();
        s.zombies.push(Zombie::new(2.0, 20.0));
        s.skeletons.push(Skeleton::new(4.0, 20.0));
        s.player.x = 7.0; // Shooter crossed the target while the arrow was in flight.
        assert!(s.hit_player_arrow_segment((0.0, 19.1), (5.0, 19.1), 5.0, 0.6));
        assert_eq!(s.zombies[0].health, 15.0);
        assert_eq!(s.skeletons[0].health, 20.0);
        assert!(s.zombies[0].vx > 0.0);
        assert!(!s.hit_player_arrow_segment((0.0, 19.1), (5.0, 19.1), 5.0, 0.1));
    }
    #[test]
    fn skeleton_initial_shot_warns_locks_aim_and_is_interruptible() {
        let mut s = Skeleton::new(0.5, 20.0);
        assert!(!s.update_ai(5.5, 20.0));
        assert_eq!(s.bow_windup, 12);
        for _ in 0..11 {
            assert!(!s.update_ai(6.0, 20.0));
        }
        assert!(s.update_ai(6.0, 20.0));
        assert_eq!(s.aim_x, 5.5);
        s.bow_windup = 8;
        s.hit_timer = 4;
        assert!(!s.update_ai(5.5, 20.0));
        assert_eq!(s.bow_windup, 0);
    }
}
