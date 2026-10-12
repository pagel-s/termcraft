//! Presentation interpolates resolved simulation positions, never velocity guesses.
use super::GameState;
use std::collections::HashMap;

/// Resolve the next local movement sample in an isolated replica. This gives
/// sub-tick response without displaying movement through a wall or adding a
/// whole simulation tick of interpolation delay to the controlled player.
pub(super) struct PlayerPreview {
    physics: Box<GameState>,
    revisions: HashMap<i32, u64>,
    raw: (f64, f64, u64),
    endpoint: (f64, f64),
    anchor: (f64, f64),
    anchor_alpha: f64,
    last: (f64, f64),
    last_alpha: f64,
}

impl PlayerPreview {
    pub(super) fn new() -> Self {
        Self {
            physics: Box::new(GameState::network_view()),
            revisions: HashMap::new(),
            raw: (f64::NAN, f64::NAN, 0),
            endpoint: (0.0, 0.0),
            anchor: (0.0, 0.0),
            anchor_alpha: 0.0,
            last: (0.0, 0.0),
            last_alpha: 0.0,
        }
    }

    pub(super) fn position(&mut self, state: &GameState, alpha: f64) -> (f64, f64) {
        let current = (state.player.x, state.player.y);
        if state.inventory_open
            || state.is_settings_menu_open()
            || state.is_showing_death_screen()
            || state.is_showing_credits()
        {
            self.raw.0 = f64::NAN;
            return current;
        }
        if state.current_dimension != self.physics.current_dimension {
            self.physics.world = crate::world::World::network_replica(state.current_dimension);
            self.revisions.clear();
            self.raw.0 = f64::NAN;
        }
        let center =
            (state.player.x.floor() as i32).div_euclid(crate::world::chunk::CHUNK_WIDTH as i32);
        self.revisions.retain(|x, _| x.abs_diff(center) <= 1);
        for chunk in center - 1..=center + 1 {
            if let Some(revision) = state.world.chunk_blocks_revision(chunk)
                && self.revisions.get(&chunk) != Some(&revision)
                && let Some((revision, blocks)) = state.world.chunk_column_snapshot(chunk)
            {
                self.physics
                    .world
                    .apply_chunk_column_snapshot(chunk, &blocks);
                self.revisions.insert(chunk, revision);
            }
        }
        let _ = self
            .physics
            .world
            .trim_chunk_cache(&[state.player.x.floor() as i32], 0);
        state.copy_motion_preview_into(&mut self.physics);
        self.physics.predict_motion_tick();
        let endpoint = (self.physics.player.x, self.physics.player.y);
        let raw = (current.0, current.1, state.player.age);
        let alpha = alpha.clamp(0.0, 1.0);
        if self.raw != raw || alpha < self.last_alpha {
            // Ordinary ticks meet the predicted endpoint exactly. A release or
            // server correction can change it: settle a small error through this
            // tick instead of snapping the player AND scrolling the whole world.
            let error = (self.endpoint.0 - current.0, self.endpoint.1 - current.1);
            self.anchor = if self.raw.0.is_finite() && error.0.hypot(error.1) < 0.75 {
                self.physics
                    .resolve_motion_presentation(self.endpoint.0, self.endpoint.1)
            } else {
                current
            };
            self.anchor_alpha = 0.0;
        } else if self.endpoint != endpoint {
            // Input may change between physics ticks. Rebase at the visible
            // position so releasing/reversing does not rewind the sprite.
            self.anchor = self.last;
            self.anchor_alpha = self.last_alpha;
        }
        self.raw = raw;
        self.endpoint = if self.anchor_alpha > 0.0 {
            // Integrate the new input only over the unrendered part of the tick.
            // A full-tick endpoint made releasing a held direction rewind.
            (
                self.anchor.0 + (endpoint.0 - current.0) * (1.0 - self.anchor_alpha),
                self.anchor.1 + (endpoint.1 - current.1) * (1.0 - self.anchor_alpha),
            )
        } else {
            endpoint
        };
        let t =
            ((alpha - self.anchor_alpha) / (1.0 - self.anchor_alpha).max(0.0001)).clamp(0.0, 1.0);
        self.last = (
            self.anchor.0 + (self.endpoint.0 - self.anchor.0) * t,
            self.anchor.1 + (self.endpoint.1 - self.anchor.1) * t,
        );
        self.last = self
            .physics
            .resolve_motion_presentation(self.last.0, self.last.1);
        self.last_alpha = alpha;
        self.last
    }
}

#[derive(Default)]
pub(super) struct MotionHistory {
    pub(super) dimension: Option<crate::world::Dimension>,
    positions: HashMap<&'static str, Vec<(f64, f64, u64)>>,
}

