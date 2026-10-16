//! Hydrostatic targets for source-fed basins. Solve geometry separately from
//! actual flow: a target can only fill from wet neighbours, bottom upward.
use super::super::{BlockType, CHUNK_HEIGHT, CHUNK_WIDTH, World};
use std::cmp::Reverse;
use std::collections::{BinaryHeap, HashMap};

#[derive(Default)]
pub(super) struct WaterBasins {
    targets: HashMap<i32, Vec<u8>>,
}

fn neighbours(i: usize, len: usize) -> impl Iterator<Item = usize> {
    let y = i % CHUNK_HEIGHT;
    [
        (y > 0).then(|| i - 1),
        (y + 1 < CHUNK_HEIGHT).then_some(i + 1),
        (i >= CHUNK_HEIGHT).then(|| i - CHUNK_HEIGHT),
        (i + CHUNK_HEIGHT < len).then_some(i + CHUNK_HEIGHT),
    ]
    .into_iter()
    .flatten()
}

impl WaterBasins {
    pub(super) fn solve(world: &World, center_chunk: i32) -> Self {
        let mut chunks: Vec<_> = world.chunks.keys().copied().collect();
        chunks.sort_unstable();
        let mut result = Self::default();
        // Separate distant co-op regions instead of allocating the space between
        // players. Include resident geometry outside the ticking window, so
        // walking across that window does not sever a still-loaded source.
        // Unknown edges are outlets, never invisible dams.
        let mut start = 0;
        while start < chunks.len() {
            let mut end = start + 1;
            while end < chunks.len() && chunks[end] == chunks[end - 1] + 1 {
                end += 1;
            }
            if chunks[start..end]
                .iter()
                .any(|cx| world.simulation_chunk_active(*cx, center_chunk))
            {
                result.solve_region(world, &chunks[start..end]);
            }
            start = end;
        }
        result
    }

    fn solve_region(&mut self, world: &World, chunks: &[i32]) {
        let width = chunks.len() * CHUNK_WIDTH;
        let len = width * CHUNK_HEIGHT;
        let mut passable = vec![false; len];
        let mut heads = vec![u8::MAX; len];
        let mut supply = BinaryHeap::new();
        for (cx, chunk_x) in chunks.iter().enumerate() {
            let chunk = &world.chunks[chunk_x];
            for x in 0..CHUNK_WIDTH {
                for y in 0..CHUNK_HEIGHT {
                    let i = (cx * CHUNK_WIDTH + x) * CHUNK_HEIGHT + y;
                    let block = chunk.get_block(x, y);
                    passable[i] = block.is_replaceable() && !matches!(block, BlockType::Lava(_));
                    if block == BlockType::Water(8) {
                        heads[i] = y as u8;
                        supply.push(Reverse((y as u8, i)));
                    }
                }
            }
        }
        if supply.is_empty() {
            return;
        }
        // Highest reachable source surface, through passages at/below that
        // surface. Air is considered for reach, but does not itself become wet.
        while let Some(Reverse((head, i))) = supply.pop() {
            if heads[i] != head {
                continue;
            }
            for ni in neighbours(i, len) {
                if passable[ni] && ni % CHUNK_HEIGHT >= usize::from(head) && head < heads[ni] {
                    heads[ni] = head;
                    supply.push(Reverse((head, ni)));
                }
            }
        }
        // Widest path to an outlet: the lowest escape crest limits each local
        // basin independently, including terraced pools and submerged tunnels.
        // Screen y grows downward, hence max/min rather than min/max.
        let mut escape = vec![-1i16; len];
        let mut outlets = BinaryHeap::new();
        for x in 0..width {
            for y in 0..CHUNK_HEIGHT {
                let i = x * CHUNK_HEIGHT + y;
                if passable[i] && (x == 0 || x + 1 == width || y + 1 == CHUNK_HEIGHT) {
                    escape[i] = y as i16;
                    outlets.push((y as i16, i));
                }
            }
        }
        while let Some((crest, i)) = outlets.pop() {
            if escape[i] != crest {
                continue;
            }
            for ni in neighbours(i, len) {
                let next = crest.min((ni % CHUNK_HEIGHT) as i16);
                if passable[ni] && next > escape[ni] {
                    escape[ni] = next;
                    outlets.push((next, ni));
                }
            }
        }
        for (cx, chunk_x) in chunks.iter().enumerate() {
            let mut targets = vec![0; CHUNK_WIDTH * CHUNK_HEIGHT];
            for x in 0..CHUNK_WIDTH {
                for y in 0..CHUNK_HEIGHT {
                    let i = (cx * CHUNK_WIDTH + x) * CHUNK_HEIGHT + y;
                    if passable[i]
                        && heads[i] != u8::MAX
                        && y >= usize::from(heads[i])
                        && (y as i16) >= escape[i]
                    {
                        targets[y * CHUNK_WIDTH + x] = if y as i16 == escape[i] { 7 } else { 9 };
                    }
                }
            }
            self.targets.insert(*chunk_x, targets);
        }
    }

    pub(super) fn target(&self, x: i32, y: i32) -> u8 {
        if !(0..CHUNK_HEIGHT as i32).contains(&y) {
            return 0;
        }
        self.targets
            .get(&x.div_euclid(CHUNK_WIDTH as i32))
            .map_or(0, |cells| {
                cells[y as usize * CHUNK_WIDTH + x.rem_euclid(CHUNK_WIDTH as i32) as usize]
            })
    }
}
