use termcraft::engine::GameLoop;
use termcraft::engine::net::{DEFAULT_SERVER_ADDR, run_headless_tcp_server};
use termcraft::engine::network_client::run_tcp_client;
use termcraft::renderer::Renderer;

fn main() -> std::io::Result<()> {
    let mut args = std::env::args().skip(1);
    if let Some(mode) = args.next() {
        match mode.as_str() {
            "server" => {
                let bind_addr = args
                    .next()
                    .unwrap_or_else(|| DEFAULT_SERVER_ADDR.to_string());
                println!("Starting headless server on {bind_addr}");
                return run_headless_tcp_server(&bind_addr);
            }
            "client" | "connect" => {
                let connect_addr = args
                    .next()
                    .unwrap_or_else(|| DEFAULT_SERVER_ADDR.to_string());
                println!("Connecting multiplayer client to {connect_addr}");
                return run_tcp_client(&connect_addr);
            }
            _ => {}
        }
    }

    // Initialize the double-buffered terminal renderer
    let mut renderer = Renderer::new()?;
    renderer.init()?;

    let mut game_loop = GameLoop::new();

    // Catch the result so we can guarantee terminal restoration
    let res = game_loop.run(&mut renderer);

    // Always restore the terminal back to normal state before exiting
    renderer.restore()?;

    if let Err(e) = res {
        eprintln!("Game Error: {}", e);
    }

    Ok(())
}
