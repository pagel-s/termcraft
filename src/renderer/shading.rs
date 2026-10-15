//! Smooth presentation lighting; world simulation and mob light rules are unchanged.
use super::{BlockType, Color, GameState, Renderer};
use crate::world::Dimension;

pub(super) struct SceneLighting {
    region: (i32, i32, usize, usize),
    levels: Vec<u8>,
    warm: Vec<u8>,
    ember: Vec<u8>,
    arcane: Vec<u8>,
    daylight: f32,
    ambient: f32,
}

impl SceneLighting {
    pub(super) fn new(state: &GameState, levels: &[u8], region: (i32, i32, usize, usize)) -> Self {
        let (left, top, width, height) = region;
        let mut result = Self {
            region,
            levels: levels.to_vec(),
            warm: vec![0; levels.len()],
            ember: vec![0; levels.len()],
            arcane: vec![0; levels.len()],
            // The End has no sun, but its open arena still needs a readable
            // cool fill. This is presentation only; mob/spawn light is unchanged.
            ambient: if state.current_dimension == Dimension::End {
                0.50
            } else {
                0.0
            },
            daylight: match state.current_dimension {
                Dimension::Overworld => 0.32 + Renderer::day_mix(state.time_of_day) * 0.68,
                Dimension::Nether => 0.75,
                Dimension::End => 0.78,
            },
        };
        let mut attenuation = vec![1; levels.len()];
        for y in 0..height {
            for x in 0..width {
                let i = y * width + x;
                let block = state.world.get_block(left + x as i32, top + y as i32);
                attenuation[i] = if block.is_solid() { 3 } else { 1 };
                match block {
                    BlockType::Torch | BlockType::Glowstone => result.warm[i] = 15,
                    BlockType::Lava(_) => result.ember[i] = 15,
                    BlockType::RedstoneTorch(true) => result.ember[i] = 11,
                    BlockType::PrimedTnt(_) => result.ember[i] = 13,
                    BlockType::NetherPortal => result.arcane[i] = 11,
                    BlockType::EndPortal => result.arcane[i] = 10,
                    _ => {}
                }
            }
        }
        for ball in &state.fireballs {
            let x = ball.x.floor() as i32 - left;
            let y = ball.y.floor() as i32 - top;
            if x >= 0 && y >= 0 && x < width as i32 && y < height as i32 {
                result.ember[y as usize * width + x as usize] = 14;
            }
        }
        let mut frontier = std::array::from_fn(|_| Vec::new());
        for channel in [&mut result.warm, &mut result.ember, &mut result.arcane] {
            if channel.iter().any(|v| *v > 0) {
                super::super::lighting::propagate(channel, &attenuation, width, &mut frontier);
            }
        }
        result
    }

    fn sample(&self, values: &[u8], x: f64, y: f64) -> f32 {
        let (left, top, width, height) = self.region;
        let x = (x - 0.5 - left as f64).clamp(0.0, (width - 1) as f64);
        let y = (y - 0.5 - top as f64).clamp(0.0, (height - 1) as f64);
        let (ix, iy) = (x.floor() as usize, y.floor() as usize);
        let (nx, ny) = ((ix + 1).min(width - 1), (iy + 1).min(height - 1));
        let (fx, fy) = ((x - ix as f64) as f32, (y - iy as f64) as f32);
        let a = values[iy * width + ix] as f32 * (1.0 - fx) + values[iy * width + nx] as f32 * fx;
        let b = values[ny * width + ix] as f32 * (1.0 - fx) + values[ny * width + nx] as f32 * fx;
        a * (1.0 - fy) + b * fy
    }

