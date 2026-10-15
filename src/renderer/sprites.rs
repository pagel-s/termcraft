//! Terminal-native pixel figures. Each cell holds two vertical color pixels.
use super::{Color, GameState, RenderCell, Renderer};
#[path = "dragon.rs"]
mod dragon;

enum SpritePixels {
    Rows(&'static [&'static str]),
    Dragon(&'static dragon::Frame),
    Item(super::ItemType),
}

impl SpritePixels {
    fn width(&self) -> usize {
        match self {
            Self::Rows(rows) => rows[0].len(),
            Self::Dragon(_) => dragon::WIDTH,
            Self::Item(_) => 8,
        }
    }
    fn height(&self) -> usize {
        match self {
            Self::Rows(rows) => rows.len(),
            Self::Dragon(_) => dragon::HEIGHT,
            Self::Item(_) => 8,
        }
    }
    fn token(&self, x: usize, y: usize) -> u8 {
        match self {
            Self::Rows(rows) => rows[y].as_bytes()[x],
            Self::Dragon(frame) => frame[y][x],
            Self::Item(item) => super::items::token(*item, x, y),
        }
    }
}

#[derive(Clone, Copy)]
pub(super) enum SpriteKind {
    Player,
    Zombie,
    Pigman,
    Enderman,
    Creeper,
    Skeleton,
    Cow,
    Sheep,
    Pig,
    Chicken,
    Wolf,
    Ocelot,
    Villager,
    Ghast,
    Blaze,
    Spider,
    Silverfish,
    Slime,
    Squid,
    Dragon,
    Crystal,
    Boat,
    Fireball,
    Arrow,
    Orb,
    Item,
}

pub(super) struct ActorSprite {
    x: i32,
    pub(super) y: i32,
    pub(super) kind: SpriteKind,
    facing_right: bool,
    moving: bool,
    phase: bool,
    colors: [Color; 3],
    crouched: bool,
    action: bool,
    subcell: (i32, i32),
    label: Option<u16>,
    equipped: bool,
    world_position: Option<(f64, f64)>,
    dimensions: Option<(usize, usize)>,
    airborne: bool,
    vertical_velocity: f64,
    swing: Option<f64>,
    warning: u8,
    held_item: Option<super::ItemType>,
    stride: usize,
    hit: bool,
    alert: bool,
    variant: bool,
    helmet: Option<Color>,
    animation_time: f64,
}

impl ActorSprite {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn humanoid(
        x: i32,
        y: i32,
        kind: SpriteKind,
        facing_right: bool,
        vx: f64,
        age: u64,
        colors: [Color; 3],
        crouched: bool,
        action: bool,
    ) -> Self {
        Self {
            x,
            y,
            kind,
            facing_right,
            moving: vx.abs() > 0.03,
            phase: (age / 3).is_multiple_of(2),
            colors,
            crouched,
            action,
            subcell: (0, 0),
            label: None,
            equipped: !matches!(kind, SpriteKind::Player),
            world_position: None,
            dimensions: None,
            airborne: false,
            vertical_velocity: 0.0,
            swing: None,
            warning: 0,
            held_item: None,
            stride: 0,
            hit: false,
            alert: false,
            variant: false,
            helmet: None,
            animation_time: age as f64,
        }
    }
    #[allow(clippy::too_many_arguments)]
    pub(super) fn mob(
        x: i32,
        y: i32,
        kind: SpriteKind,
        facing_right: bool,
        vx: f64,
        age: u64,
        color: Color,
    ) -> Self {
        Self::humanoid(x, y, kind, facing_right, vx, age, [color; 3], false, false)
    }

    pub(super) fn with_item(mut self, item: Option<super::ItemType>) -> Self {
        self.equipped = item.is_some();
        self.held_item = item;
        self
    }

    pub(super) fn with_label(mut self, id: u16) -> Self {
        self.label = Some(id);
        self
    }

    pub(super) fn with_subcell(mut self, x: f64, y: f64, scale: i32) -> Self {
        self.world_position = Some((x, y));
        self.stride = ((x * 2.0).floor() as i64).rem_euclid(4) as usize;
        self.subcell = (
            (x * 2.0 * scale as f64).round() as i32 - (x * 2.0).round() as i32 * scale,
            (y * scale as f64).floor() as i32 - y.floor() as i32 * scale,
        );
        self
    }

    pub(super) fn with_pose(mut self, airborne: bool) -> Self {
        self.airborne = airborne;
        self
    }
    pub(super) fn with_dimensions(mut self, width: usize, height: usize) -> Self {
        self.dimensions = Some((width, height));
        self
    }

    pub(super) fn with_alert(mut self, alert: bool) -> Self {
        self.alert = alert;
        self
    }
    pub(super) fn with_variant(mut self, variant: bool) -> Self {
        self.variant = variant;
        self
    }

    pub(super) fn with_motion(mut self, vy: f64) -> Self {
        self.vertical_velocity = vy;
        self
    }
    pub(super) fn with_swing(mut self, phase: Option<f64>) -> Self {
        self.swing = phase;
        self
    }
    pub(super) fn with_warning(mut self, ticks: u8) -> Self {
        self.warning = ticks;
        self
    }

    pub(super) fn with_hit(mut self, hit: bool) -> Self {
        self.hit = hit;
        self
    }

    pub(super) fn with_helmet(mut self, helmet: Option<Color>) -> Self {
        self.helmet = helmet;
        self
    }

    pub(super) fn with_animation_time(mut self, time: f64) -> Self {
        self.animation_time = time;
        self
    }

    fn pixel_dimensions(&self, scale: i32) -> (usize, usize) {
        if let Some((width, rows)) = self.dimensions {
            return (width.max(1), (rows * 2).max(1));
        }
        if matches!(self.kind, SpriteKind::Item) {
            let pixels = (scale as usize * 3 / 2).max(2);
            return (pixels, pixels);
        }
        let pixels = self.pixels();
        (
            ((pixels.width() * scale as usize + 2) / 4).max(1),
            ((pixels.height() * scale as usize + 2) / 4).max(1),
        )
    }

    fn humanoid_kind(&self) -> bool {
        matches!(
            self.kind,
            SpriteKind::Player
                | SpriteKind::Zombie
                | SpriteKind::Pigman
                | SpriteKind::Skeleton
                | SpriteKind::Villager
                | SpriteKind::Enderman
        )
    }

