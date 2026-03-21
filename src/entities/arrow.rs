pub struct Arrow {
    pub x: f64,
    pub y: f64,
    pub vx: f64,
    pub vy: f64,
    pub age: u64,
    pub dead: bool,
    pub from_player: bool,
    pub damage: f32,
}

impl Arrow {
    pub fn new(x: f64, y: f64, vx: f64, vy: f64) -> Self {
        Self::new_hostile(x, y, vx, vy)
    }

    pub fn new_hostile(x: f64, y: f64, vx: f64, vy: f64) -> Self {
        Self {
            x,
            y,
            vx,
            vy,
            age: 0,
            dead: false,
            from_player: false,
            damage: 1.0,
        }
    }

    pub fn new_player(x: f64, y: f64, vx: f64, vy: f64, damage: f32) -> Self {
        Self {
            x,
            y,
            vx,
            vy,
            age: 0,
            dead: false,
            from_player: true,
            damage,
        }
    }

    pub fn update(&mut self) {
        self.vy += 0.05; // Gravity
        self.x += self.vx;
        self.y += self.vy;
        self.age += 1;
        if self.age > 200 {
            self.dead = true;
        } // Despawn after 10s
    }
}
