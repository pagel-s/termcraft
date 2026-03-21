#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum DragonPhase {
    Patrol,
    Swoop,
}

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
                let orbit_angle = self.age as f64 * 0.032;
                let orbit_radius = 22.0 + (self.age as f64 * 0.011).sin() * 5.0;
                let target_x = 0.5 + orbit_radius * orbit_angle.cos();
                let target_y = 18.0 + 4.0 * orbit_angle.sin();
                self.steer_towards(target_x, target_y, 0.12, 0.08);

                let horizontal_player_gap = (player_x - self.x).abs();
                if self.swoop_cooldown == 0
                    && (self.phase_timer > 86 || horizontal_player_gap < 14.0)
                {
                    self.phase = DragonPhase::Swoop;
                    self.phase_timer = 0;
                }
            }
            DragonPhase::Swoop => {
                let target_y = (player_y - 1.2).clamp(8.0, 44.0);
                let dive_speed = if active_crystal_count > 0 { 0.2 } else { 0.24 };
                self.steer_towards(player_x, target_y, dive_speed, 0.16);
                let dist = ((player_x - self.x).powi(2) + (target_y - self.y).powi(2)).sqrt();
                if dist < 2.2 || self.phase_timer > 46 {
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
}

#[cfg(test)]
mod tests {
    use super::{DragonPhase, EnderDragon};

    #[test]
    fn test_dragon_transitions_to_swoop_when_cooldown_ends() {
        let mut dragon = EnderDragon::new(0.5, 18.0);
        dragon.swoop_cooldown = 0;
        dragon.phase_timer = 100;
        dragon.update_ai(2.0, 20.0, 4);
        assert_eq!(dragon.phase, DragonPhase::Swoop);
    }
}
