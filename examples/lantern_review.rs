//! Capture actual trial terrain and combat/airborne states through the game renderer.
use std::{fs, path::Path};
use termcraft::{
    engine::{command::ClientCommand, state::GameState},
    renderer::Renderer,
    worlds::WorldStore,
};
fn capture(s: &GameState, out: &Path, name: &str) -> std::io::Result<()> {
    for (w, h) in [(80, 24), (120, 40), (160, 48)] {
        let mut r = Renderer::with_size(w, h);
        let mut bytes = Vec::new();
        r.render_to(s, 0.0, &mut bytes)?;
        fs::write(out.join(format!("{name}-{w}x{h}.ansi")), bytes)?;
    }
    Ok(())
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let out = std::path::PathBuf::from(std::env::args().nth(1).ok_or("output directory required")?);
    fs::create_dir_all(&out)?;
    let root = std::env::temp_dir().join(format!("tc-lantern-render-{}", rand::random::<u64>()));
    let store = WorldStore::new(root.clone());
    GameState::create_lantern_trial(&store, "review")?;
    let path = store.open("review")?;
    let mut s = GameState::open_in_directory(&path)?;
    s.dismiss_startup_splash();
    capture(&s, &out, "camp")?;
    // Focused pose fixture on real trial terrain; player position is staged here.
    s.player.x = 36.5;
    s.update(0, 0);
    for _ in 0..4 {
        s.update(0, 0);
    }
    capture(&s, &out, "windup")?;
    for _ in 0..6 {
        s.update(0, 0);
    }
    capture(&s, &out, "hurt")?;
    s.apply_client_command(ClientCommand::SetPrimaryAction(true));
    s.update(37, 63);
    capture(&s, &out, "strike")?;
    for _ in 0..3 {
        s.update(37, 63);
    }
    capture(&s, &out, "follow-through")?;
    s.apply_client_command(ClientCommand::SetPrimaryAction(false));
    for _ in 0..7 {
        s.update(0, 0);
    }
    capture(&s, &out, "recovery")?;
    s.apply_client_command(ClientCommand::SetJumpHeld(true));
    s.apply_client_command(ClientCommand::QueueJump);
    for _ in 0..3 {
        s.update(0, 0);
    }
    capture(&s, &out, "ascent")?;
    for _ in 0..7 {
        s.update(0, 0);
    }
    capture(&s, &out, "descent")?;
    s.player.x = 49.5;
    s.player.y = 64.0;
    s.player.grounded = true;
    capture(&s, &out, "ravine")?;
    let mut r = Renderer::with_size(160, 48);
    for tick in 0..36 {
        r.capture_motion_step(&s);
        s.update(0, 0);
        for frame in 0..3 {
            let mut bytes = Vec::new();
            r.render_to(&s, frame as f64 / 3.0, &mut bytes)?;
            fs::write(
                out.join(format!("motion-{:03}.ansi", tick * 3 + frame)),
                bytes,
            )?;
        }
    }
    drop(s);
    fs::remove_dir_all(root)?;
    Ok(())
}
