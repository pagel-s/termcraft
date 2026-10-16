//! Local ignition and temporary ore emission, shared by native and hosted worlds.
use super::*;
use std::collections::BTreeMap;

const SIDES: [(i32, i32); 4] = [(0, -1), (-1, 0), (1, 0), (0, 1)];
impl World {
    pub fn activate_redstone_ore(&mut self, x: i32, y: i32) {
        if matches!(
            self.get_block(x, y),
            BlockType::RedstoneOre | BlockType::LitRedstoneOre(_)
        ) {
            self.set_block(x, y, BlockType::LitRedstoneOre(10));
        }
    }
    fn water_beside(&self, x: i32, y: i32) -> bool {
        SIDES
            .iter()
            .any(|(dx, dy)| matches!(self.get_block(x + dx, y + dy), BlockType::Water(_)))
    }
    pub(super) fn update_heat(&mut self, center: i32) {
        let mut hot = Vec::new();
        for (&cx, chunk) in &self.chunks {
            if !self.simulation_chunk_active(cx, center) || !chunk.has_heat_blocks() {
                continue;
            }
            // Lava oceans with no nearby fuel need no ignition scan. Counts
            // are maintained on edits/load, rather than rescanning cold chunks.
            if !chunk.has_timed_heat()
                && !(cx - 1..=cx + 1).any(|x| {
                    self.chunks
                        .get(&x)
                        .is_some_and(|c| c.has_flammable_blocks())
                })
            {
                continue;
            }
            for y in 0..CHUNK_HEIGHT {
                for x in 0..CHUNK_WIDTH {
                    let block = chunk.get_block(x, y);
                    if block.has_heat_tick() {
                        hot.push((cx * CHUNK_WIDTH as i32 + x as i32, y as i32, block));
                    }
                }
            }
        }
        hot.sort_unstable_by_key(|&(x, y, _)| (x, y));
        let mut changes = BTreeMap::new();
        for (x, y, block) in hot {
            match block {
                BlockType::LitRedstoneOre(n) => {
                    changes.insert(
                        (x, y),
                        if n > 1 {
                            BlockType::LitRedstoneOre(n - 1)
                        } else {
                            BlockType::RedstoneOre
                        },
                    );
                }
                BlockType::Lava(n) if n > 0 => {
                    for (dx, dy) in SIDES {
                        let (fx, fy) = (x + dx, y + dy);
                        if !self.get_block(fx, fy).is_flammable() || self.water_beside(fx, fy) {
                            continue;
                        }
                        let mut exposed = false;
                        for (ax, ay) in SIDES {
                            let p = (fx + ax, fy + ay);
                            if self.get_block(p.0, p.1) == BlockType::Air
                                && !self.water_beside(p.0, p.1)
                            {
                                changes.entry(p).or_insert(BlockType::Fire(8));
                                exposed = true;
                                break;
                            }
                        }
                        // Direct molten contact consumes submerged fuel too.
                        if !exposed
                            && SIDES.iter().all(|(ax, ay)| {
                                matches!(self.get_block(fx + ax, fy + ay), BlockType::Lava(_))
                            })
                        {
                            changes.entry((fx, fy)).or_insert(
                                if self.get_block(fx, fy) == BlockType::Tnt {
                                    BlockType::PrimedTnt(30)
                                } else {
                                    BlockType::Fire(8)
                                },
                            );
                        }
                    }
                }
                BlockType::Fire(n) => {
                    if self.water_beside(x, y) {
                        changes.insert((x, y), BlockType::Air);
                        continue;
                    }
                    let fuels: Vec<_> = SIDES
                        .iter()
                        .map(|(dx, dy)| (x + dx, y + dy))
                        .filter(|&(fx, fy)| self.get_block(fx, fy).is_flammable())
                        .collect();
                    if n == 4 {
                        for &(fx, fy) in &fuels {
                            changes.insert(
                                (fx, fy),
                                if self.get_block(fx, fy) == BlockType::Tnt {
                                    BlockType::PrimedTnt(30)
                                } else {
                                    BlockType::Air
                                },
                            );
                            // Spread into exposed air alongside fuel, never through walls.
                            for (dx, dy) in SIDES {
                                let p = (fx + dx, fy + dy);
                                if p != (x, y)
                                    && self.get_block(p.0, p.1) == BlockType::Air
                                    && !self.water_beside(p.0, p.1)
                                {
                                    changes.entry(p).or_insert(BlockType::Fire(8));
                                }
                            }
                        }
                    }
                    changes.insert(
                        (x, y),
                        if n > 1 && !fuels.is_empty() {
                            BlockType::Fire(n - 1)
                        } else if self.get_block(x, y + 1) == BlockType::Netherrack {
                            BlockType::Fire(8)
                        } else {
                            BlockType::Air
                        },
                    );
                }
                _ => {}
            }
        }
        for ((x, y), block) in changes {
            self.set_block(x, y, block);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn room() -> World {
        let mut world = World::network_replica(Dimension::Overworld);
        for cx in -1..=1 {
            world
                .apply_chunk_column_snapshot(cx, &vec![BlockType::Air; CHUNK_WIDTH * CHUNK_HEIGHT]);
        }
        world
    }
    #[test]
    fn ore_activation_expires_and_keeps_its_mining_and_drop_rules() {
        let mut w = room();
        w.set_block(3, 30, BlockType::RedstoneOre);
        w.activate_redstone_ore(3, 30);
        let lit = w.get_block(3, 30);
        assert_eq!(w.block_light_at(3, 30), 10);
        assert!(w.block_light_at(4, 30) > 0);
        assert!(matches!(lit, BlockType::LitRedstoneOre(_)));
        assert_eq!(lit.hardness(), BlockType::RedstoneOre.hardness());
        assert_eq!(
            lit.required_tool_level(),
            BlockType::RedstoneOre.required_tool_level()
        );
        assert_eq!(
            item::ItemType::from_block(lit),
            Some(item::ItemType::RedstoneDust)
        );
        for _ in 0..9 {
            w.update_heat(0);
        }
        assert!(matches!(w.get_block(3, 30), BlockType::LitRedstoneOre(_)));
        w.update_heat(0);
        assert_eq!(w.get_block(3, 30), BlockType::RedstoneOre);
        assert_eq!(w.block_light_at(4, 30), 0);
    }
    #[test]
    fn lava_ignites_exposed_wood_but_preserves_stone_and_chest_contents() {
        let mut w = room();
        w.set_block(1, 30, BlockType::Lava(8));
        w.set_block(2, 30, BlockType::Planks);
        w.set_block(0, 30, BlockType::Stone);
        w.set_block(1, 31, BlockType::Chest);
        w.chest_inventory_mut(1, 31)
            .unwrap()
            .add_item(item::ItemType::Diamond, 3);
        w.update_heat(0);
        assert!(matches!(w.get_block(2, 29), BlockType::Fire(_)));
        for _ in 0..6 {
            w.update_heat(0);
        }
        assert_eq!(w.get_block(2, 30), BlockType::Air);
        assert_eq!(w.get_block(0, 30), BlockType::Stone);
        assert!(
            w.chest_inventory(1, 31)
                .unwrap()
                .has_item(item::ItemType::Diamond, 3)
        );
    }
    #[test]
    fn water_extinguishes_fire_and_prevents_adjacent_ignition() {
        let mut w = room();
        w.set_block(1, 30, BlockType::Fire(4));
        w.set_block(1, 31, BlockType::Planks);
        w.set_block(2, 30, BlockType::Water(8));
        for _ in 0..10 {
            w.update_heat(0);
        }
        assert_eq!(w.get_block(1, 30), BlockType::Air);
        assert_eq!(w.get_block(1, 31), BlockType::Planks);
    }
    #[test]
    fn submerged_wood_is_consumed_by_direct_lava_contact() {
        let mut w = room();
        w.set_block(1, 30, BlockType::Planks);
        for (dx, dy) in SIDES {
            w.set_block(1 + dx, 30 + dy, BlockType::Lava(8));
        }
        w.update_heat(0);
        assert_ne!(w.get_block(1, 30), BlockType::Planks);
    }
}
