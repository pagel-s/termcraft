pub struct Ghast {
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
}

impl Ghast {
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
        }
    }

    pub fn update_ai(&mut self, player_x: f64, player_y: f64) -> bool {
        let dx = player_x - self.x;
        let dy = (player_y - 2.5) - self.y;
        let dist = (dx * dx + dy * dy).sqrt();
        let mut shoot = false;

        if self.shoot_cooldown > 0 {
            self.shoot_cooldown -= 1;
        }

        if dist < 22.0 {
            self.facing_right = dx > 0.0;

            // Keep pressure on the player, but float at a standoff distance.
            let target_vx = if dist > 11.0 {
                dx.signum() * 0.16
            } else if dist < 7.0 {
                -dx.signum() * 0.12
            } else {
                0.0
            };
            let target_vy = dy.clamp(-6.0, 6.0) * 0.03;

            self.vx += (target_vx - self.vx) * 0.18;
            self.vy += (target_vy - self.vy) * 0.18;

            if dist < 16.0 && self.shoot_cooldown == 0 {
                shoot = true;
                self.shoot_cooldown = 75;
            }
        } else {
            // Slow idle drift when player is far away.
            let wave = (self.age as f64 * 0.05).sin();
            self.vx += ((if self.facing_right { 0.07 } else { -0.07 }) - self.vx) * 0.08;
            self.vy += (wave * 0.05 - self.vy) * 0.08;
            if self.age.is_multiple_of(160) {
                self.facing_right = !self.facing_right;
            }
        }

        self.vx = self.vx.clamp(-0.22, 0.22);
        self.vy = self.vy.clamp(-0.16, 0.16);
        shoot
    }
}

#[cfg(test)]
mod tests {
    use super::Ghast;

    #[test]
    fn test_ghast_shoots_when_in_range_and_ready() {
        let mut ghast = Ghast::new(0.0, 30.0);
        let did_shoot = ghast.update_ai(8.0, 30.0);
        assert!(did_shoot);
        assert!(ghast.shoot_cooldown > 0);
    }

    #[test]
    fn test_ghast_shoot_cooldown_blocks_immediate_second_shot() {
        let mut ghast = Ghast::new(0.0, 30.0);
        assert!(ghast.update_ai(8.0, 30.0));
        assert!(!ghast.update_ai(8.0, 30.0));
    }
}
