//! Simulation-side light queries use the renderer's emission and attenuation rules.
use super::{BlockType, CHUNK_HEIGHT, World};
use std::{cmp::Reverse, collections::BinaryHeap};

impl BlockType {
    pub fn emitted_light(self) -> u8 {
        match self {
            Self::Torch | Self::Lava(_) | Self::Glowstone => 15,
            Self::LitRedstoneOre(_) => 10,
            Self::Fire(_) => 14,
            Self::PrimedTnt(_) => 13,
            Self::RedstoneTorch(true) | Self::NetherPortal => 11,
            Self::EndPortal => 10,
            _ => 0,
        }
    }
}

impl World {
    pub fn has_open_sky(&self, x: i32, y: i32) -> bool {
        (0..y.clamp(0, CHUNK_HEIGHT as i32)).all(|by| {
            let block = self.get_block(x, by);
            !block.is_solid() || block == BlockType::Glass
        })
    }

    /// Brightest artificial light at a cell, including light around corners.
    /// Reverse shortest paths preserve destination attenuation (air 1, solid 3).
    /// No source can contribute from beyond 14 cells, so work is strictly bounded.
    pub fn block_light_at(&self, x: i32, y: i32) -> u8 {
        self.light_at(x, y, 0)
    }

    /// Artificial light plus daylight entering caves/windows, with bounded propagation.
    pub fn light_at(&self, x: i32, y: i32, skylight: u8) -> u8 {
        const WIDTH: usize = 31;
        let mut sky_floor = [CHUNK_HEIGHT as i32; WIDTH];
        if skylight > 0 {
            for (column, floor) in sky_floor.iter_mut().enumerate() {
                let wx = x + column as i32 - 15;
                *floor = (0..CHUNK_HEIGHT as i32)
                    .find(|&wy| {
                        let block = self.get_block(wx, wy);
                        block.is_solid() && block != BlockType::Glass
                    })
                    .unwrap_or(CHUNK_HEIGHT as i32);
            }
        }
        let mut costs = [u8::MAX; WIDTH * WIDTH];
        let center = 15 * WIDTH + 15;
        costs[center] = 0;
        let mut queue = BinaryHeap::from([Reverse((0u8, center))]);
        let mut brightest = 0;
        while let Some(Reverse((cost, index))) = queue.pop() {
            if cost != costs[index] {
                continue;
            }
            if 15u8.saturating_sub(cost) <= brightest {
                break;
            }
            let wx = x + (index % WIDTH) as i32 - 15;
            let wy = y + (index / WIDTH) as i32 - 15;
            if !(0..CHUNK_HEIGHT as i32).contains(&wy) {
                continue;
            }
            let block = self.get_block(wx, wy);
            let emission = block.emitted_light().max(if wy < sky_floor[index % WIDTH] {
                skylight.min(15)
            } else {
                0
            });
            brightest = brightest.max(emission.saturating_sub(cost));
            let next_cost = cost + if block.is_solid() { 3 } else { 1 };
            if next_cost >= 15 {
                continue;
            }
            for next in [
                index.checked_sub(WIDTH),
                (index + WIDTH < costs.len()).then_some(index + WIDTH),
                (index % WIDTH > 0).then(|| index - 1),
                (index % WIDTH + 1 < WIDTH).then_some(index + 1),
            ]
            .into_iter()
            .flatten()
            {
                if next_cost < costs[next] {
                    costs[next] = next_cost;
                    queue.push(Reverse((next_cost, next)));
                }
            }
        }
        brightest
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn daylight_reaches_under_a_cave_lip_without_passing_through_the_roof() {
        let mut world = World::network_replica(super::super::Dimension::Overworld);
        for cx in -2..=2 {
            world.apply_chunk_column_snapshot(
                cx,
                &vec![BlockType::Air; super::super::CHUNK_WIDTH * CHUNK_HEIGHT],
            );
        }
        for x in 0..=48 {
            for y in 0..20 {
                world.set_block(x, y, BlockType::Stone);
            }
        }
        assert!(world.light_at(1, 21, 15) > 0);
        assert_eq!(world.light_at(20, 21, 15), 0);
        assert_eq!(world.block_light_at(1, 21), 0);
    }

    #[test]
    fn light_falls_off_through_air_and_stone_and_tracks_source_removal() {
        let mut world = World::network_replica(super::super::Dimension::Overworld);
        let blocks = vec![BlockType::Air; super::super::CHUNK_WIDTH * CHUNK_HEIGHT];
        assert!(world.apply_chunk_column_snapshot(0, &blocks));
        world.set_block(0, 20, BlockType::Glowstone);
        assert_eq!(world.block_light_at(0, 20), 15);
        assert_eq!(world.block_light_at(4, 20), 11);
        for x in 1..=4 {
            for y in 0..CHUNK_HEIGHT as i32 {
                world.set_block(x, y, BlockType::Stone);
            }
        }
        assert_eq!(world.block_light_at(5, 20), 2);
        world.set_block(0, 20, BlockType::Air);
        assert_eq!(world.block_light_at(5, 20), 0);
    }
}