impl MotionHistory {
    pub(super) fn capture(&mut self, state: &GameState) {
        self.dimension = Some(state.current_dimension);
        macro_rules! actors {
            ($($field:ident),* $(,)?) => {$(
                let positions = self.positions.entry(stringify!($field)).or_default();
                positions.clear();
                positions.extend(state.$field.iter().map(|p| (p.x, p.y, p.age)));
            )*};
        }
        actors!(
            zombies,
            pigmen,
            ghasts,
            blazes,
            endermen,
            creepers,
            skeletons,
            spiders,
            silverfish,
            slimes,
            fireballs,
            arrows,
            cows,
            sheep,
            pigs,
            chickens,
            squids,
            wolves,
            ocelots,
            villagers,
            item_entities,
            experience_orbs,
            end_crystals
        );
        let player = self.positions.entry("player").or_default();
        player.clear();
        player.push((state.player.x, state.player.y, state.player.age));
        let remotes = self.positions.entry("remote_players").or_default();
        remotes.clear();
        remotes.extend(
            state
                .remote_players
                .iter()
                .map(|p| (p.x, p.y, u64::from(p.client_id))),
        );
        let boats = self.positions.entry("boats").or_default();
        boats.clear();
        boats.extend(state.boats.iter().map(|p| (p.x, p.y, 0)));
        let dragon = self.positions.entry("dragon").or_default();
        dragon.clear();
        dragon.extend(state.ender_dragon.iter().map(|p| (p.x, p.y, p.age)));
    }

    pub(super) fn position(
        &self,
        kind: &'static str,
        index: usize,
        current: (f64, f64),
        age: u64,
        alpha: f64,
    ) -> (f64, f64) {
        let Some(&(x, y, previous_age)) = self.positions.get(kind).and_then(|v| v.get(index))
        else {
            return current;
        };
        // Entity removal, replacement, respawn and teleports must not leave trails.
        let identity_changed = if kind == "remote_players" {
            previous_age != age
        } else {
            age < previous_age || age.saturating_sub(previous_age) > 5
        };
        if identity_changed || (current.0 - x).hypot(current.1 - y) > 3.0 {
            return current;
        }
        let alpha = alpha.clamp(0.0, 1.0);
        (x + (current.0 - x) * alpha, y + (current.1 - y) * alpha)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn releasing_between_ticks_does_not_rewind_and_small_corrections_do_not_snap_camera() {
        use crate::{engine::command::ClientCommand, world::block::BlockType};
        let mut state = GameState::network_view();
        state.world.apply_chunk_column_snapshot(
            0,
            &vec![
                BlockType::Air;
                crate::world::chunk::CHUNK_WIDTH * crate::world::chunk::CHUNK_HEIGHT
            ],
        );
        for x in 0..16 {
            state.world.set_block(x, 20, BlockType::Stone);
        }
        state.player.x = 4.0;
        state.player.y = 20.0;
        state.player.grounded = true;
        state.apply_client_command(ClientCommand::SetMoveRight(true));
        state.player.vx = 0.32;
        let mut preview = PlayerPreview::new();
        preview.position(&state, 0.0);
        let before = preview.position(&state, 0.66);
        state.apply_client_command(ClientCommand::SetMoveRight(false));
        let released = preview.position(&state, 0.9);
        assert!(released.0 >= before.0, "release rewound the visible player");
        assert!(
            released.0 - before.0 < 0.04,
            "release kept moving at held speed"
        );
        state.predict_motion_tick();
        state.player.age += 1;
        let next = preview.position(&state, 0.0);
        assert!(
            (next.0 - released.0).abs() < 0.04,
            "tick boundary snapped the camera"
        );
        for _ in 0..4 {
            preview.position(&state, 0.99);
            state.predict_motion_tick();
            state.player.age += 1;
            preview.position(&state, 0.0);
        }
        assert!((preview.position(&state, 0.99).0 - state.player.x).abs() < 0.02);
    }
    #[test]
    fn local_preview_is_collision_resolved_and_never_mutates_the_game() {
        use crate::{
            engine::command::ClientCommand, engine::state::PlayerSession, world::block::BlockType,
        };
        let mut state = GameState::network_view();
        let blocks = vec![
            BlockType::Air;
            crate::world::chunk::CHUNK_WIDTH * crate::world::chunk::CHUNK_HEIGHT
        ];
        state.world.apply_chunk_column_snapshot(0, &blocks);
        for x in 0..16 {
            state.world.set_block(x, 20, BlockType::Stone);
        }
        for y in 17..20 {
            state.world.set_block(1, y, BlockType::Stone);
        }
        state.player.x = 0.65;
        state.player.y = 20.0;
        state.player.grounded = true;
        state.apply_client_command(ClientCommand::SetMoveRight(true));
        let before = PlayerSession::capture(&state);
        let mut preview = PlayerPreview::new();
        for frame in 0..60 {
            let position = preview.position(&state, frame as f64 / 60.0);
            assert!((0.65..=0.751).contains(&position.0));
            assert_eq!(position.1, 20.0);
        }
        assert_eq!(PlayerSession::capture(&state), before);
        assert_eq!(state.world.get_block(1, 19), BlockType::Stone);
    }
    #[test]
    fn collision_stop_interpolates_inside_resolved_endpoints_and_teleports_snap() {
        let mut history = MotionHistory::default();
        history.positions.insert("player", vec![(0.5, 10.0, 10)]);
        let mut last = 0.5;
        for frame in 0..=60 {
            let (x, y) = history.position("player", 0, (0.75, 10.0), 11, frame as f64 / 60.0);
            assert!((last..=0.75).contains(&x));
            assert_eq!(y, 10.0);
            last = x;
        }
        assert_eq!(
            history.position("player", 0, (25.0, 40.0), 11, 0.1),
            (25.0, 40.0)
        );
        assert_eq!(
            history.position("player", 0, (0.7, 10.0), 0, 0.1),
            (0.7, 10.0)
        );
    }
}
