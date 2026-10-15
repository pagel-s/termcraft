use crate::engine::state::{
    ARMOR_UI_OFFSET, CRAFT_GRID_UI_OFFSET, CRAFT_OUTPUT_UI_SLOT, GameState, PrecipitationType,
};
use crate::world::Dimension;
#[path = "items.rs"]
mod items;
#[path = "materials.rs"]
mod materials;
#[path = "motion.rs"]
mod motion;
#[path = "nether.rs"]
mod nether;
#[path = "shading.rs"]
mod shading;
#[path = "sprites.rs"]
mod sprites;
#[path = "weather.rs"]
mod weather;
use crate::world::block::BlockType;
use crate::world::chunk::CHUNK_HEIGHT;
use crate::world::item::{ItemStack, ItemType, Recipe};
use crossterm::{
    cursor::{Hide, MoveTo, Show},
    event::{
        DisableBracketedPaste, DisableFocusChange, DisableMouseCapture, EnableBracketedPaste,
        EnableFocusChange, EnableMouseCapture, KeyboardEnhancementFlags,
        PopKeyboardEnhancementFlags, PushKeyboardEnhancementFlags,
    },
    execute, queue,
    style::{Color, Print, ResetColor, SetBackgroundColor, SetForegroundColor},
    terminal::{
        BeginSynchronizedUpdate, Clear, ClearType, EndSynchronizedUpdate, EnterAlternateScreen,
        LeaveAlternateScreen, disable_raw_mode, enable_raw_mode, size,
        supports_keyboard_enhancement,
    },
};
use sprites::{ActorSprite, SpriteKind};
use std::io::{Stdout, Write, stdout};

#[derive(Clone, Copy, PartialEq)]
struct RenderCell {
    ch: char,
    fg: Color,
    bg: Color,
}

impl Default for RenderCell {
    fn default() -> Self {
        Self {
            ch: ' ',
            fg: Color::Reset,
            bg: Color::Reset,
        }
    }
}

pub struct Renderer {
    stdout: Stdout,
    width: u16,
    height: u16,
    buffer: Vec<RenderCell>,
    back_buffer: Vec<RenderCell>,
    light_buffer: Vec<u8>,
    light_scratch: Vec<u8>,
    light_frontier: [Vec<usize>; 16],
    frame_output: Vec<u8>,
    precipitation_buffer: Vec<PrecipitationType>,
    camera_y: f64,
    camera_initialized: bool,
    key_releases: bool,
    world_view: Option<(i32, i32, i32, i32, i32)>,
    zoom: u8,
    pixel_colors: bool,
    dragon_art_preview: Option<bool>,
    actor_sprites: Vec<ActorSprite>,
    motion: motion::MotionHistory,
    player_preview: Option<Box<motion::PlayerPreview>>,
    frame_seconds: f64,
    last_frame_at: Option<std::time::Instant>,
    vertical_pixel_offset: i32,
    pixel_horizons: Vec<i32>,
}

impl Renderer {
    /// Select side/front art for comparison and allow the detailed review zoom.
    /// The live default is frontal; `false` retains the historical art comparison.
    pub fn set_dragon_front_preview(&mut self, enabled: bool) {
        self.dragon_art_preview = Some(enabled);
    }

    pub fn new() -> std::io::Result<Self> {
        let (cols, rows) = size()?;
        Ok(Self::with_size(cols, rows))
    }

    /// Offscreen rendering uses the same composition and ANSI output as live play.
    pub fn with_size(cols: u16, rows: u16) -> Self {
        let cols = cols.max(1);
        let rows = rows.max(1);
        let buffer_size = (cols as usize) * (rows as usize);
        Self {
            stdout: stdout(),
            width: cols,
            height: rows,
            buffer: vec![RenderCell::default(); buffer_size],
            back_buffer: vec![RenderCell::default(); buffer_size],
            light_buffer: Vec::new(),
            light_scratch: Vec::new(),
            light_frontier: std::array::from_fn(|_| Vec::new()),
            frame_output: Vec::new(),
            precipitation_buffer: Vec::new(),
            camera_y: 0.0,
            camera_initialized: false,
            key_releases: false,
            world_view: None,
            zoom: 0,
            pixel_colors: true,
            dragon_art_preview: None,
            actor_sprites: Vec::new(),
            motion: motion::MotionHistory::default(),
            player_preview: None,
            frame_seconds: 1.0 / 60.0,
            last_frame_at: None,
            vertical_pixel_offset: 0,
            pixel_horizons: Vec::new(),
        }
    }

    pub fn init(&mut self) -> std::io::Result<()> {
        enable_raw_mode()?;
        if std::env::var_os("NO_COLOR").is_some() {
            self.zoom = 1;
            self.pixel_colors = false;
        }
        // Capability replies do not prove that releases make it through a terminal
        // or multiplexer. Stay bounded until a control key-up is actually received.
        let _enhancement_available = supports_keyboard_enhancement().unwrap_or(false);
        self.key_releases = false;
        execute!(
            self.stdout,
            EnterAlternateScreen,
            EnableMouseCapture,
            EnableBracketedPaste,
            EnableFocusChange,
            PushKeyboardEnhancementFlags(
                KeyboardEnhancementFlags::REPORT_EVENT_TYPES
                    | KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES
                    | KeyboardEnhancementFlags::REPORT_ALL_KEYS_AS_ESCAPE_CODES
            ),
            Hide,
            Clear(ClearType::All)
        )?;
        Ok(())
    }

    pub fn restore(&mut self) -> std::io::Result<()> {
        let screen = execute!(
            self.stdout,
            EndSynchronizedUpdate,
            PopKeyboardEnhancementFlags,
            DisableMouseCapture,
            DisableBracketedPaste,
            DisableFocusChange,
            Show,
            LeaveAlternateScreen,
            ResetColor
        );
        let raw_mode = disable_raw_mode();
        screen.and(raw_mode)
    }

    pub fn screen_to_world(&self, state: &GameState, screen_x: u16, screen_y: u16) -> (i32, i32) {
        let (view_x, view_y, center_x, center_y, scale) = self.world_view.unwrap_or_else(|| {
            (
                (state.player.x * 2.0).round() as i32,
                self.view_char_y(state.player.y),
                (self.width / 2) as i32,
                (self.height as i32 * 2) / 3,
                1,
            )
        });
        let block_x = (screen_x as i32 + view_x - center_x).div_euclid(2 * scale);
        let block_y = (screen_y as i32 * 2 + 1 + view_y * 2 + self.vertical_pixel_offset
            - center_y * 2)
            .div_euclid(scale * 2);
        (block_x, block_y)
    }

    pub fn toggle_zoom(&mut self) {
        if !self.pixel_colors {
            return;
        }
        self.zoom = (self.zoom + 1) % 3;
        // Picking continues to use the image still on screen until the next frame.
        self.buffer.fill(RenderCell::default());
    }

    pub fn reports_key_releases(&self) -> bool {
        self.key_releases
    }

    pub fn note_key_release(&mut self) {
        self.key_releases = true;
    }

    fn check_resize(&mut self) -> std::io::Result<()> {
        let (cols, rows) = size()?;
        if cols != self.width || rows != self.height {
            self.width = cols;
            self.height = rows;
            let buffer_size = (cols as usize) * (rows as usize);
            self.buffer = vec![RenderCell::default(); buffer_size];
            self.back_buffer = vec![RenderCell::default(); buffer_size];
            execute!(self.stdout, Clear(ClearType::All))?;
        }
        Ok(())
    }

    fn update_vertical_camera(&mut self, target_y: f64) {
        if !self.camera_initialized {
            self.camera_y = target_y;
            self.camera_initialized = true;
            return;
        }

        let delta = target_y - self.camera_y;
        if delta.abs() > 12.0 {
            self.camera_y = target_y;
            return;
        }

        // A dead zone keeps the terrain steady during ordinary one-block jumps.
        // Exponential time-based follow has the same response at 30, 60 or 120 fps.
        let outside = if delta < -1.5 {
            delta + 1.5
        } else if delta > 1.0 {
            delta - 1.0
        } else {
            0.0
        };
        self.camera_y += outside * (1.0 - (-12.0 * self.frame_seconds.clamp(0.0, 0.1)).exp());
    }

    pub fn capture_motion_step(&mut self, state: &GameState) {
        self.motion.capture(state);
    }

    fn view_char_y(&self, player_y: f64) -> i32 {
        if self.camera_initialized {
            self.camera_y.floor() as i32
        } else {
            player_y.floor() as i32
        }
    }

    fn clear_back_buffer(&mut self) {
        self.back_buffer.fill(RenderCell::default());
    }

    fn inherited_background_at_idx(&self, idx: usize) -> Color {
        let cell = self.back_buffer[idx];
        if cell.bg != Color::Reset {
            return cell.bg;
        }
        if cell.ch != ' ' && cell.fg != Color::Reset {
            return Self::dim_color(cell.fg, 0.32);
        }

        let width = self.width as usize;
        for neighbor_idx in [
            idx.checked_sub(width),
            (idx + width < self.back_buffer.len()).then_some(idx + width),
            (!idx.is_multiple_of(width)).then_some(idx - 1),
            (idx % width != width.saturating_sub(1)).then_some(idx + 1),
        ]
        .into_iter()
        .flatten()
        {
            let neighbor = self.back_buffer[neighbor_idx];
            if neighbor.bg != Color::Reset {
                return neighbor.bg;
            }
            if neighbor.ch != ' ' && neighbor.fg != Color::Reset {
                return Self::dim_color(neighbor.fg, 0.32);
            }
        }

        Color::Reset
    }

    fn put_char(&mut self, x: i32, y: i32, ch: char, fg: Color, bg: Color) {
        if x >= 0 && x < self.width as i32 && y >= 0 && y < self.height as i32 {
            let idx = (y as usize) * (self.width as usize) + (x as usize);
            let resolved_bg = if bg == Color::Reset {
                self.inherited_background_at_idx(idx)
            } else {
                bg
            };
            self.back_buffer[idx] = RenderCell {
                ch,
                fg,
                bg: resolved_bg,
            };
        }
    }

    fn put_str(&mut self, x: i32, y: i32, s: &str, fg: Color, bg: Color) {
        for (i, c) in s.chars().enumerate() {
            self.put_char(x + i as i32, y, c, fg, bg);
        }
    }

    fn put_world_char(&mut self, x: i32, y: i32, ch: char, fg: Color, bg: Color) {
        if x >= 0 && x < self.width as i32 && y >= 0 && y < self.height as i32 {
            let idx = (y as usize) * (self.width as usize) + (x as usize);
            self.back_buffer[idx] = RenderCell { ch, fg, bg };
        }
    }

    fn rgb_luma(rgb: (u8, u8, u8)) -> f32 {
        rgb.0 as f32 * 0.2126 + rgb.1 as f32 * 0.7152 + rgb.2 as f32 * 0.0722
    }

    fn visibility_adjusted_entity_color_at(
        &self,
        x: i32,
        y: i32,
        fg: Color,
        player: bool,
    ) -> Color {
        if x < 0 || x >= self.width as i32 || y < 0 || y >= self.height as i32 {
            return fg;
        }

        let Some(fg_rgb) = Self::color_rgb(fg) else {
            return fg;
        };

        let idx = (y as usize) * self.width as usize + x as usize;
        let Some(bg_rgb) = Self::color_rgb(self.inherited_background_at_idx(idx)) else {
            return fg;
        };

        let contrast = (Self::rgb_luma(fg_rgb) - Self::rgb_luma(bg_rgb)).abs();
        let bg_luma = Self::rgb_luma(bg_rgb);
        let bg_is_very_dark = bg_luma < 44.0;
        let bg_is_nether_dark =
            bg_is_very_dark && bg_rgb.0 > bg_rgb.1.saturating_add(8) && bg_rgb.0 > bg_rgb.2;
        let channel_distance = ((fg_rgb.0.abs_diff(bg_rgb.0) as f32)
            + (fg_rgb.1.abs_diff(bg_rgb.1) as f32)
            + (fg_rgb.2.abs_diff(bg_rgb.2) as f32))
            / 3.0;
        let has_enough_contrast = if player {
            if bg_is_nether_dark {
                contrast >= 88.0 && channel_distance >= 120.0
            } else {
                contrast >= 72.0 && channel_distance >= 96.0
            }
        } else {
            contrast >= 58.0 && channel_distance >= 78.0
        };
        if has_enough_contrast {
            return fg;
        }

        let target = if bg_luma >= 132.0 {
            if player { (4, 8, 16) } else { (16, 16, 20) }
        } else if player && bg_is_nether_dark {
            (255, 246, 232)
        } else if player {
            (255, 252, 248)
        } else {
            (244, 244, 244)
        };
        let strength = if contrast < 28.0 || channel_distance < 52.0 {
            if player && bg_is_nether_dark {
                0.64
            } else if player {
                0.48
            } else {
                0.26
            }
        } else if player {
            if bg_is_nether_dark { 0.46 } else { 0.30 }
        } else {
            0.14
        };
        Self::rgb(Self::lerp_rgb(fg_rgb, target, strength))
    }

    fn entity_visibility_color_at(&self, x: i32, y: i32, fg: Color) -> Color {
        self.visibility_adjusted_entity_color_at(x, y, fg, false)
    }

    #[cfg(test)]
    fn player_visibility_color_at(&self, x: i32, y: i32, fg: Color) -> Color {
        self.visibility_adjusted_entity_color_at(x, y, fg, true)
    }

    fn put_entity_char(&mut self, x: i32, y: i32, ch: char, fg: Color) {
        let fg = self.entity_visibility_color_at(x, y, fg);
        self.put_char(x, y, ch, fg, Color::Reset);
    }

    fn put_entity_str(&mut self, x: i32, y: i32, s: &str, fg: Color) {
        for (i, c) in s.chars().enumerate() {
            self.put_entity_char(x + i as i32, y, c, fg);
        }
    }

    fn ghast_sprite(facing_right: bool) -> [&'static str; 3] {
        [" /\\ ", "(00)", if facing_right { "v vv" } else { "vv v" }]
    }

    fn blaze_sprite(age: u64) -> [&'static str; 2] {
        [
            "B ",
            if (age / 3).is_multiple_of(2) {
                "><"
            } else {
                "<>"
            },
        ]
    }

    fn dim_color(color: Color, factor: f32) -> Color {
        let f = factor.clamp(0.0, 1.0);
        match color {
            Color::Reset => Color::Reset,
            Color::Rgb { r, g, b } => Color::Rgb {
                r: (r as f32 * f) as u8,
                g: (g as f32 * f) as u8,
                b: (b as f32 * f) as u8,
            },
            Color::White => Color::Rgb {
                r: (255.0 * f) as u8,
                g: (255.0 * f) as u8,
                b: (255.0 * f) as u8,
            },
            Color::Grey => Color::Rgb {
                r: (190.0 * f) as u8,
                g: (190.0 * f) as u8,
                b: (190.0 * f) as u8,
            },
            Color::DarkGrey => Color::Rgb {
                r: (120.0 * f) as u8,
                g: (120.0 * f) as u8,
                b: (120.0 * f) as u8,
            },
            Color::Yellow => Color::Rgb {
                r: (255.0 * f) as u8,
                g: (220.0 * f) as u8,
                b: (80.0 * f) as u8,
            },
            Color::AnsiValue(_) => {
                let base = (6.0 + 22.0 * f) as u8;
                Color::Rgb {
                    r: base,
                    g: base,
                    b: (base + 2).min(30),
                }
            }
            _ => color,
        }
    }

    fn lerp_u8(a: u8, b: u8, t: f32) -> u8 {
        let t = t.clamp(0.0, 1.0);
        (a as f32 + (b as f32 - a as f32) * t) as u8
    }

    fn lerp_rgb(a: (u8, u8, u8), b: (u8, u8, u8), t: f32) -> (u8, u8, u8) {
        (
            Self::lerp_u8(a.0, b.0, t),
            Self::lerp_u8(a.1, b.1, t),
            Self::lerp_u8(a.2, b.2, t),
        )
    }

    fn rgb(rgb: (u8, u8, u8)) -> Color {
        Color::Rgb {
            r: rgb.0,
            g: rgb.1,
            b: rgb.2,
        }
    }

    fn target_background(background: Color) -> Color {
        Self::rgb(Self::lerp_rgb(
            Self::color_rgb(background).unwrap_or((24, 24, 28)),
            (245, 195, 75),
            0.38,
        ))
    }

    fn color_rgb(color: Color) -> Option<(u8, u8, u8)> {
        match color {
            Color::Reset => None,
            Color::Black => Some((0, 0, 0)),
            Color::DarkGrey => Some((120, 120, 120)),
            Color::Grey => Some((190, 190, 190)),
            Color::White => Some((255, 255, 255)),
            Color::DarkRed => Some((152, 42, 42)),
            Color::Red => Some((255, 72, 72)),
            Color::DarkGreen => Some((42, 120, 42)),
            Color::Green => Some((72, 188, 72)),
            Color::DarkYellow => Some((172, 132, 58)),
            Color::Yellow => Some((255, 220, 80)),
            Color::DarkBlue => Some((56, 84, 152)),
            Color::Blue => Some((88, 144, 236)),
            Color::DarkMagenta => Some((136, 74, 168)),
            Color::Magenta => Some((208, 112, 236)),
            Color::DarkCyan => Some((56, 154, 154)),
            Color::Cyan => Some((92, 226, 232)),
            Color::Rgb { r, g, b } => Some((r, g, b)),
            _ => None,
        }
    }

    fn scale_rgb(rgb: (u8, u8, u8), factor: f32) -> (u8, u8, u8) {
        let f = factor.clamp(0.0, 1.0);
        (
            (rgb.0 as f32 * f) as u8,
            (rgb.1 as f32 * f) as u8,
            (rgb.2 as f32 * f) as u8,
        )
    }

    fn player_skin_rgb() -> (u8, u8, u8) {
        (236, 196, 156)
    }

    fn player_tunic_rgb() -> (u8, u8, u8) {
        (68, 102, 176)
    }

    fn player_lower_rgb() -> (u8, u8, u8) {
        (66, 58, 108)
    }

    fn armor_tint_rgb(item_type: ItemType) -> Option<(u8, u8, u8)> {
        match item_type {
            ItemType::LeatherHelmet
            | ItemType::LeatherChestplate
            | ItemType::LeatherLeggings
            | ItemType::LeatherBoots => Some((128, 82, 50)),
            ItemType::IronHelmet
            | ItemType::IronChestplate
            | ItemType::IronLeggings
            | ItemType::IronBoots => Some((216, 224, 232)),
            ItemType::DiamondHelmet
            | ItemType::DiamondChestplate
            | ItemType::DiamondLeggings
            | ItemType::DiamondBoots => Some((92, 236, 224)),
            _ => None,
        }
    }

    fn player_visible_colors(state: &GameState, light_factor: f32) -> (Color, Color, Color) {
        let skin_rgb = Self::scale_rgb(Self::player_skin_rgb(), light_factor);
        let tunic_rgb = Self::scale_rgb(Self::player_tunic_rgb(), light_factor);
        let lower_rgb = Self::scale_rgb(Self::player_lower_rgb(), light_factor);

        let head_rgb = state
            .armor_slot_item(0)
            .and_then(|stack| Self::armor_tint_rgb(stack.item_type))
            .map(|rgb| Self::scale_rgb(rgb, light_factor))
            .unwrap_or(skin_rgb);
        let torso_rgb = state
            .armor_slot_item(1)
            .and_then(|stack| Self::armor_tint_rgb(stack.item_type))
            .map(|rgb| Self::scale_rgb(rgb, light_factor))
            .unwrap_or(tunic_rgb);
        let lower_armor_rgb = state
            .armor_slot_item(2)
            .and_then(|stack| Self::armor_tint_rgb(stack.item_type))
            .map(|rgb| Self::scale_rgb(rgb, light_factor));
        let boots_rgb = state
            .armor_slot_item(3)
            .and_then(|stack| Self::armor_tint_rgb(stack.item_type))
            .map(|rgb| Self::scale_rgb(rgb, light_factor));
        let limb_rgb = boots_rgb
            .map(|boot_rgb| Self::lerp_rgb(lower_armor_rgb.unwrap_or(lower_rgb), boot_rgb, 0.6))
            .or(lower_armor_rgb)
            .unwrap_or(lower_rgb);

        (
            Self::rgb(head_rgb),
            Self::rgb(torso_rgb),
            Self::rgb(limb_rgb),
        )
    }

    fn remote_player_visible_colors(client_id: u16, light_factor: f32) -> (Color, Color, Color) {
        let palette = match client_id % 6 {
            0 => ((228, 210, 186), (88, 170, 214), (48, 102, 170)),
            1 => ((232, 214, 192), (204, 122, 88), (134, 72, 48)),
            2 => ((224, 214, 188), (128, 176, 92), (68, 112, 54)),
            3 => ((232, 210, 194), (182, 98, 154), (118, 64, 106)),
            4 => ((222, 212, 194), (112, 154, 170), (62, 96, 118)),
            _ => ((232, 214, 188), (198, 158, 76), (132, 92, 38)),
        };
        (
            Self::rgb(Self::scale_rgb(palette.0, light_factor)),
            Self::rgb(Self::scale_rgb(palette.1, light_factor)),
            Self::rgb(Self::scale_rgb(palette.2, light_factor)),
        )
    }

    fn end_victory_banner_lines(ticks_remaining: u16) -> (&'static str, &'static str) {
        if ticks_remaining > 92 {
            ("ENDER DRAGON SLAIN", "Shockwaves tear through the End")
        } else if ticks_remaining > 46 {
            ("THE END TREMBLES", "Ancient light pours from the core")
        } else {
            ("VICTORY IN THE END", "Enter the portal when you are ready")
        }
    }

    fn render_end_victory_sequence(
        &mut self,
        state: &GameState,
        view_char_x: i32,
        view_char_y: i32,
        screen_center_x: i32,
        screen_center_y: i32,
        ui_y: i32,
    ) {
        let Some((ticks_remaining, origin_x, origin_y)) = state.end_victory_sequence_state() else {
            return;
        };

        let elapsed = (140u16.saturating_sub(ticks_remaining)) as f32;
        let progress = (elapsed / 140.0).clamp(0.0, 1.0);
        let pulse = ((elapsed * 0.24).sin() * 0.5 + 0.5).clamp(0.0, 1.0);
        let scale = self.world_view.map_or(1, |view| view.4);
        let origin_sx =
            (origin_x * 2.0 * scale as f64).round() as i32 - view_char_x + screen_center_x;
        let origin_sy = (origin_y * scale as f64).floor() as i32 - view_char_y + screen_center_y;
        let violet = Self::rgb(Self::lerp_rgb(
            (168, 96, 248),
            (255, 255, 255),
            pulse * 0.45,
        ));
        let cyan = Self::rgb(Self::lerp_rgb(
            (80, 228, 255),
            (255, 235, 255),
            pulse * 0.35,
        ));
        let gold = Self::rgb(Self::lerp_rgb(
            (255, 188, 64),
            (255, 240, 190),
            pulse * 0.25,
        ));

        for beam_step in 0..9 {
            let sy = origin_sy - beam_step;
            if sy < 0 || sy >= ui_y {
                continue;
            }
            let fg = if beam_step % 2 == 0 { violet } else { cyan };
            self.put_char(origin_sx, sy, '|', fg, Color::Reset);
            if beam_step > 1 {
                self.put_char(origin_sx - 1, sy, '.', cyan, Color::Reset);
                self.put_char(origin_sx + 1, sy, '.', cyan, Color::Reset);
            }
        }

        for ring in 0..3 {
            let radius = 2.0 + progress * (8.0 + ring as f32 * 6.0) - ring as f32 * 2.2;
            if radius <= 1.0 {
                continue;
            }
            for ray in 0..28 {
                let angle = ray as f32 * 0.224 + elapsed * 0.08 + ring as f32 * 0.5;
                let sx = origin_sx + (angle.cos() * radius * 2.0).round() as i32;
                let sy = origin_sy + (angle.sin() * radius).round() as i32;
                if sx < 0 || sx >= self.width as i32 || sy < 0 || sy >= ui_y {
                    continue;
                }
                let fg = match ring {
                    0 => violet,
                    1 => cyan,
                    _ => gold,
                };
                let glyph = match (ring + ray) % 3 {
                    0 => '*',
                    1 => '+',
                    _ => '.',
                };
                self.put_char(sx, sy, glyph, fg, Color::Reset);
            }
        }

        for shard in 0..24 {
            let angle = shard as f32 * 0.41 + elapsed * 0.11;
            let distance = 1.5 + progress * 18.0 + (shard % 4) as f32 * 1.2;
            let sx = origin_sx + (angle.cos() * distance * 2.0).round() as i32;
            let sy = origin_sy + (angle.sin() * distance).round() as i32;
            if sx < 0 || sx >= self.width as i32 || sy < 0 || sy >= ui_y {
                continue;
            }
            let fg = if shard % 2 == 0 { cyan } else { violet };
            let glyph = if shard % 3 == 0 { '*' } else { '.' };
            self.put_char(sx, sy, glyph, fg, Color::Reset);
        }

        let (headline, subline) = Self::end_victory_banner_lines(ticks_remaining);
        let banner_y = 2;
        let banner_x = ((self.width as i32 - headline.len() as i32) / 2).max(1);
        let banner_fg = Self::rgb(Self::lerp_rgb(
            (210, 180, 255),
            (255, 255, 255),
            pulse * 0.6,
        ));
        self.put_str(banner_x - 4, banner_y, "*** ", cyan, Color::Reset);
        self.put_str(banner_x, banner_y, headline, banner_fg, Color::Reset);
        self.put_str(
            banner_x + headline.len() as i32,
            banner_y,
            " ***",
            cyan,
            Color::Reset,
        );
        let sub_x = ((self.width as i32 - subline.len() as i32) / 2).max(1);
        self.put_str(sub_x, banner_y + 1, subline, gold, Color::Reset);
    }

    fn lit_rgb(rgb: (u8, u8, u8), factor: f32) -> Color {
        Self::rgb(Self::scale_rgb(rgb, factor))
    }

    fn flat_style(ch: char, fg: (u8, u8, u8), light_factor: f32) -> (char, Color, Color) {
        (ch, Self::lit_rgb(fg, light_factor), Color::Reset)
    }

    fn soften_material_bg(fg: (u8, u8, u8), bg: (u8, u8, u8)) -> (u8, u8, u8) {
        Self::lerp_rgb(bg, fg, 0.58)
    }

    fn soften_covered_material_bg(fg: (u8, u8, u8), bg: (u8, u8, u8)) -> (u8, u8, u8) {
        // Many terminal fonts leave a thin amount of background visible at the
        // top of a cell. For covered blocks, keep the material backdrop much
        // closer to the foreground so that line is far less noticeable.
        Self::lerp_rgb(bg, fg, 0.86)
    }

    fn textured_style(
        ch: char,
        fg: (u8, u8, u8),
        bg: (u8, u8, u8),
        light_factor: f32,
    ) -> (char, Color, Color) {
        (
            ch,
            Self::lit_rgb(fg, light_factor),
            Self::lit_rgb(Self::soften_material_bg(fg, bg), light_factor),
        )
    }

    fn covered_textured_style(
        ch: char,
        fg: (u8, u8, u8),
        bg: (u8, u8, u8),
        light_factor: f32,
    ) -> (char, Color, Color) {
        (
            ch,
            Self::lit_rgb(fg, light_factor),
            Self::lit_rgb(Self::soften_covered_material_bg(fg, bg), light_factor),
        )
    }

    fn top_surface_exposed(block_above: BlockType) -> bool {
        !Self::sky_backdrop_occluder(block_above)
    }

