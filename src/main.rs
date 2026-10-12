use std::io::{self, Read};
use termcraft::engine::GameLoop;
use termcraft::engine::net::{
    DEFAULT_SERVER_ADDR, run_headless_tcp_server, run_headless_tcp_server_with_state,
};
use termcraft::engine::network_client::run_tcp_client_with_profile;
use termcraft::engine::state::GameState;
use termcraft::renderer::Renderer;
use termcraft::world::recipe::WorldRecipe;
use termcraft::worlds::{WorldLease, WorldStore};

fn main() -> io::Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("--help" | "-h" | "help") => {
            println!(
                "termcraft {}\n\nPlay:\n  termcraft\n  termcraft trial [ID]           Lantern Run: traverse, fight, rebuild\n  termcraft trial create ID      Create the trial without launching\n  termcraft server [ADDRESS]\n  termcraft client [ADDRESS] [--player PROFILE]\n  termcraft join USER@HOST [--player PROFILE] [--ssh-port PORT] [--game-port PORT]\n\nGenerated worlds (experimental):\n  termcraft world schema\n  termcraft world list\n  termcraft world create ID RECIPE.json [ORIGINAL_PROMPT]\n  termcraft world generate ID \"PROMPT\" [--generator PROGRAM]\n  termcraft world play ID\n  termcraft world serve ID [ADDRESS]\n\nIn-game game master (experimental, local Overworld):\n  T opens chat; Enter sends; /help lists commands.\n  Use /connect for your existing LLM client, or configure TERMCRAFT_DIRECTOR.\n  Co-op: the first connected player controls shared requests.\n  termcraft director schema\n\nLLM tools:\n  termcraft mcp                  Stdio MCP server for Claude/Codex\n  termcraft mcp config [--format json|toml]   Print client configuration\n\nEach generated world has independent saves. MCP worlds use stepped simulation.\nGeneration requires an explicitly configured runner (TERMCRAFT_WORLD_GENERATOR\nor --generator). Up to three design attempts; configured API runners may incur costs.\nUse world_schema and validate_world in your agent before create_world.",
                env!("CARGO_PKG_VERSION")
            );
            Ok(())
        }
        Some("--version" | "-V") => {
            println!("termcraft {}", env!("CARGO_PKG_VERSION"));
            Ok(())
        }
        Some("server") => run_headless_tcp_server(
            args.get(1)
                .map(String::as_str)
                .unwrap_or(DEFAULT_SERVER_ADDR),
        ),
        Some("join") => {
            let destination=args.get(1).ok_or_else(||io::Error::other("use: termcraft join USER@HOST [--player PROFILE] [--ssh-port PORT] [--game-port PORT]"))?;
            let mut profile = "player";
            let mut ssh_port = 22;
            let mut game_port = 25565;
            let mut i = 2;
            while i < args.len() {
                let value = args
                    .get(i + 1)
                    .ok_or_else(|| io::Error::other("join option needs a value"))?;
                match args[i].as_str() {
                    "--player" => profile = value,
                    "--ssh-port" => ssh_port = value.parse::<u16>().map_err(io::Error::other)?,
                    "--game-port" => game_port = value.parse::<u16>().map_err(io::Error::other)?,
                    _ => return Err(io::Error::other("unknown join option")),
                }
                i += 2;
            }
            termcraft::engine::ssh::join(destination, profile, ssh_port, game_port)
        }
        Some("client" | "connect") => {
            let mut address = DEFAULT_SERVER_ADDR;
            let mut profile = "player";
            let mut i = 1;
            while i < args.len() {
                if args[i] == "--player" {
                    profile = args
                        .get(i + 1)
                        .ok_or_else(|| io::Error::other("--player requires a profile name"))?;
                    i += 2;
                } else if i == 1 {
                    address = &args[i];
                    i += 1;
                } else {
                    return Err(io::Error::other(
                        "use: termcraft client [ADDRESS] [--player PROFILE]",
                    ));
                }
            }
            run_tcp_client_with_profile(address, profile)
        }
        Some("mcp") if args.len() == 1 => termcraft::engine::mcp::run_stdio(),
        Some("mcp") if args.get(1).is_some_and(|arg| arg == "config") => {
            let format = match args.as_slice() {
                [_, _] => "json",
                [_, _, option, format] if option == "--format" => format,
                _ => {
                    return Err(io::Error::other(
                        "use: termcraft mcp config [--format json|toml]",
                    ));
                }
            };
            print_mcp_config(format)
        }
        Some("trial") => {
            let create_only = args.get(1).is_some_and(|s| s == "create");
            let id = args
                .get(if create_only { 2 } else { 1 })
                .map(String::as_str)
                .unwrap_or("lantern-run");
            if args.len() > if create_only { 3 } else { 2 } {
                return Err(io::Error::other(
                    "use: termcraft trial [ID] | trial create ID",
                ));
            }
            let store = WorldStore::default();
            if create_only || store.open(id).is_err() {
                GameState::create_lantern_trial(&store, id)?;
            }
            let root = store.open(id).map_err(io::Error::other)?;
            if create_only {
                println!("Created Lantern Run. Play: termcraft world play {id}");
                Ok(())
            } else {
                let _lease = WorldLease::acquire(&root).map_err(io::Error::other)?;
                play(GameState::open_in_directory(&root)?)
            }
        }
        Some("world") => world_command(&args[1..]),
        Some("director") if args.len() == 2 && args[1] == "schema" => {
            println!(
                "{}",
                serde_json::to_string_pretty(
                    &serde_json::json!({"schema":termcraft::engine::director::schema(),"example":termcraft::engine::director::example()})
                )?
            );
            Ok(())
        }
        None => {
            let root = termcraft::save_directory();
            std::fs::create_dir_all(&root)?;
            let _lease = WorldLease::acquire(&root).map_err(io::Error::other)?;
            play(GameState::try_new()?)
        }
        Some(mode) => Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("unknown command {mode}; use --help"),
        )),
    }
}

