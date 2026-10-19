//! Persistent breath/fire state, shared by every creature's environmental rules.
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct HazardState {
    pub air: u16,
    pub drowning: u8,
    pub burning: u16,
    pub fire_tick: u8,
}
impl Default for HazardState {
    fn default() -> Self {
        Self {
            air: 300,
            drowning: 0,
            burning: 0,
            fire_tick: 0,
        }
    }
}
impl HazardState {
    pub fn valid(&self) -> bool {
        self.air <= 300 && self.drowning < 20 && self.burning <= 100 && self.fire_tick < 20
    }

    #[allow(clippy::too_many_arguments)]
    pub fn tick(
        &mut self,
        submerged: bool,
        wet: bool,
        lava: bool,
        fire: bool,
        fireproof: bool,
        breathes_water: bool,
        needs_air: bool,
    ) -> f32 {
        let mut damage = 0.0;
        let suffocating = needs_air
            && if breathes_water {
                !submerged
            } else {
                submerged
            };
        if suffocating {
            if self.air > 0 {
                self.air -= 1;
            } else {
                self.drowning = self.drowning.saturating_add(1);
                if self.drowning >= 20 {
                    damage += 2.0;
                    self.drowning = 0;
                }
            }
        } else {
            self.air = self.air.saturating_add(4).min(300);
            self.drowning = 0;
        }
        if fireproof || (wet && !lava) {
            self.burning = 0;
            self.fire_tick = 0;
        } else {
            if lava || fire {
                self.burning = 100;
            }
            if self.burning > 0 {
                self.burning -= 1;
                self.fire_tick = self.fire_tick.saturating_add(1);
                let interval = if lava { 10 } else { 20 };
                if self.fire_tick >= interval {
                    damage += if lava { 4.0 } else { 1.0 };
                    self.fire_tick = 0;
                }
            }
        }
        damage
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn breath_grace_damage_recovery_and_aquatic_rules() {
        let mut h = HazardState::default();
        for _ in 0..319 {
            assert_eq!(h.tick(true, true, false, false, false, false, true), 0.0);
        }
        assert_eq!(h.tick(true, true, false, false, false, false, true), 2.0);
        assert_eq!(h.tick(false, false, false, false, false, false, true), 0.0);
        assert_eq!(h.air, 4);
        let mut squid = HazardState::default();
        for _ in 0..320 {
            squid.tick(true, true, false, false, false, true, true);
        }
        assert_eq!(squid.air, 300);
        for _ in 0..319 {
            assert_eq!(
                squid.tick(false, false, false, false, false, true, true),
                0.0
            );
        }
        assert_eq!(
            squid.tick(false, false, false, false, false, true, true),
            2.0
        );
    }
    #[test]
    fn lava_burns_despite_rain_water_quenches_and_fireproof_creatures_survive() {
        let mut h = HazardState::default();
        let damage: f32 = (0..20)
            .map(|_| h.tick(false, true, true, false, false, false, true))
            .sum();
        assert_eq!(damage, 8.0);
        assert!(h.burning > 0);
        h.tick(false, true, false, false, false, false, true);
        assert_eq!(h.burning, 0);
        let damage: f32 = (0..40)
            .map(|_| h.tick(false, false, true, true, true, false, true))
            .sum();
        assert_eq!(damage, 0.0);
        assert!(h.valid());
    }
}