    fn block_render_style(
        block: BlockType,
        block_above: BlockType,
        dimension: Dimension,
        light_factor: f32,
    ) -> (char, Color, Color) {
        let flat = |ch, fg| Self::flat_style(ch, fg, light_factor);
        let top_exposed = Self::top_surface_exposed(block_above);
        let textured = |ch, fg, bg| {
            if top_exposed {
                Self::textured_style(ch, fg, bg, light_factor)
            } else {
                Self::covered_textured_style(ch, fg, bg, light_factor)
            }
        };
        let nether_theme = dimension == Dimension::Nether;

        match block {
            BlockType::Air => (' ', Color::Reset, Color::Reset),
            BlockType::Dirt => textured('▒', (176, 120, 68), (100, 62, 30)),
            BlockType::Grass => {
                if top_exposed {
                    Self::textured_style('▀', (82, 196, 84), (120, 80, 42), light_factor)
                } else {
                    Self::covered_textured_style('▒', (140, 112, 66), (108, 74, 40), light_factor)
                }
            }
            BlockType::Stone => textured('▓', (150, 150, 156), (88, 88, 94)),
            BlockType::StoneBricks => {
                if nether_theme {
                    textured('#', (154, 88, 96), (96, 42, 46))
                } else {
                    textured('#', (156, 156, 162), (96, 96, 104))
                }
            }
            BlockType::Wood => textured('▒', (164, 114, 60), (102, 66, 30)),
            BlockType::Leaves => textured('▓', (76, 164, 86), (34, 96, 58)),
            BlockType::IronOre => textured('▓', (214, 186, 146), (92, 92, 98)),
            BlockType::GoldOre => textured('▓', (255, 224, 78), (92, 92, 98)),
            BlockType::DiamondOre => textured('▓', (72, 240, 240), (88, 98, 104)),
            BlockType::CoalOre => textured('▓', (64, 64, 70), (98, 98, 104)),
            BlockType::RedstoneOre => textured('▓', (188, 56, 56), (92, 92, 98)),
            BlockType::Sand => textured('▒', (240, 230, 150), (188, 166, 96)),
            BlockType::Gravel => textured('▒', (132, 132, 138), (82, 82, 88)),
            BlockType::Bedrock => textured('▓', (44, 44, 48), (18, 18, 20)),
            BlockType::Planks => textured('=', (210, 166, 94), (122, 82, 42)),
            BlockType::CraftingTable => textured('#', (194, 140, 80), (90, 60, 34)),
            BlockType::Furnace => textured('F', (142, 142, 148), (72, 72, 78)),
            BlockType::Bed => textured('H', (220, 78, 78), (96, 30, 30)),
            BlockType::Chest => textured('C', (190, 140, 72), (98, 62, 30)),
            BlockType::Bookshelf => textured('B', (182, 128, 74), (86, 50, 28)),
            BlockType::Glass => flat('"', (190, 220, 235)),
            BlockType::Wool => textured('▓', (236, 230, 218), (170, 160, 146)),
            BlockType::StoneSlab => {
                if nether_theme {
                    textured('=', (148, 84, 92), (88, 38, 44))
                } else {
                    textured('=', (148, 148, 154), (92, 92, 98))
                }
            }
            BlockType::StoneStairs => {
                if nether_theme {
                    textured('^', (152, 86, 94), (86, 36, 42))
                } else {
                    textured('^', (144, 144, 150), (88, 88, 94))
                }
            }
            BlockType::EnchantingTable => textured('E', (134, 88, 186), (56, 34, 88)),
            BlockType::Anvil => textured('A', (136, 136, 150), (62, 62, 74)),
            BlockType::BrewingStand => textured('b', (172, 132, 80), (68, 54, 30)),
            BlockType::Torch => flat('!', (255, 220, 80)),
            BlockType::Tnt => textured('T', (236, 60, 60), (112, 20, 20)),
            BlockType::PrimedTnt(fuse) => {
                let flash_on = (fuse / 2) % 2 == 0;
                if flash_on {
                    textured('T', (255, 255, 255), (132, 50, 50))
                } else {
                    textured('T', (255, 96, 96), (112, 20, 20))
                }
            }
            BlockType::Lever(on) => {
                let c = if on { '/' } else { '\\' };
                flat(c, (200, 200, 160))
            }
            BlockType::StoneButton(timer) => {
                let c = if timer > 0 { '*' } else { 'o' };
                flat(c, (168, 168, 168))
            }
            BlockType::RedstoneTorch(lit) => {
                let c = if lit { (255, 48, 48) } else { (96, 44, 44) };
                flat('!', c)
            }
            BlockType::Snow => {
                if top_exposed {
                    Self::textured_style('▀', (255, 255, 255), (210, 222, 236), light_factor)
                } else {
                    Self::covered_textured_style(
                        '▓',
                        (236, 238, 244),
                        (214, 220, 232),
                        light_factor,
                    )
                }
            }
            BlockType::Ice => flat('▓', (173, 216, 230)),
            BlockType::Cactus => textured('#', (34, 122, 34), (12, 64, 12)),
            BlockType::DeadBush => flat('*', (139, 69, 19)),
            BlockType::BirchWood => textured('▒', (238, 228, 190), (156, 136, 88)),
            BlockType::BirchLeaves => flat('▓', (110, 204, 110)),
            BlockType::RedFlower => flat('v', (255, 0, 0)),
            BlockType::YellowFlower => flat('v', (255, 255, 0)),
            BlockType::TallGrass => flat('w', (34, 139, 34)),
            BlockType::Sapling => flat('i', (96, 176, 84)),
            BlockType::BirchSapling => flat('i', (150, 206, 124)),
            BlockType::SugarCane => flat('|', (114, 214, 104)),
            BlockType::Water(_) => flat('~', (40, 92, 255)),
            BlockType::Lava(_) => flat(
                '~',
                if nether_theme {
                    (255, 138, 42)
                } else {
                    (255, 92, 24)
                },
            ),
            BlockType::Cobblestone => textured('▒', (124, 124, 130), (70, 70, 76)),
            BlockType::Obsidian => textured('▓', (72, 42, 86), (28, 16, 40)),
            BlockType::Netherrack => textured('%', (180, 96, 84), (124, 54, 44)),
            BlockType::SoulSand => textured('&', (164, 126, 82), (112, 76, 44)),
            BlockType::Glowstone => textured('*', (255, 236, 124), (196, 144, 36)),
            BlockType::NetherPortal => textured('O', (176, 88, 214), (98, 36, 138)),
            BlockType::EndPortalFrame { filled } => {
                let c = if filled { 'O' } else { 'o' };
                textured(c, (98, 146, 98), (38, 70, 42))
            }
            BlockType::EndPortal => textured('~', (97, 166, 158), (5, 14, 26)),
            BlockType::EndStone => textured('▓', (220, 214, 156), (148, 142, 92)),
            BlockType::IronDoor(open) => {
                let c = if open { '/' } else { '|' };
                flat(c, (190, 200, 210))
            }
            BlockType::WoodDoor(open) => {
                let c = if open { '/' } else { '|' };
                flat(c, (168, 118, 76))
            }
            BlockType::Ladder => flat('#', (176, 136, 84)),
            BlockType::SilverfishSpawner => textured('%', (104, 130, 156), (52, 66, 78)),
            BlockType::BlazeSpawner => textured('%', (255, 144, 78), (94, 42, 18)),
            BlockType::ZombieSpawner => textured('%', (96, 146, 92), (42, 78, 40)),
            BlockType::SkeletonSpawner => textured('%', (196, 206, 216), (90, 102, 116)),
            BlockType::RedstoneDust(level) => {
                let glow = 90u8.saturating_add(level.saturating_mul(10));
                let c = if level > 0 { ':' } else { '.' };
                flat(c, (glow, 24, 24))
            }
            BlockType::RedstoneRepeater {
                powered,
                facing_right,
                ..
            } => {
                let c = if facing_right { '>' } else { '<' };
                if powered {
                    flat(c, (235, 70, 70))
                } else {
                    flat(c, (170, 170, 170))
                }
            }
            BlockType::Piston {
                extended,
                facing_right,
            } => {
                let c = if extended {
                    if facing_right { '>' } else { '<' }
                } else if facing_right {
                    ']'
                } else {
                    '['
                };
                textured(c, (156, 156, 156), (78, 78, 78))
            }
            BlockType::StickyPiston {
                extended,
                facing_right,
            } => {
                let c = if extended {
                    if facing_right { '}' } else { '{' }
                } else if facing_right {
                    ')'
                } else {
                    '('
                };
                textured(c, (126, 192, 126), (54, 98, 54))
            }
            BlockType::Farmland(m) => {
                let fg = if m > 0 { (72, 38, 14) } else { (110, 68, 28) };
                textured('=', fg, (56, 32, 14))
            }
            BlockType::Crops(stage) => {
                let c = match stage {
                    0..=2 => (144, 238, 144),
                    3..=5 => (50, 205, 50),
                    _ => (218, 165, 32),
                };
                flat('"', c)
            }
            BlockType::NetherWart(stage) => {
                let c = match stage {
                    0 => (138, 54, 54),
                    1 => (170, 46, 46),
                    2 => (198, 34, 34),
                    _ => (220, 26, 26),
                };
                flat('&', c)
            }
        }
    }

    fn sky_backdrop_occluder(block: BlockType) -> bool {
        !matches!(
            block,
            BlockType::Air
                | BlockType::Leaves
                | BlockType::BirchLeaves
                | BlockType::Glass
                | BlockType::Ice
                | BlockType::Torch
                | BlockType::Lever(_)
                | BlockType::StoneButton(_)
                | BlockType::RedstoneTorch(_)
                | BlockType::RedstoneRepeater { .. }
                | BlockType::RedFlower
                | BlockType::YellowFlower
                | BlockType::TallGrass
                | BlockType::DeadBush
                | BlockType::RedstoneDust(_)
                | BlockType::NetherPortal
                | BlockType::EndPortal
                | BlockType::PrimedTnt(_)
                | BlockType::IronDoor(_)
                | BlockType::WoodDoor(_)
                | BlockType::Crops(_)
                | BlockType::Ladder
                | BlockType::Water(_)
                | BlockType::Lava(_)
                | BlockType::Sapling
                | BlockType::BirchSapling
                | BlockType::SugarCane
                | BlockType::NetherWart(_)
        )
    }

    fn sky_horizon_anchor(block: BlockType) -> bool {
        if matches!(
            block,
            BlockType::Wood | BlockType::BirchWood | BlockType::Leaves | BlockType::BirchLeaves
        ) {
            return false;
        }
        matches!(block, BlockType::Water(_) | BlockType::Ice) || Self::sky_backdrop_occluder(block)
    }

    fn smooth_horizon_profile(raw: &[i32]) -> Vec<i32> {
        let mut smoothed = vec![0; raw.len()];
        let mut window = Vec::with_capacity(9);
        for (idx, smoothed_y) in smoothed.iter_mut().enumerate() {
            window.clear();
            let start = idx.saturating_sub(4);
            let end = (idx + 5).min(raw.len());
            window.extend_from_slice(&raw[start..end]);
            window.sort_unstable();
            let mid = window.len() / 2;
            *smoothed_y = if window.len() % 2 == 0 {
                (window[mid - 1] + window[mid]) / 2
            } else {
                window[mid]
            };
        }
        smoothed
    }

    fn should_apply_overworld_depth_fog(block_y: i32, sky_horizon_y: i32) -> bool {
        block_y > 34 && block_y > sky_horizon_y
    }

    fn day_mix(tod: f32) -> f32 {
        if !(4000.0..=20000.0).contains(&tod) {
            0.0
        } else if tod < 8000.0 {
            (tod - 4000.0) / 4000.0
        } else if tod > 16000.0 {
            1.0 - ((tod - 16000.0) / 4000.0)
        } else {
            1.0
        }
    }

    fn twilight_mix(tod: f32) -> f32 {
        let sunrise = (1.0 - ((tod - 6000.0).abs() / 2200.0)).clamp(0.0, 1.0);
        let sunset = (1.0 - ((tod - 18000.0).abs() / 2200.0)).clamp(0.0, 1.0);
        sunrise.max(sunset)
    }

    fn overworld_sky_background(
        block_y: i32,
        sky_occluder_y: i32,
        tod: f32,
        precipitation: PrecipitationType,
        rain_mix: f32,
        thunder_mix: f32,
        thunder_flash_active: bool,
    ) -> Color {
        let day_mix = Self::day_mix(tod);
        let twilight_mix = Self::twilight_mix(tod);
        let mut horizon = Self::lerp_rgb((22, 28, 50), (138, 184, 236), day_mix);
        let mut zenith = Self::lerp_rgb((6, 10, 24), (76, 134, 214), day_mix);

        horizon = Self::lerp_rgb(horizon, (255, 188, 126), twilight_mix * 0.62);
        zenith = Self::lerp_rgb(zenith, (126, 144, 220), twilight_mix * 0.28);

        if precipitation != PrecipitationType::None {
            horizon = Self::lerp_rgb(horizon, (82, 92, 116), 0.38 + rain_mix * 0.24);
            zenith = Self::lerp_rgb(zenith, (38, 48, 78), 0.38 + rain_mix * 0.24);
        }

        let storm_mix = (rain_mix * 0.22) + (thunder_mix * 0.36);
        if storm_mix > 0.0 {
            horizon = Self::lerp_rgb(horizon, (54, 60, 76), storm_mix.clamp(0.0, 0.7));
            zenith = Self::lerp_rgb(zenith, (20, 28, 48), storm_mix.clamp(0.0, 0.8));
        }

        if thunder_flash_active {
            horizon = Self::lerp_rgb(horizon, (124, 132, 154), 0.5);
            zenith = Self::lerp_rgb(zenith, (94, 106, 142), 0.42);
        }

        let height_mix = (((sky_occluder_y - block_y - 1) as f32) / 18.0)
            .clamp(0.0, 1.0)
            .powf(0.72);
        let mut sky_rgb = Self::lerp_rgb(horizon, zenith, height_mix);
        if day_mix > 0.0 {
            let horizon_soften = ((1.0 - height_mix).powf(1.2) * day_mix * 0.18).clamp(0.0, 0.18);
            if horizon_soften > 0.0 {
                let haze = Self::lerp_rgb((74, 88, 112), (214, 222, 232), day_mix);
                sky_rgb = Self::lerp_rgb(sky_rgb, haze, horizon_soften);
            }
        }
        Self::rgb(sky_rgb)
    }

    fn skylight_column_step(current_light: u8, block: BlockType) -> (u8, u8) {
        let cell_light = if matches!(block, BlockType::Leaves | BlockType::BirchLeaves) {
            current_light.saturating_sub(1)
        } else if matches!(block, BlockType::Wood | BlockType::BirchWood) {
            current_light.saturating_sub(3)
        } else if block.is_solid() {
            current_light.saturating_sub(2)
        } else if block.is_fluid() {
            current_light.saturating_sub(1)
        } else {
            current_light
        };

        let next_light = if matches!(block, BlockType::Leaves | BlockType::BirchLeaves) {
            cell_light
        } else if matches!(block, BlockType::Wood | BlockType::BirchWood) {
            current_light.saturating_sub(5)
        } else if block.is_solid() {
            0
        } else if block.is_fluid() {
            cell_light
        } else {
            current_light
        };

        (cell_light, next_light)
    }

    fn overworld_cave_air_background(light_val: u8) -> Option<Color> {
        if light_val == 0 {
            return None;
        }

        let glow = ((light_val.min(15) as f32) / 15.0).powf(1.15);
        Some(Self::rgb(Self::lerp_rgb((10, 12, 16), (54, 64, 84), glow)))
    }

    fn nether_air_background(block_y: i32, light_val: u8) -> Color {
        let depth = ((block_y as f32 - 10.0) / 108.0).clamp(0.0, 1.0);
        let mut base = Self::lerp_rgb((26, 8, 10), (66, 22, 16), depth);
        let ambient_glow = ((light_val.min(15) as f32) / 15.0).powf(1.05);
        base = Self::lerp_rgb(base, (112, 44, 18), ambient_glow * 0.32);
        Self::rgb(base)
    }

    pub fn render(&mut self, state: &GameState, frame_alpha: f64) -> std::io::Result<()> {
        self.check_resize()?;
        let now = std::time::Instant::now();
        self.frame_seconds = self
            .last_frame_at
            .map_or(1.0 / 60.0, |last| now.duration_since(last).as_secs_f64());
        self.last_frame_at = Some(now);
        self.render_to(state, frame_alpha, &mut stdout().lock())
    }

