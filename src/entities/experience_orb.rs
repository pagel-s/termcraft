pub struct ExperienceOrb {
    pub x: f64,
    pub y: f64,
    pub vx: f64,
    pub vy: f64,
    pub value: u32,
    pub grounded: bool,
    pub age: u64,
}

impl ExperienceOrb {
    pub fn new(x: f64, y: f64, value: u32) -> Self {
        Self {
            x,
            y,
            vx: 0.0,
            vy: 0.0,
            value,
            grounded: false,
            age: 0,
        }
    }

    pub fn get_bobbing_offset(&self) -> f64 {
        (self.age as f64 * 0.14).sin() * 0.18
    }
}
