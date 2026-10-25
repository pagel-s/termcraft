//! Multiplayer sleep votes are transient; beds still persist each player's spawn.
use super::*;

impl GameState {
    fn dawn(&mut self) {
        self.time_of_day = 4000.0;
        self.weather = WeatherType::Clear;
        self.weather_rain_intensity = self.weather_rain_intensity.min(0.15);
        self.weather_thunder_intensity = 0.0;
        self.thunder_flash_timer = 0;
        self.weather_timer = self.weather_timer.max(1800);
    }
    pub(super) fn use_bed(&mut self, x: i32, y: i32) {
        let night = !(6000.0..18000.0).contains(&self.time_of_day);
        let Some(id) = self.active_peer else {
            if night && self.active_game_rules().do_daylight_cycle {
                self.dawn();
            }
            return;
        };
        let notice = if !night {
            "Respawn point set. Sleep is available at night."
        } else if !self.active_game_rules().do_daylight_cycle {
            "Respawn point set. The daylight cycle is disabled."
        } else {
            if self
                .sleeping_beds
                .values()
                .any(|&(bx, by, _)| (bx, by) == (x, y))
            {
                self.player_notices
                    .push((id, "This bed is occupied. Use another bed.".into()));
                return;
            }
            self.clear_container_interaction_state();
            self.clear_live_input_for_container_open();
            self.player.vx = 0.0;
            self.player.vy = 0.0;
            self.sleeping_beds.insert(id, (x, y, self.player.health));
            ""
        };
        if !notice.is_empty() {
            self.player_notices.push((id, notice.into()));
        }
    }
    pub(super) fn wake_for_command(&mut self, command: ClientCommand) {
        if matches!(
            command,
            ClientCommand::SetMoveLeft(true)
                | ClientCommand::SetMoveRight(true)
                | ClientCommand::QueueJump
                | ClientCommand::SetJumpHeld(true)
                | ClientCommand::SetPrimaryAction(true)
                | ClientCommand::UseAt(_, _)
                | ClientCommand::ToggleInventory
                | ClientCommand::TravelToOverworld
                | ClientCommand::TravelToNether
                | ClientCommand::TravelToEnd
                | ClientCommand::TravelToSpawn
        ) && let Some(id) = self.active_peer
            && self.sleeping_beds.remove(&id).is_some()
        {
            self.player_notices.push((id, "You left the bed.".into()));
        }
    }
    pub(crate) fn poll_sleep(&mut self) {
        if self.sleeping_beds.is_empty() {
            return;
        }
        let selected = self.active_peer;
        let sleepers: Vec<_> = self
            .sleeping_beds
            .iter()
            .map(|(&id, &bed)| (id, bed))
            .collect();
        for (id, (x, y, health)) in sleepers {
            let valid = self.peer_dimension(id) == Dimension::Overworld
                && self.activate_peer(id)
                && !self.death_screen_active
                && self.player.health >= health
                && self.world.get_block(x, y) == BlockType::Bed
                && (self.player.x - x as f64 - 0.5).abs() <= 4.8
                && (self.player.y - y as f64 - 0.5).abs() <= 4.8;
            if !valid {
                self.sleeping_beds.remove(&id);
                self.player_notices.push((id, "Sleep interrupted.".into()));
            }
        }
        let eligible: Vec<_> = self
            .peer_ids()
            .into_iter()
            .filter(|&id| {
                self.peer_dimension(id) == Dimension::Overworld
                    && self
                        .peer_session(id)
                        .is_some_and(|p| !p.dead() && p.player().health > 0.0)
            })
            .collect();
        if !eligible.is_empty()
            && let Some(&id) = eligible.first()
        {
            self.activate_peer(id);
            if (6000.0..18000.0).contains(&self.time_of_day)
                || !self.active_game_rules().do_daylight_cycle
            {
                self.sleeping_beds.clear();
            } else if eligible
                .iter()
                .all(|id| self.sleeping_beds.contains_key(id))
            {
                self.dawn();
                self.sleeping_beds.clear();
                for &id in &eligible {
                    self.player_notices
                        .push((id, "Good morning! Respawn point set.".into()));
                }
            } else if self.world_tick.is_multiple_of(20) || !self.player_notices.is_empty() {
                let text = format!(
                    "Sleeping {}/{}. All living Overworld players need a bed. Move or jump to wake.",
                    self.sleeping_beds.len(),
                    eligible.len()
                );
                for &id in &eligible {
                    self.player_notices.push((id, text.clone()));
                }
            }
        }
        if let Some(id) = selected {
            self.activate_peer(id);
        }
    }
}
