//! Two client replicas travelling through a real portal independently.
//! Frames, starting positions and tools are staged; actions/replication are real.
use serde_json::{Value, json};
use std::{fs, path::Path};
use termcraft::{
    engine::{
        command::ClientCommand,
        net::{ClientInputFrame, ServerSimulation, SnapshotDimension},
        server_save::{ServerSave, token},
        state::{GameState, PlayerSession},
    },
    renderer::Renderer,
    world::{
        Dimension,
        block::BlockType,
        item::{ItemStack, ItemType},
    },
};

fn act(sim: &mut ServerSimulation, id: u16, command: ClientCommand) {
    let snapshot = sim.snapshot_for_client(id).unwrap();
    let mut input = ClientInputFrame::new(
        id,
        snapshot.acknowledged_sequence + 1,
        sim.tick() + 1,
        command,
    );
    input.dimension = snapshot.dimension;
    sim.enqueue_input(input);
    sim.step(0, 0);
}

fn capture(
    out: &Path,
    sim: &mut ServerSimulation,
    phase: &str,
    title: &str,
    expected: [SnapshotDimension; 2],
) -> Result<Vec<Value>, Box<dyn std::error::Error>> {
    let mut scenes = Vec::new();
    for id in 1..=2 {
        let snapshot = sim.snapshot_for_client(id).unwrap();
        assert_eq!(snapshot.dimension, expected[id as usize - 1]);
        assert_eq!(
            snapshot.remote_players.len(),
            usize::from(expected[0] == expected[1])
        );
        let view = sim.client_view(id).unwrap();
        assert_eq!(
            SnapshotDimension::from(view.current_dimension),
            snapshot.dimension
        );
        let name = format!("coop-{phase}-player-{id}");
        let mut images = Vec::new();
        for (zoom, mode) in [(0, "overview"), (2, "detail")] {
            let mut renderer = Renderer::with_size(200, 60);
            for _ in 0..zoom {
                renderer.toggle_zoom();
            }
            let mut bytes = Vec::new();
            renderer.render_to(&view, 0.0, &mut bytes)?;
            let ansi = format!("{name}-{mode}-200x60.ansi");
            fs::write(out.join(&ansi), bytes)?;
            images.push(json!({"view":mode,"ansi":ansi,"width":200,"height":60}));
        }
        scenes.push(json!({"id":name,"title":format!("{title} · player {id}"),
            "dimension":format!("{:?}",view.current_dimension),"category":"Co-op",
            "origin":"Scripted replicated gameplay","source":"examples/cooperative_review.rs",
            "caption":"Staged room, frame and starting tools; normal ignition/travel/movement commands. This view uses the native client's snapshot and terrain handlers. Socket transport is tested separately; this is not a human co-op recording.",
            "x":view.player.x,"y":view.player.y,"images":images}));
    }
    Ok(scenes)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    use BlockType::*;
    use SnapshotDimension::{Nether, Overworld};
    let out = std::path::PathBuf::from(std::env::args().nth(1).ok_or("output directory required")?);
    fs::create_dir_all(&out)?;
    let root = std::env::temp_dir().join(format!("termcraft-cooperative-review-{}", token()));
    fs::create_dir_all(&root)?;
    let seeds = bincode::serialize(&[1337u32, 7331, 9917, 4444])?;
    for dimension in ["overworld", "nether", "end"] {
        fs::write(root.join(format!("{dimension}_world_seed.bin")), &seeds)?;
    }
    let mut state = GameState::open_in_directory(&root)?;
    state.dismiss_startup_splash();
    for command in [
        ClientCommand::ToggleRuleMobSpawning,
        ClientCommand::ToggleRuleDaylightCycle,
        ClientCommand::ToggleRuleWeatherCycle,
    ] {
        state.apply_client_command(command);
    }
    state.time_of_day = 0.0;
    state.world.load_chunks_for_spawn_search(0, 64);
    for x in -18..=18 {
        for y in 24..44 {
            state.world.set_block(
                x,
                y,
                if y >= 36 || y == 30 || ((x == -12 || x == 12) && y > 30) {
                    StoneBricks
                } else {
                    Air
                },
            );
        }
    }
    for x in 4..=7 {
        for y in 32..=36 {
            if x == 4 || x == 7 || y == 32 || y == 36 {
                state.world.set_block(x, y, Obsidian);
            }
        }
    }
    state.player.y = 36.0;
    state.player.grounded = true;
    state.inventory.slots.fill(None);
    for (slot, item, count) in [(0, ItemType::FlintAndSteel, 1), (1, ItemType::Glowstone, 1)] {
        state.inventory.slots[slot] = Some(ItemStack {
            item_type: item,
            count,
            durability: item.max_durability(),
        });
    }
    let mut saved = ServerSave::read(&state)?;
    let identities = [token(), token()];
    for (id, x) in [(0, 3.5), (1, 1.5)] {
        state.player.x = x;
        saved
            .players
            .insert(identities[id].clone(), PlayerSession::capture(&state));
        saved
            .dimensions
            .insert(identities[id].clone(), Dimension::Overworld);
    }
    saved.write(&root)?;
    let mut sim = ServerSimulation::with_state(state);
    sim.load_persistent()?;
    sim.authenticate(1, Some(&identities[0]))?;
    sim.authenticate(2, Some(&identities[1]))?;
    act(&mut sim, 1, ClientCommand::SelectHotbarSlot(1));
    act(&mut sim, 1, ClientCommand::UseAt(1, 31));
    assert_eq!(
        sim.client_view(1).unwrap().world.get_block(1, 31),
        Glowstone
    );
    act(&mut sim, 1, ClientCommand::SelectHotbarSlot(0));
    act(&mut sim, 1, ClientCommand::UseAt(4, 34));
    assert_eq!(
        sim.client_view(1)
            .unwrap()
            .world
            .nether_portal_anchor_at(5, 34),
        Some((5, 36))
    );
    let mut scenes = capture(
        &out,
        &mut sim,
        "together",
        "Shared Overworld workshop",
        [Overworld, Overworld],
    )?;
    act(&mut sim, 1, ClientCommand::UseAt(5, 34));
    scenes.extend(capture(
        &out,
        &mut sim,
        "apart",
        "One travels; their friend stays",
        [Nether, Overworld],
    )?);
    let arrival = sim.client_view(1).unwrap();
    let mut anchors = std::collections::BTreeSet::new();
    for x in -24..=24 {
        for y in 0..128 {
            if let Some(anchor) = arrival.world.nether_portal_anchor_at(x, y) {
                anchors.insert(anchor);
            }
        }
    }
    assert_eq!(anchors.len(), 1);
    let (x, y) = anchors.into_iter().next().unwrap();
    act(&mut sim, 1, ClientCommand::SetMoveLeft(true));
    for _ in 0..5 {
        sim.step(0, 0);
    }
    act(&mut sim, 1, ClientCommand::ClearDirectionalInput);
    act(&mut sim, 2, ClientCommand::UseAt(5, 34));
    scenes.extend(capture(
        &out,
        &mut sim,
        "nether",
        "Friends reunited in the Nether",
        [Nether, Nether],
    )?);
    for _ in 0..42 {
        sim.step(0, 0);
    }
    // The generated clear landing may be several blocks beside the frame.
    // Walk back within interaction reach, rather than staging a teleport.
    for _ in 0..40 {
        let player_x = sim.snapshot_for_client(1).unwrap().player.x;
        if (player_x - (x as f64 + 0.5)).abs() <= 3.0 {
            break;
        }
        act(
            &mut sim,
            1,
            if player_x > x as f64 + 0.5 {
                ClientCommand::SetMoveLeft(true)
            } else {
                ClientCommand::SetMoveRight(true)
            },
        );
    }
    act(&mut sim, 1, ClientCommand::ClearDirectionalInput);
    act(&mut sim, 1, ClientCommand::UseAt(x, y - 2));
    scenes.extend(capture(
        &out,
        &mut sim,
        "return",
        "One returns; their friend keeps exploring",
        [Overworld, Nether],
    )?);
    sim.save()?;
    fs::write(
        out.join("catalog.json"),
        serde_json::to_vec_pretty(&json!({"checks":{
        "actual_glowstone_placement":true,"normal_portal_ignition":true,"independent_travel":true,
        "dimension_filtered_peers":true,"shared_arrival":true,"independent_return":true,
        "native_client_replica_handlers":true},"scenes":scenes}))?,
    )?;
    drop(sim);
    fs::remove_dir_all(root)?;
    Ok(())
}
