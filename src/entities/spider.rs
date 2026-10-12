#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Spider {
    pub director_event: u64,
    pub x: f64,
    pub y: f64,
    pub vx: f64,
    pub vy: f64,
    pub grounded: bool,
    pub facing_right: bool,
    pub age: u64,
    pub health: f32,
    pub hit_timer: u8,
    pub last_player_damage_tick: u64,
    pub attack_cooldown: u8,
    pub attack_windup: u8,
    pub stuck_ticks: u8,
    pub reroute_ticks: u8,
    pub reroute_dir: i8,
}

impl Spider {
    pub fn new(x: f64, y: f64) -> Self {
        Self {
            director_event: 0,
            x,
            y,
            vx: 0.0,
            vy: 0.0,
            grounded: false,
            facing_right: true,
            age: 0,
            health: 16.0,
            hit_timer: 0,
            last_player_damage_tick: 0,
            attack_cooldown: 0,
            attack_windup: 0,
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

    pub fn update_ai(&mut self, player_x: f64, player_y: f64, is_day: bool) {
        if self.hit_timer > 0 || self.attack_windup > 0 {
            self.attack_cooldown = self.attack_cooldown.saturating_sub(1);
            if self.hit_timer > 0 {
                self.attack_windup = 0;
            } else {
                self.vx = 0.0;
            }
            return;
        }

        if self.attack_cooldown > 0 {
            self.attack_cooldown -= 1;
        }

        let dx = player_x - self.x;
        let dy = player_y - self.y;
        let dist = (dx * dx + dy * dy).sqrt();

        // Spiders are neutral in daylight unless provoked, but for simplicity,
        // we'll make them aggro within 12 blocks at night, and 3 blocks during day
        let aggro_dist = if is_day { 3.0 } else { 12.0 };

        if dist < aggro_dist {
            self.facing_right = dx > 0.0;
            let speed = 0.4;
            if self.facing_right {
                self.vx = speed;
            } else {
                self.vx = -speed;
            }
            if dist < 2.0 && self.grounded {
                // Pounce
                self.vy = -0.4;
                self.grounded = false;
            }
        } else if self.age % 80 < 20 {
            self.vx = if self.facing_right { 0.2 } else { -0.2 };
        } else {
            self.vx = 0.0;
            if self.age % 80 == 20 {
                self.facing_right = !self.facing_right;
            }
        }
    }
}