    pub fn render_to<W: Write>(
        &mut self,
        state: &GameState,
        frame_alpha: f64,
        output: &mut W,
    ) -> std::io::Result<()> {
        self.clear_back_buffer();
        self.actor_sprites.clear();
        let physical_size = (self.width, self.height);
        let boss_view = state.current_dimension == Dimension::End && state.ender_dragon.is_some();
        // Start in a useful pixel overview. Detailed art is an explicit view,
        // rather than automatically halving world context on a larger terminal.
        let scale = if self.pixel_colors && self.zoom != 1 && self.width >= 100 && self.height >= 32
        {
            if self.zoom == 2
                && (!boss_view || self.dragon_art_preview.is_some())
                && self.width >= 140
                && self.height >= 44
            {
                4
            } else {
                2
            }
        } else {
            1
        };
        if scale > 1 {
            self.width = self.width / scale as u16 + 2;
            self.height = (self.height - 4) / scale as u16 + 5;
        }
        let frame_alpha = frame_alpha.clamp(0.0, 1.0);
        if self
            .motion
            .dimension
            .is_some_and(|d| d != state.current_dimension)
        {
            self.motion = motion::MotionHistory::default();
            self.camera_initialized = false;
        }
        let preview = self
            .player_preview
            .get_or_insert_with(|| Box::new(motion::PlayerPreview::new()));
        let (render_player_x, render_player_y) = preview.position(state, frame_alpha);

        let camera_target = if boss_view {
            render_player_y - 3.0
        } else {
            render_player_y
        };
        self.update_vertical_camera(camera_target);

        let physical_view_x = (render_player_x * 2.0 * scale as f64).round() as i32;
        let physical_view_y = (self.camera_y * scale as f64).floor() as i32;
        self.vertical_pixel_offset = if scale > 1 {
            (self.camera_y * scale as f64 * 2.0).floor() as i32 & 1
        } else {
            0
        };
        let view_char_x = physical_view_x.div_euclid(scale);
        let view_char_y = physical_view_y.div_euclid(scale);
        let screen_center_x = physical_size.0 as i32 / 2 / scale;
        let screen_center_y = if scale > 1 {
            if boss_view {
                (physical_size.1 as i32 - 4) * 3 / 4 / scale
            } else {
                ((physical_size.1 as i32 - 4) * 3 / 4 + scale - 1) / scale
            }
        } else {
            self.height as i32 * 2 / 3
        };
        let ui_y = self.height as i32 - 4;
        self.world_view = Some((
            physical_view_x,
            physical_view_y,
            screen_center_x * scale,
            screen_center_y * scale,
            scale,
        ));

        let margin = 6;
        let view_left_bx = (view_char_x - screen_center_x).div_euclid(2);
        let view_right_bx = view_left_bx + (self.width as i32 / 2) + 1;
        let view_top_by = view_char_y - screen_center_y;
        let view_bottom_by = view_top_by + self.height as i32 + 1;
        let min_bx = view_left_bx - margin;
        let max_bx = view_right_bx + margin;
        let min_by = view_top_by - 16;
        let max_by = view_bottom_by + margin;
        let width_b = (max_bx - min_bx).max(1) as usize;
        let height_b = (max_by - min_by).max(1) as usize;
        let light_len = width_b * height_b;
        let mut light = std::mem::take(&mut self.light_buffer);
        let mut light_scratch = std::mem::take(&mut self.light_scratch);
        let mut precipitation_by_bx = std::mem::take(&mut self.precipitation_buffer);
        light.resize(light_len, 0);
        light.fill(0);
        light_scratch.resize(light_len, 0);
        light_scratch.fill(0);
        precipitation_by_bx.resize(width_b, PrecipitationType::None);
        precipitation_by_bx.fill(PrecipitationType::None);
        let tod = state.time_of_day;
        let (rain_mix, wind_mix, thunder_mix) = state.weather_audio_mix();
        let is_nether = state.current_dimension == Dimension::Nether;
        let is_end = state.current_dimension == Dimension::End;
        let is_overworld = state.current_dimension == Dimension::Overworld;
        // Keep distant End stars quiet; falling weather uses age + frame_alpha.
        let end_twinkle_phase = (state.player.age / 8) as i32;
        let mut sky_light = if is_nether {
            8.0
        } else if is_end {
            10.0
        } else {
            15.0
        };
        if is_overworld {
            if !(4000.0..=20000.0).contains(&tod) {
                sky_light = 12.0;
            } else if (4000.0..8000.0).contains(&tod) {
                sky_light = 12.0 + 3.0 * ((tod - 4000.0) / 4000.0);
            } else if (16000.0..=20000.0).contains(&tod) {
                sky_light = 15.0 - 3.0 * ((tod - 16000.0) / 4000.0);
            }
            let weather_dim = (rain_mix * 2.2) + (thunder_mix * 2.6);
            sky_light = (sky_light - weather_dim).max(4.0);
            if state.thunder_flash_timer > 0 {
                sky_light = (sky_light + 5.0).min(15.0);
            }
        }
        let sky_light_u8 = sky_light.round() as u8;
        let sky_day_mix = if is_overworld {
            Self::day_mix(tod)
        } else {
            0.0
        };
        let sky_twilight_mix = if is_overworld {
            Self::twilight_mix(tod)
        } else {
            0.0
        };
        if is_overworld {
            for (x, entry) in precipitation_by_bx.iter_mut().enumerate() {
                *entry = state.precipitation_at(min_bx + x as i32);
            }
        }
        let mut sky_occluder_by_bx = vec![max_by + 1; width_b];
        let mut precipitation_cutoff_by_bx = vec![CHUNK_HEIGHT as i32; width_b];
        let mut sky_horizon_raw_by_bx = vec![max_by + 1; width_b];
        if is_overworld {
            for x in 0..width_b {
                let bx = min_bx + x as i32;
                let occluder_entry = &mut sky_occluder_by_bx[x];
                *occluder_entry = (0..CHUNK_HEIGHT as i32)
                    .find(|&y| Self::sky_backdrop_occluder(state.world.get_block(bx, y)))
                    .unwrap_or(CHUNK_HEIGHT as i32);
                if precipitation_by_bx[x] != PrecipitationType::None {
                    precipitation_cutoff_by_bx[x] = Self::precipitation_cutoff(state, bx);
                }
                let horizon_entry = &mut sky_horizon_raw_by_bx[x];
                for by in (min_by - 12)..=max_by {
                    let block = state.world.get_block(bx, by);
                    if *horizon_entry == max_by + 1 && Self::sky_horizon_anchor(block) {
                        *horizon_entry = by;
                    }
                    if *occluder_entry != max_by + 1 && *horizon_entry != max_by + 1 {
                        break;
                    }
                }
                if *horizon_entry == max_by + 1 {
                    *horizon_entry = *occluder_entry;
                }
            }
        }
        let sky_horizon_by_bx = if is_overworld {
            Self::smooth_horizon_profile(&sky_horizon_raw_by_bx)
        } else {
            sky_horizon_raw_by_bx
        };

        for x in 0..width_b {
            let bx = min_bx + x as i32;
            for y in 0..height_b {
                let by = min_by + y as i32;
                if by < 0 {
                    light[y * width_b + x] = sky_light_u8;
                }
            }

            let mut current_light = sky_light_u8;
            for by in 0..CHUNK_HEIGHT as i32 {
                let block = state.world.get_block(bx, by);
                let (cell_light, next_light) = Self::skylight_column_step(current_light, block);

                if (min_by..max_by).contains(&by) {
                    let y = (by - min_by) as usize;
                    light[y * width_b + x] = cell_light;
                }

                current_light = next_light;
            }
        }

        for y in 0..height_b {
            for x in 0..width_b {
                match state.world.get_block(min_bx + x as i32, min_by + y as i32) {
                    BlockType::Torch => light[y * width_b + x] = 15,
                    BlockType::RedstoneTorch(true) => light[y * width_b + x] = 11,
                    BlockType::PrimedTnt(_) => light[y * width_b + x] = 13,
                    BlockType::Lava(_) => light[y * width_b + x] = 15,
                    BlockType::Glowstone => light[y * width_b + x] = 15,
                    BlockType::NetherPortal => light[y * width_b + x] = 11,
                    BlockType::EndPortal => light[y * width_b + x] = 10,
                    _ => {}
                }
            }
        }
        for fireball in &state.fireballs {
            let lx = (fireball.x.floor() as i32 - min_bx).clamp(0, width_b as i32 - 1) as usize;
            let ly = (fireball.y.floor() as i32 - min_by).clamp(0, height_b as i32 - 1) as usize;
            light[ly * width_b + lx] = 14;
        }

        // Cache material attenuation once; light propagation never queries the world.
        for y in 0..height_b {
            for x in 0..width_b {
                light_scratch[y * width_b + x] = if state
                    .world
                    .get_block(min_bx + x as i32, min_by + y as i32)
                    .is_solid()
                {
                    3
                } else {
                    1
                };
            }
        }
        super::lighting::propagate(
            &mut light,
            &light_scratch,
            width_b,
            &mut self.light_frontier,
        );

        for screen_y in 0..ui_y {
            let world_char_y = screen_y + view_char_y - screen_center_y;
            let block_y = world_char_y;
            for screen_x in 0..(self.width as i32) {
                let world_char_x = screen_x + view_char_x - screen_center_x;
                let block_x = world_char_x.div_euclid(2);
                let block = state.world.get_block(block_x, block_y);
                let light_x = (block_x - min_bx).clamp(0, width_b as i32 - 1) as usize;
                let light_y = (block_y - min_by).clamp(0, height_b as i32 - 1) as usize;
                let light_val = light[light_y * width_b + light_x];
                let l_factor = (light_val as f32 / 15.0).clamp(0.6, 1.0);
                let block_above = state.world.get_block(block_x, block_y - 1);
                let (c, fg, bg) =
                    Self::block_render_style(block, block_above, state.current_dimension, l_factor);
                let mut final_char = c;
                let mut final_fg = fg;
                let mut final_bg = bg;
                let precipitation = precipitation_by_bx[light_x];
                let sky_occluder_y = sky_occluder_by_bx[light_x];
                let precipitation_cutoff_y = precipitation_cutoff_by_bx[light_x];
                let sky_horizon_y = sky_horizon_by_bx[light_x];
                let sky_exposed = is_overworld && block_y < sky_occluder_y;
                let sky_bg = if sky_exposed {
                    Some(Self::overworld_sky_background(
                        block_y,
                        sky_horizon_y,
                        tod,
                        precipitation,
                        rain_mix,
                        thunder_mix,
                        state.thunder_flash_timer > 0,
                    ))
                } else {
                    None
                };
                if block == BlockType::Air {
                    if let Some(sky_bg) = sky_bg {
                        final_bg = sky_bg;
                    }
                    if !sky_exposed
                        && is_overworld
                        && final_bg == Color::Reset
                        && let Some(cave_bg) = Self::overworld_cave_air_background(light_val)
                    {
                        final_bg = cave_bg;
                    }
                    if !sky_exposed && light_val == 0 && block_y > 32 {
                        final_bg = Color::AnsiValue(233);
                    } else if sky_exposed {
                        if self.pixel_colors {
                            // The wide view shares the same designed sky; switching
                            // views must not bring back the old noise/cloud glyphs.
                            let size = (self.width as i32, ui_y * 2);
                            let mixes = (sky_day_mix, sky_twilight_mix);
                            final_char = '▀';
                            final_fg = Self::rgb(Self::screen_sky_pixel(
                                state,
                                (screen_x, screen_y * 2),
                                size,
                                mixes,
                            ));
                            final_bg = Self::rgb(Self::screen_sky_pixel(
                                state,
                                (screen_x, screen_y * 2 + 1),
                                size,
                                mixes,
                            ));
                        }

                        if self.pixel_colors {
                            for half in 0..2 {
                                let world_y = block_y as f64 + f64::from(half) * 0.5;
                                if world_y >= precipitation_cutoff_y as f64 {
                                    continue;
                                }
                                if let Some((color, opacity)) = Self::precipitation_pixel(
                                    precipitation,
                                    (world_char_x as f64 / 2.0, world_y),
                                    state.player.age as f64 + frame_alpha,
                                    state.weather_rain_intensity,
                                    0.5,
                                ) {
                                    let pixel = if half == 0 {
                                        &mut final_fg
                                    } else {
                                        &mut final_bg
                                    };
                                    *pixel = Self::rgb(Self::lerp_rgb(
                                        Self::color_rgb(*pixel).unwrap_or((0, 0, 0)),
                                        color,
                                        opacity,
                                    ));
                                }
                            }
                        } else if block_y < precipitation_cutoff_y
                            && Self::precipitation_pixel(
                                precipitation,
                                (world_char_x as f64 / 2.0, block_y as f64),
                                state.player.age as f64 + frame_alpha,
                                state.weather_rain_intensity,
                                1.0,
                            )
                            .is_some()
                        {
                            final_char = if precipitation == PrecipitationType::Snow {
                                '*'
                            } else {
                                '|'
                            };
                            final_fg = if precipitation == PrecipitationType::Snow {
                                Color::White
                            } else {
                                Color::Rgb {
                                    r: 142,
                                    g: 184,
                                    b: 216,
                                }
                            };
                        }
                    } else if is_end && final_char == ' ' {
                        let depth =
                            (0.38 + (screen_y as f32 / ui_y.max(1) as f32) * 0.12).clamp(0.0, 1.0);
                        final_bg = Color::Rgb {
                            r: (12.0 + depth * 8.0) as u8,
                            g: (8.0 + depth * 6.0) as u8,
                            b: (24.0 + depth * 14.0) as u8,
                        };

                        let star_hash = (world_char_x.wrapping_mul(31)
                            ^ block_y.wrapping_mul(17)
                            ^ (block_y / 3).wrapping_mul(13))
                        .rem_euclid(251);
                        if star_hash <= 1 {
                            let twinkle_phase = (end_twinkle_phase
                                + world_char_x.wrapping_mul(3)
                                + block_y.wrapping_mul(5))
                            .rem_euclid(12);
                            final_char = if twinkle_phase == 0 { '*' } else { '.' };
                            final_fg = if twinkle_phase <= 1 {
                                Color::Rgb {
                                    r: 250,
                                    g: 245,
                                    b: 255,
                                }
                            } else {
                                Color::Rgb {
                                    r: 220,
                                    g: 210,
                                    b: 255,
                                }
                            }
                        }
                    } else if is_nether && final_char == ' ' {
                        final_bg = Self::nether_air_background(block_y, light_val);
                    }
                }
                if let Some(sky_bg) = sky_bg
                    && final_bg == Color::Reset
                    && !Self::sky_backdrop_occluder(block)
                {
                    final_bg = sky_bg;
                }
                if is_overworld && Self::should_apply_overworld_depth_fog(block_y, sky_horizon_y) {
                    let depth = ((block_y as f32 - 34.0) / 44.0).clamp(0.0, 1.0);
                    let player_depth = ((render_player_y as f32 - 34.0) / 44.0).clamp(0.0, 1.0);
                    let depth_strength = depth.max(player_depth * 0.72);
                    let dx = (block_x as f32 + 0.5) - render_player_x as f32;
                    let dy = block_y as f32 - render_player_y as f32;
                    let dist = (dx * dx + dy * dy).sqrt();
                    let near_clear = 9.0;
                    let far_fog = 23.0;
                    let dist_fade = ((dist - near_clear) / (far_fog - near_clear)).clamp(0.0, 1.0);
                    let cave_visibility =
                        (1.0 - dist_fade * (0.24 + depth_strength * 0.76)).clamp(0.2, 1.0);
                    if cave_visibility < 0.999 {
                        final_fg = Self::dim_color(final_fg, cave_visibility);
                        if block == BlockType::Air {
                            if final_bg == Color::Reset {
                                let shade = (5.0 + cave_visibility * 15.0) as u8;
                                final_bg = Color::Rgb {
                                    r: shade,
                                    g: shade,
                                    b: (shade + 2).min(26),
                                };
                            } else {
                                final_bg =
                                    Self::dim_color(final_bg, (cave_visibility * 0.95).max(0.3));
                            }
                            if cave_visibility < 0.28 {
                                final_char = ' ';
                            }
                        } else {
                            final_bg =
                                Self::dim_color(final_bg, (cave_visibility * 0.92).max(0.35));
                        }
                    }
                }
                if state.player.mining_timer > 0.0
                    && state.player.last_mine_x == block_x
                    && state.player.last_mine_y == block_y
                {
                    if state.player.mining_timer > 0.6 {
                        final_char = '░';
                    } else if state.player.mining_timer > 0.3 {
                        final_char = '▒';
                    }
                }
                self.put_world_char(screen_x, screen_y, final_char, final_fg, final_bg);
            }
        }

        if scale == 1 && self.pixel_colors {
            self.render_wide_torches(state, &light, (min_bx, min_by, width_b, height_b));
        }

        for bolt in &state.lightning_bolts {
            let base_sx = (bolt.x * 2) - view_char_x + screen_center_x;
            for y in bolt.y_top..=bolt.y_bottom {
                let sy = y - view_char_y + screen_center_y;
                if sy < 0 || sy >= ui_y {
                    continue;
                }
                let phase = ((y + bolt.x + state.player.age as i32) & 1) == 0;
                let sx = if phase { base_sx } else { base_sx + 1 };
                if sx >= 0 && sx < self.width as i32 {
                    let color = if bolt.ttl > 2 {
                        Color::White
                    } else {
                        Color::Rgb {
                            r: 255,
                            g: 220,
                            b: 120,
                        }
                    };
                    self.put_char(sx, sy, '|', color, Color::Reset);
                }
            }
            let impact_sy = bolt.y_bottom - view_char_y + screen_center_y;
            if impact_sy >= 0 && impact_sy < ui_y {
                self.put_char(base_sx, impact_sy, '*', Color::Yellow, Color::Reset);
            }
        }

        if state.current_dimension == Dimension::End {
            if let Some(dragon) = &state.ender_dragon {
                let dragon_sx = (dragon.x * 2.0).round() as i32 - view_char_x + screen_center_x - 2;
                let dragon_sy = dragon.y.floor() as i32 - view_char_y + screen_center_y - 1;
                for crystal in &state.end_crystals {
                    let c_sx = (crystal.x * 2.0).round() as i32 - view_char_x + screen_center_x;
                    let c_sy = crystal.y.floor() as i32 - view_char_y + screen_center_y;
                    let steps = ((dragon_sx - c_sx).abs().max((dragon_sy - c_sy).abs())).min(26);
                    if steps > 0 {
                        for step in 0..=steps {
                            let t = step as f64 / steps as f64;
                            let bx = c_sx + ((dragon_sx - c_sx) as f64 * t) as i32;
                            let by = c_sy + ((dragon_sy - c_sy) as f64 * t) as i32;
                            if bx >= 0 && bx < self.width as i32 && by >= 0 && by < ui_y {
                                self.put_char(
                                    bx,
                                    by,
                                    '.',
                                    Color::Rgb {
                                        r: 245,
                                        g: 130,
                                        b: 255,
                                    },
                                    Color::Reset,
                                );
                            }
                        }
                    }
                }
            }

            for (actor_index, crystal) in state.end_crystals.iter().enumerate() {
                let (actor_x, actor_y) = self.motion.position(
                    "end_crystals",
                    actor_index,
                    (crystal.x, crystal.y),
                    crystal.age,
                    frame_alpha,
                );
                let c_sx = (actor_x * 2.0).round() as i32 - view_char_x + screen_center_x;
                let c_sy = actor_y.floor() as i32 - view_char_y + screen_center_y;
                if c_sx >= -4
                    && c_sx < self.width as i32 + 4
                    && c_sy >= -4
                    && c_sy < self.height as i32 + 4
                {
                    let pulse = if (crystal.age / 3).is_multiple_of(2) {
                        Color::Rgb {
                            r: 255,
                            g: 180,
                            b: 255,
                        }
                    } else {
                        Color::Rgb {
                            r: 215,
                            g: 110,
                            b: 245,
                        }
                    };
                    let fg = if crystal.hit_timer > 0 {
                        Color::Rgb {
                            r: 255,
                            g: 80,
                            b: 80,
                        }
                    } else {
                        pulse
                    };
                    if scale > 1 {
                        self.actor_sprites.push(
                            ActorSprite::mob(
                                c_sx,
                                c_sy,
                                SpriteKind::Crystal,
                                true,
                                0.0,
                                crystal.age,
                                fg,
                            )
                            .with_subcell(actor_x, actor_y, scale)
                            .with_hit(crystal.hit_timer > 0),
                        );
                    } else {
                        self.put_char(c_sx, c_sy, '*', fg, Color::Reset);
                    }
                }
            }

            if let Some(dragon) = &state.ender_dragon {
                let (dragon_x, dragon_y) = self.motion.position(
                    "dragon",
                    0,
                    (dragon.x, dragon.y),
                    dragon.age,
                    frame_alpha,
                );
                let d_sx = (dragon_x * 2.0).round() as i32 - view_char_x + screen_center_x - 2;
                let d_sy = dragon_y.floor() as i32 - view_char_y + screen_center_y - 1;
                if d_sx >= -14
                    && d_sx < self.width as i32 + 14
                    && d_sy >= -6
                    && d_sy < self.height as i32 + 6
                {
                    let mut d_fg = Color::Rgb {
                        r: 205,
                        g: 90,
                        b: 240,
                    };
                    if dragon.hit_timer > 0 {
                        d_fg = Color::Rgb {
                            r: 255,
                            g: 75,
                            b: 75,
                        };
                    }
                    if scale > 1 {
                        self.actor_sprites.push(
                            ActorSprite::mob(
                                d_sx,
                                d_sy,
                                SpriteKind::Dragon,
                                dragon.facing_right || self.dragon_art_preview.unwrap_or(true),
                                dragon.vx,
                                dragon.age,
                                d_fg,
                            )
                            .with_subcell(dragon_x, dragon_y, scale)
                            .with_hit(dragon.hit_timer > 0)
                            .with_alert(matches!(
                                dragon.phase,
                                crate::entities::ender_dragon::DragonPhase::Swoop
                            ))
                            .with_variant(self.dragon_art_preview.unwrap_or(true))
                            .with_animation_time(dragon.age as f64 + frame_alpha),
                        );
                    } else {
                        let wings = if (dragon.age / 4).is_multiple_of(2) {
                            "<^^>"
                        } else {
                            "/^^\\"
                        };
                        let body = if dragon.facing_right { "D>>>" } else { "<<<D" };
                        self.put_entity_str(d_sx, d_sy - 1, wings, d_fg);
                        self.put_entity_str(d_sx, d_sy, body, d_fg);
                    }
                }
            }
        }

        for (actor_index, remote) in state.remote_players.iter().enumerate() {
            let (actor_x, actor_y) = self.motion.position(
                "remote_players",
                actor_index,
                (remote.x, remote.y),
                u64::from(remote.client_id),
                frame_alpha,
            );
            let render_remote_x = actor_x;
            let render_remote_y = actor_y;
            let remote_screen_x =
                (render_remote_x * 2.0).round() as i32 - view_char_x + screen_center_x - 1;
            let remote_screen_y =
                render_remote_y.floor() as i32 - view_char_y + screen_center_y - 1;
            if remote_screen_x < -2
                || remote_screen_x >= self.width as i32
                || remote_screen_y < -2
                || remote_screen_y >= ui_y
            {
                continue;
            }

            let remote_bx = render_remote_x.round() as i32;
            let remote_by = (render_remote_y - 1.0).floor() as i32;
            let light_x = (remote_bx - min_bx).clamp(0, width_b as i32 - 1) as usize;
            let light_y = (remote_by - min_by).clamp(0, height_b as i32 - 1) as usize;
            let light_val = light[light_y * width_b + light_x];
            let light_factor = (light_val as f32 / 15.0).clamp(0.38, 1.0);
            let (head_fg, torso_fg, limb_fg) = Self::remote_player_visible_colors(
                remote.client_id,
                if scale > 1 { 1.0 } else { light_factor },
            );
            self.actor_sprites.push(
                ActorSprite::humanoid(
                    remote_screen_x,
                    remote_screen_y,
                    SpriteKind::Player,
                    remote.facing_right,
                    remote.vx,
                    state.player.age,
                    [head_fg, torso_fg, limb_fg],
                    remote.sneaking,
                    false,
                )
                .with_subcell(render_remote_x, render_remote_y, scale)
                .with_label(remote.client_id),
            );
        }

        let player_bx = render_player_x.round() as i32;
        let player_by = (render_player_y - 1.0).floor() as i32;
        let p_light_x = (player_bx - min_bx).clamp(0, width_b as i32 - 1) as usize;
        let p_light_y = (player_by - min_by).clamp(0, height_b as i32 - 1) as usize;
        let p_light_val = light[p_light_y * width_b + p_light_x];
        let p_factor = (p_light_val as f32 / 15.0).clamp(0.4, 1.0);
        let (player_head_fg, player_torso_fg, player_limb_fg) =
            Self::player_visible_colors(state, if scale > 1 { 1.0 } else { p_factor });

        let player_screen_x =
            (render_player_x * 2.0).round() as i32 - view_char_x + screen_center_x - 1;
        let player_screen_y = render_player_y.floor() as i32 - view_char_y + screen_center_y - 1;
        self.actor_sprites.push(
            ActorSprite::humanoid(
                player_screen_x,
                player_screen_y,
                SpriteKind::Player,
                state.player.facing_right,
                state.player.vx,
                state.player.age,
                [
                    if scale > 1 {
                        Self::rgb(Self::player_skin_rgb())
                    } else {
                        player_head_fg
                    },
                    player_torso_fg,
                    player_limb_fg,
                ],
                state.player.sneaking,
                state.player.mining_timer > 0.0 || state.player.attack_timer > 5,
            )
            .with_subcell(render_player_x, render_player_y, scale)
            .with_item(
                state.inventory.slots[state.hotbar_index as usize]
                    .as_ref()
                    .map(|s| s.item_type),
            )
            .with_pose(!state.player.grounded)
            .with_motion(state.player.vy)
            .with_hit(state.player_hurt_ticks() > 5)
            .with_swing(if state.player.attack_timer > 0 {
                Some(
                    0.3 + 0.7 * (10.0 - state.player.attack_timer.min(10) as f64 + frame_alpha)
                        / 10.0,
                )
            } else if state.player.mining_timer > 0.0 {
                Some((state.player.age as f64 + frame_alpha).rem_euclid(8.0) / 8.0)
            } else {
                None
            })
            .with_helmet(
                state
                    .armor_slot_item(0)
                    .and_then(|s| Self::armor_tint_rgb(s.item_type))
                    .map(Self::rgb),
            ),
        );

        for (actor_index, zombie) in state.zombies.iter().enumerate() {
            let (actor_x, actor_y) = self.motion.position(
                "zombies",
                actor_index,
                (zombie.x, zombie.y),
                zombie.age,
                frame_alpha,
            );
            let z_sx = (actor_x * 2.0).round() as i32 - view_char_x + screen_center_x - 1;
            let z_sy = actor_y.floor() as i32 - view_char_y + screen_center_y - 1;
            if z_sx >= -4
                && z_sx < self.width as i32 + 4
                && z_sy >= -4
                && z_sy < self.height as i32 + 4
            {
                let z_f = (light[((actor_y - 1.0).floor() as i32 - min_by)
                    .clamp(0, height_b as i32 - 1) as usize
                    * width_b
                    + (actor_x.round() as i32 - min_bx).clamp(0, width_b as i32 - 1) as usize]
                    as f32
                    / 15.0)
                    .clamp(0.4, 1.0);
                let mut z_fg = Color::Rgb {
                    r: (34.0 * z_f) as u8,
                    g: (139.0 * z_f) as u8,
                    b: (34.0 * z_f) as u8,
                };
                if zombie.hit_timer > 0 {
                    z_fg = Color::Rgb {
                        r: 255,
                        g: 50,
                        b: 50,
                    };
                } else if zombie.burning_timer > 0 {
                    let pulse = (state.time_of_day * 0.1).sin().abs();
                    z_fg = Color::Rgb {
                        r: (255.0 * pulse) as u8,
                        g: (139.0 * z_f * (1.0 - pulse)) as u8,
                        b: (34.0 * z_f * (1.0 - pulse)) as u8,
                    };
                }
                self.actor_sprites.push(
                    ActorSprite::mob(
                        z_sx,
                        z_sy,
                        SpriteKind::Zombie,
                        zombie.facing_right,
                        zombie.vx,
                        zombie.age,
                        z_fg,
                    )
                    .with_subcell(actor_x, actor_y, scale)
                    .with_hit(zombie.hit_timer > 0)
                    .with_warning(zombie.attack_windup),
                );
            }
        }

        for (actor_index, pigman) in state.pigmen.iter().enumerate() {
            let (actor_x, actor_y) = self.motion.position(
                "pigmen",
                actor_index,
                (pigman.x, pigman.y),
                pigman.age,
                frame_alpha,
            );
            let p_sx = (actor_x * 2.0).round() as i32 - view_char_x + screen_center_x - 1;
            let p_sy = actor_y.floor() as i32 - view_char_y + screen_center_y - 1;
            if p_sx >= -4
                && p_sx < self.width as i32 + 4
                && p_sy >= -4
                && p_sy < self.height as i32 + 4
            {
                let p_f = (light[((actor_y - 1.0).floor() as i32 - min_by)
                    .clamp(0, height_b as i32 - 1) as usize
                    * width_b
                    + (actor_x.round() as i32 - min_bx).clamp(0, width_b as i32 - 1) as usize]
                    as f32
                    / 15.0)
                    .clamp(0.4, 1.0);
                let mut p_fg = if pigman.is_aggressive() {
                    Color::Rgb {
                        r: (255.0 * p_f) as u8,
                        g: (190.0 * p_f) as u8,
                        b: (90.0 * p_f) as u8,
                    }
                } else {
                    Color::Rgb {
                        r: (220.0 * p_f) as u8,
                        g: (160.0 * p_f) as u8,
                        b: (150.0 * p_f) as u8,
                    }
                };
                if pigman.hit_timer > 0 {
                    p_fg = Color::Rgb {
                        r: 255,
                        g: 50,
                        b: 50,
                    };
                }
                self.actor_sprites.push(
                    ActorSprite::mob(
                        p_sx,
                        p_sy,
                        SpriteKind::Pigman,
                        pigman.facing_right,
                        pigman.vx,
                        pigman.age,
                        p_fg,
                    )
                    .with_subcell(actor_x, actor_y, scale)
                    .with_hit(pigman.hit_timer > 0)
                    .with_alert(pigman.is_aggressive())
                    .with_warning(pigman.attack_windup),
                );
            }
        }

        for (actor_index, ghast) in state.ghasts.iter().enumerate() {
            let (actor_x, actor_y) = self.motion.position(
                "ghasts",
                actor_index,
                (ghast.x, ghast.y),
                ghast.age,
                frame_alpha,
            );
            let g_sx = (actor_x * 2.0).round() as i32 - view_char_x + screen_center_x - 2;
            let g_sy = actor_y.floor() as i32 - view_char_y + screen_center_y - 1;
            if g_sx >= -4
                && g_sx < self.width as i32 + 4
                && g_sy >= -4
                && g_sy < self.height as i32 + 4
            {
                let g_f = (light[((actor_y - 1.0).floor() as i32 - min_by)
                    .clamp(0, height_b as i32 - 1) as usize
                    * width_b
                    + (actor_x.round() as i32 - min_bx).clamp(0, width_b as i32 - 1) as usize]
                    as f32
                    / 15.0)
                    .clamp(0.4, 1.0);
                let mut g_fg = Color::Rgb {
                    r: (240.0 * g_f) as u8,
                    g: (240.0 * g_f) as u8,
                    b: (240.0 * g_f) as u8,
                };
                if ghast.hit_timer > 0 {
                    g_fg = Color::Rgb {
                        r: 255,
                        g: 70,
                        b: 70,
                    };
                } else if ghast.shoot_cooldown < 16 {
                    g_fg = Color::Rgb {
                        r: 255,
                        g: (220.0 * g_f) as u8,
                        b: (220.0 * g_f) as u8,
                    };
                }
                if scale > 1 {
                    self.actor_sprites.push(
                        ActorSprite::mob(
                            g_sx,
                            g_sy,
                            SpriteKind::Ghast,
                            ghast.facing_right,
                            ghast.vx,
                            ghast.age,
                            g_fg,
                        )
                        .with_subcell(actor_x, actor_y, scale)
                        .with_hit(ghast.hit_timer > 0)
                        .with_animation_time(ghast.age as f64 + frame_alpha)
                        .with_alert(ghast.shoot_cooldown < 16),
                    );
                } else {
                    let sprite = Self::ghast_sprite(ghast.facing_right);
                    self.put_entity_str(g_sx, g_sy - 2, sprite[0], g_fg);
                    self.put_entity_str(g_sx, g_sy - 1, sprite[1], g_fg);
                    self.put_entity_str(g_sx, g_sy, sprite[2], g_fg);
                }
            }
        }

        for (actor_index, blaze) in state.blazes.iter().enumerate() {
            let (actor_x, actor_y) = self.motion.position(
                "blazes",
                actor_index,
                (blaze.x, blaze.y),
                blaze.age,
                frame_alpha,
            );
            let b_sx = (actor_x * 2.0).round() as i32 - view_char_x + screen_center_x - 1;
            let b_sy = actor_y.floor() as i32 - view_char_y + screen_center_y - 1;
            if b_sx >= -4
                && b_sx < self.width as i32 + 4
                && b_sy >= -4
                && b_sy < self.height as i32 + 4
            {
                let b_f = (light[((actor_y - 1.0).floor() as i32 - min_by)
                    .clamp(0, height_b as i32 - 1) as usize
                    * width_b
                    + (actor_x.round() as i32 - min_bx).clamp(0, width_b as i32 - 1) as usize]
                    as f32
                    / 15.0)
                    .clamp(0.4, 1.0);
                let mut b_fg = Color::Rgb {
                    r: (245.0 * b_f) as u8,
                    g: (188.0 * b_f) as u8,
                    b: (85.0 * b_f) as u8,
                };
                if blaze.hit_timer > 0 {
                    b_fg = Color::Rgb {
                        r: 255,
                        g: 70,
                        b: 70,
                    };
                } else if blaze.shoot_cooldown < 10 {
                    b_fg = Color::Yellow;
                }
                if scale > 1 {
                    self.actor_sprites.push(
                        ActorSprite::mob(
                            b_sx,
                            b_sy,
                            SpriteKind::Blaze,
                            blaze.facing_right,
                            blaze.vx,
                            blaze.age,
                            b_fg,
                        )
                        .with_subcell(actor_x, actor_y, scale)
                        .with_hit(blaze.hit_timer > 0)
                        .with_animation_time(blaze.age as f64 + frame_alpha)
                        .with_warning(blaze.attack_windup)
                        .with_alert(blaze.shoot_cooldown < 10),
                    );
                } else {
                    let sprite = Self::blaze_sprite(blaze.age);
                    self.put_entity_str(b_sx, b_sy - 1, sprite[0], b_fg);
                    self.put_entity_str(b_sx, b_sy, sprite[1], b_fg);
                }
            }
        }

        for (actor_index, enderman) in state.endermen.iter().enumerate() {
            let (actor_x, actor_y) = self.motion.position(
                "endermen",
                actor_index,
                (enderman.x, enderman.y),
                enderman.age,
                frame_alpha,
            );
            let e_sx = (actor_x * 2.0).round() as i32 - view_char_x + screen_center_x - 1;
            let e_sy = actor_y.floor() as i32 - view_char_y + screen_center_y - 1;
            if e_sx >= -4
                && e_sx < self.width as i32 + 4
                && e_sy >= -4
                && e_sy < self.height as i32 + 4
            {
                let e_f = (light[((actor_y - 1.0).floor() as i32 - min_by)
                    .clamp(0, height_b as i32 - 1) as usize
                    * width_b
                    + (actor_x.round() as i32 - min_bx).clamp(0, width_b as i32 - 1) as usize]
                    as f32
                    / 15.0)
                    .clamp(0.3, 1.0);
                let mut e_fg = if enderman.aggressive_timer > 0 {
                    Color::Rgb {
                        r: (230.0 * e_f) as u8,
                        g: (75.0 * e_f) as u8,
                        b: (235.0 * e_f) as u8,
                    }
                } else {
                    Color::Rgb {
                        r: (170.0 * e_f) as u8,
                        g: (120.0 * e_f) as u8,
                        b: (220.0 * e_f) as u8,
                    }
                };
                if enderman.hit_timer > 0 {
                    e_fg = Color::Rgb {
                        r: 255,
                        g: 70,
                        b: 70,
                    };
                }
                self.actor_sprites.push(
                    ActorSprite::mob(
                        e_sx,
                        e_sy,
                        SpriteKind::Enderman,
                        enderman.facing_right,
                        enderman.vx,
                        enderman.age,
                        e_fg,
                    )
                    .with_subcell(actor_x, actor_y, scale)
                    .with_hit(enderman.hit_timer > 0)
                    .with_warning(enderman.attack_windup),
                );
            }
        }

        for (actor_index, creeper) in state.creepers.iter().enumerate() {
            let (actor_x, actor_y) = self.motion.position(
                "creepers",
                actor_index,
                (creeper.x, creeper.y),
                creeper.age,
                frame_alpha,
            );
            let sx = (actor_x * 2.0).round() as i32 - view_char_x + screen_center_x - 1;
            let sy = actor_y.floor() as i32 - view_char_y + screen_center_y - 1;
            if sx >= -4 && sx < self.width as i32 + 4 && sy >= -4 && sy < self.height as i32 + 4 {
                let f = (light[((actor_y - 1.0).floor() as i32 - min_by)
                    .clamp(0, height_b as i32 - 1) as usize
                    * width_b
                    + (actor_x.round() as i32 - min_bx).clamp(0, width_b as i32 - 1) as usize]
                    as f32
                    / 15.0)
                    .clamp(0.4, 1.0);
                let mut fg = if creeper.hit_timer > 0 {
                    Color::Rgb {
                        r: 255,
                        g: 50,
                        b: 50,
                    }
                } else {
                    Color::Rgb {
                        r: (50.0 * f) as u8,
                        g: (205.0 * f) as u8,
                        b: (50.0 * f) as u8,
                    }
                };
                if creeper.charged && creeper.hit_timer == 0 {
                    fg = Color::Rgb {
                        r: (120.0 * f) as u8,
                        g: (200.0 * f) as u8,
                        b: (255.0 * f) as u8,
                    };
                }
                if creeper.fuse_timer > 0 && (creeper.age / 2) % 2 == 0 {
                    fg = Color::White;
                }
                self.actor_sprites.push(
                    ActorSprite::mob(
                        sx,
                        sy,
                        SpriteKind::Creeper,
                        creeper.facing_right,
                        creeper.vx,
                        creeper.age,
                        fg,
                    )
                    .with_subcell(actor_x, actor_y, scale)
                    .with_hit(creeper.hit_timer > 0)
                    .with_alert(creeper.fuse_timer > 0)
                    .with_variant(creeper.charged),
                );
            }
        }

        for (actor_index, skeleton) in state.skeletons.iter().enumerate() {
            let (actor_x, actor_y) = self.motion.position(
                "skeletons",
                actor_index,
                (skeleton.x, skeleton.y),
                skeleton.age,
                frame_alpha,
            );
            let sx = (actor_x * 2.0).round() as i32 - view_char_x + screen_center_x - 1;
            let sy = actor_y.floor() as i32 - view_char_y + screen_center_y - 1;
            if sx >= -4 && sx < self.width as i32 + 4 && sy >= -4 && sy < self.height as i32 + 4 {
                let f = (light[((actor_y - 1.0).floor() as i32 - min_by)
                    .clamp(0, height_b as i32 - 1) as usize
                    * width_b
                    + (actor_x.round() as i32 - min_bx).clamp(0, width_b as i32 - 1) as usize]
                    as f32
                    / 15.0)
                    .clamp(0.4, 1.0);
                let mut fg = if skeleton.hit_timer > 0 {
                    Color::Rgb {
                        r: 255,
                        g: 50,
                        b: 50,
                    }
                } else {
                    Color::Rgb {
                        r: (200.0 * f) as u8,
                        g: (200.0 * f) as u8,
                        b: (200.0 * f) as u8,
                    }
                };
                if skeleton.burning_timer > 0 {
                    let pulse = (state.time_of_day * 0.1).sin().abs();
                    fg = Color::Rgb {
                        r: (255.0 * pulse) as u8,
                        g: (200.0 * f * (1.0 - pulse)) as u8,
                        b: (200.0 * f * (1.0 - pulse)) as u8,
                    };
                }
                self.actor_sprites.push(
                    ActorSprite::mob(
                        sx,
                        sy,
                        SpriteKind::Skeleton,
                        skeleton.facing_right,
                        skeleton.vx,
                        skeleton.age,
                        fg,
                    )
                    .with_subcell(actor_x, actor_y, scale)
                    .with_hit(skeleton.hit_timer > 0)
                    .with_warning(skeleton.bow_windup)
                    .with_item(Some(ItemType::Bow))
                    .with_swing((skeleton.bow_windup > 0).then_some(0.05)),
                );
            }
        }

        for (actor_index, spider) in state.spiders.iter().enumerate() {
            let (actor_x, actor_y) = self.motion.position(
                "spiders",
                actor_index,
                (spider.x, spider.y),
                spider.age,
                frame_alpha,
            );
            let sx = (actor_x * 2.0).round() as i32 - view_char_x + screen_center_x - 1;
            let sy = actor_y.floor() as i32 - view_char_y + screen_center_y - 1;
            if sx >= -4 && sx < self.width as i32 + 4 && sy >= -4 && sy < self.height as i32 + 4 {
                let f = (light[(sy + view_char_y - screen_center_y - min_by)
                    .clamp(0, height_b as i32 - 1) as usize
                    * width_b
                    + (sx / 2 + view_char_x / 2 - screen_center_x / 2 - min_bx)
                        .clamp(0, width_b as i32 - 1) as usize] as f32
                    / 15.0)
                    .clamp(0.4, 1.0);
                let fg = if spider.hit_timer > 0 {
                    Color::Rgb {
                        r: 255,
                        g: 50,
                        b: 50,
                    }
                } else {
                    Color::Rgb {
                        r: (100.0 * f) as u8,
                        g: (0.0 * f) as u8,
                        b: (0.0 * f) as u8,
                    }
                };
                if scale > 1 {
                    self.actor_sprites.push(
                        ActorSprite::mob(
                            sx,
                            sy,
                            SpriteKind::Spider,
                            spider.facing_right,
                            spider.vx,
                            spider.age,
                            fg,
                        )
                        .with_subcell(actor_x, actor_y, scale)
                        .with_hit(spider.hit_timer > 0)
                        .with_warning(spider.attack_windup),
                    );
                } else {
                    self.put_entity_str(sx - 1, sy, "/v\\\\", fg);
                }
            }
        }

        for (actor_index, silverfish) in state.silverfish.iter().enumerate() {
            let (actor_x, actor_y) = self.motion.position(
                "silverfish",
                actor_index,
                (silverfish.x, silverfish.y),
                silverfish.age,
                frame_alpha,
            );
            let sx = (actor_x * 2.0).round() as i32 - view_char_x + screen_center_x - 1;
            let sy = actor_y.floor() as i32 - view_char_y + screen_center_y;
            if sx >= -4 && sx < self.width as i32 + 4 && sy >= -4 && sy < self.height as i32 + 4 {
                let f = (light[((actor_y - 0.35).floor() as i32 - min_by)
                    .clamp(0, height_b as i32 - 1) as usize
                    * width_b
                    + (actor_x.round() as i32 - min_bx).clamp(0, width_b as i32 - 1) as usize]
                    as f32
                    / 15.0)
                    .clamp(0.4, 1.0);
                let fg = if silverfish.hit_timer > 0 {
                    Color::Rgb {
                        r: 255,
                        g: 70,
                        b: 70,
                    }
                } else {
                    Color::Rgb {
                        r: (170.0 * f) as u8,
                        g: (185.0 * f) as u8,
                        b: (200.0 * f) as u8,
                    }
                };
                if scale > 1 {
                    self.actor_sprites.push(
                        ActorSprite::mob(
                            sx,
                            sy,
                            SpriteKind::Silverfish,
                            silverfish.facing_right,
                            silverfish.vx,
                            silverfish.age,
                            fg,
                        )
                        .with_subcell(actor_x, actor_y, scale)
                        .with_hit(silverfish.hit_timer > 0)
                        .with_warning(silverfish.attack_windup),
                    );
                } else {
                    let body = if (silverfish.age / 3).is_multiple_of(2) {
                        "~~"
                    } else {
                        "=="
                    };
                    self.put_entity_str(sx, sy, body, fg);
                }
            }
        }

        for (actor_index, slime) in state.slimes.iter().enumerate() {
            let (actor_x, actor_y) = self.motion.position(
                "slimes",
                actor_index,
                (slime.x, slime.y),
                slime.age,
                frame_alpha,
            );
            let sx = (actor_x * 2.0).round() as i32 - view_char_x + screen_center_x - 1;
            let sy = actor_y.floor() as i32 - view_char_y + screen_center_y;
            if sx >= -3 && sx < self.width as i32 + 3 && sy >= -2 && sy < self.height as i32 + 2 {
                let f = (light[((actor_y - slime.height() * 0.5).floor() as i32 - min_by)
                    .clamp(0, height_b as i32 - 1) as usize
                    * width_b
                    + (actor_x.round() as i32 - min_bx).clamp(0, width_b as i32 - 1) as usize]
                    as f32
                    / 15.0)
                    .clamp(0.35, 1.0);
                let mut fg = if slime.hit_timer > 0 {
                    Color::Rgb {
                        r: 255,
                        g: 90,
                        b: 90,
                    }
                } else {
                    Color::Rgb {
                        r: (94.0 * f) as u8,
                        g: (205.0 * f) as u8,
                        b: (96.0 * f) as u8,
                    }
                };
                if slime.attack_cooldown > 0 && slime.contact_damage() > 0.0 {
                    fg = Color::Rgb {
                        r: (130.0 * f) as u8,
                        g: (230.0 * f) as u8,
                        b: (130.0 * f) as u8,
                    };
                }
                if scale > 1 {
                    self.actor_sprites.push(
                        ActorSprite::mob(
                            sx,
                            sy,
                            SpriteKind::Slime,
                            slime.facing_right,
                            slime.vx,
                            slime.age,
                            fg,
                        )
                        .with_subcell(actor_x, actor_y, scale)
                        .with_hit(slime.hit_timer > 0)
                        .with_warning(slime.attack_windup)
                        .with_dimensions(
                            (slime.half_width() * 4.0 * scale as f64).ceil() as usize,
                            (slime.height() * scale as f64).ceil() as usize,
                        ),
                    );
                } else {
                    match slime.size {
                        4 => {
                            self.put_entity_str(sx - 1, sy - 1, "ooo", fg);
                            self.put_entity_str(sx - 1, sy, "O_O", fg);
                        }
                        2 => {
                            self.put_entity_str(sx, sy - 1, "oo", fg);
                            self.put_entity_str(sx, sy, "o_", fg);
                        }
                        _ => {
                            let ch = if (slime.age / 4).is_multiple_of(2) {
                                'o'
                            } else {
                                '.'
                            };
                            self.put_entity_char(sx + 1, sy, ch, fg);
                        }
                    }
                }
            }
        }

        for (actor_index, fireball) in state.fireballs.iter().enumerate() {
            let (actor_x, actor_y) = self.motion.position(
                "fireballs",
                actor_index,
                (fireball.x, fireball.y),
                fireball.age,
                frame_alpha,
            );
            let sx = (actor_x * 2.0).round() as i32 - view_char_x + screen_center_x;
            let sy = actor_y.floor() as i32 - view_char_y + screen_center_y;
            if sx >= -4 && sx < self.width as i32 + 4 && sy >= -4 && sy < self.height as i32 + 4 {
                let pulse = if (fireball.age / 2).is_multiple_of(2) {
                    Color::Rgb {
                        r: 255,
                        g: 170,
                        b: 40,
                    }
                } else {
                    Color::Rgb {
                        r: 255,
                        g: 90,
                        b: 20,
                    }
                };
                if scale > 1 {
                    self.actor_sprites.push(
                        ActorSprite::mob(
                            sx,
                            sy,
                            SpriteKind::Fireball,
                            fireball.vx >= 0.0,
                            fireball.vx,
                            fireball.age,
                            pulse,
                        )
                        .with_subcell(actor_x, actor_y, scale)
                        .with_dimensions(scale as usize, (scale / 2).max(1) as usize),
                    );
                } else {
                    self.put_char(sx, sy, '@', pulse, Color::Reset);
                }
            }
        }

        for (actor_index, arrow) in state.arrows.iter().enumerate() {
            let (actor_x, actor_y) = self.motion.position(
                "arrows",
                actor_index,
                (arrow.x, arrow.y),
                arrow.age,
                frame_alpha,
            );
            let sx = (actor_x * 2.0).round() as i32 - view_char_x + screen_center_x;
            let sy = actor_y.floor() as i32 - view_char_y + screen_center_y;
            if sx >= -4 && sx < self.width as i32 + 4 && sy >= -4 && sy < self.height as i32 + 4 {
                if scale > 1 {
                    self.actor_sprites.push(
                        ActorSprite::mob(
                            sx,
                            sy,
                            SpriteKind::Arrow,
                            arrow.vx >= 0.0,
                            arrow.vx,
                            arrow.age,
                            Color::White,
                        )
                        .with_subcell(actor_x, actor_y, scale)
                        .with_dimensions((scale * 2) as usize, 1),
                    );
                } else {
                    self.put_char(sx, sy, '*', Color::White, Color::Reset);
                }
            }
        }

        if let Some((bobber_x, bobber_y, bite_ready)) = state.fishing_bobber() {
            let sx = (bobber_x * 2.0).round() as i32 - view_char_x + screen_center_x;
            let sy = bobber_y.floor() as i32 - view_char_y + screen_center_y;
            if sx >= -4 && sx < self.width as i32 + 4 && sy >= -4 && sy < self.height as i32 + 4 {
                let fg = if bite_ready {
                    Color::Yellow
                } else {
                    Color::Cyan
                };
                let ch = if bite_ready { '@' } else { 'o' };
                self.put_char(sx, sy, ch, fg, Color::Reset);
            }
        }

        for (actor_index, boat) in state.boats.iter().enumerate() {
            let (actor_x, actor_y) =
                self.motion
                    .position("boats", actor_index, (boat.x, boat.y), 0, frame_alpha);
            let b_sx = (actor_x * 2.0).round() as i32 - view_char_x + screen_center_x - 1;
            let b_sy = actor_y.floor() as i32 - view_char_y + screen_center_y;
            if b_sx >= -2 && b_sx < self.width as i32 && b_sy >= 0 && b_sy < self.height as i32 {
                let light_x =
                    (actor_x.round() as i32 - min_bx).clamp(0, width_b as i32 - 1) as usize;
                let light_y =
                    (actor_y.floor() as i32 - min_by).clamp(0, height_b as i32 - 1) as usize;
                let light_factor =
                    (light[light_y * width_b + light_x] as f32 / 15.0).clamp(0.45, 1.0);
                let fg = Self::rgb(Self::scale_rgb((176, 128, 72), light_factor));
                if scale > 1 {
                    self.actor_sprites.push(
                        ActorSprite::mob(
                            b_sx,
                            b_sy,
                            SpriteKind::Boat,
                            boat.facing_right,
                            boat.vx,
                            state.player.age,
                            fg,
                        )
                        .with_subcell(actor_x, actor_y, scale),
                    );
                } else {
                    let hull = if boat.wobble_timer > 0 {
                        if boat.facing_right { "\\_/" } else { "/_\\" }
                    } else if boat.facing_right {
                        "[_>"
                    } else {
                        "<_]"
                    };
                    self.put_entity_str(b_sx, b_sy, hull, fg);
                }
            }
        }

        for (actor_index, cow) in state.cows.iter().enumerate() {
            let (actor_x, actor_y) =
                self.motion
                    .position("cows", actor_index, (cow.x, cow.y), cow.age, frame_alpha);
            let c_sx = (actor_x * 2.0).round() as i32 - view_char_x + screen_center_x - 1;
            let c_sy = actor_y.floor() as i32 - view_char_y + screen_center_y - 1;
            if c_sx >= -4
                && c_sx < self.width as i32 + 4
                && c_sy >= -4
                && c_sy < self.height as i32 + 4
            {
                let c_f = (light[((actor_y - 1.0).floor() as i32 - min_by)
                    .clamp(0, height_b as i32 - 1) as usize
                    * width_b
                    + (actor_x.round() as i32 - min_bx).clamp(0, width_b as i32 - 1) as usize]
                    as f32
                    / 15.0)
                    .clamp(0.4, 1.0);
                let mut c_fg = Color::Rgb {
                    r: (139.0 * c_f) as u8,
                    g: (69.0 * c_f) as u8,
                    b: (19.0 * c_f) as u8,
                };
                if cow.hit_timer > 0 {
                    c_fg = Color::Rgb {
                        r: 255,
                        g: 50,
                        b: 50,
                    };
                }
                self.actor_sprites.push(
                    ActorSprite::mob(
                        c_sx,
                        c_sy,
                        SpriteKind::Cow,
                        cow.facing_right,
                        cow.vx,
                        cow.age,
                        c_fg,
                    )
                    .with_subcell(actor_x, actor_y, scale)
                    .with_hit(cow.hit_timer > 0),
                );
            }
        }

        for (actor_index, sheep) in state.sheep.iter().enumerate() {
            let (actor_x, actor_y) = self.motion.position(
                "sheep",
                actor_index,
                (sheep.x, sheep.y),
                sheep.age,
                frame_alpha,
            );
            let s_sx = (actor_x * 2.0).round() as i32 - view_char_x + screen_center_x - 1;
            let s_sy = actor_y.floor() as i32 - view_char_y + screen_center_y - 1;
            if s_sx >= -4
                && s_sx < self.width as i32 + 4
                && s_sy >= -4
                && s_sy < self.height as i32 + 4
            {
                let s_f = (light[((actor_y - 1.0).floor() as i32 - min_by)
                    .clamp(0, height_b as i32 - 1) as usize
                    * width_b
                    + (actor_x.round() as i32 - min_bx).clamp(0, width_b as i32 - 1) as usize]
                    as f32
                    / 15.0)
                    .clamp(0.4, 1.0);
                let mut s_fg = Color::Rgb {
                    r: (240.0 * s_f) as u8,
                    g: (240.0 * s_f) as u8,
                    b: (240.0 * s_f) as u8,
                };
                if sheep.hit_timer > 0 {
                    s_fg = Color::Rgb {
                        r: 255,
                        g: 50,
                        b: 50,
                    };
                }
                self.actor_sprites.push(
                    ActorSprite::mob(
                        s_sx,
                        s_sy,
                        SpriteKind::Sheep,
                        sheep.facing_right,
                        sheep.vx,
                        sheep.age,
                        s_fg,
                    )
                    .with_subcell(actor_x, actor_y, scale)
                    .with_hit(sheep.hit_timer > 0)
                    .with_variant(sheep.sheared),
                );
            }
        }

        for (actor_index, pig) in state.pigs.iter().enumerate() {
            let (actor_x, actor_y) =
                self.motion
                    .position("pigs", actor_index, (pig.x, pig.y), pig.age, frame_alpha);
            let p_sx = (actor_x * 2.0).round() as i32 - view_char_x + screen_center_x - 1;
            let p_sy = actor_y.floor() as i32 - view_char_y + screen_center_y - 1;
            if p_sx >= -4
                && p_sx < self.width as i32 + 4
                && p_sy >= -4
                && p_sy < self.height as i32 + 4
            {
                let p_f = (light[((actor_y - 1.0).floor() as i32 - min_by)
                    .clamp(0, height_b as i32 - 1) as usize
                    * width_b
                    + (actor_x.round() as i32 - min_bx).clamp(0, width_b as i32 - 1) as usize]
                    as f32
                    / 15.0)
                    .clamp(0.4, 1.0);
                let mut p_fg = Color::Rgb {
                    r: (245.0 * p_f) as u8,
                    g: (160.0 * p_f) as u8,
                    b: (175.0 * p_f) as u8,
                };
                if pig.hit_timer > 0 {
                    p_fg = Color::Rgb {
                        r: 255,
                        g: 50,
                        b: 50,
                    };
                }
                self.actor_sprites.push(
                    ActorSprite::mob(
                        p_sx,
                        p_sy,
                        SpriteKind::Pig,
                        pig.facing_right,
                        pig.vx,
                        pig.age,
                        p_fg,
                    )
                    .with_subcell(actor_x, actor_y, scale)
                    .with_hit(pig.hit_timer > 0),
                );
            }
        }

        for (actor_index, chicken) in state.chickens.iter().enumerate() {
            let (actor_x, actor_y) = self.motion.position(
                "chickens",
                actor_index,
                (chicken.x, chicken.y),
                chicken.age,
                frame_alpha,
            );
            let c_sx = (actor_x * 2.0).round() as i32 - view_char_x + screen_center_x - 1;
            let c_sy = actor_y.floor() as i32 - view_char_y + screen_center_y - 1;
            if c_sx >= -4
                && c_sx < self.width as i32 + 4
                && c_sy >= -4
                && c_sy < self.height as i32 + 4
            {
                let c_f = (light[((actor_y - 1.0).floor() as i32 - min_by)
                    .clamp(0, height_b as i32 - 1) as usize
                    * width_b
                    + (actor_x.round() as i32 - min_bx).clamp(0, width_b as i32 - 1) as usize]
                    as f32
                    / 15.0)
                    .clamp(0.4, 1.0);
                let mut c_fg = Color::Rgb {
                    r: (245.0 * c_f) as u8,
                    g: (245.0 * c_f) as u8,
                    b: (180.0 * c_f) as u8,
                };
                if chicken.hit_timer > 0 {
                    c_fg = Color::Rgb {
                        r: 255,
                        g: 50,
                        b: 50,
                    };
                }
                self.actor_sprites.push(
                    ActorSprite::mob(
                        c_sx,
                        c_sy,
                        SpriteKind::Chicken,
                        chicken.facing_right,
                        chicken.vx,
                        chicken.age,
                        c_fg,
                    )
                    .with_subcell(actor_x, actor_y, scale)
                    .with_hit(chicken.hit_timer > 0),
                );
            }
        }

        for (actor_index, squid) in state.squids.iter().enumerate() {
            let (actor_x, actor_y) = self.motion.position(
                "squids",
                actor_index,
                (squid.x, squid.y),
                squid.age,
                frame_alpha,
            );
            let s_sx = (actor_x * 2.0).round() as i32 - view_char_x + screen_center_x - 1;
            let s_sy = actor_y.floor() as i32 - view_char_y + screen_center_y - 1;
            if s_sx >= -4
                && s_sx < self.width as i32 + 4
                && s_sy >= -4
                && s_sy < self.height as i32 + 4
            {
                let s_f = (light[((actor_y - 0.45).floor() as i32 - min_by)
                    .clamp(0, height_b as i32 - 1) as usize
                    * width_b
                    + (actor_x.round() as i32 - min_bx).clamp(0, width_b as i32 - 1) as usize]
                    as f32
                    / 15.0)
                    .clamp(0.35, 1.0);
                let mut s_fg = Color::Rgb {
                    r: (120.0 * s_f) as u8,
                    g: (165.0 * s_f) as u8,
                    b: (182.0 * s_f) as u8,
                };
                if squid.hit_timer > 0 {
                    s_fg = Color::Rgb {
                        r: 255,
                        g: 70,
                        b: 70,
                    };
                }
                if scale > 1 {
                    self.actor_sprites.push(
                        ActorSprite::mob(
                            s_sx,
                            s_sy,
                            SpriteKind::Squid,
                            squid.facing_right,
                            squid.vx,
                            squid.age,
                            s_fg,
                        )
                        .with_subcell(actor_x, actor_y, scale)
                        .with_hit(squid.hit_timer > 0),
                    );
                } else {
                    self.put_entity_str(s_sx, s_sy - 1, "Sq", s_fg);
                    let tentacles = if squid.vx.abs() > 0.02 && (squid.age / 4).is_multiple_of(2) {
                        "ww"
                    } else {
                        "vv"
                    };
                    self.put_entity_str(s_sx, s_sy, tentacles, s_fg);
                }
            }
        }

        for (actor_index, wolf) in state.wolves.iter().enumerate() {
            let (actor_x, actor_y) = self.motion.position(
                "wolves",
                actor_index,
                (wolf.x, wolf.y),
                wolf.age,
                frame_alpha,
            );
            let w_sx = (actor_x * 2.0).round() as i32 - view_char_x + screen_center_x - 1;
            let w_sy = actor_y.floor() as i32 - view_char_y + screen_center_y - 1;
            if w_sx >= -4
                && w_sx < self.width as i32 + 4
                && w_sy >= -4
                && w_sy < self.height as i32 + 4
            {
                let w_f = (light[((actor_y - 0.6).floor() as i32 - min_by)
                    .clamp(0, height_b as i32 - 1) as usize
                    * width_b
                    + (actor_x.round() as i32 - min_bx).clamp(0, width_b as i32 - 1) as usize]
                    as f32
                    / 15.0)
                    .clamp(0.35, 1.0);
                let mut w_fg = if wolf.is_aggressive() {
                    Color::Rgb {
                        r: (215.0 * w_f) as u8,
                        g: (110.0 * w_f) as u8,
                        b: (110.0 * w_f) as u8,
                    }
                } else {
                    Color::Rgb {
                        r: (185.0 * w_f) as u8,
                        g: (185.0 * w_f) as u8,
                        b: (175.0 * w_f) as u8,
                    }
                };
                if wolf.hit_timer > 0 {
                    w_fg = Color::Rgb {
                        r: 255,
                        g: 70,
                        b: 70,
                    };
                }
                self.actor_sprites.push(
                    ActorSprite::mob(
                        w_sx,
                        w_sy,
                        SpriteKind::Wolf,
                        wolf.facing_right,
                        wolf.vx,
                        wolf.age,
                        w_fg,
                    )
                    .with_subcell(actor_x, actor_y, scale)
                    .with_hit(wolf.hit_timer > 0)
                    .with_warning(wolf.attack_windup)
                    .with_alert(wolf.aggressive_timer > 0),
                );
            }
        }

        for (actor_index, ocelot) in state.ocelots.iter().enumerate() {
            let (actor_x, actor_y) = self.motion.position(
                "ocelots",
                actor_index,
                (ocelot.x, ocelot.y),
                ocelot.age,
                frame_alpha,
            );
            let o_sx = (actor_x * 2.0).round() as i32 - view_char_x + screen_center_x - 1;
            let o_sy = actor_y.floor() as i32 - view_char_y + screen_center_y - 1;
            if o_sx >= -4
                && o_sx < self.width as i32 + 4
                && o_sy >= -4
                && o_sy < self.height as i32 + 4
            {
                let o_f = (light[((actor_y - 0.6).floor() as i32 - min_by)
                    .clamp(0, height_b as i32 - 1) as usize
                    * width_b
                    + (actor_x.round() as i32 - min_bx).clamp(0, width_b as i32 - 1) as usize]
                    as f32
                    / 15.0)
                    .clamp(0.35, 1.0);
                let mut o_fg = Color::Rgb {
                    r: (235.0 * o_f) as u8,
                    g: (185.0 * o_f) as u8,
                    b: (110.0 * o_f) as u8,
                };
                if ocelot.hit_timer > 0 {
                    o_fg = Color::Rgb {
                        r: 255,
                        g: 70,
                        b: 70,
                    };
                } else if ocelot.panic_timer > 0 {
                    o_fg = Color::Rgb {
                        r: (245.0 * o_f) as u8,
                        g: (145.0 * o_f) as u8,
                        b: (90.0 * o_f) as u8,
                    };
                }
                self.actor_sprites.push(
                    ActorSprite::mob(
                        o_sx,
                        o_sy,
                        SpriteKind::Ocelot,
                        ocelot.facing_right,
                        ocelot.vx,
                        ocelot.age,
                        o_fg,
                    )
                    .with_subcell(actor_x, actor_y, scale)
                    .with_hit(ocelot.hit_timer > 0),
                );
            }
        }

        for (actor_index, villager) in state.villagers.iter().enumerate() {
            let (actor_x, actor_y) = self.motion.position(
                "villagers",
                actor_index,
                (villager.x, villager.y),
                villager.age,
                frame_alpha,
            );
            let v_sx = (actor_x * 2.0).round() as i32 - view_char_x + screen_center_x - 1;
            let v_sy = actor_y.floor() as i32 - view_char_y + screen_center_y - 1;
            if v_sx >= -4
                && v_sx < self.width as i32 + 4
                && v_sy >= -4
                && v_sy < self.height as i32 + 4
            {
                let v_f = (light[((actor_y - 1.0).floor() as i32 - min_by)
                    .clamp(0, height_b as i32 - 1) as usize
                    * width_b
                    + (actor_x.round() as i32 - min_bx).clamp(0, width_b as i32 - 1) as usize]
                    as f32
                    / 15.0)
                    .clamp(0.4, 1.0);
                let mut v_fg = Color::Rgb {
                    r: (178.0 * v_f) as u8,
                    g: (130.0 * v_f) as u8,
                    b: (88.0 * v_f) as u8,
                };
                if villager.hit_timer > 0 {
                    v_fg = Color::Rgb {
                        r: 255,
                        g: 70,
                        b: 70,
                    };
                }
                self.actor_sprites.push(
                    ActorSprite::mob(
                        v_sx,
                        v_sy,
                        SpriteKind::Villager,
                        villager.facing_right,
                        villager.vx,
                        villager.age,
                        v_fg,
                    )
                    .with_subcell(actor_x, actor_y, scale)
                    .with_hit(villager.hit_timer > 0),
                );
            }
        }

        for (actor_index, item) in state.item_entities.iter().enumerate() {
            let (actor_x, actor_y) = self.motion.position(
                "item_entities",
                actor_index,
                (item.x, item.y),
                item.age,
                frame_alpha,
            );
            let i_char_x = (actor_x * 2.0).round() as i32;
            let i_char_y = actor_y.floor() as i32;
            let i_screen_x = i_char_x - view_char_x + screen_center_x;
            let i_screen_y = i_char_y - view_char_y + screen_center_y;
            if i_screen_x >= 0
                && i_screen_x < self.width as i32
                && i_screen_y >= 0
                && i_screen_y < self.height as i32
            {
                let i_fg = match item.item_type {
                    crate::world::item::ItemType::Dirt => Color::Rgb {
                        r: 150,
                        g: 100,
                        b: 50,
                    },
                    crate::world::item::ItemType::Grass => Color::Rgb {
                        r: 60,
                        g: 180,
                        b: 60,
                    },
                    crate::world::item::ItemType::Stone => Color::Rgb {
                        r: 130,
                        g: 130,
                        b: 130,
                    },
                    crate::world::item::ItemType::Wood => Color::Rgb {
                        r: 101,
                        g: 67,
                        b: 33,
                    },
                    crate::world::item::ItemType::Leaves => Color::Rgb {
                        r: 34,
                        g: 139,
                        b: 34,
                    },
                    crate::world::item::ItemType::Sapling => Color::Rgb {
                        r: 96,
                        g: 176,
                        b: 84,
                    },
                    crate::world::item::ItemType::BirchSapling => Color::Rgb {
                        r: 150,
                        g: 206,
                        b: 124,
                    },
                    crate::world::item::ItemType::Planks => Color::Rgb {
                        r: 180,
                        g: 140,
                        b: 80,
                    },
                    crate::world::item::ItemType::Stick => Color::Rgb {
                        r: 139,
                        g: 69,
                        b: 19,
                    },
                    crate::world::item::ItemType::RawIron => Color::Rgb {
                        r: 210,
                        g: 180,
                        b: 140,
                    },
                    crate::world::item::ItemType::IronIngot => Color::White,
                    crate::world::item::ItemType::Diamond => Color::Cyan,
                    crate::world::item::ItemType::RedstoneDust => Color::Red,
                    crate::world::item::ItemType::Lever => Color::Grey,
                    crate::world::item::ItemType::StoneButton => Color::Grey,
                    crate::world::item::ItemType::RedstoneTorch => Color::DarkRed,
                    crate::world::item::ItemType::RedstoneRepeater => Color::DarkRed,
                    crate::world::item::ItemType::Tnt => Color::Red,
                    crate::world::item::ItemType::Piston => Color::DarkGrey,
                    crate::world::item::ItemType::Netherrack => Color::DarkRed,
                    crate::world::item::ItemType::SoulSand => Color::DarkYellow,
                    crate::world::item::ItemType::Glowstone => Color::Rgb {
                        r: 255,
                        g: 225,
                        b: 80,
                    },
                    crate::world::item::ItemType::EndStone => Color::Rgb {
                        r: 215,
                        g: 208,
                        b: 150,
                    },
                    crate::world::item::ItemType::EyeOfEnder => Color::Rgb {
                        r: 85,
                        g: 210,
                        b: 120,
                    },
                    crate::world::item::ItemType::EnderPearl => Color::Rgb {
                        r: 95,
                        g: 245,
                        b: 175,
                    },
                    crate::world::item::ItemType::BlazeRod => Color::Rgb {
                        r: 250,
                        g: 205,
                        b: 90,
                    },
                    crate::world::item::ItemType::BlazePowder => Color::Rgb {
                        r: 255,
                        g: 180,
                        b: 60,
                    },
                    crate::world::item::ItemType::Slimeball => Color::Rgb {
                        r: 120,
                        g: 235,
                        b: 120,
                    },
                    crate::world::item::ItemType::Bed => Color::Rgb {
                        r: 220,
                        g: 70,
                        b: 70,
                    },
                    crate::world::item::ItemType::Chest => Color::Rgb {
                        r: 170,
                        g: 120,
                        b: 70,
                    },
                    crate::world::item::ItemType::Bookshelf => Color::Rgb {
                        r: 176,
                        g: 124,
                        b: 72,
                    },
                    crate::world::item::ItemType::Glass => Color::Rgb {
                        r: 190,
                        g: 220,
                        b: 235,
                    },
                    crate::world::item::ItemType::WoodDoor => Color::Rgb {
                        r: 160,
                        g: 110,
                        b: 70,
                    },
                    crate::world::item::ItemType::Boat => Color::Rgb {
                        r: 182,
                        g: 136,
                        b: 80,
                    },
                    crate::world::item::ItemType::Ladder => Color::Rgb {
                        r: 170,
                        g: 130,
                        b: 80,
                    },
                    crate::world::item::ItemType::StoneSlab
                    | crate::world::item::ItemType::StoneStairs => Color::Grey,
                    crate::world::item::ItemType::Torch => Color::Yellow,
                    crate::world::item::ItemType::Bucket => Color::Grey,
                    crate::world::item::ItemType::Shears => Color::Grey,
                    crate::world::item::ItemType::WaterBucket => Color::Blue,
                    crate::world::item::ItemType::LavaBucket => Color::Red,
                    crate::world::item::ItemType::FlintAndSteel => Color::Rgb {
                        r: 210,
                        g: 210,
                        b: 220,
                    },
                    crate::world::item::ItemType::Cobblestone => Color::DarkGrey,
                    crate::world::item::ItemType::Obsidian => Color::Rgb {
                        r: 48,
                        g: 25,
                        b: 52,
                    },
                    crate::world::item::ItemType::SugarCane => Color::Rgb {
                        r: 114,
                        g: 214,
                        b: 104,
                    },
                    crate::world::item::ItemType::Paper => Color::Rgb {
                        r: 230,
                        g: 226,
                        b: 208,
                    },
                    crate::world::item::ItemType::Book => Color::Rgb {
                        r: 146,
                        g: 96,
                        b: 58,
                    },
                    _ => Color::White,
                };
                let bob_y = (item.get_bobbing_offset() * 1.0).round() as i32;
                if scale > 1 {
                    self.actor_sprites.push(
                        ActorSprite::mob(
                            i_screen_x,
                            i_screen_y,
                            SpriteKind::Item,
                            true,
                            item.vx,
                            item.age,
                            Self::inventory_item_color(item.item_type),
                        )
                        .with_subcell(actor_x, actor_y + item.get_bobbing_offset(), scale)
                        .with_item(Some(item.item_type)),
                    );
                } else {
                    self.put_char(i_screen_x, i_screen_y + bob_y, 'i', i_fg, Color::Reset);
                }
            }
        }

        for (actor_index, orb) in state.experience_orbs.iter().enumerate() {
            let (actor_x, actor_y) = self.motion.position(
                "experience_orbs",
                actor_index,
                (orb.x, orb.y),
                orb.age,
                frame_alpha,
            );
            let o_char_x = (actor_x * 2.0).round() as i32;
            let o_char_y = actor_y.floor() as i32;
            let o_screen_x = o_char_x - view_char_x + screen_center_x;
            let o_screen_y = o_char_y - view_char_y + screen_center_y;
            if o_screen_x >= 0
                && o_screen_x < self.width as i32
                && o_screen_y >= 0
                && o_screen_y < self.height as i32
            {
                let pulse = ((state.player.age + orb.age) / 3).is_multiple_of(2);
                let orb_fg = if pulse {
                    Color::Rgb {
                        r: 190,
                        g: 255,
                        b: 110,
                    }
                } else {
                    Color::Rgb {
                        r: 120,
                        g: 225,
                        b: 65,
                    }
                };
                let bob_y = (orb.get_bobbing_offset() * 1.0).round() as i32;
                if scale > 1 {
                    self.actor_sprites.push(
                        ActorSprite::mob(
                            o_screen_x,
                            o_screen_y,
                            SpriteKind::Orb,
                            true,
                            orb.vx,
                            orb.age,
                            orb_fg,
                        )
                        .with_subcell(actor_x, actor_y + orb.get_bobbing_offset(), scale)
                        .with_dimensions(2, 1),
                    );
                } else {
                    self.put_char(o_screen_x, o_screen_y + bob_y, '*', orb_fg, Color::Reset);
                }
            }
        }

        self.finish_world_view(
            physical_size,
            scale,
            state,
            &light,
            (min_bx, min_by, width_b, height_b),
            frame_alpha,
        );
        let ui_y = self.height as i32 - 4;
        let screen_center_x = screen_center_x * scale;
        let screen_center_y = screen_center_y * scale;
        let view_char_x = physical_view_x;
        let view_char_y = physical_view_y;
        self.render_end_victory_sequence(
            state,
            view_char_x,
            view_char_y,
            screen_center_x,
            screen_center_y,
            ui_y,
        );
        let (hover_bx, hover_by) = self.screen_to_world(state, state.mouse_x, state.mouse_y);
        let px = render_player_x;
        let py = render_player_y - 1.0;
        let dx = px - (hover_bx as f64 + 0.5);
        let dy = py - (hover_by as f64 + 0.5);
        let dist = (dx * dx + dy * dy).sqrt();
        if dist <= 4.5 && !state.inventory_open {
            let h_sx = (hover_bx * 2 * scale) - view_char_x + screen_center_x;
            let h_sy = hover_by * scale - view_char_y + screen_center_y;
            for y in h_sy..h_sy + scale {
                for x in h_sx..h_sx + 2 * scale {
                    if x >= 0 && x < self.width as i32 && y >= 0 && y < ui_y {
                        let cell =
                            &mut self.back_buffer[y as usize * self.width as usize + x as usize];
                        cell.bg = Self::target_background(cell.bg);
                    }
                }
            }
        }

        let hud_bg = Color::AnsiValue(236);
        for y in ui_y..(self.height as i32) {
            for x in 0..(self.width as i32) {
                self.put_world_char(x, y, ' ', Color::Reset, hud_bg);
            }
        }
        for x in 0..(self.width as i32) {
            self.put_world_char(x, ui_y, '-', Color::White, hud_bg);
        }
        let slot_w = 11;
        let slot_gap = 1;
        let total_hotbar_w = slot_w * 9 + slot_gap * 8;
        let hotbar_x = ((self.width as i32 - total_hotbar_w) / 2).max(1);
        for i in 0..9 {
            let slot_x = hotbar_x + i as i32 * (slot_w + slot_gap);
            let is_selected = state.hotbar_index == i as u8;
            let stack = state.inventory.slots[i].as_ref();
            let slot_bg = Self::hotbar_slot_background(stack, is_selected);
            let slot_fg = Self::hotbar_slot_text_color(slot_bg, is_selected);

            for row in 1..=2 {
                for col in 0..slot_w {
                    self.put_world_char(slot_x + col, ui_y + row, ' ', Color::Reset, slot_bg);
                }
            }

            let (label, badge, icon, icon_fg, metric, metric_fg) = if let Some(stack) = stack {
                let enchant_level = state.inventory_slot_enchant_level(i);
                (
                    Self::inventory_item_label(stack.item_type),
                    if enchant_level > 0 {
                        let badge_level = enchant_level.min(9);
                        format!("+{badge_level}")
                    } else {
                        "  ".to_string()
                    },
                    Self::hotbar_item_glyph(stack.item_type),
                    Self::inventory_item_color(stack.item_type),
                    Self::hotbar_stack_metric(stack).0,
                    Self::hotbar_stack_metric(stack).1,
                )
            } else {
                (
                    "--".to_string(),
                    "  ".to_string(),
                    "    ",
                    slot_fg,
                    String::new(),
                    Color::DarkGrey,
                )
            };
            let left_edge = if is_selected { '>' } else { '[' };
            let right_edge = if is_selected { '<' } else { ']' };
            let slot_number = i + 1;
            self.put_str(
                slot_x,
                ui_y + 1,
                &format!("{left_edge}{slot_number:>2} {label:<2} {badge:>2} {right_edge}"),
                slot_fg,
                slot_bg,
            );
            self.put_char(slot_x, ui_y + 2, left_edge, slot_fg, slot_bg);
            self.put_str(slot_x + 1, ui_y + 2, icon, icon_fg, slot_bg);
            self.put_str(
                slot_x + 5,
                ui_y + 2,
                &format!("{metric:>5}"),
                metric_fg,
                slot_bg,
            );
            self.put_char(slot_x + slot_w - 1, ui_y + 2, right_edge, slot_fg, slot_bg);
            if self.pixel_colors
                && let Some(stack) = stack
            {
                self.put_str(
                    slot_x,
                    ui_y + 1,
                    &format!("{slot_number}    {label:<2} {badge:>2}{right_edge}"),
                    slot_fg,
                    slot_bg,
                );
                self.draw_item_icon(slot_x + 1, ui_y + 1, stack.item_type, slot_bg);
            }
        }

        let detail_line = if let Some(stack) = &state.inventory.slots[state.hotbar_index as usize] {
            Self::hotbar_selected_detail(stack, state.selected_hotbar_enchant_level())
        } else {
            let empty_slot = state.hotbar_index + 1;
            format!("Held: slot {empty_slot} empty")
        };
        let detail_fg = state.inventory.slots[state.hotbar_index as usize]
            .as_ref()
            .map(|stack| Self::inventory_item_color(stack.item_type))
            .unwrap_or(Color::DarkGrey);
        let detail_w = (self.width as i32 - 2).max(0) as usize;
        if detail_w > 0 {
            let mut detail = detail_line;
            if detail.len() > detail_w {
                detail.truncate(detail_w);
            }
            self.put_str(
                1,
                ui_y + 3,
                &format!("{detail:<detail_w$}"),
                detail_fg,
                hud_bg,
            );
        }

        let hud_x = self.width as i32 - 22;
        let boss_active = state.current_dimension == Dimension::End && state.ender_dragon.is_some();
        let focused = state.focused_entity(hover_bx, hover_by);
        let show_target = focused.is_some()
            || (dist <= 4.5 && state.world.get_block(hover_bx, hover_by) != BlockType::Air)
            || state.player.mining_timer > 0.0;
        for x in 0..self.width as i32 {
            self.put_world_char(x, 0, ' ', Color::White, Color::AnsiValue(234));
        }
        for y in 1..=if boss_active || !show_target { 5 } else { 11 } {
            for x in (hud_x - 1).max(0)..self.width as i32 {
                self.put_world_char(x, y, ' ', Color::White, Color::AnsiValue(234));
            }
        }
        let mut h_str = String::new();
        for _ in 0..(state.player.health / 2.0).floor() as i32 {
            h_str.push('♥');
        }
        if state.player.health % 2.0 >= 1.0 {
            h_str.push('♡');
        }
        let mut n_str = String::new();
        for _ in 0..(state.player.hunger / 2.0).floor() as i32 {
            n_str.push('#');
        }
        if state.player.hunger % 2.0 >= 1.0 {
            n_str.push('+');
        }
        self.put_str(
            hud_x,
            1,
            &format!("HP   {h_str:<15}"),
            Color::Red,
            Color::Reset,
        );
        self.put_str(
            hud_x,
            2,
            &format!("Food {n_str:<15}"),
            Color::Rgb {
                r: 205,
                g: 133,
                b: 63,
            },
            Color::Reset,
        );
        let armor_points = state.total_armor_points();
        self.put_str(
            hud_x,
            3,
            &format!("Armor {armor_points:>2}/20"),
            Color::Cyan,
            Color::Reset,
        );
        let xp_progress = state.player.experience_progress.clamp(0.0, 1.0);
        let xp_fill = (xp_progress * 10.0).round() as usize;
        let mut xp_bar = String::new();
        for i in 0..10 {
            xp_bar.push(if i < xp_fill { '=' } else { '-' });
        }
        self.put_str(
            hud_x,
            4,
            &format!(
                "Lv {:>2} [{}] {:>3}%",
                state.player.experience_level,
                xp_bar,
                (xp_progress * 100.0).round() as i32
            ),
            Color::Green,
            Color::Reset,
        );
        self.put_str(
            hud_x,
            5,
            if state.is_sprinting() {
                "Sprint ON       "
            } else {
                "Sprint OFF      "
            },
            if state.is_sprinting() {
                Color::Yellow
            } else {
                Color::DarkGrey
            },
            Color::Reset,
        );
        if state.current_dimension == Dimension::End {
            if let Some((ticks_remaining, _, _)) = state.end_victory_sequence_state() {
                let (headline, _) = Self::end_victory_banner_lines(ticks_remaining);
                let pulse = (((140u16.saturating_sub(ticks_remaining)) as f32 * 0.24).sin() * 0.5
                    + 0.5)
                    .clamp(0.0, 1.0);
                self.put_str(
                    hud_x - 2,
                    3,
                    &format!("{headline:<20}"),
                    Self::rgb(Self::lerp_rgb(
                        (185, 135, 255),
                        (255, 255, 255),
                        pulse * 0.5,
                    )),
                    Color::Reset,
                );
            } else if let Some(dragon) = &state.ender_dragon {
                let hp_ratio = (dragon.health / dragon.max_health).clamp(0.0, 1.0);
                let bars = (hp_ratio * 20.0).round() as usize;
                let mut hp_bar = String::new();
                for i in 0..20 {
                    hp_bar.push(if i < bars { '█' } else { '░' });
                }
                let phase = match dragon.phase {
                    crate::entities::ender_dragon::DragonPhase::Patrol => "FLYING",
                    crate::entities::ender_dragon::DragonPhase::Swoop => "SWOOP",
                };
                let banner = format!("Ender Dragon [{hp_bar}] {phase}");
                let banner_width = banner.chars().count() as i32;
                let banner_x = ((self.width as i32 - banner_width) / 2)
                    .min(hud_x - banner_width - 2)
                    .max(0);
                self.put_str(
                    banner_x,
                    1,
                    &banner,
                    if dragon.hit_timer > 0 {
                        Color::Red
                    } else {
                        Color::Rgb {
                            r: 235,
                            g: 115,
                            b: 245,
                        }
                    },
                    Color::AnsiValue(234),
                );
            } else if state.has_defeated_dragon() && !state.has_seen_completion_credits() {
                self.put_str(
                    hud_x - 2,
                    3,
                    "Use portal for ending",
                    Color::Rgb {
                        r: 175,
                        g: 225,
                        b: 255,
                    },
                    Color::Reset,
                );
            }
        }

        if state.show_debug_overlay {
            let chunk_ms = state.world.chunk_metrics.last_load_us as f64 / 1000.0;
            let chunk_max_ms = state.world.chunk_metrics.max_load_us as f64 / 1000.0;
            let (rule_spawn, rule_day, rule_weather, rule_keep_inv) = state.game_rule_flags();
            self.put_str(
            1,
            0,
            &format!(
                "Chunk {:>5.2}ms max {:>5.2}ms p{:>2} sg:{} ag:{} dim:{:?} w:{:?} mv:{} df:{} gr:{} r:{}/{}/{}/{} opt:O amb:{:>2}/{:>2}/{:>2}",
                chunk_ms,
                chunk_max_ms,
                state.world.chunk_metrics.pending_requests,
                state.world.chunk_metrics.sync_generated,
                state.world.chunk_metrics.async_generated,
                state.current_dimension,
                state.weather,
                state.movement_profile_name(),
                state.difficulty_name(),
                state.game_rules_preset_name(),
                u8::from(rule_spawn),
                u8::from(rule_day),
                u8::from(rule_weather),
                u8::from(rule_keep_inv),
                (rain_mix * 9.0).round() as i32,
                (wind_mix * 9.0).round() as i32,
                (thunder_mix * 9.0).round() as i32
            ),
            Color::DarkGrey,
            Color::Reset,
        );
            self.put_str(1, self.height.saturating_sub(5) as i32,
                &format!("p95 tick {:.2}ms render {:.2}ms save/cache {:.2}ms | chunks {} evicted {} | drops {} | keys:{}",
                    state.performance.tick.p95_ms(), state.performance.render.p95_ms(),
                    state.performance.save_and_cache.p95_ms(), state.world.loaded_chunk_count(),
                    state.world.chunk_metrics.evicted_chunks, state.performance.dropped_catchups,
                    if self.key_releases { "release" } else { "repeat" }),
                Color::Grey, Color::AnsiValue(234));
        } else {
            let chat_hint = if state.chat.mode.is_empty() {
                ""
            } else {
                "T: chat  "
            };
            let settings_hint = if state.world.is_replica() {
                ""
            } else {
                "  O: settings"
            };
            self.put_str(
                1,
                0,
                &format!(
                    "Termcraft | {:?} | {} | {}    {chat_hint}E: inventory{settings_hint}  F3: diagnostics  F4: view",
                    state.current_dimension,
                    Self::local_weather_label(state),
                    state.difficulty_name()
                ),
                Color::Grey,
                Color::Reset,
            );
        }
        if let Some(error) = state.save_error.borrow().as_ref() {
            let message: String = error
                .chars()
                .map(|c| if c.is_control() { ' ' } else { c })
                .collect();
            self.put_str(
                1,
                self.height.saturating_sub(1) as i32,
                &message,
                Color::Yellow,
                Color::AnsiValue(52),
            );
        }
        if state.eye_guidance_timer > 0 {
            let dir = if state.eye_guidance_dir >= 0 {
                "E"
            } else {
                "W"
            };
            self.put_str(
                1,
                1,
                &format!(
                    "Eye points {}  ~{} blocks  t-{}",
                    dir, state.eye_guidance_distance, state.eye_guidance_timer
                ),
                Color::Cyan,
                Color::Reset,
            );
        }
        if let Some(fishing_status) = state.fishing_status_line() {
            self.put_str(1, 2, fishing_status, Color::Yellow, Color::Reset);
        }
        if !state.chat.status.is_empty() {
            let text: String = format!("GM: {}", state.chat.status)
                .chars()
                .take((hud_x - 2).max(0) as usize)
                .collect();
            self.put_str(1, 3, &text, Color::Cyan, Color::Reset);
        }

        if let Some(trial) = &state.trial {
            let available = (hud_x - 2).max(0) as usize;
            let full = trial.objective();
            let goal = if full.chars().count() <= available {
                full
            } else {
                trial.compact_objective()
            };
            let goal: String = goal.chars().take(available).collect();
            self.put_str(
                1,
                2,
                &goal,
                Self::rgb((241, 212, 137)),
                Color::AnsiValue(234),
            );
            if state.chat.status.is_empty()
                && !state.inventory_open
                && let Some(hint) = state.trial_controls_hint()
            {
                let hint: String = hint.chars().take(available).collect();
                self.put_str(
                    1,
                    3,
                    &hint,
                    Self::rgb((172, 185, 195)),
                    Color::AnsiValue(234),
                );
            }
        }

        if state.inventory_open {
            self.draw_inventory_overlay(state);
        } else if show_target {
            let t_str = if let Some((name, _)) = focused {
                name
            } else if dist <= 4.5 {
                match state.world.get_block(hover_bx, hover_by) {
                    BlockType::Air => "Air",
                    BlockType::Dirt => "Dirt",
                    BlockType::Grass => "Grass",
                    BlockType::Stone => "Stone",
                    BlockType::StoneBricks => "Stone Bricks",
                    BlockType::Wood => "Wood",
                    BlockType::Leaves => "Leaves",
                    BlockType::IronOre => "Iron Ore",
                    BlockType::GoldOre => "Gold Ore",
                    BlockType::DiamondOre => "Diamond Ore",
                    BlockType::CoalOre => "Coal Ore",
                    BlockType::RedstoneOre => "Redst. Ore",
                    BlockType::Sand => "Sand",
                    BlockType::Gravel => "Gravel",
                    BlockType::Bedrock => "Bedrock",
                    BlockType::Planks => "Planks",
                    BlockType::CraftingTable => "Craft.Table",
                    BlockType::Bed => "Bed",
                    BlockType::Chest => "Chest",
                    BlockType::Bookshelf => "Bookshelf",
                    BlockType::Glass => "Glass",
                    BlockType::Wool => "Wool",
                    BlockType::StoneSlab => "Stone Slab",
                    BlockType::StoneStairs => "Stone Stairs",
                    BlockType::EnchantingTable => "Enchant Tbl",
                    BlockType::Anvil => "Anvil",
                    BlockType::BrewingStand => "Brewing Std",
                    BlockType::Torch => "Torch",
                    BlockType::Tnt => "TNT",
                    BlockType::PrimedTnt(_) => "Primed TNT",
                    BlockType::Lever(on) => {
                        if on {
                            "Lever (On)"
                        } else {
                            "Lever (Off)"
                        }
                    }
                    BlockType::StoneButton(t) => {
                        if t > 0 {
                            "Button (On)"
                        } else {
                            "Button (Off)"
                        }
                    }
                    BlockType::RedstoneTorch(lit) => {
                        if lit {
                            "RSTorch (On)"
                        } else {
                            "RSTorch (Off)"
                        }
                    }
                    BlockType::Furnace => "Furnace",
                    BlockType::Snow => "Snow",
                    BlockType::Ice => "Ice",
                    BlockType::Cactus => "Cactus",
                    BlockType::DeadBush => "Dead Bush",
                    BlockType::BirchWood => "Birch Wood",
                    BlockType::BirchLeaves => "Birch Leaves",
                    BlockType::RedFlower => "Red Flower",
                    BlockType::YellowFlower => "Yellow Flower",
                    BlockType::TallGrass => "Tall Grass",
                    BlockType::Sapling => "Sapling",
                    BlockType::BirchSapling => "Birch Sapling",
                    BlockType::SugarCane => "Sugar Cane",
                    BlockType::Water(_) => "Water",
                    BlockType::Lava(_) => "Lava",
                    BlockType::Cobblestone => "Cobblestone",
                    BlockType::Obsidian => "Obsidian",
                    BlockType::Netherrack => "Netherrack",
                    BlockType::SoulSand => "Soul Sand",
                    BlockType::Glowstone => "Glowstone",
                    BlockType::NetherPortal => "Nether Portal",
                    BlockType::EndPortalFrame { filled } => {
                        if filled {
                            "End Frame (Eye)"
                        } else {
                            "End Frame"
                        }
                    }
                    BlockType::EndPortal => "End Portal",
                    BlockType::EndStone => "End Stone",
                    BlockType::IronDoor(open) => {
                        if open {
                            "Iron Door (Open)"
                        } else {
                            "Iron Door"
                        }
                    }
                    BlockType::WoodDoor(open) => {
                        if open {
                            "Wood Door (Open)"
                        } else {
                            "Wood Door"
                        }
                    }
                    BlockType::Ladder => "Ladder",
                    BlockType::SilverfishSpawner => "Sfish Spwnr",
                    BlockType::BlazeSpawner => "Blaze Spwnr",
                    BlockType::ZombieSpawner => "Zombie Spwnr",
                    BlockType::SkeletonSpawner => "Skel Spwnr",
                    BlockType::RedstoneDust(p) => {
                        if p > 0 {
                            "Powered Dust"
                        } else {
                            "Redstone Dust"
                        }
                    }
                    BlockType::RedstoneRepeater {
                        powered,
                        facing_right,
                        delay,
                        ..
                    } => match (powered, facing_right, delay.clamp(1, 4)) {
                        (true, true, 1) => "Rpt>On d1",
                        (true, true, 2) => "Rpt>On d2",
                        (true, true, 3) => "Rpt>On d3",
                        (true, true, 4) => "Rpt>On d4",
                        (true, false, 1) => "Rpt<On d1",
                        (true, false, 2) => "Rpt<On d2",
                        (true, false, 3) => "Rpt<On d3",
                        (true, false, 4) => "Rpt<On d4",
                        (false, true, 1) => "Rpt>Offd1",
                        (false, true, 2) => "Rpt>Offd2",
                        (false, true, 3) => "Rpt>Offd3",
                        (false, true, 4) => "Rpt>Offd4",
                        (false, false, 1) => "Rpt<Offd1",
                        (false, false, 2) => "Rpt<Offd2",
                        (false, false, 3) => "Rpt<Offd3",
                        (false, false, 4) => "Rpt<Offd4",
                        _ => "Repeater",
                    },
                    BlockType::Piston {
                        extended,
                        facing_right,
                    } => {
                        if extended {
                            if facing_right {
                                "Piston (Ext >)"
                            } else {
                                "Piston (Ext <)"
                            }
                        } else if facing_right {
                            "Piston (Idle >)"
                        } else {
                            "Piston (Idle <)"
                        }
                    }
                    BlockType::StickyPiston {
                        extended,
                        facing_right,
                    } => {
                        if extended {
                            if facing_right {
                                "Sticky (Ext >)"
                            } else {
                                "Sticky (Ext <)"
                            }
                        } else if facing_right {
                            "Sticky (Idle >)"
                        } else {
                            "Sticky (Idle <)"
                        }
                    }
                    BlockType::Farmland(m) => {
                        if m > 0 {
                            "Wet Farmland"
                        } else {
                            "Farmland"
                        }
                    }
                    BlockType::Crops(7) => "Ripe Wheat",
                    BlockType::Crops(_) => "Growing Wheat",
                    BlockType::NetherWart(3) => "Mature Nether Wart",
                    BlockType::NetherWart(_) => "Growing Nether Wart",
                }
            } else {
                "Too Far"
            };
            let target: String = t_str.chars().take(16).collect();
            let target_y = if boss_active { (ui_y - 6).max(7) } else { 7 };
            if boss_active {
                // Keep the airspace around the boss clear while preserving
                // readable mining/reach feedback above the hotbar.
                for y in target_y..=target_y + 3 {
                    for x in (hud_x - 1).max(0)..self.width as i32 {
                        self.put_world_char(x, y, ' ', Color::White, Color::AnsiValue(234));
                    }
                }
            }
            self.put_str(
                hud_x,
                target_y,
                "┌─ Target ─────────┐",
                Color::White,
                Color::Reset,
            );
            self.put_str(
                hud_x,
                target_y + 1,
                &format!("│ {target:<16} │"),
                Color::Yellow,
                Color::Reset,
            );
            self.put_str(
                hud_x,
                target_y + 2,
                "└──────────────────┘",
                Color::White,
                Color::Reset,
            );
            let progress =
                if state.player.last_mine_x == hover_bx && state.player.last_mine_y == hover_by {
                    state.player.mining_timer.clamp(0.0, 1.0)
                } else {
                    0.0
                };
            let feedback = if let Some((_, health)) = focused {
                format!("{health:.0} HP | L-click hit")
            } else if progress > 0.0 {
                let filled = (progress * 8.0).floor() as usize;
                format!(
                    "Mine [{}{}] {:>3}%",
                    "#".repeat(filled),
                    "-".repeat(8 - filled),
                    (progress * 100.0) as u32
                )
            } else if dist > 4.5 {
                "Move closer to target".into()
            } else if let Some(hint) = state.portal_interaction_hint(hover_bx, hover_by) {
                hint.into()
            } else if !state.can_see_block(state.player.x, state.player.y - 1.0, hover_bx, hover_by)
            {
                "Blocked: move around".into()
            } else if matches!(
                state.world.get_block(hover_bx, hover_by),
                BlockType::Chest
                    | BlockType::Furnace
                    | BlockType::CraftingTable
                    | BlockType::Bed
                    | BlockType::WoodDoor(_)
                    | BlockType::IronDoor(_)
            ) {
                "R-click / F: use".into()
            } else {
                format!("Reach {dist:.1} / 4.5")
            };
            self.put_str(hud_x, target_y + 3, &feedback, Color::Cyan, Color::Reset);
        }

        if state.is_showing_startup_splash() {
            self.draw_startup_splash_overlay(state);
        } else if state.is_showing_death_screen() {
            self.draw_death_overlay(state);
        } else if state.is_showing_credits() {
            self.draw_credits_overlay(state);
        } else if state.is_settings_menu_open() {
            self.draw_settings_overlay(state);
        } else if state.chat.open {
            self.draw_chat_overlay(state);
        }

        let mut encoded = std::mem::take(&mut self.frame_output);
        encoded.clear();
        queue!(&mut encoded, BeginSynchronizedUpdate)?;
        // Uniform pixel halves are a background cell. Invisible foreground
        // colors must not cause extra updates or split otherwise identical runs.
        for cell in &mut self.back_buffer {
            if matches!(cell.ch, '▀' | '▄' | '█') && cell.fg == cell.bg {
                cell.ch = ' ';
            }
            if cell.ch == ' ' {
                cell.fg = Color::Reset;
            }
        }
        let mut run = String::new();
        let mut cursor = None;
        let mut last_fg = Color::Reset;
        let mut last_bg = Color::Reset;
        let row_width = self.width as usize;
        for y in 0..self.height {
            let row_start = (y as usize) * row_width;
            let mut x = 0usize;
            while x < row_width {
                let idx = row_start + x;
                let cell = self.back_buffer[idx];
                if self.buffer[idx] == cell {
                    x += 1;
                    continue;
                }

                let run_fg = cell.fg;
                let run_bg = cell.bg;
                let run_start = x;
                run.clear();

                while x < row_width {
                    let run_idx = row_start + x;
                    let run_cell = self.back_buffer[run_idx];
                    if self.buffer[run_idx] != run_cell
                        && run_cell.fg == run_fg
                        && run_cell.bg == run_bg
                    {
                        self.buffer[run_idx] = run_cell;
                        run.push(run_cell.ch);
                        x += 1;
                    } else {
                        break;
                    }
                }

                if cursor != Some((run_start, y)) {
                    queue!(&mut encoded, MoveTo(run_start as u16, y))?;
                }
                cursor = Some((x, y));
                if run_fg != last_fg {
                    queue!(&mut encoded, SetForegroundColor(run_fg))?;
                    last_fg = run_fg;
                }
                if run_bg != last_bg {
                    queue!(&mut encoded, SetBackgroundColor(run_bg))?;
                    last_bg = run_bg;
                }
                queue!(&mut encoded, Print(&run))?;
            }
        }
        queue!(&mut encoded, ResetColor, EndSynchronizedUpdate)?;
        let result = output.write_all(&encoded).and_then(|_| output.flush());
        self.frame_output = encoded;
        result?;
        Ok(())
    }