    fn pixels(&self) -> SpritePixels {
        if matches!(self.kind, SpriteKind::Dragon) {
            return SpritePixels::Dragon(if self.variant {
                dragon::front_frame(self.animation_time, self.alert)
            } else {
                dragon::frame(self.animation_time, self.alert)
            });
        }
        SpritePixels::Rows(match self.kind {
            SpriteKind::Player => &[
                " hhhh ", " hssse", " sssn ", "  ss  ", " acca ", "acccaw", "acccaw", " cccc ",
                " -cc- ", " ll l ", " ll l ", " l ll ", " l ll ", " b bb ",
            ],
            SpriteKind::Zombie => &[
                " hhhh ", " sshse", " seesn", " ssss ", "  ss  ", " cccss", "scccaa", "scccs ",
                " cccc ", " -cc- ", " l ll ", " l ll ", " ll l ", " b bb ",
            ],
            SpriteKind::Pigman => &[
                " hhhh ", " srsse", " ssrrn", " sssn ", "  ss  ", " assa ", "asssrw", "asssrw",
                " -rr- ", " ll l ", " ll l ", " l ll ", " l ll ", " b bb ",
            ],
            SpriteKind::Skeleton => &[
                " ssss ", " sssse", " seenn", " ssss ", "  ss  ", "  ss t", " asssa", " a s t",
                " asssa", "  ss t", " l  l ", " l  l ", " l  l ", " b  b ",
            ],
            SpriteKind::Enderman => &[
                " ssss ", " ssss ", " sees ", " ssss ", "  ss  ", " acca ", " acca ", " acca ",
                " acca ", " acca ", " acca ", " a  a ", " a  a ", " l  l ", " l  l ", " l  l ",
                " l  l ", " l  l ", " l  l ", " l  l ", " l  l ", " b  b ",
            ],
            SpriteKind::Villager => &[
                " hhhh ", " hssse", " sssnn", " sssnn", "  ssn ", " cccc ", "acccca", "acccca",
                " aaaa ", " -cc- ", " cccc ", " cccc ", " l  l ", " b  b ",
            ],
            SpriteKind::Creeper => &[
                " ssss ", "s+ss+s", "se  es", "ssnnss", "s nn s", " ssss ", " s+ss ", " ss+s ",
                " ssss ", " ssss ", " llll ", " l ll ", " l ll ", " b bb ",
            ],
            SpriteKind::Cow => &[
                "        g g ",
                "        sss ",
                "  sssfffsse ",
                " sssfffssnnn",
                "ssffssssfss ",
                "ssffsssffss ",
                " sssssssss  ",
                " ss-ss-sss  ",
                "  l l  l l  ",
                "  l l  l l  ",
                "  b b  b b  ",
            ],
            SpriteKind::Sheep => &[
                "  ffffff    ",
                " ffffffffss ",
                "ffffffffsse ",
                "ffffffffsnnn",
                "ffffffffss  ",
                " fffffffss  ",
                "  l l  l l  ",
                "  l l  l l  ",
                "  b b  b b  ",
            ],
            SpriteKind::Pig => &[
                "       s s ",
                "  ssssssss ",
                " sss+ssssen",
                "ssssssssnnn",
                " sssssssnn ",
                "  -sssss   ",
                "  l l  ll  ",
                "  b b  bb  ",
            ],
            SpriteKind::Chicken => &[
                "   r  ", "   ffe", "   ffn", "f fff ", "fffff ", " fff  ", "  tt  ", "  tt  ",
            ],
            SpriteKind::Wolf => &[
                "       h h  ",
                "       sss  ",
                "  sssssssenn",
                "hsssssssnnn ",
                " sssppssss  ",
                "  -ssssss   ",
                "   l l l l  ",
                "   b b b b  ",
            ],
            SpriteKind::Ocelot => &[
                "        s s ",
                "        sse ",
                "  spspssssnn",
                " sspsspsssn ",
                "s ssssssss  ",
                "s  l l l l  ",
                "   b b b b  ",
            ],
            SpriteKind::Ghast => &[
                "   ++++++++++   ",
                "  +ssssssssss+  ",
                " +ss+ssss+ssss+ ",
                " +ssssssssssss+ ",
                " +ssppssssppss+ ",
                " +sseesssseess+ ",
                " +ssppssssppss+ ",
                " +ssssssssssss+ ",
                " +ssssnnnnssss+ ",
                "  +sssnnnnsss+  ",
                "   ++++++++++   ",
                "   l  l l  l    ",
                "   l  l l  l    ",
                "  l  l  l   l   ",
                "  l  l   l  l   ",
                "  b  b   b  b   ",
            ],
            SpriteKind::Blaze => &[
                "  t    t  ",
                "  t hh t  ",
                "  t ss t  ",
                "    ee    ",
                "t   ss   t",
                "t   cc   t",
                "t  rrrr  t",
                "    rr    ",
                "  t cc t  ",
                "  t rr t  ",
                "  t ll t  ",
                "    rr    ",
                "  t    t  ",
                "  t    t  ",
            ],
            SpriteKind::Spider => &[
                "    ssssss    ",
                "   ss+sss+s   ",
                "  ssseeeesss  ",
                " hssssssssssh ",
                "h hss--ssh h h",
                "h  l    l  h  ",
                "b b      b b b",
            ],
            SpriteKind::Silverfish => &["   ++   ", " +ssss+ ", "sspsssee", " llllll ", " b b bb "],
            SpriteKind::Slime => &[
                " ++++++ ", "+ssssss+", "+spppps+", "+seesse+", "+ssssss+", "+ssnsss+", "+ssssss+",
                " ++++++ ",
            ],
            SpriteKind::Squid => &[
                " ++++++ ", "+ssssss+", "+ssssss+", "+seesse+", "+ssssss+", " +ssss+ ", "  llll  ",
                " l ll l ", " l ll l ", "l  ll  l",
            ],
            SpriteKind::Dragon => unreachable!(),
            SpriteKind::Crystal => &[
                "    +   ", "   +v+  ", "  +vvv+ ", " +vvwvv+", "  +vvv+ ", "   +v+  ", "    +   ",
                "   ppp  ", "  ppppp ", " ppppppp",
            ],
            SpriteKind::Boat => &[
                "t          t",
                "tt        tt",
                "tsttttttttst",
                "ttsssssssstt",
                " tsssssssst ",
                "  pppppppp  ",
            ],
            SpriteKind::Fireball => &[
                "    r   ", "  rrrr  ", " rtttrr ", "rtwwttrr", " rtttrr ", "  rrrr  ", "    r   ",
            ],
            SpriteKind::Arrow => &["       g", "tppppppg", "       g"],
            SpriteKind::Item => {
                return SpritePixels::Item(self.held_item.unwrap_or(super::ItemType::Dirt));
            }
            SpriteKind::Orb => &[" + ", "+w+", " + "],
        })
    }

