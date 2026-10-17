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

    pub fn update_ai(&mut self, player_x: f64, player_y: f64, active_crystal_count: usize) {
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
                // The waypoint must orbit slower than the dragon can fly.
                // The previous 196-tick orbit left it chasing a moving target
                // six times too fast, oscillating near the arena center.
                let orbit_angle = self.age as f64 * 0.005;
                let orbit_radius = 22.0 + (self.age as f64 * 0.011).sin() * 5.0;
                let target_x = 0.5 + orbit_radius * orbit_angle.cos();
                let target_y = 18.0 + 4.0 * orbit_angle.sin();
                self.steer_towards(target_x, target_y, 0.18, 0.08);

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
                let dive_speed = if active_crystal_count > 0 { 0.26 } else { 0.30 };
                self.steer_towards(player_x, target_y, dive_speed, 0.16);
                let dist = ((player_x - self.x).powi(2) + (target_y - self.y).powi(2)).sqrt();
                if dist < 0.7 || self.phase_timer > 300 {
                    self.phase = DragonPhase::Patrol;
                    self.phase_timer = 0;
                    self.swoop_cooldown = if active_crystal_count > 0 { 70 } else { 44 };
                }
            }
        }

        self.vx = self.vx.clamp(-0.32, 0.32);
        self.vy = self.vy.clamp(-0.24, 0.24);
        self.facing_right = self.vx >= 0.0;
    }

    /// Keep the head, horns and body out of terrain, with wing room on patrol.
    /// Look ahead and climb before a tower; never bounce back into the AI goal.
    /// This also works with the larger towers already stored in older saves.
    pub(crate) fn advance_flight(&mut self, world: &World, player_x: f64) {
        // Frontal wings project toward the viewer during a dive; their span
        // cannot turn every gap between pillars into an impassable 8-block wall.
        let half_span = if self.phase == DragonPhase::Patrol {
            4.0
        } else {
            1.5
        };
        let direction = if self.vx >= 0.0 { 1.0 } else { -1.0 };
        let mut look_x = self.x + direction * (3.0 + self.vx.abs() * 16.0);
        if self.phase == DragonPhase::Swoop {
            // Do not climb for a tower beyond the player: leave a real attack
            // lane between the columns instead of hovering above every dive.
            look_x = if direction > 0.0 {
                look_x.min(player_x)
            } else {
                look_x.max(player_x)
            };
        }
        let left = (self.x.min(look_x) - half_span).floor() as i32;
        let right = (self.x.max(look_x) + half_span).floor() as i32;
        let search_bottom = (self.y + 5.0).ceil() as i32;
        let mut safe_feet = 52.0_f64;
        for x in left..=right {
            if let Some(top) = (1..=search_bottom).find(|&y| world.get_block(x, y).is_solid()) {
                safe_feet = safe_feet.min(top as f64 - 0.65);
            }
        }
        let climb = self.y + self.vy - safe_feet;
        if climb > 0.0 {
            self.vy = (self.vy - 0.045).clamp(-0.24, -0.06);
            self.vx *= (1.0 - climb * 0.09).clamp(0.15, 0.95);
        }
        let next_y = (self.y + self.vy).clamp(5.5, 52.0);
        let next_x = (self.x + self.vx).clamp(-72.0, 72.0);
        if self.flight_space_clear(world, next_x, next_y) {
            self.x = next_x;
            self.y = next_y;
        } else {
            // Vertical escape also frees a dragon loaded inside an old pillar.
            // No terrain is removed and no saved geometry is regenerated.
            self.vx *= 0.6;
            self.vy = -0.24;
            let escape_y = (self.y + self.vy).max(5.5);
            if !self.flight_space_clear(world, self.x, self.y)
                || self.flight_space_clear(world, self.x, escape_y)
            {
                self.y = escape_y;
            } else {
                // A player-built ceiling must not become a tunnel through rock.
                self.vy = 0.0;
                self.vx = -direction * 0.08;
                if self.flight_space_clear(world, self.x + self.vx, self.y) {
                    self.x += self.vx;
                }
            }
        }
        self.facing_right = self.vx >= 0.0;
    }

    pub(crate) fn flight_space_clear(&self, world: &World, x: f64, y: f64) -> bool {
        for bx in (x - 1.5).floor() as i32..=(x + 1.5 - 0.001).floor() as i32 {
            for by in (y - 4.7).floor() as i32..=(y - 0.001).floor() as i32 {
                if world.get_block(bx, by).is_solid() {
                    return false;
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
        let mut world = World::network_replica(Dimension::End);
        for cx in -3..=3 {
            world
                .apply_chunk_column_snapshot(cx, &vec![BlockType::Air; CHUNK_WIDTH * CHUNK_HEIGHT]);
        }
        for x in -80..=80 {
            world.set_block(x, 34, BlockType::EndStone);
        }
        for (x, top) in [(-28, 20), (-14, 23), (14, 21), (28, 18)] {
            for bx in x - 3..=x + 3 {
                world.set_block(bx, top, BlockType::Obsidian);
                if (bx - x).abs() <= 2 {
                    for by in top..31 {
                        world.set_block(bx, by, BlockType::Obsidian);
                    }
                }
            }
        }
        world
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
                dragon.update_ai(player_x, 34.0, 4);
                dragon.advance_flight(&world, player_x);
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
            dragon.update_ai(6.5, 34.0, 4);
            dragon.advance_flight(&world, 6.5);
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
