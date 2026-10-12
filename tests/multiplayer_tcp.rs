//! Real socket/process regression: two players, contested mining and restart.
#![cfg(unix)]
use std::{
    io::{self, BufRead, Read, Write},
    net::{Shutdown, TcpListener, TcpStream},
    process::{Child, Command, Stdio},
    sync::mpsc,
    thread,
    time::{Duration, Instant},
};
use termcraft::engine::{command::ClientCommand, net::*, state::GameState};
use termcraft::{
    world::{
        item::{ItemType, Recipe},
        recipe::{Terrain, WorldRecipe},
    },
    worlds::WorldStore,
};
struct Server(Child);
impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

// Ordered byte relay with independent readers/writers: delay is applied to each
// received buffer's original timestamp, rather than sleeping before every read
// and accidentally turning latency into a bandwidth cap. The queues are bounded.
struct DelayedLink {
    address: String,
    stop: TcpStream,
}
impl DelayedLink {
    fn start(upstream: &str) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap().to_string();
        let target = TcpStream::connect(upstream).unwrap();
        let stop = target.try_clone().unwrap();
        thread::spawn(move || {
            let (downstream, _) = listener.accept().unwrap();
            for (mut reader, mut writer) in [
                (downstream.try_clone().unwrap(), target.try_clone().unwrap()),
                (target, downstream),
            ] {
                let _ = writer.set_nodelay(true);
                let _ = writer.set_write_timeout(Some(Duration::from_secs(2)));
                let (tx, rx) = mpsc::sync_channel::<(Instant, Vec<u8>)>(64);
                thread::spawn(move || {
                    let mut bytes = [0; 65536];
                    let mut packet = 0u64;
                    loop {
                        let size = match reader.read(&mut bytes) {
                            Ok(0) | Err(_) => break,
                            Ok(size) => size,
                        };
                        let due = Instant::now() + Duration::from_millis(80 + packet % 3 * 20);
                        packet += 1;
                        if tx.send((due, bytes[..size].to_vec())).is_err() {
                            break;
                        }
                    }
                    let _ = reader.shutdown(Shutdown::Both);
                });
                thread::spawn(move || {
                    while let Ok((due, bytes)) = rx.recv() {
                        thread::sleep(due.saturating_duration_since(Instant::now()));
                        if writer.write_all(&bytes).is_err() {
                            break;
                        }
                    }
                    let _ = writer.shutdown(Shutdown::Both);
                });
            }
        });
        Self { address, stop }
    }
}
impl Drop for DelayedLink {
    fn drop(&mut self) {
        let _ = self.stop.shutdown(Shutdown::Both);
    }
}

