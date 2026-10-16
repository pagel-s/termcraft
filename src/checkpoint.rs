//! Recoverable, cross-file checkpoints for an exclusively owned world.
//!
//! Chunk eviction/autosave can publish terrain before the player/server file.
//! Keep synced before-images until the owner commits all dimensions and player
//! records. Startup rolls an interrupted checkpoint back before loading anything.
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    fs::{self, File, OpenOptions},
    io::{self, Read, Write},
    path::{Path, PathBuf},
};

const DIRECTORY: &str = ".world-checkpoint";
const MANIFEST: &str = "manifest.json";
const COMPLETE: &str = "complete";
const MAX_FILE: u64 = 32 * 1024 * 1024;
const MAX_MANIFEST: u64 = 4 * 1024 * 1024;

#[derive(Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Journal {
    version: u8,
    files: BTreeMap<String, Option<Before>>,
    crc32: u32,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Before {
    size: u64,
    crc32: u32,
}

fn invalid(message: &str) -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidData,
        format!("world checkpoint {message}; journal preserved"),
    )
}

fn world_file(name: &str) -> bool {
    if matches!(
        name,
        "player_progression.bin"
            | "server-state.json"
            | "environment.json"
            | "director.json"
            | "lantern-trial.json"
            | "world.json"
            | "request.json"
    ) {
        return true;
    }
    for dimension in ["overworld", "nether", "end"] {
        if name == format!("{dimension}_world_seed.bin") {
            return true;
        }
        if let Some(x) = name
            .strip_prefix(&format!("{dimension}_chunk_"))
            .and_then(|s| s.strip_suffix(".bin"))
        {
            return x.parse::<i32>().is_ok();
        }
    }
    false
}

fn regular(path: &Path) -> io::Result<bool> {
    match fs::symlink_metadata(path) {
        Ok(m) if m.is_file() => Ok(true),
        Ok(_) => Err(invalid("contains a non-regular file")),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(e) => Err(e),
    }
}

