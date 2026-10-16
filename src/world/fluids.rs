//! Snapshot-based, gravity-first block fluids. Level 8 is a persistent source;
//! levels 1–7 are supplied flows, not independent pools.

use super::{BlockType, CHUNK_HEIGHT, CHUNK_WIDTH, Dimension, OVERWORLD_LAVA_TICK_INTERVAL, World};
use std::collections::HashSet;

fn level(block: BlockType, water: bool) -> Option<u8> {
    match (water, block) {
        (true, BlockType::Water(n)) | (false, BlockType::Lava(n)) => Some(n.min(8)),
        _ => None,
    }
}

fn fluid(water: bool, amount: u8) -> BlockType {
    if amount == 0 {
        BlockType::Air
    } else if water {
        BlockType::Water(amount)
    } else {
        BlockType::Lava(amount)
    }
}

impl World {
    fn fluid_cell_loaded(&self, x: i32, y: i32) -> bool {
        (0..CHUNK_HEIGHT as i32).contains(&y)
            && self.chunks.contains_key(&x.div_euclid(CHUNK_WIDTH as i32))
    }

    fn fluid_can_enter(&self, x: i32, y: i32, water: bool) -> bool {
        if !self.fluid_cell_loaded(x, y) {
            return false;
        }
        let target = self.get_block(x, y);
        target.is_replaceable()
            && (!target.is_fluid() || level(target, water).is_some_and(|n| n < 8))
    }

    fn fluid_has_floor(&self, x: i32, y: i32, water: bool) -> bool {
        y + 1 == CHUNK_HEIGHT as i32
            || !self.get_block(x, y + 1).is_replaceable()
            || level(self.get_block(x, y + 1), water) == Some(8)
    }

    fn min_horizontal_fluid_level(&self, water: bool) -> u8 {
        if water || self.dimension == Dimension::Nether {
            1
        } else {
            5
        }
    }

    // Prefer the closest reachable drop. On a flat floor both directions are
    // allowed. A falling column never becomes a sideways source at every row.
    fn fluid_spreads_toward(&self, x: i32, y: i32, water: bool, direction: i32) -> bool {
        let Some(amount) = level(self.get_block(x, y), water) else {
            return false;
        };
        if !self.fluid_has_floor(x, y, water) {
            return false;
        }
        let reach = amount.saturating_sub(self.min_horizontal_fluid_level(water));
        let drop_distance = |sign: i32| {
            for distance in 1..=i32::from(reach) {
                let nx = x + sign * distance;
                if !self.fluid_can_enter(nx, y, water) {
                    break;
                }
                if self.fluid_can_enter(nx, y + 1, water) {
                    return Some(distance);
                }
            }
            None
        };
        match (drop_distance(direction), drop_distance(-direction)) {
            (Some(ours), Some(other)) => ours <= other,
            (None, Some(_)) => false,
            _ => true,
        }
    }

    fn can_form_infinite_water_source(&self, x: i32, y: i32) -> bool {
        self.fluid_has_floor(x, y, true)
            && self.get_block(x - 1, y) == BlockType::Water(8)
            && self.get_block(x + 1, y) == BlockType::Water(8)
    }

    fn supplied_fluid_level(&self, x: i32, y: i32, water: bool) -> u8 {
        if level(self.get_block(x, y - 1), water).is_some_and(|n| n > 0) {
            return 7;
        }
        [-1, 1]
            .into_iter()
            .filter_map(|direction| {
                let nx = x + direction;
                let amount = level(self.get_block(nx, y), water)?;
                (amount > self.min_horizontal_fluid_level(water)
                    && self.fluid_spreads_toward(nx, y, water, -direction))
                .then_some(amount.saturating_sub(1))
            })
            .max()
            .unwrap_or(0)
    }

    fn water_cools_lava_at(&self, x: i32, y: i32) -> bool {
        [(x - 1, y), (x + 1, y), (x, y - 1)]
            .into_iter()
            .any(|(nx, ny)| matches!(self.get_block(nx, ny), BlockType::Water(_)))
    }

