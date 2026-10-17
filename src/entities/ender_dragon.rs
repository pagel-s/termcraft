use super::end_crystal::EndCrystal;
use crate::world::World;

#[derive(Clone, Copy, PartialEq, Eq, Debug, serde::Serialize, serde::Deserialize)]
pub enum DragonPhase {
    Patrol,
    Swoop,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct EnderDragon {
    pub x: f64,
    pub y: f64,
    pub vx: f64,
    pub vy: f64,
    pub facing_right: bool,
    pub age: u64,
    pub health: f32,
    pub max_health: f32,
    pub hit_timer: u8,
    pub attack_cooldown: u8,
    pub phase: DragonPhase,
    pub phase_timer: u16,
    pub swoop_cooldown: u16,
}

impl EnderDragon {
    /// Regeneration does not stack with crystal count: show one readable tether
    /// to the nearest active crystal instead of a fan across the whole arena.
    pub fn healing_crystal<'a>(&self, crystals: &'a [EndCrystal]) -> Option<&'a EndCrystal> {
        let distance = |c: &EndCrystal| (c.x - self.x).powi(2) + (c.y - self.y).powi(2);
        crystals
            .iter()
            .filter(|c| c.health > 0.0)
            .min_by(|a, b| distance(a).total_cmp(&distance(b)))
    }

    /// Head, armored torso and haunches in the frontal eight-pixels-per-block art.
    /// Decorative wings/tail do not give the boss invisible extra attack reach.
    pub fn hit_regions(&self) -> [[f64; 4]; 3] {
        [
            [self.x, self.y - 2.9, 0.9, 1.0],
            [self.x, self.y - 1.5, 0.8, 0.85],
            [self.x, self.y - 0.65, 1.15, 0.4],
        ]
    }

    pub fn hit_test(&self, x: f64, y: f64) -> bool {
        self.hit_regions()
            .iter()
            .any(|r| ((x - r[0]) / r[2]).powi(2) + ((y - r[1]) / r[3]).powi(2) <= 1.0)
    }

    pub fn new(x: f64, y: f64) -> Self {
        Self {
            x,
            y,
            vx: 0.0,
            vy: 0.0,
            facing_right: true,
            age: 0,
            health: 200.0,
            max_health: 200.0,
            hit_timer: 0,
            attack_cooldown: 0,
            phase: DragonPhase::Patrol,
            phase_timer: 0,
            swoop_cooldown: 30,
        }
    }

    fn steer_towards(&mut self, target_x: f64, target_y: f64, speed: f64, turn_rate: f64) {
        let dx = target_x - self.x;
        let dy = target_y - self.y;
        let dist = (dx * dx + dy * dy).sqrt().max(0.001);
        let tvx = (dx / dist) * speed;
        let tvy = (dy / dist) * speed;
        self.vx += (tvx - self.vx) * turn_rate;
        self.vy += (tvy - self.vy) * turn_rate;
    }

    fn update_phase(&mut self, player_x: f64, player_y: f64, active_crystal_count: usize) {
        self.age += 1;
        self.phase_timer = self.phase_timer.saturating_add(1);
        if self.hit_timer > 0 {
            self.hit_timer -= 1;
        }
        if self.attack_cooldown > 0 {
            self.attack_cooldown -= 1;
        }
        if self.swoop_cooldown > 0 {
            self.swoop_cooldown -= 1;
        }

        match self.phase {
            DragonPhase::Patrol => {
                let horizontal_player_gap = (player_x - self.x).abs();
                if self.swoop_cooldown == 0
                    && (self.phase_timer > 240
                        || (self.phase_timer > 140 && horizontal_player_gap < 10.0))
                {
                    self.phase = DragonPhase::Swoop;
                    self.phase_timer = 0;
                }
            }
            DragonPhase::Swoop => {
                let target_y = (player_y - 0.35).clamp(8.0, 44.0);
                let dist = ((player_x - self.x).powi(2) + (target_y - self.y).powi(2)).sqrt();
                if dist < 0.7 || self.phase_timer > 300 {
                    self.phase = DragonPhase::Patrol;
                    self.phase_timer = 0;
                    self.swoop_cooldown = if active_crystal_count > 0 { 70 } else { 44 };
                }
            }
        }
    }

    fn flight_goal(&self, player_x: f64, player_y: f64, crystals: usize) -> (f64, f64, f64, f64) {
        match self.phase {
            DragonPhase::Patrol => {
                let angle = self.age as f64 * 0.005;
                let radius = 22.0 + (self.age as f64 * 0.011).sin() * 5.0;
                (
                    0.5 + radius * angle.cos(),
                    18.0 + 4.0 * angle.sin(),
                    0.18,
                    0.18,
                )
            }
            DragonPhase::Swoop => (
                player_x,
                (player_y - 0.35).clamp(8.0, 44.0),
                if crystals > 0 { 0.26 } else { 0.30 },
                0.22,
            ),
        }
    }

    pub fn update_ai(&mut self, player_x: f64, player_y: f64, active_crystal_count: usize) {
        self.update_phase(player_x, player_y, active_crystal_count);
        let (x, y, speed, turn) = self.flight_goal(player_x, player_y, active_crystal_count);
        self.steer_towards(x, y, speed, turn);
    }

    /// Route across the complete obstacle before descending. A covered player
    /// cancels the dive rather than pinning the boss above an impossible target.
    pub(crate) fn update_flight(
        &mut self,
        world: &World,
        player_x: f64,
        player_y: f64,
        crystals: usize,
    ) {
        self.update_phase(player_x, player_y, crystals);
        if !self.flight_space_clear(world, self.x, self.y) {
            // Recover an embedded saved/explosion-displaced body without editing
            // terrain or teleporting it. Normal flight never enters this branch.
            self.vx = 0.0;
            self.vy = -0.24;
            self.y = (self.y + self.vy).max(5.5);
            return;
        }
        let (mut goal_x, mut goal_y, mut speed, mut turn) =
            self.flight_goal(player_x, player_y, crystals);
        if self.phase == DragonPhase::Swoop {
            let side = if player_x >= self.x { -1.0 } else { 1.0 };
            let open = [0.0, side * 0.6, -side * 0.6, side * 1.2, -side * 1.2]
                .into_iter()
                .map(|dx| goal_x + dx)
                .find(|&x| {
                    (-72.0..=72.0).contains(&x)
                        && Self::flight_corridor_clear(world, x, self.y.min(goal_y), x, goal_y)
                });
            match open {
                Some(x) if (x - self.x).hypot(goal_y - self.y) >= 0.7 => goal_x = x,
                _ => {
                    self.phase = DragonPhase::Patrol;
                    self.phase_timer = 0;
                    self.swoop_cooldown = if crystals > 0 { 70 } else { 44 };
                    (goal_x, goal_y, speed, turn) = self.flight_goal(player_x, player_y, crystals);
                }
            }
        }
        let direction = if goal_x >= self.x { 1.0 } else { -1.0 };
        let left = (self.x.min(goal_x) - 1.55).floor() as i32;
        let right = (self.x.max(goal_x) + 1.55).floor() as i32;
        let bottom = (self.y.max(goal_y) - 0.001).floor() as i32;
        let mut route_y = goal_y;
        let mut approach = f64::MAX;
        for x in left..=right {
            if let Some(top) = (1..=bottom).find(|&y| world.get_block(x, y).is_solid()) {
                let ceiling = top as f64 - 0.75;
                route_y = route_y.min(ceiling);
                if self.y > ceiling {
                    let gap = if direction > 0.0 {
                        x as f64 - self.x - 1.5
                    } else {
                        self.x - 1.5 - f64::from(x + 1)
                    };
                    approach = approach.min(gap);
                }
            }
        }
        route_y = route_y.clamp(5.5, 52.0);
        let climb = (self.y - route_y).max(0.0);
        let mut target_x = goal_x;
        let mut target_y = route_y;
        let mut retreating = false;
        if climb > 0.1 && !Self::flight_corridor_clear(world, self.x, self.y, self.x, route_y) {
            // Under a crown lip: back out to a clear launch column first. Once
            // outside, the approach speed cap keeps us there until fully above
            // the crown, so steering cannot alternate into and out of its face.
            'launch: for step in 1..=24 {
                for side in [-direction, direction] {
                    let x = self.x + side * f64::from(step) * 0.5;
                    if (-72.0..=72.0).contains(&x)
                        && Self::flight_corridor_clear(world, self.x, self.y, x, self.y)
                        && Self::flight_corridor_clear(world, x, self.y, x, route_y)
                    {
                        target_x = x;
                        target_y = self.y;
                        retreating = true;
                        break 'launch;
                    }
                }
            }
        }
        let mut desired_vx = ((target_x - self.x) * 0.22).clamp(-speed, speed);
        if climb > 0.1 && !retreating {
            let ascent_ticks = climb / 0.20 + 6.0;
            let limit = ((approach - 0.75).max(0.0) / ascent_ticks).min(speed);
            desired_vx = desired_vx.clamp(-limit, limit);
        }
        let desired_vy = ((target_y - self.y) * 0.20).clamp(-0.24, 0.24);
        self.vx += (desired_vx - self.vx) * turn;
        self.vy += (desired_vy - self.vy) * turn;
        self.vx = self.vx.clamp(-0.32, 0.32);
        self.vy = self.vy.clamp(-0.24, 0.24);
        self.move_flight_axis(world, self.vy, false);
        self.move_flight_axis(world, self.vx, true);
        self.facing_right = self.vx >= 0.0;
    }

    fn move_flight_axis(&mut self, world: &World, delta: f64, horizontal: bool) {
        let (x, y) = if horizontal {
            ((self.x + delta).clamp(-72.0, 72.0), self.y)
        } else {
            (self.x, (self.y + delta).clamp(5.5, 52.0))
        };
        if Self::flight_corridor_clear(world, self.x, self.y, x, y) {
            self.x = x;
            self.y = y;
        } else {
            let (mut low, mut high) = (0.0, 1.0);
            for _ in 0..10 {
                let t = (low + high) * 0.5;
                if Self::flight_corridor_clear(
                    world,
                    self.x,
                    self.y,
                    self.x + (x - self.x) * t,
                    self.y + (y - self.y) * t,
                ) {
                    low = t;
                } else {
                    high = t;
                }
            }
            self.x += (x - self.x) * low;
            self.y += (y - self.y) * low;
            if horizontal {
                self.vx = 0.0;
            } else {
                self.vy = 0.0;
            }
        }
    }

    pub(crate) fn flight_space_clear(&self, world: &World, x: f64, y: f64) -> bool {
        Self::flight_corridor_clear(world, x, y, x, y)
    }

    fn flight_corridor_clear(world: &World, x0: f64, y0: f64, x1: f64, y1: f64) -> bool {
        let (left, right, top, bottom) = (
            x0.min(x1) - 1.5,
            x0.max(x1) + 1.5,
            y0.min(y1) - 4.7,
            y0.max(y1),
        );
        for bx in left.floor() as i32..=(right - 0.00001).floor() as i32 {
            for by in top.floor() as i32..=(bottom - 0.00001).floor() as i32 {
                for b in world.collision_boxes_at(bx, by) {
                    if right > bx as f64 + b[0] + 0.00001
                        && left < bx as f64 + b[2] - 0.00001
                        && bottom > by as f64 + b[1] + 0.00001
                        && top < by as f64 + b[3] - 0.00001
                    {
                        return false;
                    }
                }
            }
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::{DragonPhase, EnderDragon};
    use crate::world::{
        Dimension, World,
        block::BlockType,
        chunk::{CHUNK_HEIGHT, CHUNK_WIDTH},
    };

    #[test]
    fn healing_tether_tracks_the_nearest_surviving_crystal() {
        use super::EndCrystal;
        let dragon = EnderDragon::new(0.5, 18.0);
        let mut crystals = vec![EndCrystal::new(-14.0, 24.0), EndCrystal::new(6.0, 22.0)];
        assert_eq!(dragon.healing_crystal(&crystals).unwrap().x, 6.0);
        crystals[1].health = 0.0;
        assert_eq!(dragon.healing_crystal(&crystals).unwrap().x, -14.0);
        crystals[0].health = 0.0;
        assert!(dragon.healing_crystal(&crystals).is_none());
    }

    fn old_arena() -> World {
        arena_with_towers([(-28, 20), (-14, 23), (14, 21), (28, 18)], 2, 3)
    }

    fn arena_with_towers(towers: [(i32, i32); 4], shaft: i32, crown: i32) -> World {
        let mut world = World::network_replica(Dimension::End);
        for cx in -3..=3 {
            world
                .apply_chunk_column_snapshot(cx, &vec![BlockType::Air; CHUNK_WIDTH * CHUNK_HEIGHT]);
        }
        for x in -80..=80 {
            world.set_block(x, 34, BlockType::EndStone);
        }
        for (x, top) in towers {
            for bx in x - crown..=x + crown {
                world.set_block(bx, top, BlockType::Obsidian);
                if (bx - x).abs() <= shaft {
                    for by in top..31 {
                        world.set_block(bx, by, BlockType::Obsidian);
                    }
                }
            }
        }
        world
    }

    #[test]
    fn crosses_every_pillar_from_both_sides_even_starting_under_a_crown_lip() {
        for (tops, shaft, crown) in [
            ([(-28, 20), (-14, 23), (14, 21), (28, 18)], 2, 3),
            ([(-28, 23), (-14, 25), (14, 24), (28, 22)], 1, 2),
        ] {
            let world = arena_with_towers(tops, shaft, crown);
            for (tower, top) in tops {
                for direction in [-1.0, 1.0] {
                    let center = tower as f64 + 0.5;
                    let player_x = center + direction * 7.5;
                    let mut dragon = EnderDragon::new(
                        center - direction * (shaft as f64 + 2.05),
                        top as f64 + 6.5,
                    );
                    dragon.phase = DragonPhase::Swoop;
                    assert!(
                        dragon.flight_space_clear(&world, dragon.x, dragon.y),
                        "invalid lip fixture at {tower} toward {direction}"
                    );
                    let (mut crossed, mut hold, mut max_hold) = (None, 0, 0);
                    for tick in 0..240 {
                        let x = dragon.x;
                        dragon.update_flight(&world, player_x, 34.0, 4);
                        assert!(
                            dragon.flight_space_clear(&world, dragon.x, dragon.y),
                            "clipped {tower} at tick {tick}"
                        );
                        hold = if (dragon.x - x).abs() < 0.005 {
                            hold + 1
                        } else {
                            0
                        };
                        max_hold = max_hold.max(hold);
                        if crossed.is_none() && (dragon.x - center) * direction > crown as f64 + 2.1
                        {
                            crossed = Some(tick);
                        }
                    }
                    assert!(
                        crossed.is_some_and(|tick| tick < 190),
                        "failed to cross tower {tower} toward {direction}: {crossed:?}, {},{}",
                        dragon.x,
                        dragon.y
                    );
                    assert!(
                        max_hold < 70,
                        "parked for {max_hold} ticks at tower {tower}, side {direction}"
                    );
                }
            }
        }
    }

    #[test]
    fn sheltered_attack_point_under_a_crown_lip_also_cancels_the_dive() {
        let world = old_arena();
        let mut dragon = EnderDragon::new(24.0, 18.0);
        dragon.phase = DragonPhase::Swoop;
        dragon.update_flight(&world, 17.35, 34.0, 4);
        assert_eq!(dragon.phase, DragonPhase::Patrol);
        assert!(dragon.flight_space_clear(&world, dragon.x, dragon.y));
    }

    #[test]
    fn reaching_an_offset_attack_point_returns_to_patrol() {
        let mut world = arena_with_towers([(-60, 10), (-50, 10), (50, 10), (60, 10)], 1, 2);
        for y in 32..34 {
            world.set_block(10, y, BlockType::Stone);
        }
        let mut dragon = EnderDragon::new(12.55, 33.4);
        dragon.phase = DragonPhase::Swoop;
        dragon.update_flight(&world, 11.35, 34.0, 4);
        assert_eq!(dragon.phase, DragonPhase::Patrol);
        assert!(dragon.flight_space_clear(&world, dragon.x, dragon.y));
    }

    #[test]
    fn player_outside_the_arena_does_not_pin_flight_at_its_boundary() {
        let world = old_arena();
        let mut dragon = EnderDragon::new(0.5, 18.0);
        dragon.phase = DragonPhase::Swoop;
        dragon.update_flight(&world, 100_000.0, 34.0, 4);
        assert_eq!(dragon.phase, DragonPhase::Patrol);
        assert!(dragon.flight_space_clear(&world, dragon.x, dragon.y));
    }

    #[test]
    fn player_under_a_pillar_does_not_pin_the_dragon_above_its_crown() {
        let world = old_arena();
        let mut dragon = EnderDragon::new(8.0, 22.0);
        dragon.phase = DragonPhase::Swoop;
        let mut swoop_ticks = 0;
        for _ in 0..160 {
            dragon.update_flight(&world, 14.5, 34.0, 4);
            if dragon.phase == DragonPhase::Swoop {
                swoop_ticks += 1;
            }
        }
        assert!(
            swoop_ticks < 25,
            "kept diving at a covered player for {swoop_ticks} ticks"
        );
        assert!(
            (dragon.x - 14.5).abs() > 4.0,
            "still parked over the pillar: {},{}",
            dragon.x,
            dragon.y
        );
    }

    #[test]
    fn legacy_towers_do_not_hide_or_trap_flying_dragon_and_swoops_reach_player() {
        let world = old_arena();
        for (player_x, start_x) in [(6.5, 0.5), (-6.5, -38.0), (36.5, 0.5)] {
            let mut dragon = EnderDragon::new(start_x, 17.0);
            let mut nearest = f64::MAX;
            let mut attack_reaches = 0;
            let mut stalled = 0;
            let mut max_stalled = 0;
            let (mut min_x, mut max_x) = (start_x, start_x);
            for tick in 0..3600 {
                let before = (dragon.x, dragon.y);
                dragon.update_flight(&world, player_x, 34.0, 4);
                assert!(
                    dragon.flight_space_clear(&world, dragon.x, dragon.y),
                    "terrain overlap at {tick}: {},{}",
                    dragon.x,
                    dragon.y
                );
                let moved = (dragon.x - before.0).hypot(dragon.y - before.1);
                stalled = if moved < 0.015 { stalled + 1 } else { 0 };
                max_stalled = max_stalled.max(stalled);
                min_x = min_x.min(dragon.x);
                max_x = max_x.max(dragon.x);
                nearest = nearest.min((dragon.x - player_x).hypot(dragon.y - 33.0));
                if dragon.hit_regions().iter().any(|r| {
                    ((player_x - r[0]) / (r[2] + 0.3)).powi(2)
                        + ((33.1 - r[1]) / (r[3] + 0.9)).powi(2)
                        <= 1.0
                }) {
                    attack_reaches += 1;
                }
            }
            assert!(max_stalled < 50, "hovered for {max_stalled} ticks");
            assert!(max_x - min_x > 15.0, "flight confined to {min_x},{max_x}");
            assert!(
                nearest < 3.0,
                "swoop never reached player at {player_x}: {nearest}"
            );
            assert!(
                attack_reaches > 0,
                "no body contact on any swoop at {player_x}"
            );
        }
    }

    #[test]
    fn dragon_loaded_inside_legacy_tower_climbs_out_without_altering_it() {
        let world = old_arena();
        let before = world.chunk_column_snapshot(0).unwrap();
        let mut dragon = EnderDragon::new(14.5, 27.0);
        for _ in 0..100 {
            dragon.update_flight(&world, 6.5, 34.0, 4);
        }
        assert!(dragon.flight_space_clear(&world, dragon.x, dragon.y));
        assert_eq!(world.chunk_column_snapshot(0).unwrap(), before);
    }

    #[test]
    fn test_dragon_transitions_to_swoop_when_cooldown_ends() {
        let mut dragon = EnderDragon::new(0.5, 18.0);
        dragon.swoop_cooldown = 0;
        dragon.phase_timer = 250;
        dragon.update_ai(2.0, 20.0, 4);
        assert_eq!(dragon.phase, DragonPhase::Swoop);
    }
}
