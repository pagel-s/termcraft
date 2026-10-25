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
    for (protocol, expected) in [
        (NET_PROTOCOL_VERSION, "server is full"),
        (NET_PROTOCOL_VERSION - 1, "unsupported protocol"),
    ] {
        let mut extra = TcpStream::connect(&address)?;
        extra.set_read_timeout(Some(Duration::from_secs(5)))?;
        // Encode the old-version Hello directly; the regular client encoder
        // intentionally refuses to generate an unsupported protocol.
        let hello = bincode::serialize(&ClientHello {
            protocol,
            token: None,
        })
        .unwrap();
        extra.write_all(&(hello.len() as u32).to_be_bytes())?;
        extra.write_all(&hello)?;
        assert!(matches!(read_framed_server_message(&mut extra)?,
            ServerWireMessage::Rejected(_, reason) if reason.contains(expected)));
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
    // Fill all 32 history slots using four legal messages from each of the
    // eight authenticated players, within every sender's burst allowance.
    // A single sender can no longer fill the history by flooding large prompts.
    for writer in std::iter::once(&mut host.writer).chain(stalled.iter_mut()) {
        for _ in 0..4 {
            write_framed_client_chat(writer, &"λ".repeat(280))?;
        }
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
            "slow readers froze the healthy client: worst gap {worst_stall_gap:.1} ms, tick {}",
            snapshot.tick
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

#[test]
#[ignore = "requires local TCP sockets"]
fn cli_join_explains_closed_connections_and_mismatched_builds_before_terminal_setup()
-> io::Result<()> {
    let root = std::env::temp_dir().join(format!(
        "termcraft-join-errors-{:032x}",
        rand::random::<u128>()
    ));
    std::fs::create_dir_all(&root)?;
    for (old_protocol, expected) in [
        (false, "closed the connection while joining"),
        (true, "builds do not match"),
    ] {
        let listener = TcpListener::bind("127.0.0.1:0")?;
        let address = listener.local_addr()?.to_string();
        let handler = thread::spawn(move || -> io::Result<()> {
            let (mut stream, _) = listener.accept()?;
            stream.set_read_timeout(Some(Duration::from_secs(5)))?;
            let mut length = [0; 4];
            stream.read_exact(&mut length)?;
            let length = u32::from_be_bytes(length) as usize;
            assert!(length < 8192);
            stream.read_exact(&mut vec![0; length])?;
            if old_protocol {
                let response = bincode::serialize(&ServerWireMessage::Rejected(
                    NET_PROTOCOL_VERSION - 1,
                    "old server".into(),
                ))
                .unwrap();
                stream.write_all(&(response.len() as u32).to_be_bytes())?;
                stream.write_all(&response)?;
            }
            stream.shutdown(Shutdown::Both)
        });
        let result = Command::new(env!("CARGO_BIN_EXE_termcraft"))
            .args(["client", &address, "--player", "join-errors"])
            .env("TERMCRAFT_SAVE_DIR", &root)
            .output()?;
        handler.join().unwrap()?;
        assert!(!result.status.success());
        let error = String::from_utf8_lossy(&result.stderr);
        assert!(error.contains(expected), "{error}");
        assert!(!error.contains("Os {"), "raw OS debug output: {error}");
        assert!(
            !result
                .stdout
                .windows(8)
                .any(|bytes| bytes == b"\x1b[?1049h"),
            "failed join entered alternate screen"
        );
    }
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
    let mut server = Server::start_with_env(
        &root,
        &addr,
        &[("TERMCRAFT_EXPERIMENTAL_GAME_MASTER", "1".into())],
    );
    let mut host = Client::connect(&addr, None);
    let mut guest = Client::connect(&addr, None);
    host.until(|s| s.remote_players.len() == 1);
    guest.until(|s| s.chat.player_id() == Some(guest.welcome.client_id));
    write_framed_client_chat(&mut guest.writer, "/connect")?;
    thread::sleep(Duration::from_millis(120));
    assert!(!world.join(".director-bridge.json").exists());
    write_framed_client_chat(&mut guest.writer, "Hello")?;
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
    write_framed_client_chat(&mut host.writer, "/gm Make this area snowy at dawn")?;
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
            && s.chat.player_id() == Some(guest.welcome.client_id)
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
        Self::start_with_env(root, addr, &[])
    }
    fn start_with_env(root: &std::path::Path, addr: &str, env: &[(&str, String)]) -> Self {
        let log = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(root.join("server.log"))
            .unwrap();
        let child = Command::new(env!("CARGO_BIN_EXE_termcraft"))
            .args(["world", "serve", "tcp", addr])
            .env("TERMCRAFT_SAVE_DIR", root)
            .env_remove("TERMCRAFT_EXPERIMENTAL_GAME_MASTER")
            .env_remove("TERMCRAFT_DIRECTOR")
            .envs(env.iter().map(|(key, value)| (*key, value)))
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
    dimension: std::cell::Cell<SnapshotDimension>,
    writer: TcpStream,
    incoming: mpsc::Receiver<ServerWireMessage>,
    welcome: Welcome,
    sequence: u32,
}

#[test]
#[ignore = "requires real loopback TCP and Unix process signals"]
fn real_players_take_melee_and_bow_damage_and_host_can_disable_pvp() -> io::Result<()> {
    use termcraft::{
        engine::{
            server_save::{ServerSave, token},
            state::PlayerSession,
        },
        world::item::ItemStack,
    };
    let root = std::env::temp_dir().join(format!("termcraft-pvp-{}", token()));
    std::fs::create_dir_all(&root)?;
    let mut recipe = WorldRecipe::example();
    recipe.terrain = Terrain::Plains;
    recipe.relief = 0;
    recipe.regions.clear();
    recipe.landmarks.clear();
    let world = WorldStore::new(root.join("worlds"))
        .create("tcp", "PvP regression", &recipe)
        .unwrap();
    let mut state = GameState::open_in_directory(&world)?;
    state.apply_client_command(ClientCommand::ToggleRuleMobSpawning);
    state.player.x = 0.5;
    state.player.y = 64.0;
    state.inventory.slots.fill(None);
    for (i, item, count) in [
        (0, ItemType::DiamondSword, 1),
        (1, ItemType::Bow, 1),
        (2, ItemType::Arrow, 32),
    ] {
        state.inventory.slots[i] = Some(ItemStack {
            item_type: item,
            count,
            durability: item.max_durability(),
        });
    }
    let mut saved = ServerSave::read(&state)?;
    let (a, b) = (token(), token());
    saved
        .players
        .insert(a.clone(), PlayerSession::capture(&state));
    state.player.x = 2.0;
    state.inventory.slots.fill(None);
    saved
        .players
        .insert(b.clone(), PlayerSession::capture(&state));
    state.try_persist_world_and_progress()?;
    saved.write(&world)?;
    drop(state);
    let reservation = TcpListener::bind("127.0.0.1:0")?;
    let address = reservation.local_addr()?.to_string();
    drop(reservation);
    let mut server = Server::start(&root, &address);
    let mut attacker = Client::connect(&address, Some(a));
    let defender = Client::connect(&address, Some(b));
    attacker.until(|s| s.remote_players.len() == 1);
    attacker.send(ClientCommand::AimAt(2, 63));
    attacker.send(ClientCommand::SetPrimaryAction(true));
    let melee = defender.until(|s| s.player.health < 20.0);
    attacker.send(ClientCommand::SetPrimaryAction(false));
    let target = defender.until(|s| s.tick >= melee.tick + 14);
    attacker.send(ClientCommand::SelectHotbarSlot(1));
    attacker.send(ClientCommand::AimAt(
        target.player.x.floor() as i32,
        (target.player.y - 1.5).floor() as i32,
    ));
    attacker.send(ClientCommand::SetPrimaryAction(true));
    attacker.until(|s| s.tick >= target.tick + 18);
    attacker.send(ClientCommand::SetPrimaryAction(false));
    let arrow = defender.until(|s| s.player.health < melee.player.health);
    assert!(arrow.player.health > 0.0);
    attacker.send(ClientCommand::TogglePvp);
    let disabled = attacker.until(|s| serde_json::to_value(&s.settings).unwrap()["pvp"] == false);
    attacker.send(ClientCommand::AimAt(
        arrow.player.x.floor() as i32,
        (arrow.player.y - 1.5).floor() as i32,
    ));
    attacker.send(ClientCommand::SetPrimaryAction(true));
    attacker.until(|s| s.tick >= disabled.tick + 18);
    attacker.send(ClientCommand::SetPrimaryAction(false));
    let after = defender.until(|s| s.tick >= disabled.tick + 32);
    assert!(
        after.player.health >= arrow.player.health,
        "PvP-off still dealt damage; natural food regeneration may raise health"
    );
    drop((attacker, defender));
    server.stop();
    let saved: serde_json::Value =
        serde_json::from_slice(&std::fs::read(world.join("server-state.json"))?)?;
    assert_eq!(saved["settings"]["pvp"], false);
    std::fs::remove_dir_all(root)?;
    Ok(())
}

#[test]
#[ignore = "requires local TCP sockets and Unix process signals"]
fn interrupting_server_startup_releases_the_world_lease() -> io::Result<()> {
    let root =
        std::env::temp_dir().join(format!("termcraft-startup-{:032x}", rand::random::<u128>()));
    std::fs::create_dir_all(&root)?;
    let log = std::fs::File::create(root.join("startup.log"))?;
    let mut process = Command::new(env!("CARGO_BIN_EXE_termcraft"))
        .args(["server", "127.0.0.1:0"])
        .env("TERMCRAFT_SAVE_DIR", &root)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(log)
        .spawn()?;
    let lock = root.join("multiplayer/default/.session.lock");
    let deadline = Instant::now() + Duration::from_secs(10);
    while !lock.exists() {
        assert!(
            process.try_wait()?.is_none(),
            "server exited before acquiring its lease"
        );
        assert!(Instant::now() < deadline, "startup lease timeout");
        thread::sleep(Duration::from_millis(1));
    }
    assert!(
        Command::new("kill")
            .args(["-INT", &process.id().to_string()])
            .status()?
            .success()
    );
    loop {
        if let Some(status) = process.try_wait()? {
            assert!(status.success(), "startup interruption failed: {status}");
            break;
        }
        if Instant::now() >= deadline {
            let _ = process.kill();
            let _ = process.wait();
            panic!("server did not finish cancelling startup");
        }
        thread::sleep(Duration::from_millis(10));
    }
    assert!(!lock.exists(), "cancelled startup left a stale lease");
    let text = std::fs::read_to_string(root.join("startup.log"))?;
    assert!(
        !text.contains("Starting headless server"),
        "fixture missed startup cancellation"
    );
    std::fs::remove_dir_all(root)?;
    Ok(())
}

#[test]
#[ignore = "requires local TCP sockets and Unix process signals"]
fn players_chat_across_dimensions_without_enabling_experimental_editing() -> io::Result<()> {
    use termcraft::{
        engine::{
            server_save::{ServerSave, token},
            state::PlayerSession,
        },
        world::{Dimension, World, block::BlockType},
    };
    let root = std::env::temp_dir().join(format!("termcraft-chat-{:032x}", rand::random::<u128>()));
    std::fs::create_dir_all(&root)?;
    let mut recipe = WorldRecipe::example();
    recipe.terrain = Terrain::Plains;
    recipe.relief = 0;
    recipe.regions.clear();
    recipe.landmarks.clear();
    let world = WorldStore::new(root.join("worlds"))
        .create("tcp", "Player chat regression", &recipe)
        .unwrap();
    let mut terrain = World::open_in_directory(Dimension::Nether, &world)?;
    terrain.load_chunks_around(0);
    for x in -8..=8 {
        for y in 52..=64 {
            terrain.set_block(
                x,
                y,
                if y == 64 {
                    BlockType::StoneBricks
                } else {
                    BlockType::Air
                },
            );
        }
    }
    terrain.try_save_all()?;
    drop(terrain);
    let mut state = GameState::open_in_directory(&world)?;
    state.apply_client_command(ClientCommand::ToggleRuleMobSpawning);
    let guest_token = token();
    let mut saved = ServerSave::read(&state)?;
    saved
        .players
        .insert(guest_token.clone(), PlayerSession::capture(&state));
    saved
        .dimensions
        .insert(guest_token.clone(), Dimension::Nether);
    state.try_persist_world_and_progress()?;
    saved.write(&world)?;
    drop(state);
    // Previously saved experimental preferences must not enable the feature.
    std::fs::write(world.join("director.json"), br#"{"proactive":"combat"}"#)?;
    let reservation = TcpListener::bind("127.0.0.1:0")?;
    let address = reservation.local_addr()?.to_string();
    drop(reservation);
    let mut server = Server::start(&root, &address);
    let mut host = Client::connect(&address, None);
    let mut guest = Client::connect(&address, Some(guest_token));
    host.until(|s| s.dimension == SnapshotDimension::Overworld);
    guest.until(|s| s.dimension == SnapshotDimension::Nether);
    let host_id = host.welcome.client_id;
    let guest_id = guest.welcome.client_id;
    write_framed_client_chat(&mut guest.writer, "Hello from the guest!")?;
    let expected = format!("P{guest_id}: Hello from the guest!");
    host.until(|s| s.chat.status == expected);
    let received = guest.until(|s| s.chat.status == expected);
    assert_eq!(received.chat.player_id(), Some(guest_id));
    write_framed_client_chat(&mut host.writer, "Can you hear me in the Nether?")?;
    let expected = format!("P{host_id}: Can you hear me in the Nether?");
    guest.until(|s| s.chat.status == expected);
    write_framed_client_chat(&mut host.writer, "/connect")?;
    host.until(|s| s.chat.status.contains("Unknown chat command"));
    write_framed_client_chat(&mut host.writer, "Make it rain")?;
    let expected = format!("P{host_id}: Make it rain");
    let seen = host.until(|s| s.chat.status == expected);
    assert!(!seen.chat.pending);
    assert!(seen.chat.lines.iter().all(|line| line.starts_with('P')));
    assert!(!world.join(".director-bridge.json").exists());
    drop(host);
    write_framed_client_chat(&mut guest.writer, "Still here")?;
    let expected = format!("P{guest_id}: Still here");
    guest.until(|s| s.chat.status == expected);
    drop(guest);
    server.stop();
    assert_eq!(
        std::fs::read(world.join("director.json"))?,
        br#"{"proactive":"combat"}"#
    );
    std::fs::remove_dir_all(root)?;
    Ok(())
}

#[cfg(target_os = "linux")]
#[test]
#[ignore = "requires real TCP, C compiler, LD_PRELOAD and SIGKILL; disposable hosted worlds"]
fn interrupted_hosted_checkpoint_keeps_three_players_and_dimensions_together() -> io::Result<()> {
    use termcraft::{
        engine::{
            server_save::{ServerSave, token},
            state::PlayerSession,
        },
        world::{Dimension, World, block::BlockType, item::ItemStack},
    };
    let root = std::env::temp_dir().join(format!("termcraft-storage-tcp-{}", token()));
    std::fs::create_dir(&root)?;
    let library = root.join("faults.so");
    assert!(
        Command::new("cc")
            .args([
                "-std=c11", "-shared", "-fPIC", "-O2", "-Wall", "-Wextra", "-Werror", "-pthread"
            ])
            .arg(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/scripts/storage_faults.c"
            ))
            .args(["-ldl", "-o"])
            .arg(&library)
            .status()?
            .success()
    );
    let dimensions = [Dimension::Overworld, Dimension::Nether, Dimension::End];
    let snapshots = [
        SnapshotDimension::Overworld,
        SnapshotDimension::Nether,
        SnapshotDimension::End,
    ];
    let mut results = Vec::new();
    for (index, (selector, mode, committed)) in [
        ("chunks", "freeze-after", false),
        ("server", "full", false),
        ("server", "freeze-before", false),
        ("server", "freeze-after", false),
        ("commit", "freeze-before", false),
        ("commit", "freeze-after", true),
    ]
    .into_iter()
    .enumerate()
    {
        let saves = root.join(format!("case-{index}"));
        let mut recipe = WorldRecipe::example();
        recipe.terrain = Terrain::Plains;
        recipe.relief = 0;
        recipe.regions.clear();
        recipe.landmarks.clear();
        let world = WorldStore::new(saves.join("worlds"))
            .create("tcp", "Storage fault fixture", &recipe)
            .unwrap();
        for dimension in dimensions {
            let mut terrain = World::open_in_directory(dimension, &world)?;
            terrain.load_chunks_around(0);
            for x in -8..=8 {
                for y in 52..=64 {
                    terrain.set_block(
                        x,
                        y,
                        if y == 64 {
                            BlockType::StoneBricks
                        } else {
                            BlockType::Air
                        },
                    );
                }
            }
            terrain.try_save_all()?;
        }
        let mut state = GameState::open_in_directory(&world)?;
        state.apply_client_command(ClientCommand::ToggleRuleMobSpawning);
        state.player.x = 0.5;
        state.player.y = 64.0;
        state.inventory.slots.fill(None);
        state.inventory.slots[0] = Some(ItemStack {
            item_type: ItemType::Cobblestone,
            count: 32,
            durability: None,
        });
        let tokens = [token(), token(), token()];
        let mut saved = ServerSave::read(&state)?;
        for (identity, dimension) in tokens.iter().zip(dimensions) {
            saved
                .players
                .insert(identity.clone(), PlayerSession::capture(&state));
            saved.dimensions.insert(identity.clone(), dimension);
        }
        state.try_persist_world_and_progress()?;
        saved.write(&world)?;
        drop(state);
        let probe = TcpListener::bind("127.0.0.1:0")?;
        let addr = probe.local_addr()?.to_string();
        drop(probe);
        let control = saves.join("fault-mode");
        let marker = saves.join("fault-hit");
        std::fs::write(&control, "off")?;
        let mut server = Server::start_with_env(
            &saves,
            &addr,
            &[
                ("LD_PRELOAD", library.display().to_string()),
                ("TERMCRAFT_TEST_FAULT_ROOT", world.display().to_string()),
                (
                    "TERMCRAFT_TEST_FAULT_CONTROL",
                    control.display().to_string(),
                ),
                ("TERMCRAFT_TEST_FAULT_MARKER", marker.display().to_string()),
                ("TERMCRAFT_TEST_FAULT_FILE", selector.into()),
            ],
        );
        let mut players: Vec<_> = tokens
            .iter()
            .map(|t| Client::connect(&addr, Some(t.clone())))
            .collect();
        for (player, dimension) in players.iter().zip(snapshots) {
            player.until(|s| s.dimension == dimension && count(s, ItemType::Cobblestone) == 32);
        }
        for player in &mut players {
            player.send(ClientCommand::UseAt(3, 63));
            player.until(|s| count(s, ItemType::Cobblestone) == 31);
        }
        std::fs::write(&control, mode)?;
        drop(players); // Real disconnects trigger the normal shared save path.
        let deadline = Instant::now() + Duration::from_secs(12);
        while !marker.exists() {
            assert!(
                server.0.try_wait()?.is_none(),
                "server exited before fault gate"
            );
            assert!(
                Instant::now() < deadline,
                "fault gate timeout: {selector}/{mode}"
            );
            thread::sleep(Duration::from_millis(5));
        }
        let pid = server.0.id();
        server.0.kill()?;
        let status = server.0.wait()?;
        use std::os::unix::process::ExitStatusExt;
        assert_eq!(status.signal(), Some(9));
        // Only this test's dead process can have its stale ownership lease removed.
        assert_eq!(
            std::fs::read_to_string(world.join(".session.lock"))?.trim(),
            pid.to_string()
        );
        std::fs::remove_file(world.join(".session.lock"))?;
        for _ in 0..2 {
            let mut server = Server::start(&saves, &addr);
            let players: Vec<_> = tokens
                .iter()
                .map(|t| Client::connect(&addr, Some(t.clone())))
                .collect();
            for (player, dimension) in players.iter().zip(snapshots) {
                player.until(|s| {
                    s.dimension == dimension
                        && count(s, ItemType::Cobblestone) == if committed { 31 } else { 32 }
                });
            }
            server.stop();
            drop(players);
            for dimension in dimensions {
                let mut terrain = World::open_in_directory(dimension, &world)?;
                terrain.load_chunks_around(0);
                assert_eq!(
                    terrain.get_block(3, 63),
                    if committed {
                        BlockType::Cobblestone
                    } else {
                        BlockType::Air
                    }
                );
            }
        }
        results.push(serde_json::json!({"selector":selector,"fault":mode,"outcome":if committed {"new"} else {"previous"},"status":"passed"}));
    }
    if let Some(report) = std::env::var_os("TERMCRAFT_HOSTED_STORAGE_REPORT") {
        std::fs::write(
            report,
            serde_json::to_vec_pretty(
                &serde_json::json!({"status":"passed","cases":results,"players":3,"dimensions":3,"restarts_per_case":2,"transport":"real TCP","termination":"SIGKILL","actions":"normal UseAt placement; staged inventories and floor"}),
            )?,
        )?;
    }
    std::fs::remove_dir_all(root)
}
impl Drop for Client {
    fn drop(&mut self) {
        let _ = self.writer.shutdown(Shutdown::Both);
    }
}
impl Client {
    fn connect(addr: &str, token: Option<String>) -> Self {
        let deadline = Instant::now() + Duration::from_secs(5);
        let (writer, welcome) = loop {
            let mut writer = TcpStream::connect(addr).unwrap();
            writer
                .set_read_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            write_client_hello(
                &mut writer,
                &ClientHello {
                    protocol: NET_PROTOCOL_VERSION,
                    token: token.clone(),
                },
            )
            .unwrap();
            match read_framed_server_message(&mut writer).unwrap() {
                ServerWireMessage::Welcome(welcome) => break (writer, welcome),
                // An immediate reconnect may reach the Hello queue before the
                // previous reader thread reports its closed socket. Wait only
                // for that explicit transient condition, never an auth failure.
                ServerWireMessage::Rejected(_, reason)
                    if token.is_some()
                        && reason == "this player is already connected; use another profile"
                        && Instant::now() < deadline =>
                {
                    eprintln!("Waiting for disconnected profile cleanup before reconnect");
                    let _ = writer.shutdown(Shutdown::Both);
                    thread::sleep(Duration::from_millis(10));
                }
                other => panic!("expected welcome, received {other:?}"),
            }
        };
        writer.set_read_timeout(None).unwrap();
        let mut reader = writer.try_clone().unwrap();
        let (tx, incoming) = mpsc::channel();
        thread::spawn(move || {
            loop {
                match read_framed_server_message(&mut reader) {
                    Ok(message) => {
                        if tx.send(message).is_err() {
                            break;
                        }
                    }
                    Err(error) => {
                        eprintln!("Client snapshot reader stopped: {error}");
                        break;
                    }
                }
            }
        });
        Self {
            dimension: std::cell::Cell::new(SnapshotDimension::Overworld),
            writer,
            incoming,
            welcome,
            sequence: 0,
        }
    }
    fn send(&mut self, cmd: ClientCommand) {
        self.sequence += 1;
        // The server assigns the connection's identity, ignoring this forged ID.
        let mut frame = ClientInputFrame::new(65535, self.sequence, 0, cmd);
        frame.dimension = self.dimension.get();
        write_framed_client_input(&mut self.writer, &frame).unwrap();
    }
    #[track_caller]
    fn until(&self, predicate: impl Fn(&ServerSnapshot) -> bool) -> Box<ServerSnapshot> {
        let deadline = Instant::now() + Duration::from_secs(8);
        let caller = std::panic::Location::caller();
        let mut last = String::from("no snapshot received");
        loop {
            let msg = self
                .incoming
                .recv_timeout(deadline.saturating_duration_since(Instant::now()))
                .unwrap_or_else(|error| panic!("snapshot timeout at {caller}: {error}; {last}"));
            if let ServerWireMessage::Snapshot(snapshot) = msg {
                self.dimension.set(snapshot.dimension);
                if predicate(&snapshot) {
                    return snapshot;
                }
                last = format!(
                    "tick {}, player ({:.3},{:.3}), wood {}, planks {}, ack {}",
                    snapshot.tick,
                    snapshot.player.x,
                    snapshot.player.y,
                    count(&snapshot, ItemType::Wood),
                    count(&snapshot, ItemType::Planks),
                    snapshot.acknowledged_sequence
                );
            }
        }
    }
    fn terrain_until(&self, predicate: impl Fn(&ChunkDeltaPacket) -> bool) {
        let deadline = Instant::now() + Duration::from_secs(8);
        loop {
            let message = self
                .incoming
                .recv_timeout(deadline.saturating_duration_since(Instant::now()))
                .expect("terrain stream timeout");
            match message {
                ServerWireMessage::ChunkDelta(delta) if predicate(&delta) => return,
                ServerWireMessage::Snapshot(s) => self.dimension.set(s.dimension),
                _ => {}
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
fn source_height_pool_replicates_reopens_and_drains_for_both_players() -> io::Result<()> {
    use termcraft::{
        engine::{
            server_save::{ServerSave, token},
            state::PlayerSession,
        },
        world::{
            block::BlockType,
            chunk::{CHUNK_HEIGHT, CHUNK_WIDTH},
            item::ItemStack,
        },
    };
    let root = std::env::temp_dir().join(format!("termcraft-water-tcp-{}", token()));
    std::fs::create_dir_all(&root)?;
    let mut recipe = WorldRecipe::example();
    recipe.terrain = Terrain::Plains;
    recipe.relief = 0;
    recipe.regions.clear();
    recipe.landmarks.clear();
    let world = WorldStore::new(root.join("worlds"))
        .create("tcp", "Water transport fixture", &recipe)
        .unwrap();
    let mut state = GameState::open_in_directory(&world)?;
    state.apply_client_command(ClientCommand::ToggleRuleMobSpawning);
    for cx in 0..=1 {
        state
            .world
            .apply_chunk_column_snapshot(cx, &vec![BlockType::Air; CHUNK_WIDTH * CHUNK_HEIGHT]);
    }
    for x in 28..=38 {
        state.world.set_block(x, 30, BlockType::Stone);
    }
    for y in 21..30 {
        for x in [28, 38] {
            state.world.set_block(x, y, BlockType::Stone);
        }
    }
    state.world.set_block(29, 26, BlockType::Water(8));
    state.player.x = 29.5;
    state.player.y = 30.0;
    state.inventory.slots.fill(None);
    state.inventory.slots[0] = Some(ItemStack {
        item_type: ItemType::Bucket,
        count: 1,
        durability: None,
    });
    let mut saved = ServerSave::read(&state)?;
    let (token_a, token_b) = (token(), token());
    saved
        .players
        .insert(token_a.clone(), PlayerSession::capture(&state));
    state.player.x = 35.5;
    saved
        .players
        .insert(token_b.clone(), PlayerSession::capture(&state));
    state.try_persist_world_and_progress()?;
    saved.write(&world)?;
    drop(state);
    let probe = TcpListener::bind("127.0.0.1:0")?;
    let addr = probe.local_addr()?.to_string();
    drop(probe);
    let at = |packet: &ChunkDeltaPacket, expected| {
        packet
            .chunks
            .iter()
            .any(|c| c.chunk_x == 1 && c.blocks[26 * CHUNK_WIDTH + 2] == expected)
    };
    let mut server = Server::start(&root, &addr);
    let a = Client::connect(&addr, Some(token_a.clone()));
    let b = Client::connect(&addr, Some(token_b.clone()));
    a.terrain_until(|p| at(p, BlockType::Water(9)));
    b.terrain_until(|p| at(p, BlockType::Water(9)));
    drop((a, b));
    server.stop();
    let reopened = GameState::open_in_directory(&world)?;
    assert_eq!(reopened.world.get_block(34, 26), BlockType::Water(9));
    drop(reopened);
    let mut server = Server::start(&root, &addr);
    let mut a = Client::connect(&addr, Some(token_a));
    let b = Client::connect(&addr, Some(token_b));
    a.terrain_until(|p| at(p, BlockType::Water(9)));
    b.terrain_until(|p| at(p, BlockType::Water(9)));
    a.send(ClientCommand::UseAt(29, 26));
    a.until(|s| count(s, ItemType::WaterBucket) == 1);
    a.terrain_until(|p| at(p, BlockType::Air));
    b.terrain_until(|p| at(p, BlockType::Air));
    drop((a, b));
    server.stop();
    let reopened = GameState::open_in_directory(&world)?;
    assert_eq!(reopened.world.get_block(34, 26), BlockType::Air);
    drop(reopened);
    std::fs::remove_dir_all(root)?;
    Ok(())
}

#[test]
#[ignore = "requires local TCP sockets and Unix process signals"]
fn portals_stream_separate_worlds_and_restore_dimensions_after_restart() -> io::Result<()> {
    use termcraft::{
        engine::{
            server_save::{ServerSave, token},
            state::PlayerSession,
        },
        world::{Dimension, block::BlockType, item::ItemStack},
    };
    let root = std::env::temp_dir().join(format!("termcraft-portals-tcp-{}", token()));
    std::fs::create_dir_all(&root)?;
    let mut recipe = WorldRecipe::example();
    recipe.terrain = Terrain::Plains;
    recipe.relief = 0;
    recipe.regions.clear();
    recipe.landmarks.clear();
    let world = WorldStore::new(root.join("worlds"))
        .create("tcp", "Portal transport fixture", &recipe)
        .unwrap();
    let mut state = GameState::open_in_directory(&world)?;
    state.apply_client_command(ClientCommand::ToggleRuleMobSpawning);
    for x in -1..=2 {
        for y in 60..=64 {
            state.world.set_block(
                x,
                y,
                if x == -1 || x == 2 || y == 60 || y == 64 {
                    BlockType::Obsidian
                } else {
                    BlockType::Air
                },
            );
        }
    }
    state.world.set_block(6, 63, BlockType::Glowstone);
    state.player.x = 0.5;
    state.player.y = 64.0;
    state.inventory.slots.fill(None);
    state.inventory.slots[0] = Some(ItemStack {
        item_type: ItemType::FlintAndSteel,
        count: 1,
        durability: Some(65),
    });
    let mut saved = ServerSave::read(&state)?;
    let (token_a, token_b) = (token(), token());
    saved
        .players
        .insert(token_a.clone(), PlayerSession::capture(&state));
    state.inventory.slots.fill(None);
    state.player.x = 3.5;
    saved
        .players
        .insert(token_b.clone(), PlayerSession::capture(&state));
    state.try_persist_world_and_progress()?;
    saved.write(&world)?;
    drop(state);
    let probe = TcpListener::bind("127.0.0.1:0")?;
    let addr = probe.local_addr()?.to_string();
    drop(probe);
    let mut server = Server::start(&root, &addr);
    let mut a = Client::connect(&addr, Some(token_a.clone()));
    let mut b = Client::connect(&addr, Some(token_b.clone()));
    a.until(|s| s.dimension == SnapshotDimension::Overworld && s.remote_players.len() == 1);
    b.until(|s| s.dimension == SnapshotDimension::Overworld && s.remote_players.len() == 1);
    a.send(ClientCommand::UseAt(-1, 62));
    let ignition = a.sequence;
    a.until(|s| s.acknowledged_sequence >= ignition);
    a.terrain_until(|p| {
        p.dimension == SnapshotDimension::Overworld
            && p.chunks
                .iter()
                .any(|c| c.blocks.contains(&BlockType::NetherPortal))
    });
    a.send(ClientCommand::UseAt(0, 62));
    let arrived =
        a.until(|s| s.dimension == SnapshotDimension::Nether && s.remote_players.is_empty());
    let base_y = arrived.player.y.ceil() as i32;
    a.terrain_until(|p| {
        p.dimension == SnapshotDimension::Nether
            && p.chunks
                .iter()
                .any(|c| c.blocks.contains(&BlockType::NetherPortal))
    });
    b.until(|s| s.dimension == SnapshotDimension::Overworld && s.remote_players.is_empty());
    drop(a);
    let mut a = Client::connect(&addr, Some(token_a.clone()));
    a.until(|s| s.dimension == SnapshotDimension::Nether);
    b.send(ClientCommand::UseAt(0, 62));
    b.until(|s| s.dimension == SnapshotDimension::Nether && s.remote_players.len() == 1);
    a.until(|s| {
        s.dimension == SnapshotDimension::Nether
            && s.remote_players.len() == 1
            && s.tick > arrived.tick + 45
    });
    a.send(ClientCommand::SetMoveLeft(true));
    a.until(|s| s.player.x <= 3.8);
    a.send(ClientCommand::ClearDirectionalInput);
    a.send(ClientCommand::UseAt(0, base_y - 2));
    a.until(|s| s.dimension == SnapshotDimension::Overworld && s.remote_players.is_empty());
    a.terrain_until(|p| {
        p.dimension == SnapshotDimension::Overworld
            && p.chunks.iter().any(|c| {
                c.blocks.contains(&BlockType::Glowstone)
                    && c.blocks.contains(&BlockType::NetherPortal)
            })
    });
    b.until(|s| s.dimension == SnapshotDimension::Nether && s.remote_players.is_empty());
    drop(a);
    drop(b);
    server.stop();
    let mut restarted = Server::start(&root, &addr);
    let a = Client::connect(&addr, Some(token_a));
    let b = Client::connect(&addr, Some(token_b));
    a.until(|s| s.dimension == SnapshotDimension::Overworld && s.remote_players.is_empty());
    b.until(|s| s.dimension == SnapshotDimension::Nether && s.remote_players.is_empty());
    drop(a);
    drop(b);
    restarted.stop();
    let saved = ServerSave::read(&GameState::open_in_directory(&world)?)?;
    assert_eq!(
        saved
            .dimensions
            .values()
            .filter(|&&d| d == Dimension::Nether)
            .count(),
        1
    );
    assert_eq!(saved.worlds.len(), 2);
    std::fs::remove_dir_all(root)?;
    Ok(())
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
    // An asynchronous rejection must arrive intact before the writer closes,
    // without registering an actor or disrupting the two healthy clients.
    let mut rejected = TcpStream::connect(&addr)?;
    rejected.set_read_timeout(Some(Duration::from_secs(5)))?;
    write_client_hello(
        &mut rejected,
        &ClientHello {
            protocol: NET_PROTOCOL_VERSION,
            token: Some("0".repeat(32)),
        },
    )?;
    assert!(
        matches!(read_framed_server_message(&mut rejected)?, ServerWireMessage::Rejected(_, message) if message.contains("unrecognized"))
    );
    assert_eq!(rejected.read(&mut [0])?, 0);
    a.until(|s| s.remote_players.len() == 1);
    assert!(
        first.player.x.abs() < 2.0,
        "unexpected fixture spawn {}",
        first.player.x
    );
    // Both can reach the contested block; only A can reach its drop. Letting
    // equally positioned players race the pickup made this test nondeterministic.
    b.send(ClientCommand::SetMoveLeft(true));
    b.until(|s| s.player.x <= -1.0);
    b.send(ClientCommand::ClearDirectionalInput);
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
    // Wait for the actual pickup, not the original spawn x (already >= 0.5).
    let collected = a.until(|s| count(s, ItemType::Wood) == 1);
    a.send(ClientCommand::ClearDirectionalInput);
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

#[test]
#[ignore = "requires local TCP sockets and Unix process signals"]
fn entities_and_collected_loot_survive_real_server_restarts() -> io::Result<()> {
    use termcraft::{
        engine::{
            server_save::{ServerSave, token},
            state::PlayerSession,
        },
        entities::{boat::Boat, cow::Cow, experience_orb::ExperienceOrb, item_entity::ItemEntity},
        world::block::BlockType,
    };
    let root = std::env::temp_dir().join(format!("termcraft-persistence-tcp-{}", token()));
    std::fs::create_dir_all(&root)?;
    let mut recipe = WorldRecipe::example();
    recipe.relief = 0;
    recipe.surface_y = 64;
    recipe.regions.clear();
    recipe.landmarks.clear();
    let world = WorldStore::new(root.join("worlds"))
        .create("tcp", "Entity restart fixture", &recipe)
        .unwrap();
    let mut state = GameState::open_in_directory(&world)?;
    state.apply_client_command(ClientCommand::ToggleRuleMobSpawning);
    for x in -16..=48 {
        for y in 0..64 {
            state.world.set_block(x, y, BlockType::Air);
        }
        state.world.set_block(x, 64, BlockType::Stone);
    }
    state.player.x = 0.5;
    state.player.y = 64.0;
    state.inventory.slots.fill(None);
    state.cows.push(Cow::new(12.5, 64.0));
    state.cows[0].health = 5.0;
    state.boats.push(Boat::new(17.5, 64.0));
    state
        .item_entities
        .push(ItemEntity::new(4.5, 64.0, ItemType::Diamond));
    state.item_entities[0].age = 1234;
    state
        .experience_orbs
        .push(ExperienceOrb::new(22.5, 63.0, 7));
    state.try_persist_world_and_progress()?;
    let player_token = token();
    let mut saved = ServerSave::read(&state)?;
    saved
        .players
        .insert(player_token.clone(), PlayerSession::capture(&state));
    saved.write(&world)?;
    drop(state);
    let probe = TcpListener::bind("127.0.0.1:0")?;
    let addr = probe.local_addr()?.to_string();
    drop(probe);
    let entities = |snapshot: &ServerSnapshot| serde_json::to_value(&snapshot.entities).unwrap();
    let mut server = Server::start(&root, &addr);
    let a = Client::connect(&addr, Some(player_token.clone()));
    a.until(|s| {
        let e = entities(s);
        e["cows"].as_array().unwrap().len() == 1
            && e["boats"].as_array().unwrap().len() == 1
            && e["item_entities"].as_array().unwrap().len() == 1
            && e["experience_orbs"].as_array().unwrap().len() == 1
    });
    drop(a);
    server.stop();
    let disk = GameState::open_in_directory(&world)?;
    assert_eq!(disk.cows.len(), 1);
    assert_eq!(disk.cows[0].health, 5.0);
    assert_eq!(disk.boats.len(), 1);
    assert_eq!(disk.item_entities.len(), 1);
    assert!(disk.item_entities[0].age >= 1234);
    assert_eq!(disk.experience_orbs.len(), 1);
    drop(disk);
    let mut server = Server::start(&root, &addr);
    let mut a = Client::connect(&addr, Some(player_token.clone()));
    a.until(|s| entities(s)["item_entities"].as_array().unwrap().len() == 1);
    a.send(ClientCommand::SetMoveRight(true));
    a.until(|s| count(s, ItemType::Diamond) == 1 && s.player.x >= 4.0);
    a.send(ClientCommand::ClearDirectionalInput);
    a.until(|s| entities(s)["item_entities"].as_array().unwrap().is_empty());
    drop(a);
    server.stop();
    let mut server = Server::start(&root, &addr);
    let a = Client::connect(&addr, Some(player_token));
    a.until(|s| {
        count(s, ItemType::Diamond) == 1
            && entities(s)["item_entities"].as_array().unwrap().is_empty()
    });
    drop(a);
    server.stop();
    let disk = GameState::open_in_directory(&world)?;
    assert!(disk.item_entities.is_empty());
    assert_eq!(disk.boats.len(), 1);
    drop(disk);
    std::fs::remove_dir_all(root)?;
    Ok(())
}

#[test]
#[ignore = "requires local TCP sockets and process signals"]
fn farm_feeding_birth_and_young_animals_replicate_and_survive_restart() -> io::Result<()> {
    use termcraft::{
        engine::{
            server_save::{ServerSave, token},
            state::PlayerSession,
        },
        entities::cow::Cow,
        world::{block::BlockType, item::ItemStack},
    };
    let root = std::env::temp_dir().join(format!("termcraft-farm-tcp-{}", token()));
    let mut recipe = WorldRecipe::example();
    recipe.relief = 0;
    recipe.surface_y = 64;
    recipe.regions.clear();
    recipe.landmarks.clear();
    let world = WorldStore::new(root.join("worlds"))
        .create("tcp", "Farm lifecycle", &recipe)
        .unwrap();
    let mut state = GameState::open_in_directory(&world)?;
    state.dismiss_startup_splash();
    for command in [
        ClientCommand::ToggleRuleMobSpawning,
        ClientCommand::ToggleRuleDaylightCycle,
        ClientCommand::ToggleRuleWeatherCycle,
    ] {
        state.apply_client_command(command);
    }
    for x in -16..48 {
        for y in 0..64 {
            state.world.set_block(x, y, BlockType::Air);
        }
        state.world.set_block(x, 64, BlockType::Grass);
    }
    state.player.x = 0.5;
    state.player.y = 64.0;
    state.inventory.slots.fill(None);
    state.inventory.slots[0] = Some(ItemStack {
        item_type: ItemType::Wheat,
        count: 8,
        durability: None,
    });
    state.cows = vec![Cow::new(1.5, 64.0), Cow::new(2.5, 64.0)];
    state.try_persist_world_and_progress()?;
    let a_token = token();
    let b_token = token();
    let mut saved = ServerSave::read(&state)?;
    saved
        .players
        .insert(a_token.clone(), PlayerSession::capture(&state));
    saved
        .players
        .insert(b_token.clone(), PlayerSession::capture(&state));
    saved.write(&world)?;
    drop(state);
    let reservation = TcpListener::bind("127.0.0.1:0")?;
    let address = reservation.local_addr()?.to_string();
    drop(reservation);
    let cows = |s: &ServerSnapshot| {
        serde_json::to_value(&s.entities).unwrap()["cows"]
            .as_array()
            .unwrap()
            .clone()
    };
    let mut server = Server::start(&root, &address);
    let mut a = Client::connect(&address, Some(a_token.clone()));
    let b = Client::connect(&address, Some(b_token.clone()));
    a.until(|s| cows(s).len() == 2);
    a.send(ClientCommand::UseAt(1, 63));
    a.until(|s| count(s, ItemType::Wheat) == 7);
    a.send(ClientCommand::UseAt(2, 63));
    a.until(|s| count(s, ItemType::Wheat) == 6);
    for client in [&a, &b] {
        client.until(|s| {
            let c = cows(s);
            c.len() == 3
                && c.iter()
                    .filter(|m| m["husbandry"]["child_ticks"].as_u64().unwrap() > 0)
                    .count()
                    == 1
        });
    }
    drop(a);
    drop(b);
    server.stop();
    let mut server = Server::start(&root, &address);
    let a = Client::connect(&address, Some(a_token));
    let b = Client::connect(&address, Some(b_token));
    for client in [&a, &b] {
        client.until(|s| {
            let c = cows(s);
            c.len() == 3
                && c.iter().all(|m| m["husbandry"]["persistent"] == true)
                && c.iter()
                    .any(|m| m["husbandry"]["child_ticks"].as_u64().unwrap() > 0)
        });
    }
    drop(a);
    drop(b);
    server.stop();
    std::fs::remove_dir_all(root)?;
    Ok(())
}
