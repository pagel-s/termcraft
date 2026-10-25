//! Keep presentation current when a terminal briefly stops consuming updates.
//! Snapshots replace complete state; terrain packets contain complete columns.
//! Retain a preceding world as an anchor so an away-and-back trip resets terrain.
use crate::engine::net::{
    ChunkColumnSnapshot, ChunkDeltaPacket, ServerSnapshot, ServerWireMessage, SnapshotDimension,
};
use std::collections::{BTreeMap, VecDeque};
use std::io;
use std::sync::{Arc, Mutex};
use std::time::Instant;

const MAX_DIMENSION_BATCHES: usize = 2;
// Match prediction's terrain cache, keeping the streamed window and a margin.
const TERRAIN_RADIUS: u32 = 6;

#[derive(Default)]
pub(super) struct ServerInbox(Arc<Mutex<Pending>>);

#[derive(Default)]
struct Pending {
    updates: VecDeque<ServerUpdate>,
    error: Option<io::Error>,
}

pub(super) struct ServerUpdate {
    pub dimension: SnapshotDimension,
    pub snapshot: Option<Box<ServerSnapshot>>,
    pub received_at: Instant,
    pub superseded_snapshots: u64,
    center: i32,
    terrain_tick: u64,
    terrain_protocol: u16,
    columns: BTreeMap<i32, ChunkColumnSnapshot>,
}

impl ServerUpdate {
    fn new(dimension: SnapshotDimension) -> Self {
        Self {
            dimension,
            snapshot: None,
            received_at: Instant::now(),
            superseded_snapshots: 0,
            center: 0,
            terrain_tick: 0,
            terrain_protocol: crate::engine::net::NET_PROTOCOL_VERSION,
            columns: BTreeMap::new(),
        }
    }

    fn trim_columns(&mut self) {
        self.columns
            .retain(|x, _| x.abs_diff(self.center) <= TERRAIN_RADIUS);
    }

    pub fn take_terrain(&mut self) -> Option<ChunkDeltaPacket> {
        if self.columns.is_empty() {
            return None;
        }
        Some(ChunkDeltaPacket {
            protocol: self.terrain_protocol,
            tick: self.terrain_tick,
            dimension: self.dimension,
            center_chunk_x: self.center,
            chunks: std::mem::take(&mut self.columns).into_values().collect(),
        })
    }
}

impl Clone for ServerInbox {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl ServerInbox {
    /// Only complete superseded snapshots/columns are replaced. Keep the last
    /// two world epochs separate: the preceding epoch forces a terrain reset
    /// even if the final dimension matches the previously displayed world.
    pub fn push(&self, message: ServerWireMessage) -> bool {
        let mut pending = self.0.lock().unwrap_or_else(|e| e.into_inner());
        if pending.error.is_some() {
            return false;
        }
        let dimension = match &message {
            ServerWireMessage::Snapshot(snapshot) => snapshot.dimension,
            ServerWireMessage::ChunkDelta(delta) => delta.dimension,
            ServerWireMessage::Welcome(_) => {
                pending.error = Some(io::Error::other("unexpected second welcome"));
                return false;
            }
            ServerWireMessage::Rejected(_, reason) => {
                pending.error = Some(io::Error::other(reason.clone()));
                return false;
            }
        };
        if pending
            .updates
            .back()
            .is_none_or(|u| u.dimension != dimension)
        {
            if pending.updates.len() == MAX_DIMENSION_BATCHES {
                pending.updates.pop_front();
            }
            pending.updates.push_back(ServerUpdate::new(dimension));
        }
        let update = pending.updates.back_mut().unwrap();
        match message {
            ServerWireMessage::Snapshot(snapshot) => {
                if update
                    .snapshot
                    .as_ref()
                    .is_some_and(|old| old.tick > snapshot.tick)
                {
                    return true;
                }
                update.superseded_snapshots += u64::from(update.snapshot.is_some());
                update.center = (snapshot.player.x.floor() as i32)
                    .div_euclid(crate::world::chunk::CHUNK_WIDTH as i32);
                update.snapshot = Some(snapshot);
                update.received_at = Instant::now();
                update.trim_columns();
            }
            ServerWireMessage::ChunkDelta(delta) => {
                if update.snapshot.is_none() {
                    update.center = delta.center_chunk_x;
                }
                update.terrain_tick = delta.tick;
                update.terrain_protocol = delta.protocol;
                for column in delta.chunks {
                    if column.chunk_x.abs_diff(update.center) <= TERRAIN_RADIUS {
                        update.columns.insert(column.chunk_x, column);
                    }
                }
                update.trim_columns();
            }
            _ => unreachable!(),
        }
        true
    }

    pub fn fail(&self, error: io::Error) {
        let mut pending = self.0.lock().unwrap_or_else(|e| e.into_inner());
        if pending.error.is_none() {
            pending.error = Some(error);
        }
    }

