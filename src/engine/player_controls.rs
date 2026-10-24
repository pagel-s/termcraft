use super::*;

impl GameState {
    // Debug travel belongs to a player session, never to the shared world. A
    // fresh launch/reconnect always starts with the shortcuts disabled.
    pub fn shortcuts_enabled(&self) -> bool {
        self.shortcut_setting().unwrap_or(false)
    }
    pub fn shortcuts_unlocked(&self) -> bool {
        self.shortcut_setting().is_some()
    }
    fn shortcut_setting(&self) -> Option<bool> {
        self.active_peer.map_or(self.solo_shortcuts, |id| {
            self.shortcut_players.get(&id).copied()
        })
    }
    pub(crate) fn install_shortcuts(&mut self, value: Option<bool>) {
        self.solo_shortcuts = value;
    }
    pub(super) fn toggle_shortcuts(&mut self) {
        if let Some(id) = self.active_peer {
            let enabled = self.shortcut_players.entry(id).or_insert(false);
            *enabled = !*enabled;
        } else {
            self.solo_shortcuts = Some(!self.solo_shortcuts.unwrap_or(false));
        }
    }
    pub(crate) fn private_player_command(&mut self, text: &str) -> Option<String> {
        let mut words = text.split_whitespace();
        if !words.next()?.eq_ignore_ascii_case("/gamemode") {
            return None;
        }
        let option = words.next();
        if words.next().is_some() || option.is_some_and(|v| !matches!(v, "on" | "off")) {
            return Some("Use /gamemode, /gamemode on or /gamemode off.".into());
        }
        let desired = option
            .map(|v| v == "on")
            .unwrap_or(!self.shortcuts_enabled());
        if !self.shortcuts_unlocked() || desired != self.shortcuts_enabled() {
            self.toggle_shortcuts();
        }
        if let Some(id) = self.active_peer {
            self.shortcut_players.insert(id, desired);
        } else {
            self.solo_shortcuts = Some(desired);
        }
        Some(
            if desired {
                "Your travel/loadout shortcuts are ON (F5–F9). You can change them in O: settings."
            } else {
                "Your travel/loadout shortcuts are OFF. You can change them in O: settings."
            }
            .into(),
        )
    }
    pub(crate) fn can_change_world_settings(&self) -> bool {
        !self.world.is_replica()
            || (self.local_player_id.is_some() && self.local_player_id == self.host_client_id)
    }
    pub(crate) fn settings_rows(&self) -> Vec<(String, Option<ClientCommand>)> {
        let mut rows = Vec::new();
        let on_off = |v| if v { "ON" } else { "OFF" };
        if self.can_change_world_settings() {
            let (spawn, day, weather, keep) = self.game_rule_flags();
            rows.extend([
                (
                    format!("Difficulty        : {}", self.difficulty_name()),
                    Some(ClientCommand::CycleDifficulty),
                ),
                (
                    format!("Gamerule preset   : {}", self.game_rules_preset_name()),
                    Some(ClientCommand::CycleGameRulesPreset),
                ),
                (
                    format!("doMobSpawning     : {}", on_off(spawn)),
                    Some(ClientCommand::ToggleRuleMobSpawning),
                ),
                (
                    format!("doDaylightCycle   : {}", on_off(day)),
                    Some(ClientCommand::ToggleRuleDaylightCycle),
                ),
                (
                    format!("doWeatherCycle    : {}", on_off(weather)),
                    Some(ClientCommand::ToggleRuleWeatherCycle),
                ),
                (
                    format!("keepInventory     : {}", on_off(keep)),
                    Some(ClientCommand::ToggleRuleKeepInventory),
                ),
            ]);
            if self.world.is_replica() || self.cooperative() {
                rows.push((
                    format!("PvP combat        : {}", on_off(self.pvp_enabled)),
                    Some(ClientCommand::TogglePvp),
                ));
            }
        }
        if self.shortcuts_unlocked() {
            rows.push((
                format!("Travel/loadout    : {}", on_off(self.shortcuts_enabled())),
                Some(ClientCommand::ToggleShortcuts),
            ));
        }
        rows.push(("Close settings menu".into(), None));
        rows
    }
    pub(crate) fn settings_selected_command(&self) -> Option<ClientCommand> {
        self.settings_rows()
            .get(self.settings_menu_selected as usize)
            .and_then(|(_, command)| *command)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> GameState {
        let mut s = GameState::network_view();
        for cx in -1..=1 {
            s.world
                .apply_chunk_column_snapshot(cx, &vec![BlockType::Air; CHUNK_WIDTH * CHUNK_HEIGHT]);
        }
        for x in -32..=63 {
            s.world.set_block(x, 20, BlockType::Grass);
        }
        s.join_peer(1);
        s.player.x = 0.5;
        s.player.y = 20.0;
        s.join_peer(2);
        s.activate_peer(2);
        s.player.x = 2.0;
        s.player.y = 20.0;
        s.activate_peer(1);
        s
    }
    #[test]
    fn shortcut_unlock_and_settings_are_personal_and_start_disabled() {
        let mut s = fixture();
        assert!(!s.shortcuts_enabled() && !s.shortcuts_unlocked());
        s.private_player_command("/gamemode").unwrap();
        assert!(s.shortcuts_enabled());
        s.activate_peer(2);
        assert!(!s.shortcuts_enabled() && !s.shortcuts_unlocked());
        s.apply_client_command(ClientCommand::ToggleShortcuts);
        assert!(!s.shortcuts_enabled());
        s.private_player_command("/gamemode off").unwrap();
        assert!(s.shortcuts_unlocked() && !s.shortcuts_enabled());
        s.apply_client_command(ClientCommand::ToggleShortcuts);
        assert!(s.shortcuts_enabled());
        s.activate_peer(1);
        assert!(s.shortcuts_enabled());
    }
    #[test]
    fn pvp_melee_respects_walls_cooldown_armor_and_disabled_rule() {
        let mut s = fixture();
        assert!(s.perform_melee(2.0, 19.2));
        let health = s.peer_session(2).unwrap().player().health;
        assert!(health < 20.0);
        s.perform_melee(2.0, 19.2);
        assert_eq!(s.peer_session(2).unwrap().player().health, health);
        s.activate_peer(2);
        s.player_combat_hurt_cooldown = 0;
        s.apply_client_command(ClientCommand::EquipDiamondLoadout);
        s.activate_peer(1);
        s.perform_melee(2.0, 19.2);
        let armored_health = s.peer_session(2).unwrap().player().health;
        assert!(health - armored_health < 20.0 - health);
        s.activate_peer(2);
        s.player_combat_hurt_cooldown = 0;
        s.activate_peer(1);
        s.world.set_block(1, 19, BlockType::Stone);
        s.world.set_block(1, 18, BlockType::Stone);
        assert!(!s.perform_melee(2.0, 19.2));
        s.world.set_block(1, 19, BlockType::Air);
        s.world.set_block(1, 18, BlockType::Air);
        s.pvp_enabled = false;
        assert!(!s.perform_melee(2.0, 19.2));
        assert_eq!(s.peer_session(2).unwrap().player().health, armored_health);
    }
    #[test]
    fn owned_arrows_hit_other_players_and_never_the_shooter_or_through_terrain() {
        let mut s = fixture();
        s.activate_peer(2);
        assert!(!s.hit_owned_player_arrow_segment((0.5, 19.2), (3.0, 19.2), 8.0, 0.3, Some(1)));
        assert!(s.hit_owned_player_arrow_segment(
            (0.5, 19.2),
            (3.0, 19.2),
            8.0,
            f64::INFINITY,
            Some(1)
        ));
        assert_eq!(s.peer_session(1).unwrap().player().health, 20.0);
        assert!(s.peer_session(2).unwrap().player().health < 20.0);
        s.pvp_enabled = false;
        assert!(!s.hit_owned_player_arrow_segment(
            (0.5, 19.2),
            (3.0, 19.2),
            8.0,
            f64::INFINITY,
            Some(1)
        ));
    }
}
