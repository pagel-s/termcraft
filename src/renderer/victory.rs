//! Fourteen-second, simulation-timed boss finale; no camera shake or random flashes.
use super::Renderer;

impl Renderer {
    pub(super) fn render_victory_pixels(
        &mut self,
        x: i32,
        feet: i32,
        scale: i32,
        elapsed: f32,
        bottom: i32,
    ) {
        let time = elapsed;
        // Hold the dragon together during the charge; give the explosion its
        // own act, followed by a lingering portal reveal.
        let elapsed = if time < 90.0 {
            time * (24.0 / 90.0)
        } else if time < 200.0 {
            24.0 + (time - 90.0) * (66.0 / 110.0)
        } else {
            90.0 + (time - 200.0) * (50.0 / 80.0)
        };
        let unit = (2 * scale) as f32;
        let core = (x as f32, feet as f32 - unit * 2.4);
        // A widening column and rising motes establish the scale of the charge
        // before the dragon fractures. No screen-wide flash or camera shake.
        let charge =
            (time / 90.0).clamp(0.0, 1.0) * (1.0 - ((time - 130.0) / 60.0).clamp(0.0, 1.0));
        for py in 4..bottom {
            let spread = (scale as f32 * (0.7 + charge * 1.1)).ceil() as i32;
            for dx in -spread..=spread {
                let strength = (1.0 - dx.abs() as f32 / (spread + 1) as f32) * charge * 0.55;
                let color = self.blend_pixel(x + dx, py, Self::rgb((198, 139, 255)), strength);
                self.put_pixel(x + dx, py, color);
            }
        }
        if time < 220.0 {
            for n in 0..24 {
                let angle = n as f32 * 2.39996 + time * 0.018;
                let radius = unit * (2.0 + (n % 5) as f32 * 0.7) * (1.0 - charge * 0.3);
                let px = (core.0 + angle.cos() * radius).round() as i32;
                let py = (core.1 + angle.sin() * radius * 0.5
                    - ((time * 0.04 + n as f32) % 5.0) * unit)
                    .round() as i32;
                if py >= 4 && py < bottom {
                    let c = self.blend_pixel(px, py, Self::rgb((230, 197, 255)), charge * 0.8);
                    self.put_pixel(px, py, c);
                }
            }
        }
        // Bloom breathes into one controlled burst, then fades into the shards.
        let burst = ((elapsed - 22.0) / 15.0).clamp(0.0, 1.0)
            * (1.0 - ((elapsed - 45.0) / 45.0).clamp(0.0, 1.0));
        let radius = unit * (2.0 + burst * 5.0);
        for py in ((core.1 - radius) as i32).max(4)..=((core.1 + radius) as i32).min(bottom - 1) {
            for px in ((core.0 - radius) as i32).max(0)
                ..=((core.0 + radius) as i32).min(self.width as i32 - 1)
            {
                let d =
                    ((px as f32 - core.0).powi(2) + (py as f32 - core.1).powi(2)).sqrt() / radius;
                if d < 1.0 {
                    let color = self.blend_pixel(
                        px,
                        py,
                        Self::rgb((166, 82, 240)),
                        (1.0 - d).powi(2) * (0.12 + burst * 0.5),
                    );
                    self.put_pixel(px, py, color);
                }
            }
        }
        // Fixed ray directions keep the breakup legible as energy expands.
        if elapsed > 20.0 && elapsed < 108.0 {
            let expansion = ((elapsed - 20.0) / 40.0).clamp(0.0, 1.0);
            for ray in 0..12 {
                let angle = ray as f32 * std::f32::consts::TAU / 12.0 + 0.17;
                let length = unit * (2.0 + expansion * (7.0 + (ray % 3) as f32));
                for step in 1..=length as i32 {
                    let t = step as f32 / length;
                    let px = (core.0 + angle.cos() * step as f32).round() as i32;
                    let py = (core.1 + angle.sin() * step as f32).round() as i32;
                    if py < 4 || py >= bottom {
                        continue;
                    }
                    let color = self.blend_pixel(
                        px,
                        py,
                        Self::rgb((228, 197, 255)),
                        (1.0 - t) * burst * 0.75,
                    );
                    self.put_pixel(px, py, color);
                }
            }
        }
        self.draw_victory_dragon(x, feet, scale, elapsed, bottom);
        // Two coherent shock fronts replace screen-space ASCII confetti.
        for delay in [30.0, 48.0] {
            let t = (elapsed - delay) / 64.0;
            if !(0.0..1.0).contains(&t) {
                continue;
            }
            let r = unit * (1.0 + t * 15.0);
            let steps = (r * 8.0).ceil() as i32;
            for step in 0..steps {
                let a = step as f32 / steps as f32 * std::f32::consts::TAU;
                let px = (core.0 + a.cos() * r).round() as i32;
                let py = (core.1 + a.sin() * r * 0.56).round() as i32;
                if py < 4 || py >= bottom {
                    continue;
                }
                let c = self.blend_pixel(px, py, Self::rgb((185, 153, 255)), (1.0 - t) * 0.8);
                self.put_pixel(px, py, c);
            }
        }
        // The last act draws the eye back to the actual return portal.
        if elapsed > 70.0 {
            let (vx, vy, cx, cy, _) = self.world_view.unwrap();
            let px = (0.5 * unit).round() as i32 - vx + cx;
            let py = (33.0 * unit).round() as i32 - vy * 2 - self.vertical_pixel_offset + cy * 2;
            let strength = ((elapsed - 70.0) / 24.0).min(1.0)
                * (1.0 - ((elapsed - 118.0) / 22.0).clamp(0.0, 1.0));
            for dy in 0..(unit * 8.0) as i32 {
                let y = py - dy;
                if y < 4 || y >= bottom {
                    continue;
                }
                for dx in -scale..=scale {
                    let c = self.blend_pixel(
                        px + dx,
                        y,
                        Self::rgb((208, 177, 255)),
                        strength * 0.5 * (1.0 - dy as f32 / (unit * 8.0)),
                    );
                    self.put_pixel(px + dx, y, c);
                }
            }
        }
    }
}