    pub(super) fn shade(
        &self,
        color: Color,
        x: f64,
        y: f64,
        occlusion: f32,
        emissive: bool,
    ) -> Color {
        let Some(rgb) = Renderer::color_rgb(color) else {
            return color;
        };
        if emissive {
            return color;
        }
        let warm = self.sample(&self.warm, x, y);
        let ember = self.sample(&self.ember, x, y);
        let arcane = self.sample(&self.arcane, x, y);
        let emitted = warm.max(ember).max(arcane);
        let scalar = self.sample(&self.levels, x, y);
        let level = ((scalar * self.daylight).max(emitted) / 15.0).max(self.ambient);
        let exposure = (0.18 + 0.82 * level.clamp(0.0, 1.0).powf(1.12)) * occlusion;
        let mut lit = Renderer::scale_rgb(rgb, exposure);
        // Cool ambient shadows and restrained local color, shared by terrain and actors.
        let cool = (1.0 - level).clamp(0.0, 1.0) * 0.12;
        lit = Renderer::lerp_rgb(lit, (lit.0 / 2, lit.1, lit.2.saturating_add(8)), cool);
        let sum = warm + ember + arcane;
        if sum > 0.0 {
            let tint = (
                (warm * 255.0 + ember * 255.0 + arcane * 190.0) / sum,
                (warm * 210.0 + ember * 145.0 + arcane * 147.0) / sum,
                (warm * 155.0 + ember * 100.0 + arcane * 255.0) / sum,
            );
            let strength = (emitted / (scalar + 1.0)).clamp(0.0, 1.0) * 0.72;
            lit = (
                (lit.0 as f32 * (1.0 - strength + strength * tint.0 / 255.0)) as u8,
                (lit.1 as f32 * (1.0 - strength + strength * tint.1 / 255.0)) as u8,
                (lit.2 as f32 * (1.0 - strength + strength * tint.2 / 255.0)) as u8,
            );
        }
        Renderer::rgb(lit)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn light_sampling_is_continuous_and_bounded_across_tile_edges() {
        let l = SceneLighting {
            region: (-1, -1, 2, 2),
            levels: vec![0, 15, 0, 15],
            warm: vec![0; 4],
            ember: vec![0; 4],
            arcane: vec![0; 4],
            daylight: 1.0,
            ambient: 0.0,
        };
        let a = l.sample(&l.levels, -0.001, 0.0);
        let b = l.sample(&l.levels, 0.001, 0.0);
        assert!((a - b).abs() < 0.04);
        assert_eq!(l.sample(&l.levels, -100.0, 100.0), 0.0);
        assert_eq!(l.sample(&l.levels, 100.0, -100.0), 15.0);
    }
    #[test]
    fn end_fill_keeps_dark_materials_readable_without_brightening_overworld_caves() {
        let mut state = GameState::network_view();
        let dark = Renderer::rgb((76, 65, 94));
        let cave = SceneLighting::new(&state, &[0; 9], (0, 0, 3, 3));
        state.current_dimension = Dimension::End;
        let end = SceneLighting::new(&state, &[0; 9], (0, 0, 3, 3));
        let a = Renderer::color_rgb(cave.shade(dark, 1.5, 1.5, 1.0, false)).unwrap();
        let b = Renderer::color_rgb(end.shade(dark, 1.5, 1.5, 1.0, false)).unwrap();
        assert!(b.0 >= 40 && b.2 >= 50);
        assert!(a.0 < 20 && b.0 > a.0 * 2);
    }
    #[test]
    fn local_light_is_warm_and_occluded_while_emission_keeps_its_color() {
        let mut state = GameState::network_view();
        let blocks = vec![
            BlockType::Air;
            crate::world::chunk::CHUNK_WIDTH * crate::world::chunk::CHUNK_HEIGHT
        ];
        state.world.apply_chunk_column_snapshot(0, &blocks);
        state.world.set_block(3, 6, BlockType::Stone);
        state.world.set_block(3, 5, BlockType::Torch);
        for y in 0..12 {
            state.world.set_block(5, y, BlockType::Stone);
        }
        let lighting = SceneLighting::new(&state, &[10; 144], (0, 0, 12, 12));
        assert!(
            lighting.sample(&lighting.warm, 4.5, 5.5) > lighting.sample(&lighting.warm, 6.5, 5.5)
        );
        let white = Color::Rgb {
            r: 180,
            g: 180,
            b: 180,
        };
        let (r, _, b) = Renderer::color_rgb(lighting.shade(white, 3.5, 5.5, 1.0, false)).unwrap();
        assert!(r > b);
        assert_eq!(lighting.shade(white, 0.5, 0.5, 0.2, true), white);
    }
}