    fn next_fluid_block(&self, x: i32, y: i32, lava_due: bool) -> BlockType {
        let current = self.get_block(x, y);
        if let BlockType::Lava(amount) = current {
            if self.water_cools_lava_at(x, y) {
                return if amount == 8 {
                    BlockType::Obsidian
                } else {
                    BlockType::Cobblestone
                };
            }
            if !lava_due {
                return current;
            }
        }
        // Cooling the lava itself wins over also consuming water beneath it.
        if matches!(current, BlockType::Water(_))
            && lava_due
            && matches!(self.get_block(x, y - 1), BlockType::Lava(_))
            && !self.water_cools_lava_at(x, y - 1)
        {
            return BlockType::Stone;
        }
        if !current.is_replaceable() {
            return current;
        }
        if current == BlockType::Water(8) || current == BlockType::Lava(8) {
            return current;
        }
        if !matches!(current, BlockType::Lava(_)) && self.can_form_infinite_water_source(x, y) {
            return BlockType::Water(8);
        }
        let water = if !matches!(current, BlockType::Lava(_)) {
            self.supplied_fluid_level(x, y, true)
                .max(level(current, true).unwrap_or(0).saturating_sub(1))
        } else {
            0
        };
        let lava = if lava_due && !matches!(current, BlockType::Water(_)) {
            self.supplied_fluid_level(x, y, false)
                .max(level(current, false).unwrap_or(0).saturating_sub(1))
        } else {
            0
        };
        match (water, lava) {
            (1.., 1..) => BlockType::Cobblestone,
            (1.., _) => fluid(true, water),
            (_, 1..) => fluid(false, lava),
            _ if current.is_fluid() => BlockType::Air,
            _ => current,
        }
    }

    pub(super) fn update_fluids(&mut self, center_chunk: i32) {
        let scheduled: Vec<_> = self
            .active_fluid_chunks
            .iter()
            .copied()
            .filter(|cx| self.simulation_chunk_active(*cx, center_chunk))
            .collect();
        let lava_due = self.dimension == Dimension::Nether
            || self
                .tick_counter
                .is_multiple_of(OVERWORLD_LAVA_TICK_INTERVAL);
        let mut candidates = HashSet::new();
        let mut waiting_lava_chunks = HashSet::new();
        for cx in &scheduled {
            let Some(chunk) = self.chunks.get(cx).filter(|c| c.has_fluids()) else {
                continue;
            };
            for y in 0..CHUNK_HEIGHT {
                for x in 0..CHUNK_WIDTH {
                    let block = chunk.get_block(x, y);
                    if !block.is_fluid() {
                        continue;
                    }
                    let wx = cx * CHUNK_WIDTH as i32 + x as i32;
                    let wy = y as i32;
                    let water = matches!(block, BlockType::Water(_));
                    if !water && !lava_due {
                        waiting_lava_chunks.insert(*cx);
                    }
                    // Interiors of full source pools cannot change except by
                    // mixing. Keep them out of the expensive candidate set.
                    if level(block, water) != Some(8)
                        || (!water && self.water_cools_lava_at(wx, wy))
                        || (water && matches!(self.get_block(wx, wy - 1), BlockType::Lava(_)))
                    {
                        candidates.insert((wx, wy));
                    }
                    for (nx, ny) in [(wx - 1, wy), (wx + 1, wy), (wx, wy + 1)] {
                        let target = self.get_block(nx, ny);
                        if target.is_replaceable() && level(target, water) != Some(8) {
                            candidates.insert((nx, ny));
                        }
                    }
                }
            }
        }
        let mut candidates: Vec<_> = candidates
            .into_iter()
            .filter(|&(x, y)| {
                self.fluid_cell_loaded(x, y)
                    && self.simulation_chunk_active(x.div_euclid(CHUNK_WIDTH as i32), center_chunk)
            })
            .collect();
        candidates.sort_unstable();
        // Compute the entire next frame before writing any cells. No source,
        // drainage or mixing result depends on chunk/hash iteration order.
        let changes: Vec<_> = candidates
            .into_iter()
            .filter_map(|(x, y)| {
                let before = self.get_block(x, y);
                let after = self.next_fluid_block(x, y, lava_due);
                (before != after).then_some((x, y, before, after))
            })
            .collect();
        for cx in scheduled {
            if !waiting_lava_chunks.contains(&cx) {
                self.active_fluid_chunks.remove(&cx);
            }
        }
        for (x, y, before, after) in changes {
            if after.is_fluid() && !before.is_fluid() {
                self.drop_items_for_support_break(x, y, before);
            }
            self.set_block(x, y, after);
        }
    }

    /// Shared by rendering, immersion, breathing and buoyancy; y grows down.
    pub fn fluid_surface_y(&self, x: i32, y: i32) -> Option<f64> {
        let block = self.get_block(x, y);
        let (water, amount) = match block {
            BlockType::Water(n) => (true, n),
            BlockType::Lava(n) => (false, n),
            _ => return None,
        };
        let depth =
            if amount == 0 || amount >= 8 || level(self.get_block(x, y - 1), water).is_some() {
                1.0
            } else {
                f64::from(amount) / 8.0
            };
        Some(y as f64 + 1.0 - depth)
    }

