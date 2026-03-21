pub struct Villager {
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
    pub home_x: i32,
    pub home_y: i32,
}

impl Villager {
    pub fn new(x: f64, y: f64, home_x: i32, home_y: i32) -> Self {
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
            health: 20.0,
            hit_timer: 0,
            last_player_damage_tick: 0,
            home_x,
            home_y,
        }
    }

    pub fn set_home(&mut self, home_x: i32, home_y: i32) {
        self.home_x = home_x;
        self.home_y = home_y;
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

    pub fn update_ai(&mut self, is_day: bool) {
        use rand::Rng;

        if self.jump_cooldown > 0 {
            self.jump_cooldown -= 1;
        }

        let mut rng = rand::thread_rng();
        let home_target_x = self.home_x as f64 + 0.5;
        let home_dx = home_target_x - self.x;

        if is_day {
            if home_dx.abs() > 10.0 {
                self.walk(home_dx.signum() * 0.12);
                self.wander_timer = 16;
            } else if self.wander_timer > 0 {
                self.wander_timer -= 1;
                let home_bias = (home_dx * 0.035).clamp(-0.03, 0.03);
                let base = if self.facing_right { 0.1 } else { -0.1 };
                self.walk(base + home_bias);
            } else {
                let roll = rng.gen_range(0..100);
                if roll < 58 {
                    self.wander_timer = rng.gen_range(16..82);
                    self.facing_right = rng.gen_bool(0.5);
                    let base = if self.facing_right { 0.1 } else { -0.1 };
                    self.walk(base);
                } else {
                    self.vx = 0.0;
                }
            }
        } else if home_dx.abs() > 0.9 {
            self.wander_timer = 0;
            self.walk(home_dx.signum() * 0.12);
        } else {
            self.wander_timer = 0;
            self.vx = 0.0;
        }
    }
}
