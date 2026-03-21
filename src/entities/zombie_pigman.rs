pub struct ZombiePigman {
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
    pub attack_cooldown: u8,
    pub stuck_ticks: u8,
    pub reroute_ticks: u8,
    pub reroute_dir: i8,
}

impl ZombiePigman {
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
            health: 20.0,
            hit_timer: 0,
            last_player_damage_tick: 0,
            aggressive_timer: 0,
            attack_cooldown: 0,
            stuck_ticks: 0,
            reroute_ticks: 0,
            reroute_dir: 0,
        }
    }

    pub fn jump(&mut self) {
        if self.grounded && self.jump_cooldown == 0 {
            self.vy = -0.55;
            self.grounded = false;
            self.jump_cooldown = 10;
        }
    }

    pub fn provoke(&mut self) {
        self.aggressive_timer = self.aggressive_timer.max(260);
    }

    pub fn is_aggressive(&self) -> bool {
        self.aggressive_timer > 0
    }

    pub fn update_ai(&mut self, player_x: f64, player_y: f64) {
        if self.jump_cooldown > 0 {
            self.jump_cooldown -= 1;
        }
        if self.attack_cooldown > 0 {
            self.attack_cooldown -= 1;
        }

        if self.aggressive_timer > 0 {
            self.aggressive_timer -= 1;
            let dx = player_x - self.x;
            let dy = player_y - self.y;
            let dist = (dx * dx + dy * dy).sqrt();
            if dist < 22.0 {
                let speed = 0.17;
                if dx > 0.5 {
                    self.vx = speed;
                    self.facing_right = true;
                } else if dx < -0.5 {
                    self.vx = -speed;
                    self.facing_right = false;
                } else {
                    self.vx = 0.0;
                }
            } else {
                self.vx = 0.0;
            }
        } else if self.age % 120 < 16 {
            self.vx = if self.facing_right { 0.08 } else { -0.08 };
        } else {
            self.vx = 0.0;
            if self.age % 120 == 16 {
                self.facing_right = !self.facing_right;
            }
        }
    }
}
