use super::*;

#[test]
fn biome_cache_preserves_samples_across_worlds_and_coordinate_aliases() {
    for seed in [0u32, 1337, 42, 1337, u32::MAX, 0] {
        let temp = Perlin::new(seed);
        let moist = Perlin::new(seed.wrapping_add(31));
        for x in [-4097, -513, -512, -511, -1, 0, 1, 511, 512, 513, 4097] {
            assert_eq!(
                World::biome_for_x(&temp, &moist, x),
                World::sample_biome_for_x(&temp, &moist, x)
            );
            // Repeated queries exercise the cached result as well as aliases.
            assert_eq!(
                World::biome_for_x(&temp, &moist, x),
                World::sample_biome_for_x(&temp, &moist, x)
            );
        }
    }
}

#[test]
fn overworld_travel_has_continuous_land_and_gentle_inland_grades() {
    let mut failures = Vec::new();
    for seed in [1337, 1, 42, 8675309, 2026, 13579, 73, 99991] {
        let terrain = Perlin::new(seed);
        let temp = Perlin::new(seed.wrapping_add(5994));
        let moist = Perlin::new(seed.wrapping_add(8580));
        let mut wet = 0;
        let mut crossings = 0;
        let mut land_run = 0;
        let mut longest_land = 0;
        let mut dry_steps = 0;
        let mut steep_steps = 0;
        let mut worst_step = 0;
        let mut previous: Option<(i32, bool)> = None;
        for x in -4096..4096 {
            let biome = World::biome_for_x(&temp, &moist, x);
            let y = World::blended_overworld_surface_y(&terrain, &temp, &moist, x, biome);
            let water = World::column_has_surface_water(&terrain, &temp, &moist, x, biome, y);
            if !World::is_surface_water_biome(biome) && !water {
                assert!(
                    y <= OVERWORLD_SEA_LEVEL,
                    "inland ground below the shared water table creates perched rivers: seed={seed} x={x} y={y}"
                );
            }
            if water {
                wet += 1;
                land_run = 0;
                if previous.is_some_and(|(_, was_wet)| !was_wet) {
                    crossings += 1;
                }
            } else {
                land_run += 1;
                longest_land = longest_land.max(land_run);
                if let Some((last_y, false)) = previous {
                    let step = (y - last_y).abs();
                    dry_steps += 1;
                    steep_steps += usize::from(step > 1);
                    worst_step = worst_step.max(step);
                    if step > 3 && worst_step == step {
                        println!("abrupt x={x} biome={biome:?} from={last_y} to={y}");
                    }
                }
            }
            previous = Some((y, water));
        }
        println!(
            "seed={seed} water={wet}/8192 crossings={crossings} longest_land={longest_land} steep={steep_steps}/{dry_steps} worst_step={worst_step}"
        );
        if wet * 100 > 8192 * 25
            || crossings > 35
            || longest_land < 384
            || steep_steps * 100 > dry_steps
            || worst_step > 3
        {
            failures.push(seed);
        }
    }
    assert!(
        failures.is_empty(),
        "fragmented or abrupt travel terrain: {failures:?}"
    );
}