    fn color(&self, pixel: u8) -> Option<Color> {
        use SpriteKind::*;
        if matches!(self.kind, Item) {
            return super::items::color(self.held_item.unwrap_or(super::ItemType::Dirt), pixel);
        }
        let (skin, coat, legs, accent) = match self.kind {
            Player => (
                self.colors[0],
                self.colors[1],
                self.colors[2],
                Renderer::rgb((174, 105, 78)),
            ),
            Zombie => palette((105, 139, 83), (63, 111, 133), (49, 65, 91), (66, 88, 61)),
            Pigman => palette(
                (185, 138, 118),
                (161, 99, 87),
                (119, 89, 68),
                (104, 134, 82),
            ),
            Skeleton => palette(
                (218, 213, 190),
                (191, 187, 170),
                (190, 186, 166),
                (108, 98, 83),
            ),
            Enderman => palette((38, 35, 51), (30, 28, 42), (33, 30, 46), (68, 60, 84)),
            Villager => palette((182, 136, 102), (111, 77, 72), (68, 53, 48), (149, 101, 75)),
            Creeper if self.variant => palette(
                (111, 185, 178),
                (70, 147, 154),
                (62, 112, 129),
                (153, 222, 216),
            ),
            Creeper => palette((99, 151, 72), (79, 128, 60), (63, 100, 49), (125, 169, 85)),
            Cow => palette((99, 73, 58), (218, 209, 185), (73, 58, 48), (170, 130, 111)),
            Sheep => palette(
                (116, 106, 90),
                (225, 218, 191),
                (96, 84, 70),
                (149, 127, 105),
            ),
            Pig => palette(
                (209, 146, 139),
                (191, 119, 119),
                (166, 106, 103),
                (224, 164, 151),
            ),
            Chicken => palette(
                (222, 216, 192),
                (231, 226, 207),
                (187, 134, 62),
                (202, 76, 57),
            ),
            Wolf => palette(
                (165, 166, 154),
                (118, 125, 122),
                (91, 98, 95),
                (205, 199, 177),
            ),
            Ocelot => palette(
                (195, 162, 88),
                (148, 117, 61),
                (123, 95, 59),
                (220, 198, 126),
            ),
            Ghast => palette(
                (220, 218, 205),
                (198, 199, 195),
                (176, 177, 176),
                (132, 139, 144),
            ),
            Blaze => palette(
                (227, 163, 65),
                (164, 100, 42),
                (192, 121, 43),
                (255, 215, 113),
            ),
            Spider => palette((74, 57, 51), (55, 44, 43), (55, 44, 43), (115, 79, 58)),
            Silverfish => palette(
                (129, 136, 139),
                (101, 110, 118),
                (84, 94, 100),
                (159, 164, 161),
            ),
            Slime => palette(
                (100, 167, 86),
                (87, 145, 73),
                (73, 125, 66),
                (150, 194, 112),
            ),
            Squid => palette((69, 113, 128), (53, 87, 105), (53, 84, 99), (106, 146, 151)),
            Dragon => palette((104, 108, 124), (65, 68, 84), (87, 90, 106), (105, 73, 133)),
            Crystal => palette(
                (186, 119, 226),
                (95, 75, 122),
                (83, 72, 99),
                (233, 173, 255),
            ),
            Boat => palette((152, 106, 65), (116, 77, 49), (90, 61, 43), (190, 143, 83)),
            Fireball => palette(
                (242, 147, 58),
                (231, 100, 39),
                (177, 66, 31),
                (255, 218, 128),
            ),
            Arrow => palette(
                (205, 201, 176),
                (123, 91, 58),
                (96, 74, 50),
                (231, 220, 181),
            ),
            Orb | Item => (
                self.colors[0],
                self.colors[1],
                self.colors[2],
                self.colors[0],
            ),
        };
        let color = match pixel {
            b' ' => return None,
            b'e' => Renderer::rgb(match self.kind {
                Enderman => (214, 133, 244),
                Dragon => (244, 132, 246),
                Ghast | Blaze | Creeper | Wolf if self.alert => (229, 112, 66),
                Spider => (205, 83, 55),
                _ => (26, 27, 29),
            }),
            b'h' => self.helmet.unwrap_or_else(|| match self.kind {
                Player => Renderer::rgb((73, 53, 43)),
                Cow => Renderer::rgb((207, 195, 164)),
                Enderman => accent,
                Dragon => Renderer::rgb((197, 192, 208)),
                _ => Renderer::dim_color(skin, 0.65),
            }),
            b's' | b'a' => skin,
            b'+' => Renderer::rgb(Renderer::lerp_rgb(
                Renderer::color_rgb(skin).unwrap_or((150, 150, 150)),
                (232, 228, 207),
                0.22,
            )),
            b'-' | b'p' => Renderer::dim_color(skin, 0.64),
            b'c' => coat,
            b'l' => legs,
            b'b' => Renderer::dim_color(legs, 0.64),
            b'n' => accent,
            b'f' => {
                if matches!(self.kind, Sheep) && self.variant {
                    skin
                } else {
                    coat
                }
            }
            b'g' => Renderer::rgb((196, 188, 159)),
            b'r' => {
                if matches!(self.kind, Fireball | Blaze) {
                    Renderer::rgb((239, 117, 42))
                } else {
                    Renderer::rgb((165, 79, 65))
                }
            }
            b'v' => accent,
            b'o' if matches!(self.kind, Dragon) => Renderer::rgb((72, 66, 91)),
            b'i' if matches!(self.kind, Dragon) => Renderer::rgb((174, 170, 196)),
            b'z' if matches!(self.kind, Dragon) => Renderer::rgb((127, 116, 158)),
            b'j' if matches!(self.kind, Dragon) => Renderer::rgb((68, 50, 91)),
            b'w' if matches!(self.kind, Dragon) => Renderer::rgb((229, 219, 231)),
            b'Q' if matches!(self.kind, Dragon) => Renderer::rgb((82, 88, 109)),
            b'K' if matches!(self.kind, Dragon) => Renderer::rgb((153, 150, 179)),
            b'M' if matches!(self.kind, Dragon) => Renderer::rgb((105, 48, 132)),
            b'D' if matches!(self.kind, Dragon) => Renderer::rgb((163, 86, 192)),
            b'U' if matches!(self.kind, Dragon) => Renderer::rgb((38, 28, 57)),
            b'X' if matches!(self.kind, Dragon) => Renderer::rgb((14, 11, 22)),
            b'V' if matches!(self.kind, Dragon) => Renderer::rgb((134, 79, 161)),
            b't' => {
                if matches!(self.kind, Blaze) {
                    accent
                } else {
                    Renderer::rgb((177, 128, 69))
                }
            }
            b'w' if self.equipped => self.held_item.map_or_else(
                || {
                    if matches!(self.kind, Crystal | Fireball | Orb) {
                        Renderer::rgb((255, 232, 172))
                    } else {
                        Renderer::rgb((213, 179, 99))
                    }
                },
                Renderer::inventory_item_color,
            ),
            _ => return None,
        };
        if self.hit && pixel != b'e' {
            Some(Renderer::rgb(Renderer::lerp_rgb(
                Renderer::color_rgb(color).unwrap_or((180, 130, 120)),
                (250, 102, 80),
                0.64,
            )))
        } else {
            Some(color)
        }
    }
}

fn palette(
    a: (u8, u8, u8),
    b: (u8, u8, u8),
    c: (u8, u8, u8),
    d: (u8, u8, u8),
) -> (Color, Color, Color, Color) {
    (
        Renderer::rgb(a),
        Renderer::rgb(b),
        Renderer::rgb(c),
        Renderer::rgb(d),
    )
}

