//! World-anchored falling precipitation, independent of decorative sky horizons.
use super::{BlockType, GameState, PrecipitationType, Renderer};

impl Renderer {
    pub(super) fn local_weather_label(state: &GameState) -> &'static str {
        use crate::engine::state::WeatherType;
        match (
            state.weather,
            state.precipitation_at(state.player.x.floor() as i32),
        ) {
            (WeatherType::Clear, _) => "Clear",
            (WeatherType::Rain, PrecipitationType::Snow) => "Snow",
            (WeatherType::Rain, PrecipitationType::Rain) => "Rain",
            (WeatherType::Rain, PrecipitationType::None) => "Overcast",
            (WeatherType::Thunderstorm, PrecipitationType::Snow) => "Snowstorm",
            (WeatherType::Thunderstorm, _) => "Thunderstorm",
        }
    }

    pub(super) fn precipitation_cutoff(state: &GameState, x: i32) -> i32 {
        // Include roofs outside the camera, thin ceilings, glass and canopies.
        // Rain also ends at the surface of a body of water, never below it.
        (0..crate::world::chunk::CHUNK_HEIGHT as i32)
            .find(|&y| {
                let b = state.world.get_block(x, y);
                b.is_solid()
                    || b.is_leaf_block()
                    || matches!(b, BlockType::Water(_) | BlockType::Lava(_))
            })
            .unwrap_or(crate::world::chunk::CHUNK_HEIGHT as i32)
    }

    pub(super) fn precipitation_pixel(
        kind: PrecipitationType,
        position: (f64, f64),
        age: f64,
        intensity: f32,
        pixel_size: f64,
    ) -> Option<((u8, u8, u8), f32)> {
        let (speed, spacing, period, drift) = match kind {
            PrecipitationType::Rain => (0.32, 3.0, 10.0, 0.015),
            PrecipitationType::Snow => (0.065, 2.8, 9.0, 0.008),
            PrecipitationType::None => return None,
        };
        if intensity <= 0.05 {
            return None;
        }
        let x = position.0 - age * drift;
        let column = (x / spacing).floor() as i64;
        let seed = column
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        if seed.rem_euclid(100) as f32 > 20.0 + intensity.clamp(0.0, 1.0) * 80.0 {
            return None;
        }
        let offset = 0.4 + seed.rem_euclid(997) as f64 / 997.0 * (spacing - 0.8);
        let dx = x - (column as f64 * spacing + offset);
        let phase = seed.div_euclid(997).rem_euclid(991) as f64 / 991.0 * period;
        // Positive world y points DOWN. Subtract a head that advances downward.
        let dy = (position.1 - age * speed - phase).rem_euclid(period);
        match kind {
            PrecipitationType::Rain => {
                let width = (pixel_size * 0.5).max(0.065);
                (dx.abs() <= width && (dy < pixel_size * 0.5 || dy > period - 0.65))
                    .then_some(((142, 184, 216), 0.72))
            }
            PrecipitationType::Snow => {
                let radius = (pixel_size * 0.6).max(0.10);
                let dy = dy.min(period - dy);
                (dx * dx + dy * dy <= radius * radius).then_some(((232, 240, 245), 0.94))
            }
            PrecipitationType::None => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        engine::state::WeatherType,
        world::{
            BiomeType,
            chunk::{CHUNK_HEIGHT, CHUNK_WIDTH},
        },
    };
    use crossterm::style::Color;
    use std::collections::HashSet;

    fn fixture(snow: bool, roof: Option<BlockType>, floor: bool) -> GameState {
        let mut s = GameState::network_view();
        s.dismiss_startup_splash();
        for cx in -3..=3 {
            s.world
                .apply_chunk_column_snapshot(cx, &vec![BlockType::Air; CHUNK_WIDTH * CHUNK_HEIGHT]);
            s.world.apply_replica_biomes(
                cx,
                &[if snow {
                    BiomeType::Tundra
                } else {
                    BiomeType::Plains
                }; CHUNK_WIDTH],
            );
        }
        for x in -80..=80 {
            if floor {
                for y in 64..CHUNK_HEIGHT as i32 {
                    s.world.set_block(x, y, BlockType::Stone);
                }
            }
            if x >= 0
                && let Some(b) = roof
            {
                s.world.set_block(x, 10, b);
            }
        }
        s.player.x = 0.5;
        s.player.y = 64.0;
        s.player.grounded = true;
        s.time_of_day = 12000.0;
        s.weather_rain_intensity = 1.0;
        s.weather = WeatherType::Rain;
        s
    }

    fn rendered(s: &GameState, zoom: u8) -> Renderer {
        let mut r = Renderer::with_size(160, 48);
        r.zoom = zoom;
        r.render_to(s, 0.0, &mut Vec::new()).unwrap();
        r
    }

    fn pixels(r: &Renderer) -> Vec<Color> {
        let mut p = vec![Color::Reset; 160 * 88];
        for y in 0..44 {
            for x in 0..160 {
                let c = r.back_buffer[y * 160 + x];
                let (top, bottom) = match c.ch {
                    '▀' => (c.fg, c.bg),
                    '▄' => (c.bg, c.fg),
                    '█' => (c.fg, c.fg),
                    _ => (c.bg, c.bg),
                };
                p[y * 2 * 160 + x] = top;
                p[(y * 2 + 1) * 160 + x] = bottom;
            }
        }
        p
    }

    fn precipitation_mask(s: &mut GameState, zoom: u8) -> (Renderer, HashSet<(i32, i32)>) {
        s.weather = WeatherType::Rain;
        let wet = rendered(s, zoom);
        let a = pixels(&wet);
        s.weather = WeatherType::Clear;
        let dry = rendered(s, zoom);
        let b = pixels(&dry);
        s.weather = WeatherType::Rain;
        let mask = a
            .iter()
            .zip(b)
            .enumerate()
            .filter_map(|(i, (a, b))| (*a != b).then_some(((i % 160) as i32, (i / 160) as i32)))
            .collect();
        (wet, mask)
    }

    #[test]
    fn rendered_rain_and_snow_stop_at_roofs_above_the_view_in_all_color_views() {
        for snow in [false, true] {
            for roof in [
                BlockType::Stone,
                BlockType::Planks,
                BlockType::Glass,
                BlockType::StoneSlab,
                BlockType::Leaves,
            ] {
                let mut s = fixture(snow, Some(roof), true);
                for zoom in [0, 1, 2] {
                    let (r, mask) = precipitation_mask(&mut s, zoom);
                    let header: String = r.back_buffer[..160].iter().map(|c| c.ch).collect();
                    assert!(header.contains(if snow { "| Snow |" } else { "| Rain |" }));
                    let (vx, vy, cx, cy, scale) = r.world_view.unwrap();
                    let unit = (scale * 2) as f64;
                    let position = |(x, y): (i32, i32)| {
                        (
                            (x + vx - cx) as f64 / unit,
                            (y + vy * 2 + r.vertical_pixel_offset - cy * 2) as f64 / unit,
                        )
                    };
                    let covered = mask
                        .iter()
                        .filter(|&&p| {
                            let (x, y) = position(p);
                            (2.0..8.0).contains(&x) && (12.0..62.0).contains(&y)
                        })
                        .count();
                    let outside = mask
                        .iter()
                        .filter(|&&p| {
                            let (x, y) = position(p);
                            (-8.0..-2.0).contains(&x) && (12.0..62.0).contains(&y)
                        })
                        .count();
                    assert_eq!(
                        covered, 0,
                        "precipitation under {roof:?}, snow={snow}, scale={scale}"
                    );
                    assert!(
                        outside > 0,
                        "precipitation missing outdoors: {roof:?}, snow={snow}, scale={scale}"
                    );
                }
            }
        }
    }

    #[test]
    fn rendered_particles_move_downward_in_every_color_view() {
        for snow in [false, true] {
            for zoom in [0, 1, 2] {
                let mut s = fixture(snow, None, false);
                s.player.age = 0;
                let (r, before) = precipitation_mask(&mut s, zoom);
                // Keep the faster rain within the small detail viewport.
                let elapsed = if snow { 12 } else { 4 };
                s.player.age = elapsed;
                let (_, after) = precipitation_mask(&mut s, zoom);
                let scale = r.world_view.unwrap().4;
                let dx = (elapsed as f64 * if snow { 0.008 } else { 0.015 } * f64::from(scale * 2))
                    .round() as i32;
                let dy = (elapsed as f64 * if snow { 0.065 } else { 0.32 } * f64::from(scale * 2))
                    .round() as i32;
                let mut down = 0;
                let mut up = 0;
                for &(x, y) in &before {
                    if !(4..136).contains(&x) || y < dy + 4 || y + dy >= 80 {
                        continue;
                    }
                    let matches = |sign| {
                        (-1..=1).any(|ox| {
                            (-1..=1).any(|oy| after.contains(&(x + dx + ox, y + sign * dy + oy)))
                        })
                    };
                    down += usize::from(matches(1));
                    up += usize::from(matches(-1));
                }
                assert!(
                    down > 0 && down > up,
                    "snow={snow} scale={scale} down={down} up={up}"
                );
            }
        }
    }
}
