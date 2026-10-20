//! A simulation-timed finale: recoil, last stand, rupture, then the portal's light.
use super::{RenderCell, Renderer};

fn ramp(time: f32, start: f32, end: f32) -> f32 {
    let t = ((time - start) / (end - start)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

pub(super) fn rise(time: f32) -> f32 {
    ramp(time, 20.0, 100.0) * 1.35
}

// Violet is the End's base; cyan, hot pink and amber separate the energy paths
// so the burst reads in color without washing out the dragon's dark silhouette.
fn energy_color(index: usize) -> (u8, u8, u8) {
    [
        (175, 83, 255),
        (60, 223, 245),
        (255, 75, 191),
        (255, 191, 79),
    ][index % 4]
}

// Move the projection only when the last wing spread would cross the header.
// The saved view/resolution and the ordinary gameplay camera stay untouched.
pub(super) fn camera_y(base: f64, origin_y: f64, time: f32, center_y: i32, scale: i32) -> f64 {
    let highest_fit = origin_y - 5.0 - 1.35 + center_y as f64 - 4.0 / scale as f64 - 0.6;
    let weight = ramp(time, 0.0, 32.0) * (1.0 - ramp(time, 212.0, 280.0));
    base + (highest_fit - base).min(0.0) * weight as f64
}

impl Renderer {
    pub(super) fn render_victory_pixels(
        &mut self,
        x: i32,
        feet: i32,
        scale: i32,
        time: f32,
        bottom: i32,
    ) {
        if !self.pixel_colors || !(0.0..280.0).contains(&time) {
            return;
        }
        let unit = (2 * scale) as f32;
        let core = (x as f32, feet as f32 - unit * (1.75 + rise(time)));
        let charge = ramp(time, 16.0, 130.0) * (1.0 - ramp(time, 140.0, 174.0));
        let impact = ramp(time, 136.0, 141.0) * (1.0 - ramp(time, 144.0, 174.0));
        // Palette steps give lighting its own beats without recoloring every
        // terrain pixel on every display frame (expensive over a terminal/SSH).
        let dim = (ramp(time, 6.0, 76.0) * (1.0 - ramp(time, 166.0, 250.0)) * 7.0).round() / 12.0;
        let arena_flash = if (139.0..145.0).contains(&time) {
            0.22
        } else if (145.0..153.0).contains(&time) {
            0.12
        } else if (153.0..163.0).contains(&time) {
            0.04
        } else {
            0.0
        };

        // Let the whole arena react, preserving terrain, player and pillar shapes.
        // One sustained illumination pulse, rather than repeated white flashes.
        for py in 4..(bottom / 2).min(self.height as i32 - 4) {
            for px in 0..self.width as i32 {
                let i = py as usize * self.width as usize + px as usize;
                let cell = self.back_buffer[i];
                let grade = |color| {
                    Self::color_rgb(color).map_or(color, |rgb| {
                        let rgb = Self::scale_rgb(rgb, 1.0 - dim);
                        Self::rgb(Self::lerp_rgb(rgb, (221, 196, 245), arena_flash))
                    })
                };
                self.back_buffer[i] = RenderCell {
                    ch: cell.ch,
                    fg: grade(cell.fg),
                    bg: grade(cell.bg),
                };
            }
        }

        // Light is pulled inward before it is released. Stable curved paths
        // make the buildup readable even in the smallest terminal view.
        if time < 139.0 {
            for mote in 0..40 {
                let seed = mote as f32;
                let p = ((time * 0.009 + seed * 0.173) % 1.0).clamp(0.0, 1.0);
                let angle = seed * 2.39996 + p * 1.1;
                let radius = unit * (1.0 + (1.0 - p).powi(2) * (7.0 + seed % 7.0));
                let point = (
                    core.0 + angle.cos() * radius,
                    core.1 + angle.sin() * radius * 0.65,
                );
                let tail = (
                    point.0 + angle.cos() * unit * 0.7,
                    point.1 + angle.sin() * unit * 0.7,
                );
                let color = energy_color(mote);
                self.finale_line(tail, point, color, charge * p * 0.75, 0, bottom);
                self.finale_pixel(point, color, charge * p, bottom);
            }
        }

        // Broad, asymmetrical shafts split out of the chest; no static tube
        // running through the entire screen. Their edges taper with distance.
        for ray in 0..9 {
            let angle = [
                -2.89_f32, -2.28, -1.69, -1.16, -0.48, 0.24, 0.88, 1.64, 2.48,
            ][ray];
            let strength = ramp(time, 48.0 + ray as f32 * 5.0, 98.0 + ray as f32 * 3.0)
                * (1.0 - ramp(time, 146.0, 190.0));
            let length = unit * (6.0 + (ray % 4) as f32 * 3.0) * (0.4 + charge * 0.6);
            let end = (core.0 + angle.cos() * length, core.1 + angle.sin() * length);
            self.finale_line(
                core,
                end,
                energy_color(ray),
                strength * 0.56,
                scale * 2,
                bottom,
            );
            self.finale_line(core, end, (246, 215, 255), strength * 0.72, 0, bottom);
        }
        self.finale_glow(
            core,
            unit * (1.8 + charge * 2.0),
            (163, 69, 219),
            charge * 0.48,
            bottom,
        );
        self.draw_victory_dragon(x, feet, scale, time, bottom);
        self.finale_glow(
            core,
            unit * (0.5 + impact * 5.0),
            (255, 236, 207),
            impact * 0.8,
            bottom,
        );

        // The wing panels remain intact as they separate. These additional
        // bright fragments follow ballistic arcs rather than dissolving to noise.
        if (136.0..236.0).contains(&time) {
            let t = (time - 136.0) / 20.0;
            for shard in 0..32 {
                let angle = shard as f32 * 2.39996;
                let speed = 1.1 + (shard % 7) as f32 * 0.36;
                let point = (
                    core.0 + angle.cos() * speed * t * unit,
                    core.1 + (angle.sin() * speed * t - t * 1.15 + t * t * 0.24) * unit,
                );
                let alpha = (1.0 - ramp(time, 168.0 + (shard % 5) as f32 * 6.0, 235.0)) * 0.9;
                let color = energy_color(shard);
                self.finale_line(
                    (
                        point.0 - angle.cos() * unit * 0.7,
                        point.1 - angle.sin() * unit * 0.7,
                    ),
                    point,
                    color,
                    alpha * 0.5,
                    0,
                    bottom,
                );
                self.finale_pixel(point, color, alpha, bottom);
            }
        }
        for delay in [136.0, 151.0] {
            let t = (time - delay) / 82.0;
            if (0.0..1.0).contains(&t) {
                self.finale_ring(
                    core,
                    unit * (0.8 + t * 21.0),
                    0.52,
                    if delay == 136.0 {
                        (86, 228, 247)
                    } else {
                        (255, 108, 207)
                    },
                    (1.0 - t).powf(0.65) * 0.9,
                    bottom,
                );
            }
        }

        // The last energy leaves the dragon and bends into the actual portal.
        let (vx, vy, cx, cy, _) = self.world_view.unwrap();
        let portal = (
            (0.5 * unit).round() - vx as f32 + cx as f32,
            (33.0 * unit).round() - vy as f32 * 2.0 - self.vertical_pixel_offset as f32
                + cy as f32 * 2.0,
        );
        if (172.0..264.0).contains(&time) {
            for mote in 0..28 {
                let t = (time - 172.0 - mote as f32 * 1.3) / 54.0;
                if !(0.0..1.0).contains(&t) {
                    continue;
                }
                let control = ((core.0 + portal.0) * 0.5, core.1 - unit * 4.0);
                let point = (
                    (1.0 - t).powi(2) * core.0 + 2.0 * (1.0 - t) * t * control.0 + t * t * portal.0,
                    (1.0 - t).powi(2) * core.1 + 2.0 * (1.0 - t) * t * control.1 + t * t * portal.1,
                );
                self.finale_glow(point, scale as f32 * 1.5, energy_color(mote), 0.75, bottom);
                self.finale_pixel(point, (243, 226, 184), 0.95, bottom);
            }
        }
        let portal_light = ramp(time, 202.0, 228.0) * (1.0 - ramp(time, 260.0, 280.0));
        self.finale_line(
            portal,
            (portal.0, portal.1 - unit * 8.0),
            (185, 132, 244),
            portal_light * 0.65,
            scale,
            bottom,
        );
        self.finale_glow(
            (portal.0, portal.1 - unit * 0.5),
            unit * 2.8,
            (255, 210, 143),
            portal_light * 0.4,
            bottom,
        );
        self.finale_ring(
            portal,
            unit * 2.0,
            0.25,
            (94, 226, 242),
            portal_light * 0.8,
            bottom,
        );
    }

    fn finale_pixel(&mut self, point: (f32, f32), color: (u8, u8, u8), alpha: f32, bottom: i32) {
        let (x, y) = (point.0.round() as i32, point.1.round() as i32);
        if x < 0 || x >= self.width as i32 || y < 8 || y >= bottom || alpha <= 0.0 {
            return;
        }
        // Sixteen stable coverage levels preserve the pixel-art glow while
        // preventing tiny alpha changes from rewriting whole colored shafts.
        let alpha = (alpha.clamp(0.0, 1.0) * 16.0).round() / 16.0;
        if alpha == 0.0 {
            return;
        }
        let color = self.blend_pixel(x, y, Self::rgb(color), alpha);
        self.put_pixel(x, y, color);
    }

    #[allow(clippy::too_many_arguments)]
    fn finale_line(
        &mut self,
        from: (f32, f32),
        to: (f32, f32),
        color: (u8, u8, u8),
        alpha: f32,
        width: i32,
        bottom: i32,
    ) {
        if alpha <= 0.0 {
            return;
        }
        let delta = (to.0 - from.0, to.1 - from.1);
        let length = delta.0.hypot(delta.1).max(1.0);
        let normal = (-delta.1 / length, delta.0 / length);
        for step in 0..=length.ceil() as i32 {
            let t = step as f32 / length;
            let p = (from.0 + delta.0 * t, from.1 + delta.1 * t);
            let spread = (width as f32 * (0.15 + t * 1.6)).round() as i32;
            for offset in -spread..=spread {
                let feather = 1.0 - offset.abs() as f32 / (spread + 1) as f32;
                self.finale_pixel(
                    (
                        p.0 + normal.0 * offset as f32,
                        p.1 + normal.1 * offset as f32,
                    ),
                    color,
                    alpha * feather * (1.0 - t * 0.65),
                    bottom,
                );
            }
        }
    }

    fn finale_glow(
        &mut self,
        center: (f32, f32),
        radius: f32,
        color: (u8, u8, u8),
        alpha: f32,
        bottom: i32,
    ) {
        if alpha <= 0.0 {
            return;
        }
        for y in ((center.1 - radius) as i32).max(8)..=((center.1 + radius) as i32).min(bottom - 1)
        {
            for x in ((center.0 - radius) as i32).max(0)
                ..=((center.0 + radius) as i32).min(self.width as i32 - 1)
            {
                let d =
                    ((x as f32 - center.0).powi(2) + (y as f32 - center.1).powi(2)).sqrt() / radius;
                if d < 1.0 {
                    self.finale_pixel(
                        (x as f32, y as f32),
                        color,
                        alpha * (1.0 - d).powi(2),
                        bottom,
                    );
                }
            }
        }
    }

    fn finale_ring(
        &mut self,
        center: (f32, f32),
        radius: f32,
        flatten: f32,
        color: (u8, u8, u8),
        alpha: f32,
        bottom: i32,
    ) {
        let steps = (radius * 8.0).ceil() as i32;
        for step in 0..steps {
            let angle = step as f32 / steps as f32 * std::f32::consts::TAU;
            self.finale_pixel(
                (
                    center.0 + angle.cos() * radius,
                    center.1 + angle.sin() * radius * flatten,
                ),
                color,
                alpha,
                bottom,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::Color;
    use super::*;

    #[test]
    fn finale_is_deterministic_and_cannot_paint_header_or_inventory_in_any_view() {
        for (w, h, scale) in [
            (80, 24, 1),
            (120, 40, 2),
            (160, 48, 2),
            (160, 48, 4),
            (200, 60, 1),
        ] {
            let mut r = Renderer::with_size(w, h);
            r.world_view = Some((0, 0, w as i32 / 2, h as i32 / 2, scale));
            let base = RenderCell {
                ch: '▀',
                fg: Color::Rgb {
                    r: 71,
                    g: 58,
                    b: 95,
                },
                bg: Color::Rgb {
                    r: 48,
                    g: 39,
                    b: 67,
                },
            };
            r.back_buffer.fill(base);
            let original = r.back_buffer.clone();
            for time in [
                0.0, 24.0, 56.0, 92.5, 128.0, 142.0, 170.0, 200.0, 228.0, 252.0, 279.5,
            ] {
                r.back_buffer.clone_from(&original);
                r.render_victory_pixels(w as i32 / 2, h as i32, scale, time, (h as i32 - 4) * 2);
                let first = r.back_buffer.clone();
                assert_eq!(&first[..w as usize * 4], &original[..w as usize * 4]);
                assert_eq!(
                    &first[w as usize * (h as usize - 4)..],
                    &original[w as usize * (h as usize - 4)..]
                );
                r.back_buffer.clone_from(&original);
                r.render_victory_pixels(w as i32 / 2, h as i32, scale, time, (h as i32 - 4) * 2);
                assert_eq!(first, r.back_buffer);
            }
            r.back_buffer.clone_from(&original);
            r.render_victory_pixels(w as i32 / 2, h as i32, scale, 280.0, (h as i32 - 4) * 2);
            assert_eq!(
                r.back_buffer, original,
                "last frame must restore the ordinary world"
            );
        }
    }

    #[test]
    fn glyph_view_stays_glyphs_during_the_finale() {
        let mut r = Renderer::with_size(80, 24);
        r.pixel_colors = false;
        let original = r.back_buffer.clone();
        r.render_victory_pixels(40, 30, 1, 140.0, 40);
        assert_eq!(r.back_buffer, original);
    }
}