impl Renderer {
    // Keep wide color views on the same support-aware torch art as the detailed
    // views, while preserving classic glyph rendering when color is disabled.
    pub(super) fn render_wide_torches(
        &mut self,
        state: &GameState,
        light: &[u8],
        region: (i32, i32, usize, usize),
    ) {
        let Some((vx, vy, cx, cy, scale)) = self.world_view else {
            return;
        };
        if scale != 1 || !self.pixel_colors {
            return;
        }
        let left = (vx - cx).div_euclid(2);
        let right = (vx - cx + i32::from(self.width)).div_euclid(2);
        // Include a clipped torch above the viewport whose shaft can extend
        // into the visible upper half of a lower slab.
        let top = vy - cy - 1;
        let bottom = vy - cy + i32::from(self.height.saturating_sub(4));
        let mut torches = Vec::new();
        for by in top..=bottom {
            for bx in left..=right {
                if state.world.get_block(bx, by) == super::BlockType::Torch {
                    torches.push((bx, by, state.world.torch_attachment(bx, by)));
                }
            }
        }
        if torches.is_empty() {
            return;
        }
        for &(bx, by, _) in &torches {
            let sx = bx * 2 - vx + cx;
            let sy = by - vy + cy;
            for x in sx..sx + 2 {
                if x >= 0
                    && x < i32::from(self.width)
                    && sy >= 0
                    && sy < i32::from(self.height.saturating_sub(4))
                {
                    let cell =
                        &mut self.back_buffer[sy as usize * self.width as usize + x as usize];
                    *cell = RenderCell {
                        ch: ' ',
                        fg: cell.bg,
                        bg: cell.bg,
                    };
                }
            }
        }
        let lighting = super::shading::SceneLighting::new(state, light, region);
        for (bx, by, attachment) in torches {
            if let Some(attachment) = attachment {
                self.render_attached_torch(bx, by, attachment, &lighting);
            }
        }
    }

    pub(super) fn finish_world_view(
        &mut self,
        physical: (u16, u16),
        scale: i32,
        state: &super::GameState,
        light: &[u8],
        region: (i32, i32, usize, usize),
        frame_alpha: f64,
    ) {
        let lighting =
            (scale > 1).then(|| super::shading::SceneLighting::new(state, light, region));
        if scale > 1 {
            (self.width, self.height) = physical;
            self.clear_back_buffer();
            self.render_pixel_sky(state, lighting.as_ref().unwrap(), frame_alpha);
            self.render_pixel_materials(state, lighting.as_ref().unwrap());
            self.render_pixel_effects(state);
        }
        let mut actors = std::mem::take(&mut self.actor_sprites);
        actors.sort_by(|a, b| {
            a.world_position
                .map_or(a.y as f64, |p| p.1)
                .total_cmp(&b.world_position.map_or(b.y as f64, |p| p.1))
                .then_with(|| {
                    matches!(a.kind, SpriteKind::Player).cmp(&matches!(b.kind, SpriteKind::Player))
                })
        });
        if scale > 1 {
            for actor in &actors {
                self.draw_contact_shadow(actor, scale, state);
            }
        }
        for actor in &actors {
            self.draw_actor_sprite(actor, scale, lighting.as_ref(), state);
        }
        self.actor_sprites = actors;
        if scale == 1 {
            self.render_classic_warnings(state);
        }
    }

    fn blend_pixel(&self, x: i32, y: i32, color: Color, amount: f32) -> Color {
        if x < 0 || y < 0 || x >= self.width as i32 || y >= self.height as i32 * 2 {
            return color;
        }
        let Some(cell) = self
            .back_buffer
            .get(y.div_euclid(2).max(0) as usize * self.width as usize + x.max(0) as usize)
        else {
            return color;
        };
        let pixels = Self::cell_pixels(*cell);
        let under = if y.rem_euclid(2) == 0 {
            pixels.0
        } else {
            pixels.1
        };
        match (Self::color_rgb(under), Self::color_rgb(color)) {
            (Some(a), Some(b)) => Self::rgb(Self::lerp_rgb(a, b, amount)),
            _ => color,
        }
    }

    fn draw_contact_shadow(&mut self, actor: &ActorSprite, scale: i32, state: &super::GameState) {
        if matches!(
            actor.kind,
            SpriteKind::Fireball
                | SpriteKind::Arrow
                | SpriteKind::Orb
                | SpriteKind::Squid
                | SpriteKind::Boat
        ) {
            return;
        }
        let Some((wx, wy)) = actor.world_position else {
            return;
        };
        let surface_at = |wx: f64| {
            let bx = wx.floor() as i32;
            let local = wx.rem_euclid(1.0);
            (wy.floor() as i32..=wy.floor() as i32 + 4)
                .flat_map(|by| {
                    state
                        .world
                        .get_block(bx, by)
                        .collision_boxes()
                        .iter()
                        .filter_map(move |b| {
                            let top = by as f64 + b[1];
                            (local >= b[0] && local < b[2] && top >= wy - 0.05).then_some(top)
                        })
                })
                .min_by(f64::total_cmp)
        };
        let Some(ground) = surface_at(wx) else {
            return;
        };
        let height = (ground - wy).max(0.0);
        let (width, _) = actor.pixel_dimensions(scale);
        let (vx, vy, cx, cy, _) = self.world_view.unwrap();
        let unit = (scale * 2) as f64;
        let center = (wx * unit).round() as i32 - vx + cx + (height * 0.18 * unit) as i32;

        let radius = (width as f64 * 0.55 + scale as f64 * 0.2).max(1.0);
        for dx in -(radius.ceil() as i32)..=radius.ceil() as i32 {
            let x = center + dx;
            if x < 0 || x >= self.width as i32 {
                continue;
            }
            let sample_x = (x + vx - cx) as f64 / unit;
            let Some(surface) = surface_at(sample_x) else {
                continue;
            };
            if (surface - ground).abs() > 0.55 {
                continue;
            }
            let y = (surface * unit).round() as i32 - vy * 2 - self.vertical_pixel_offset + cy * 2;
            let soft = (1.0 - (dx as f64 / radius).powi(2)).max(0.0);
            let alpha = (soft * 0.31 * (-height * 0.55).exp()) as f32;
            for depth in 0..2 {
                let color = self.blend_pixel(
                    x,
                    y + depth,
                    Self::rgb((13, 17, 26)),
                    alpha / (depth + 1) as f32,
                );
                self.put_pixel(x, y + depth, color);
            }
        }
    }

