use super::Renderer;
use std::{
    io::{Read, Write},
    path::Path,
};

impl Renderer {
    pub(super) fn saved_view() -> u8 {
        read_view(&crate::save_path::save_file("view.json"))
    }
}
fn read_view(path: &Path) -> u8 {
    let mut bytes = Vec::new();
    let Ok(file) = std::fs::File::open(path) else {
        return 0;
    };
    if file.take(1025).read_to_end(&mut bytes).is_err() || bytes.len() > 1024 {
        return 0;
    }
    let Ok(value) = serde_json::from_slice::<serde_json::Value>(&bytes) else {
        return 0;
    };
    if value["version"] != 1 {
        return 0;
    }
    value["view"].as_u64().filter(|v| *v <= 2).unwrap_or(0) as u8
}
pub(super) fn write_view(path: &Path, view: u8) -> std::io::Result<()> {
    if view > 2 {
        return Err(std::io::Error::other("invalid view"));
    }
    let parent = path
        .parent()
        .ok_or_else(|| std::io::Error::other("view has no parent"))?;
    std::fs::create_dir_all(parent)?;
    let temp = parent.join(format!(
        ".view-{}-{}.tmp",
        std::process::id(),
        rand::random::<u64>()
    ));
    let result = (|| {
        let mut file = std::fs::File::create(&temp)?;
        writeln!(file, "{{\"version\":1,\"view\":{view}}}")?;
        file.sync_all()?;
        std::fs::rename(&temp, path)
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(temp);
    }
    result
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn selected_view_roundtrips_and_invalid_preferences_fall_back_safely() {
        let root = std::env::temp_dir().join(format!("tc-view-{}", rand::random::<u64>()));
        let path = root.join("view.json");
        for view in [2, 1, 0, 2] {
            write_view(&path, view).unwrap();
            assert_eq!(read_view(&path), view);
        }
        std::fs::write(&path, b"{\"version\":1,\"view\":255}").unwrap();
        assert_eq!(read_view(&path), 0);
        std::fs::write(&path, b"bad").unwrap();
        assert_eq!(read_view(&path), 0);
        std::fs::remove_dir_all(root).unwrap();
    }
}
