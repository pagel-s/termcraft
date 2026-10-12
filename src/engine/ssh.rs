//! SSH forwards a local-only game port; game input/rendering stay in the local terminal.
use std::io;
use std::net::{TcpListener, TcpStream};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

fn tunnel_arguments(
    destination: &str,
    local_port: u16,
    ssh_port: u16,
    game_port: u16,
) -> io::Result<Vec<String>> {
    if destination.is_empty()
        || destination.len() > 255
        || !destination.as_bytes()[0].is_ascii_alphanumeric()
        || !destination
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"._-@[]:".contains(&b))
        || ssh_port == 0
        || game_port == 0
    {
        return Err(io::Error::other(
            "use an SSH host alias or user@host, with valid nonzero ports",
        ));
    }
    Ok(vec![
        "-N".into(),
        "-T".into(),
        "-o".into(),
        "ExitOnForwardFailure=yes".into(),
        "-o".into(),
        "ServerAliveInterval=15".into(),
        "-o".into(),
        "ServerAliveCountMax=3".into(),
        "-p".into(),
        ssh_port.to_string(),
        "-L".into(),
        format!("127.0.0.1:{local_port}:127.0.0.1:{game_port}"),
        "--".into(),
        destination.into(),
    ])
}
struct Tunnel(Child);
impl Drop for Tunnel {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}
pub fn join(destination: &str, profile: &str, ssh_port: u16, game_port: u16) -> io::Result<()> {
    // Validate before asking SSH to authenticate or creating local profile data.
    if profile.is_empty()
        || profile.len() > 32
        || !profile
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"_-".contains(&b))
    {
        return Err(io::Error::other(
            "player profile needs 1–32 letters, digits, underscores or hyphens",
        ));
    }
    let listener = TcpListener::bind("127.0.0.1:0")?;
    let port = listener.local_addr()?.port();
    drop(listener);
    let arguments = tunnel_arguments(destination, port, ssh_port, game_port)?;
    eprintln!(
        "Connecting through SSH to {destination}. Complete SSH authentication before the game opens."
    );
    let mut tunnel = Tunnel(
        Command::new("ssh")
            .args(arguments)
            .stdin(Stdio::inherit())
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .spawn()
            .map_err(|e| io::Error::other(format!("cannot start OpenSSH client: {e}")))?,
    );
    let deadline = Instant::now() + Duration::from_secs(120);
    let address = format!("127.0.0.1:{port}");
    loop {
        if let Some(status) = tunnel.0.try_wait()? {
            return Err(io::Error::other(format!(
                "SSH exited ({status}); check host access and the server's loopback game port"
            )));
        }
        if TcpStream::connect_timeout(&address.parse().unwrap(), Duration::from_millis(100)).is_ok()
        {
            break;
        }
        if Instant::now() >= deadline {
            return Err(io::Error::other(
                "SSH authentication/forward startup timed out",
            ));
        }
        std::thread::sleep(Duration::from_millis(30));
    }
    // A fresh tunnel port must not create a fresh reconnect identity each run.
    super::network_client::run_tcp_client_with_identity(
        &address,
        profile,
        &format!("ssh:{destination}:{ssh_port}/{game_port}"),
    )
    .map_err(|error| {
        io::Error::new(
            error.kind(),
            format!(
                "SSH game connection failed: {error}. Check that TCP forwarding is permitted, \
                 the game server is running on loopback port {game_port}, and both Termcraft builds match."
            ),
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ssh_uses_loopback_forwarding_and_rejects_option_or_command_injection() {
        let args = tunnel_arguments("alice@game.example", 12345, 22, 25565).unwrap();
        assert!(args.contains(&"127.0.0.1:12345:127.0.0.1:25565".into()));
        assert_eq!(args[args.len() - 2], "--");
        for bad in [
            "-oProxyCommand=bad",
            "host;touch /tmp/x",
            "host $(bad)",
            "",
            "host\nother",
        ] {
            assert!(tunnel_arguments(bad, 12345, 22, 25565).is_err());
        }
        assert!(tunnel_arguments("game", 12345, 0, 25565).is_err());
    }
}