    fn draw_actor_sprite(
        &mut self,
        actor: &ActorSprite,
        scale: i32,
        lighting: Option<&super::shading::SceneLighting>,
        state: &super::GameState,
    ) {
        if scale == 1 {
            let head = if actor.hit {
                "×"
            } else {
                match actor.kind {
                    SpriteKind::Player | SpriteKind::Villager => "◕",
                    SpriteKind::Zombie => "☠",
                    SpriteKind::Skeleton => "ö",
                    SpriteKind::Creeper => "▣",
                    SpriteKind::Enderman => "••",
                    _ => "ö",
                }
            };
            self.put_entity_str(actor.x, actor.y - 1, head, actor.colors[0]);
            self.put_entity_str(
                actor.x,
                actor.y,
                if actor.phase && actor.moving {
                    "╱▌"
                } else {
                    "▐╲"
                },
                actor.colors[1],
            );
            if let Some(id) = actor.label {
                self.put_str(
                    actor.x - 1,
                    actor.y - 2,
                    &format!("P{id}"),
                    Color::White,
                    Color::AnsiValue(234),
                );
            }
            return;
        }
        let pixels = actor.pixels();
        let source_width = pixels.width();
        let source_height = pixels.height();
        let (width, mut pixel_height) = actor.pixel_dimensions(scale);
        if actor.crouched {
            pixel_height = (pixel_height * 4 / 5).max(1);
        }
        let (vx, vy, cx, cy, _) = self.world_view.unwrap();
        let (x, feet) = if let Some((wx, wy)) = actor.world_position {
            (
                (wx * 2.0 * scale as f64).round() as i32 - vx + cx - width as i32 / 2,
                (wy * scale as f64 * 2.0).round() as i32 - vy * 2 - self.vertical_pixel_offset
                    + cy * 2,
            )
        } else {
            (
                actor.x * scale - vx.rem_euclid(scale),
                (actor.y + 1) * scale * 2 - vy.rem_euclid(scale) * 2,
            )
        };
        let y_pixel = feet - pixel_height as i32;
        let y = y_pixel.div_euclid(2);
        for py in 0..pixel_height {
            for col in 0..width {
                let mut source_col =
                    ((col * 2 + 1) * source_width / (2 * width)).min(source_width - 1);
                if !actor.facing_right {
                    source_col = source_width - 1 - source_col;
                }
                let source_y =
                    ((py * 2 + 1) * source_height / (pixel_height * 2)).min(source_height - 1);
                let mut token = pixels.token(source_col, source_y);
                // Keep identity pixels when a small terminal combines source pixels.
                // Center-only sampling previously discarded entire eye rows.
                if pixel_height < source_height {
                    let out_col = if actor.facing_right {
                        col
                    } else {
                        width - 1 - col
                    };
                    let x0 = out_col * source_width / width;
                    let x1 = ((out_col + 1) * source_width)
                        .div_ceil(width)
                        .min(source_width);
                    let y0 = py * source_height / pixel_height;
                    let y1 = ((py + 1) * source_height)
                        .div_ceil(pixel_height)
                        .min(source_height);
                    if (y0..y1).any(|y| (x0..x1).any(|x| pixels.token(x, y) == b'e')) {
                        token = b'e';
                    }
                }
                if actor.humanoid_kind()
                    && actor.airborne
                    && !actor.crouched
                    && py >= pixel_height.saturating_sub(3)
                {
                    let rising = actor.vertical_velocity < -0.03;
                    let left = if width <= 3 { 0 } else { width / 2 - 2 };
                    let right = (width / 2 + 1).min(width - 1);
                    token = if rising {
                        if py + 3 == pixel_height && (col == left || col == right) {
                            b'l'
                        } else if py + 2 == pixel_height
                            && (col == (left + 1).min(width - 1) || col == right)
                        {
                            b'b'
                        } else {
                            b' '
                        }
                    } else if col == left || col == right {
                        if py + 1 == pixel_height { b'b' } else { b'l' }
                    } else {
                        b' '
                    };
                } else if actor.humanoid_kind()
                    && actor.moving
                    && !actor.crouched
                    && source_y >= source_height - 4
                {
                    let middle = source_width / 2;
                    let left = middle - 1 - usize::from(actor.stride == 0);
                    let right = middle + usize::from(actor.stride == 2);
                    token = if source_col == left || source_col == right {
                        if source_y + 1 == source_height {
                            b'b'
                        } else {
                            b'l'
                        }
                    } else {
                        b' '
                    };
                }
                if let Some(mut color) = actor.color(token) {
                    let edge = |dx: isize, dy: isize| {
                        let nx = source_col as isize + dx;
                        let ny = source_y as isize + dy;
                        nx < 0
                            || ny < 0
                            || ny >= source_height as isize
                            || nx >= source_width as isize
                            || pixels.token(nx as usize, ny as usize) == b' '
                    };
                    let normal = if edge(0, -1) {
                        1.1
                    } else if edge(1, 0) {
                        0.78
                    } else if edge(-1, 0) {
                        1.02
                    } else {
                        0.94
                    };
                    let (vx, vy, cx, cy, _) = self.world_view.unwrap();
                    let wx = (x + col as i32 + vx - cx) as f64 / (2 * scale) as f64;
                    let wy = (y_pixel + py as i32 + vy * 2 + self.vertical_pixel_offset - cy * 2)
                        as f64
                        / (2 * scale) as f64;
                    let covered = state.world.get_block(wx.floor() as i32, wy.floor() as i32);
                    if covered != super::BlockType::Glass
                        && covered.collision_boxes().iter().any(|b| {
                            wx.rem_euclid(1.0) >= b[0]
                                && wx.rem_euclid(1.0) < b[2]
                                && wy.rem_euclid(1.0) >= b[1]
                                && wy.rem_euclid(1.0) < b[3]
                        })
                    {
                        continue;
                    }
                    let emissive = matches!(actor.kind, SpriteKind::Fireball | SpriteKind::Orb)
                        || (matches!(actor.kind, SpriteKind::Crystal)
                            && matches!(token, b'+' | b'v' | b'w'))
                        || (matches!(actor.kind, SpriteKind::Blaze)
                            && matches!(token, b't' | b'r'))
                        || (matches!(actor.kind, SpriteKind::Enderman | SpriteKind::Dragon)
                            && token == b'e');
                    color = lighting.unwrap().shade(color, wx, wy, normal, emissive);
                    if matches!(actor.kind, SpriteKind::Slime) && matches!(token, b's' | b'+') {
                        color = self.blend_pixel(x + col as i32, y_pixel + py as i32, color, 0.78);
                    }
                    self.put_pixel(x + col as i32, y_pixel + py as i32, color);
                }
            }
        }
        if let Some(id) = actor.label {
            self.put_str(
                x,
                y - 1,
                &format!("P{id}"),
                Color::White,
                Color::AnsiValue(234),
            );
        }
        if let Some(phase) = actor.swing {
            let direction = if actor.facing_right { 1.0 } else { -1.0 };
            let shoulder = (
                x + if actor.facing_right {
                    width as i32 * 2 / 3
                } else {
                    width as i32 / 3
                },
                y_pixel + pixel_height as i32 * 3 / 8,
            );
            let angle = -1.1 + (phase.clamp(0.0, 1.0) * std::f64::consts::PI).sin() * 1.8;
            let arm_length = (pixel_height as f64 * 0.22).round().max(1.0) as i32;
            let skin = actor.color(b'a').unwrap_or(actor.colors[0]);
            let sleeve = actor.color(b'c').unwrap_or(actor.colors[1]);
            for step in 0..=arm_length {
                let px = shoulder.0 + (direction * angle.cos() * f64::from(step)).round() as i32;
                let py = shoulder.1 + (angle.sin() * f64::from(step)).round() as i32;
                self.put_swing_pixel(
                    px,
                    py,
                    if step * 2 < arm_length { sleeve } else { skin },
                    state,
                    lighting,
                );
            }
            if let Some(item) = actor.held_item {
                let tool_length = if matches!(
                    item,
                    super::ItemType::WoodPickaxe
                        | super::ItemType::StonePickaxe
                        | super::ItemType::IronPickaxe
                        | super::ItemType::DiamondPickaxe
                        | super::ItemType::WoodAxe
                        | super::ItemType::StoneAxe
                        | super::ItemType::IronAxe
                        | super::ItemType::DiamondAxe
                        | super::ItemType::WoodShovel
                        | super::ItemType::StoneShovel
                        | super::ItemType::IronShovel
                        | super::ItemType::DiamondShovel
                        | super::ItemType::WoodHoe
                        | super::ItemType::StoneHoe
                        | super::ItemType::IronHoe
                        | super::ItemType::DiamondHoe
                        | super::ItemType::WoodSword
                        | super::ItemType::StoneSword
                        | super::ItemType::IronSword
                        | super::ItemType::DiamondSword
                        | super::ItemType::FishingRod
                        | super::ItemType::Bow
                ) {
                    (scale / 2).max(1)
                } else {
                    1
                };
                for step in arm_length + 1..=arm_length + tool_length {
                    self.put_swing_pixel(
                        shoulder.0 + (direction * angle.cos() * f64::from(step)).round() as i32,
                        shoulder.1 + (angle.sin() * f64::from(step)).round() as i32,
                        Self::inventory_item_color(item),
                        state,
                        lighting,
                    );
                }
            }
        } else if actor.action {
            let direction = if actor.facing_right { 1 } else { -1 };
            let hand = x + width as i32 / 2;
            for dx in 1..=3 {
                self.put_visible_effect(
                    hand + direction * dx,
                    y_pixel + pixel_height as i32 / 2 - dx,
                    Color::Yellow,
                    state,
                );
            }
        }
        if actor.warning > 0 {
            let center = x + width as i32 / 2;
            let amber = Self::rgb((255, 202, 100));
            for dy in 0..3 {
                self.put_visible_effect(center, y_pixel - 5 + dy, amber, state);
            }
            self.put_visible_effect(center, y_pixel - 1, amber, state);
        }
    }
    fn render_classic_warnings(&mut self, state: &GameState) {
        let (vx, vy, cx, cy, _) = self.world_view.unwrap();
        let mut positions = Vec::new();
        macro_rules! warning {
            ($field:ident,$height:expr) => {
                for e in &state.$field {
                    if e.attack_windup > 0 {
                        positions.push((e.x, e.y - $height));
                    }
                }
            };
        }
        warning!(zombies, 1.75);
        warning!(pigmen, 1.75);
        warning!(spiders, 0.75);
        warning!(silverfish, 0.5);
        warning!(slimes, 1.0);
        warning!(endermen, 2.75);
        warning!(blazes, 1.75);
        warning!(wolves, 0.875);
        for e in &state.skeletons {
            if e.bow_windup > 0 {
                positions.push((e.x, e.y - 1.75));
            }
        }
        for (x, y) in positions {
            self.put_str(
                (x * 2.0).round() as i32 - vx + cx,
                y.floor() as i32 - vy + cy - 1,
                "!",
                Color::Yellow,
                Color::Reset,
            );
        }
    }
    fn put_visible_effect(&mut self, x: i32, y: i32, color: Color, state: &GameState) {
        let (vx, vy, cx, cy, scale) = self.world_view.unwrap();
        let wx = (x + vx - cx) as f64 / (2 * scale) as f64;
        let wy = (y + vy * 2 + self.vertical_pixel_offset - cy * 2) as f64 / (2 * scale) as f64;
        let block = state.world.get_block(wx.floor() as i32, wy.floor() as i32);
        if block == super::BlockType::Glass
            || !block.collision_boxes().iter().any(|b| {
                wx.rem_euclid(1.0) >= b[0]
                    && wx.rem_euclid(1.0) < b[2]
                    && wy.rem_euclid(1.0) >= b[1]
                    && wy.rem_euclid(1.0) < b[3]
            })
        {
            self.put_pixel(x, y, color);
        }
    }

