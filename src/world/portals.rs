//! The same bounded frame rules govern ignition, travel and frame destruction.
use super::{BlockType, CHUNK_HEIGHT, CHUNK_WIDTH, World};

impl World {
    pub(crate) fn end_portal_frame_at(&self, inner_x: i32, inner_y: i32) -> bool {
        if inner_y < 1 || inner_y + 2 >= CHUNK_HEIGHT as i32 {
            return false;
        }
        for x in inner_x - 1..=inner_x + 2 {
            for y in inner_y - 1..=inner_y + 2 {
                let block = self.get_block(x, y);
                if x == inner_x - 1 || x == inner_x + 2 || y == inner_y - 1 || y == inner_y + 2 {
                    if block != (BlockType::EndPortalFrame { filled: true }) {
                        return false;
                    }
                } else if !matches!(block, BlockType::Air | BlockType::EndPortal) {
                    return false;
                }
            }
        }
        true
    }

    pub(crate) fn nether_portal_frame_at(&self, inner_x: i32, inner_y: i32) -> bool {
        if inner_y < 1 || inner_y + 3 >= CHUNK_HEIGHT as i32 {
            return false;
        }
        // A 2×3 opening needs ten obsidian blocks; corner blocks are optional.
        for x in inner_x..=inner_x + 1 {
            if self.get_block(x, inner_y - 1) != BlockType::Obsidian
                || self.get_block(x, inner_y + 3) != BlockType::Obsidian
            {
                return false;
            }
        }
        for y in inner_y..=inner_y + 2 {
            if self.get_block(inner_x - 1, y) != BlockType::Obsidian
                || self.get_block(inner_x + 2, y) != BlockType::Obsidian
            {
                return false;
            }
            for x in inner_x..=inner_x + 1 {
                if !matches!(
                    self.get_block(x, y),
                    BlockType::Air | BlockType::NetherPortal
                ) {
                    return false;
                }
            }
        }
        true
    }

    /// Return (left interior column, floor row) only for a complete active portal.
    pub fn nether_portal_anchor_at(&self, x: i32, y: i32) -> Option<(i32, i32)> {
        if self.get_block(x, y) != BlockType::NetherPortal {
            return None;
        }
        // Fixed-size candidates also bound the work for malformed saved tiles.
        for inner_x in x - 1..=x {
            for inner_y in y - 2..=y {
                if self.nether_portal_frame_at(inner_x, inner_y)
                    && (inner_x..=inner_x + 1).all(|px| {
                        (inner_y..=inner_y + 2)
                            .all(|py| self.get_block(px, py) == BlockType::NetherPortal)
                    })
                {
                    return Some((inner_x, inner_y + 3));
                }
            }
        }
        None
    }

    pub(super) fn prune_broken_nether_portals_near(&mut self, x: i32, y: i32) {
        let mut broken = Vec::new();
        for px in x - 3..=x + 3 {
            for py in y - 4..=y + 4 {
                if self.get_block(px, py) == BlockType::NetherPortal
                    && self.nether_portal_anchor_at(px, py).is_none()
                {
                    broken.push((px, py));
                }
            }
        }
        self.clear_portal_tiles(broken);
    }

    pub(super) fn prune_broken_end_portals_near(&mut self, x: i32, y: i32) {
        let mut broken = Vec::new();
        for inner_x in x - 2..=x + 1 {
            for inner_y in y - 2..=y + 1 {
                let on_frame =
                    x == inner_x - 1 || x == inner_x + 2 || y == inner_y - 1 || y == inner_y + 2;
                if on_frame
                    && !self.end_portal_frame_at(inner_x, inner_y)
                    && (inner_x..=inner_x + 1).all(|px| {
                        (inner_y..=inner_y + 1)
                            .all(|py| self.get_block(px, py) == BlockType::EndPortal)
                    })
                {
                    for px in inner_x..=inner_x + 1 {
                        for py in inner_y..=inner_y + 1 {
                            broken.push((px, py));
                        }
                    }
                }
            }
        }
        self.clear_portal_tiles(broken);
    }

    fn clear_portal_tiles(&mut self, broken: Vec<(i32, i32)>) {
        // Remove atomically without recursively scanning neighbouring cells.
        for (px, py) in broken {
            let chunk_x = px.div_euclid(CHUNK_WIDTH as i32);
            if let Some(chunk) = self.chunks.get_mut(&chunk_x) {
                chunk.set_block(
                    px.rem_euclid(CHUNK_WIDTH as i32) as usize,
                    py as usize,
                    BlockType::Air,
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn active_frames_are_consistent_across_chunk_seams_and_optional_corners() {
        for inner_x in [-33, -32, -1, 0, 30, 31, 32] {
            let mut world = crate::engine::state::GameState::network_view().world;
            let air = vec![BlockType::Air; CHUNK_WIDTH * CHUNK_HEIGHT];
            for chunk in -2..=2 {
                world.apply_chunk_column_snapshot(chunk, &air);
            }
            for x in inner_x - 1..=inner_x + 2 {
                for y in 8..=12 {
                    if x == inner_x - 1 || x == inner_x + 2 || y == 8 || y == 12 {
                        world.set_block(x, y, BlockType::Obsidian);
                    }
                }
            }
            for x in inner_x..=inner_x + 1 {
                for y in 9..=11 {
                    world.set_block(x, y, BlockType::NetherPortal);
                }
            }
            for (x, y) in [
                (inner_x - 1, 8),
                (inner_x + 2, 8),
                (inner_x - 1, 12),
                (inner_x + 2, 12),
            ] {
                world.set_block(x, y, BlockType::Air);
            }
            assert!(world.nether_portal_frame_at(inner_x, 9));
            for x in inner_x..=inner_x + 1 {
                for y in 9..=11 {
                    assert_eq!(world.nether_portal_anchor_at(x, y), Some((inner_x, 12)));
                }
            }
            world.set_block(inner_x, 10, BlockType::Air);
            for x in inner_x..=inner_x + 1 {
                for y in 9..=11 {
                    assert_eq!(world.get_block(x, y), BlockType::Air);
                    assert_eq!(world.nether_portal_anchor_at(x, y), None);
                }
            }
        }
    }
}
