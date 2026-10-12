//! Named worlds keep generated content and player saves separate from classic saves.
use crate::world::recipe::{RECIPE_FILE, WorldRecipe};
use serde_json::json;
use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

pub struct WorldStore {
    root: PathBuf,
}

impl Default for WorldStore {
    fn default() -> Self {
        Self::new(crate::save_path::save_dir().join("worlds"))
    }
}

impl WorldStore {
    pub fn new(root: PathBuf) -> Self {
        Self { root }
    }

    pub fn path(&self, id: &str) -> Result<PathBuf, String> {
        if id.is_empty()
            || id.len() > 48
            || !id
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
        {
            return Err(
                "world ID must contain 1–48 ASCII letters, digits, hyphens or underscores".into(),
            );
        }
        Ok(self.root.join(format!("world-{id}")))
    }

    pub fn create(&self, id: &str, prompt: &str, recipe: &WorldRecipe) -> Result<PathBuf, String> {
        recipe.validate()?;
        if prompt.len() > 4000
            || prompt
                .chars()
                .any(|c| c.is_control() && c != '\n' && c != '\t')
        {
            return Err("prompt exceeds 4000 bytes or contains control characters".into());
        }
        let path = self.path(id)?;
        let encoded = serde_json::to_vec_pretty(recipe).map_err(|e| e.to_string())?;
        fs::create_dir_all(&self.root).map_err(|e| e.to_string())?;
        // Exclusive creation also prevents replacing a previously played world.
        fs::create_dir(&path).map_err(|e| format!("cannot create world {id}: {e}"))?;
        let result = (|| -> io::Result<()> {
            let mut request = OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(path.join("request.json"))?;
            request.write_all(&serde_json::to_vec_pretty(&json!({
                "prompt":prompt, "generator_version":env!("CARGO_PKG_VERSION")
            }))?)?;
            request.sync_all()?;
            let temporary = path.join("world.json.tmp");
            let mut file = OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&temporary)?;
            file.write_all(&encoded)?;
            file.sync_all()?;
            fs::rename(temporary, path.join(RECIPE_FILE))?;
            Ok(())
        })();
        if let Err(error) = result {
            // Only this call's exclusively created directory can be removed here.
            let _ = fs::remove_dir_all(&path);
            return Err(error.to_string());
        }
        Ok(path)
    }

    pub fn open(&self, id: &str) -> Result<PathBuf, String> {
        let path = self.path(id)?;
        if WorldRecipe::read(&path)?.is_none() {
            return Err(format!("world {id} has no committed recipe"));
        }
        Ok(path)
    }

    pub fn list(&self) -> Result<Vec<serde_json::Value>, String> {
        let entries = match fs::read_dir(&self.root) {
            Ok(entries) => entries,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(e) => return Err(e.to_string()),
        };
        let mut worlds = Vec::new();
        for entry in entries {
            let entry = entry.map_err(|e| e.to_string())?;
            if !entry.file_type().map_err(|e| e.to_string())?.is_dir() {
                continue;
            }
            let name = entry.file_name().to_string_lossy().into_owned();
            let Some(id) = name.strip_prefix("world-") else {
                continue;
            };
            match WorldRecipe::read(&entry.path()) {
                Ok(Some(recipe)) => worlds
                    .push(json!({"id":id,"name":recipe.name,"description":recipe.description})),
                Ok(None) => {}
                Err(error) => worlds.push(json!({"id":id,"error":error})),
            }
        }
        worlds.sort_by_key(|world| world["id"].as_str().unwrap_or_default().to_string());
        Ok(worlds)
    }
}

/// Prevents two processes from writing one world's chunks/progression concurrently.
pub struct WorldLease {
    path: PathBuf,
}

impl WorldLease {
    pub fn acquire(root: &Path) -> Result<Self, String> {
        let path = root.join(".session.lock");
        let mut file = OpenOptions::new().write(true).create_new(true).open(&path)
            .map_err(|e| format!("world is already open, or cannot be locked ({e}); after a crash, verify it is closed before removing {}", path.display()))?;
        let lease = Self { path };
        writeln!(file, "{}", std::process::id()).map_err(|e| e.to_string())?;
        file.sync_all().map_err(|e| e.to_string())?;
        Ok(lease)
    }
}

impl Drop for WorldLease {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::state::GameState;
    use crate::world::{Dimension, block::BlockType};

    #[test]
    fn worlds_are_isolated_persist_edits_and_do_not_overwrite() {
        let root =
            std::env::temp_dir().join(format!("termcraft-world-isolation-{}", std::process::id()));
        let store = WorldStore::new(root.clone());
        let a = store.create("a", "snow", &WorldRecipe::example()).unwrap();
        let b = store.create("b", "snow", &WorldRecipe::example()).unwrap();
        assert!(store.path("../escape").is_err());
        assert!(
            store
                .create("a", "overwrite", &WorldRecipe::example())
                .is_err()
        );
        let lease = WorldLease::acquire(&a).unwrap();
        assert!(WorldLease::acquire(&a).is_err());
        let mut first = GameState::open_in_directory(&a).unwrap();
        first.world.set_block(0, 20, BlockType::Glass);
        first.player.health = 13.0;
        first.try_persist_world_and_progress().unwrap();
        let nether = first.world.for_dimension(Dimension::Nether);
        assert_eq!(nether.save_root(), a);
        drop(first);
        drop(lease);
        let loaded = GameState::open_in_directory(&a).unwrap();
        assert_eq!(loaded.world.get_block(0, 20), BlockType::Glass);
        assert_eq!(loaded.player.health, 13.0);
        let second = GameState::open_in_directory(&b).unwrap();
        assert_eq!(second.world.get_block(0, 20), BlockType::Air);
        assert_eq!(store.list().unwrap().len(), 2);
        drop((loaded, second, nether));
        fs::remove_dir_all(root).unwrap();
    }
}
