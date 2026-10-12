use std::path::{Path, PathBuf};
use std::sync::Once;

pub const LEGACY_SAVE_DIR: &str = "saves";
#[cfg(not(test))]
const APP_DIR_NAME: &str = "termcraft";
#[cfg(not(test))]
const SAVE_SUBDIR: &str = "saves";
const SAVE_DIR_ENV: &str = "TERMCRAFT_SAVE_DIR";
static MIGRATE_LEGACY_SAVES: Once = Once::new();

pub fn save_dir() -> PathBuf {
    #[cfg(test)]
    {
        default_save_dir()
    }
    #[cfg(not(test))]
    {
        explicit_save_dir().unwrap_or_else(default_save_dir)
    }
}

pub fn save_file(file_name: &str) -> PathBuf {
    save_dir().join(file_name)
}

pub fn migrate_legacy_saves_once() {
    MIGRATE_LEGACY_SAVES.call_once(|| {
        if cfg!(test) || explicit_save_dir().is_some() {
            return;
        }

        let legacy_dir = PathBuf::from(LEGACY_SAVE_DIR);
        let target_dir = save_dir();
        if !legacy_dir.is_dir() || paths_are_same(&legacy_dir, &target_dir) {
            return;
        }

        let _ = copy_missing_files(&legacy_dir, &target_dir);
    });
}

fn explicit_save_dir() -> Option<PathBuf> {
    std::env::var_os(SAVE_DIR_ENV)
        .filter(|path| !path.is_empty())
        .map(PathBuf::from)
}

#[cfg(test)]
fn default_save_dir() -> PathBuf {
    // Parallel tests must never share progression/chunk files or touch real saves.
    // PID namespaces can reuse IDs across invocations sharing the same /tmp.
    static ROOT: std::sync::OnceLock<PathBuf> = std::sync::OnceLock::new();
    let root = ROOT.get_or_init(|| {
        std::env::temp_dir().join(format!(
            "termcraft-unit-{}-{:032x}",
            std::process::id(),
            rand::random::<u128>()
        ))
    });
    let thread = std::thread::current();
    let name = thread.name().unwrap_or("worker");
    let hash = name.bytes().fold(0xcbf29ce484222325u64, |hash, byte| {
        (hash ^ byte as u64).wrapping_mul(0x100000001b3)
    });
    root.join(format!("{hash:016x}"))
}

#[cfg(all(not(test), target_os = "windows"))]
fn default_save_dir() -> PathBuf {
    env_path("APPDATA")
        .or_else(|| env_path("USERPROFILE").map(|home| home.join("AppData").join("Roaming")))
        .map(|base| base.join(APP_DIR_NAME).join(SAVE_SUBDIR))
        .unwrap_or_else(|| PathBuf::from(LEGACY_SAVE_DIR))
}

#[cfg(all(not(test), target_os = "macos"))]
fn default_save_dir() -> PathBuf {
    env_path("HOME")
        .map(|home| {
            home.join("Library")
                .join("Application Support")
                .join(APP_DIR_NAME)
                .join(SAVE_SUBDIR)
        })
        .unwrap_or_else(|| PathBuf::from(LEGACY_SAVE_DIR))
}

#[cfg(all(not(test), unix, not(target_os = "macos")))]
fn default_save_dir() -> PathBuf {
    env_path("XDG_DATA_HOME")
        .or_else(|| env_path("HOME").map(|home| home.join(".local").join("share")))
        .map(|base| base.join(APP_DIR_NAME).join(SAVE_SUBDIR))
        .unwrap_or_else(|| PathBuf::from(LEGACY_SAVE_DIR))
}

#[cfg(all(not(test), not(unix), not(target_os = "windows")))]
fn default_save_dir() -> PathBuf {
    PathBuf::from(LEGACY_SAVE_DIR)
}

#[cfg(not(test))]
fn env_path(name: &str) -> Option<PathBuf> {
    std::env::var_os(name)
        .filter(|path| !path.is_empty())
        .map(PathBuf::from)
}

fn paths_are_same(a: &Path, b: &Path) -> bool {
    match (a.canonicalize(), b.canonicalize()) {
        (Ok(a), Ok(b)) => a == b,
        _ => false,
    }
}

fn copy_missing_files(source: &Path, target: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(target)?;
    for entry in std::fs::read_dir(source)? {
        let entry = entry?;
        let source_path = entry.path();
        let target_path = target.join(entry.file_name());
        let file_type = entry.file_type()?;
        if file_type.is_dir() {
            copy_missing_files(&source_path, &target_path)?;
        } else if file_type.is_file() && !is_temp_save_file(&source_path) && !target_path.exists() {
            std::fs::copy(source_path, target_path)?;
        }
    }
    Ok(())
}

fn is_temp_save_file(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.ends_with(".tmp"))
}