    pub fn fluid_at_point(&self, x: f64, y: f64) -> BlockType {
        let bx = x.floor() as i32;
        let by = y.floor() as i32;
        if self
            .fluid_surface_y(bx, by)
            .is_some_and(|surface| y >= surface)
        {
            self.get_block(bx, by)
        } else {
            BlockType::Air
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::*;
    use super::fluid;

    fn scene(dimension: Dimension, reverse: bool) -> World {
        let mut world = World::network_replica(dimension);
        let mut chunks: Vec<_> = (-2..=2).collect();
        if reverse {
            chunks.reverse();
        }
        for chunk in chunks {
            world.apply_chunk_column_snapshot(
                chunk,
                &vec![BlockType::Air; CHUNK_WIDTH * CHUNK_HEIGHT],
            );
        }
        world
    }

    fn floor(world: &mut World, y: i32) {
        for x in -50..=50 {
            world.set_block(x, y, BlockType::Stone);
        }
    }

    #[test]
    fn waterfall_stays_vertical_until_it_lands() {
        for (dimension, source, falling) in [
            (
                Dimension::Overworld,
                BlockType::Water(8),
                BlockType::Water(7),
            ),
            (Dimension::Nether, BlockType::Lava(8), BlockType::Lava(7)),
        ] {
            let mut world = scene(dimension, false);
            floor(&mut world, 20);
            world.set_block(0, 5, source);
            for _ in 0..40 {
                world.update_fluids(0);
            }
            for y in 6..19 {
                assert_eq!(world.get_block(0, y), falling);
                for x in [-1, 1] {
                    assert_eq!(
                        world.get_block(x, y),
                        BlockType::Air,
                        "sideways waterfall at {x},{y}"
                    );
                }
            }
            assert!(world.get_block(1, 19).is_fluid(), "landing must spread");
        }
    }

    #[test]
    fn weak_flow_adjusts_to_its_actual_supply() {
        let mut world = scene(Dimension::Overworld, false);
        floor(&mut world, 30);
        world.set_block(0, 29, BlockType::Water(8));
        world.set_block(1, 29, BlockType::Water(1));
        world.update_fluids(0);
        assert_eq!(world.get_block(1, 29), BlockType::Water(7));
    }

    #[test]
    fn supported_flow_takes_the_nearest_drop_instead_of_spreading_both_ways() {
        let mut world = scene(Dimension::Overworld, false);
        floor(&mut world, 30);
        world.set_block(3, 30, BlockType::Air);
        world.set_block(0, 29, BlockType::Water(8));
        for _ in 0..12 {
            world.update_fluids(0);
        }
        assert_eq!(world.get_block(-1, 29), BlockType::Air);
        assert!(matches!(world.get_block(3, 31), BlockType::Water(_)));
    }

    #[test]
    fn water_cools_adjacent_lava_even_while_the_water_can_fall() {
        let mut world = scene(Dimension::Overworld, false);
        world.set_block(0, 10, BlockType::Water(8));
        world.set_block(1, 10, BlockType::Lava(8));
        world.update_fluids(0);
        assert_eq!(world.get_block(1, 10), BlockType::Obsidian);
        assert_eq!(world.get_block(0, 10), BlockType::Water(8));
    }

    #[test]
    fn one_source_drains_completely_after_removal_including_its_waterfall() {
        let mut world = scene(Dimension::Overworld, false);
        floor(&mut world, 25);
        world.set_block(0, 8, BlockType::Water(8));
        for _ in 0..50 {
            world.update_fluids(0);
        }
        world.set_block(0, 8, BlockType::Air);
        for _ in 0..160 {
            world.update_fluids(0);
        }
        for x in -30..=30 {
            for y in 0..25 {
                assert!(
                    !world.get_block(x, y).is_fluid(),
                    "orphaned fluid at {x},{y}"
                );
            }
        }
    }

    #[test]
    fn flow_and_mixing_match_every_tick_across_chunk_insertion_orders() {
        let mut a = scene(Dimension::Overworld, false);
        let mut b = scene(Dimension::Overworld, true);
        for world in [&mut a, &mut b] {
            floor(world, 30);
            world.set_block(31, 29, BlockType::Water(8));
            world.set_block(33, 29, BlockType::Lava(8));
            world.set_block(-32, 8, BlockType::Water(8));
        }
        for _ in 0..50 {
            a.update_fluids(0);
            b.update_fluids(0);
            for x in -48..=48 {
                for y in 5..=30 {
                    assert_eq!(
                        a.get_block(x, y),
                        b.get_block(x, y),
                        "order-dependent flow at {x},{y}"
                    );
                }
            }
        }
        assert_eq!(a.get_block(32, 29), BlockType::Cobblestone);
    }

    #[test]
    fn sources_regenerate_only_with_support_and_both_horizontal_sources() {
        let mut world = scene(Dimension::Overworld, false);
        world.set_block(-1, 10, BlockType::Water(8));
        world.set_block(1, 10, BlockType::Water(8));
        world.update_fluids(0);
        assert_eq!(world.get_block(0, 10), BlockType::Air);
        world.set_block(0, 11, BlockType::Stone);
        world.update_fluids(0);
        assert_eq!(world.get_block(0, 10), BlockType::Water(8));
    }

    #[test]
    fn water_reach_is_seven_and_overworld_lava_reach_is_three() {
        for (dimension, water, reach) in [
            (Dimension::Overworld, true, 7),
            (Dimension::Overworld, false, 3),
            (Dimension::Nether, false, 7),
        ] {
            let mut world = scene(dimension, false);
            floor(&mut world, 30);
            world.set_block(0, 29, fluid(water, 8));
            for _ in 0..20 {
                world.update_fluids(0);
            }
            assert!(world.get_block(reach, 29).is_fluid());
            assert!(world.get_block(-reach, 29).is_fluid());
            assert_eq!(world.get_block(reach + 1, 29), BlockType::Air);
            assert_eq!(world.get_block(-reach - 1, 29), BlockType::Air);
        }
    }

    #[test]
    fn blocked_source_removal_does_not_leave_a_hanging_sheet() {
        let mut world = scene(Dimension::Overworld, false);
        floor(&mut world, 30);
        world.set_block(0, 8, BlockType::Water(8));
        for _ in 0..35 {
            world.update_fluids(0);
        }
        world.set_block(0, 12, BlockType::Stone);
        for _ in 0..80 {
            world.update_fluids(0);
        }
        assert!(world.get_block(1, 11).is_fluid());
        assert_eq!(world.get_block(0, 13), BlockType::Air);
        assert_eq!(world.get_block(0, 12), BlockType::Stone);
    }

    #[test]
    fn water_washes_out_crops_once_without_erasing_the_soil() {
        let mut world = scene(Dimension::Overworld, false);
        floor(&mut world, 30);
        world.set_block(0, 29, BlockType::Water(8));
        world.set_block(1, 30, BlockType::Farmland(7));
        world.set_block(1, 29, BlockType::Crops(7));
        for _ in 0..12 {
            world.update_fluids(0);
        }
        assert!(matches!(world.get_block(1, 29), BlockType::Water(_)));
        assert_eq!(world.get_block(1, 30), BlockType::Farmland(7));
        assert_eq!(
            world
                .recent_environment_drops
                .iter()
                .filter(|(x, y, item)| *x == 1
                    && *y == 29
                    && *item == super::super::ItemType::Wheat)
                .count(),
            1
        );
    }

    #[test]
    fn fluid_surfaces_distinguish_puddles_and_full_falling_columns() {
        let mut world = scene(Dimension::Overworld, false);
        world.set_block(0, 10, BlockType::Water(1));
        assert_eq!(world.fluid_surface_y(0, 10), Some(10.875));
        assert_eq!(world.fluid_at_point(0.5, 10.5), BlockType::Air);
        assert_eq!(world.fluid_at_point(0.5, 10.9), BlockType::Water(1));
        world.set_block(0, 9, BlockType::Water(7));
        assert_eq!(world.fluid_surface_y(0, 10), Some(10.0));
        world.set_block(0, 9, BlockType::Air);
        world.set_block(0, 10, BlockType::Water(8));
        assert_eq!(world.fluid_surface_y(0, 10), Some(10.0));
    }

    #[test]
    fn legacy_zero_level_flows_decay_without_underflow_or_creating_sources() {
        for water in [true, false] {
            let mut world = scene(Dimension::Nether, false);
            for x in 0..3 {
                world.set_block(
                    x,
                    10,
                    if water {
                        BlockType::Water(0)
                    } else {
                        BlockType::Lava(0)
                    },
                );
            }
            world.update_fluids(0);
            for x in -1..4 {
                for y in 10..12 {
                    assert_eq!(world.get_block(x, y), BlockType::Air);
                }
            }
        }
    }

    #[test]
    fn a_door_that_blocks_fluid_also_supports_sideways_spill() {
        let mut world = scene(Dimension::Overworld, false);
        world.set_block(0, 22, BlockType::Stone);
        world.set_block(0, 21, BlockType::WoodDoor(true));
        world.set_block(0, 20, BlockType::WoodDoor(true));
        world.set_block(0, 19, BlockType::Water(8));
        world.update_fluids(0);
        assert_eq!(world.get_block(-1, 19), BlockType::Water(7));
        assert_eq!(world.get_block(1, 19), BlockType::Water(7));
        assert_eq!(world.get_block(0, 20), BlockType::WoodDoor(true));
    }
}