    fn draw_chat_overlay(&mut self, state: &GameState) {
        let w = (self.width as i32 - 4).clamp(1, 100);
        let h = (self.height as i32 - 4).clamp(1, 24);
        let x = ((self.width as i32 - w) / 2).max(0);
        let y = ((self.height as i32 - h) / 2).max(0);
        let bg = Color::AnsiValue(235);
        for py in y..y + h {
            for px in x..x + w {
                self.put_char(px, py, ' ', Color::White, bg);
            }
        }
        if w < 30 || h < 9 {
            self.put_str(x, y, "Resize terminal for chat", Color::Yellow, bg);
            return;
        }
        for px in x..x + w {
            self.put_char(px, y, '─', Color::Cyan, bg);
            self.put_char(px, y + h - 1, '─', Color::Cyan, bg);
        }
        for py in y..y + h {
            self.put_char(x, py, '│', Color::Cyan, bg);
            self.put_char(x + w - 1, py, '│', Color::Cyan, bg);
        }
        let title: String = format!(" AI GAME MASTER | {} ", state.chat.mode)
            .chars()
            .take((w - 4) as usize)
            .collect();
        self.put_str(x + 2, y, &title, Color::Cyan, bg);
        let hint: String = if state.chat.mode == "host-only" {
            "Shared game-master messages. The first connected player controls requests."
        } else {
            "Describe a battle, mobs, weather or structure. /connect: your LLM client."
        }
        .chars()
        .take((w - 4) as usize)
        .collect();
        self.put_str(x + 2, y + 1, &hint, Color::Grey, bg);
        let width = (w - 4) as usize;
        let mut lines = Vec::new();
        for message in &state.chat.lines {
            let color = if message.starts_with("You:") {
                Color::Yellow
            } else {
                Color::White
            };
            let chars: Vec<char> = message.chars().collect();
            for part in chars.chunks(width) {
                lines.push((part.iter().collect::<String>(), color));
            }
        }
        let rows = (h - 7) as usize;
        for (row, (line, color)) in lines
            .iter()
            .skip(lines.len().saturating_sub(rows))
            .enumerate()
        {
            self.put_str(x + 2, y + 3 + row as i32, line, *color, bg);
        }
        let prompt: Vec<char> = state.chat.draft.chars().collect();
        let tail: String = prompt
            .iter()
            .skip(prompt.len().saturating_sub(width - 3))
            .collect();
        self.put_str(
            x + 2,
            y + h - 3,
            &format!("> {tail}_"),
            Color::White,
            Color::AnsiValue(237),
        );
        let controls: String = "Enter: send/resume   Esc: close   Ctrl+C: cancel request"
            .chars()
            .take(width)
            .collect();
        self.put_str(x + 2, y + h - 2, &controls, Color::Grey, bg);
    }

