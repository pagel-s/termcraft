pub struct Ocelot {
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
    pub panic_timer: u16,
    pub attack_cooldown: u8,
}

impl Ocelot {
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
            panic_timer: 0,
            attack_cooldown: 0,
        }
    }

    pub fn jump(&mut self) {
        if self.grounded && self.jump_cooldown == 0 {
            self.vy = -0.58;
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

    pub fn spook_from(&mut self, player_x: f64) {
        self.panic_timer = self.panic_timer.max(110);
        self.facing_right = self.x >= player_x;
    }

    pub fn update_ai(&mut self, player_x: f64, player_y: f64, chicken_target: Option<(f64, f64)>) {
        use rand::Rng;

        if self.jump_cooldown > 0 {
            self.jump_cooldown -= 1;
        }
        if self.attack_cooldown > 0 {
            self.attack_cooldown -= 1;
        }

        let player_dx = player_x - self.x;
        let player_dy = player_y - self.y;
        let player_dist = (player_dx * player_dx + player_dy * player_dy).sqrt();
        if player_dist < 4.0 {
            self.spook_from(player_x);
        }

        if self.panic_timer > 0 {
            self.panic_timer -= 1;
            let away = if self.x >= player_x { 1.0 } else { -1.0 };
            self.walk(away * 0.2);
            return;
        }

        if let Some((chicken_x, _chicken_y)) = chicken_target {
            let dx = chicken_x - self.x;
            if dx.abs() > 0.35 {
                self.walk(dx.signum() * 0.16);
            } else {
                self.vx = 0.0;
            }
            self.wander_timer = 0;
            return;
        }

        let mut rng = rand::thread_rng();
        if self.wander_timer > 0 {
            self.wander_timer -= 1;
            let walk_speed = if self.wander_timer < 8 { 0.09 } else { 0.11 };
            if self.facing_right {
                self.walk(walk_speed);
            } else {
                self.walk(-walk_speed);
            }
        } else {
            let roll = rng.gen_range(0..100);
            if roll < 9 {
                self.wander_timer = rng.gen_range(20..88);
                self.facing_right = rng.gen_bool(0.5);
            } else {
                self.vx = 0.0;
            }
        }
    }
}
