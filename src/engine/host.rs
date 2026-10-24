//! One-terminal hosting. The child saves and stops when its control pipe closes.
use std::{
    io,
    net::{SocketAddr, TcpStream, UdpSocket},
    process::{Child, Command, Stdio},
    thread,
    time::{Duration, Instant},
};

struct HostedServer(Child);
impl Drop for HostedServer {
    fn drop(&mut self) {
        drop(self.0.stdin.take());
        let deadline = Instant::now() + Duration::from_secs(10);
        while self.0.try_wait().ok().flatten().is_none() {
            if Instant::now() >= deadline {
                let _ = self.0.kill();
                let _ = self.0.wait();
                break;
            }
            thread::sleep(Duration::from_millis(10));
        }
    }
}

pub fn host(address: &str, profile: &str) -> io::Result<()> {
    let bind: SocketAddr = address.parse().map_err(io::Error::other)?;
    if bind.port() == 0 {
        return Err(io::Error::other("choose a nonzero host port"));
    }
    let local = SocketAddr::new(
        if bind.ip().is_unspecified() {
            if bind.is_ipv4() {
                std::net::Ipv4Addr::LOCALHOST.into()
            } else {
                std::net::Ipv6Addr::LOCALHOST.into()
            }
        } else {
            bind.ip()
        },
        bind.port(),
    );
    if TcpStream::connect_timeout(&local, Duration::from_millis(100)).is_ok() {
        return Err(io::Error::other(format!(
            "{address} is already in use; choose another port"
        )));
    }
    let root = crate::save_directory().join("multiplayer/default");
    std::fs::create_dir_all(&root)?;
    let log_path = root.join("host.log");
    let log = std::fs::File::create(&log_path)?;
    let child = Command::new(std::env::current_exe()?)
        .args(["server", address])
        .env("TERMCRAFT_MANAGED_SERVER", "1")
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(log)
        .spawn()?;
    let mut server = HostedServer(child);
    let deadline = Instant::now() + Duration::from_secs(30);
    eprintln!("Opening your multiplayer world…");
    loop {
        if server.0.try_wait()?.is_some() {
            return Err(io::Error::other(std::fs::read_to_string(&log_path)?));
        }
        if TcpStream::connect_timeout(&local, Duration::from_millis(100)).is_ok() {
            break;
        }
        if Instant::now() >= deadline {
            return Err(io::Error::other(
                "multiplayer startup timed out; inspect host.log in the multiplayer save",
            ));
        }
        thread::sleep(Duration::from_millis(20));
    }
    let advertised = if bind.ip().is_unspecified() && bind.is_ipv4() {
        UdpSocket::bind("0.0.0.0:0")
            .and_then(|socket| {
                // connect selects the route's local address; no datagram is sent.
                socket.connect("192.0.2.1:9")?;
                socket.local_addr()
            })
            .map(|a| SocketAddr::new(a.ip(), bind.port()))
            .unwrap_or(local)
    } else {
        local
    };
    let hint = advertised.to_string();
    super::network_client::run_tcp_client_with_hint(&local.to_string(), profile, &hint)
}