    pub fn take(&self) -> (VecDeque<ServerUpdate>, Option<io::Error>) {
        let mut pending = self.0.lock().unwrap_or_else(|e| e.into_inner());
        (std::mem::take(&mut pending.updates), pending.error.take())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::net::NET_PROTOCOL_VERSION;
    use crate::engine::state::GameState;
    use crate::world::block::BlockType;
    use crate::world::chunk::{CHUNK_HEIGHT, CHUNK_WIDTH};

    fn snapshot(tick: u64, dimension: SnapshotDimension) -> ServerWireMessage {
        let mut snapshot = ServerSnapshot::from_state(tick, &GameState::network_view());
        snapshot.dimension = dimension;
        ServerWireMessage::Snapshot(Box::new(snapshot))
    }

    fn terrain(
        tick: u64,
        dimension: SnapshotDimension,
        chunk: i32,
        block: BlockType,
    ) -> ServerWireMessage {
        ServerWireMessage::ChunkDelta(ChunkDeltaPacket {
            protocol: NET_PROTOCOL_VERSION,
            tick,
            dimension,
            center_chunk_x: chunk,
            chunks: vec![ChunkColumnSnapshot {
                chunk_x: chunk,
                revision: tick,
                biomes: Vec::new(),
                blocks: vec![block; CHUNK_HEIGHT * CHUNK_WIDTH],
            }],
        })
    }

    #[test]
    fn paused_renderer_receives_latest_state_and_latest_complete_column_edits() {
        let inbox = ServerInbox::default();
        for tick in 0..200 {
            assert!(inbox.push(snapshot(tick, SnapshotDimension::Overworld)));
            assert!(inbox.push(terrain(
                tick,
                SnapshotDimension::Overworld,
                0,
                if tick == 199 {
                    BlockType::Air
                } else {
                    BlockType::Stone
                }
            )));
            assert!(inbox.push(terrain(
                tick,
                SnapshotDimension::Overworld,
                1,
                BlockType::Dirt
            )));
        }
        let (mut updates, error) = inbox.take();
        assert!(error.is_none());
        assert_eq!(updates.len(), 1);
        let mut update = updates.pop_front().unwrap();
        assert_eq!(update.snapshot.as_ref().unwrap().tick, 199);
        assert_eq!(update.superseded_snapshots, 199);
        let terrain = update.take_terrain().unwrap();
        assert_eq!(terrain.chunks.len(), 2);
        assert_eq!(terrain.chunks[0].revision, 199);
        assert!(
            terrain.chunks[0]
                .blocks
                .iter()
                .all(|b| *b == BlockType::Air)
        );
        assert!(
            terrain.chunks[1]
                .blocks
                .iter()
                .all(|b| *b == BlockType::Dirt)
        );
        assert!(inbox.take().0.is_empty());
    }

    #[test]
    fn away_and_back_keeps_world_transitions_and_disjoint_terrain_ordered() {
        let inbox = ServerInbox::default();
        for (tick, dimension, block) in [
            (1, SnapshotDimension::Overworld, BlockType::Stone),
            (2, SnapshotDimension::Nether, BlockType::Netherrack),
            (3, SnapshotDimension::Overworld, BlockType::Dirt),
        ] {
            assert!(inbox.push(snapshot(tick, dimension)));
            assert!(inbox.push(terrain(tick, dimension, 0, block)));
        }
        let (mut updates, error) = inbox.take();
        assert!(error.is_none());
        assert_eq!(updates.len(), 2);
        for (tick, block) in [(2, BlockType::Netherrack), (3, BlockType::Dirt)] {
            let mut update = updates.pop_front().unwrap();
            assert_eq!(update.snapshot.as_ref().unwrap().tick, tick);
            assert_eq!(update.take_terrain().unwrap().chunks[0].blocks[0], block);
        }
    }

    #[test]
    fn repeated_world_changes_stay_bounded_without_disconnects() {
        let inbox = ServerInbox::default();
        for tick in 0..200 {
            assert!(inbox.push(snapshot(
                tick,
                if tick % 2 == 0 {
                    SnapshotDimension::Overworld
                } else {
                    SnapshotDimension::Nether
                }
            )));
        }
        let (mut updates, error) = inbox.take();
        assert!(error.is_none());
        assert_eq!(updates.len(), 2);
        assert_eq!(updates.pop_front().unwrap().snapshot.unwrap().tick, 198);
        assert_eq!(updates.pop_front().unwrap().snapshot.unwrap().tick, 199);
    }

    #[test]
    fn traveling_keeps_a_bounded_terrain_window_and_delivers_pending_state_before_error() {
        let inbox = ServerInbox::default();
        for chunk in 0..200 {
            let mut s = ServerSnapshot::from_state(chunk as u64, &GameState::network_view());
            s.player.x = (chunk * CHUNK_WIDTH as i32) as f64;
            assert!(inbox.push(ServerWireMessage::Snapshot(Box::new(s))));
            assert!(inbox.push(terrain(
                chunk as u64,
                SnapshotDimension::Overworld,
                chunk,
                BlockType::Stone
            )));
        }
        inbox.fail(io::Error::new(io::ErrorKind::ConnectionReset, "fixture"));
        let (mut updates, error) = inbox.take();
        let mut update = updates.pop_front().unwrap();
        assert_eq!(update.snapshot.as_ref().unwrap().tick, 199);
        assert!(update.take_terrain().unwrap().chunks.len() <= 13);
        assert_eq!(error.unwrap().kind(), io::ErrorKind::ConnectionReset);
    }
}
