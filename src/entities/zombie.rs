pub struct Zombie {
    pub x: f64,
    pub y: f64,
    pub vx: f64,
    pub vy: f64,
    pub grounded: bool,
    pub facing_right: bool,
    pub jump_cooldown: u8,
    pub age: u64,
    pub health: f32,
    pub burning_timer: i32,
    pub hit_timer: u8,
    pub last_player_damage_tick: u64,
    pub attack_cooldown: u8,
    pub stuck_ticks: u8,
    pub reroute_ticks: u8,
    pub reroute_dir: i8,
}

impl Zombie {
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
            burning_timer: 0,
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
            self.vy = -0.55; // Slightly higher jump than player to climb easier
            self.grounded = false;
            self.jump_cooldown = 10; // Prevent spam jumping
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

        // Detection radius: 20 blocks
        if dist < 20.0 {
            let speed = 0.15;
            if dx > 0.5 {
                self.walk(speed);
            } else if dx < -0.5 {
                self.walk(-speed);
            } else {
                self.vx = 0.0;
            }
        } else {
            self.vx = 0.0;
        }
    }
}