#[test]
#[ignore = "requires local TCP sockets and Unix process signals; default 12-second network soak"]
fn eight_players_with_delayed_link_and_stalled_readers_keep_server_responsive() -> io::Result<()> {
    let root =
        std::env::temp_dir().join(format!("termcraft-network-{:032x}", rand::random::<u128>()));
    std::fs::create_dir_all(&root)?;
    let mut recipe = WorldRecipe::example();
    recipe.terrain = Terrain::Plains;
    recipe.relief = 0;
    recipe.regions.clear();
    recipe.landmarks.clear();
    let world = WorldStore::new(root.join("worlds"))
        .create("tcp", "Delayed network qualification", &recipe)
        .unwrap();
    let mut state = GameState::open_in_directory(&world)?;
    state.apply_client_command(ClientCommand::ToggleRuleMobSpawning);
    state.try_save_progression()?;
    drop(state);
    let reservation = TcpListener::bind("127.0.0.1:0")?;
    let address = reservation.local_addr()?.to_string();
    drop(reservation);
    let mut server = Server::start(&root, &address);
    let link = DelayedLink::start(&address);
    let mut players = Vec::new();
    for index in 0..8 {
        players.push(Client::connect(
            if index == 7 { &link.address } else { &address },
            None,
        ));
    }
    for player in &players {
        player.until(|s| s.remote_players.len() == 7);
    }
    let reconnect_token = players[7].welcome.token.clone();
    let server_id = players[7].welcome.server_id.clone();
    let duration = std::env::var("TERMCRAFT_NETWORK_SOAK_SECONDS")
        .ok()
        .map(|v| v.parse::<u64>().expect("invalid soak duration"))
        .unwrap_or(12)
        .clamp(6, 3600);
    let started = Instant::now();
    let mut next_input = Instant::now();
    let mut last_snapshot = [Instant::now(); 8];
    let mut counts = [0u64; 8];
    let mut gaps = Vec::new();
    let mut delayed_gaps = Vec::new();
    let mut last_tick = [0u64; 8];
    let mut min_x = [f64::INFINITY; 8];
    let mut max_x = [f64::NEG_INFINITY; 8];
    while started.elapsed() < Duration::from_secs(duration) {
        if Instant::now() >= next_input {
            let right = (started.elapsed().as_secs() / 4).is_multiple_of(2);
            for (index, player) in players.iter_mut().enumerate() {
                let right = right != (index % 2 == 0);
                player.send(ClientCommand::SetMoveLeft(!right));
                player.send(ClientCommand::SetMoveRight(right));
            }
            next_input = Instant::now() + Duration::from_millis(250);
        }
        for (index, player) in players.iter().enumerate() {
            while let Ok(message) = player.incoming.try_recv() {
                if let ServerWireMessage::Snapshot(snapshot) = message {
                    assert!(!snapshot.death_screen_active, "fixture actor died");
                    assert_eq!(snapshot.remote_players.len(), 7);
                    assert!(snapshot.tick > last_tick[index]);
                    last_tick[index] = snapshot.tick;
                    min_x[index] = min_x[index].min(snapshot.player.x);
                    max_x[index] = max_x[index].max(snapshot.player.x);
                    if counts[index] > 0 {
                        let ms = last_snapshot[index].elapsed().as_secs_f64() * 1000.0;
                        if index == 7 {
                            delayed_gaps.push(ms);
                        } else {
                            gaps.push(ms);
                        }
                    }
                    last_snapshot[index] = Instant::now();
                    counts[index] += 1;
                }
            }
            assert!(
                last_snapshot[index].elapsed() < Duration::from_secs(2),
                "snapshot stream stalled"
            );
        }
        assert!(server.0.try_wait()?.is_none());
        thread::sleep(Duration::from_millis(5));
    }
    for (index, player) in players.iter_mut().enumerate() {
        assert!(
            counts[index] >= duration * 15,
            "snapshot rate fell below 15 Hz"
        );
        assert!(max_x[index] - min_x[index] > 1.0, "actor never moved");
        player.send(ClientCommand::ClearDirectionalInput);
    }
    let mut host = players.remove(0);
    drop(players);
    drop(link);
    host.until(|s| s.remote_players.is_empty());
    // Fill the bounded chat history so these authenticated clients exhaust their
    // receive buffers promptly. They never read beyond Welcome. Healthy players
    // must keep receiving snapshots while stalled sockets are removed.
    let mut stalled = Vec::new();
    for _ in 0..7 {
        let mut stream = TcpStream::connect(&address)?;
        stream.set_read_timeout(Some(Duration::from_secs(5)))?;
        write_client_hello(
            &mut stream,
            &ClientHello {
                protocol: NET_PROTOCOL_VERSION,
                token: None,
            },
        )?;
        assert!(matches!(
            read_framed_server_message(&mut stream)?,
            ServerWireMessage::Welcome(_)
        ));
        stalled.push(stream);
    }
    host.until(|s| s.remote_players.len() == 7);
    for _ in 0..16 {
        write_framed_client_chat(&mut host.writer, &"network qualification ".repeat(70))?;
    }
    let stall_started = Instant::now();
    let mut previous = Instant::now();
    let mut worst_stall_gap = 0.0f64;
    loop {
        let snapshot = host.until(|_| true);
        worst_stall_gap = worst_stall_gap.max(previous.elapsed().as_secs_f64() * 1000.0);
        previous = Instant::now();
        assert!(
            worst_stall_gap < 1000.0,
            "slow readers froze the healthy client"
        );
        if snapshot.remote_players.is_empty() {
            break;
        }
        assert!(
            stall_started.elapsed() < Duration::from_secs(40),
            "stalled clients not evicted"
        );
    }
    let stall_phase_seconds = stall_started.elapsed().as_secs_f64();
    drop(stalled);
    drop(host);
    server.stop();
    assert!(!world.join(".session.lock").exists());
    let saved: serde_json::Value =
        serde_json::from_slice(&std::fs::read(world.join("server-state.json"))?)?;
    assert_eq!(saved["players"].as_object().unwrap().len(), 15);
    let mut restarted = Server::start(&root, &address);
    let resumed = Client::connect(&address, Some(reconnect_token));
    assert_eq!(resumed.welcome.server_id, server_id);
    resumed.until(|s| s.remote_players.is_empty() && !s.death_screen_active);
    drop(resumed);
    restarted.stop();
    gaps.sort_by(f64::total_cmp);
    delayed_gaps.sort_by(f64::total_cmp);
    let report = serde_json::json!({
        "status":"passed", "players":8, "wall_clock_soak_seconds":duration,
        "snapshots_per_player":counts, "direct_snapshot_gap_p95_ms":gaps[(gaps.len()-1)*95/100],
        "delayed_snapshot_gap_p95_ms":delayed_gaps[(delayed_gaps.len()-1)*95/100],
        "relay_delay_per_direction_ms":[80,100,120], "stalled_readers_evicted":7,
        "stalled_reader_phase_seconds":stall_phase_seconds,
        "worst_healthy_snapshot_gap_during_stall_ms":worst_stall_gap, "reconnect_after_restart":true,
        "limits":"Automated normal-inventory movement on flat terrain with spawning disabled. Ordered delayed TCP buffers, no packet loss or terminal rendering. Not a human co-op session or survival campaign."
    });
    if let Ok(path) = std::env::var("TERMCRAFT_NETWORK_REPORT") {
        std::fs::write(path, serde_json::to_vec_pretty(&report)?)?;
    }
    println!("{report}");
    std::fs::remove_dir_all(root)?;
    Ok(())
}