    fn draw_credits_overlay(&mut self, state: &GameState) {
        for cell in &mut self.back_buffer {
            if matches!(cell.bg, Color::Reset) {
                cell.bg = Color::AnsiValue(233);
            }
        }

        let panel_w = (self.width as i32 - 10).clamp(44, 90);
        let panel_h = (self.height as i32 - 8).clamp(15, 28);
        let panel_x = ((self.width as i32 - panel_w) / 2).max(0);
        let panel_y = ((self.height as i32 - panel_h) / 2).max(0);
        let panel_bg = Color::AnsiValue(236);

        for y in panel_y..(panel_y + panel_h) {
            for x in panel_x..(panel_x + panel_w) {
                self.put_char(x, y, ' ', Color::White, panel_bg);
            }
        }

        self.put_char(panel_x, panel_y, '+', Color::White, panel_bg);
        self.put_char(panel_x + panel_w - 1, panel_y, '+', Color::White, panel_bg);
        self.put_char(panel_x, panel_y + panel_h - 1, '+', Color::White, panel_bg);
        self.put_char(
            panel_x + panel_w - 1,
            panel_y + panel_h - 1,
            '+',
            Color::White,
            panel_bg,
        );
        for x in (panel_x + 1)..(panel_x + panel_w - 1) {
            self.put_char(x, panel_y, '-', Color::White, panel_bg);
            self.put_char(x, panel_y + panel_h - 1, '-', Color::White, panel_bg);
        }
        for y in (panel_y + 1)..(panel_y + panel_h - 1) {
            self.put_char(panel_x, y, '|', Color::White, panel_bg);
            self.put_char(panel_x + panel_w - 1, y, '|', Color::White, panel_bg);
        }

        let title = "THE END";
        self.draw_termcraft_wordmark(
            panel_x + (panel_w / 2),
            panel_y + 1,
            Color::Rgb {
                r: 190,
                g: 220,
                b: 255,
            },
            panel_bg,
        );
        self.put_str(
            panel_x + ((panel_w - title.len() as i32) / 2),
            panel_y + 3,
            title,
            Color::Rgb {
                r: 220,
                g: 220,
                b: 255,
            },
            panel_bg,
        );

        let credits_lines = [
            "You defeated the Ender Dragon.",
            "",
            "Peace returns to the End sky.",
            "",
            "termcraft",
            "2D terminal survival sandbox",
            "",
            "Built with Rust and Crossterm",
            "",
            "Design, world systems, and gameplay",
            "Sebastian + Codex",
            "",
            "Thank you for playing.",
            "",
            "Press Enter, Space, or Esc",
            "to continue.",
        ];
        let scroll = state.credits_scroll_row();
        let text_top = panel_y + 5;
        let text_bottom = panel_y + panel_h - 2;
        for (i, line) in credits_lines.iter().enumerate() {
            let y = text_bottom - scroll + i as i32;
            if y < text_top || y >= text_bottom {
                continue;
            }
            let line_x = panel_x + ((panel_w - line.len() as i32) / 2).max(1);
            self.put_str(
                line_x,
                y,
                line,
                Color::Rgb {
                    r: 210,
                    g: 210,
                    b: 225,
                },
                panel_bg,
            );
        }
    }

