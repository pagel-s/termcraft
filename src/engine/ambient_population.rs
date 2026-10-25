//! Reuse inactive ambient population slots without raising the hostile cap.
use super::*;

#[derive(Clone, Copy)]
pub(super) enum OverworldHostileSlot {
    Zombie(usize),
    Creeper(usize),
    Skeleton(usize),
    Spider(usize),
    Slime(usize),
}

impl GameState {
    fn overworld_hostile_count(&self) -> usize {
        self.zombies.len()
            + self.creepers.len()
            + self.skeletons.len()
            + self.spiders.len()
            + self.slimes.len()
    }

    pub(super) fn inactive_overworld_hostile(&self) -> Option<OverworldHostileSlot> {
        let players: Vec<_> = std::iter::once(&self.player)
            .chain(self.other_players_here().map(|p| p.player()))
            .filter(|p| p.health > 0.0)
            .collect();
        if players.is_empty() {
            return None;
        }
        let mut best: Option<(OverworldHostileSlot, f64)> = None;
        macro_rules! consider {
            ($field:ident, $variant:ident, $height:expr, $protected:expr) => {
                for (index, mob) in self.$field.iter().enumerate() {
                    // Give fresh spawns time to act, and preserve recent combat
                    // and explicitly managed encounters even behind terrain.
                    if mob.age < 200
                        || mob.hit_timer > 0
                        || (mob.last_player_damage_tick != 0
                            && self.world_tick.saturating_sub(mob.last_player_damage_tick) < 200)
                        || $protected(mob)
                    {
                        continue;
                    }
                    let distance = players
                        .iter()
                        .map(|p| (mob.x - p.x).powi(2) + (mob.y - p.y).powi(2))
                        .fold(f64::INFINITY, f64::min);
                    if distance <= 24.0 * 24.0
                        || players.iter().any(|p| {
                            [0.2, $height(mob) * 0.5, $height(mob) - 0.2]
                                .into_iter()
                                .any(|eye| {
                                    self.has_line_of_sight(mob.x, mob.y - eye, p.x, p.y - 1.5)
                                })
                        })
                    {
                        continue;
                    }
                    if best.is_none_or(|(_, previous)| distance > previous) {
                        best = Some((OverworldHostileSlot::$variant(index), distance));
                    }
                }
            };
        }
        consider!(zombies, Zombie, |_: &Zombie| 1.8, |m: &Zombie| {
            m.director_event != 0 || m.attack_windup > 0
        });
        consider!(creepers, Creeper, |_: &Creeper| 1.8, |m: &Creeper| {
            m.charged || m.fuse_timer > 0
        });
        consider!(skeletons, Skeleton, |_: &Skeleton| 1.8, |m: &Skeleton| {
            m.director_event != 0 || m.bow_windup > 0
        });
        consider!(spiders, Spider, |_: &Spider| 0.9, |m: &Spider| {
            m.director_event != 0 || m.attack_windup > 0
        });
        consider!(slimes, Slime, |m: &Slime| m.height(), |m: &Slime| {
            m.attack_windup > 0
        });
        best.map(|(slot, _)| slot)
    }

    /// Called only after finding a dark, safe spawn site. Retiring an unseen
    /// occupant frees exactly one slot, with no loot/XP or death side effects.
    pub(super) fn reserve_overworld_hostile_slot(&mut self) -> bool {
        let total = self.overworld_hostile_count();
        let cap = self.scaled_hostile_cap(OVERWORLD_HOSTILE_CAP);
        if total < cap {
            return true;
        }
        if total > cap {
            return false;
        }
        let Some(slot) = self.inactive_overworld_hostile() else {
            return false;
        };
        match slot {
            OverworldHostileSlot::Zombie(i) => {
                self.zombies.remove(i);
            }
            OverworldHostileSlot::Creeper(i) => {
                self.creepers.remove(i);
            }
            OverworldHostileSlot::Skeleton(i) => {
                self.skeletons.remove(i);
            }
            OverworldHostileSlot::Spider(i) => {
                self.spiders.remove(i);
            }
            OverworldHostileSlot::Slime(i) => {
                self.slimes.remove(i);
            }
        }
        true
    }
}
