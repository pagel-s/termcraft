pub struct EndCrystal {
    pub x: f64,
    pub y: f64,
    pub health: f32,
    pub hit_timer: u8,
    pub age: u64,
}

impl EndCrystal {
    pub fn new(x: f64, y: f64) -> Self {
        Self {
            x,
            y,
            health: 6.0,
            hit_timer: 0,
            age: 0,
        }
    }

    pub fn update_tick(&mut self) {
        self.age += 1;
        if self.hit_timer > 0 {
            self.hit_timer -= 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::EndCrystal;

    #[test]
    fn test_end_crystal_tick_advances_age() {
        let mut crystal = EndCrystal::new(0.5, 10.0);
        crystal.update_tick();
        assert_eq!(crystal.age, 1);
    }
}