struct Mcp {
    process: Child,
    input: std::process::ChildStdin,
    output: io::BufReader<std::process::ChildStdout>,
    id: u32,
}
impl Mcp {
    fn start(root: &std::path::Path) -> Self {
        let mut process = Command::new(env!("CARGO_BIN_EXE_termcraft"))
            .arg("mcp")
            .env("TERMCRAFT_SAVE_DIR", root)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let input = process.stdin.take().unwrap();
        let output = io::BufReader::new(process.stdout.take().unwrap());
        Self {
            process,
            input,
            output,
            id: 0,
        }
    }
    fn call(&mut self, name: &str, args: serde_json::Value) -> serde_json::Value {
        self.id += 1;
        writeln!(self.input,"{}",serde_json::json!({"jsonrpc":"2.0","id":self.id,"method":"tools/call","params":{"name":name,"arguments":args}})).unwrap();
        self.input.flush().unwrap();
        let mut line = String::new();
        self.output.read_line(&mut line).unwrap();
        serde_json::from_str::<serde_json::Value>(&line).unwrap()["result"].clone()
    }
}
impl Drop for Mcp {
    fn drop(&mut self) {
        let _ = self.process.kill();
        let _ = self.process.wait();
    }
}

#[test]
#[ignore = "requires local TCP sockets and Unix process signals"]
fn live_game_master_uses_mcp_without_second_save_owner_and_rejects_stale_plans() -> io::Result<()> {
    use serde_json::json;
    let root = std::env::temp_dir().join(format!("termcraft-live-{:032x}", rand::random::<u128>()));
    std::fs::create_dir_all(&root)?;
    let mut recipe = WorldRecipe::example();
    recipe.terrain = Terrain::Plains;
    recipe.relief = 0;
    recipe.regions.clear();
    recipe.landmarks.clear();
    let world = WorldStore::new(root.join("worlds"))
        .create("tcp", "Live authoring regression", &recipe)
        .unwrap();
    let listener = TcpListener::bind("127.0.0.1:0")?;
    let addr = listener.local_addr()?.to_string();
    drop(listener);
    let mut server = Server::start(&root, &addr);
    let mut host = Client::connect(&addr, None);
    let mut guest = Client::connect(&addr, None);
    host.until(|s| s.remote_players.len() == 1);
    guest.until(|s| s.chat.mode == "host-only");
    write_framed_client_chat(&mut guest.writer, "/connect")?;
    thread::sleep(Duration::from_millis(120));
    assert!(!world.join(".director-bridge.json").exists());
    let mut mcp = Mcp::start(&root);
    assert_eq!(
        mcp.call("director_connect", json!({"world_id":"tcp"}))["isError"],
        true
    );
    write_framed_client_chat(&mut host.writer, "/connect")?;
    host.until(|s| s.chat.status.contains("enabled"));
    let connected = mcp.call("director_connect", json!({"world_id":"tcp"}));
    assert_eq!(connected["isError"], false, "{connected}");
    assert!(world.join(".session.lock").exists());
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            std::fs::metadata(world.join(".director-bridge.json"))?
                .permissions()
                .mode()
                & 0o777,
            0o600
        );
    }
    write_framed_client_chat(&mut host.writer, "Make this area snowy at dawn")?;
    let before = host.until(|s| s.chat.pending);
    host.until(|s| s.tick >= before.tick + 2);
    let pending =
        mcp.call("director_wait", json!({"timeout_seconds":1}))["structuredContent"]["pending"]
            .clone();
    assert_eq!(pending["request"]["prompt"], "Make this area snowy at dawn");
    let id = pending["request_id"].clone();
    let bad=mcp.call("director_apply",json!({"request_id":id,"plan":{"message":"Invalid","actions":[{"type":"spawn","mob":"zombie","count":255,"side":"east","distance":8}]}}));
    assert_eq!(bad["isError"], true);
    assert!(host.until(|s| s.tick > before.tick + 2).chat.pending);
    let accepted=mcp.call("director_apply",json!({"request_id":id,"plan":{"message":"Snow at dawn","actions":[{"type":"environment","terrain":"tundra","radius":8},{"type":"time","phase":"dawn"}]}}));
    assert_eq!(
        accepted["structuredContent"]["accepted"], true,
        "{accepted}"
    );
    host.until(|s| !s.chat.pending && s.chat.status.contains("Snow at dawn"));
    guest.until(|s| s.chat.status.contains("Snow at dawn"));
    let result = mcp.call("director_observe", json!({}));
    assert_eq!(
        result["structuredContent"]["last_result"]["status"],
        "applied"
    );
    assert_eq!(
        mcp.call("director_wait", json!({"timeout_seconds":1}))["structuredContent"]["timed_out"],
        true
    );
    assert_eq!(
        mcp.call(
            "director_apply",
            json!({"request_id":id,"plan":{"message":"Duplicate","actions":[]}})
        )["isError"],
        true
    );
    let next =
        mcp.call("director_request", json!({"prompt":"A battle"}))["structuredContent"]["pending"]
            ["request_id"]
            .clone();
    assert!(next.is_string());
    assert_eq!(mcp.call("director_cancel", json!({}))["isError"], false);
    assert_eq!(
        mcp.call(
            "director_apply",
            json!({"request_id":next,"plan":{"message":"Late","actions":[]}})
        )["isError"],
        true
    );
    write_framed_client_chat(&mut host.writer, "/disconnect")?;
    host.until(|s| s.chat.status.contains("disconnected"));
    assert!(!world.join(".director-bridge.json").exists());
    assert_eq!(mcp.call("director_observe", json!({}))["isError"], true);
    write_framed_client_chat(&mut host.writer, "/connect")?;
    host.until(|s| s.chat.status.contains("enabled"));
    assert_eq!(
        mcp.call("director_connect", json!({"world_id":"tcp"}))["isError"],
        false
    );
    assert_eq!(
        mcp.call("director_request", json!({"prompt":"Wait for the host"}))["isError"],
        false
    );
    drop(host);
    guest.until(|s| {
        s.chat.status.contains("Host disconnected")
            && !s.chat.pending
            && s.chat.mode == "request-only"
    });
    assert!(!world.join(".director-bridge.json").exists());
    assert_eq!(mcp.call("director_observe", json!({}))["isError"], true);
    drop(mcp);
    drop(guest);
    server.stop();
    assert!(world.join("environment.json").exists());
    std::fs::remove_dir_all(root)?;
    Ok(())
}
impl Server {
    fn start(root: &std::path::Path, addr: &str) -> Self {
        let log = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(root.join("server.log"))
            .unwrap();
        let child = Command::new(env!("CARGO_BIN_EXE_termcraft"))
            .args(["world", "serve", "tcp", addr])
            .env("TERMCRAFT_SAVE_DIR", root)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(log)
            .spawn()
            .unwrap();
        let mut server = Self(child);
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            if let Ok(stream) = TcpStream::connect(addr) {
                drop(stream);
                break;
            }
            assert!(
                server.0.try_wait().unwrap().is_none(),
                "server failed: {}",
                std::fs::read_to_string(root.join("server.log")).unwrap()
            );
            assert!(Instant::now() < deadline, "server startup timeout");
            thread::sleep(Duration::from_millis(20));
        }
        server
    }
    fn stop(&mut self) {
        assert!(
            Command::new("kill")
                .args(["-INT", &self.0.id().to_string()])
                .status()
                .unwrap()
                .success()
        );
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            if let Some(status) = self.0.try_wait().unwrap() {
                assert!(status.success());
                break;
            }
            assert!(Instant::now() < deadline, "graceful server stop timed out");
            thread::sleep(Duration::from_millis(20));
        }
    }
}
struct Client {
    writer: TcpStream,
    incoming: mpsc::Receiver<ServerWireMessage>,
    welcome: Welcome,
    sequence: u32,
}
impl Drop for Client {
    fn drop(&mut self) {
        let _ = self.writer.shutdown(Shutdown::Both);
    }
}
impl Client {
    fn connect(addr: &str, token: Option<String>) -> Self {
        let mut writer = TcpStream::connect(addr).unwrap();
        writer
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        write_client_hello(
            &mut writer,
            &ClientHello {
                protocol: NET_PROTOCOL_VERSION,
                token,
            },
        )
        .unwrap();
        let ServerWireMessage::Welcome(welcome) = read_framed_server_message(&mut writer).unwrap()
        else {
            panic!("no welcome")
        };
        writer.set_read_timeout(None).unwrap();
        let mut reader = writer.try_clone().unwrap();
        let (tx, incoming) = mpsc::channel();
        thread::spawn(move || {
            while let Ok(message) = read_framed_server_message(&mut reader) {
                if tx.send(message).is_err() {
                    break;
                }
            }
        });
        Self {
            writer,
            incoming,
            welcome,
            sequence: 0,
        }
    }
    fn send(&mut self, cmd: ClientCommand) {
        self.sequence += 1;
        // The server assigns the connection's identity, ignoring this forged ID.
        write_framed_client_input(
            &mut self.writer,
            &ClientInputFrame::new(65535, self.sequence, 0, cmd),
        )
        .unwrap();
    }
    #[track_caller]
    fn until(&self, predicate: impl Fn(&ServerSnapshot) -> bool) -> Box<ServerSnapshot> {
        let deadline = Instant::now() + Duration::from_secs(8);
        loop {
            let msg = self
                .incoming
                .recv_timeout(deadline.saturating_duration_since(Instant::now()))
                .expect("snapshot timeout");
            if let ServerWireMessage::Snapshot(snapshot) = msg
                && predicate(&snapshot)
            {
                return snapshot;
            }
        }
    }
}
fn count(s: &ServerSnapshot, item: ItemType) -> u32 {
    s.session
        .inventory()
        .slots
        .iter()
        .flatten()
        .filter(|s| s.item_type == item)
        .map(|s| s.count)
        .sum()
}
#[test]
#[ignore = "requires local TCP sockets and Unix process signals"]
fn two_socket_players_mine_build_disconnect_and_resume_after_restart() -> io::Result<()> {
    let root = std::env::temp_dir().join(format!("termcraft-tcp-{:032x}", rand::random::<u128>()));
    std::fs::create_dir_all(&root)?;
    let mut recipe = WorldRecipe::example();
    recipe.terrain = Terrain::Plains;
    recipe.relief = 0;
    recipe.regions.clear();
    recipe.landmarks.clear();
    let world = WorldStore::new(root.join("worlds"))
        .create("tcp", "TCP regression", &recipe)
        .unwrap();
    let mut state = GameState::open_in_directory(&world)?;
    state.apply_client_command(ClientCommand::ToggleRuleMobSpawning);
    state
        .world
        .set_block(2, 63, termcraft::world::block::BlockType::Wood);
    state.world.try_save_all()?;
    state.try_save_progression()?;
    drop(state);
    let listener = TcpListener::bind("127.0.0.1:0")?;
    let addr = listener.local_addr()?.to_string();
    drop(listener);
    let mut server = Server::start(&root, &addr);
    let mut a = Client::connect(&addr, None);
    let mut b = Client::connect(&addr, None);
    assert_ne!(a.welcome.token, b.welcome.token);
    let first = a.until(|s| s.remote_players.len() == 1);
    b.until(|s| s.remote_players.len() == 1);
    assert!(
        first.player.x.abs() < 2.0,
        "unexpected fixture spawn {}",
        first.player.x
    );
    for c in [&mut a, &mut b] {
        c.send(ClientCommand::AimAt(2, 63));
        c.send(ClientCommand::SetPrimaryAction(true));
    }
    let deadline = Instant::now() + Duration::from_secs(4);
    while Instant::now() < deadline {
        thread::sleep(Duration::from_millis(200));
        for c in [&mut a, &mut b] {
            c.send(ClientCommand::AimAt(2, 63));
        }
    }
    for c in [&mut a, &mut b] {
        c.send(ClientCommand::SetPrimaryAction(false));
    }
    a.send(ClientCommand::SetMoveRight(true));
    a.until(|s| s.player.x >= 0.5);
    a.send(ClientCommand::ClearDirectionalInput);
    let collected = a.until(|s| count(s, ItemType::Wood) == 1);
    let other = b.until(|s| s.tick >= collected.tick);
    assert_eq!(count(&other, ItemType::Wood), 0);
    let recipe = Recipe::all()
        .iter()
        .position(|r| r.result == ItemType::Planks)
        .unwrap();
    a.send(ClientCommand::CraftRecipe(recipe as u16));
    a.until(|s| count(s, ItemType::Planks) == 4);
    a.send(ClientCommand::UseAt(5, 63));
    a.until(|s| count(s, ItemType::Planks) == 3);
    let token = a.welcome.token.clone();
    let server_id = a.welcome.server_id.clone();
    drop(a);
    b.until(|s| s.remote_players.is_empty());
    let a = Client::connect(&addr, Some(token.clone()));
    a.until(|s| count(s, ItemType::Planks) == 3);
    drop(a);
    drop(b);
    server.stop();
    assert!(!world.join(".session.lock").exists());
    let reopened = GameState::open_in_directory(&world)?;
    assert_eq!(
        reopened.world.get_block(5, 63),
        termcraft::world::block::BlockType::Planks
    );
    drop(reopened);
    let mut restarted = Server::start(&root, &addr);
    let a = Client::connect(&addr, Some(token));
    assert_eq!(a.welcome.server_id, server_id);
    a.until(|s| count(s, ItemType::Planks) == 3);
    drop(a);
    restarted.stop();
    std::fs::remove_dir_all(root)?;
    Ok(())
}
