pub struct Cow {
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
}

impl Cow {
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
            health: 10.0,
            hit_timer: 0,
            last_player_damage_tick: 0,
        }
    }

    pub fn jump(&mut self) {
        if self.grounded && self.jump_cooldown == 0 {
            self.vy = -0.55;
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

    pub fn update_ai(&mut self) {
        use rand::Rng;
        let mut rng = rand::thread_rng();

        if self.jump_cooldown > 0 {
            self.jump_cooldown -= 1;
        }

        if self.wander_timer > 0 {
            self.wander_timer -= 1;
            // Continue walking in current direction
            if self.facing_right {
                self.walk(0.1);
            } else {
                self.walk(-0.1);
            }
        } else {
            // Pick a new action
            let roll = rng.gen_range(0..100);
            if roll < 5 {
                // Start walking
                self.wander_timer = rng.gen_range(20..100);
                self.facing_right = rng.gen_bool(0.5);
            } else {
                self.vx = 0.0;
            }
        }
    }
}