fn read_bounded(path: &Path, max: u64) -> io::Result<Vec<u8>> {
    if !regular(path)? {
        return Err(invalid("is missing a required file"));
    }
    let mut bytes = Vec::new();
    File::open(path)?.take(max + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > max {
        return Err(invalid("file exceeds its limit"));
    }
    Ok(bytes)
}

fn crc(bytes: &[u8]) -> u32 {
    let mut crc = flate2::Crc::new();
    crc.update(bytes);
    crc.sum()
}

pub(crate) fn sync_directory(path: &Path) -> io::Result<()> {
    #[cfg(unix)]
    File::open(path)?.sync_all()?;
    #[cfg(not(unix))]
    let _ = path; // Rust's portable API cannot flush Windows directory handles.
    Ok(())
}

fn private_directory(path: &Path) -> io::Result<()> {
    let builder = fs::DirBuilder::new();
    #[cfg(unix)]
    let builder = {
        use std::os::unix::fs::DirBuilderExt;
        let mut builder = builder;
        builder.mode(0o700);
        builder
    };
    builder.create(path)
}

fn write_synced(path: &Path, bytes: &[u8]) -> io::Result<()> {
    if fs::symlink_metadata(path).is_ok() && !regular(path)? {
        return Err(invalid("invalid output file"));
    }
    let mut options = OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(path)?;
    file.write_all(bytes)?;
    file.sync_all()
}

fn replace(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let mut name = path.as_os_str().to_os_string();
    name.push(".tmp");
    replace_via(path, &PathBuf::from(name), bytes)
}

fn replace_via(path: &Path, temp: &Path, bytes: &[u8]) -> io::Result<()> {
    write_synced(temp, bytes)?;
    fs::rename(temp, path)?;
    sync_directory(path.parent().unwrap())
}

fn encode(data: &mut Journal) -> io::Result<Vec<u8>> {
    data.crc32 = crc(&serde_json::to_vec(&(data.version, &data.files))?);
    Ok(serde_json::to_vec(data)?)
}

fn present(root: &Path) -> io::Result<bool> {
    match fs::symlink_metadata(root.join(DIRECTORY)) {
        Ok(m) if m.is_dir() => Ok(true),
        Ok(_) => Err(invalid("directory is not a directory")),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(e) => Err(e),
    }
}

fn finished(directory: &Path) -> io::Result<bool> {
    if !regular(&directory.join(COMPLETE))? {
        return Ok(false);
    }
    match read_bounded(&directory.join(COMPLETE), 16)?.as_slice() {
        b"committed\n" | b"rolled-back\n" => Ok(true),
        _ => Err(invalid("has an invalid completion marker")),
    }
}

fn journal(directory: &Path) -> io::Result<Journal> {
    if !fs::symlink_metadata(directory.join("before"))?.is_dir() {
        return Err(invalid("before-images directory is invalid"));
    }
    let data: Journal =
        serde_json::from_slice(&read_bounded(&directory.join(MANIFEST), MAX_MANIFEST)?)
            .map_err(|_| invalid("manifest cannot be decoded"))?;
    if data.version != 1
        || data.crc32 != crc(&serde_json::to_vec(&(data.version, &data.files))?)
        || data.files.len() > 32768
        || data.files.iter().any(|(name, before)| {
            !world_file(name) || before.as_ref().is_some_and(|b| b.size > MAX_FILE)
        })
    {
        return Err(invalid("manifest is invalid"));
    }
    Ok(data)
}

fn cleanup(root: &Path) -> io::Result<()> {
    // Never delete the completion marker before the active directory disappears.
    // A process killed during recursive deletion must not mistake the remnants
    // for an unfinished checkpoint with missing before-images.
    let retired = root.join(format!(
        ".world-checkpoint-done-{:032x}",
        rand::random::<u128>()
    ));
    fs::rename(root.join(DIRECTORY), &retired)?;
    sync_directory(root)?;
    fs::remove_dir_all(retired)?;
    sync_directory(root)
}

fn nonce(text: &str) -> bool {
    text.len() == 32 && text.bytes().all(|b| b.is_ascii_hexdigit())
}

fn discard_orphans(root: &Path) -> io::Result<()> {
    // These namespaces are reserved for unpublished initialization, completed
    // cleanup and rollback temporaries. Never follow a symlink or remove an
    // unfinished active journal. Called only after successful checked recovery.
    if !root.exists() {
        return Ok(());
    }
    for entry in fs::read_dir(root)? {
        let entry = entry?;
        let name = entry.file_name();
        let Some(name) = name.to_str() else { continue };
        let retired = [".world-checkpoint-done-", ".world-checkpoint-init-"]
            .iter()
            .any(|prefix| name.strip_prefix(prefix).is_some_and(nonce));
        let restore = name
            .strip_prefix('.')
            .and_then(|s| s.strip_suffix(".tmp"))
            .and_then(|s| s.rsplit_once(".restore-"))
            .is_some_and(|(file, id)| world_file(file) && nonce(id));
        let kind = entry.file_type()?;
        if retired && kind.is_dir() {
            fs::remove_dir_all(entry.path())?;
        } else if restore && kind.is_file() {
            fs::remove_file(entry.path())?;
        }
    }
    sync_directory(root)
}

/// Activate checkpoint protection after checked startup. Raw World builders that
/// never open a GameState retain their existing standalone file-save behavior.
pub(crate) fn begin(root: &Path) -> io::Result<()> {
    fs::create_dir_all(root)?;
    let directory = root.join(DIRECTORY);
    if present(root)? {
        if !finished(&directory)? {
            journal(&directory)?;
            return Ok(());
        }
        cleanup(root)?;
    }
    let stage = root.join(format!(
        ".world-checkpoint-init-{:032x}",
        rand::random::<u128>()
    ));
    private_directory(&stage)?;
    let result = (|| {
        private_directory(&stage.join("before"))?;
        write_synced(
            &stage.join(MANIFEST),
            &encode(&mut Journal {
                version: 1,
                files: BTreeMap::new(),
                crc32: 0,
            })?,
        )?;
        sync_directory(&stage.join("before"))?;
        sync_directory(&stage)?;
        fs::rename(&stage, &directory)?;
        sync_directory(root)
    })();
    if result.is_err() {
        let _ = fs::remove_dir_all(stage);
    }
    result
}

/// Save the previous version *before* any temporary file or destination changes.
pub(crate) fn protect(path: &Path) -> io::Result<()> {
    let root = path.parent().ok_or_else(|| invalid("path has no parent"))?;
    if !present(root)? {
        return Ok(());
    }
    let name = path
        .file_name()
        .and_then(|s| s.to_str())
        .filter(|s| world_file(s))
        .ok_or_else(|| invalid("refuses an unexpected world filename"))?;
    begin(root)?;
    let directory = root.join(DIRECTORY);
    let mut data = journal(&directory)?;
    if data.files.contains_key(name) {
        return Ok(());
    }
    let before = if regular(path)? {
        let bytes = read_bounded(path, MAX_FILE)?;
        let backup = directory.join("before").join(name);
        write_synced(&backup, &bytes)?;
        sync_directory(&directory.join("before"))?;
        Some(Before {
            size: bytes.len() as u64,
            crc32: crc(&bytes),
        })
    } else {
        None
    };
    data.files.insert(name.to_string(), before);
    let bytes = encode(&mut data)?;
    if bytes.len() as u64 > MAX_MANIFEST {
        return Err(invalid("manifest exceeds its limit"));
    }
    replace(&directory.join(MANIFEST), &bytes)
}

/// Called only after every dimension, inventory/runtime and sidecar was saved.
pub(crate) fn commit(root: &Path) -> io::Result<()> {
    if !present(root)? {
        return Ok(());
    }
    let directory = root.join(DIRECTORY);
    if finished(&directory)? {
        return Ok(());
    }
    journal(&directory)?;
    sync_directory(root)?;
    replace(&directory.join(COMPLETE), b"committed\n")
}

/// Idempotent rollback before constructing any world, runtime or player session.
/// Recovery itself may be interrupted; before-images remain until its durable
/// completion marker exists. Preflight every backup before replacing any files.
pub(crate) fn recover(root: &Path) -> io::Result<()> {
    if !present(root)? {
        return discard_orphans(root);
    }
    let directory = root.join(DIRECTORY);
    if finished(&directory)? {
        cleanup(root)?;
        return discard_orphans(root);
    }
    let data = journal(&directory)?;
    for (name, before) in &data.files {
        if let Some(before) = before {
            let bytes = read_bounded(&directory.join("before").join(name), MAX_FILE)?;
            if bytes.len() as u64 != before.size || crc(&bytes) != before.crc32 {
                return Err(invalid("backup checksum does not match"));
            }
        }
        regular(&root.join(name))?;
    }
    for (name, before) in &data.files {
        let path = root.join(name);
        if before.is_some() {
            // Ordinary save temporaries may be left behind or obstructed after
            // the failed write. Recovery owns a separate, unpredictable name.
            let temp = root.join(format!(
                ".{name}.restore-{:032x}.tmp",
                rand::random::<u128>()
            ));
            replace_via(
                &path,
                &temp,
                &read_bounded(&directory.join("before").join(name), MAX_FILE)?,
            )?;
        } else if regular(&path)? {
            fs::remove_file(path)?;
        }
    }
    sync_directory(root)?;
    replace(&directory.join(COMPLETE), b"rolled-back\n")?;
    cleanup(root)?;
    discard_orphans(root)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn root() -> PathBuf {
        let root =
            std::env::temp_dir().join(format!("termcraft-journal-{:032x}", rand::random::<u128>()));
        fs::create_dir(&root).unwrap();
        root
    }
    #[test]
    fn rollback_restores_all_files_and_removes_new_files_and_is_repeatable() {
        let root = root();
        for name in [
            "player_progression.bin",
            "overworld_chunk_-1.bin",
            "nether_chunk_2.bin",
            "end_chunk_0.bin",
            "server-state.json",
            "environment.json",
            "director.json",
        ] {
            fs::write(root.join(name), name).unwrap();
        }
        begin(&root).unwrap();
        for name in [
            "player_progression.bin",
            "overworld_chunk_-1.bin",
            "nether_chunk_2.bin",
            "end_chunk_0.bin",
            "server-state.json",
            "environment.json",
            "director.json",
            "nether_chunk_99.bin",
        ] {
            protect(&root.join(name)).unwrap();
            fs::write(root.join(name), b"new").unwrap();
        }
        recover(&root).unwrap();
        recover(&root).unwrap();
        assert!(!root.join("nether_chunk_99.bin").exists());
        for name in [
            "player_progression.bin",
            "overworld_chunk_-1.bin",
            "nether_chunk_2.bin",
            "end_chunk_0.bin",
            "server-state.json",
            "environment.json",
            "director.json",
        ] {
            assert_eq!(fs::read(root.join(name)).unwrap(), name.as_bytes());
        }
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn successful_checkpoint_survives_partial_cleanup_and_next_checkpoint_can_rollback() {
        let root = root();
        let path = root.join("player_progression.bin");
        fs::write(&path, b"old").unwrap();
        begin(&root).unwrap();
        protect(&path).unwrap();
        fs::write(&path, b"new").unwrap();
        commit(&root).unwrap();
        fs::remove_file(root.join(DIRECTORY).join("before/player_progression.bin")).unwrap();
        recover(&root).unwrap();
        assert_eq!(fs::read(&path).unwrap(), b"new");
        begin(&root).unwrap();
        protect(&path).unwrap();
        fs::write(&path, b"newer").unwrap();
        protect(&path).unwrap();
        fs::write(&path, b"newest").unwrap();
        recover(&root).unwrap();
        assert_eq!(fs::read(&path).unwrap(), b"new");
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn damaged_before_image_fails_closed_before_replacing_any_world_files() {
        let root = root();
        let path = root.join("player_progression.bin");
        fs::write(&path, b"old").unwrap();
        begin(&root).unwrap();
        protect(&path).unwrap();
        fs::write(&path, b"new").unwrap();
        fs::write(
            root.join(DIRECTORY).join("before/player_progression.bin"),
            b"bad",
        )
        .unwrap();
        assert!(recover(&root).is_err());
        assert_eq!(fs::read(&path).unwrap(), b"new");
        assert!(root.join(DIRECTORY).exists());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn deleted_manifest_entry_fails_closed_before_replacing_any_world_files() {
        let root = root();
        let path = root.join("player_progression.bin");
        fs::write(&path, b"old").unwrap();
        begin(&root).unwrap();
        protect(&path).unwrap();
        fs::write(&path, b"new").unwrap();
        let manifest = root.join(DIRECTORY).join(MANIFEST);
        let mut data: Journal = serde_json::from_slice(&fs::read(&manifest).unwrap()).unwrap();
        data.files.clear();
        fs::write(manifest, serde_json::to_vec(&data).unwrap()).unwrap();
        assert!(recover(&root).is_err());
        assert_eq!(fs::read(&path).unwrap(), b"new");
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn startup_removes_only_reserved_abandoned_temporaries() {
        let root = root();
        let retired = root.join(format!(".world-checkpoint-done-{:032x}", 17));
        let init = root.join(format!(".world-checkpoint-init-{:032x}", 18));
        let restore = root.join(format!(".player_progression.bin.restore-{:032x}.tmp", 19));
        fs::create_dir(&retired).unwrap();
        fs::write(retired.join("partially-deleted"), b"old").unwrap();
        fs::create_dir(&init).unwrap();
        fs::write(&restore, b"partial").unwrap();
        let unrelated = root.join(".world-checkpoint-done-my-backup");
        fs::create_dir(&unrelated).unwrap();
        recover(&root).unwrap();
        assert!(!retired.exists() && !init.exists() && !restore.exists());
        assert!(unrelated.exists());
        fs::remove_dir_all(root).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn symlinked_before_images_are_rejected_without_touching_destination() {
        let root = root();
        begin(&root).unwrap();
        fs::remove_dir(root.join(DIRECTORY).join("before")).unwrap();
        std::os::unix::fs::symlink(&root, root.join(DIRECTORY).join("before")).unwrap();
        assert!(protect(&root.join("player_progression.bin")).is_err());
        assert!(recover(&root).is_err());
        assert!(!root.join("player_progression.bin").exists());
        fs::remove_dir_all(root).unwrap();
    }
}
