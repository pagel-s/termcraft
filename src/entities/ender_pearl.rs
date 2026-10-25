//! A thrown pearl is a session-owned projectile, never a loose inventory item.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct EnderPearl {
    pub x: f64,
    pub y: f64,
    pub vx: f64,
    pub vy: f64,
    pub age: u64,
    pub impact_ticks: u8,
    // Connection identities must never be restored from a save or sent as owners
    // to a replica. Only the authoritative running session resolves a landing.
    #[serde(skip)]
    pub owner: Option<u16>,
}

impl EnderPearl {
    pub const SPEED: f64 = 1.35;
    pub const MAX_AGE: u64 = 120;

    pub fn new(x: f64, y: f64, direction: (f64, f64), owner: Option<u16>) -> Self {
        Self {
            x,
            y,
            vx: direction.0 * Self::SPEED,
            vy: direction.1 * Self::SPEED,
            age: 0,
            impact_ticks: 0,
            owner,
        }
    }

    pub fn advance(&mut self, underwater: bool) {
        let drag = if underwater { 0.8 } else { 0.99 };
        self.vx *= drag;
        self.vy = self.vy * drag + 0.035;
        self.x += self.vx;
        self.y += self.vy;
        self.age += 1;
    }
}
