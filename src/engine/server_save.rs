//! Multiplayer identities and full player records, separate from single-player progression.
use super::state::{GameState, PlayerSession, SharedProgress, SharedSettings};
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
            version: 2,
            id: token(),
            settings: SharedSettings::capture(state),
            players: BTreeMap::new(),
            dimensions: BTreeMap::new(),
            worlds: BTreeMap::new(),
            progression: SharedProgress::capture(state),
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
        file.take(8 * 1024 * 1024 + 1).read_to_end(&mut bytes)?;
        if bytes.len() > 8 * 1024 * 1024 {
            return Err(io::Error::other("server save exceeds 8 MiB"));
        }
        let mut save: Self = serde_json::from_slice(&bytes)?;
        if !matches!(save.version, 1 | 2)
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
        save.version = 2;
        Ok(save)
    }
    pub fn write(&self, root: &Path) -> io::Result<()> {
        let data = serde_json::to_vec(self)?;
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
            std::fs::rename(&temp, root.join("server-state.json"))
        })();
        if result.is_err() {
            let _ = std::fs::remove_file(temp);
        }
        result
    }
}
