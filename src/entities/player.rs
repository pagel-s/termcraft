pub struct Player {
    pub x: f64,
    pub y: f64,
    pub vx: f64,
    pub vy: f64,
    pub grounded: bool,
    pub facing_right: bool,
    pub mining_timer: f32,
    pub last_mine_x: i32,
    pub last_mine_y: i32,
    pub health: f32,
    pub max_health: f32,
    pub hunger: f32,
    pub max_hunger: f32,
    pub sneaking: bool,
    pub age: u64,
    pub attack_timer: u32,
    pub fall_distance: f32,
    pub drowning_timer: i32,
    pub burning_timer: i32,
    pub experience_level: u32,
    pub experience_progress: f32,
    pub experience_total: u32,
}

impl Player {
    pub fn new(x: f64, y: f64) -> Self {
        Self {
            x,
            y,
            vx: 0.0,
            vy: 0.0,
            grounded: false,
            facing_right: true,
            mining_timer: 0.0,
            last_mine_x: 0,
            last_mine_y: 0,
            health: 20.0,
            max_health: 20.0,
            hunger: 20.0,
            max_hunger: 20.0,
            sneaking: false,
            age: 0,
            attack_timer: 0,
            fall_distance: 0.0,
            drowning_timer: 300,
            burning_timer: 0,
            experience_level: 0,
            experience_progress: 0.0,
            experience_total: 0,
        }
    }

    pub fn jump(&mut self, water_submersion: f64, lava_submersion: f64) {
        if water_submersion > 0.0 {
            let lift = if water_submersion >= 0.82 {
                0.24
            } else if water_submersion >= 0.42 {
                0.17
            } else {
                0.10
            };
            self.vy = self.vy.min(-lift);
            if water_submersion < 0.68 {
                self.vx *= 0.68;
            } else {
                self.vx *= 0.88;
            }
            self.grounded = false;
            if self.hunger > 0.0 {
                self.hunger -= 0.04;
            }
        } else if lava_submersion > 0.0 {
            let lift = if lava_submersion >= 0.5 { 0.2 } else { 0.14 };
            self.vy = self.vy.min(-lift);
            self.vx *= 0.88;
            self.grounded = false;
            if self.hunger > 0.0 {
                self.hunger -= 0.03;
            }
        } else if self.grounded {
            self.vy = -0.5; // Exactly enough to clear slightly more than 1 block
            self.grounded = false;
            if self.hunger > 0.0 {
                self.hunger -= 0.2; // Jumping costs hunger
            }
        }
    }

    pub fn swim_up(&mut self, water_submersion: f64, lava_submersion: f64) {
        if water_submersion > 0.0 {
            let (rise, ceiling, horizontal_drag) = if water_submersion >= 0.82 {
                (0.04, -0.25, 0.985)
            } else if water_submersion >= 0.42 {
                (0.03, -0.19, 0.96)
            } else {
                (0.018, -0.12, 0.92)
            };
            self.vy = (self.vy - rise).max(ceiling).min(-rise);
            self.vx *= horizontal_drag;
            self.grounded = false;
        } else if lava_submersion > 0.0 {
            let (rise, ceiling) = if lava_submersion >= 0.5 {
                (0.03, -0.18)
            } else {
                (0.02, -0.14)
            };
            self.vy = (self.vy - rise).max(ceiling).min(-rise);
            self.vx *= 0.93;
            self.grounded = false;
        }
    }

    pub fn swim_down(&mut self, water_submersion: f64, lava_submersion: f64) {
        if water_submersion > 0.0 {
            let (fall, floor, horizontal_drag) = if water_submersion >= 0.82 {
                (0.03, 0.18, 0.985)
            } else if water_submersion >= 0.42 {
                (0.022, 0.14, 0.96)
            } else {
                (0.014, 0.1, 0.93)
            };
            self.vy = (self.vy + fall).min(floor).max(fall);
            self.vx *= horizontal_drag;
            self.grounded = false;
        } else if lava_submersion > 0.0 {
            let (fall, floor) = if lava_submersion >= 0.5 {
                (0.018, 0.12)
            } else {
                (0.012, 0.09)
            };
            self.vy = (self.vy + fall).min(floor).max(fall);
            self.vx *= 0.92;
            self.grounded = false;
        }
    }
}
