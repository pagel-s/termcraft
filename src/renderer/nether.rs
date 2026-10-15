//! Recessed cavern art. Every feature uses world coordinates at the terrain's
//! scale: camera motion never moves a second landscape at a different speed.
use super::Renderer;

fn hash(cell: i32, salt: u32) -> u32 {
    let mut n = (cell as u32).wrapping_mul(0x9e37_79b9) ^ salt;
    n = (n ^ (n >> 16)).wrapping_mul(0x85eb_ca6b);
    n ^ (n >> 13)
}

pub(super) struct Column {
    ridge: f64,
    face: f32,
    seam: f64,
    ember: Option<f64>,
}

impl Column {
    pub(super) fn new(x: f64) -> Self {
        let cell = (x / 24.0).floor() as i32;
        let seed = hash(cell, 0x6273_18ac);
        let center = cell as f64 * 24.0 + 12.0 + f64::from(seed % 7) - 3.0;
        let dx = x - center;
        let offset = center - (cell as f64 * 24.0 + 12.0);
        let radius = (7.0 + f64::from((seed >> 4) % 6)).min(11.0 - offset.abs());
        let peak = ((1.0 - dx.abs() / radius) * 2.5).clamp(0.0, 1.0);
        let ridge = 70.0 - peak * (22.0 + f64::from((seed >> 12) % 15));
        let face = if dx < 0.0 {
            1.0
        } else {
            1.0 - peak as f32 * 0.38
        };
        let ember_cell = (x * 2.0).floor() as i32;
        let ember_seed = hash(ember_cell, 0x4f72_d9a1);
        Self {
            ridge,
            face,
            seam: f64::from((seed >> 16) % 11) * peak,
            ember: ember_seed
                .is_multiple_of(71)
                .then_some(f64::from(ember_seed % 47)),
        }
    }

    pub(super) fn pixel(&self, y: f64, time: f64) -> (u8, u8, u8) {
        let depth = ((y - 12.0) / 75.0).clamp(0.0, 1.0) as f32;
        let mut color = Renderer::lerp_rgb((25, 17, 34), (84, 35, 42), depth);
        if y >= self.ridge {
            let haze = ((y - self.ridge) / 22.0).clamp(0.0, 1.0) as f32;
            color = Renderer::lerp_rgb((33, 24, 39), (56, 29, 39), haze);
            if y - self.ridge < 0.3 {
                color = (56, 35, 47);
            } else {
                color = Renderer::scale_rgb(color, 0.76 + self.face * 0.24);
            }
            // Broad, quiet strata, rather than per-pixel texture noise.
            if (y + self.seam).rem_euclid(9.0) < 0.25 {
                color = Renderer::lerp_rgb(color, (96, 48, 47), 0.25);
            }
        }
        // Sparse embers drift upwards. Their positions stay in world space,
        // independent of the camera, zoom and terminal dimensions.
        if let Some(start) = self.ember {
            let center = 19.0 + (start - time * 0.035).rem_euclid(47.0);
            let distance = (y - center).abs();
            if distance < 0.16 {
                color = (183, 99, 66);
            } else if distance < 0.4 {
                color = Renderer::lerp_rgb(color, (121, 57, 50), 0.45);
            }
        }
        color
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backdrop_is_continuous_across_origin_and_bounded_for_large_worlds() {
        for x in [-1_000_000.0, -24.0, -0.01, 0.0, 24.0, 1_000_000.0] {
            for y in [-20.0, 0.0, 40.0, 128.0] {
                let rgb = Column::new(x).pixel(y, 2_000.0);
                assert!(rgb.0 <= 183 && rgb.1 <= 99 && rgb.2 <= 66);
            }
        }
        for y in [10.0, 30.0, 75.0] {
            assert_eq!(
                Column::new(-0.001).pixel(y, 0.0),
                Column::new(0.001).pixel(y, 0.0)
            );
        }
    }
}