    fn draw_death_overlay(&mut self, state: &GameState) {
        for cell in &mut self.back_buffer {
            if matches!(cell.bg, Color::Reset) {
                cell.bg = Color::AnsiValue(52);
            }
        }

        let panel_w = (self.width as i32 - 14).clamp(38, 72);
        let panel_h = (self.height as i32 - 10).clamp(11, 16);
        let panel_x = ((self.width as i32 - panel_w) / 2).max(0);
        let panel_y = ((self.height as i32 - panel_h) / 2).max(0);
        let panel_bg = Color::AnsiValue(236);

        for y in panel_y..(panel_y + panel_h) {
            for x in panel_x..(panel_x + panel_w) {
                self.put_char(x, y, ' ', Color::White, panel_bg);
            }
        }

        self.put_char(panel_x, panel_y, '+', Color::White, panel_bg);
        self.put_char(panel_x + panel_w - 1, panel_y, '+', Color::White, panel_bg);
        self.put_char(panel_x, panel_y + panel_h - 1, '+', Color::White, panel_bg);
        self.put_char(
            panel_x + panel_w - 1,
            panel_y + panel_h - 1,
            '+',
            Color::White,
            panel_bg,
        );
        for x in (panel_x + 1)..(panel_x + panel_w - 1) {
            self.put_char(x, panel_y, '-', Color::White, panel_bg);
            self.put_char(x, panel_y + panel_h - 1, '-', Color::White, panel_bg);
        }
        for y in (panel_y + 1)..(panel_y + panel_h - 1) {
            self.put_char(panel_x, y, '|', Color::White, panel_bg);
            self.put_char(panel_x + panel_w - 1, y, '|', Color::White, panel_bg);
        }

        let title = "YOU DIED";
        self.put_str(
            panel_x + ((panel_w - title.len() as i32) / 2),
            panel_y + 2,
            title,
            Color::Rgb {
                r: 255,
                g: 80,
                b: 80,
            },
            panel_bg,
        );
        let (_, _, _, rule_keep_inv) = state.game_rule_flags();
        let respawn_ready = state.can_respawn_from_death_screen();
        if respawn_ready {
            self.put_str(
                panel_x + ((panel_w - 24) / 2),
                panel_y + 5,
                "Press R / Enter / Space",
                Color::Rgb {
                    r: 220,
                    g: 220,
                    b: 220,
                },
                panel_bg,
            );
            self.put_str(
                panel_x + ((panel_w - 10) / 2),
                panel_y + 6,
                "to respawn",
                Color::Rgb {
                    r: 180,
                    g: 240,
                    b: 180,
                },
                panel_bg,
            );
        } else {
            let ticks_left = state.death_respawn_ticks_remaining();
            let secs_left = ticks_left as f32 / 20.0;
            self.put_str(
                panel_x + ((panel_w - 25) / 2),
                panel_y + 5,
                "Respawn available in...",
                Color::Rgb {
                    r: 220,
                    g: 220,
                    b: 220,
                },
                panel_bg,
            );
            self.put_str(
                panel_x + ((panel_w - 8) / 2),
                panel_y + 6,
                &format!("{secs_left:>3.1}s"),
                Color::Rgb {
                    r: 180,
                    g: 240,
                    b: 180,
                },
                panel_bg,
            );
        }
        self.put_str(
            panel_x + ((panel_w - 18) / 2),
            panel_y + 8,
            "Press Q / Esc to quit",
            Color::DarkGrey,
            panel_bg,
        );
        self.put_str(
            panel_x + 2,
            panel_y + panel_h - 3,
            &format!(
                "Keep inventory: {}",
                if rule_keep_inv { "ON" } else { "OFF" }
            ),
            Color::DarkGrey,
            panel_bg,
        );
        let last_health = state.player.health.max(0.0);
        self.put_str(
            panel_x + 2,
            panel_y + panel_h - 2,
            &format!("Last health: {last_health:.1}"),
            Color::DarkGrey,
            panel_bg,
        );
    }

    fn draw_settings_overlay(&mut self, state: &GameState) {
        for cell in &mut self.back_buffer {
            if matches!(cell.bg, Color::Reset) {
                cell.bg = Color::AnsiValue(236);
            }
        }

        let panel_w = (self.width as i32 - 18).clamp(54, 82);
        let panel_h = (self.height as i32 - 8).clamp(16, 22);
        let panel_x = ((self.width as i32 - panel_w) / 2).max(0);
        let panel_y = ((self.height as i32 - panel_h) / 2).max(0);
        let panel_bg = Color::AnsiValue(235);

        for y in panel_y..(panel_y + panel_h) {
            for x in panel_x..(panel_x + panel_w) {
                self.put_char(x, y, ' ', Color::White, panel_bg);
            }
        }

        self.put_char(panel_x, panel_y, '+', Color::White, panel_bg);
        self.put_char(panel_x + panel_w - 1, panel_y, '+', Color::White, panel_bg);
        self.put_char(panel_x, panel_y + panel_h - 1, '+', Color::White, panel_bg);
        self.put_char(
            panel_x + panel_w - 1,
            panel_y + panel_h - 1,
            '+',
            Color::White,
            panel_bg,
        );
        for x in (panel_x + 1)..(panel_x + panel_w - 1) {
            self.put_char(x, panel_y, '-', Color::White, panel_bg);
            self.put_char(x, panel_y + panel_h - 1, '-', Color::White, panel_bg);
        }
        for y in (panel_y + 1)..(panel_y + panel_h - 1) {
            self.put_char(panel_x, y, '|', Color::White, panel_bg);
            self.put_char(panel_x + panel_w - 1, y, '|', Color::White, panel_bg);
        }

        let title = "SETTINGS";
        self.draw_termcraft_wordmark(
            panel_x + (panel_w / 2),
            panel_y + 1,
            Color::Rgb {
                r: 185,
                g: 220,
                b: 255,
            },
            panel_bg,
        );
        self.put_str(
            panel_x + ((panel_w - title.len() as i32) / 2),
            panel_y + 3,
            title,
            Color::Rgb {
                r: 185,
                g: 220,
                b: 255,
            },
            panel_bg,
        );
        self.put_str(
            panel_x + 2,
            panel_y + 4,
            "Arrows/W/S: Move  Enter/Space: Apply  O/Esc: Close",
            Color::DarkGrey,
            panel_bg,
        );

        let (rule_spawn, rule_day, rule_weather, rule_keep_inv) = state.game_rule_flags();
        let difficulty_name = state.difficulty_name();
        let game_rules_preset_name = state.game_rules_preset_name();
        let rows = [
            format!("Difficulty        : {difficulty_name}"),
            format!("Gamerule preset   : {game_rules_preset_name}"),
            format!(
                "doMobSpawning     : {}",
                if rule_spawn { "ON" } else { "OFF" }
            ),
            format!(
                "doDaylightCycle   : {}",
                if rule_day { "ON" } else { "OFF" }
            ),
            format!(
                "doWeatherCycle    : {}",
                if rule_weather { "ON" } else { "OFF" }
            ),
            format!(
                "keepInventory     : {}",
                if rule_keep_inv { "ON" } else { "OFF" }
            ),
            "Close settings menu".to_string(),
        ];

        let selected = state.settings_menu_selected_index() as usize;
        let line_width = (panel_w - 6).max(8) as usize;
        let text_top = panel_y + 6;
        for (idx, row) in rows.iter().enumerate() {
            let y = text_top + idx as i32;
            if y >= panel_y + panel_h - 2 {
                break;
            }
            let is_selected = idx == selected;
            let fg = if is_selected {
                Color::Black
            } else {
                Color::Rgb {
                    r: 220,
                    g: 220,
                    b: 220,
                }
            };
            let bg = if is_selected {
                Color::Rgb {
                    r: 210,
                    g: 210,
                    b: 120,
                }
            } else {
                panel_bg
            };
            let prefix = if is_selected { ">" } else { " " };
            let mut line = format!("{prefix} {row}");
            if line.len() > line_width {
                line.truncate(line_width);
            }
            self.put_str(panel_x + 3, y, &format!("{line:<line_width$}"), fg, bg);
        }
    }

