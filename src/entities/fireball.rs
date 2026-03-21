pub struct Fireball {
    pub x: f64,
    pub y: f64,
    pub vx: f64,
    pub vy: f64,
    pub age: u64,
    pub dead: bool,
}

impl Fireball {
    pub fn new(x: f64, y: f64, vx: f64, vy: f64) -> Self {
        Self {
            x,
            y,
            vx,
            vy,
            age: 0,
            dead: false,
        }
    }

    pub fn update(&mut self) {
        self.x += self.vx;
        self.y += self.vy;
        self.age += 1;
        if self.age > 120 {
            self.dead = true;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Fireball;

    #[test]
    fn test_fireball_despawns_after_lifetime() {
        let mut fireball = Fireball::new(0.0, 0.0, 0.1, 0.0);
        for _ in 0..121 {
            fireball.update();
        }
        assert!(fireball.dead);
    }
}
