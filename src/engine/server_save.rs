//! Multiplayer identities and full player records, separate from single-player progression.
use super::state::{
    GameState, MAX_CHECKPOINT_BYTES, PlayerSession, RuntimeCheckpoint, SharedProgress,
    SharedSettings,
};
use crate::world::Dimension;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::io::{self, Read, Write};
use std::path::Path;

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ServerSave {
    pub version: u8,
    pub id: String,
    pub settings: SharedSettings,
    pub players: BTreeMap<String, PlayerSession>,
    #[serde(default)]
    pub dimensions: BTreeMap<String, Dimension>,
    #[serde(default)]
    pub worlds: BTreeMap<Dimension, SharedSettings>,
    #[serde(default)]
    pub(crate) progression: SharedProgress,
    #[serde(default)]
    pub(crate) runtime: Option<RuntimeCheckpoint>,
}
pub fn token() -> String {
    format!("{:032x}", rand::random::<u128>())
}
pub fn valid_token(s: &str) -> bool {
    s.len() == 32 && s.bytes().all(|c| c.is_ascii_hexdigit())
}
impl ServerSave {
    pub(crate) fn fresh(state: &GameState) -> Self {
        Self {
            version: 3,
            id: token(),
            settings: SharedSettings::capture(state),
            players: BTreeMap::new(),
            dimensions: BTreeMap::new(),
            worlds: BTreeMap::new(),
            progression: SharedProgress::capture(state),
            runtime: Some(RuntimeCheckpoint::capture(state)),
        }
    }
    pub fn read(state: &GameState) -> io::Result<Self> {
        let file = match std::fs::File::open(state.world.save_root().join("server-state.json")) {
            Ok(f) => f,
            Err(e) if e.kind() == io::ErrorKind::NotFound => {
                return Ok(Self::fresh(state));
            }
            Err(e) => return Err(e),
        };
        let mut bytes = Vec::new();
        file.take(MAX_CHECKPOINT_BYTES + 1)
            .read_to_end(&mut bytes)?;
        if bytes.len() as u64 > MAX_CHECKPOINT_BYTES {
            return Err(io::Error::other("server save exceeds 16 MiB"));
        }
        let save: Self = serde_json::from_slice(&bytes)?;
        if !matches!(save.version, 1..=3)
            || (save.version == 3 && save.runtime.is_none())
            || !valid_token(&save.id)
            || save.players.len() > 64
            || save.players.keys().any(|s| !valid_token(s))
            || save
                .dimensions
                .keys()
                .any(|s| !save.players.contains_key(s))
        {
            return Err(io::Error::other("invalid server save; preserved"));
        }
        for player in save.players.values() {
            player.validate().map_err(io::Error::other)?;
        }
        if let Some(runtime) = &save.runtime {
            runtime.validate().map_err(io::Error::other)?;
        }
        Ok(save)
    }
    /// Publish player records and finish their world checkpoint. The owner must
    /// flush every dimension and sidecar first (as ServerSimulation::save does).
    pub fn write(&self, root: &Path) -> io::Result<()> {
        if let Some(runtime) = &self.runtime {
            runtime.validate().map_err(io::Error::other)?;
        }
        let data = serde_json::to_vec(self)?;
        if data.len() as u64 > MAX_CHECKPOINT_BYTES {
            return Err(io::Error::other(
                "server save exceeds 16 MiB; previous save preserved",
            ));
        }
        crate::checkpoint::protect(&root.join("server-state.json"))?;
        let temp = root.join(format!(
            ".server-state-{}-{}.tmp",
            std::process::id(),
            rand::random::<u64>()
        ));
        let result = (|| {
            let mut options = std::fs::OpenOptions::new();
            options.write(true).create_new(true);
            #[cfg(unix)]
            {
                use std::os::unix::fs::OpenOptionsExt;
                options.mode(0o600);
            }
            let mut f = options.open(&temp)?;
            f.write_all(&data)?;
            f.sync_all()?;
            drop(f);
            std::fs::rename(&temp, root.join("server-state.json"))?;
            crate::checkpoint::sync_directory(root)
        })();
        if result.is_err() {
            let _ = std::fs::remove_file(temp);
        }
        result?;
        crate::checkpoint::commit(root)
    }
}
