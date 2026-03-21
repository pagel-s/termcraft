pub struct Squid {
    pub x: f64,
    pub y: f64,
    pub vx: f64,
    pub vy: f64,
    pub grounded: bool,
    pub facing_right: bool,
    pub wander_timer: u16,
    pub flop_cooldown: u8,
    pub swim_dir_x: f64,
    pub swim_dir_y: f64,
    pub age: u64,
    pub health: f32,
    pub hit_timer: u8,
    pub last_player_damage_tick: u64,
}

impl Squid {
    pub fn new(x: f64, y: f64) -> Self {
        Self {
            x,
            y,
            vx: 0.0,
            vy: 0.0,
            grounded: false,
            facing_right: true,
            wander_timer: 0,
            flop_cooldown: 0,
            swim_dir_x: 0.0,
            swim_dir_y: 0.0,
            age: 0,
            health: 10.0,
            hit_timer: 0,
            last_player_damage_tick: 0,
        }
    }

    pub fn update_ai(&mut self, in_water: bool) {
        use rand::Rng;

        let mut rng = rand::thread_rng();
        if self.flop_cooldown > 0 {
            self.flop_cooldown -= 1;
        }

        if self.wander_timer > 0 {
            self.wander_timer -= 1;
        } else if in_water {
            self.wander_timer = rng.gen_range(20..96);
            self.swim_dir_x = rng.gen_range(-0.11..=0.11);
            self.swim_dir_y = rng.gen_range(-0.09..=0.09);
        } else {
            self.wander_timer = rng.gen_range(10..38);
            self.swim_dir_x = if rng.gen_bool(0.5) { 0.09 } else { -0.09 };
            self.swim_dir_y = 0.0;
        }

        if in_water {
            self.vx = (self.vx + self.swim_dir_x * 0.22).clamp(-0.18, 0.18);
            self.vy = (self.vy + self.swim_dir_y * 0.2).clamp(-0.14, 0.14);
        } else {
            self.vx *= 0.88;
            if self.grounded && self.flop_cooldown == 0 {
                self.vy = -0.32;
                self.vx += if rng.gen_bool(0.5) { 0.07 } else { -0.07 };
                self.flop_cooldown = 14;
            }
        }

        if self.vx > 0.02 {
            self.facing_right = true;
        } else if self.vx < -0.02 {
            self.facing_right = false;
        }
    }
}
