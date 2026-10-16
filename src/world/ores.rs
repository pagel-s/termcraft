//! Small seeded deposits shared by ordinary and recipe terrain.
//!
//! Broad noise lobes made many neighbouring pockets the same ore. One independently
//! chosen deposit per jittered cell keeps local abundance bounded as well as vein
//! size. Global coordinates (including negatives) keep veins intact at chunk seams.
use super::{
    block::BlockType,
    chunk::{CHUNK_HEIGHT, CHUNK_WIDTH, Chunk},
};

// Values are deposit probabilities per 10×8 cell, not percentages of blocks.
// World Y increases downwards; all peaks are inside the 128-row world.
const COAL: &[(i32, f64)] = &[
    (0, 0.12),
    (38, 0.26),
    (64, 0.28),
    (82, 0.22),
    (104, 0.12),
    (116, 0.04),
    (126, 0.025),
];
const IRON: &[(i32, f64)] = &[
    (38, 0.0),
    (48, 0.12),
    (72, 0.26),
    (90, 0.25),
    (104, 0.18),
    (116, 0.08),
    (126, 0.045),
];
const REDSTONE: &[(i32, f64)] = &[
    (68, 0.0),
    (84, 0.025),
    (104, 0.06),
    (116, 0.10),
    (126, 0.10),
];
const GOLD: &[(i32, f64)] = &[
    (83, 0.0),
    (94, 0.025),
    (110, 0.055),
    (120, 0.07),
    (126, 0.06),
];
const DIAMOND: &[(i32, f64)] = &[(105, 0.0), (110, 0.025), (118, 0.07), (126, 0.055)];

fn mix(mut value: u64) -> u64 {
    value = (value ^ (value >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    value = (value ^ (value >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    value ^ (value >> 31)
}

fn unit(value: u64) -> f64 {
    (value >> 40) as f64 / (1u32 << 24) as f64
}

fn weight(y: i32, points: &[(i32, f64)]) -> f64 {
    if y < points[0].0 {
        return 0.0;
    }
    for pair in points.windows(2) {
        let [(start, a), (end, b)] = pair else {
            unreachable!()
        };
        if y < *end {
            let t = f64::from(y - start) / f64::from(end - start);
            return a + (b - a) * t;
        }
    }
    points.last().unwrap().1
}

pub(super) fn at(seed: u32, x: i32, y: i32, exposed: bool) -> Option<BlockType> {
    if !(0..CHUNK_HEIGHT as i32 - 1).contains(&y) {
        return None;
    }
    let shift_x = i64::from(seed % 97);
    let shift_y = i64::from((seed >> 8) % 8);
    let cell_x = (i64::from(x) + shift_x).div_euclid(10);
    let cell_y = (i64::from(y) + shift_y).div_euclid(8);
    let hash = mix(u64::from(seed)
        ^ (cell_x as u64).wrapping_mul(0x9e37_79b9_7f4a_7c15)
        ^ (cell_y as u64).wrapping_mul(0xd1b5_4a32_d192_ed03));
    // At least one clear block around every cell; seed-dependent positions,
    // rotation and length avoid a repeated rectangular pocket silhouette.
    let center_x = cell_x * 10 - shift_x + 3 + (hash % 4) as i64;
    let center_y = (cell_y * 8 - shift_y + 3 + ((hash >> 3) % 2) as i64) as i32;
    let weights = [COAL, IRON, REDSTONE, GOLD, DIAMOND].map(|p| weight(center_y, p));
    let total: f64 = weights.iter().sum();
    let mut choice = unit(mix(hash ^ 0x243f_6a88_85a3_08d3)) * total;
    let kind = weights.iter().position(|w| {
        choice -= w;
        choice < 0.0
    })?;
    let (ore, minimum_y, min_size, sizes) = match kind {
        0 => (BlockType::CoalOre, 0, 5, 8),
        1 => (BlockType::IronOre, 38, 3, 6),
        2 => (BlockType::RedstoneOre, 68, 3, 4),
        3 => (BlockType::GoldOre, 83, 2, 4),
        _ => (BlockType::DiamondOre, 105, 2, 3),
    };
    let exposure = if !exposed {
        1.0
    } else if kind < 2 {
        1.25
    } else {
        // Make small rare finds discoverable from caves without broad ore fields.
        3.0
    };
    if y < minimum_y || unit(mix(hash ^ 0x1319_8a2e_0370_7344)) >= total * exposure {
        return None;
    }
    // Every prefix is connected; cave carving may split or shorten it naturally.
    const SHAPES: [[(i64, i32); 12]; 2] = [
        [
            (0, 0),
            (1, 0),
            (0, 1),
            (-1, 0),
            (0, -1),
            (1, 1),
            (-1, 1),
            (1, -1),
            (-1, -1),
            (2, 0),
            (-2, 0),
            (0, 2),
        ],
        [
            (0, 0),
            (1, 0),
            (1, 1),
            (0, 1),
            (-1, 0),
            (-1, -1),
            (0, -1),
            (1, -1),
            (-1, 1),
            (2, 0),
            (2, 1),
            (-2, 0),
        ],
    ];
    let size = min_size + (mix(hash ^ 0xa409_3822_299f_31d0) % sizes) as usize;
    let shape = &SHAPES[((hash >> 14) % 2) as usize];
    for &(dx, dy) in &shape[..size] {
        let (dx, dy) = match (hash >> 12) % 4 {
            0 => (dx, dy),
            1 => (-i64::from(dy), dx as i32),
            2 => (-dx, -dy),
            _ => (i64::from(dy), -dx as i32),
        };
        if i64::from(x) == center_x + dx && y == center_y + dy {
            return Some(ore);
        }
    }
    None
}

pub(super) fn expose_cave_walls(
    chunk: &mut Chunk,
    seed: u32,
    surfaces: &[i32; CHUNK_WIDTH],
    surface_water: &[bool; CHUNK_WIDTH],
) {
    for x in 0..CHUNK_WIDTH {
        if surface_water[x] {
            continue;
        }
        for y in
            (surfaces[x] + 7).max(super::OVERWORLD_VISIBLE_ORE_MIN_Y) as usize..CHUNK_HEIGHT - 1
        {
            if chunk.get_block(x, y) != BlockType::Stone {
                continue;
            }
            let exposed = chunk.get_block(x, y - 1) == BlockType::Air
                || chunk.get_block(x, y + 1) == BlockType::Air
                || (x > 0 && chunk.get_block(x - 1, y) == BlockType::Air)
                || (x + 1 < CHUNK_WIDTH && chunk.get_block(x + 1, y) == BlockType::Air);
            if exposed
                && let Some(ore) = at(
                    seed,
                    chunk.x * CHUNK_WIDTH as i32 + x as i32,
                    y as i32,
                    true,
                )
            {
                chunk.set_block(x, y, ore);
            }
        }
    }
}
