//! Deterministic settlements clipped into individual freshly generated chunks.
use super::*;
const SPACING: i32 = CHUNK_WIDTH as i32 * 24;
pub const VILLAGE_RADIUS: i32 = 56;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BuildingRole {
    Cottage,
    Workshop,
    Farmhouse,
}
#[derive(Clone, Debug)]
pub struct VillageSite {
    pub center_x: i32,
    pub ground_y: i32,
    pub biome: BiomeType,
    hash: u32,
    left_ground: i32,
    right_ground: i32,
}
impl VillageSite {
    pub fn buildings(&self) -> [(i32, i32, BuildingRole); 3] {
        [
            (
                self.center_x - 39,
                self.center_x - 27,
                BuildingRole::Cottage,
            ),
            (
                self.center_x - 16,
                self.center_x - 2,
                BuildingRole::Workshop,
            ),
            (
                self.center_x + 17,
                self.center_x + 29,
                BuildingRole::Farmhouse,
            ),
        ]
    }
    fn road_height(&self, x: i32) -> f64 {
        let d = x - self.center_x;
        let (distance, edge) = if d < 0 {
            (-d, self.left_ground)
        } else {
            (d, self.right_ground)
        };
        let t = ((distance - 47) as f64 / 9.0).clamp(0.0, 1.0);
        ((self.ground_y as f64 + (edge - self.ground_y) as f64 * t) * 2.0).round() / 2.0
    }
}
fn center_for_cell(cell: i32, seed: u32) -> Option<(i32, u32)> {
    let hash = (cell as u32).wrapping_mul(2_654_435_761) ^ seed.rotate_left(13);
    let jitter = ((hash >> 8) % 3) as i32 - 1;
    Some((
        cell.checked_mul(SPACING)?.checked_add(176 + jitter * 16)?,
        hash,
    ))
}
fn site_for_cell(
    cell: i32,
    terrain: &Perlin,
    temp: &Perlin,
    moist: &Perlin,
) -> Option<VillageSite> {
    let (base, hash) = center_for_cell(cell, terrain.seed())?;
    [0, -32, 32, -64, 64, -96, 96]
        .into_iter()
        .find_map(|offset| candidate_site(base + offset, hash, terrain, temp, moist))
}
fn candidate_site(
    center: i32,
    hash: u32,
    terrain: &Perlin,
    temp: &Perlin,
    moist: &Perlin,
) -> Option<VillageSite> {
    if (i64::from(center) - i64::from(STRONGHOLD_CENTER_X)).abs() < 160 {
        return None;
    }
    let biome = World::biome_for_x(temp, moist, center);
    if !matches!(
        biome,
        BiomeType::Plains
            | BiomeType::Forest
            | BiomeType::Taiga
            | BiomeType::Tundra
            | BiomeType::Desert
    ) {
        return None;
    }
    let mut heights = Vec::with_capacity((VILLAGE_RADIUS * 2 + 1) as usize);
    for x in center - VILLAGE_RADIUS..=center + VILLAGE_RADIUS {
        let b = World::biome_for_x(temp, moist, x);
        if matches!(
            b,
            BiomeType::Ocean | BiomeType::River | BiomeType::Swamp | BiomeType::ExtremeHills
        ) {
            return None;
        }
        let y = World::blended_overworld_surface_y(terrain, temp, moist, x, b);
        if !(16..=80).contains(&y) || World::column_has_surface_water(terrain, temp, moist, x, b, y)
        {
            return None;
        }
        heights.push(y);
    }
    let left_ground = heights[0];
    let right_ground = *heights.last()?;
    heights.sort_unstable();
    if heights.last()? - heights[0] > 12 {
        return None;
    }
    let low = left_ground.max(right_ground) - 4;
    let high = left_ground.min(right_ground) + 4;
    if low > high {
        return None;
    }
    let ground = heights[heights.len() / 2].clamp(low, high);
    Some(VillageSite {
        center_x: center,
        ground_y: ground,
        biome,
        hash,
        left_ground,
        right_ground,
    })
}
fn put(chunk: &mut Chunk, start: i32, x: i32, y: i32, b: BlockType) {
    if x >= start && x < start + CHUNK_WIDTH as i32 && y > 0 && y < CHUNK_HEIGHT as i32 - 1 {
        chunk.set_block((x - start) as usize, y as usize, b);
    }
}
fn building(
    chunk: &mut Chunk,
    start: i32,
    site: &VillageSite,
    left: i32,
    right: i32,
    role: BuildingRole,
) {
    let g = site.ground_y;
    let roof = g - 7;
    let desert = site.biome == BiomeType::Desert;
    let post = if role == BuildingRole::Farmhouse && !desert {
        BlockType::BirchWood
    } else if desert {
        BlockType::StoneBricks
    } else {
        BlockType::Wood
    };
    let ceiling = if desert || role == BuildingRole::Workshop {
        BlockType::StoneBricks
    } else {
        BlockType::Planks
    };
    for x in left..=right {
        put(chunk, start, x, g, BlockType::Planks);
        put(chunk, start, x, g + 1, BlockType::Cobblestone);
        put(chunk, start, x, roof, ceiling);
    }
    for x in [left, right] {
        for y in roof + 1..g {
            put(chunk, start, x, y, post);
        }
        for y in [g - 1, g - 2] {
            put(chunk, start, x, y, BlockType::WoodDoor(false));
        }
        // Real lintel/sill frame each upper window rather than a glass wall.
        put(chunk, start, x, g - 4, BlockType::Glass);
    }
    for x in [left - 1, right + 1] {
        put(chunk, start, x, roof, BlockType::StoneSlab);
    }
    let rise = 1;
    if !desert {
        for row in 1..=rise {
            for x in left + row * 2..=right - row * 2 {
                put(
                    chunk,
                    start,
                    x,
                    roof - row,
                    if x == left + row * 2 || x == right - row * 2 {
                        BlockType::StoneSlab
                    } else {
                        ceiling
                    },
                );
            }
        }
    } else {
        for x in [left, right] {
            put(chunk, start, x, roof - 1, BlockType::StoneSlab);
        }
    }
    // Furniture lives above the clear street-level corridor, with a reachable landing.
    let mirror = role == BuildingRole::Cottage;
    let at = |offset: i32| {
        if mirror {
            left + offset
        } else {
            right - offset
        }
    };
    for offset in 1..=4 {
        put(chunk, start, at(offset), g - 3, BlockType::Planks);
    }
    for y in g - 4..g {
        put(chunk, start, at(4), y, BlockType::Ladder);
    }
    let chest_x = at(1);
    put(chunk, start, chest_x, g - 4, BlockType::Chest);
    put(chunk, start, chest_x, g - 5, BlockType::Torch);
    if chest_x >= start && chest_x < start + CHUNK_WIDTH as i32 {
        World::fill_village_chest_loot(
            chunk,
            (chest_x - start) as usize,
            (g - 4) as usize,
            site.hash ^ left as u32,
        );
    }
    put(
        chunk,
        start,
        at(2),
        g - 4,
        if role == BuildingRole::Workshop {
            BlockType::CraftingTable
        } else {
            BlockType::Bed
        },
    );
    if role == BuildingRole::Workshop {
        put(chunk, start, at(3), g - 4, BlockType::Furnace);
        put(chunk, start, left + 3, roof - 1, BlockType::Cobblestone);
    }
    if role == BuildingRole::Farmhouse {
        put(chunk, start, left - 2, roof + 1, BlockType::StoneSlab);
        put(chunk, start, left - 1, roof + 1, BlockType::Planks);
    }
    put(chunk, start, left + 3, g - 2, BlockType::Torch);
}
impl World {
    /// Fresh-generation sites for review tooling; does not modify or claim saved terrain.
    pub fn village_sites_near(&self, x: i32, radius: i32) -> Vec<VillageSite> {
        if self.dimension != Dimension::Overworld || self.recipe.is_some() {
            return Vec::new();
        }
        let radius = radius.clamp(0, 10000);
        (x.saturating_sub(radius).div_euclid(SPACING) - 1
            ..=x.saturating_add(radius).div_euclid(SPACING) + 1)
            .filter_map(|cell| {
                site_for_cell(cell, &self.perlin, &self.temp_perlin, &self.moist_perlin)
            })
            .filter(|s| (i64::from(s.center_x) - i64::from(x)).abs() <= i64::from(radius))
            .collect()
    }
    pub(super) fn build_overworld_village(
        chunk: &mut Chunk,
        chunk_x: i32,
        terrain: &Perlin,
        temp: &Perlin,
        moist: &Perlin,
    ) {
        let start = chunk_x * CHUNK_WIDTH as i32;
        for cell in start.div_euclid(SPACING) - 1..=start.div_euclid(SPACING) + 1 {
            let Some((center, _)) = center_for_cell(cell, terrain.seed()) else {
                continue;
            };
            if start > center + VILLAGE_RADIUS + 96
                || start + CHUNK_WIDTH as i32 <= center - VILLAGE_RADIUS - 96
            {
                continue;
            }
            let Some(site) = site_for_cell(cell, terrain, temp, moist) else {
                continue;
            };
            let center = site.center_x;
            if start > center + VILLAGE_RADIUS
                || start + CHUNK_WIDTH as i32 <= center - VILLAGE_RADIUS
            {
                continue;
            }
            for x in (center - VILLAGE_RADIUS).max(start)
                ..=(center + VILLAGE_RADIUS).min(start + CHUNK_WIDTH as i32 - 1)
            {
                let floor = site.road_height(x);
                let row = floor.floor() as i32;
                let b = Self::biome_for_x(temp, moist, x);
                let natural = Self::blended_overworld_surface_y(terrain, temp, moist, x, b);
                for y in 1..row {
                    put(chunk, start, x, y, BlockType::Air);
                }
                for y in row..=(natural + 3).max(row + 3) {
                    put(chunk, start, x, y, BlockType::Dirt);
                }
                put(
                    chunk,
                    start,
                    x,
                    row,
                    if floor.fract() != 0.0 {
                        BlockType::StoneSlab
                    } else {
                        BlockType::Cobblestone
                    },
                );
            }
            for (left, right, role) in site.buildings() {
                building(chunk, start, &site, left, right, role);
            }
            let g = site.ground_y;
            // A shared stone square with a covered cistern beneath the walking lane.
            for x in center..=center + 10 {
                put(chunk, start, x, g, BlockType::StoneBricks);
            }
            for x in center + 4..=center + 6 {
                put(chunk, start, x, g, BlockType::StoneBricks);
                put(chunk, start, x, g + 1, BlockType::Water(8));
                put(chunk, start, x, g + 2, BlockType::StoneBricks);
            }
            for x in [center + 3, center + 7] {
                put(chunk, start, x, g + 1, BlockType::StoneBricks);
            }
            // A roofed market gate is a landmark and a normal passable archway.
            for x in center + 1..=center + 9 {
                put(chunk, start, x, g - 6, BlockType::StoneBricks);
            }
            for x in [center + 1, center + 9] {
                for y in g - 5..=g - 3 {
                    put(chunk, start, x, y, BlockType::Wood);
                }
                for y in [g - 2, g - 1] {
                    put(chunk, start, x, y, BlockType::WoodDoor(true));
                }
                put(chunk, start, x, g + 1, BlockType::StoneBricks);
            }
            for x in center + 4..=center + 6 {
                put(chunk, start, x, g - 5, BlockType::Wool);
            }
            put(chunk, start, center + 3, g - 2, BlockType::Torch);
            for x in center + 35..=center + 43 {
                put(chunk, start, x, g, BlockType::Farmland(7));
                put(
                    chunk,
                    start,
                    x,
                    g - 1,
                    BlockType::Crops([3, 5, 7][(x - center).rem_euclid(3) as usize]),
                );
                put(chunk, start, x, g + 1, BlockType::StoneBricks);
            }
            put(chunk, start, center + 39, g + 1, BlockType::Water(8));
            put(chunk, start, center + 39, g + 2, BlockType::StoneBricks);
            for x in [
                center - 44,
                center - 22,
                center - 19,
                center + 12,
                center + 32,
                center + 46,
            ] {
                put(chunk, start, x, g - 1, BlockType::Torch);
            }
            for (x, b) in [
                (center - 24, BlockType::RedFlower),
                (center + 14, BlockType::YellowFlower),
                (center + 34, BlockType::YellowFlower),
                (center + 44, BlockType::RedFlower),
            ] {
                put(chunk, start, x, g - 1, b);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> (VillageSite, Perlin, Perlin, Perlin, Perlin) {
        let (p, t, m, c) = (
            Perlin::new(1337),
            Perlin::new(7331),
            Perlin::new(9917),
            Perlin::new(4444),
        );
        let site = (-26..0)
            .filter_map(|cell| site_for_cell(cell, &p, &t, &m))
            .min_by_key(|v| v.center_x.abs())
            .unwrap();
        (site, p, t, m, c)
    }
    fn chunks(site: &VillageSite, reverse: bool) -> HashMap<i32, Chunk> {
        let (_, p, t, m, c) = fixture();
        let mut order = ((site.center_x - VILLAGE_RADIUS).div_euclid(CHUNK_WIDTH as i32)
            ..=(site.center_x + VILLAGE_RADIUS).div_euclid(CHUNK_WIDTH as i32))
            .collect::<Vec<_>>();
        if reverse {
            order.reverse();
        }
        order
            .into_iter()
            .map(|cx| {
                (
                    cx,
                    World::build_chunk_with_noise(
                        cx,
                        Dimension::Overworld,
                        "village-fixture",
                        &p,
                        &t,
                        &m,
                        &c,
                    ),
                )
            })
            .collect()
    }
    fn at(chunks: &HashMap<i32, Chunk>, x: i32, y: i32) -> BlockType {
        chunks[&x.div_euclid(CHUNK_WIDTH as i32)]
            .get_block(x.rem_euclid(CHUNK_WIDTH as i32) as usize, y as usize)
    }
    #[test]
    fn negative_settlement_is_identical_in_either_chunk_order() {
        let (site, ..) = fixture();
        let forward = chunks(&site, false);
        let reverse = chunks(&site, true);
        assert!(forward.len() >= 4);
        for (&cx, a) in &forward {
            let b = &reverse[&cx];
            for x in 0..CHUNK_WIDTH {
                for y in 0..CHUNK_HEIGHT {
                    assert_eq!(
                        a.get_block(x, y),
                        b.get_block(x, y),
                        "chunk-order seam at {cx}:{x},{y}"
                    );
                    if a.get_block(x, y) == BlockType::Chest {
                        assert_eq!(
                            serde_json::to_value(a.chest_inventory(x, y)).unwrap(),
                            serde_json::to_value(b.chest_inventory(x, y)).unwrap()
                        );
                    }
                }
            }
        }
    }
    #[test]
    fn settlement_corridor_doors_and_loft_beds_are_clear_and_supported() {
        let (site, ..) = fixture();
        let blocks = chunks(&site, false);
        let g = site.ground_y;
        for x in site.center_x - 47..=site.center_x + 47 {
            assert!(
                at(&blocks, x, g).is_solid(),
                "missing street support at {x}"
            );
            for y in [g - 1, g - 2] {
                assert!(
                    !at(&blocks, x, y).is_solid()
                        || matches!(at(&blocks, x, y), BlockType::WoodDoor(false)),
                    "blocked corridor {x},{y}"
                );
            }
        }
        for (l, r, role) in site.buildings() {
            for x in [l, r] {
                assert_eq!(at(&blocks, x, g - 1), BlockType::WoodDoor(false));
                assert_eq!(at(&blocks, x, g - 2), BlockType::WoodDoor(false));
                for dx in [-1, 1] {
                    for y in [g - 1, g - 2] {
                        assert!(!at(&blocks, x + dx, y).is_solid());
                    }
                }
            }
            if role != BuildingRole::Workshop {
                let bed = if role == BuildingRole::Cottage {
                    l + 2
                } else {
                    r - 2
                };
                assert_eq!(at(&blocks, bed, g - 4), BlockType::Bed);
                assert_eq!(at(&blocks, bed, g - 5), BlockType::Air);
                assert_eq!(at(&blocks, bed, g - 6), BlockType::Air);
            }
        }
    }
    #[test]
    fn road_grades_never_require_a_full_block_climb() {
        let (site, ..) = fixture();
        for x in site.center_x - VILLAGE_RADIUS..site.center_x + VILLAGE_RADIUS {
            assert!((site.road_height(x + 1) - site.road_height(x)).abs() <= 0.5);
        }
        assert_eq!(
            site.road_height(site.center_x - VILLAGE_RADIUS),
            site.left_ground as f64
        );
        assert_eq!(
            site.road_height(site.center_x + VILLAGE_RADIUS),
            site.right_ground as f64
        );
    }
    #[test]
    fn garden_stays_hydrated_and_loot_and_edits_survive_reopen() {
        let root = std::env::temp_dir().join(format!("tc-village-save-{}", rand::random::<u64>()));
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(
            root.join("overworld_world_seed.bin"),
            bincode::serialize(&[1337u32, 7331, 9917, 4444]).unwrap(),
        )
        .unwrap();
        let mut w = World::open_in_directory(Dimension::Overworld, &root).unwrap();
        let site = w
            .village_sites_near(0, 10000)
            .into_iter()
            .filter(|v| v.center_x < 0)
            .min_by_key(|v| v.center_x.abs())
            .unwrap();
        w.load_chunks_for_spawn_search(site.center_x, 100);
        let g = site.ground_y;
        for _ in 0..100 {
            w.update(site.center_x + 39);
        }
        for x in site.center_x + 35..=site.center_x + 43 {
            assert_eq!(w.get_block(x, g), BlockType::Farmland(7));
            assert!(matches!(w.get_block(x, g - 1), BlockType::Crops(_)));
        }
        let (left, _, _) = site.buildings()[0];
        let chest = left + 1;
        assert!(
            w.chest_inventory(chest, g - 4)
                .unwrap()
                .has_item(ItemType::Bread, 1)
        );
        w.try_save_all().unwrap();
        w.chest_inventory_mut(chest, g - 4)
            .unwrap()
            .slots
            .fill(None);
        w.set_block(left, g - 1, BlockType::Air);
        w.try_save_all().unwrap();
        drop(w);
        let mut reopened = World::open_in_directory(Dimension::Overworld, &root).unwrap();
        reopened.load_chunks_for_spawn_search(site.center_x, 100);
        assert!(
            reopened
                .chest_inventory(chest, g - 4)
                .unwrap()
                .slots
                .iter()
                .all(Option::is_none)
        );
        assert_eq!(reopened.get_block(left, g - 1), BlockType::Air);
        drop(reopened);
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn seed_fixture_has_viable_village_sites() {
        let (p, t, m) = (Perlin::new(1337), Perlin::new(7331), Perlin::new(9917));
        assert!((-26..0).any(|cell| site_for_cell(cell, &p, &t, &m).is_some()));
        assert!((0..26).any(|cell| site_for_cell(cell, &p, &t, &m).is_some()));
    }
}
