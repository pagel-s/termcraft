pub struct Wolf {
    pub x: f64,
    pub y: f64,
    pub vx: f64,
    pub vy: f64,
    pub grounded: bool,
    pub facing_right: bool,
    pub jump_cooldown: u8,
    pub wander_timer: u16,
    pub age: u64,
    pub health: f32,
    pub hit_timer: u8,
    pub last_player_damage_tick: u64,
    pub aggressive_timer: u16,
    pub attack_cooldown: u8,
}

impl Wolf {
    pub fn new(x: f64, y: f64) -> Self {
        Self {
            x,
            y,
            vx: 0.0,
            vy: 0.0,
            grounded: false,
            facing_right: true,
            jump_cooldown: 0,
            wander_timer: 0,
            age: 0,
            health: 16.0,
            hit_timer: 0,
            last_player_damage_tick: 0,
            aggressive_timer: 0,
            attack_cooldown: 0,
        }
    }

    pub fn jump(&mut self) {
        if self.grounded && self.jump_cooldown == 0 {
            self.vy = -0.56;
            self.grounded = false;
            self.jump_cooldown = 10;
        }
    }

    pub fn walk(&mut self, direction: f64) {
        self.vx = direction;
        if direction > 0.0 {
            self.facing_right = true;
        } else if direction < 0.0 {
            self.facing_right = false;
        }
    }

    pub fn provoke(&mut self) {
        self.aggressive_timer = self.aggressive_timer.max(240);
    }

    pub fn is_aggressive(&self) -> bool {
        self.aggressive_timer > 0
    }

    pub fn update_ai(&mut self, player_x: f64, player_y: f64, sheep_target: Option<(f64, f64)>) {
        use rand::Rng;

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
            if dist < 18.0 {
                if dx > 0.45 {
                    self.walk(0.17);
                } else if dx < -0.45 {
                    self.walk(-0.17);
                } else {
                    self.vx = 0.0;
                }
            } else {
                self.vx = 0.0;
            }
            return;
        }

        if let Some((sheep_x, _sheep_y)) = sheep_target {
            let dx = sheep_x - self.x;
            if dx.abs() > 0.35 {
                self.walk(dx.signum() * 0.14);
            } else {
                self.vx = 0.0;
            }
            self.wander_timer = 0;
            return;
        }

        let mut rng = rand::thread_rng();
        if self.wander_timer > 0 {
            self.wander_timer -= 1;
            if self.facing_right {
                self.walk(0.1);
            } else {
                self.walk(-0.1);
            }
        } else {
            let roll = rng.gen_range(0..100);
            if roll < 8 {
                self.wander_timer = rng.gen_range(24..110);
                self.facing_right = rng.gen_bool(0.5);
            } else {
                self.vx = 0.0;
            }
        }
    }
}
