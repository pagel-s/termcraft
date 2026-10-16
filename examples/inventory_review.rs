//! Disposable UI and material captures; uses the live renderer and normal interactions.
use std::{fs, path::Path};
use termcraft::{
    engine::{command::ClientCommand, state::GameState},
    renderer::Renderer,
    world::{
        block::BlockType,
        chunk::{CHUNK_HEIGHT, CHUNK_WIDTH},
        item::{ItemStack, ItemType},
    },
};
fn capture(
    out: &Path,
    id: &str,
    s: &GameState,
    cols: u16,
    rows: u16,
    book: bool,
) -> std::io::Result<()> {
    let mut r = Renderer::with_size(cols, rows);
    if (cols >= 100) != book {
        r.toggle_recipe_book();
    }
    let mut bytes = Vec::new();
    r.render_to(s, 0.0, &mut bytes)?;
    fs::write(out.join(format!("{id}-{cols}x{rows}.ansi")), bytes)
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let out = std::path::PathBuf::from(
        std::env::args_os()
            .nth(1)
            .ok_or("output directory required")?,
    );
    fs::create_dir_all(&out)?;
    let root = std::env::temp_dir().join(format!(
        "termcraft-inventory-review-{:032x}",
        rand::random::<u128>()
    ));
    fs::create_dir(&root)?;
    let mut s = GameState::open_in_directory(&root)?;
    s.dismiss_startup_splash();
    for cx in -2..=2 {
        s.world
            .apply_chunk_column_snapshot(cx, &vec![BlockType::Air; CHUNK_WIDTH * CHUNK_HEIGHT]);
    }
    for x in -30..30 {
        s.world.set_block(x, 40, BlockType::StoneBricks);
    }
    s.player.x = 0.5;
    s.player.y = 40.0;
    s.player.grounded = true;
    s.player.hunger = 19.9975;
    s.inventory.slots.fill(None);
    for (slot, item, count) in [
        (0, ItemType::Wood, 16),
        (1, ItemType::Cobblestone, 32),
        (2, ItemType::IronPickaxe, 1),
        (3, ItemType::CookedBeef, 4),
        (4, ItemType::Coal, 16),
        (5, ItemType::Torch, 8),
        (9, ItemType::RawIron, 12),
        (10, ItemType::Planks, 32),
        (11, ItemType::Diamond, 8),
        (12, ItemType::Stick, 32),
        (13, ItemType::Wool, 16),
    ] {
        s.inventory.slots[slot] = Some(ItemStack {
            item_type: item,
            count,
            durability: item.max_durability(),
        });
    }
    s.apply_client_command(ClientCommand::ToggleInventory);
    capture(&out, "inventory-recipes", &s, 160, 48, true)?;
    capture(&out, "compact-bag", &s, 80, 24, false)?;
    capture(&out, "compact-recipes", &s, 80, 24, true)?;
    s.apply_client_command(ClientCommand::ToggleInventory);
    s.world.set_block(3, 39, BlockType::Chest);
    s.world
        .chest_inventory_mut(3, 39)
        .unwrap()
        .add_item(ItemType::Stone, 64);
    s.world
        .chest_inventory_mut(3, 39)
        .unwrap()
        .add_item(ItemType::Diamond, 12);
    s.apply_client_command(ClientCommand::UseAt(3, 39));
    capture(&out, "chest", &s, 160, 48, true)?;
    s.apply_client_command(ClientCommand::ToggleInventory);
    s.world.set_block(3, 39, BlockType::Furnace);
    s.apply_client_command(ClientCommand::UseAt(3, 39));
    capture(&out, "furnace", &s, 160, 48, true)?;
    s.apply_client_command(ClientCommand::ToggleInventory);
    s.world.set_block(3, 39, BlockType::Air);
    for x in -30..30 {
        for y in 0..40 {
            s.world.set_block(
                x,
                y,
                if (-18..=18).contains(&x) && y >= 30 {
                    BlockType::Air
                } else {
                    BlockType::Stone
                },
            );
        }
    }
    s.world.set_block(3, 38, BlockType::RedstoneOre);
    s.inventory.slots[0] = None;
    capture(&out, "ore-unlit", &s, 160, 48, false)?;
    s.apply_client_command(ClientCommand::UseAt(3, 38));
    assert!(matches!(
        s.world.get_block(3, 38),
        BlockType::LitRedstoneOre(_)
    ));
    capture(&out, "ore-lit", &s, 160, 48, false)?;
    s.world.set_block(4, 39, BlockType::Lava(8));
    for x in 5..=8 {
        s.world.set_block(x, 39, BlockType::Planks);
    }
    for _ in 0..20 {
        s.world.update(0);
    }
    capture(&out, "lava-ignition", &s, 160, 48, false)?;
    drop(s);
    fs::remove_dir_all(root)?;
    Ok(())
}