    fn termcraft_logo_lines() -> &'static [&'static str] {
        &[
            " _____ _____ ____  __  __  ____ ____   _    _____ _____ ",
            "|_   _| ____|  _ \\|  \\/  |/ ___|  _ \\ / \\  |  ___|_   _|",
            "  | | |  _| | |_) | |\\/| | |   | |_) / _ \\ | |_    | |  ",
            "  | | | |___|  _ <| |  | | |___|  _ </ ___ \\|  _|   | |  ",
            "  |_| |_____|_| \\_\\_|  |_|\\____|_| \\_/_/   \\_\\_|     |_|  ",
        ]
    }

    fn draw_termcraft_logo(
        &mut self,
        center_x: i32,
        top_y: i32,
        fg: Color,
        shadow_fg: Color,
        bg: Color,
    ) -> i32 {
        let lines = Self::termcraft_logo_lines();
        for (row, line) in lines.iter().enumerate() {
            let x = center_x - (line.len() as i32 / 2);
            let y = top_y + row as i32;
            self.put_str(x + 1, y + 1, line, shadow_fg, bg);
            self.put_str(x, y, line, fg, bg);
        }
        lines.len() as i32
    }

    fn draw_termcraft_wordmark(&mut self, center_x: i32, y: i32, fg: Color, bg: Color) {
        let wordmark = "= termcraft =";
        let x = center_x - (wordmark.len() as i32 / 2);
        self.put_str(x + 1, y + 1, wordmark, Color::AnsiValue(233), bg);
        self.put_str(x, y, wordmark, fg, bg);
    }

    fn draw_startup_splash_overlay(&mut self, _state: &GameState) {
        for cell in &mut self.back_buffer {
            cell.bg = match cell.bg {
                Color::Reset => Color::AnsiValue(233),
                bg => Self::dim_color(bg, 0.6),
            };
            if cell.fg != Color::Reset {
                cell.fg = Self::dim_color(cell.fg, 0.85);
            }
        }

        let panel_w = (self.width as i32 - 10).clamp(58, 88);
        let panel_h = (self.height as i32 - 6).clamp(14, 20);
        let panel_x = ((self.width as i32 - panel_w) / 2).max(0);
        let panel_y = ((self.height as i32 - panel_h) / 2).max(0);
        let panel_bg = Color::AnsiValue(235);

        for y in panel_y..(panel_y + panel_h) {
            for x in panel_x..(panel_x + panel_w) {
                self.put_char(x, y, ' ', Color::White, panel_bg);
            }
        }

        self.put_char(panel_x, panel_y, '+', Color::White, panel_bg);
        self.put_char(panel_x + panel_w - 1, panel_y, '+', Color::White, panel_bg);
        self.put_char(panel_x, panel_y + panel_h - 1, '+', Color::White, panel_bg);
        self.put_char(
            panel_x + panel_w - 1,
            panel_y + panel_h - 1,
            '+',
            Color::White,
            panel_bg,
        );
        for x in (panel_x + 1)..(panel_x + panel_w - 1) {
            self.put_char(x, panel_y, '-', Color::White, panel_bg);
            self.put_char(x, panel_y + panel_h - 1, '-', Color::White, panel_bg);
        }
        for y in (panel_y + 1)..(panel_y + panel_h - 1) {
            self.put_char(panel_x, y, '|', Color::White, panel_bg);
            self.put_char(panel_x + panel_w - 1, y, '|', Color::White, panel_bg);
        }

        let logo_h = self.draw_termcraft_logo(
            panel_x + (panel_w / 2),
            panel_y + 2,
            Color::Rgb {
                r: 222,
                g: 232,
                b: 255,
            },
            Color::AnsiValue(233),
            panel_bg,
        );
        self.put_str(
            panel_x + ((panel_w - 34) / 2),
            panel_y + logo_h + 4,
            "terminal-first 2D sandbox survival",
            Color::Rgb {
                r: 208,
                g: 192,
                b: 146,
            },
            panel_bg,
        );
        self.put_str(
            panel_x + ((panel_w - 59) / 2),
            panel_y + logo_h + 5,
            "Early alpha build  |  classic progression, terminal adaptation",
            Color::DarkGrey,
            panel_bg,
        );

        let prompt = "Press any key to start";
        self.put_str(
            panel_x + ((panel_w - prompt.len() as i32) / 2),
            panel_y + panel_h - 3,
            prompt,
            Color::Rgb {
                r: 255,
                g: 224,
                b: 120,
            },
            panel_bg,
        );
    }

    fn compact_item_label(name: &str) -> String {
        let words: Vec<&str> = name
            .split_whitespace()
            .filter(|word| !word.is_empty())
            .collect();
        if words.len() >= 2 {
            let first = words[0].chars().next().unwrap_or('?').to_ascii_uppercase();
            let second = words[1].chars().next().unwrap_or('?').to_ascii_uppercase();
            return format!("{first}{second}");
        }

        let mut letters = words
            .first()
            .copied()
            .unwrap_or("?")
            .chars()
            .filter(|ch| ch.is_ascii_alphanumeric());
        let first = letters.next().unwrap_or('?').to_ascii_uppercase();
        let second = letters.next().unwrap_or(' ').to_ascii_lowercase();
        format!("{first}{second}")
    }

    fn hotbar_item_glyph(item_type: ItemType) -> &'static str {
        match item_type {
            ItemType::WoodPickaxe
            | ItemType::StonePickaxe
            | ItemType::IronPickaxe
            | ItemType::DiamondPickaxe
            | ItemType::WoodAxe
            | ItemType::StoneAxe
            | ItemType::IronAxe
            | ItemType::DiamondAxe
            | ItemType::WoodShovel
            | ItemType::StoneShovel
            | ItemType::IronShovel
            | ItemType::DiamondShovel => "_/\\_",
            ItemType::Stick => " /  ",
            ItemType::RedstoneDust => " .. ",
            ItemType::RedstoneTorch => " !  ",
            ItemType::Lever => " /  ",
            ItemType::StoneButton => " () ",
            ItemType::Bucket => "\\_/ ",
            ItemType::Tnt => " TNT",
            ItemType::Piston => "[>] ",
            ItemType::RedstoneRepeater => " >| ",
            ItemType::Netherrack => " n  ",
            ItemType::SoulSand => " s  ",
            ItemType::Glowstone => " *  ",
            ItemType::EndStone => " e  ",
            ItemType::EyeOfEnder => " oo ",
            ItemType::EnderPearl => " @  ",
            ItemType::BlazeRod => " /| ",
            ItemType::BlazePowder => " :: ",
            ItemType::Slimeball => " oo ",
            ItemType::Bed => " H  ",
            ItemType::Chest => " C  ",
            ItemType::Glass => "[ ] ",
            ItemType::WoodDoor => " |  ",
            ItemType::Ladder => " #  ",
            ItemType::StoneSlab => " == ",
            ItemType::StoneStairs => " ^^ ",
            ItemType::Sapling | ItemType::BirchSapling => " i  ",
            _ => " ██ ",
        }
    }

    fn inventory_item_label(item_type: ItemType) -> String {
        match item_type {
            ItemType::Boat => "Bt".to_string(),
            ItemType::Bucket => "Bk".to_string(),
            ItemType::WaterBucket => "Wk".to_string(),
            ItemType::LavaBucket => "Lk".to_string(),
            ItemType::WaterBottle => "WB".to_string(),
            ItemType::GlassBottle => "GB".to_string(),
            ItemType::Bow => "Bw".to_string(),
            ItemType::Bone => "Bn".to_string(),
            ItemType::BoneMeal => "BM".to_string(),
            ItemType::Flint => "Fl".to_string(),
            ItemType::FlintAndSteel => "FS".to_string(),
            ItemType::Shears => "Sh".to_string(),
            ItemType::Stick => "Sk".to_string(),
            ItemType::Leaves => "Lf".to_string(),
            ItemType::Sapling => "Sp".to_string(),
            ItemType::BirchSapling => "BS".to_string(),
            ItemType::SugarCane => "SC".to_string(),
            ItemType::Paper => "Pa".to_string(),
            ItemType::Book => "Bk".to_string(),
            ItemType::Bookshelf => "Bh".to_string(),
            ItemType::Lever => "Lv".to_string(),
            ItemType::Wool => "Wl".to_string(),
            ItemType::Bed => "Bd".to_string(),
            ItemType::WoodDoor => "Dr".to_string(),
            ItemType::Ladder => "Ld".to_string(),
            ItemType::StoneSlab => "Sl".to_string(),
            ItemType::StoneStairs => "Sr".to_string(),
            ItemType::EyeOfEnder => "EE".to_string(),
            ItemType::EnderPearl => "EP".to_string(),
            ItemType::NetherWart => "NW".to_string(),
            ItemType::FishingRod => "FR".to_string(),
            ItemType::PotionHealing => "PH".to_string(),
            ItemType::PotionStrength => "PS".to_string(),
            ItemType::PotionRegeneration => "PR".to_string(),
            ItemType::PotionFireResistance => "PF".to_string(),
            ItemType::AwkwardPotion => "AP".to_string(),
            ItemType::RedFlower => "RF".to_string(),
            ItemType::YellowFlower => "YF".to_string(),
            ItemType::RedstoneDust => "RD".to_string(),
            ItemType::RedstoneTorch => "RT".to_string(),
            ItemType::RedstoneRepeater => "RR".to_string(),
            ItemType::CraftingTable => "CT".to_string(),
            ItemType::BrewingStand => "BS".to_string(),
            ItemType::EnchantingTable => "ET".to_string(),
            ItemType::EndStone => "ES".to_string(),
            ItemType::SoulSand => "SS".to_string(),
            ItemType::Tnt => "TN".to_string(),
            _ => Self::compact_item_label(item_type.name()),
        }
    }

    fn ui_warm_material_color(item_type: ItemType) -> Option<Color> {
        let rgb = match item_type {
            ItemType::Dirt => (166, 114, 60),
            ItemType::Grass => (84, 188, 80),
            ItemType::Wood => (152, 106, 60),
            ItemType::BirchWood => (220, 210, 176),
            ItemType::Planks => (186, 142, 82),
            ItemType::Boat => (184, 136, 76),
            ItemType::CraftingTable => (182, 132, 76),
            ItemType::Chest => (176, 132, 70),
            ItemType::Bookshelf => (170, 118, 70),
            ItemType::WoodDoor => (166, 122, 74),
            ItemType::Ladder => (172, 130, 80),
            _ => return None,
        };
        Some(Self::rgb(rgb))
    }

    fn inventory_item_color(item_type: ItemType) -> Color {
        if let Some(color) = Self::ui_warm_material_color(item_type) {
            return color;
        }

        match item_type {
            ItemType::Grass
            | ItemType::Leaves
            | ItemType::BirchLeaves
            | ItemType::Sapling
            | ItemType::BirchSapling
            | ItemType::TallGrass
            | ItemType::Cactus
            | ItemType::SugarCane
            | ItemType::WheatSeeds
            | ItemType::Wheat
            | ItemType::Slimeball => Color::Rgb {
                r: 95,
                g: 190,
                b: 90,
            },
            ItemType::RedFlower => Color::Rgb {
                r: 220,
                g: 70,
                b: 70,
            },
            ItemType::YellowFlower
            | ItemType::Bread
            | ItemType::BlazePowder
            | ItemType::Glowstone
            | ItemType::GoldIngot
            | ItemType::Torch => Color::Rgb {
                r: 235,
                g: 205,
                b: 80,
            },
            ItemType::Stick
            | ItemType::WoodPickaxe
            | ItemType::WoodAxe
            | ItemType::WoodShovel
            | ItemType::WoodSword
            | ItemType::WoodHoe
            | ItemType::Leather
            | ItemType::LeatherHelmet
            | ItemType::LeatherChestplate
            | ItemType::LeatherLeggings
            | ItemType::LeatherBoots
            | ItemType::Bow
            | ItemType::FishingRod => Color::Rgb {
                r: 160,
                g: 115,
                b: 75,
            },
            ItemType::Stone
            | ItemType::Cobblestone
            | ItemType::StonePickaxe
            | ItemType::StoneAxe
            | ItemType::StoneShovel
            | ItemType::StoneSword
            | ItemType::StoneHoe
            | ItemType::StoneButton
            | ItemType::StoneSlab
            | ItemType::StoneStairs
            | ItemType::Furnace
            | ItemType::Anvil
            | ItemType::Gravel
            | ItemType::Flint
            | ItemType::Shears => Color::Grey,
            ItemType::IronIngot
            | ItemType::RawIron
            | ItemType::IronPickaxe
            | ItemType::IronAxe
            | ItemType::IronShovel
            | ItemType::IronSword
            | ItemType::IronHoe
            | ItemType::IronHelmet
            | ItemType::IronChestplate
            | ItemType::IronLeggings
            | ItemType::IronBoots
            | ItemType::Bucket
            | ItemType::GlassBottle
            | ItemType::BoneMeal => Color::White,
            ItemType::Diamond
            | ItemType::DiamondPickaxe
            | ItemType::DiamondAxe
            | ItemType::DiamondShovel
            | ItemType::DiamondSword
            | ItemType::DiamondHoe
            | ItemType::DiamondHelmet
            | ItemType::DiamondChestplate
            | ItemType::DiamondLeggings
            | ItemType::DiamondBoots => Color::Cyan,
            ItemType::RedstoneDust
            | ItemType::RedstoneTorch
            | ItemType::RedstoneRepeater
            | ItemType::Tnt
            | ItemType::RawBeef
            | ItemType::RawPorkchop
            | ItemType::RawChicken
            | ItemType::RawMutton => Color::Red,
            ItemType::WaterBucket | ItemType::WaterBottle | ItemType::PotionRegeneration => {
                Color::Blue
            }
            ItemType::LavaBucket | ItemType::PotionHealing => Color::DarkRed,
            ItemType::PotionStrength
            | ItemType::PotionFireResistance
            | ItemType::MagmaCream
            | ItemType::BlazeRod => Color::Rgb {
                r: 245,
                g: 165,
                b: 70,
            },
            ItemType::Netherrack | ItemType::NetherWart | ItemType::Gunpowder => Color::DarkRed,
            ItemType::SoulSand => Color::DarkYellow,
            ItemType::EndStone => Color::Rgb {
                r: 215,
                g: 208,
                b: 150,
            },
            ItemType::EyeOfEnder | ItemType::EnderPearl => Color::Rgb {
                r: 95,
                g: 225,
                b: 150,
            },
            ItemType::Obsidian => Color::Rgb {
                r: 60,
                g: 40,
                b: 75,
            },
            ItemType::FlintAndSteel => Color::Rgb {
                r: 210,
                g: 210,
                b: 220,
            },
            ItemType::Paper => Color::Rgb {
                r: 230,
                g: 226,
                b: 208,
            },
            ItemType::Book => Color::Rgb {
                r: 140,
                g: 92,
                b: 54,
            },
            ItemType::Snow | ItemType::Ice | ItemType::Glass | ItemType::Feather => Color::Rgb {
                r: 205,
                g: 225,
                b: 240,
            },
            _ => Color::White,
        }
    }

    fn hotbar_slot_background(stack: Option<&ItemStack>, selected: bool) -> Color {
        let base = if selected {
            (104, 88, 52)
        } else {
            (52, 56, 64)
        };
        let Some(stack) = stack else {
            return Self::rgb(base);
        };
        let Some(accent) = Self::color_rgb(Self::inventory_item_color(stack.item_type)) else {
            return Self::rgb(base);
        };
        let tint = if selected { 0.34 } else { 0.18 };
        Self::rgb(Self::lerp_rgb(base, accent, tint))
    }

    fn hotbar_slot_text_color(bg: Color, selected: bool) -> Color {
        match Self::color_rgb(bg) {
            Some(rgb) if Self::rgb_luma(rgb) > 150.0 => Color::Black,
            _ if selected => Color::Rgb {
                r: 255,
                g: 245,
                b: 210,
            },
            _ => Color::Rgb {
                r: 232,
                g: 232,
                b: 236,
            },
        }
    }

    fn hotbar_stack_metric(stack: &ItemStack) -> (String, Color) {
        if let (Some(d), Some(m)) = (stack.durability, stack.item_type.max_durability()) {
            let pct = ((d as f32 / m as f32) * 100.0).round() as i32;
            let fg = if pct >= 66 {
                Color::Rgb {
                    r: 104,
                    g: 218,
                    b: 108,
                }
            } else if pct >= 33 {
                Color::Rgb {
                    r: 242,
                    g: 212,
                    b: 104,
                }
            } else {
                Color::Rgb {
                    r: 236,
                    g: 116,
                    b: 96,
                }
            };
            return (format!("{pct:>3}%"), fg);
        }

        if stack.count > 1 {
            let stack_count = stack.count.min(99);
            return (format!("x{stack_count:>2}"), Color::Cyan);
        }

        ("".to_string(), Color::DarkGrey)
    }

    fn hotbar_selected_detail(stack: &ItemStack, enchant_level: u8) -> String {
        let item_name = stack.item_type.name();
        let stack_count = stack.count;
        let mut detail = format!("Held: {item_name} x{stack_count}");
        if let (Some(d), Some(m)) = (stack.durability, stack.item_type.max_durability()) {
            let durability_pct = ((d as f32 / m as f32) * 100.0) as i32;
            detail.push_str(&format!(" | Dur {durability_pct}%"));
        }
        if enchant_level > 0 {
            detail.push_str(&format!(" | Ench +{enchant_level}"));
        }
        detail
    }

    fn draw_item_icon(&mut self, sx: i32, sy: i32, item: ItemType, background: Color) {
        for y in 0..4 {
            for x in 0..4 {
                let token = items::token(item, x * 2, y * 2);
                let color = items::color(item, token).unwrap_or(background);
                self.put_ui_pixel(sx + x as i32, sy * 2 + y as i32, color);
            }
        }
    }

    fn draw_inventory_stack(
        &mut self,
        sx: i32,
        sy: i32,
        stack: &ItemStack,
        is_selected: bool,
        slot_bg: Color,
    ) {
        if self.pixel_colors {
            self.put_str(sx, sy + 1, "      ", Color::Reset, slot_bg);
            // Counts share the corner as in the hotbar; the selected item's full
            // name, durability and enchantment remain in the detail row.
            self.draw_item_icon(sx + 1, sy, stack.item_type, slot_bg);
            if stack.count > 1 {
                self.put_str(
                    sx + 3,
                    sy + 1,
                    &format!("{:>2}", stack.count.min(99)),
                    if is_selected {
                        Color::Black
                    } else {
                        Color::White
                    },
                    slot_bg,
                );
            }
            return;
        }
        let icon = Self::inventory_item_label(stack.item_type);
        let icon_fg = if is_selected {
            Color::Black
        } else {
            Self::inventory_item_color(stack.item_type)
        };
        self.put_str(sx + 1, sy, &icon, icon_fg, slot_bg);
        let stack_count = stack.count.min(99);
        self.put_str(
            sx + 3,
            sy,
            &format!("{stack_count:>2}"),
            if is_selected {
                Color::Black
            } else {
                Color::Cyan
            },
            slot_bg,
        );
    }

    fn draw_selected_inventory_detail(
        &mut self,
        state: &GameState,
        x: i32,
        y: i32,
        width: usize,
        bg: Color,
    ) {
        let (fg, mut line) = if let Some(stack) = state.selected_inventory_preview_item() {
            let item_name = stack.item_type.name();
            let stack_count = stack.count;
            (Color::Cyan, format!("Selected: {item_name} x{stack_count}"))
        } else {
            (
                Color::DarkGrey,
                "Selected: none. Click a slot to inspect it.".to_string(),
            )
        };
        if line.len() > width {
            line.truncate(width);
        }
        self.put_str(x, y, &format!("{line:<width$}"), fg, bg);
    }

    fn draw_inventory_overlay(&mut self, state: &GameState) {
        let box_w = 60;
        let box_h = 24;
        let box_x = (self.width as i32 - box_w) / 2;
        let box_y = (self.height as i32 - box_h) / 2;
        let bg = Color::AnsiValue(235);
        const CHEST_UI_OFFSET: usize = 27;

        for y in 0..box_h {
            for x in 0..box_w {
                self.put_char(box_x + x, box_y + y, ' ', Color::Reset, bg);
            }
        }

        let title = if state.at_chest {
            "CHEST"
        } else if state.at_furnace {
            "FURNACE (SMELTING)"
        } else if state.at_enchanting_table {
            "ENCHANTING TABLE"
        } else if state.at_anvil {
            "ANVIL"
        } else if state.at_brewing_stand {
            "BREWING STAND"
        } else if state.at_crafting_table {
            "CRAFTING TABLE (3x3)"
        } else {
            "INVENTORY & CRAFTING (2x2)"
        };
        self.put_str(box_x + 2, box_y + 1, title, Color::Yellow, bg);

        if state.at_chest {
            self.put_str(box_x + 2, box_y + 3, "CHEST SLOTS", Color::Yellow, bg);
            for i in 0..27 {
                let row = i / 9;
                let col = i % 9;
                let sx = box_x + 2 + (col as i32 * 6);
                let sy = box_y + 4 + (row as i32 * 2);
                let ui_slot = i;
                let is_selected = state.selected_inventory_slot == Some(ui_slot);
                let slot_bg = if is_selected { Color::Cyan } else { bg };
                let bracket_fg = if is_selected {
                    Color::Black
                } else {
                    Color::White
                };
                self.put_str(sx, sy, "[    ]", bracket_fg, slot_bg);
                if let Some(stack) = state.chest_slot_item(i) {
                    self.draw_inventory_stack(sx, sy, stack, is_selected, slot_bg);
                }
            }
            self.put_str(box_x + 2, box_y + 11, "PLAYER INVENTORY", Color::Yellow, bg);
            for i in 0..27 {
                let row = i / 9;
                let col = i % 9;
                let sx = box_x + 2 + (col as i32 * 6);
                let sy = box_y + 12 + (row as i32 * 2);
                let ui_slot = CHEST_UI_OFFSET + i;
                let is_selected = state.selected_inventory_slot == Some(ui_slot);
                let slot_bg = if is_selected {
                    Color::Cyan
                } else if i < 9 && state.hotbar_index == i as u8 {
                    Color::AnsiValue(238)
                } else {
                    bg
                };
                let bracket_fg = if is_selected {
                    Color::Black
                } else {
                    Color::White
                };
                self.put_str(sx, sy, "[    ]", bracket_fg, slot_bg);
                if let Some(stack) = &state.inventory.slots[i] {
                    self.draw_inventory_stack(sx, sy, stack, is_selected, slot_bg);
                }
            }
            self.put_str(
                box_x + 2,
                box_y + 20,
                "L-click swap. Shift+L quick-move. R-click split/place 1.",
                Self::rgb((172, 185, 195)),
                bg,
            );
            self.draw_selected_inventory_detail(state, box_x + 2, box_y + 22, 56, bg);
            return;
        }

        for i in 0..27 {
            let row = i / 9;
            let col = i % 9;
            let sx = box_x + 2 + (col as i32 * 6);
            let sy = box_y + 3 + (row as i32 * 2);

            let is_selected = state.selected_inventory_slot == Some(i);
            let slot_bg = if is_selected {
                Color::Cyan
            } else if i < 9 && state.hotbar_index == i as u8 {
                Color::AnsiValue(238)
            } else {
                bg
            };
            let bracket_fg = if is_selected {
                Color::Black
            } else {
                Color::White
            };

            self.put_str(sx, sy, "[    ]", bracket_fg, slot_bg);
            if let Some(stack) = &state.inventory.slots[i] {
                self.draw_inventory_stack(sx, sy, stack, is_selected, slot_bg);
            }
        }

        if state.has_personal_armor_ui() {
            const ARMOR_SLOT_LABELS: [&str; 4] = ["H", "C", "L", "B"];

            self.put_str(box_x + 2, box_y + 10, "ARMOR", Color::Yellow, bg);
            for (armor_slot_idx, slot_label) in ARMOR_SLOT_LABELS.iter().enumerate() {
                let sx = box_x + 2;
                let sy = box_y + 11 + (armor_slot_idx as i32 * 2);
                let ui_slot = ARMOR_UI_OFFSET + armor_slot_idx;
                let is_selected = state.selected_inventory_slot == Some(ui_slot);
                let slot_bg = if is_selected { Color::Cyan } else { bg };
                let bracket_fg = if is_selected {
                    Color::Black
                } else {
                    Color::White
                };
                self.put_str(sx, sy, slot_label, Color::DarkGrey, bg);
                self.put_str(sx + 2, sy, "[    ]", bracket_fg, slot_bg);
                if let Some(stack) = state.armor_slot_item(armor_slot_idx) {
                    self.draw_inventory_stack(sx + 2, sy, stack, is_selected, slot_bg);
                }
            }
        }

        if state.at_enchanting_table {
            self.put_str(
                box_x + 2,
                box_y + 10,
                "ENCHANT OPTIONS (CLICK TO APPLY)",
                Color::Yellow,
                bg,
            );
            self.put_str(
                box_x + 2,
                box_y + 11,
                &state.enchanting_status_line(),
                Color::Cyan,
                bg,
            );
            for option_idx in 0..3 {
                let cost = state.enchant_option_cost(option_idx).unwrap_or(0);
                let fg = if state.can_apply_enchant_option(option_idx) {
                    Color::Green
                } else {
                    Color::DarkGrey
                };
                let desc = match option_idx {
                    0 => "Minor Enchant + Repair",
                    1 => "Standard Enchant + Repair",
                    _ => "Major Enchant + Repair",
                };
                let option_number = option_idx + 1;
                self.put_str(
                    box_x + 2,
                    box_y + 13 + option_idx as i32,
                    &format!("{option_number}. Cost {cost} levels -> {desc}"),
                    fg,
                    bg,
                );
            }
        } else if state.at_anvil {
            self.put_str(box_x + 2, box_y + 10, "ANVIL ACTIONS", Color::Yellow, bg);
            self.put_str(
                box_x + 2,
                box_y + 11,
                &state.anvil_status_line(),
                Color::Cyan,
                bg,
            );
            let fg = if state.can_apply_anvil_combine() {
                Color::Green
            } else {
                Color::DarkGrey
            };
            self.put_str(
                box_x + 2,
                box_y + 13,
                "1. Combine with matching item (+repair/+enchant)",
                fg,
                bg,
            );
        } else if state.at_brewing_stand {
            self.put_str(box_x + 2, box_y + 10, "BREWING OPTIONS", Color::Yellow, bg);
            self.put_str(
                box_x + 2,
                box_y + 11,
                &state.brewing_status_line(),
                Color::Cyan,
                bg,
            );
            for option_idx in 0..5 {
                let fg = if state.can_apply_brew_option(option_idx) {
                    Color::Green
                } else {
                    Color::DarkGrey
                };
                let desc = match option_idx {
                    0 => "Water Bottle + Nether Wart -> Awkward Potion",
                    1 => "Awkward Potion + Red Flower -> Potion of Healing",
                    2 => "Awkward Potion + Blaze Powder -> Potion of Strength",
                    3 => "Awkward Potion + Ghast Tear -> Potion of Regeneration",
                    _ => "Awkward Potion + Magma Cream -> Fire Resistance",
                };
                let option_number = option_idx + 1;
                self.put_str(
                    box_x + 2,
                    box_y + 13 + option_idx as i32,
                    &format!("{option_number}. {desc}"),
                    fg,
                    bg,
                );
            }
        } else if state.at_furnace {
            self.put_str(
                box_x + 2,
                box_y + 10,
                "AVAILABLE SMELTS (CLICK TO START)",
                Color::Yellow,
                bg,
            );
            self.put_str(
                box_x + 2,
                box_y + 11,
                &state.furnace_status_line(),
                Color::Cyan,
                bg,
            );
            let recipe_start_y = 13;
            let all_recipes = Recipe::all();
            let mut visible_idx = 0;
            for (i, r) in all_recipes.iter().enumerate() {
                if !r.needs_furnace {
                    continue;
                }
                let rx = box_x + 2;
                let ry = box_y + recipe_start_y + visible_idx;
                let can_craft = state.can_start_furnace_recipe(i);
                let fg = if can_craft {
                    Color::Green
                } else {
                    Color::DarkGrey
                };
                let mut ing_str = String::new();
                for (ing, amt) in r.ingredient_requirements() {
                    let ingredient_name = ing.name();
                    ing_str.push_str(&format!("{ingredient_name} x{amt}, "));
                }
                let recipe_number = i + 1;
                let result_name = r.result.name();
                let result_count = r.result_count;
                self.put_str(
                    rx,
                    ry,
                    &format!("{recipe_number}. {result_name} x{result_count} <- {ing_str}"),
                    fg,
                    bg,
                );
                visible_idx += 1;
            }
            self.put_str(
                box_x + 2,
                box_y + 20,
                "Shift+L on smeltable item/coal to quick-start furnace job.",
                Self::rgb((172, 185, 195)),
                bg,
            );
        } else {
            let grid_size = if state.at_crafting_table { 3 } else { 2 };
            let grid_x = box_x + 30;
            let grid_y = box_y + 11;
            let craft_info_x = if state.has_personal_armor_ui() {
                box_x + 14
            } else {
                box_x + 2
            };
            self.put_str(craft_info_x, box_y + 10, "CRAFTING GRID", Color::Yellow, bg);
            self.put_str(
                craft_info_x,
                box_y + 18,
                if state.at_crafting_table {
                    "3x3 table; shapes matter"
                } else {
                    "2x2 inventory; shapes matter"
                },
                Color::Cyan,
                bg,
            );
            for y in 0..grid_size {
                for x in 0..grid_size {
                    let sx = grid_x + (x as i32 * 6);
                    let sy = grid_y + (y as i32 * 2);
                    let cell_idx = y * 3 + x;
                    let ui_slot = CRAFT_GRID_UI_OFFSET + cell_idx;
                    let is_selected = state.selected_inventory_slot == Some(ui_slot);
                    let slot_bg = if is_selected { Color::Cyan } else { bg };
                    let bracket_fg = if is_selected {
                        Color::Black
                    } else {
                        Color::White
                    };
                    self.put_str(sx, sy, "[    ]", bracket_fg, slot_bg);
                    if let Some(stack) = state.crafting_grid_slot_stack(cell_idx) {
                        self.draw_inventory_stack(sx, sy, stack, is_selected, slot_bg);
                    }
                }
            }

            let output_x = grid_x + (grid_size as i32 * 6) + 6;
            let output_y = grid_y + 2;
            self.put_str(output_x - 4, output_y, "=>", Color::Yellow, bg);
            let output_selected = state.selected_inventory_slot == Some(CRAFT_OUTPUT_UI_SLOT);
            let output_bg = if output_selected { Color::Cyan } else { bg };
            let output_fg = if output_selected {
                Color::Black
            } else {
                Color::White
            };
            self.put_str(output_x, output_y, "[    ]", output_fg, output_bg);
            if let Some((result, count)) = state.crafting_output_preview() {
                self.draw_inventory_stack(
                    output_x,
                    output_y,
                    &ItemStack {
                        item_type: result,
                        count,
                        durability: None,
                    },
                    output_selected,
                    output_bg,
                );
            }
            self.put_str(
                box_x + 2,
                box_y + 20,
                if state.world.is_replica() {
                    "L-click swap. R-click split. Shift+L quick-move."
                } else {
                    "L-click swap. L/R-drag spread 1. Shift+L output crafts max."
                },
                Self::rgb((172, 185, 195)),
                bg,
            );
            self.put_str(
                box_x + 2,
                box_y + 21,
                "Enter craft 1, Shift+Enter max, Del clears grid.",
                Self::rgb((172, 185, 195)),
                bg,
            );
        }
        self.draw_selected_inventory_detail(state, box_x + 2, box_y + 22, 56, bg);
    }

    pub fn get_inventory_click(
        &self,
        state: &GameState,
        screen_x: u16,
        screen_y: u16,
    ) -> Option<usize> {
        let box_w = 60;
        let box_h = 24;
        let box_x = (self.width as i32 - box_w) / 2;
        let box_y = (self.height as i32 - box_h) / 2;
        if (screen_x as i32) < box_x
            || (screen_x as i32) >= box_x + box_w
            || (screen_y as i32) < box_y
            || (screen_y as i32) >= box_y + box_h
        {
            return None;
        }
        let rel_x = screen_x as i32 - box_x;
        let rel_y = screen_y as i32 - box_y;
        if state.at_chest {
            if (4..10).contains(&rel_y) {
                let row = (rel_y - 4) / 2;
                let col = (rel_x - 2) / 6;
                if (0..9).contains(&col) {
                    return Some((row * 9 + col) as usize);
                }
            }
            if (12..18).contains(&rel_y) {
                let row = (rel_y - 12) / 2;
                let col = (rel_x - 2) / 6;
                if (0..9).contains(&col) {
                    return Some(27 + (row * 9 + col) as usize);
                }
            }
            return None;
        }
        if (3..9).contains(&rel_y) {
            let row = (rel_y - 3) / 2;
            let col = (rel_x - 2) / 6;
            if (0..9).contains(&col) {
                return Some((row * 9 + col) as usize);
            }
        }
        if state.has_personal_armor_ui() && (11..19).contains(&rel_y) {
            let row = (rel_y - 11) / 2;
            let col = rel_x - 4;
            if (0..4).contains(&row) && (0..6).contains(&col) {
                return Some(ARMOR_UI_OFFSET + row as usize);
            }
        }
        if !state.at_furnace
            && !state.at_enchanting_table
            && !state.at_anvil
            && !state.at_brewing_stand
        {
            let grid_size = if state.at_crafting_table { 3 } else { 2 };
            let grid_rel_x = rel_x - 30;
            let grid_rel_y = rel_y - 11;
            if grid_rel_x >= 0 && grid_rel_y >= 0 {
                let col = grid_rel_x / 6;
                let row = grid_rel_y / 2;
                if (0..grid_size).contains(&(col as usize))
                    && (0..grid_size).contains(&(row as usize))
                {
                    let cell_idx = (row as usize) * 3 + (col as usize);
                    return Some(CRAFT_GRID_UI_OFFSET + cell_idx);
                }
            }
            let output_rel_x = rel_x - (30 + (grid_size as i32 * 6) + 6);
            let output_rel_y = rel_y - (11 + 2);
            if (0..6).contains(&output_rel_x) && output_rel_y == 0 {
                return Some(CRAFT_OUTPUT_UI_SLOT);
            }
        }
        None
    }

    pub fn get_recipe_click(
        &self,
        state: &GameState,
        screen_x: u16,
        screen_y: u16,
    ) -> Option<usize> {
        if state.at_chest
            || state.at_enchanting_table
            || state.at_anvil
            || state.at_brewing_stand
            || !state.at_furnace
        {
            return None;
        }
        let box_w = 60;
        let box_h = 24;
        let box_x = (self.width as i32 - box_w) / 2;
        let box_y = (self.height as i32 - box_h) / 2;
        let rel_x = screen_x as i32 - box_x;
        let rel_y = screen_y as i32 - box_y;
        let recipe_start_y = 13;
        if (2..58).contains(&rel_x) && rel_y >= recipe_start_y {
            let clicked_row = (rel_y - recipe_start_y) as usize;
            let all_recipes = Recipe::all();
            let mut visible_idx = 0;
            for (i, r) in all_recipes.iter().enumerate() {
                if !r.needs_furnace {
                    continue;
                }
                if visible_idx == clicked_row {
                    return Some(i);
                }
                visible_idx += 1;
            }
        }
        None
    }

    pub fn get_enchant_option_click(
        &self,
        state: &GameState,
        screen_x: u16,
        screen_y: u16,
    ) -> Option<usize> {
        if !state.at_enchanting_table {
            return None;
        }
        let box_w = 60;
        let box_h = 24;
        let box_x = (self.width as i32 - box_w) / 2;
        let box_y = (self.height as i32 - box_h) / 2;
        let rel_x = screen_x as i32 - box_x;
        let rel_y = screen_y as i32 - box_y;
        if !(2..58).contains(&rel_x) {
            return None;
        }
        if (13..16).contains(&rel_y) {
            return Some((rel_y - 13) as usize);
        }
        None
    }

    pub fn get_anvil_action_click(&self, state: &GameState, screen_x: u16, screen_y: u16) -> bool {
        if !state.at_anvil {
            return false;
        }
        let box_w = 60;
        let box_h = 24;
        let box_x = (self.width as i32 - box_w) / 2;
        let box_y = (self.height as i32 - box_h) / 2;
        let rel_x = screen_x as i32 - box_x;
        let rel_y = screen_y as i32 - box_y;
        (2..58).contains(&rel_x) && rel_y == 13
    }

    pub fn get_brewing_option_click(
        &self,
        state: &GameState,
        screen_x: u16,
        screen_y: u16,
    ) -> Option<usize> {
        if !state.at_brewing_stand {
            return None;
        }
        let box_w = 60;
        let box_h = 24;
        let box_x = (self.width as i32 - box_w) / 2;
        let box_y = (self.height as i32 - box_h) / 2;
        let rel_x = screen_x as i32 - box_x;
        let rel_y = screen_y as i32 - box_y;
        if !(2..58).contains(&rel_x) {
            return None;
        }
        if (13..18).contains(&rel_y) {
            return Some((rel_y - 13) as usize);
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_view_has_context_and_cycles_wide_then_detailed() {
        let state = GameState::network_view();
        let mut renderer = Renderer::with_size(160, 48);
        let mut output = Vec::new();
        for expected_scale in [2, 1, 4, 2] {
            output.clear();
            renderer.render_to(&state, 0.0, &mut output).unwrap();
            assert_eq!(renderer.world_view.unwrap().4, expected_scale);
            assert!(output.starts_with(b"\x1b[?2026h"));
            assert!(output.ends_with(b"\x1b[?2026l"));
            renderer.toggle_zoom();
        }
    }

    #[test]
    fn surface_view_keeps_the_horizon_above_the_player_at_each_elevation() {
        let mut state = GameState::network_view();
        state.dismiss_startup_splash();
        for (width, height) in [(100, 32), (120, 40), (160, 48), (200, 60)] {
            for y in [18.0, 36.0, 110.0] {
                state.player.y = y;
                state.player.grounded = true;
                let mut r = Renderer::with_size(width, height);
                r.render_to(&state, 0.0, &mut Vec::new()).unwrap();
                let (_, top) = r.screen_to_world(&state, width / 2, 2);
                let (_, bottom) = r.screen_to_world(&state, width / 2, height - 6);
                assert!(
                    top <= y as i32 - 4,
                    "surface/sky hidden at {width}x{height}"
                );
                assert!(
                    bottom > y as i32,
                    "landing ground hidden at {width}x{height}"
                );
                assert!(
                    y as i32 - top > bottom - y as i32,
                    "view favors caves over sky"
                );
            }
        }
    }

    #[test]
    fn detailed_view_keeps_an_eight_block_roof_below_the_header() {
        let mut s = GameState::network_view();
        s.dismiss_startup_splash();
        s.player.y = 31.0;
        let mut r = Renderer::with_size(160, 48);
        r.render_to(&s, 0.0, &mut Vec::new()).unwrap();
        let (_, view_y, _, center_y, scale) = r.world_view.unwrap();
        let roof = (23 * scale) - view_y + center_y;
        let feet = (31 * scale) - view_y + center_y;
        assert!(roof >= 2, "roof hidden by header");
        assert!(feet < 43, "floor hidden by hotbar");
    }
    #[test]
    fn compact_trial_hud_keeps_goal_and_contextual_controls_visible() {
        let mut s = GameState::network_view();
        s.dismiss_startup_splash();
        s.trial = Some(
            serde_json::from_value(serde_json::json!({
                "version": 1, "stage": "build", "kills": 3
            }))
            .unwrap(),
        );
        s.player.x = 55.5;
        s.player.y = 64.0;
        let mut r = Renderer::with_size(80, 24);
        r.render_to(&s, 0.0, &mut Vec::new()).unwrap();
        let text: String = r.back_buffer.iter().map(|c| c.ch).collect();
        assert!(text.contains("Bridge gap + light signal"));
        assert!(text.contains("Select planks · R-click beside deck"));
        s.world.apply_chunk_column_snapshot(
            55 / crate::world::chunk::CHUNK_WIDTH as i32,
            &vec![
                BlockType::Air;
                crate::world::chunk::CHUNK_WIDTH * crate::world::chunk::CHUNK_HEIGHT
            ],
        );
        s.world.set_block(55, 63, BlockType::Ladder);
        r.render_to(&s, 0.0, &mut Vec::new()).unwrap();
        let text: String = r.back_buffer.iter().map(|c| c.ch).collect();
        assert!(text.contains("Hold W/Space to climb the ladder"));
    }

    #[test]
    fn camera_keeps_normal_jumps_steady_and_follow_is_frame_rate_independent() {
        let mut results = Vec::new();
        for fps in [30, 60, 120] {
            let mut renderer = Renderer::with_size(160, 48);
            renderer.frame_seconds = 1.0 / fps as f64;
            renderer.update_vertical_camera(20.0);
            for _ in 0..fps {
                renderer.update_vertical_camera(18.7);
            }
            assert_eq!(renderer.camera_y, 20.0);
            for _ in 0..fps {
                renderer.update_vertical_camera(26.0);
            }
            results.push(renderer.camera_y);
        }
        assert!(results.windows(2).all(|w| (w[0] - w[1]).abs() < 0.00001));
    }

    #[test]
    fn half_pixel_overlay_preserves_the_other_half_and_fine_view_picking() {
        let mut renderer = Renderer::with_size(160, 48);
        renderer.zoom = 2;
        renderer.put_pixel(2, 8, Color::Red);
        renderer.put_pixel(2, 9, Color::Blue);
        let cell = renderer.back_buffer[4 * 160 + 2];
        assert_eq!(cell.fg, Color::Red);
        assert_eq!(cell.bg, Color::Blue);
        let mut state = GameState::network_view();
        state.player.x = -0.25;
        state.player.y = 34.375;
        renderer.render_to(&state, 0.0, &mut Vec::new()).unwrap();
        let (vx, vy, cx, cy, scale) = renderer.world_view.unwrap();
        assert_eq!(scale, 4);
        assert_eq!(renderer.vertical_pixel_offset, 1);
        for dx in 0..scale * 2 {
            let x = (-scale * 2 - vx + cx + dx) as u16;
            let y = (33 * scale - vy + cy + scale / 2) as u16;
            assert_eq!(renderer.screen_to_world(&state, x, y), (-1, 33));
        }
    }

    #[test]
    fn rendered_hud_reports_mining_and_peer_identity_with_one_write() {
        struct Sink {
            writes: usize,
        }
        impl Write for Sink {
            fn write(&mut self, b: &[u8]) -> std::io::Result<usize> {
                self.writes += 1;
                Ok(b.len())
            }
            fn flush(&mut self) -> std::io::Result<()> {
                Ok(())
            }
        }
        let root =
            std::env::temp_dir().join(format!("termcraft-render-test-{}", rand::random::<u64>()));
        let mut state = GameState::open_in_directory(&root).unwrap();
        state.dismiss_startup_splash();
        state.player.x = 0.5;
        state.player.y = 64.0;
        state.mouse_x = 68;
        state.mouse_y = 26; // Block 63 in the roof-friendly detailed viewport.
        state.player.last_mine_x = 2;
        state.player.last_mine_y = 63;
        state.player.mining_timer = 0.45;
        // Reach/mining feedback must not depend on a random cave wall or a
        // spawn that left chunk zero unloaded.
        state.world.load_chunks_for_spawn_search(0, 8);
        for x in -8..=8 {
            for y in 55..70 {
                state.world.set_block(
                    x,
                    y,
                    if y >= 64 {
                        BlockType::Stone
                    } else {
                        BlockType::Air
                    },
                );
            }
        }
        state.world.set_block(2, 63, BlockType::Stone);
        state
            .remote_players
            .push(crate::engine::state::RemotePlayerState {
                client_id: 7,
                x: 3.0,
                y: 64.0,
                vx: 0.0,
                vy: 0.0,
                grounded: true,
                facing_right: true,
                sneaking: false,
                health: 20.0,
                max_health: 20.0,
                hunger: 20.0,
                max_hunger: 20.0,
            });
        let mut renderer = Renderer::with_size(120, 40);
        let mut sink = Sink { writes: 0 };
        renderer.render_to(&state, 0.0, &mut sink).unwrap();
        let row = |y: usize| {
            renderer.back_buffer[y * 120..(y + 1) * 120]
                .iter()
                .map(|c| c.ch)
                .collect::<String>()
        };
        assert!(row(10).contains("45%"));
        assert!(row(5).contains("Sprint"));
        assert!(
            renderer
                .back_buffer
                .iter()
                .map(|c| c.ch)
                .collect::<String>()
                .contains("P7")
        );
        assert_eq!(sink.writes, 1);
        assert_eq!(renderer.back_buffer[0].bg, Color::AnsiValue(234));
        drop(state);
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn zoom_target_mapping_and_jump_position_follow_the_rendered_camera() {
        let mut state = GameState::network_view();
        state.player.x = -0.25;
        state.player.y = 34.0;
        let mut renderer = Renderer::with_size(121, 41);
        renderer.render_to(&state, 0.0, &mut Vec::new()).unwrap();
        let (vx, vy, cx, cy, scale) = renderer.world_view.unwrap();
        assert_eq!(scale, 2);
        // Every physical cell in a scaled tile maps to that tile, including x < 0.
        for dy in 0..scale {
            for dx in 0..scale * 2 {
                let x = (-scale * 2 - vx + cx + dx) as u16;
                let y = (33 * scale - vy + cy + dy) as u16;
                assert_eq!(renderer.screen_to_world(&state, x, y), (-1, 33));
            }
        }
        let before = renderer
            .actor_sprites
            .iter()
            .find(|a| matches!(a.kind, SpriteKind::Player))
            .unwrap()
            .y;
        state.player.y = 32.8;
        state.player.vy = -0.3;
        renderer.render_to(&state, 0.0, &mut Vec::new()).unwrap();
        let after = renderer
            .actor_sprites
            .iter()
            .find(|a| matches!(a.kind, SpriteKind::Player))
            .unwrap()
            .y;
        assert!(after < before, "camera easing pinned the jumping player");
        assert_eq!((renderer.width, renderer.height), (121, 41));
    }

    #[test]
    fn sky_backdrop_occluder_keeps_transparent_surface_blocks_open() {
        assert!(!Renderer::sky_backdrop_occluder(BlockType::Leaves));
        assert!(!Renderer::sky_backdrop_occluder(BlockType::Glass));
        assert!(!Renderer::sky_backdrop_occluder(BlockType::Water(8)));
        assert!(Renderer::sky_backdrop_occluder(BlockType::Grass));
        assert!(Renderer::sky_backdrop_occluder(BlockType::Stone));
    }

    #[test]
    fn sky_horizon_anchor_ignores_tree_canopy_blocks() {
        assert!(!Renderer::sky_horizon_anchor(BlockType::Wood));
        assert!(!Renderer::sky_horizon_anchor(BlockType::BirchLeaves));
        assert!(Renderer::sky_horizon_anchor(BlockType::Grass));
        assert!(Renderer::sky_horizon_anchor(BlockType::Water(8)));
    }

    #[test]
    fn smooth_horizon_profile_filters_tree_spikes_and_rough_columns() {
        let smoothed = Renderer::smooth_horizon_profile(&[34, 33, 20, 32, 31, 35, 34, 33, 32]);
        assert_eq!(smoothed[2], 33);
        assert!((smoothed[4] - smoothed[5]).abs() <= 1);
    }

    #[test]
    fn overworld_depth_fog_stays_off_for_sky_and_surface_columns() {
        assert!(!Renderer::should_apply_overworld_depth_fog(20, 32));
        assert!(!Renderer::should_apply_overworld_depth_fog(32, 32));
        assert!(Renderer::should_apply_overworld_depth_fog(48, 32));
    }

    #[test]
    fn overworld_sky_background_uses_a_real_gradient() {
        let horizon = Renderer::overworld_sky_background(
            26,
            32,
            6000.0,
            PrecipitationType::None,
            0.0,
            0.0,
            false,
        );
        let zenith = Renderer::overworld_sky_background(
            4,
            32,
            6000.0,
            PrecipitationType::None,
            0.0,
            0.0,
            false,
        );
        let storm = Renderer::overworld_sky_background(
            10,
            32,
            6000.0,
            PrecipitationType::Rain,
            0.7,
            0.4,
            false,
        );

        assert!(matches!(horizon, Color::Rgb { .. }));
        assert!(matches!(zenith, Color::Rgb { .. }));
        assert_ne!(horizon, zenith);
        assert_ne!(zenith, storm);
    }

    #[test]
    fn subterranean_material_blocks_keep_distinct_palette_layers() {
        let (wood_ch, wood_fg, wood_bg) = Renderer::block_render_style(
            BlockType::Wood,
            BlockType::Air,
            Dimension::Overworld,
            1.0,
        );
        let (table_ch, table_fg, table_bg) = Renderer::block_render_style(
            BlockType::CraftingTable,
            BlockType::Air,
            Dimension::Overworld,
            1.0,
        );
        let (grass_ch, grass_fg, grass_bg) = Renderer::block_render_style(
            BlockType::Grass,
            BlockType::Air,
            Dimension::Overworld,
            1.0,
        );

        assert_eq!(wood_ch, '▒');
        assert_eq!(table_ch, '#');
        assert_eq!(grass_ch, '▀');
        assert!(matches!(wood_bg, Color::Rgb { .. }));
        assert!(matches!(table_bg, Color::Rgb { .. }));
        assert!(matches!(grass_bg, Color::Rgb { .. }));
        assert_ne!(wood_fg, table_fg);
        assert_ne!(grass_fg, grass_bg);
    }

    #[test]
    fn transparent_blocks_keep_reset_backgrounds_for_backdrops() {
        let (_, glass_fg, glass_bg) = Renderer::block_render_style(
            BlockType::Glass,
            BlockType::Air,
            Dimension::Overworld,
            1.0,
        );
        let (_, water_fg, water_bg) = Renderer::block_render_style(
            BlockType::Water(8),
            BlockType::Air,
            Dimension::Overworld,
            1.0,
        );
        let (_, door_fg, door_bg) = Renderer::block_render_style(
            BlockType::WoodDoor(false),
            BlockType::Air,
            Dimension::Overworld,
            1.0,
        );

        assert!(matches!(glass_fg, Color::Rgb { .. }));
        assert!(matches!(water_fg, Color::Rgb { .. }));
        assert!(matches!(door_fg, Color::Rgb { .. }));
        assert_eq!(glass_bg, Color::Reset);
        assert_eq!(water_bg, Color::Reset);
        assert_eq!(door_bg, Color::Reset);
    }

    #[test]
    fn covered_grass_and_snow_drop_surface_caps() {
        let (covered_grass_ch, covered_grass_fg, covered_grass_bg) = Renderer::block_render_style(
            BlockType::Grass,
            BlockType::Stone,
            Dimension::Overworld,
            1.0,
        );
        let (covered_snow_ch, covered_snow_fg, covered_snow_bg) = Renderer::block_render_style(
            BlockType::Snow,
            BlockType::Stone,
            Dimension::Overworld,
            1.0,
        );

        assert_eq!(covered_grass_ch, '▒');
        assert_eq!(covered_snow_ch, '▓');
        assert!(matches!(covered_grass_bg, Color::Rgb { .. }));
        assert!(matches!(covered_snow_bg, Color::Rgb { .. }));
        assert_ne!(covered_grass_fg, covered_grass_bg);
        assert_ne!(covered_snow_fg, covered_snow_bg);
    }

    #[test]
    fn opaque_terrain_blocks_use_softened_material_backgrounds() {
        let (_, stone_fg, stone_bg) = Renderer::block_render_style(
            BlockType::Stone,
            BlockType::Air,
            Dimension::Overworld,
            1.0,
        );
        let (_, wood_fg, wood_bg) = Renderer::block_render_style(
            BlockType::Wood,
            BlockType::Air,
            Dimension::Overworld,
            1.0,
        );
        let (_, table_fg, table_bg) = Renderer::block_render_style(
            BlockType::CraftingTable,
            BlockType::Air,
            Dimension::Overworld,
            1.0,
        );

        assert!(matches!(stone_fg, Color::Rgb { .. }));
        assert!(matches!(wood_fg, Color::Rgb { .. }));
        assert!(matches!(table_fg, Color::Rgb { .. }));
        assert!(matches!(stone_bg, Color::Rgb { .. }));
        assert!(matches!(wood_bg, Color::Rgb { .. }));
        assert!(matches!(table_bg, Color::Rgb { .. }));
    }

    #[test]
    fn softened_material_backgrounds_avoid_near_black_banding() {
        let (_, _, stone_bg) = Renderer::block_render_style(
            BlockType::Stone,
            BlockType::Air,
            Dimension::Overworld,
            1.0,
        );
        let (_, _, wood_bg) = Renderer::block_render_style(
            BlockType::Wood,
            BlockType::Air,
            Dimension::Overworld,
            1.0,
        );

        let Color::Rgb {
            r: stone_r,
            g: stone_g,
            b: stone_b,
        } = stone_bg
        else {
            panic!("stone background should be rgb");
        };
        let Color::Rgb {
            r: wood_r,
            g: wood_g,
            b: wood_b,
        } = wood_bg
        else {
            panic!("wood background should be rgb");
        };

        assert!(stone_r >= 120 && stone_g >= 120 && stone_b >= 120);
        assert!(wood_r >= 110 && wood_g >= 70 && wood_b >= 38);
    }

    #[test]
    fn covered_material_backgrounds_stay_close_to_foreground_to_reduce_top_banding() {
        let (_, stone_fg, stone_bg) = Renderer::block_render_style(
            BlockType::Stone,
            BlockType::Stone,
            Dimension::Overworld,
            1.0,
        );
        let (_, wood_fg, wood_bg) = Renderer::block_render_style(
            BlockType::Wood,
            BlockType::Stone,
            Dimension::Overworld,
            1.0,
        );

        let Color::Rgb {
            r: stone_fg_r,
            g: stone_fg_g,
            b: stone_fg_b,
        } = stone_fg
        else {
            panic!("stone foreground should be rgb");
        };
        let Color::Rgb {
            r: stone_bg_r,
            g: stone_bg_g,
            b: stone_bg_b,
        } = stone_bg
        else {
            panic!("stone background should be rgb");
        };
        let Color::Rgb {
            r: wood_fg_r,
            g: wood_fg_g,
            b: wood_fg_b,
        } = wood_fg
        else {
            panic!("wood foreground should be rgb");
        };
        let Color::Rgb {
            r: wood_bg_r,
            g: wood_bg_g,
            b: wood_bg_b,
        } = wood_bg
        else {
            panic!("wood background should be rgb");
        };

        let stone_delta = (stone_fg_r as i16 - stone_bg_r as i16).abs()
            + (stone_fg_g as i16 - stone_bg_g as i16).abs()
            + (stone_fg_b as i16 - stone_bg_b as i16).abs();
        let wood_delta = (wood_fg_r as i16 - wood_bg_r as i16).abs()
            + (wood_fg_g as i16 - wood_bg_g as i16).abs()
            + (wood_fg_b as i16 - wood_bg_b as i16).abs();

        assert!(stone_delta <= 40);
        assert!(wood_delta <= 54);
    }

    #[test]
    fn nether_themes_fortress_blocks_away_from_overworld_stone_palette() {
        let (_, overworld_fg, overworld_bg) = Renderer::block_render_style(
            BlockType::StoneBricks,
            BlockType::Air,
            Dimension::Overworld,
            1.0,
        );
        let (_, nether_fg, nether_bg) = Renderer::block_render_style(
            BlockType::StoneBricks,
            BlockType::Air,
            Dimension::Nether,
            1.0,
        );

        assert_ne!(overworld_fg, nether_fg);
        assert_ne!(overworld_bg, nether_bg);
    }

    #[test]
    fn nether_materials_use_distinct_ascii_glyphs_and_warm_palettes() {
        let (rack_ch, rack_fg, _) = Renderer::block_render_style(
            BlockType::Netherrack,
            BlockType::Air,
            Dimension::Nether,
            1.0,
        );
        let (soul_ch, soul_fg, _) = Renderer::block_render_style(
            BlockType::SoulSand,
            BlockType::Air,
            Dimension::Nether,
            1.0,
        );
        let (glow_ch, glow_fg, _) = Renderer::block_render_style(
            BlockType::Glowstone,
            BlockType::Air,
            Dimension::Nether,
            1.0,
        );
        let (portal_ch, portal_fg, _) = Renderer::block_render_style(
            BlockType::NetherPortal,
            BlockType::Air,
            Dimension::Nether,
            1.0,
        );

        assert_eq!(rack_ch, '%');
        assert_eq!(soul_ch, '&');
        assert_eq!(glow_ch, '*');
        assert_eq!(portal_ch, 'O');

        let Color::Rgb {
            r: rack_r,
            g: rack_g,
            b: rack_b,
        } = rack_fg
        else {
            panic!("expected rgb netherrack foreground");
        };
        let Color::Rgb {
            r: soul_r,
            g: soul_g,
            b: soul_b,
        } = soul_fg
        else {
            panic!("expected rgb soul sand foreground");
        };
        let Color::Rgb {
            r: glow_r,
            g: glow_g,
            b: glow_b,
        } = glow_fg
        else {
            panic!("expected rgb glowstone foreground");
        };
        let Color::Rgb {
            r: portal_r,
            b: portal_b,
            ..
        } = portal_fg
        else {
            panic!("expected rgb portal foreground");
        };

        assert!(rack_r > rack_g && rack_g > rack_b);
        assert!(soul_r > soul_g && soul_g > soul_b);
        assert!(glow_r >= glow_g && glow_g > glow_b);
        assert!(portal_b > portal_r);
    }

    #[test]
    fn glowstone_lights_a_cave_floor_and_removal_restores_darkness_in_every_view() {
        let mut state = GameState::network_view();
        state.dismiss_startup_splash();
        state.player.x = 0.5;
        state.player.y = 35.0;
        state.time_of_day = 0.0;
        let air = vec![BlockType::Air; crate::world::chunk::CHUNK_WIDTH * CHUNK_HEIGHT];
        for chunk in -2..=2 {
            state.world.apply_chunk_column_snapshot(chunk, &air);
        }
        for x in -64..=64 {
            state.world.set_block(x, 25, BlockType::Stone);
            state.world.set_block(x, 35, BlockType::Stone);
        }
        for zoom in 0..3 {
            let mut renderer = Renderer::with_size(160, 48);
            renderer.zoom = zoom;
            let brightness = |r: &Renderer, state: &GameState| -> u64 {
                let mut sum = 0;
                for y in 1..44 {
                    for x in 0..136 {
                        if r.screen_to_world(state, x, y) == (4, 35) {
                            let cell = r.back_buffer[y as usize * 160 + x as usize];
                            for color in [cell.fg, cell.bg] {
                                if let Some((r, g, b)) = Renderer::color_rgb(color) {
                                    sum += r as u64 + g as u64 + b as u64;
                                }
                            }
                        }
                    }
                }
                sum
            };
            renderer.render_to(&state, 0.0, &mut Vec::new()).unwrap();
            let dark = brightness(&renderer, &state);
            state.world.set_block(3, 34, BlockType::Glowstone);
            renderer.render_to(&state, 0.0, &mut Vec::new()).unwrap();
            let lit = brightness(&renderer, &state);
            state.world.set_block(3, 34, BlockType::Air);
            renderer.render_to(&state, 0.0, &mut Vec::new()).unwrap();
            assert!(
                dark > 0 && lit > dark * 6 / 5,
                "zoom {zoom}: {dark} -> {lit}"
            );
            assert_eq!(
                brightness(&renderer, &state),
                dark,
                "stale glow at zoom {zoom}"
            );
        }
    }

    #[test]
    fn nether_air_background_gets_warmer_with_depth_and_light() {
        let shallow_dark = Renderer::nether_air_background(18, 0);
        let deep_lit = Renderer::nether_air_background(96, 12);

        let Color::Rgb {
            r: shallow_r,
            g: shallow_g,
            ..
        } = shallow_dark
        else {
            panic!("expected shallow nether air rgb");
        };
        let Color::Rgb {
            r: deep_r,
            g: deep_g,
            ..
        } = deep_lit
        else {
            panic!("expected deep nether air rgb");
        };

        assert!(deep_r > shallow_r);
        assert!(deep_g > shallow_g);
    }

    #[test]
    fn earthy_block_palette_stays_close_to_mar15_baseline() {
        fn assert_warm_brown(color: Color) {
            let Color::Rgb { r, g, b } = color else {
                panic!("expected rgb brown");
            };
            assert!(r > g && g > b);
            assert!(r.saturating_sub(b) >= 55);
        }

        fn assert_close_to_baseline(color: Color, baseline: (u8, u8, u8), tolerance: u8) {
            let Color::Rgb { r, g, b } = color else {
                panic!("expected rgb baseline comparison");
            };
            assert!(r.abs_diff(baseline.0) <= tolerance);
            assert!(g.abs_diff(baseline.1) <= tolerance);
            assert!(b.abs_diff(baseline.2) <= tolerance);
        }

        let (_, dirt_fg, dirt_bg) = Renderer::block_render_style(
            BlockType::Dirt,
            BlockType::Air,
            Dimension::Overworld,
            1.0,
        );
        let (_, wood_fg, wood_bg) = Renderer::block_render_style(
            BlockType::Wood,
            BlockType::Air,
            Dimension::Overworld,
            1.0,
        );
        let (_, table_fg, table_bg) = Renderer::block_render_style(
            BlockType::CraftingTable,
            BlockType::Air,
            Dimension::Overworld,
            1.0,
        );
        let (_, birch_fg, birch_bg) = Renderer::block_render_style(
            BlockType::BirchWood,
            BlockType::Air,
            Dimension::Overworld,
            1.0,
        );

        assert_warm_brown(dirt_fg);
        assert_warm_brown(dirt_bg);
        assert_warm_brown(wood_fg);
        assert_warm_brown(wood_bg);
        assert_warm_brown(table_fg);
        assert_warm_brown(table_bg);
        assert!(matches!(birch_fg, Color::Rgb { .. }));
        assert!(matches!(birch_bg, Color::Rgb { .. }));
        assert_close_to_baseline(dirt_fg, (170, 118, 72), 10);
        assert_close_to_baseline(wood_fg, (164, 114, 60), 10);
        assert_close_to_baseline(table_fg, (188, 136, 78), 10);
        assert_close_to_baseline(birch_fg, (238, 228, 190), 12);
    }

    #[test]
    fn ui_warm_material_palette_keeps_dirt_and_workstations_near_baseline() {
        assert_eq!(
            Renderer::ui_warm_material_color(ItemType::Dirt),
            Some(Renderer::rgb((166, 114, 60)))
        );
        assert_eq!(
            Renderer::ui_warm_material_color(ItemType::CraftingTable),
            Some(Renderer::rgb((182, 132, 76)))
        );
        assert_eq!(
            Renderer::inventory_item_color(ItemType::Dirt),
            Renderer::rgb((166, 114, 60))
        );
        assert_eq!(
            Renderer::inventory_item_color(ItemType::Planks),
            Renderer::rgb((186, 142, 82))
        );
    }

    #[test]
    fn overworld_day_horizon_stays_tempered_near_ground() {
        let horizon = Renderer::overworld_sky_background(
            28,
            40,
            6000.0,
            PrecipitationType::None,
            0.0,
            0.0,
            false,
        );

        let Color::Rgb { r, g, b } = horizon else {
            panic!("expected rgb horizon color");
        };

        assert!(b > g && g > r, "expected readable blue day horizon");
        assert!(
            b - r <= 120,
            "expected bounded low-horizon saturation, got ({r},{g},{b})"
        );
    }

    #[test]
    fn wood_trunks_only_partially_occlude_skylight_columns() {
        let (wood_cell, below_wood) = Renderer::skylight_column_step(15, BlockType::Wood);
        let (birch_cell, below_birch) = Renderer::skylight_column_step(15, BlockType::BirchWood);
        let (stone_cell, below_stone) = Renderer::skylight_column_step(15, BlockType::Stone);

        assert!(wood_cell >= 12);
        assert!(below_wood >= 10);
        assert!(birch_cell >= 12);
        assert!(below_birch >= 10);
        assert!(stone_cell >= 12);
        assert_eq!(below_stone, 0);
    }

    #[test]
    fn ghast_sprite_is_larger_than_blaze_sprite() {
        let ghast = Renderer::ghast_sprite(true);
        let blaze = Renderer::blaze_sprite(0);

        let ghast_width = ghast.iter().map(|row| row.chars().count()).max().unwrap();
        let blaze_width = blaze.iter().map(|row| row.chars().count()).max().unwrap();

        assert!(ghast_width > blaze_width);
        assert!(ghast.len() > blaze.len());
    }

    #[test]
    fn world_cells_do_not_inherit_neighbor_backgrounds() {
        let mut renderer = Renderer {
            stdout: stdout(),
            width: 4,
            height: 3,
            buffer: vec![RenderCell::default(); 12],
            back_buffer: vec![RenderCell::default(); 12],
            light_buffer: Vec::new(),
            light_scratch: Vec::new(),
            light_frontier: std::array::from_fn(|_| Vec::new()),
            frame_output: Vec::new(),
            precipitation_buffer: Vec::new(),
            camera_y: 0.0,
            camera_initialized: false,
            key_releases: false,
            world_view: None,
            zoom: 0,
            pixel_colors: true,
            dragon_art_preview: None,
            actor_sprites: Vec::new(),
            motion: motion::MotionHistory::default(),
            player_preview: None,
            frame_seconds: 1.0 / 60.0,
            last_frame_at: None,
            vertical_pixel_offset: 0,
            pixel_horizons: Vec::new(),
        };
        renderer.back_buffer[5] = RenderCell {
            ch: '#',
            fg: Color::White,
            bg: Color::Rgb {
                r: 64,
                g: 48,
                b: 32,
            },
        };

        renderer.put_world_char(1, 1, ' ', Color::Reset, Color::Reset);

        assert_eq!(renderer.back_buffer[5].bg, Color::Reset);
    }

    #[test]
    fn lit_cave_air_gets_a_visible_background_tint() {
        let dark = Renderer::overworld_cave_air_background(1);
        let bright = Renderer::overworld_cave_air_background(12);

        let Some(Color::Rgb { r: dark_r, .. }) = dark else {
            panic!("expected dark cave air rgb");
        };
        let Some(Color::Rgb {
            r: bright_r,
            g: bright_g,
            b: bright_b,
        }) = bright
        else {
            panic!("expected bright cave air rgb");
        };

        assert!(bright_r > dark_r);
        assert!(bright_b > bright_g && bright_g > bright_r);
        assert!(Renderer::overworld_cave_air_background(0).is_none());
    }

    #[test]
    fn reset_background_overlay_preserves_existing_scene_background() {
        let mut renderer = Renderer {
            stdout: stdout(),
            width: 4,
            height: 3,
            buffer: vec![RenderCell::default(); 12],
            back_buffer: vec![RenderCell::default(); 12],
            light_buffer: Vec::new(),
            light_scratch: Vec::new(),
            light_frontier: std::array::from_fn(|_| Vec::new()),
            frame_output: Vec::new(),
            precipitation_buffer: Vec::new(),
            camera_y: 0.0,
            camera_initialized: false,
            key_releases: false,
            world_view: None,
            zoom: 0,
            pixel_colors: true,
            dragon_art_preview: None,
            actor_sprites: Vec::new(),
            motion: motion::MotionHistory::default(),
            player_preview: None,
            frame_seconds: 1.0 / 60.0,
            last_frame_at: None,
            vertical_pixel_offset: 0,
            pixel_horizons: Vec::new(),
        };
        renderer.back_buffer[5] = RenderCell {
            ch: '#',
            fg: Color::White,
            bg: Color::Rgb {
                r: 48,
                g: 62,
                b: 80,
            },
        };

        renderer.put_char(1, 1, '@', Color::Yellow, Color::Reset);

        assert_eq!(
            renderer.back_buffer[5].bg,
            Color::Rgb {
                r: 48,
                g: 62,
                b: 80,
            }
        );
    }

    #[test]
    fn reset_background_overlay_can_derive_from_existing_foreground() {
        let mut renderer = Renderer {
            stdout: stdout(),
            width: 4,
            height: 3,
            buffer: vec![RenderCell::default(); 12],
            back_buffer: vec![RenderCell::default(); 12],
            light_buffer: Vec::new(),
            light_scratch: Vec::new(),
            light_frontier: std::array::from_fn(|_| Vec::new()),
            frame_output: Vec::new(),
            precipitation_buffer: Vec::new(),
            camera_y: 0.0,
            camera_initialized: false,
            key_releases: false,
            world_view: None,
            zoom: 0,
            pixel_colors: true,
            dragon_art_preview: None,
            actor_sprites: Vec::new(),
            motion: motion::MotionHistory::default(),
            player_preview: None,
            frame_seconds: 1.0 / 60.0,
            last_frame_at: None,
            vertical_pixel_offset: 0,
            pixel_horizons: Vec::new(),
        };
        renderer.back_buffer[5] = RenderCell {
            ch: '*',
            fg: Color::Rgb {
                r: 180,
                g: 120,
                b: 60,
            },
            bg: Color::Reset,
        };

        renderer.put_char(1, 1, '@', Color::Yellow, Color::Reset);

        assert_eq!(
            renderer.back_buffer[5].bg,
            Color::Rgb {
                r: 57,
                g: 38,
                b: 19,
            }
        );
    }

    #[test]
    fn entity_overlay_darkens_slightly_against_bright_day_backgrounds() {
        let mut renderer = Renderer {
            stdout: stdout(),
            width: 4,
            height: 3,
            buffer: vec![RenderCell::default(); 12],
            back_buffer: vec![RenderCell::default(); 12],
            light_buffer: Vec::new(),
            light_scratch: Vec::new(),
            light_frontier: std::array::from_fn(|_| Vec::new()),
            frame_output: Vec::new(),
            precipitation_buffer: Vec::new(),
            camera_y: 0.0,
            camera_initialized: false,
            key_releases: false,
            world_view: None,
            zoom: 0,
            pixel_colors: true,
            dragon_art_preview: None,
            actor_sprites: Vec::new(),
            motion: motion::MotionHistory::default(),
            player_preview: None,
            frame_seconds: 1.0 / 60.0,
            last_frame_at: None,
            vertical_pixel_offset: 0,
            pixel_horizons: Vec::new(),
        };
        renderer.back_buffer[5] = RenderCell {
            ch: ' ',
            fg: Color::Reset,
            bg: Color::Rgb {
                r: 182,
                g: 212,
                b: 238,
            },
        };

        let original = Renderer::rgb((88, 128, 198));
        let adjusted = renderer.entity_visibility_color_at(1, 1, original);

        let Color::Rgb { r, g, b } = adjusted else {
            panic!("expected adjusted rgb");
        };
        assert!(r < 88 && g < 128 && b < 198);
    }

    #[test]
    fn entity_overlay_lifts_slightly_against_dark_terrain_backgrounds() {
        let mut renderer = Renderer {
            stdout: stdout(),
            width: 4,
            height: 3,
            buffer: vec![RenderCell::default(); 12],
            back_buffer: vec![RenderCell::default(); 12],
            light_buffer: Vec::new(),
            light_scratch: Vec::new(),
            light_frontier: std::array::from_fn(|_| Vec::new()),
            frame_output: Vec::new(),
            precipitation_buffer: Vec::new(),
            camera_y: 0.0,
            camera_initialized: false,
            key_releases: false,
            world_view: None,
            zoom: 0,
            pixel_colors: true,
            dragon_art_preview: None,
            actor_sprites: Vec::new(),
            motion: motion::MotionHistory::default(),
            player_preview: None,
            frame_seconds: 1.0 / 60.0,
            last_frame_at: None,
            vertical_pixel_offset: 0,
            pixel_horizons: Vec::new(),
        };
        renderer.back_buffer[5] = RenderCell {
            ch: ' ',
            fg: Color::Reset,
            bg: Color::Rgb {
                r: 44,
                g: 36,
                b: 30,
            },
        };

        let original = Renderer::rgb((139, 69, 19));
        let adjusted = renderer.entity_visibility_color_at(1, 1, original);

        let Color::Rgb { r, g, b } = adjusted else {
            panic!("expected adjusted rgb");
        };
        assert!(r > 139 && g > 69 && b > 19);
    }

    #[test]
    fn player_overlay_pushes_stronger_contrast_than_generic_entity_overlay() {
        let mut renderer = Renderer {
            stdout: stdout(),
            width: 4,
            height: 3,
            buffer: vec![RenderCell::default(); 12],
            back_buffer: vec![RenderCell::default(); 12],
            light_buffer: Vec::new(),
            light_scratch: Vec::new(),
            light_frontier: std::array::from_fn(|_| Vec::new()),
            frame_output: Vec::new(),
            precipitation_buffer: Vec::new(),
            camera_y: 0.0,
            camera_initialized: false,
            key_releases: false,
            world_view: None,
            zoom: 0,
            pixel_colors: true,
            dragon_art_preview: None,
            actor_sprites: Vec::new(),
            motion: motion::MotionHistory::default(),
            player_preview: None,
            frame_seconds: 1.0 / 60.0,
            last_frame_at: None,
            vertical_pixel_offset: 0,
            pixel_horizons: Vec::new(),
        };
        renderer.back_buffer[5] = RenderCell {
            ch: ' ',
            fg: Color::Reset,
            bg: Color::Rgb {
                r: 186,
                g: 214,
                b: 240,
            },
        };

        let original = Renderer::rgb((76, 114, 188));
        let generic = renderer.entity_visibility_color_at(1, 1, original);
        let player = renderer.player_visibility_color_at(1, 1, original);
        let bg_rgb = (186, 214, 240);
        let generic_rgb = Renderer::color_rgb(generic).expect("generic should stay rgb");
        let player_rgb = Renderer::color_rgb(player).expect("player should stay rgb");

        let generic_contrast = (Renderer::rgb_luma(generic_rgb) - Renderer::rgb_luma(bg_rgb)).abs();
        let player_contrast = (Renderer::rgb_luma(player_rgb) - Renderer::rgb_luma(bg_rgb)).abs();

        assert!(player_contrast > generic_contrast);
    }

    #[test]
    fn player_overlay_pushes_stronger_contrast_than_generic_on_dark_nether_backgrounds() {
        let mut renderer = Renderer {
            stdout: stdout(),
            width: 4,
            height: 3,
            buffer: vec![RenderCell::default(); 12],
            back_buffer: vec![RenderCell::default(); 12],
            light_buffer: Vec::new(),
            light_scratch: Vec::new(),
            light_frontier: std::array::from_fn(|_| Vec::new()),
            frame_output: Vec::new(),
            precipitation_buffer: Vec::new(),
            camera_y: 0.0,
            camera_initialized: false,
            key_releases: false,
            world_view: None,
            zoom: 0,
            pixel_colors: true,
            dragon_art_preview: None,
            actor_sprites: Vec::new(),
            motion: motion::MotionHistory::default(),
            player_preview: None,
            frame_seconds: 1.0 / 60.0,
            last_frame_at: None,
            vertical_pixel_offset: 0,
            pixel_horizons: Vec::new(),
        };
        renderer.back_buffer[5] = RenderCell {
            ch: ' ',
            fg: Color::Reset,
            bg: Color::Rgb {
                r: 38,
                g: 12,
                b: 14,
            },
        };

        let original = Renderer::rgb((68, 102, 176));
        let generic = renderer.entity_visibility_color_at(1, 1, original);
        let player = renderer.player_visibility_color_at(1, 1, original);
        let bg_rgb = (38, 12, 14);
        let generic_rgb = Renderer::color_rgb(generic).expect("generic should stay rgb");
        let player_rgb = Renderer::color_rgb(player).expect("player should stay rgb");

        let generic_contrast = (Renderer::rgb_luma(generic_rgb) - Renderer::rgb_luma(bg_rgb)).abs();
        let player_contrast = (Renderer::rgb_luma(player_rgb) - Renderer::rgb_luma(bg_rgb)).abs();

        assert!(player_contrast > generic_contrast);
        assert!(Renderer::rgb_luma(player_rgb) > Renderer::rgb_luma(generic_rgb));
    }

    #[test]
    fn player_visible_colors_follow_equipped_armor() {
        let mut state = GameState::new();
        state.apply_client_command(crate::engine::command::ClientCommand::EquipDiamondLoadout);

        let (head, torso, limb) = Renderer::player_visible_colors(&state, 1.0);

        assert_eq!(head, Renderer::rgb((92, 236, 224)));
        assert_eq!(torso, Renderer::rgb((92, 236, 224)));
        assert_eq!(limb, Renderer::rgb((92, 236, 224)));
    }

    #[test]
    fn player_visible_colors_use_deeper_default_daytime_palette() {
        let state = GameState::new();

        let (head, torso, limb) = Renderer::player_visible_colors(&state, 1.0);

        assert_eq!(head, Renderer::rgb((236, 196, 156)));
        assert_eq!(torso, Renderer::rgb((68, 102, 176)));
        assert_eq!(limb, Renderer::rgb((66, 58, 108)));
    }

    #[test]
    fn hotbar_slot_background_picks_up_item_family_tints() {
        let dirt = ItemStack {
            item_type: ItemType::Dirt,
            count: 32,
            durability: None,
        };
        let diamond = ItemStack {
            item_type: ItemType::DiamondSword,
            count: 1,
            durability: ItemType::DiamondSword.max_durability(),
        };

        let dirt_bg = Renderer::color_rgb(Renderer::hotbar_slot_background(Some(&dirt), false))
            .expect("dirt hotbar bg should resolve to rgb");
        let diamond_bg =
            Renderer::color_rgb(Renderer::hotbar_slot_background(Some(&diamond), false))
                .expect("diamond hotbar bg should resolve to rgb");

        assert_ne!(dirt_bg, diamond_bg);
        assert!(
            dirt_bg.0 > dirt_bg.2,
            "dirt slot should stay warmer than blue"
        );
        assert!(
            diamond_bg.2 > diamond_bg.0,
            "diamond slot should skew cooler"
        );
    }

    #[test]
    fn hotbar_stack_metric_uses_counts_for_stackables_and_percent_for_tools() {
        let blocks = ItemStack {
            item_type: ItemType::Cobblestone,
            count: 64,
            durability: None,
        };
        let tool = ItemStack {
            item_type: ItemType::IronPickaxe,
            count: 1,
            durability: Some(126),
        };

        let (block_metric, _) = Renderer::hotbar_stack_metric(&blocks);
        let (tool_metric, _) = Renderer::hotbar_stack_metric(&tool);

        assert_eq!(block_metric, "x64");
        assert_eq!(tool_metric, " 50%");
    }

    #[test]
    fn armor_tint_palette_distinguishes_armor_materials() {
        assert_eq!(
            Renderer::armor_tint_rgb(ItemType::LeatherChestplate),
            Some((128, 82, 50))
        );
        assert_eq!(
            Renderer::armor_tint_rgb(ItemType::IronChestplate),
            Some((216, 224, 232))
        );
        assert_eq!(
            Renderer::armor_tint_rgb(ItemType::DiamondChestplate),
            Some((92, 236, 224))
        );
    }

    #[test]
    fn end_victory_banner_lines_shift_through_stages() {
        assert_eq!(
            Renderer::end_victory_banner_lines(120),
            ("ENDER DRAGON SLAIN", "Shockwaves tear through the End")
        );
        assert_eq!(
            Renderer::end_victory_banner_lines(70),
            ("THE END TREMBLES", "Ancient light pours from the core")
        );
        assert_eq!(
            Renderer::end_victory_banner_lines(18),
            ("VICTORY IN THE END", "Enter the portal when you are ready")
        );
    }
}