    fn put_swing_pixel(
        &mut self,
        x: i32,
        y: i32,
        color: Color,
        state: &GameState,
        lighting: Option<&super::shading::SceneLighting>,
    ) {
        let (vx, vy, cx, cy, scale) = self.world_view.unwrap();
        let wx = (x + vx - cx) as f64 / f64::from(2 * scale);
        let wy = (y + vy * 2 + self.vertical_pixel_offset - cy * 2) as f64 / f64::from(2 * scale);
        let color = lighting.map_or(color, |light| light.shade(color, wx, wy, 0.94, false));
        self.put_visible_effect(x, y, color, state);
    }

    pub(super) fn put_pixel(&mut self, x: i32, y_pixel: i32, color: Color) {
        self.put_pixel_bounded(x, y_pixel, color, self.height as i32 - 4);
    }

    pub(super) fn put_ui_pixel(&mut self, x: i32, y_pixel: i32, color: Color) {
        self.put_pixel_bounded(x, y_pixel, color, self.height as i32);
    }

    fn put_pixel_bounded(&mut self, x: i32, y_pixel: i32, color: Color, bottom: i32) {
        let y = y_pixel.div_euclid(2);
        if x < 0 || x >= self.width as i32 || y < 0 || y >= bottom {
            return;
        }
        let idx = y as usize * self.width as usize + x as usize;
        let (mut upper, mut lower) = Self::cell_pixels(self.back_buffer[idx]);
        if y_pixel.rem_euclid(2) == 0 {
            upper = color;
        } else {
            lower = color;
        }
        self.back_buffer[idx] = RenderCell {
            ch: '▀',
            fg: upper,
            bg: lower,
        };
    }

