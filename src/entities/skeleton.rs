pub struct Skeleton {
    pub x: f64,
    pub y: f64,
    pub vx: f64,
    pub vy: f64,
    pub grounded: bool,
    pub facing_right: bool,
    pub age: u64,
    pub health: f32,
    pub burning_timer: i32,
    pub hit_timer: u8,
    pub last_player_damage_tick: u64,
    pub bow_cooldown: u8,
    pub stuck_ticks: u8,
    pub reroute_ticks: u8,
    pub reroute_dir: i8,
}

impl Skeleton {
    pub fn new(x: f64, y: f64) -> Self {
        Self {
            x,
            y,
            vx: 0.0,
            vy: 0.0,
            grounded: false,
            facing_right: true,
            age: 0,
            health: 20.0,
            burning_timer: 0,
            hit_timer: 0,
            last_player_damage_tick: 0,
            bow_cooldown: 0,
            stuck_ticks: 0,
            reroute_ticks: 0,
            reroute_dir: 0,
        }
    }

    pub fn jump(&mut self) {
        if self.grounded {
            self.vy = -0.5;
            self.grounded = false;
        }
    }

    pub fn update_ai(&mut self, player_x: f64, player_y: f64) -> bool {
        let dx = player_x - self.x;
        let dy = player_y - self.y;
        let dist = (dx * dx + dy * dy).sqrt();
        let mut shoot = false;

        if dist < 12.0 {
            self.facing_right = dx > 0.0;
            if dist > 6.0 {
                let speed = 0.3;
                if self.facing_right {
                    self.vx = speed;
                } else {
                    self.vx = -speed;
                }
            } else if dist < 4.0 {
                let speed = 0.3;
                if self.facing_right {
                    self.vx = -speed;
                } else {
                    self.vx = speed;
                }
            } else {
                self.vx = 0.0;
            }

            if self.bow_cooldown > 0 {
                self.bow_cooldown -= 1;
            } else if dist < 10.0 {
                shoot = true;
                self.bow_cooldown = 60; // 3 seconds at 20tps
            }
        } else if self.age % 100 < 20 {
            self.vx = if self.facing_right { 0.1 } else { -0.1 };
        } else {
            self.vx = 0.0;
            if self.age % 100 == 20 {
                self.facing_right = !self.facing_right;
            }
        }
        shoot
    }
}
