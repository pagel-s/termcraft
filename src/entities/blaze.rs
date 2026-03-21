pub struct Blaze {
    pub x: f64,
    pub y: f64,
    pub vx: f64,
    pub vy: f64,
    pub facing_right: bool,
    pub age: u64,
    pub health: f32,
    pub hit_timer: u8,
    pub last_player_damage_tick: u64,
    pub shoot_cooldown: u8,
    pub attack_cooldown: u8,
}

impl Blaze {
    pub fn new(x: f64, y: f64) -> Self {
        Self {
            x,
            y,
            vx: 0.0,
            vy: 0.0,
            facing_right: true,
            age: 0,
            health: 20.0,
            hit_timer: 0,
            last_player_damage_tick: 0,
            shoot_cooldown: 0,
            attack_cooldown: 0,
        }
    }

    pub fn update_ai(&mut self, player_x: f64, player_y: f64) -> bool {
        let dx = player_x - self.x;
        let dy = (player_y - 1.5) - self.y;
        let dist = (dx * dx + dy * dy).sqrt();
        let mut shoot = false;

        if self.shoot_cooldown > 0 {
            self.shoot_cooldown -= 1;
        }
        if self.attack_cooldown > 0 {
            self.attack_cooldown -= 1;
        }

        if dist < 18.0 {
            self.facing_right = dx > 0.0;

            let strafe = (self.age as f64 * 0.14).sin() * 0.05;
            let target_vx = if dist > 8.0 {
                dx.signum() * 0.14 + strafe
            } else if dist < 5.0 {
                -dx.signum() * 0.12 + strafe
            } else {
                strafe
            };
            let target_vy = dy.clamp(-6.0, 6.0) * 0.035 + (self.age as f64 * 0.09).cos() * 0.02;

            self.vx += (target_vx - self.vx) * 0.18;
            self.vy += (target_vy - self.vy) * 0.18;

            if dist < 14.0 && self.shoot_cooldown == 0 {
                shoot = true;
                self.shoot_cooldown = 60;
            }
        } else {
            let wave = (self.age as f64 * 0.06).sin();
            self.vx += ((if self.facing_right { 0.05 } else { -0.05 }) - self.vx) * 0.08;
            self.vy += (wave * 0.04 - self.vy) * 0.08;
            if self.age.is_multiple_of(180) {
                self.facing_right = !self.facing_right;
            }
        }

        self.vx = self.vx.clamp(-0.24, 0.24);
        self.vy = self.vy.clamp(-0.2, 0.2);
        shoot
    }
}

#[cfg(test)]
mod tests {
    use super::Blaze;

    #[test]
    fn test_blaze_shoots_when_in_range_and_ready() {
        let mut blaze = Blaze::new(0.0, 30.0);
        let did_shoot = blaze.update_ai(7.0, 30.0);
        assert!(did_shoot);
        assert!(blaze.shoot_cooldown > 0);
    }
}
