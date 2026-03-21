pub struct Creeper {
    pub x: f64,
    pub y: f64,
    pub vx: f64,
    pub vy: f64,
    pub grounded: bool,
    pub facing_right: bool,
    pub age: u64,
    pub health: f32,
    pub hit_timer: u8,
    pub last_player_damage_tick: u64,
    pub fuse_timer: u8,
    pub charged: bool,
    pub stuck_ticks: u8,
    pub reroute_ticks: u8,
    pub reroute_dir: i8,
}

impl Creeper {
    pub const CHASE_SPEED: f64 = 0.19;
    pub const WANDER_SPEED: f64 = 0.06;

    pub fn new(x: f64, y: f64) -> Self {
        Self {
            x,
            y,
            vx: 0.0,
            vy: 0.0,
            grounded: false,
            facing_right: true,
            age: 0,
            health: 20.0,
            hit_timer: 0,
            last_player_damage_tick: 0,
            fuse_timer: 0,
            charged: false,
            stuck_ticks: 0,
            reroute_ticks: 0,
            reroute_dir: 0,
        }
    }

    pub fn jump(&mut self) {
        if self.grounded {
            self.vy = -0.5;
            self.grounded = false;
        }
    }

    pub fn update_ai(&mut self, player_x: f64, player_y: f64) {
        let dx = player_x - self.x;
        let dy = player_y - self.y;
        let dist = (dx * dx + dy * dy).sqrt();

        if dist < 12.0 {
            if dist < 2.0 {
                self.vx = 0.0;
                if self.fuse_timer < 30 {
                    self.fuse_timer += 1;
                }
            } else {
                if self.fuse_timer > 0 {
                    self.fuse_timer -= 1;
                }
                self.facing_right = dx > 0.0;
                let speed = Self::CHASE_SPEED;
                if self.facing_right {
                    self.vx = speed;
                } else {
                    self.vx = -speed;
                }
            }
        } else {
            if self.fuse_timer > 0 {
                self.fuse_timer -= 1;
            }
            if self.age % 100 < 20 {
                self.vx = if self.facing_right {
                    Self::WANDER_SPEED
                } else {
                    -Self::WANDER_SPEED
                };
            } else {
                self.vx = 0.0;
                if self.age % 100 == 20 {
                    self.facing_right = !self.facing_right;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Creeper;

    #[test]
    fn creeper_chase_speed_matches_tuned_pacing() {
        let mut creeper = Creeper::new(0.0, 10.0);

        creeper.update_ai(6.0, 10.0);

        assert!((creeper.vx - Creeper::CHASE_SPEED).abs() < f64::EPSILON);
    }

    #[test]
    fn creeper_wander_speed_stays_slower_than_chase() {
        let mut creeper = Creeper::new(0.0, 10.0);
        creeper.age = 5;

        creeper.update_ai(32.0, 10.0);

        assert!((creeper.vx - Creeper::WANDER_SPEED).abs() < f64::EPSILON);
        assert!(creeper.vx.abs() < Creeper::CHASE_SPEED);
    }
}
