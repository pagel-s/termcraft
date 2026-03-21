pub struct Silverfish {
    pub x: f64,
    pub y: f64,
    pub vx: f64,
    pub vy: f64,
    pub grounded: bool,
    pub facing_right: bool,
    pub jump_cooldown: u8,
    pub age: u64,
    pub health: f32,
    pub hit_timer: u8,
    pub last_player_damage_tick: u64,
    pub attack_cooldown: u8,
    pub stuck_ticks: u8,
    pub reroute_ticks: u8,
    pub reroute_dir: i8,
}

impl Silverfish {
    pub fn new(x: f64, y: f64) -> Self {
        Self {
            x,
            y,
            vx: 0.0,
            vy: 0.0,
            grounded: false,
            facing_right: true,
            jump_cooldown: 0,
            age: 0,
            health: 8.0,
            hit_timer: 0,
            last_player_damage_tick: 0,
            attack_cooldown: 0,
            stuck_ticks: 0,
            reroute_ticks: 0,
            reroute_dir: 0,
        }
    }

    pub fn jump(&mut self) {
        if self.grounded && self.jump_cooldown == 0 {
            self.vy = -0.48;
            self.grounded = false;
            self.jump_cooldown = 10;
        }
    }

    pub fn update_ai(&mut self, player_x: f64, player_y: f64) {
        if self.jump_cooldown > 0 {
            self.jump_cooldown -= 1;
        }
        if self.attack_cooldown > 0 {
            self.attack_cooldown -= 1;
        }

        let dx = player_x - self.x;
        let dy = player_y - self.y;
        let dist = (dx * dx + dy * dy).sqrt();
        if dist < 12.0 {
            let speed = 0.23;
            if dx > 0.4 {
                self.vx = speed;
                self.facing_right = true;
            } else if dx < -0.4 {
                self.vx = -speed;
                self.facing_right = false;
            } else {
                self.vx = 0.0;
            }
            if dy < -0.9 && self.grounded {
                self.jump();
            }
        } else if self.age % 100 < 18 {
            self.vx = if self.facing_right { 0.07 } else { -0.07 };
        } else {
            self.vx = 0.0;
            if self.age % 100 == 18 {
                self.facing_right = !self.facing_right;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Silverfish;

    #[test]
    fn test_silverfish_chases_near_player() {
        let mut s = Silverfish::new(0.0, 10.0);
        s.update_ai(3.0, 10.0);
        assert!(s.vx > 0.0);
    }
}