fn print_mcp_config(format: &str) -> io::Result<()> {
    let command = std::env::current_exe()?;
    let command = command
        .to_str()
        .ok_or_else(|| io::Error::other("MCP configuration requires a Unicode executable path"))?;
    let mut server = serde_json::json!({"command": command, "args": ["mcp"]});
    if let Some(root) = std::env::var_os("TERMCRAFT_SAVE_DIR") {
        let root = std::path::PathBuf::from(root);
        let root = if root.is_absolute() {
            root
        } else {
            std::env::current_dir()?.join(root)
        };
        server["env"] = serde_json::json!({"TERMCRAFT_SAVE_DIR": root});
    }
    match format {
        "json" => println!(
            "{}",
            serde_json::to_string_pretty(
                &serde_json::json!({"mcpServers": {"termcraft": server}})
            )?
        ),
        "toml" => {
            println!(
                "[mcp_servers.termcraft]\ncommand = {}\nargs = [\"mcp\"]",
                serde_json::to_string(command)?
            );
            if let Some(root) = server.get("env").and_then(|e| e.get("TERMCRAFT_SAVE_DIR")) {
                println!("\n[mcp_servers.termcraft.env]\nTERMCRAFT_SAVE_DIR = {root}");
            }
        }
        _ => return Err(io::Error::other("MCP config format must be json or toml")),
    }
    Ok(())
}

fn world_command(args: &[String]) -> io::Result<()> {
    let store = WorldStore::default();
    match args.first().map(String::as_str) {
        Some("schema") if args.len() == 1 => {
            println!(
                "{}",
                serde_json::to_string_pretty(&serde_json::json!({
                    "schema":WorldRecipe::schema(),"example":WorldRecipe::example()
                }))?
            );
            Ok(())
        }
        Some("list") if args.len() == 1 => {
            println!(
                "{}",
                serde_json::to_string_pretty(&store.list().map_err(io::Error::other)?)?
            );
            Ok(())
        }
        Some("create") if (3..=4).contains(&args.len()) => {
            let mut bytes = Vec::new();
            std::fs::File::open(&args[2])?
                .take(256 * 1024 + 1)
                .read_to_end(&mut bytes)?;
            if bytes.len() > 256 * 1024 {
                return Err(io::Error::other("recipe exceeds 256 KiB"));
            }
            let recipe: WorldRecipe = serde_json::from_slice(&bytes)?;
            store
                .create(
                    &args[1],
                    args.get(3).map(String::as_str).unwrap_or(""),
                    &recipe,
                )
                .map_err(io::Error::other)?;
            println!(
                "Created {}. Play with: termcraft world play {}",
                recipe.name, args[1]
            );
            Ok(())
        }
        Some("generate") if args.len() == 3 || (args.len() == 5 && args[3] == "--generator") => {
            let program = args.get(4).map(std::ffi::OsString::from)
                .or_else(|| std::env::var_os("TERMCRAFT_WORLD_GENERATOR"))
                .ok_or_else(|| io::Error::other("set TERMCRAFT_WORLD_GENERATOR or --generator to a trusted executable; see docs/agents.md"))?;
            termcraft::generator::generate(
                &store,
                &args[1],
                &args[2],
                std::path::Path::new(&program),
                |message| eprintln!("{message}"),
            )
            .map_err(io::Error::other)?;
            println!("World ready. Enter with: termcraft world play {}", args[1]);
            Ok(())
        }
        Some("serve") if (2..=3).contains(&args.len()) => {
            let root = store.open(&args[1]).map_err(io::Error::other)?;
            let _lease = WorldLease::acquire(&root).map_err(io::Error::other)?;
            run_headless_tcp_server_with_state(
                args.get(2)
                    .map(String::as_str)
                    .unwrap_or(DEFAULT_SERVER_ADDR),
                GameState::open_in_directory(&root)?,
            )
        }
        Some("play") if args.len() == 2 => {
            let root = store.open(&args[1]).map_err(io::Error::other)?;
            let _lease = WorldLease::acquire(&root).map_err(io::Error::other)?;
            play(GameState::open_in_directory(&root)?)
        }
        _ => Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "use: termcraft world schema | list | create ID RECIPE.json [PROMPT] | generate ID PROMPT [--generator PROGRAM] | play ID | serve ID [ADDRESS]",
        )),
    }
}

fn play(state: GameState) -> io::Result<()> {
    let mut renderer = Renderer::new()?;
    if let Err(error) = renderer.init() {
        let _ = renderer.restore();
        return Err(error);
    }
    let mut game_loop = GameLoop::with_state(state);
    let result = game_loop.run(&mut renderer);
    let restored = renderer.restore();
    result.and(restored)
}
