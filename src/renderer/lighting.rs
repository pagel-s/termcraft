//! Descending light propagation. Each cell is finalized at its brightest level.
pub(super) fn propagate(
    light: &mut [u8],
    attenuation: &[u8],
    width: usize,
    frontier: &mut [Vec<usize>; 16],
) {
    if width < 3 || light.len() / width < 3 {
        return;
    }
    for bucket in frontier.iter_mut() {
        bucket.clear();
    }
    for (index, &level) in light.iter().enumerate() {
        if level > 1 {
            frontier[level as usize].push(index);
        }
    }
    for level in (2..=15).rev() {
        while let Some(index) = frontier[level].pop() {
            if light[index] as usize != level {
                continue;
            }
            let x = index % width;
            for next in [
                index.checked_sub(width),
                (index + width < light.len()).then_some(index + width),
                (x > 0).then(|| index - 1),
                (x + 1 < width).then_some(index + 1),
            ]
            .into_iter()
            .flatten()
            {
                // The previous relaxation left the outer border fixed as sources.
                if next < width
                    || next >= light.len() - width
                    || next % width == 0
                    || next % width == width - 1
                {
                    continue;
                }
                let candidate = (level as u8).saturating_sub(attenuation[next]);
                if candidate > light[next] {
                    light[next] = candidate;
                    if candidate > 1 {
                        frontier[candidate as usize].push(next);
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn priority_lighting_matches_full_relaxation_with_occluders_and_border_sources() {
        let mut seed = 42u64;
        for (width, height) in [(3, 3), (19, 27), (113, 82)] {
            let mut light = vec![0; width * height];
            let mut drop = vec![1; light.len()];
            for index in 0..light.len() {
                seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
                if seed.is_multiple_of(7) {
                    light[index] = ((seed >> 32) % 16) as u8;
                }
                if seed.is_multiple_of(3) {
                    drop[index] = 3;
                }
            }
            let mut expected = light.clone();
            for _ in 0..18 {
                let mut next = expected.clone();
                for y in 1..height - 1 {
                    for x in 1..width - 1 {
                        let i = y * width + x;
                        next[i] = next[i].max(
                            expected[i - 1]
                                .max(expected[i + 1])
                                .max(expected[i - width])
                                .max(expected[i + width])
                                .saturating_sub(drop[i]),
                        );
                    }
                }
                expected = next;
            }
            propagate(
                &mut light,
                &drop,
                width,
                &mut std::array::from_fn(|_| Vec::new()),
            );
            assert_eq!(light, expected, "{width}x{height}");
        }
    }
}
