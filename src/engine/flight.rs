//! Gravity-free motion for full flying bodies against the same terrain shapes as walkers.
use super::*;

impl GameState {
    pub(super) fn move_flying_body(
        &self,
        position: (f64, f64),
        velocity: (f64, f64),
        dimensions: (f64, f64),
        bounce: f64,
    ) -> (f64, f64, f64, f64) {
        let (half, height) = dimensions;
        let (x, y, vx, vy, _, _) = self.resolve_entity_overlap(
            position.0, position.1, position.0, position.1, velocity.0, velocity.1, half, height,
            false,
        );
        let (nx, hit_x) = self.sweep_flying_axis((x, y), vx, dimensions, true);
        let (ny, hit_y) = self.sweep_flying_axis((nx, y), vy, dimensions, false);
        (
            nx,
            ny,
            if hit_x { -vx * bounce } else { vx },
            if hit_y { -vy * bounce } else { vy },
        )
    }

    fn sweep_flying_axis(
        &self,
        position: (f64, f64),
        delta: f64,
        dimensions: (f64, f64),
        horizontal: bool,
    ) -> (f64, bool) {
        const EPS: f64 = 0.00001;
        let (x, y) = position;
        let (half, height) = dimensions;
        let start = if horizontal { x } else { y };
        if delta == 0.0 {
            return (start, false);
        }
        let end = start + delta;
        let (left, right, top, bottom) = if horizontal {
            (x.min(end) - half, x.max(end) + half, y - height, y)
        } else {
            (x - half, x + half, y.min(end) - height, y.max(end))
        };
        let mut stop = end;
        for bx in left.floor() as i32..=right.floor() as i32 {
            for by in top.floor() as i32..=bottom.floor() as i32 {
                if !self.block_has_entity_collision_at(bx, by) {
                    continue;
                }
                for b in self.world.get_block(bx, by).collision_boxes() {
                    let (l, r, t, bottom) = (
                        bx as f64 + b[0],
                        bx as f64 + b[2],
                        by as f64 + b[1],
                        by as f64 + b[3],
                    );
                    let (lo, hi) = if horizontal {
                        if y <= t + EPS || y - height >= bottom - EPS {
                            continue;
                        }
                        (l - half, r + half)
                    } else {
                        if x + half <= l + EPS || x - half >= r - EPS {
                            continue;
                        }
                        (t, bottom + height)
                    };
                    if delta > 0.0 && start <= lo + EPS && stop > lo - EPS {
                        stop = stop.min((lo - EPS).max(start));
                    } else if delta < 0.0 && start >= hi - EPS && stop < hi + EPS {
                        stop = stop.max((hi + EPS).min(start));
                    }
                }
            }
        }
        (stop, (stop - end).abs() > EPS * 0.5)
    }
}
