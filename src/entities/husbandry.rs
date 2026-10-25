//! Persistent farm-animal lifecycle; timers advance only in simulated terrain.
use serde::{Deserialize, Serialize};

pub const LOVE_TICKS: u16 = 600;
pub const BREED_COOLDOWN_TICKS: u16 = 6000;
pub const CHILD_TICKS: u16 = 12000;
pub const COURTSHIP_TICKS: u16 = 60;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Husbandry {
    pub persistent: bool,
    pub love_ticks: u16,
    pub cooldown_ticks: u16,
    pub child_ticks: u16,
    pub courtship_ticks: u16,
}
impl Husbandry {
    pub fn baby() -> Self {
        Self {
            persistent: true,
            child_ticks: CHILD_TICKS,
            ..Self::default()
        }
    }
    pub fn scale(self) -> f64 {
        if self.child_ticks > 0 { 0.6 } else { 1.0 }
    }
    pub fn ready(self) -> bool {
        self.child_ticks == 0 && self.cooldown_ticks == 0 && self.love_ticks > 0
    }
    pub fn feed(&mut self) -> bool {
        if self.child_ticks > 0 || self.cooldown_ticks > 0 || self.love_ticks > 0 {
            return false;
        }
        self.persistent = true;
        self.love_ticks = LOVE_TICKS;
        true
    }
    pub fn tick(&mut self) {
        self.child_ticks = self.child_ticks.saturating_sub(1);
        self.cooldown_ticks = self.cooldown_ticks.saturating_sub(1);
        self.love_ticks = self.love_ticks.saturating_sub(1);
        if !self.ready() {
            self.courtship_ticks = 0;
        }
    }
    pub fn after_birth(&mut self) {
        self.love_ticks = 0;
        self.courtship_ticks = 0;
        self.cooldown_ticks = BREED_COOLDOWN_TICKS;
    }
    pub fn valid(self) -> bool {
        self.love_ticks <= LOVE_TICKS
            && self.cooldown_ticks <= BREED_COOLDOWN_TICKS
            && self.child_ticks <= CHILD_TICKS
            && self.courtship_ticks <= COURTSHIP_TICKS
            && (self.love_ticks == 0 || (self.child_ticks == 0 && self.cooldown_ticks == 0))
    }
}
