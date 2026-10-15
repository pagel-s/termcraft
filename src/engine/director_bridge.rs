//! Opt-in, authenticated loopback bridge. Only the simulation applies world edits.
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::io::{self, BufRead, BufReader, Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
    mpsc,
};
use std::time::Duration;

const FILE: &str = ".director-bridge.json";
const LIMIT: u64 = 65536;

#[derive(Deserialize, Serialize)]
#[serde(tag = "method", rename_all = "snake_case", deny_unknown_fields)]
pub(crate) enum Command {
    Observe,
    Poll,
    Request { prompt: String },
    Apply { request_id: String, plan: Value },
    Cancel,
}
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Envelope {
    token: String,
    command: Command,
}
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Descriptor {
    version: u8,
    address: SocketAddr,
    token: String,
}

pub(crate) struct Request {
    pub command: Command,
    pub reply: mpsc::SyncSender<Result<Value, String>>,
}
pub(crate) struct Host {
    pub root: PathBuf,
    pub requests: mpsc::Receiver<Request>,
    stop: Arc<AtomicBool>,
    token: String,
}

fn read_json<R: Read>(reader: R) -> Result<Value, String> {
    let mut bytes = Vec::new();
    BufReader::new(reader.take(LIMIT + 1))
        .read_until(b'\n', &mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() as u64 > LIMIT {
        return Err("bridge message exceeds 64 KiB".into());
    }
    serde_json::from_slice(&bytes).map_err(|e| format!("invalid bridge message: {e}"))
}
fn write_json(writer: &mut impl Write, value: &Value) -> io::Result<()> {
    let bytes = serde_json::to_vec(value)?;
    if bytes.len() as u64 > LIMIT {
        return Err(io::Error::other("bridge response exceeds 64 KiB"));
    }
    writer.write_all(&bytes)?;
    writer.write_all(b"\n")?;
    writer.flush()
}
impl Host {
    pub fn start(root: &Path) -> Result<Self, String> {
        let listener = TcpListener::bind("127.0.0.1:0")
            .map_err(|e| format!("cannot start local game-master bridge: {e}"))?;
        listener.set_nonblocking(true).map_err(|e| e.to_string())?;
        let token = super::server_save::token();
        let descriptor = Descriptor {
            version: 1,
            address: listener.local_addr().map_err(|e| e.to_string())?,
            token: token.clone(),
        };
        std::fs::create_dir_all(root).map_err(|e| e.to_string())?;
        let temporary = root.join(format!("{FILE}.{}.tmp", super::server_save::token()));
        let mut options = std::fs::OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let save = (|| -> io::Result<()> {
            let mut file = options.open(&temporary)?;
            serde_json::to_writer(&mut file, &descriptor)?;
            file.sync_all()?;
            std::fs::rename(&temporary, root.join(FILE))
        })();
        if let Err(e) = save {
            let _ = std::fs::remove_file(&temporary);
            return Err(e.to_string());
        }
        let (send, requests) = mpsc::sync_channel(16);
        let stop = Arc::new(AtomicBool::new(false));
        let stopped = stop.clone();
        let credential = token.clone();
        std::thread::spawn(move || {
            while !stopped.load(Ordering::Relaxed) {
                let (mut stream, _) = match listener.accept() {
                    Ok(pair) => pair,
                    Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                        std::thread::sleep(Duration::from_millis(10));
                        continue;
                    }
                    Err(_) => break,
                };
                // Accepted sockets can inherit nonblocking mode on BSD/macOS.
                if stream.set_nonblocking(false).is_err() {
                    continue;
                }
                let _ = stream.set_read_timeout(Some(Duration::from_millis(500)));
                let _ = stream.set_write_timeout(Some(Duration::from_millis(500)));
                let result = (|| -> Result<Value, String> {
                    let envelope: Envelope = serde_json::from_value(read_json(&mut stream)?)
                        .map_err(|e| e.to_string())?;
                    if envelope.token != credential {
                        return Err("game-master session credential rejected; reconnect".into());
                    }
                    let (reply, result) = mpsc::sync_channel(1);
                    send.try_send(Request {
                        command: envelope.command,
                        reply,
                    })
                    .map_err(|_| "game-master bridge busy or closed".to_string())?;
                    result
                        .recv_timeout(Duration::from_secs(2))
                        .map_err(|_| "game is not responding; no result confirmed".to_string())?
                })();
                let value = match result {
                    Ok(value) => json!({"ok":true,"result":value}),
                    Err(error) => json!({"ok":false,"error":error}),
                };
                let _ = write_json(&mut stream, &value);
            }
        });
        Ok(Self {
            root: root.to_path_buf(),
            requests,
            stop,
            token,
        })
    }
}
impl Drop for Host {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        let file = self.root.join(FILE);
        if std::fs::read(&file)
            .ok()
            .and_then(|bytes| serde_json::from_slice::<Descriptor>(&bytes).ok())
            .is_some_and(|d| d.token == self.token)
        {
            let _ = std::fs::remove_file(file);
        }
    }
}

pub(crate) struct Client {
    descriptor: Descriptor,
}
impl Client {
    pub fn connect(root: &Path) -> Result<Self, String> {
        let file = std::fs::File::open(root.join(FILE)).map_err(|_| {
            "Enable live editing with /connect in the running game's T chat first.".to_string()
        })?;
        let mut bytes = Vec::new();
        file.take(8193)
            .read_to_end(&mut bytes)
            .map_err(|e| e.to_string())?;
        if bytes.len() > 8192 {
            return Err("invalid bridge descriptor".into());
        }
        let descriptor: Descriptor = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
        if descriptor.version != 1
            || !descriptor.address.ip().is_loopback()
            || !super::server_save::valid_token(&descriptor.token)
        {
            return Err("invalid bridge descriptor".into());
        }
        let client = Self { descriptor };
        client.call(Command::Observe)?;
        Ok(client)
    }
    pub fn call(&self, command: Command) -> Result<Value, String> {
        let mut stream =
            TcpStream::connect_timeout(&self.descriptor.address, Duration::from_secs(1))
                .map_err(|e| format!("running game unavailable; reconnect: {e}"))?;
        stream
            .set_read_timeout(Some(Duration::from_secs(3)))
            .map_err(|e| e.to_string())?;
        stream
            .set_write_timeout(Some(Duration::from_secs(1)))
            .map_err(|e| e.to_string())?;
        write_json(
            &mut stream,
            &serde_json::to_value(Envelope {
                token: self.descriptor.token.clone(),
                command,
            })
            .map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())?;
        let value = read_json(stream)?;
        if value["ok"] == true {
            Ok(value["result"].clone())
        } else {
            Err(value["error"]
                .as_str()
                .unwrap_or("bridge failed")
                .to_string())
        }
    }
}
