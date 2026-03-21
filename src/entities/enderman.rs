pub struct Enderman {
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
    pub aggressive_timer: u16,
    pub teleport_cooldown: u8,
    pub attack_cooldown: u8,
    pub stuck_ticks: u8,
    pub reroute_ticks: u8,
    pub reroute_dir: i8,
}

impl Enderman {
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
            health: 40.0,
            hit_timer: 0,
            last_player_damage_tick: 0,
            aggressive_timer: 0,
            teleport_cooldown: 0,
            attack_cooldown: 0,
            stuck_ticks: 0,
            reroute_ticks: 0,
            reroute_dir: 0,
        }
    }

    pub fn provoke(&mut self) {
        self.aggressive_timer = self.aggressive_timer.max(260);
    }

    pub fn jump(&mut self) {
        if self.grounded && self.jump_cooldown == 0 {
            self.vy = -0.62;
            self.grounded = false;
            self.jump_cooldown = 12;
        }
    }

    pub fn update_ai(&mut self, player_x: f64, player_y: f64, in_end: bool) -> bool {
        if self.jump_cooldown > 0 {
            self.jump_cooldown -= 1;
        }
        if self.teleport_cooldown > 0 {
            self.teleport_cooldown -= 1;
        }
        if self.attack_cooldown > 0 {
            self.attack_cooldown -= 1;
        }

        let dx = player_x - self.x;
        let dy = player_y - self.y;
        let dist = (dx * dx + dy * dy).sqrt();

        let sees_player = dist < if in_end { 14.0 } else { 10.0 };
        if sees_player {
            self.aggressive_timer = self.aggressive_timer.max(if in_end { 150 } else { 90 });
        } else if self.aggressive_timer > 0 {
            self.aggressive_timer -= 1;
        }

        if self.aggressive_timer > 0 {
            let speed = if in_end { 0.22 } else { 0.19 };
            if dx > 0.6 {
                self.vx = speed;
                self.facing_right = true;
            } else if dx < -0.6 {
                self.vx = -speed;
                self.facing_right = false;
            } else {
                self.vx = 0.0;
            }
        } else if self.age % 140 < 24 {
            self.vx = if self.facing_right { 0.09 } else { -0.09 };
        } else {
            self.vx = 0.0;
            if self.age % 140 == 24 {
                self.facing_right = !self.facing_right;
            }
        }

        if self.grounded && dy < -1.8 && self.jump_cooldown == 0 {
            self.jump();
        }

        self.aggressive_timer > 0
            && self.teleport_cooldown == 0
            && (dist > 8.0 && self.age.is_multiple_of(45))
    }
}

#[cfg(test)]
mod tests {
    use super::Enderman;

    #[test]
    fn test_enderman_becomes_aggressive_when_player_near() {
        let mut e = Enderman::new(0.0, 10.0);
        let teleport_ready = e.update_ai(3.0, 10.0, false);
        assert!(e.aggressive_timer > 0);
        assert!(!teleport_ready);
    }
}
