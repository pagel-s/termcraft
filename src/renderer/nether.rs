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
    far_ridge: f64,
    face: f32,
    far_face: f32,
    facet: f32,
    seam: f64,
    ember: Option<f64>,
}

impl Column {
    pub(super) fn new(x: f64) -> Self {
        // Unclipped angular peaks replace the huge rectangular mesas. Two
        // recessed silhouettes share world coordinates and camera speed.
        let profile = |span: f64, salt: u32, base: f64, height: f64| {
            let cell = (x / span).floor() as i32;
            let mut ridge = base;
            let mut face = 1.0;
            for candidate in cell - 1..=cell + 1 {
                let seed = hash(candidate, salt);
                let center = (candidate as f64 + 0.5) * span + f64::from(seed % 7) - 3.0;
                let dx = x - center;
                let radius = span * (0.35 + f64::from((seed >> 4) % 8) * 0.025);
                let peak = (1.0 - dx.abs() / radius).clamp(0.0, 1.0);
                let top = base - peak * (height + f64::from((seed >> 12) % 9));
                if top < ridge {
                    ridge = top;
                    face = if dx < 0.0 { 1.0 } else { 0.72 };
                }
            }
            (ridge, face)
        };
        let (ridge, face) = profile(11.0, 0x6273_18ac, 74.0, 11.0);
        let (far_ridge, far_face) = profile(16.0, 0x5829_a731, 69.0, 15.0);
        let ledge = f64::from(hash((x * 2.0).round() as i32, 0x721a_b891) % 3) * 0.35;
        let ember_cell = (x * 2.0).floor() as i32;
        let ember_seed = hash(ember_cell, 0x4f72_d9a1);
        Self {
            ridge: ridge + ledge,
            far_ridge: far_ridge + ledge * 0.5,
            face,
            far_face,
            facet: 0.92 + (hash((x / 1.5).round() as i32, 0x1426_27ac) % 7) as f32 * 0.02,
            seam: x * 0.35,
            ember: ember_seed
                .is_multiple_of(71)
                .then_some(f64::from(ember_seed % 47)),
        }
    }

    pub(super) fn pixel(&self, y: f64, time: f64) -> (u8, u8, u8) {
        let depth = ((y - 12.0) / 75.0).clamp(0.0, 1.0) as f32;
        let mut color = Renderer::lerp_rgb((29, 18, 32), (91, 39, 37), depth);
        if y >= self.far_ridge {
            let haze = ((y - self.far_ridge) / 32.0).clamp(0.0, 1.0) as f32;
            color = Renderer::lerp_rgb((43, 27, 38), (72, 34, 37), haze);
            color = Renderer::scale_rgb(color, (0.82 + self.far_face * 0.18) * self.facet);
            if (y + self.seam * 0.4).rem_euclid(3.0) < 0.20 {
                color = Renderer::scale_rgb(color, 0.88);
            }
        }
        if y >= self.ridge {
            let haze = ((y - self.ridge) / 22.0).clamp(0.0, 1.0) as f32;
            color = Renderer::lerp_rgb((38, 26, 36), (64, 31, 35), haze);
            if y - self.ridge < 0.3 {
                color = (66, 39, 45);
            } else {
                color = Renderer::scale_rgb(color, (0.76 + self.face * 0.24) * self.facet);
            }
            // Sloped seams cannot be mistaken for a horizontal walking ledge.
            if (y + self.seam).rem_euclid(8.0) < 0.18 {
                color = Renderer::lerp_rgb(color, (88, 44, 43), 0.18);
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
