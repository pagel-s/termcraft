//! Local reconnect credentials. Profiles distinguish players on one computer.
use crate::engine::net::Welcome;
use crate::engine::server_save::valid_token;
use serde::{Deserialize, Serialize};
use std::io::{self, Read, Write};
use std::path::PathBuf;

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Identity {
    address: String,
    profile: String,
    server_id: String,
    pub token: String,
}
pub(super) fn path(address: &str, profile: &str) -> io::Result<PathBuf> {
    if profile.is_empty()
        || profile.len() > 32
        || !profile
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || c == b'_' || c == b'-')
    {
        return Err(io::Error::other(
            "player profile must contain 1–32 letters, digits, underscores or hyphens",
        ));
    }
    // Stable FNV-1a; collisions are checked against the stored address on read.
    let hash = address.bytes().fold(0xcbf29ce484222325u64, |h, c| {
        (h ^ u64::from(c)).wrapping_mul(0x100000001b3)
    });
    Ok(crate::save_path::save_dir()
        .join("clients")
        .join(format!("{hash:016x}-{profile}.json")))
}
pub(super) fn read(address: &str, profile: &str) -> io::Result<Option<Identity>> {
    let file = match std::fs::File::open(path(address, profile)?) {
        Ok(f) => f,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e),
    };
    let mut bytes = Vec::new();
    file.take(8193).read_to_end(&mut bytes)?;
    if bytes.len() > 8192 {
        return Err(io::Error::other(
            "player credential exceeds 8 KiB; preserved",
        ));
    }
    let identity: Identity = serde_json::from_slice(&bytes)?;
    if identity.address != address
        || identity.profile != profile
        || !valid_token(&identity.token)
        || !valid_token(&identity.server_id)
    {
        return Err(io::Error::other("invalid player credential; preserved"));
    }
    Ok(Some(identity))
}
pub(super) fn store(
    address: &str,
    profile: &str,
    previous: Option<&Identity>,
    welcome: &Welcome,
) -> io::Result<()> {
    if !valid_token(&welcome.server_id)
        || !valid_token(&welcome.token)
        || previous.is_some_and(|p| p.server_id != welcome.server_id || p.token != welcome.token)
    {
        return Err(io::Error::other(
            "server identity changed; player credential preserved; use a different profile for a new world",
        ));
    }
    let path = path(address, profile)?;
    std::fs::create_dir_all(path.parent().expect("credential directory"))?;
    let temp = path.with_extension(format!("{}.tmp", crate::engine::server_save::token()));
    let identity = Identity {
        address: address.into(),
        profile: profile.into(),
        server_id: welcome.server_id.clone(),
        token: welcome.token.clone(),
    };
    let result = (|| {
        let mut options = std::fs::OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = options.open(&temp)?;
        file.write_all(&serde_json::to_vec(&identity)?)?;
        file.sync_all()?;
        drop(file);
        std::fs::rename(&temp, &path)
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
    fn profiles_are_separate_and_corrupt_credentials_are_preserved() {
        let addr = format!("test-{}", crate::engine::server_save::token());
        let welcome = Welcome {
            protocol: crate::engine::net::NET_PROTOCOL_VERSION,
            client_id: 3,
            server_id: crate::engine::server_save::token(),
            token: crate::engine::server_save::token(),
        };
        store(&addr, "alice", None, &welcome).unwrap();
        assert_eq!(read(&addr, "alice").unwrap().unwrap().token, welcome.token);
        assert!(read(&addr, "bob").unwrap().is_none());
        assert!(path(&addr, "../alice").is_err());
        let file = path(&addr, "alice").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                std::fs::metadata(&file).unwrap().permissions().mode() & 0o777,
                0o600
            );
        }
        std::fs::write(&file, b"broken").unwrap();
        assert!(read(&addr, "alice").is_err());
        assert_eq!(std::fs::read(&file).unwrap(), b"broken");
        std::fs::remove_file(file).unwrap();
    }
}
