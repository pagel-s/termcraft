//! Runtime attachment rules shared by placement, support removal and rendering.
use super::{World, block::BlockType};
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RoomMaterial {
    Wood,
    Masonry,
    Obsidian,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum TorchAttachment {
    Floor { offset: f64 },
    LeftWall,
    RightWall,
    BackWall,
}
impl World {
    /// Select an actual surface, or the same enclosed cutaway wall used in rendering.
    /// Floor offsets preserve contact on lower slabs without changing collision.
    pub fn torch_attachment(&self, x: i32, y: i32) -> Option<TorchAttachment> {
        let floor = self
            .get_block(x, y + 1)
            .collision_boxes()
            .iter()
            .filter(|b| b[0] <= 0.5 && b[2] > 0.5)
            .map(|b| b[1])
            .min_by(f64::total_cmp);
        if let Some(offset) = floor {
            return Some(TorchAttachment::Floor { offset });
        }
        if self
            .get_block(x - 1, y)
            .collision_boxes()
            .iter()
            .any(|b| b[2] == 1.0 && b[1] <= 0.75 && b[3] > 0.75)
        {
            return Some(TorchAttachment::LeftWall);
        }
        if self
            .get_block(x + 1, y)
            .collision_boxes()
            .iter()
            .any(|b| b[0] == 0.0 && b[1] <= 0.75 && b[3] > 0.75)
        {
            return Some(TorchAttachment::RightWall);
        }
        self.room_enclosure(x, y).map(|_| TorchAttachment::BackWall)
    }
    pub fn room_enclosure(&self, x: i32, y: i32) -> Option<RoomMaterial> {
        // Short masonry passages are a deliberate cutaway beneath Nether
        // pylons/arches. Recessed supports connect their caps to foundations
        // visually while leaving the side-view walking lane open.
        if self.dimension == crate::world::Dimension::Nether {
            let roof = (1..=3).find(|dy| self.get_block(x, y - dy) != BlockType::Air);
            let floor = (1..=3).find(|dy| self.get_block(x, y + dy) != BlockType::Air);
            if let (Some(roof), Some(floor)) = (roof, floor)
                && roof + floor <= 4
                && self.get_block(x, y - roof) == BlockType::StoneBricks
                && self.get_block(x, y + floor) == BlockType::StoneBricks
            {
                return Some(RoomMaterial::Masonry);
            }
        }
        // End tunnels deliberately use their own short obsidian recess.
        if self.dimension == crate::world::Dimension::End {
            let roof = (1..=6).find(|dy| self.get_block(x, y - dy) != BlockType::Air)?;
            if roof <= 3
                && self.get_block(x, y - roof) == BlockType::Obsidian
                && (1..=3).any(|dy| self.get_block(x, y + dy) == BlockType::Obsidian)
            {
                return Some(RoomMaterial::Obsidian);
            }
        }
        let roof_material = |b| {
            matches!(
                b,
                BlockType::Planks
                    | BlockType::Wood
                    | BlockType::BirchWood
                    | BlockType::StoneBricks
                    | BlockType::Cobblestone
                    | BlockType::StoneSlab
                    | BlockType::StoneStairs
                    | BlockType::Glass
            )
        };
        let wall_material =
            |b| roof_material(b) || matches!(b, BlockType::WoodDoor(_) | BlockType::IronDoor(_));
        let furnishing = |b| {
            matches!(
                b,
                BlockType::Chest
                    | BlockType::CraftingTable
                    | BlockType::Furnace
                    | BlockType::Bed
                    | BlockType::Anvil
                    | BlockType::EnchantingTable
                    | BlockType::BrewingStand
            )
        };
        let floor = (1..=6).find(|dy| {
            let b = self.get_block(x, y + dy);
            b.is_solid() && !furnishing(b)
        })?;
        let ground = y + floor;
        // A partial loft is not the room's ceiling. Try each structural candidate
        // until a complete enclosing roof is found; furnishings never define it.
        for roof in 1..=6 {
            let ceiling = y - roof;
            let material = self.get_block(x, ceiling);
            if material == BlockType::Glass || !roof_material(material) || roof + floor > 8 {
                continue;
            }
            let side = |direction| {
                (1..=16).any(|dx| {
                    let bx = x + dx * direction;
                    (ceiling + 1..ground).all(|by| wall_material(self.get_block(bx, by)))
                        && (x.min(bx)..=x.max(bx))
                            .all(|rx| roof_material(self.get_block(rx, ceiling)))
                })
            };
            if !side(-1) || !side(1) {
                continue;
            }
            return Some(
                if matches!(
                    material,
                    BlockType::StoneBricks
                        | BlockType::Cobblestone
                        | BlockType::StoneSlab
                        | BlockType::StoneStairs
                ) {
                    RoomMaterial::Masonry
                } else {
                    RoomMaterial::Wood
                },
            );
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::{
        Dimension,
        chunk::{CHUNK_HEIGHT, CHUNK_WIDTH},
        item::ItemType,
    };
    fn empty() -> World {
        let mut world = World::network_replica(Dimension::Overworld);
        for chunk in -1..=1 {
            world.apply_chunk_column_snapshot(
                chunk,
                &vec![BlockType::Air; CHUNK_WIDTH * CHUNK_HEIGHT],
            );
        }
        world
    }
    #[test]
    fn nether_cutaway_supports_are_bounded_by_real_masonry() {
        let mut w = empty();
        w.dimension = Dimension::Nether;
        w.set_block(2, 16, BlockType::StoneBricks);
        w.set_block(2, 20, BlockType::StoneBricks);
        for y in 17..20 {
            assert_eq!(w.room_enclosure(2, y), Some(RoomMaterial::Masonry));
            assert!(
                !w.get_block(2, y).is_solid(),
                "cutaway must remain walkable"
            );
        }
        assert_eq!(w.room_enclosure(3, 18), None);
        w.set_block(2, 20, BlockType::Air);
        assert_eq!(w.room_enclosure(2, 18), None, "unsupported floating prop");
        w.set_block(2, 22, BlockType::StoneBricks);
        assert_eq!(
            w.room_enclosure(2, 19),
            None,
            "must not paint tall sky columns"
        );
    }

    #[test]
    fn floor_slab_and_real_side_faces_define_attachment() {
        let mut w = empty();
        assert_eq!(w.torch_attachment(0, 9), None);
        w.set_block(0, 8, BlockType::Stone); // A ceiling is not a torch mounting face.
        assert_eq!(w.torch_attachment(0, 9), None);
        w.set_block(0, 10, BlockType::StoneSlab);
        assert_eq!(
            w.torch_attachment(0, 9),
            Some(TorchAttachment::Floor { offset: 0.5 })
        );
        w.set_block(0, 10, BlockType::StoneStairs);
        assert_eq!(
            w.torch_attachment(0, 9),
            Some(TorchAttachment::Floor { offset: 0.0 })
        );
        w.set_block(0, 10, BlockType::Water(8));
        assert_eq!(w.torch_attachment(0, 9), None);
        w.set_block(-1, 9, BlockType::Stone);
        assert_eq!(w.torch_attachment(0, 9), Some(TorchAttachment::LeftWall));
        w.set_block(-1, 9, BlockType::Air);
        w.set_block(1, 9, BlockType::Stone);
        assert_eq!(w.torch_attachment(0, 9), Some(TorchAttachment::RightWall));
    }
    #[test]
    fn unsupported_torches_drop_once_including_negative_chunk_seams() {
        let mut w = empty();
        w.set_block(-1, 10, BlockType::Stone);
        w.set_block(-1, 9, BlockType::Torch);
        w.set_block(-1, 10, BlockType::Air);
        assert_eq!(w.get_block(-1, 9), BlockType::Air);
        assert_eq!(w.recent_environment_drops, vec![(-1, 9, ItemType::Torch)]);
        w.set_block(-1, 10, BlockType::Air);
        assert_eq!(w.recent_environment_drops.len(), 1);
        w.recent_environment_drops.clear();
        w.set_block(-1, 9, BlockType::Stone);
        w.set_block(0, 9, BlockType::Torch);
        w.set_block(-1, 9, BlockType::Air);
        assert_eq!(w.get_block(0, 9), BlockType::Air);
        assert_eq!(w.recent_environment_drops, vec![(0, 9, ItemType::Torch)]);
    }
    #[test]
    fn enclosed_back_wall_support_disappears_when_the_roof_is_broken_or_replaced() {
        for replacement in [BlockType::Air, BlockType::Stone] {
            let mut w = empty();
            for x in 1..=9 {
                w.set_block(x, 16, BlockType::Planks);
                w.set_block(x, 20, BlockType::Stone);
            }
            for x in [1, 9] {
                for y in 17..20 {
                    w.set_block(x, y, BlockType::Wood);
                }
            }
            w.set_block(4, 18, BlockType::Torch);
            assert_eq!(w.torch_attachment(4, 18), Some(TorchAttachment::BackWall));
            w.set_block(5, 16, replacement);
            assert_eq!(w.get_block(4, 18), BlockType::Air);
            assert_eq!(w.recent_environment_drops, vec![(4, 18, ItemType::Torch)]);
        }
    }
    #[test]
    fn legacy_load_checks_wait_for_neighbor_data_and_drop_unmounted_torches() {
        let mut w = empty();
        w.chunks
            .get_mut(&0)
            .unwrap()
            .set_block(31, 9, BlockType::Torch);
        w.chunks.remove(&1);
        w.queue_loaded_torch_checks(0);
        w.check_loaded_torch_attachments();
        assert_eq!(w.get_block(31, 9), BlockType::Torch);
        w.apply_chunk_column_snapshot(1, &vec![BlockType::Air; CHUNK_WIDTH * CHUNK_HEIGHT]);
        w.set_block(32, 9, BlockType::Stone);
        w.check_loaded_torch_attachments();
        assert_eq!(w.get_block(31, 9), BlockType::Torch);
        assert!(w.pending_torch_checks.is_empty());
        w.chunks
            .get_mut(&0)
            .unwrap()
            .set_block(0, 9, BlockType::Torch);
        w.queue_loaded_torch_checks(0);
        w.update(0);
        assert_eq!(w.get_block(0, 9), BlockType::Air);
        assert_eq!(w.recent_environment_drops, vec![(0, 9, ItemType::Torch)]);
    }
    #[test]
    fn freshly_generated_village_and_stronghold_lights_have_support() {
        let (p, t, m, c) = (
            noise::Perlin::new(1337),
            noise::Perlin::new(7331),
            noise::Perlin::new(9917),
            noise::Perlin::new(4444),
        );
        for center in [-8224, crate::world::STRONGHOLD_CENTER_X] {
            let mut w = empty();
            for cx in (center - 128).div_euclid(CHUNK_WIDTH as i32)
                ..=(center + 128).div_euclid(CHUNK_WIDTH as i32)
            {
                w.chunks.insert(
                    cx,
                    World::build_chunk_with_noise(
                        cx,
                        Dimension::Overworld,
                        "torch-generation",
                        &p,
                        &t,
                        &m,
                        &c,
                    ),
                );
            }
            let mut count = 0;
            for x in center - 100..=center + 100 {
                for y in 1..CHUNK_HEIGHT as i32 - 1 {
                    if w.get_block(x, y) == BlockType::Torch {
                        count += 1;
                        assert!(
                            w.torch_attachment(x, y).is_some(),
                            "unmounted generated torch at {x},{y}"
                        );
                    }
                }
            }
            assert!(count > 0);
        }
    }
}
