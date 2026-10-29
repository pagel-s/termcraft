//! Synchronous working filesystem. JavaScript commits complete snapshots to
//! IndexedDB atomically; no browser file ever maps to a host filesystem path.
use serde::{Deserialize, Serialize};
use std::{
    cell::RefCell,
    collections::BTreeMap,
    ffi::OsString,
    io::{self, Read, Write},
    path::{Component, Path, PathBuf},
};

const MAX_BYTES: usize = 32 * 1024 * 1024;
thread_local! { static FILES: RefCell<BTreeMap<String, Option<Vec<u8>>>> = RefCell::new(BTreeMap::from([(String::new(), None)])); }
fn error(kind: io::ErrorKind, text: &str) -> io::Error {
    io::Error::new(kind, text)
}
fn key(path: impl AsRef<Path>) -> io::Result<String> {
    let mut parts = Vec::new();
    for part in path.as_ref().components() {
        match part {
            Component::Normal(s) => parts.push(
                s.to_str()
                    .ok_or_else(|| error(io::ErrorKind::InvalidInput, "invalid browser path"))?,
            ),
            Component::CurDir | Component::RootDir => {}
            _ => {
                return Err(error(
                    io::ErrorKind::InvalidInput,
                    "browser paths cannot escape their save",
                ));
            }
        }
    }
    Ok(parts.join("/"))
}
fn parent(path: &str) -> &str {
    path.rsplit_once('/').map_or("", |(p, _)| p)
}
fn missing() -> io::Error {
    error(io::ErrorKind::NotFound, "browser save file not found")
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Snapshot {
    format: String,
    version: u8,
    files: BTreeMap<String, Option<Vec<u8>>>,
}
pub(crate) fn reset() {
    FILES.with(|f| *f.borrow_mut() = BTreeMap::from([(String::new(), None)]));
}
pub(crate) fn snapshot() -> io::Result<String> {
    FILES.with(|f| {
        let files = f
            .borrow()
            .iter()
            .filter(|(name, _)| !name.ends_with("/.session.lock"))
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();
        serde_json::to_string(&Snapshot {
            format: "termcraft-browser".into(),
            version: 1,
            files,
        })
        .map_err(io::Error::other)
    })
}
pub(crate) fn restore(text: &str) -> io::Result<()> {
    if text.len() > MAX_BYTES * 4 {
        return Err(error(
            io::ErrorKind::InvalidData,
            "browser save exceeds limit",
        ));
    }
    let saved: Snapshot = serde_json::from_str(text)?;
    if saved.format != "termcraft-browser" || saved.version != 1 || saved.files.len() > 4096 {
        return Err(error(
            io::ErrorKind::InvalidData,
            "unsupported browser save",
        ));
    }
    let mut total = 0usize;
    for (name, bytes) in &saved.files {
        if name.len() > 512 || key(name)? != *name || name.ends_with("/.session.lock") {
            return Err(error(
                io::ErrorKind::InvalidData,
                "invalid browser save path",
            ));
        }
        total = total.saturating_add(bytes.as_ref().map_or(0, Vec::len));
        if total > MAX_BYTES {
            return Err(error(
                io::ErrorKind::InvalidData,
                "browser save exceeds limit",
            ));
        }
        if !name.is_empty() && saved.files.get(parent(name)) != Some(&None) {
            return Err(error(
                io::ErrorKind::InvalidData,
                "browser save has no parent directory",
            ));
        }
    }
    if saved.files.get("") != Some(&None) {
        return Err(error(
            io::ErrorKind::InvalidData,
            "browser save has no root",
        ));
    }
    FILES.with(|f| *f.borrow_mut() = saved.files);
    Ok(())
}
pub(crate) fn exists(path: impl AsRef<Path>) -> io::Result<bool> {
    let p = key(path)?;
    Ok(FILES.with(|f| f.borrow().contains_key(&p)))
}
pub(crate) fn create_dir_all(path: impl AsRef<Path>) -> io::Result<()> {
    let path = key(path)?;
    FILES.with(|f| {
        let mut files = f.borrow_mut();
        let mut p = String::new();
        for part in path.split('/').filter(|s| !s.is_empty()) {
            if !p.is_empty() {
                p.push('/');
            }
            p.push_str(part);
            match files.get(&p) {
                Some(Some(_)) => {
                    return Err(error(io::ErrorKind::AlreadyExists, "file blocks directory"));
                }
                Some(None) => {}
                None => {
                    files.insert(p.clone(), None);
                }
            }
        }
        Ok(())
    })
}
pub(crate) fn create_dir(path: impl AsRef<Path>) -> io::Result<()> {
    let p = key(path)?;
    FILES.with(|f| {
        let mut files = f.borrow_mut();
        if files.contains_key(&p) {
            return Err(error(
                io::ErrorKind::AlreadyExists,
                "directory already exists",
            ));
        }
        if files.get(parent(&p)) != Some(&None) {
            return Err(missing());
        }
        files.insert(p, None);
        Ok(())
    })
}
pub(crate) struct DirBuilder;
impl DirBuilder {
    pub fn new() -> Self {
        Self
    }
    pub fn create(&self, path: impl AsRef<Path>) -> io::Result<()> {
        create_dir(path)
    }
}
pub(crate) struct Metadata {
    directory: bool,
    size: u64,
}
impl Metadata {
    pub fn is_dir(&self) -> bool {
        self.directory
    }
    pub fn is_file(&self) -> bool {
        !self.directory
    }
    pub fn len(&self) -> u64 {
        self.size
    }
    pub fn file_type(&self) -> FileType {
        FileType {
            directory: self.directory,
        }
    }
}
pub(crate) struct FileType {
    directory: bool,
}
impl FileType {
    pub fn is_dir(&self) -> bool {
        self.directory
    }
    pub fn is_file(&self) -> bool {
        !self.directory
    }
    pub fn is_symlink(&self) -> bool {
        false
    }
}
pub(crate) fn symlink_metadata(path: impl AsRef<Path>) -> io::Result<Metadata> {
    let p = key(path)?;
    FILES.with(|f| {
        f.borrow()
            .get(&p)
            .map(|v| Metadata {
                directory: v.is_none(),
                size: v.as_ref().map_or(0, Vec::len) as u64,
            })
            .ok_or_else(missing)
    })
}
pub(crate) struct DirEntry {
    path: String,
    directory: bool,
}
impl DirEntry {
    pub fn path(&self) -> PathBuf {
        PathBuf::from(&self.path)
    }
    pub fn file_name(&self) -> OsString {
        self.path.rsplit('/').next().unwrap_or("").into()
    }
    pub fn file_type(&self) -> io::Result<FileType> {
        Ok(FileType {
            directory: self.directory,
        })
    }
}
pub(crate) fn read_dir(
    path: impl AsRef<Path>,
) -> io::Result<std::vec::IntoIter<io::Result<DirEntry>>> {
    let p = key(path)?;
    FILES.with(|f| {
        let files = f.borrow();
        if files.get(&p) != Some(&None) {
            return Err(missing());
        }
        let entries = files
            .iter()
            .filter(|(name, _)| !name.is_empty() && parent(name) == p)
            .map(|(name, v)| {
                Ok(DirEntry {
                    path: name.clone(),
                    directory: v.is_none(),
                })
            })
            .collect::<Vec<_>>();
        Ok(entries.into_iter())
    })
}
pub(crate) fn read(path: impl AsRef<Path>) -> io::Result<Vec<u8>> {
    let p = key(path)?;
    FILES.with(|f| {
        f.borrow()
            .get(&p)
            .and_then(Option::as_ref)
            .cloned()
            .ok_or_else(missing)
    })
}
pub(crate) fn write(path: impl AsRef<Path>, bytes: impl AsRef<[u8]>) -> io::Result<()> {
    File::create(path)?.write_all(bytes.as_ref())
}
pub(crate) fn rename(from: impl AsRef<Path>, to: impl AsRef<Path>) -> io::Result<()> {
    let from = key(from)?;
    let to = key(to)?;
    if from == to {
        return if exists(&from)? {
            Ok(())
        } else {
            Err(missing())
        };
    }
    FILES.with(|f| {
        let mut files = f.borrow_mut();
        if !files.contains_key(&from) || files.get(parent(&to)) != Some(&None) {
            return Err(missing());
        }
        if to.starts_with(&(from.clone() + "/")) {
            return Err(error(
                io::ErrorKind::InvalidInput,
                "cannot move directory into itself",
            ));
        }
        if files.get(&to) == Some(&None) {
            return Err(error(
                io::ErrorKind::AlreadyExists,
                "destination directory exists",
            ));
        }
        let names = files
            .keys()
            .filter(|p| **p == from || p.starts_with(&(from.clone() + "/")))
            .cloned()
            .collect::<Vec<_>>();
        for name in names {
            let value = files.remove(&name).unwrap();
            files.insert(format!("{to}{}", &name[from.len()..]), value);
        }
        Ok(())
    })
}
pub(crate) fn remove_file(path: impl AsRef<Path>) -> io::Result<()> {
    let p = key(path)?;
    FILES.with(|f| {
        let mut files = f.borrow_mut();
        if !matches!(files.get(&p), Some(Some(_))) {
            return Err(missing());
        }
        files.remove(&p);
        Ok(())
    })
}
pub(crate) fn remove_dir_all(path: impl AsRef<Path>) -> io::Result<()> {
    let p = key(path)?;
    FILES.with(|f| {
        let mut files = f.borrow_mut();
        if files.get(&p) != Some(&None) {
            return Err(missing());
        }
        files.retain(|name, _| name != &p && !name.starts_with(&(p.clone() + "/")));
        Ok(())
    })
}
pub(crate) fn copy(from: impl AsRef<Path>, to: impl AsRef<Path>) -> io::Result<u64> {
    let bytes = read(from)?;
    write(to, &bytes)?;
    Ok(bytes.len() as u64)
}
pub(crate) fn hard_link(from: impl AsRef<Path>, to: impl AsRef<Path>) -> io::Result<()> {
    let bytes = read(from)?;
    OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(to)?
        .write_all(&bytes)
}
pub(crate) struct File {
    path: String,
    offset: usize,
    writable: bool,
}
impl File {
    pub fn open(path: impl AsRef<Path>) -> io::Result<Self> {
        OpenOptions::new().read(true).open(path)
    }
    pub fn create(path: impl AsRef<Path>) -> io::Result<Self> {
        OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(path)
    }
    pub fn sync_all(&self) -> io::Result<()> {
        Ok(())
    }
    pub fn metadata(&self) -> io::Result<Metadata> {
        symlink_metadata(&self.path)
    }
}
impl Read for File {
    fn read(&mut self, output: &mut [u8]) -> io::Result<usize> {
        FILES.with(|f| {
            let files = f.borrow();
            let bytes = files
                .get(&self.path)
                .and_then(Option::as_ref)
                .ok_or_else(missing)?;
            let count = output.len().min(bytes.len().saturating_sub(self.offset));
            output[..count].copy_from_slice(&bytes[self.offset..self.offset + count]);
            self.offset += count;
            Ok(count)
        })
    }
}
impl Write for File {
    fn write(&mut self, input: &[u8]) -> io::Result<usize> {
        if !self.writable {
            return Err(error(
                io::ErrorKind::PermissionDenied,
                "browser file is read only",
            ));
        }
        let end = self
            .offset
            .checked_add(input.len())
            .filter(|end| *end <= MAX_BYTES)
            .ok_or_else(|| error(io::ErrorKind::OutOfMemory, "browser file exceeds limit"))?;
        FILES.with(|f| {
            let mut files = f.borrow_mut();
            let bytes = files
                .get_mut(&self.path)
                .and_then(Option::as_mut)
                .ok_or_else(missing)?;
            bytes.resize(bytes.len().max(end), 0);
            bytes[self.offset..end].copy_from_slice(input);
            self.offset = end;
            Ok(input.len())
        })
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
#[derive(Default)]
pub(crate) struct OpenOptions {
    read: bool,
    write: bool,
    create: bool,
    create_new: bool,
    truncate: bool,
}
impl OpenOptions {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn read(&mut self, v: bool) -> &mut Self {
        self.read = v;
        self
    }
    pub fn write(&mut self, v: bool) -> &mut Self {
        self.write = v;
        self
    }
    pub fn create(&mut self, v: bool) -> &mut Self {
        self.create = v;
        self
    }
    pub fn create_new(&mut self, v: bool) -> &mut Self {
        self.create_new = v;
        self
    }
    pub fn truncate(&mut self, v: bool) -> &mut Self {
        self.truncate = v;
        self
    }
    pub fn open(&self, path: impl AsRef<Path>) -> io::Result<File> {
        let p = key(path)?;
        FILES.with(|f| {
            let mut files = f.borrow_mut();
            if self.create_new && files.contains_key(&p) {
                return Err(error(
                    io::ErrorKind::AlreadyExists,
                    "browser file already exists",
                ));
            }
            if files.get(parent(&p)) != Some(&None) {
                return Err(missing());
            }
            if files.get(&p) == Some(&None) {
                return Err(error(
                    io::ErrorKind::IsADirectory,
                    "browser path is a directory",
                ));
            }
            if !files.contains_key(&p) {
                if !(self.create || self.create_new) {
                    return Err(missing());
                }
                files.insert(p.clone(), Some(Vec::new()));
            }
            if self.truncate {
                files.insert(p.clone(), Some(Vec::new()));
            }
            Ok(File {
                path: p,
                offset: 0,
                writable: self.write,
            })
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn snapshots_preserve_atomic_replacement_and_reject_invalid_imports() {
        reset();
        create_dir_all("saves/world").unwrap();
        write("saves/world/chunk.bin", b"old").unwrap();
        let old = snapshot().unwrap();
        write("saves/world/chunk.tmp", b"new").unwrap();
        rename("saves/world/chunk.tmp", "saves/world/chunk.bin").unwrap();
        assert_eq!(read("saves/world/chunk.bin").unwrap(), b"new");
        restore(&old).unwrap();
        assert_eq!(read("saves/world/chunk.bin").unwrap(), b"old");
        assert!(restore(&old.replace("saves/world/chunk.bin", "../escape")).is_err());
        assert_eq!(read("saves/world/chunk.bin").unwrap(), b"old");
    }
}
