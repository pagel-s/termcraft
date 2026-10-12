//! Four-by-four, world-anchored material pixels. The same atlas serves every world.
use super::{BlockType, GameState, Renderer};

type Rgb = (u8, u8, u8);
pub(super) struct Tile {
    pub(super) pixels: [&'static str; 4],
    pub(super) colors: [Rgb; 4], // body, highlight, shadow, accent
    pub(super) detail: Option<[&'static str; 8]>,
}

impl Tile {
    pub(super) fn for_block(
        block: BlockType,
        dimension: crate::world::Dimension,
        above: BlockType,
    ) -> Option<Self> {
        use BlockType::*;
        let (_, fg, bg) = Renderer::block_render_style(block, above, dimension, 1.0);
        let foreground = Renderer::color_rgb(fg).unwrap_or((180, 190, 200));
        let background = Renderer::color_rgb(bg).unwrap_or(foreground);
        let body = Renderer::lerp_rgb(background, foreground, 0.42);
        let mut colors = [
            body,
            Renderer::lerp_rgb(body, (240, 231, 212), 0.22),
            Renderer::lerp_rgb(body, (18, 24, 36), 0.35),
            foreground,
        ];
        let pixels = match block {
            Air => return None,
            Grass => {
                colors = [(116, 81, 54), (129, 166, 79), (87, 61, 45), (84, 132, 61)];
                if above.is_solid() {
                    ["....", ".-..", "..+.", ".-.."]
                } else {
                    ["++++", "aaaa", "a..a", ".-.."]
                }
            }
            Dirt => {
                colors = [(111, 80, 56), (125, 91, 63), (97, 68, 51), (116, 84, 58)];
                ["....", ".-..", "..+.", "-..-"]
            }
            Farmland(hydration) => {
                colors = if hydration > 0 {
                    [(80, 57, 43), (101, 73, 51), (53, 40, 34), (87, 62, 46)]
                } else {
                    [(119, 86, 56), (143, 105, 67), (87, 62, 45), (126, 91, 59)]
                };
                ["+-+-", ".-.-", "....", "..-."]
            }
            Stone | Bedrock => {
                colors = [
                    (99, 105, 111),
                    (111, 118, 123),
                    (86, 93, 100),
                    (105, 111, 116),
                ];
                ["....", ".+..", "...-", "-..."]
            }
            Cobblestone => {
                colors = [
                    (106, 111, 113),
                    (127, 130, 128),
                    (82, 89, 94),
                    (113, 119, 118),
                ];
                ["++-+", ".-..", "--.-", "..+."]
            }
            StoneBricks => {
                colors = [
                    (111, 115, 115),
                    (130, 134, 130),
                    (86, 92, 96),
                    (118, 124, 121),
                ];
                ["++++", "...-", "----", "-..."]
            }
            Wood => {
                colors = [(119, 85, 55), (150, 111, 69), (91, 64, 46), (133, 96, 59)];
                ["-..+", "-.+.", "-..+", "..-+"]
            }
            BirchWood => {
                colors = [
                    (195, 192, 165),
                    (218, 214, 188),
                    (91, 91, 79),
                    (170, 166, 144),
                ];
                ["++++", ".-..", "...-", "..+."]
            }
            Leaves | BirchLeaves => {
                colors = [(58, 104, 58), (78, 130, 68), (43, 83, 49), (67, 116, 61)];
                ["a++a", "+..+", "..-.", "-..a"]
            }
            IronOre | GoldOre | DiamondOre | CoalOre | RedstoneOre => {
                let accent = match block {
                    IronOre => (173, 137, 110),
                    GoldOre => (217, 176, 73),
                    DiamondOre => (91, 191, 192),
                    CoalOre => (47, 51, 57),
                    _ => (176, 70, 57),
                };
                colors = [(99, 105, 111), (115, 121, 124), (82, 88, 95), accent];
                ["....", ".aa.", "a..a", ".-.."]
            }
            Sand | Gravel | SoulSand => ["..+.", ".-..", "....", "..-."],
            EndStone => {
                colors = [
                    (167, 164, 117),
                    (186, 182, 132),
                    (147, 144, 104),
                    (175, 171, 119),
                ];
                ["..+.", ".-..", "....", "..-."]
            }
            Planks => {
                colors = [
                    (153, 111, 68),
                    (170, 128, 77),
                    (118, 82, 53),
                    (163, 119, 70),
                ];
                ["++++", "...-", "----", "-..."]
            }
            CraftingTable => ["a-a-", "-a-a", "+--+", ".--."],
            Furnace => ["++++", "+--+", "+aa+", "----"],
            Chest => ["++++", "....", "-aa-", "+..+"],
            Bed => {
                colors = [(132, 61, 57), (211, 195, 159), (92, 58, 45), (163, 75, 65)];
                ["++aa", "aaaa", "aaaa", "-  -"]
            }
            Bookshelf => {
                colors[3] = (168, 77, 68);
                ["++++", "a.+a", "----", "+a.+"]
            }
            Glass => ["+  +", " a  ", "  a ", "+  +"],
            Wool | Snow | Ice => ["++++", "....", ".+..", "..-."],
            StoneSlab => ["    ", "    ", "++++", "...."],
            StoneStairs => ["  ++", "  ..", "++..", "...."],
            EnchantingTable => {
                colors[3] = (122, 231, 223);
                [" aa ", "++++", "-aa-", "----"]
            }
            Anvil => ["++++", " -- ", " .. ", "++++"],
            BrewingStand => {
                colors[3] = (214, 92, 156);
                [" +  ", "a+ a", "a+ a", "----"]
            }
            Torch => {
                colors = [(119, 82, 45), (251, 175, 72), (75, 53, 34), (255, 221, 130)];
                [" aa ", " ++ ", " .  ", " .  "]
            }
            RedstoneTorch(on) => {
                colors = [(103, 71, 44), (222, 93, 58), (66, 44, 32), (249, 116, 76)];
                if !on {
                    colors = [(86, 62, 43), (111, 58, 49), (55, 41, 33), (123, 63, 52)];
                }
                [" aa ", " ++ ", " .  ", " .  "]
            }
            Tnt | PrimedTnt(_) => {
                colors[3] = (232, 227, 205);
                ["++++", "aaaa", "-a-a", ".-.-"]
            }
            Lever(on) => {
                if on {
                    ["   +", "  + ", " +  ", "----"]
                } else {
                    ["+   ", " +  ", "  + ", "----"]
                }
            }
            StoneButton(timer) => {
                if timer > 0 {
                    ["    ", "    ", "    ", " -- "]
                } else {
                    ["    ", "    ", " ++ ", " -- "]
                }
            }
            Cactus => [" +. ", "a+.a", " +. ", "a+.a"],
            DeadBush => ["    ", "+ + ", " + +", "  + "],
            RedFlower | YellowFlower => {
                colors[0] = (74, 132, 65);
                [" aa ", "aaa ", " .+ ", "+.  "]
            }
            TallGrass | Sapling | BirchSapling | SugarCane => [" + +", "+. +", " .+.", "+.. "],
            Water(_) => {
                colors = [(42, 98, 145), (99, 186, 208), (31, 72, 118), (56, 132, 177)];
                if matches!(above, Water(_)) {
                    ["....", "a...", "..a.", "...."]
                } else {
                    ["++++", "a..a", ".aa.", "...."]
                }
            }
            Lava(_) => {
                colors = [
                    (223, 91, 34),
                    (255, 216, 111),
                    (148, 51, 32),
                    (249, 153, 54),
                ];
                if matches!(above, Lava(_)) {
                    ["a..a", ".aa.", "..a.", ".--."]
                } else {
                    ["++++", "a..a", ".aa.", "..a."]
                }
            }
            Obsidian => {
                colors = [(76, 65, 94), (107, 91, 127), (43, 37, 58), (86, 72, 105)];
                ["+..-", ".a.-", ".-..", "..a-"]
            }
            Netherrack => {
                colors = [(106, 59, 54), (128, 72, 62), (86, 45, 45), (119, 58, 48)];
                ["+..-", ".-..", "a.+.", "..-."]
            }
            Glowstone => ["+aa+", "a++a", "a+a+", "+aa+"],
            NetherPortal | EndPortal => ["a..a", ".aa.", "a..a", ".aa."],
            EndPortalFrame { filled } => {
                colors = [
                    (66, 91, 76),
                    (123, 143, 105),
                    (41, 56, 49),
                    if filled {
                        (108, 206, 164)
                    } else {
                        (27, 37, 38)
                    },
                ];
                ["+aa+", "+--+", "....", "----"]
            }
            IronDoor(open) | WoodDoor(open) => {
                if open {
                    ["+   ", "+   ", "+a  ", "+   "]
                } else {
                    ["++++", "+--+", "+-a+", "+--+"]
                }
            }
            Ladder => ["+  +", "++++", "+  +", "++++"],
            RedstoneDust(_) => ["    ", "    ", "  a ", "aaaa"],
            RedstoneRepeater {
                powered,
                facing_right,
                ..
            } => {
                colors[3] = if powered {
                    (239, 99, 69)
                } else {
                    (117, 54, 47)
                };
                if facing_right {
                    ["    ", "a a ", "--a-", "...."]
                } else {
                    ["    ", " a a", "-a--", "...."]
                }
            }
            Piston {
                extended,
                facing_right,
            }
            | StickyPiston {
                extended,
                facing_right,
            } => {
                colors = [
                    (109, 108, 100),
                    (159, 137, 98),
                    (67, 70, 72),
                    if matches!(block, StickyPiston { .. }) {
                        (110, 159, 88)
                    } else {
                        (169, 139, 93)
                    },
                ];
                match (extended, facing_right) {
                    (false, true) => ["...a", ".++a", ".--a", "---a"],
                    (false, false) => ["a...", "a++.", "a--.", "a---"],
                    (true, true) => [".. a", ".++a", ".--a", "-- a"],
                    (true, false) => ["a ..", "a++.", "a--.", "a --"],
                }
            }
            Crops(stage) => {
                colors = [
                    (73, 128, 57),
                    (127, 176, 72),
                    (49, 96, 47),
                    if stage >= 7 {
                        (224, 188, 81)
                    } else {
                        (99, 157, 59)
                    },
                ];
                match stage {
                    0 => ["    ", "    ", "    ", " + +"],
                    1..=3 => ["    ", "    ", " + +", "+.+."],
                    4..=6 => ["    ", " + +", "+.+.", " . ."],
                    _ => [" a a", "a.a.", " . .", " . ."],
                }
            }
            NetherWart(stage) => {
                colors = [(143, 54, 65), (196, 76, 81), (86, 39, 50), (219, 105, 103)];
                match stage {
                    0 => ["    ", "    ", "    ", " + +"],
                    1..=2 => ["    ", "    ", " ++ ", "+..+"],
                    _ => ["    ", " ++ ", "+aa+", ".--."],
                }
            }
            SilverfishSpawner | BlazeSpawner | ZombieSpawner | SkeletonSpawner => {
                colors = [
                    (70, 77, 83),
                    (117, 130, 135),
                    (37, 43, 51),
                    match block {
                        SilverfishSpawner => (155, 161, 170),
                        BlazeSpawner => (244, 168, 74),
                        ZombieSpawner => (95, 159, 86),
                        _ => (213, 210, 187),
                    },
                ];
                ["++++", "+a-+", "+-a+", "++++"]
            }
        };
        let detail = match block {
            Torch | RedstoneTorch(_) => Some([
                "        ", "        ", "   a    ", "  a+a   ", "   +    ", "   .    ", "   .    ",
                "   .    ",
            ]),
            Chest => Some([
                " ++++++ ", "+......+", "+......+", "---aa---", "...aa...", "........", "+......+",
                "--------",
            ]),
            CraftingTable => Some([
                "++-++-++", "++-++-++", "--------", "++-++-++", "-......-", "-a....a-", "-a....a-",
                "--------",
            ]),
            Furnace => Some([
                "++++++++", "+......+", "+------+", "+------+", "+......+", "+.aaaa.+", "+.aaaa.+",
                "--------",
            ]),
            Glass => Some([
                "+      +", " .a     ", "   a    ", "    a   ", "     a  ", "  a     ", "      a ",
                "+      +",
            ]),
            Ladder => Some([
                " +    + ", " +    + ", " ++++++ ", " +    + ", " +    + ", " +    + ", " ++++++ ",
                " +    + ",
            ]),
            WoodDoor(false) | IronDoor(false) => {
                Some(if matches!(above, WoodDoor(_) | IronDoor(_)) {
                    [
                        "+......+", "+......+", "+....a.+", "+......+", "+.++++.+", "+.+..+.+",
                        "+.++++.+", "++++++++",
                    ]
                } else {
                    [
                        "++++++++", "+.++++.+", "+.+..+.+", "+.++++.+", "+......+", "+......+",
                        "+......+", "+......+",
                    ]
                })
            }
            _ => None,
        };
        Some(Self {
            pixels,
            colors,
            detail,
        })
    }
}

impl Renderer {
    // Sky art lives in screen pixels. Walking and jumping must not scroll a
    // second landscape behind the collision world or make stars blink in/out.
    pub(super) fn screen_sky_pixel(
        state: &GameState,
        (sx, sy): (i32, i32),
        (width, rows): (i32, i32),
        (day, twilight): (f32, f32),
    ) -> Rgb {
        let height = sy as f32 / rows.max(1) as f32;
        let zenith = Self::lerp_rgb((13, 22, 43), (91, 143, 181), day);
        let horizon = Self::lerp_rgb((31, 39, 61), (177, 204, 210), day);
        let mut sky = Self::lerp_rgb(zenith, horizon, height * 0.78);
        sky = Self::lerp_rgb(sky, (221, 156, 124), twilight * height * 0.32);
        sky = Self::lerp_rgb(sky, (69, 85, 102), state.weather_rain_intensity * 0.6);
        if state.thunder_flash_timer > 0 {
            sky = Self::lerp_rgb(sky, (227, 236, 245), 0.65);
        }

        if day < 0.25 && sy < rows / 2 {
            // One restrained star per screen-space region, with a fixed offset.
            let hash = ((sx.div_euclid(32) * 73) ^ (sy.div_euclid(16) * 137)).rem_euclid(997);
            if sx.rem_euclid(32) == 4 + hash % 24 && sy.rem_euclid(16) == 3 + (hash / 24) % 10 {
                sky = Self::lerp_rgb(sky, (185, 199, 219), 1.0 - day * 4.0);
            }
        }

        if state.weather_rain_intensity < 0.2 {
            let (progress, moon) = if day > 0.3 {
                (
                    ((state.time_of_day - 4000.0) / 16000.0).clamp(0.0, 1.0),
                    false,
                )
            } else if day < 0.25 {
                (
                    if state.time_of_day < 4000.0 {
                        (state.time_of_day + 4000.0) / 8000.0
                    } else {
                        (state.time_of_day - 20000.0) / 8000.0
                    },
                    true,
                )
            } else {
                (-1.0, false)
            };
            let center_x = (4.0 + (width - 9).max(1) as f32 * progress).round() as i32;
            let center_y = (rows as f32 * (0.13 + 0.2 * (progress - 0.5).abs())).round() as i32;
            let px = sx - center_x + 4;
            let py = sy - center_y + 4;
            // A nine-pixel disc with deliberately stepped corners, rather than
            // a sampled ellipse that changes shape with fractional positions.
            let inset = match py {
                0 | 8 => 3,
                1 | 7 => 1,
                2..=6 => 0,
                _ => 9,
            };
            if (0.0..=1.0).contains(&progress) && px >= inset && px < 9 - inset {
                sky = if moon {
                    if matches!((px, py), (5, 2) | (6, 3) | (3, 6)) {
                        (166, 184, 200)
                    } else if py >= 6 {
                        (192, 206, 215)
                    } else {
                        (218, 227, 229)
                    }
                } else if py >= 6 {
                    (245, 198, 105)
                } else {
                    (255, 231, 164)
                };
            }
        }

        if day > 0.2 {
            // Authored compact silhouettes drift one pixel every four seconds.
            // Both clouds use the same sky space; there is no camera parallax.
            let drift = ((state.player.age / 80) % 192) as i32;
            let x = (sx + drift).rem_euclid(192);
            let first_y = (rows / 7).clamp(7, 18);
            let spans = [(9, 15), (6, 19), (3, 22), (0, 26), (1, 25), (4, 22)];
            let second_spans = [(6, 11), (3, 15), (0, 19), (2, 17)];
            let cloud = if let Some(&(left, right)) = spans.get((sy - first_y) as usize) {
                (22 + left..=22 + right).contains(&x)
            } else if let Some(&(left, right)) =
                second_spans.get((sy - first_y - rows / 8) as usize)
            {
                (123 + left..=123 + right).contains(&x)
            } else {
                false
            };
            if cloud {
                let underside = sy == first_y + 5 || sy == first_y + rows / 8 + 3;
                let tint = if underside {
                    (186, 205, 213)
                } else {
                    (229, 236, 232)
                };
                let tint = Self::lerp_rgb(tint, (168, 181, 193), state.weather_rain_intensity);
                sky = Self::lerp_rgb(sky, tint, ((day - 0.2) / 0.8).clamp(0.0, 1.0));
            }
        }
        sky
    }

    pub(super) fn render_pixel_effects(&mut self, state: &GameState) {
        let (vx, vy, cx, cy, scale) = self.world_view.unwrap();
        let project = |x: f64, y: f64| {
            (
                (x * scale as f64 * 2.0).round() as i32 - vx + cx,
                (y * scale as f64 * 2.0).round() as i32 - vy * 2 + cy * 2
                    - self.vertical_pixel_offset,
            )
        };
        let mut lines = Vec::new();
        let bobber = state
            .fishing_bobber()
            .map(|(x, y, bite)| (project(x, y), bite));
        for bolt in &state.lightning_bolts {
            lines.push((
                project(bolt.x as f64, bolt.y_top as f64),
                project(bolt.x as f64, bolt.y_bottom as f64),
                Self::rgb((255, 238, 189)),
            ));
        }
        if let Some(dragon) = &state.ender_dragon {
            for crystal in &state.end_crystals {
                lines.push((
                    project(crystal.x, crystal.y - 0.5),
                    project(dragon.x, dragon.y - 1.4),
                    Self::rgb((168, 94, 184)),
                ));
            }
        }
        for ((x0, y0), (x1, y1), color) in lines {
            let steps = x0.abs_diff(x1).max(y0.abs_diff(y1)).min(1024);
            for step in 0..=steps {
                let t = f64::from(step) / f64::from(steps.max(1));
                self.put_pixel(
                    x0 + ((x1 as f64 - x0 as f64) * t).round() as i32,
                    y0 + ((y1 as f64 - y0 as f64) * t).round() as i32,
                    color,
                );
            }
        }
        if let Some(((x, y), bite)) = bobber {
            self.put_pixel(x, y - 1, Self::rgb((235, 230, 213)));
            self.put_pixel(
                x,
                y,
                Self::rgb(if bite { (255, 204, 91) } else { (215, 71, 66) }),
            );
        }
    }

    pub(super) fn render_pixel_sky(
        &mut self,
        state: &GameState,
        lighting: &super::shading::SceneLighting,
    ) {
        use crate::world::Dimension;
        let (vx, vy, cx, cy, scale) = self.world_view.unwrap();
        let unit = scale * 2;
        let left = (vx - cx).div_euclid(unit);
        let right = (vx - cx + i32::from(self.width)).div_euclid(unit);
        let mut horizons = std::mem::take(&mut self.pixel_horizons);
        horizons.clear();
        for bx in left..=right {
            let ground = (0..crate::world::chunk::CHUNK_HEIGHT as i32)
                .find(|&by| {
                    // Isolated props and floating blocks cannot turn all the air
                    // below them into underground. Find an actual terrain layer.
                    (0..3).all(|dy| {
                        matches!(
                            state.world.get_block(bx, by + dy),
                            BlockType::Grass
                                | BlockType::Dirt
                                | BlockType::Sand
                                | BlockType::Snow
                                | BlockType::Stone
                                | BlockType::Water(_)
                                | BlockType::Gravel
                                | BlockType::Farmland(_)
                                | BlockType::Ice
                        )
                    })
                })
                .unwrap_or(crate::world::chunk::CHUNK_HEIGHT as i32);
            horizons.push(ground);
        }
        let day = Self::day_mix(state.time_of_day);
        let twilight = Self::twilight_mix(state.time_of_day);
        let rows = i32::from(self.height - 4) * 2;
        for sy in 0..rows {
            let world_y = (sy + vy * 2 + self.vertical_pixel_offset - cy * 2) as f64 / unit as f64;
            for sx in 0..i32::from(self.width) {
                let world_x = (sx + vx - cx) as f64 / unit as f64;
                let bx = world_x.floor() as i32;
                let ground = horizons[(bx - left) as usize];
                let rgb = match state.current_dimension {
                    Dimension::Overworld if world_y < ground as f64 => {
                        let mut sky = Self::screen_sky_pixel(
                            state,
                            (sx, sy),
                            (i32::from(self.width), rows),
                            (day, twilight),
                        );
                        let precipitation = state.precipitation_at(bx);
                        if state.weather_rain_intensity > 0.05
                            && precipitation != super::PrecipitationType::None
                        {
                            let rain =
                                (sx * 17 + sy * 3 + (state.player.age * 2) as i32).rem_euclid(83);
                            if rain < 2 {
                                sky = Self::lerp_rgb(sky, (169, 196, 210), 0.55);
                            }
                        }
                        sky
                    }
                    Dimension::Overworld => Self::color_rgb(lighting.shade(
                        Self::rgb((53, 57, 65)),
                        world_x,
                        world_y,
                        0.72,
                        false,
                    ))
                    .unwrap(),
                    Dimension::Nether => {
                        let glow = (world_y as f32 / 110.0).clamp(0.0, 1.0);
                        let mut sky = Self::lerp_rgb((28, 16, 27), (72, 31, 30), glow);
                        let ember = ((world_x * 8.0).floor() as i32)
                            .wrapping_mul(19)
                            .wrapping_add(((world_y * 8.0).floor() as i32).wrapping_mul(37))
                            .rem_euclid(719);
                        if ember == 0 {
                            sky = (171, 88, 51);
                        }
                        sky
                    }
                    Dimension::End => {
                        let hash = sx.wrapping_mul(31) ^ sy.wrapping_mul(67);
                        if hash.rem_euclid(347) == 0 {
                            (176, 156, 207)
                        } else {
                            (22, 17, 38)
                        }
                    }
                };
                self.put_pixel(sx, sy, Self::rgb(rgb));
            }
        }
        self.pixel_horizons = horizons;
    }

    pub(super) fn render_pixel_materials(
        &mut self,
        state: &GameState,
        lighting: &super::shading::SceneLighting,
    ) {
        let (vx, vy, cx, cy, scale) = self.world_view.unwrap();
        let unit = scale * 2;
        let left = (vx - cx).div_euclid(unit);
        let right = (vx - cx + i32::from(self.width)).div_euclid(unit);
        let top = (vy * 2 + self.vertical_pixel_offset - cy * 2).div_euclid(unit);
        let bottom = (vy * 2 + self.vertical_pixel_offset - cy * 2
            + i32::from(self.height - 4) * 2)
            .div_euclid(unit);
        for by in top..=bottom {
            for bx in left..=right {
                let block = state.world.get_block(bx, by);
                let above = state.world.get_block(bx, by - 1);
                let tile = Tile::for_block(block, state.current_dimension, above);
                let backdrop = if !block.is_solid() {
                    Self::room_backdrop(state, bx, by)
                } else {
                    None
                };
                if tile.is_none() && backdrop.is_none() {
                    continue;
                }
                let upper_left = state.world.get_block(bx - 1, by - 1).is_solid();
                let upper_right = state.world.get_block(bx + 1, by - 1).is_solid();
                let left_solid = state.world.get_block(bx - 1, by).is_solid();
                let right_solid = state.world.get_block(bx + 1, by).is_solid();
                let below_solid = state.world.get_block(bx, by + 1).is_solid();
                let sx = bx * unit - vx + cx;
                let sy = by * unit - vy * 2 - self.vertical_pixel_offset + cy * 2;
                // Texture variation is tied to world coordinates, never frame or camera.
                let variation =
                    ((bx.wrapping_mul(31) ^ by.wrapping_mul(17)).rem_euclid(5) - 2) as f32 * 0.012;
                let backdrop_pixel = |px: usize, py: usize| {
                    let color = backdrop?;
                    let wx = bx as f64 + (px as f64 + 0.5) / unit as f64;
                    let wy = by as f64 + (py as f64 + 0.5) / unit as f64;
                    let u = px as f32 / unit as f32;
                    let v = py as f32 / unit as f32;
                    let contact = f32::from(above.is_solid()) * (1.0 - v)
                        + f32::from(below_solid) * v
                        + f32::from(left_solid) * (1.0 - u)
                        + f32::from(right_solid) * u;
                    let seam = if py == unit as usize - 1 { 0.92 } else { 1.0 };
                    Some(lighting.shade(
                        Self::rgb(color),
                        wx,
                        wy,
                        (0.88 - contact * 0.12) * seam,
                        false,
                    ))
                };
                for py in 0..unit as usize {
                    for px in 0..unit as usize {
                        let color = if let Some(tile) = &tile {
                            let token = if let Some(detail) = tile.detail {
                                detail[py * 8 / unit as usize].as_bytes()[px * 8 / unit as usize]
                            } else {
                                tile.pixels[py * 4 / unit as usize].as_bytes()
                                    [px * 4 / unit as usize]
                            };
                            let index = match token {
                                b'.' => 0,
                                b'+' => 1,
                                b'-' => 2,
                                b'a' => 3,
                                _ => {
                                    if let Some(color) = backdrop_pixel(px, py) {
                                        self.put_pixel(sx + px as i32, sy + py as i32, color);
                                    }
                                    continue;
                                }
                            };
                            let mut rgb = tile.colors[index];
                            if block.is_solid() && !matches!(block, BlockType::Glass) {
                                if py == 0
                                    && !above.is_solid()
                                    && !matches!(block, BlockType::Grass)
                                {
                                    rgb = Self::lerp_rgb(rgb, tile.colors[1], 0.32);
                                }
                                if px == unit as usize - 1 && !right_solid {
                                    rgb = Self::lerp_rgb(rgb, tile.colors[2], 0.32);
                                }
                            }
                            {
                                let u = (px as f32 + 0.5) / unit as f32;
                                let v = (py as f32 + 0.5) / unit as f32;
                                let corner = if !above.is_solid() {
                                    f32::from(upper_left) * (1.0 - u) * (1.0 - v)
                                        + f32::from(upper_right) * u * (1.0 - v)
                                } else {
                                    0.0
                                };
                                let grain = ((bx * 71 + by * 29 + px as i32 * 17 + py as i32 * 43)
                                    .rem_euclid(7)
                                    - 3) as f32
                                    * 0.004;
                                let occlusion =
                                    (0.98 + variation + grain - corner * 0.2).clamp(0.6, 1.05);
                                let emissive = matches!(
                                    block,
                                    BlockType::Lava(_)
                                        | BlockType::Glowstone
                                        | BlockType::NetherPortal
                                        | BlockType::EndPortal
                                ) || (matches!(
                                    block,
                                    BlockType::Torch | BlockType::RedstoneTorch(true)
                                ) && matches!(token, b'a' | b'+'));
                                lighting.shade(
                                    Self::rgb(rgb),
                                    bx as f64 + (px as f64 + 0.5) / unit as f64,
                                    by as f64 + (py as f64 + 0.5) / unit as f64,
                                    occlusion,
                                    emissive,
                                )
                            }
                        } else {
                            backdrop_pixel(px, py).unwrap()
                        };
                        self.put_pixel(sx + px as i32, sy + py as i32, color);
                    }
                }
                // Small, stable cracks make mining visible without replacing the material.
                if state.player.mining_timer > 0.0
                    && state.player.last_mine_x == bx
                    && state.player.last_mine_y == by
                {
                    let count = (state.player.mining_timer * 9.0).ceil().clamp(1.0, 9.0) as usize;
                    for &(x, y) in [
                        (1, 0),
                        (2, 1),
                        (1, 2),
                        (2, 3),
                        (3, 2),
                        (0, 1),
                        (3, 0),
                        (0, 3),
                        (2, 2),
                    ]
                    .iter()
                    .take(count)
                    {
                        self.put_pixel(sx + x, sy + y, Self::rgb((35, 30, 36)));
                    }
                }
            }
        }
    }

    // A bounded, visual-only cutaway back wall. It has no collision or saved blocks.
    fn room_backdrop(state: &GameState, x: i32, y: i32) -> Option<Rgb> {
        // End tunnels deliberately use their own short obsidian recess.
        if state.current_dimension == crate::world::Dimension::End {
            let roof = (1..=6).find(|dy| state.world.get_block(x, y - dy) != BlockType::Air)?;
            if roof <= 3
                && state.world.get_block(x, y - roof) == BlockType::Obsidian
                && (1..=3).any(|dy| state.world.get_block(x, y + dy) == BlockType::Obsidian)
            {
                return Some((31, 27, 43));
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
            let b = state.world.get_block(x, y + dy);
            b.is_solid() && !furnishing(b)
        })?;
        let ground = y + floor;
        // A partial loft is not the room's ceiling. Try each structural candidate
        // until a complete enclosing roof is found; furnishings never define it.
        for roof in 1..=6 {
            let ceiling = y - roof;
            let material = state.world.get_block(x, ceiling);
            if material == BlockType::Glass || !roof_material(material) || roof + floor > 8 {
                continue;
            }
            let side = |direction| {
                (1..=16).any(|dx| {
                    let bx = x + dx * direction;
                    (ceiling + 1..ground).all(|by| wall_material(state.world.get_block(bx, by)))
                        && (x.min(bx)..=x.max(bx))
                            .all(|rx| roof_material(state.world.get_block(rx, ceiling)))
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
                    (103, 112, 116)
                } else {
                    (132, 119, 98)
                },
            );
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sky_art_stays_together_during_camera_motion_in_every_pixel_scale() {
        let mut state = GameState::network_view();
        let air = vec![
            BlockType::Air;
            crate::world::chunk::CHUNK_WIDTH * crate::world::chunk::CHUNK_HEIGHT
        ];
        for chunk in -2..=2 {
            state.world.apply_chunk_column_snapshot(chunk, &air);
        }
        for dimension in [
            crate::world::Dimension::Overworld,
            crate::world::Dimension::End,
        ] {
            state.current_dimension = dimension;
            for time in [12000.0, 22000.0] {
                state.time_of_day = time;
                for scale in [2, 3, 4] {
                    let mut renderer = Renderer::with_size(160, 48);
                    let lighting =
                        super::super::shading::SceneLighting::new(&state, &[15], (0, 0, 1, 1));
                    renderer.world_view = Some((0, 30 * scale, 80, 36, scale));
                    renderer.render_pixel_sky(&state, &lighting);
                    let still = renderer.back_buffer.clone();
                    for (vx, vy, half) in [(-13, 27 * scale, 1), (17, 33 * scale, 0)] {
                        renderer.world_view = Some((vx, vy, 80, 36, scale));
                        renderer.vertical_pixel_offset = half;
                        renderer.render_pixel_sky(&state, &lighting);
                        assert!(
                            renderer.back_buffer == still,
                            "sky split/flickered during camera motion in {dimension:?}, time {time}, scale {scale}"
                        );
                    }
                }
            }
        }
    }

    fn enclosed_room() -> GameState {
        let mut s = GameState::network_view();
        s.world.apply_chunk_column_snapshot(
            0,
            &vec![
                BlockType::Air;
                crate::world::chunk::CHUNK_WIDTH * crate::world::chunk::CHUNK_HEIGHT
            ],
        );
        for x in 1..=9 {
            s.world.set_block(x, 16, BlockType::Planks);
            s.world.set_block(x, 20, BlockType::Stone);
        }
        for x in [1, 9] {
            for y in 17..20 {
                s.world.set_block(x, y, BlockType::Wood);
            }
        }
        s
    }
    #[test]
    fn partial_lofts_do_not_cut_sky_holes_in_room_backgrounds() {
        let mut s = enclosed_room();
        for x in 5..9 {
            s.world.set_block(x, 18, BlockType::Planks);
        }
        s.world.set_block(6, 17, BlockType::Chest);
        let main = Renderer::room_backdrop(&s, 3, 19).unwrap();
        assert_eq!(Renderer::room_backdrop(&s, 6, 19), Some(main));
        assert!(Renderer::room_backdrop(&s, 6, 17).is_some());
    }
    #[test]
    fn enclosed_rooms_keep_backdrop_through_glazing_doors_and_props() {
        let mut s = enclosed_room();
        s.world.set_block(1, 18, BlockType::Glass);
        for y in 18..20 {
            s.world.set_block(9, y, BlockType::WoodDoor(true));
        }
        s.world.set_block(4, 19, BlockType::Torch);
        s.world.set_block(4, 17, BlockType::Glass);
        let wood = Renderer::room_backdrop(&s, 4, 18).unwrap();
        assert_eq!(Renderer::room_backdrop(&s, 4, 19), Some(wood));
        for x in 1..=9 {
            s.world.set_block(x, 16, BlockType::StoneBricks);
        }
        let stone = Renderer::room_backdrop(&s, 4, 18).unwrap();
        assert_ne!(stone, wood);
        assert!(stone.2 > stone.0 && wood.0 > wood.2);
    }
    #[test]
    fn incomplete_ruins_do_not_paint_disconnected_background_panels() {
        let mut s = enclosed_room();
        for x in [1, 9] {
            for y in 18..20 {
                s.world.set_block(x, y, BlockType::Air);
            }
        }
        for x in [3, 7] {
            s.world.set_block(x, 19, BlockType::StoneBricks);
        }
        for x in 2..9 {
            for y in 17..20 {
                if s.world.get_block(x, y) == BlockType::Air {
                    assert_eq!(Renderer::room_backdrop(&s, x, y), None);
                }
            }
        }
    }
    #[test]
    fn detached_trees_cannot_close_an_open_shelter() {
        let mut s = enclosed_room();
        for x in 6..=9 {
            s.world.set_block(x, 16, BlockType::Air);
        }
        assert_eq!(Renderer::room_backdrop(&s, 4, 18), None);
    }
    #[test]
    fn end_tunnels_retain_their_obsidian_recess() {
        let mut s = enclosed_room();
        s.current_dimension = crate::world::Dimension::End;
        s.world.set_block(4, 16, BlockType::Obsidian);
        s.world.set_block(4, 20, BlockType::Obsidian);
        assert_eq!(Renderer::room_backdrop(&s, 4, 18), Some((31, 27, 43)));
    }

    #[test]
    fn floating_blocks_do_not_paint_opaque_columns_through_open_air() {
        let mut state = GameState::network_view();
        state.world.apply_chunk_column_snapshot(
            0,
            &vec![
                BlockType::Air;
                crate::world::chunk::CHUNK_WIDTH * crate::world::chunk::CHUNK_HEIGHT
            ],
        );
        for x in 0..16 {
            for y in 20..24 {
                state.world.set_block(x, y, BlockType::Stone);
            }
        }
        state.player.x = 4.5;
        state.player.y = 20.0;
        state.player.grounded = true;
        state.time_of_day = 12000.0;
        let sample = |s: &GameState| {
            let mut renderer = Renderer::with_size(160, 48);
            renderer.render_to(s, 0.0, &mut Vec::new()).unwrap();
            let (vx, vy, cx, cy, scale) = renderer.world_view.unwrap();
            let x = (2.5 * (scale * 2) as f64) as i32 - vx + cx;
            let y = (18.5 * scale as f64) as i32 - vy + cy;
            {
                let cell = renderer.back_buffer[y as usize * 160 + x as usize];
                (cell.ch, cell.fg, cell.bg)
            }
        };
        let open_air = sample(&state);
        for block in [BlockType::Stone, BlockType::Obsidian, BlockType::Chest] {
            state.world.set_block(2, 17, block);
            assert_eq!(
                sample(&state),
                open_air,
                "false background column below {block:?}"
            );
        }
    }
    #[test]
    fn crop_growth_hydration_and_spawner_species_have_visible_distinctions() {
        let tile =
            |b| Tile::for_block(b, crate::world::Dimension::Overworld, BlockType::Air).unwrap();
        for (a, b) in [
            (BlockType::Crops(0), BlockType::Crops(3)),
            (BlockType::Crops(3), BlockType::Crops(7)),
            (BlockType::NetherWart(0), BlockType::NetherWart(1)),
            (BlockType::NetherWart(1), BlockType::NetherWart(3)),
        ] {
            assert_ne!(tile(a).pixels, tile(b).pixels);
        }
        assert_ne!(
            tile(BlockType::Farmland(0)).colors,
            tile(BlockType::Farmland(7)).colors
        );
        let colors: std::collections::HashSet<_> = [
            BlockType::SilverfishSpawner,
            BlockType::BlazeSpawner,
            BlockType::ZombieSpawner,
            BlockType::SkeletonSpawner,
        ]
        .map(|b| tile(b).colors)
        .into_iter()
        .collect();
        assert_eq!(colors.len(), 4);
    }
    #[test]
    fn every_atlas_row_has_four_pixels_and_distinct_materials_remain_distinct() {
        let mut patterns = std::collections::HashSet::new();
        for block in [
            BlockType::Grass,
            BlockType::Stone,
            BlockType::Wood,
            BlockType::Planks,
            BlockType::Chest,
            BlockType::Furnace,
            BlockType::Glass,
            BlockType::Ladder,
            BlockType::Water(0),
            BlockType::Lava(0),
            BlockType::NetherPortal,
            BlockType::EndStone,
        ] {
            let tile =
                Tile::for_block(block, crate::world::Dimension::Overworld, BlockType::Air).unwrap();
            for row in tile.pixels {
                assert_eq!(row.len(), 4);
            }
            patterns.insert(tile.pixels);
        }
        assert!(patterns.len() >= 10);
    }
}