    fn cell_pixels(cell: RenderCell) -> (Color, Color) {
        match cell.ch {
            '▀' => (cell.fg, cell.bg),
            '▄' => (cell.bg, cell.fg),
            '█' => (cell.fg, cell.fg),
            ' ' => (cell.bg, cell.bg),
            _ => {
                let color = match (Self::color_rgb(cell.fg), Self::color_rgb(cell.bg)) {
                    (Some(fg), Some(bg)) => Self::rgb(Self::lerp_rgb(bg, fg, 0.3)),
                    _ => cell.bg,
                };
                (color, color)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_hand_swing_keeps_arm_proportions_and_never_draws_a_white_weapon() {
        let state = fixture();
        for scale in [2, 3, 4] {
            for facing in [false, true] {
                for phase in [0.0, 0.25, 0.5, 0.75, 0.99] {
                    let mut renderer = Renderer::with_size(160, 48);
                    renderer.world_view = Some((0, 0, 0, 0, scale));
                    let lighting = super::super::shading::SceneLighting::new(
                        &state,
                        &[15; 1024],
                        (0, 0, 32, 32),
                    );
                    let actor = ActorSprite::humanoid(
                        0,
                        0,
                        SpriteKind::Player,
                        facing,
                        0.0,
                        0,
                        [Renderer::rgb((190, 150, 120)), Color::Blue, Color::DarkBlue],
                        false,
                        false,
                    )
                    .with_subcell(5.0, 8.0, scale)
                    .with_swing(Some(phase));
                    renderer.draw_actor_sprite(&actor, scale, Some(&lighting), &state);
                    let (width, height) = actor.pixel_dimensions(scale);
                    let center = 5 * 2 * scale;
                    let radius = width as i32 / 2 + (height as f64 * 0.25).ceil() as i32;
                    let mut drawn = 0;
                    for (i, cell) in renderer.back_buffer.iter().enumerate() {
                        for color in [cell.fg, cell.bg] {
                            if color != Color::Reset {
                                drawn += 1;
                                assert_ne!(color, Color::White, "empty hand became a white rod");
                                let x = (i % 160) as i32;
                                assert!(
                                    (center - radius..=center + radius).contains(&x),
                                    "swing overhang exceeded a normal arm at scale {scale}"
                                );
                            }
                        }
                    }
                    assert!(drawn > 0);
                }
            }
        }
    }

    #[test]
    fn compact_airborne_pose_keeps_both_boots() {
        let mut state = crate::engine::state::GameState::network_view();
        state.world.apply_chunk_column_snapshot(
            0,
            &vec![
                super::super::BlockType::Air;
                crate::world::chunk::CHUNK_WIDTH * crate::world::chunk::CHUNK_HEIGHT
            ],
        );
        let light = super::super::shading::SceneLighting::new(&state, &[15; 1024], (0, 0, 32, 32));
        for vy in [-0.3, 0.3] {
            let mut r = Renderer::with_size(120, 40);
            r.world_view = Some((0, 0, 0, 0, 2));
            let actor = ActorSprite::humanoid(
                0,
                0,
                SpriteKind::Player,
                true,
                0.0,
                0,
                [Color::White; 3],
                false,
                false,
            )
            .with_subcell(5.0, 10.0, 2)
            .with_pose(true)
            .with_motion(vy);
            r.draw_actor_sprite(&actor, 2, Some(&light), &state);
            let y = if vy < 0.0 { 38 } else { 39 };
            let visible = (19..22)
                .filter(|&x| {
                    let (c1, c2) = Renderer::cell_pixels(r.back_buffer[(y / 2) * 120 + x]);
                    (if y % 2 == 0 { c1 } else { c2 }) != Color::Reset
                })
                .count();
            assert!(
                visible >= 2,
                "compact airborne boots disappeared at vy={vy}"
            );
        }
    }
    #[test]
    fn classic_warnings_are_present_before_damage() {
        let mut state = crate::engine::state::GameState::network_view();
        state.dismiss_startup_splash();
        state.player.x = 0.5;
        state.player.y = 20.0;
        state.world.apply_chunk_column_snapshot(
            0,
            &vec![
                super::super::BlockType::Air;
                crate::world::chunk::CHUNK_WIDTH * crate::world::chunk::CHUNK_HEIGHT
            ],
        );
        let mut z = crate::entities::zombie::Zombie::new(2.5, 20.0);
        z.attack_windup = 8;
        state.zombies.push(z);
        let mut r = Renderer::with_size(80, 24);
        r.render_to(&state, 0.0, &mut Vec::new()).unwrap();
        assert!(r.back_buffer.iter().any(|c| c.ch == '!'));
        assert_eq!(state.player.health, 20.0);
    }
    #[test]
    fn every_creature_has_a_rectangular_pixel_silhouette() {
        use SpriteKind::*;
        for kind in [
            Player, Zombie, Pigman, Enderman, Creeper, Skeleton, Cow, Sheep, Pig, Chicken, Wolf,
            Ocelot, Villager, Ghast, Blaze, Spider, Silverfish, Slime, Squid, Dragon, Crystal,
            Boat, Fireball, Arrow, Orb, Item,
        ] {
            let sprite = ActorSprite::mob(0, 0, kind, true, 0.0, 0, Color::White);
            let pixels = sprite.pixels();
            assert!(pixels.height() >= 3);
            if let SpritePixels::Rows(rows) = pixels {
                assert!(rows.iter().all(|r| r.len() == rows[0].len()));
            }
        }
    }
    fn fixture() -> super::super::GameState {
        let mut state = super::super::GameState::network_view();
        let blocks = vec![
            super::super::BlockType::Air;
            crate::world::chunk::CHUNK_WIDTH * crate::world::chunk::CHUNK_HEIGHT
        ];
        for x in -1..=1 {
            state.world.apply_chunk_column_snapshot(x, &blocks);
        }
        for x in -16..=16 {
            state.world.set_block(x, 10, super::super::BlockType::Grass);
        }
        state.player.x = 0.5;
        state.player.y = 10.0;
        state.player.grounded = true;
        state.time_of_day = 12000.0;
        state.dismiss_startup_splash();
        state
    }

    #[test]
    fn damage_and_alert_are_explicit_and_do_not_replace_species_palette() {
        let ghast = ActorSprite::mob(
            0,
            0,
            SpriteKind::Ghast,
            true,
            0.0,
            0,
            Color::Rgb {
                r: 255,
                g: 70,
                b: 70,
            },
        );
        let normal_eye = ghast.color(b'e');
        let body = ghast.color(b's').unwrap();
        let (r, g, b) = Renderer::color_rgb(body).unwrap();
        assert!(r.abs_diff(g) < 5 && g.abs_diff(b) < 20);
        let alert = ghast.with_alert(true);
        assert_eq!(alert.color(b's'), Some(body));
        assert_ne!(alert.color(b'e'), normal_eye);
        let hit = alert.with_hit(true);
        assert_ne!(hit.color(b's'), Some(body));
        let sheep = ActorSprite::mob(0, 0, SpriteKind::Sheep, true, 0.0, 0, Color::White);
        let wool = sheep.color(b'f');
        assert_ne!(sheep.with_variant(true).color(b'f'), wool);
    }

    #[test]
    fn helmet_changes_head_covering_and_preserves_the_face() {
        let skin = Renderer::rgb((190, 150, 120));
        let player = ActorSprite::humanoid(
            0,
            0,
            SpriteKind::Player,
            true,
            0.0,
            0,
            [skin, Color::Blue, Color::DarkBlue],
            false,
            false,
        );
        let iron = Renderer::rgb((216, 224, 232));
        let armored = player.with_helmet(Some(iron));
        assert_eq!(armored.color(b'h'), Some(iron));
        assert_eq!(armored.color(b's'), Some(skin));
    }

    #[test]
    fn contact_shadow_stays_on_solid_support_and_never_bridges_a_gap_or_lava() {
        let mut state = fixture();
        for x in -16..=16 {
            state.world.set_block(x, 10, super::super::BlockType::Air);
        }
        state.world.set_block(0, 10, super::super::BlockType::Stone);
        let mut renderer = Renderer::with_size(80, 100);
        renderer.world_view = Some((0, 0, 40, 0, 4));
        let white = RenderCell {
            ch: '▀',
            fg: Color::White,
            bg: Color::White,
        };
        renderer.back_buffer.fill(white);
        let actor = ActorSprite::mob(0, 0, SpriteKind::Zombie, true, 0.0, 0, Color::White)
            .with_subcell(0.95, 10.0, 4);
        renderer.draw_contact_shadow(&actor, 4, &state);
        assert!(renderer.back_buffer[40 * 80 + 46] != white);
        assert!(renderer.back_buffer[40 * 80 + 49] == white);
        assert!(renderer.back_buffer[39 * 80 + 46] == white);
        renderer.back_buffer.fill(white);
        state
            .world
            .set_block(0, 10, super::super::BlockType::Lava(0));
        renderer.draw_contact_shadow(&actor, 4, &state);
        assert!(renderer.back_buffer.iter().all(|cell| *cell == white));
    }

    #[test]
    fn transparent_torch_pixels_show_the_same_sky_without_legacy_glyph_residue() {
        let mut state = fixture();
        let mut baseline = Renderer::with_size(160, 48);
        baseline.render_to(&state, 0.0, &mut Vec::new()).unwrap();
        state.world.set_block(2, 9, super::super::BlockType::Torch);
        let mut actual = Renderer::with_size(160, 48);
        actual.render_to(&state, 0.0, &mut Vec::new()).unwrap();
        let (vx, vy, cx, cy, scale) = actual.world_view.unwrap();
        let x = 2 * scale * 2 - vx + cx;
        let y = 9 * scale * 2 - vy * 2 - actual.vertical_pixel_offset + cy * 2;
        for dx in 0..scale * 2 {
            let i = y.div_euclid(2) as usize * 160 + (x + dx) as usize;
            // The top half-pixel is transparent even when the flame occupies
            // the lower half of this terminal cell in the compact view.
            assert_eq!(
                Renderer::cell_pixels(actual.back_buffer[i]).0,
                Renderer::cell_pixels(baseline.back_buffer[i]).0
            );
        }
    }

    #[test]
    fn wide_color_view_keeps_torch_flame_and_support_distinct_without_legacy_glyphs() {
        let mut state = fixture();
        state.world.set_block(2, 9, super::super::BlockType::Torch);
        let mut renderer = Renderer::with_size(160, 48);
        renderer.zoom = 1;
        renderer.render_to(&state, 0.0, &mut Vec::new()).unwrap();
        let (vx, vy, cx, cy, scale) = renderer.world_view.unwrap();
        assert_eq!(scale, 1);
        let x = 2 * 2 - vx + cx;
        let y = 9 - vy + cy;
        let torch = renderer.back_buffer[y as usize * 160 + x as usize + 1];
        assert_eq!(torch.ch, '▀');
        assert_eq!(torch.fg, Renderer::rgb((255, 221, 130)));
        assert_ne!(torch.bg, torch.fg);
        for dx in 0..2 {
            assert_ne!(
                renderer.back_buffer[y as usize * 160 + (x + dx) as usize].ch,
                '!'
            );
        }
    }

    #[test]
    fn monochrome_view_retains_the_classic_torch_glyph() {
        let mut state = fixture();
        state.world.set_block(2, 9, super::super::BlockType::Torch);
        let mut renderer = Renderer::with_size(160, 48);
        renderer.pixel_colors = false;
        renderer.zoom = 1;
        renderer.render_to(&state, 0.0, &mut Vec::new()).unwrap();
        let (vx, vy, cx, cy, scale) = renderer.world_view.unwrap();
        assert_eq!(scale, 1);
        let x = 2 * 2 - vx + cx;
        let y = 9 - vy + cy;
        assert_eq!(renderer.back_buffer[y as usize * 160 + x as usize].ch, '!');
    }

    #[test]
    fn a_wall_occludes_a_creatures_overhanging_arm() {
        let mut state = fixture();
        for y in 7..10 {
            state.world.set_block(3, y, super::super::BlockType::Stone);
        }
        let mut baseline = Renderer::with_size(160, 48);
        baseline.render_to(&state, 0.0, &mut Vec::new()).unwrap();
        state
            .zombies
            .push(crate::entities::zombie::Zombie::new(2.7, 10.0));
        let mut actual = Renderer::with_size(160, 48);
        actual.render_to(&state, 0.0, &mut Vec::new()).unwrap();
        let (vx, vy, cx, cy, scale) = actual.world_view.unwrap();
        let x = 3 * scale * 2 - vx + cx;
        for by in 7..10 {
            let y = by * scale - vy + cy;
            for dy in 0..scale {
                let i = (y + dy) as usize * 160 + x as usize;
                assert!(
                    actual.back_buffer[i] == baseline.back_buffer[i],
                    "opaque wall was overpainted"
                );
            }
        }
    }
    #[test]
    fn enderman_identity_pixels_survive_both_pixel_view_resolutions() {
        let mut state = fixture();
        state
            .endermen
            .push(crate::entities::enderman::Enderman::new(5.0, 10.0));
        let eye = Renderer::rgb((214, 133, 244));
        for (w, h) in [(120, 40), (160, 48)] {
            let mut renderer = Renderer::with_size(w, h);
            renderer.render_to(&state, 0.0, &mut Vec::new()).unwrap();
            assert!(
                renderer
                    .back_buffer
                    .iter()
                    .any(|c| c.fg == eye || c.bg == eye),
                "Enderman eyes lost at {w}x{h}"
            );
        }
    }

    #[test]
    fn dragon_eyes_survive_facing_and_wing_changes_in_each_pixel_view() {
        let mut state = fixture();
        state.current_dimension = crate::world::Dimension::End;
        let eye = Renderer::rgb((244, 132, 246));
        for front in [false, true] {
            for zoom in [0, 2] {
                for (w, h) in [(120, 40), (160, 48), (200, 60)] {
                    for facing_right in [false, true] {
                        for age in [0, 6, 11] {
                            for x in [0.5, 6.0] {
                                let mut dragon =
                                    crate::entities::ender_dragon::EnderDragon::new(x, 7.0);
                                dragon.facing_right = facing_right;
                                dragon.age = age;
                                state.ender_dragon = Some(dragon);
                                let mut renderer = Renderer::with_size(w, h);
                                renderer.set_dragon_front_preview(front);
                                renderer.zoom = zoom;
                                renderer.render_to(&state, 0.0, &mut Vec::new()).unwrap();
                                assert!(
                                    renderer
                                        .back_buffer
                                        .iter()
                                        .any(|cell| cell.fg == eye || cell.bg == eye),
                                    "dragon eyes lost (front={front}, zoom={zoom}) at {w}x{h}, facing right {facing_right}, age {age}, x {x}"
                                );
                                let row = |y: usize| {
                                    renderer.back_buffer[y * w as usize..(y + 1) * w as usize]
                                        .iter()
                                        .map(|c| c.ch)
                                        .collect::<String>()
                                };
                                assert!(row(1).contains("Ender Dragon"));
                                assert!(row(3).contains("Armor"));
                                assert!(
                                    !row(h as usize - 10).contains("Too Far"),
                                    "idle pointer must not cover boss airspace"
                                );
                            }
                        }
                    }
                }
            }
        }
    }
}
