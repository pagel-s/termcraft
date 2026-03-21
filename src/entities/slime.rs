pub struct Slime {
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
    pub jump_cooldown: u8,
    pub attack_cooldown: u8,
    pub size: u8,
    pub stuck_ticks: u8,
    pub reroute_ticks: u8,
    pub reroute_dir: i8,
}

impl Slime {
    pub fn new(x: f64, y: f64, size: u8) -> Self {
        let normalized_size = if size >= 4 {
            4
        } else if size >= 2 {
            2
        } else {
            1
        };
        Self {
            x,
            y,
            vx: 0.0,
            vy: 0.0,
            grounded: false,
            facing_right: true,
            age: 0,
            health: Self::max_health_for_size(normalized_size),
            hit_timer: 0,
            last_player_damage_tick: 0,
            jump_cooldown: 0,
            attack_cooldown: 0,
            size: normalized_size,
            stuck_ticks: 0,
            reroute_ticks: 0,
            reroute_dir: 0,
        }
    }

    pub fn max_health_for_size(size: u8) -> f32 {
        match size {
            4 => 16.0,
            2 => 4.0,
            _ => 1.0,
        }
    }

    pub fn half_width(&self) -> f64 {
        match self.size {
            4 => 0.7,
            2 => 0.48,
            _ => 0.3,
        }
    }

    pub fn height(&self) -> f64 {
        match self.size {
            4 => 1.6,
            2 => 1.15,
            _ => 0.75,
        }
    }

    pub fn move_speed(&self) -> f64 {
        match self.size {
            4 => 0.22,
            2 => 0.17,
            _ => 0.125,
        }
    }

    pub fn jump_impulse(&self) -> f64 {
        match self.size {
            4 => -0.52,
            2 => -0.46,
            _ => -0.38,
        }
    }

    pub fn jump_interval_ticks(&self) -> u8 {
        match self.size {
            4 => 18,
            2 => 22,
            _ => 27,
        }
    }

    pub fn contact_damage(&self) -> f32 {
        match self.size {
            4 => 4.0,
            2 => 2.0,
            _ => 0.0,
        }
    }

    pub fn split_size(&self) -> Option<u8> {
        match self.size {
            4 => Some(2),
            2 => Some(1),
            _ => None,
        }
    }

    pub fn jump(&mut self) {
        if self.grounded && self.jump_cooldown == 0 {
            self.vy = self.jump_impulse();
            self.grounded = false;
            self.jump_cooldown = self.jump_interval_ticks();
        }
    }

    pub fn update_ai(&mut self, player_x: f64, player_y: f64, is_day: bool) {
        if self.jump_cooldown > 0 {
            self.jump_cooldown -= 1;
        }
        if self.attack_cooldown > 0 {
            self.attack_cooldown -= 1;
        }

        let dx = player_x - self.x;
        let dy = player_y - self.y;
        let dist = (dx * dx + dy * dy).sqrt();
        let aggro_dist = match (self.size, is_day) {
            (4, true) => 9.0,
            (4, false) => 14.5,
            (_, true) => 6.5,
            (_, false) => 11.5,
        };

        if dist < aggro_dist {
            self.facing_right = dx > 0.0;
            if dx.abs() > 0.35 {
                self.vx = self.move_speed() * dx.signum();
            } else {
                self.vx = 0.0;
            }
            if self.grounded && self.jump_cooldown == 0 {
                self.jump();
            }
        } else if self.age % 96 < 36 {
            self.vx = if self.facing_right {
                self.move_speed() * 0.45
            } else {
                -self.move_speed() * 0.45
            };
            if self.grounded && self.jump_cooldown == 0 && self.age.is_multiple_of(10) {
                self.jump();
            }
        } else {
            self.vx *= 0.75;
            if self.vx.abs() < 0.01 {
                self.vx = 0.0;
            }
            if self.age % 96 == 36 {
                self.facing_right = !self.facing_right;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Slime;

    #[test]
    fn large_slime_splits_to_medium() {
        let s = Slime::new(0.0, 12.0, 4);
        assert_eq!(s.split_size(), Some(2));
    }

    #[test]
    fn small_slime_is_non_damaging() {
        let s = Slime::new(0.0, 12.0, 1);
        assert_eq!(s.contact_damage(), 0.0);
    }
}
