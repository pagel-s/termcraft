//! Authored terminal-pixel dragon. Wings animate around a fixed body/combat anchor.
//! Rasterized at compile time; no allocation or image decoding in the draw loop.
pub(super) const WIDTH: usize = 64;
pub(super) const HEIGHT: usize = 40;
pub(super) type Frame = [[u8; WIDTH]; HEIGHT];

static FRAMES: [Frame; 7] = build_frames();
static FRONT_FRAMES: [Frame; 7] = build_front_frames();

pub(super) fn frame(time: f64, diving: bool) -> &'static Frame {
    // Twelve poses per cycle, with smooth turnaround at each wing extreme.
    let rate = if diving { 0.85 } else { 0.55 };
    let phase = (time * rate).floor() as usize % 12;
    &FRAMES[if phase <= 6 { phase } else { 12 - phase }]
}

pub(super) fn front_frame(time: f64, diving: bool) -> &'static Frame {
    let rate = if diving { 0.85 } else { 0.55 };
    let phase = (time * rate).floor() as usize % 12;
    &FRONT_FRAMES[if phase <= 6 { phase } else { 12 - phase }]
}

const fn dot(image: &mut Frame, x: i32, y: i32, token: u8) {
    if x >= 0 && y >= 0 && x < WIDTH as i32 && y < HEIGHT as i32 {
        image[y as usize][x as usize] = token;
    }
}

const fn line(image: &mut Frame, from: (i32, i32), to: (i32, i32), token: u8) {
    let (mut x, mut y) = from;
    let dx = (to.0 - x).abs();
    let dy = -(to.1 - y).abs();
    let sx = if x < to.0 { 1 } else { -1 };
    let sy = if y < to.1 { 1 } else { -1 };
    let mut error = dx + dy;
    loop {
        dot(image, x, y, token);
        if x == to.0 && y == to.1 {
            break;
        }
        let twice = error * 2;
        if twice >= dy {
            error += dy;
            x += sx;
        }
        if twice <= dx {
            error += dx;
            y += sy;
        }
    }
}

const fn polygon(image: &mut Frame, points: &[(i32, i32)], token: u8) {
    // Scanline even/odd fill, so compile-time work scales with the edges and
    // painted area rather than testing every polygon at every canvas pixel.
    let mut y = 0;
    while y < HEIGHT as i32 {
        let mut crossings = [0; 16];
        let mut count = 0;
        let mut previous = points.len() - 1;
        let mut current = 0;
        while current < points.len() {
            let a = points[current];
            let b = points[previous];
            if (a.1 > y) != (b.1 > y) {
                let crossing = a.0 + (y - a.1) * (b.0 - a.0) / (b.1 - a.1);
                let mut index = count;
                while index > 0 && crossings[index - 1] > crossing {
                    crossings[index] = crossings[index - 1];
                    index -= 1;
                }
                crossings[index] = crossing;
                count += 1;
            }
            previous = current;
            current += 1;
        }
        let mut pair = 0;
        while pair + 1 < count {
            let mut x = crossings[pair];
            while x < crossings[pair + 1] {
                dot(image, x, y, token);
                x += 1;
            }
            pair += 2;
        }
        y += 1;
    }
}

const fn outlined(image: &mut Frame, points: &[(i32, i32)], fill: u8, edge: u8) {
    polygon(image, points, fill);
    let mut i = 0;
    while i < points.len() {
        line(image, points[i], points[(i + 1) % points.len()], edge);
        i += 1;
    }
}

const fn build_frames() -> [Frame; 7] {
    let mut frames = [[[b' '; WIDTH]; HEIGHT]; 7];
    let mut pose = 0;
    while pose < frames.len() {
        let art = &mut frames[pose];
        // Ease the tip around a fixed shoulder. Far and near wings have
        // different spans, colors and scallops, giving a side-on projection.
        let tip_y = [3, 4, 7, 12, 18, 23, 25][pose];
        let tip_x = [22, 20, 17, 14, 12, 13, 16][pose];
        outlined(
            art,
            &[
                (41, 26),
                (48, tip_y + 3),
                (58, tip_y + 1),
                (55, tip_y + 8),
                (52, tip_y + 8),
                (48, 24),
                (45, 24),
            ],
            b'j',
            b'o',
        );
        line(art, (41, 26), (48, tip_y + 3), b'z');
        line(art, (48, tip_y + 3), (58, tip_y + 1), b'z');
        line(art, (48, tip_y + 3), (52, tip_y + 8), b'p');
        outlined(
            art,
            &[
                (36, 27),
                (29, 16 + tip_y / 3),
                (tip_x, tip_y),
                (tip_x - 7, tip_y + 9),
                (tip_x - 3, tip_y + 8),
                (tip_x - 2, tip_y + 14),
                (tip_x + 3, tip_y + 12),
                (28, 27),
                (31, 24),
            ],
            b'v',
            b'o',
        );
        line(art, (36, 27), (29, 16 + tip_y / 3), b'i');
        line(art, (29, 16 + tip_y / 3), (tip_x, tip_y), b'i');
        line(art, (tip_x, tip_y), (tip_x - 7, tip_y + 9), b'z');
        line(art, (29, 16 + tip_y / 3), (tip_x - 2, tip_y + 14), b'z');
        line(art, (29, 16 + tip_y / 3), (28, 27), b'z');
        dot(art, tip_x, tip_y - 1, b'h');
        dot(art, tip_x + 1, tip_y - 1, b'h');

        // Tapered tail, with a visible curved tip instead of a flat body bar.
        outlined(
            art,
            &[
                (32, 27),
                (29, 31),
                (22, 33),
                (14, 32),
                (7, 29),
                (2, 30),
                (7, 27),
                (15, 29),
                (23, 30),
                (29, 27),
            ],
            b'c',
            b'o',
        );
        line(art, (7, 28), (15, 30), b's');
        line(art, (15, 30), (23, 31), b's');
        line(art, (23, 31), (30, 28), b's');

        // Fixed torso, neck and feet. Only wings change between frames.
        outlined(
            art,
            &[
                (28, 26),
                (34, 24),
                (43, 24),
                (48, 27),
                (47, 31),
                (40, 33),
                (32, 32),
                (28, 30),
            ],
            b'c',
            b'o',
        );
        polygon(
            art,
            &[(30, 26), (35, 25), (42, 25), (45, 27), (40, 29), (33, 29)],
            b's',
        );
        line(art, (33, 30), (42, 31), b'p');
        outlined(
            art,
            &[(43, 25), (48, 22), (51, 21), (54, 24), (50, 28), (47, 29)],
            b's',
            b'o',
        );
        line(art, (44, 25), (49, 23), b'i');
        polygon(
            art,
            &[(33, 31), (35, 33), (32, 36), (28, 36), (31, 34), (31, 32)],
            b'l',
        );
        polygon(
            art,
            &[(43, 30), (46, 30), (46, 34), (49, 36), (44, 36), (42, 33)],
            b'l',
        );
        line(art, (28, 36), (31, 36), b'h');
        line(art, (45, 36), (49, 36), b'h');

        // Horned wedge head with a snout, nostril, split jaw and pale teeth.
        outlined(
            art,
            &[
                (50, 20),
                (54, 19),
                (58, 20),
                (60, 22),
                (63, 22),
                (63, 25),
                (60, 26),
                (58, 27),
                (53, 26),
                (50, 24),
            ],
            b's',
            b'o',
        );
        polygon(
            art,
            &[(51, 21), (54, 20), (57, 21), (59, 23), (55, 24), (52, 23)],
            b'c',
        );
        line(art, (53, 20), (51, 16), b'h');
        line(art, (57, 20), (58, 17), b'h');
        line(art, (59, 24), (63, 24), b'o');
        dot(art, 60, 25, b'w');
        dot(art, 62, 25, b'w');
        dot(art, 62, 22, b'p');
        line(art, (55, 22), (57, 22), b'e');
        dot(art, 56, 21, b'i');
        // Short dorsal spines remain readable at half resolution.
        line(art, (31, 25), (32, 23), b'h');
        line(art, (36, 24), (37, 22), b'h');
        line(art, (41, 24), (42, 22), b'h');
        pose += 1;
    }
    frames
}

const fn paired_line(image: &mut Frame, from: (i32, i32), to: (i32, i32), token: u8) {
    line(image, from, to, token);
    line(image, (63 - from.0, from.1), (63 - to.0, to.1), token);
}

const fn build_front_frames() -> [Frame; 7] {
    let mut frames = [[[b' '; WIDTH]; HEIGHT]; 7];
    let mut pose = 0;
    while pose < frames.len() {
        let art = &mut frames[pose];
        let tip = [3, 5, 8, 13, 18, 23, 26][pose];
        let elbow = [10, 11, 13, 15, 18, 20, 21][pose];
        let reach = [1, 1, 2, 3, 4, 6, 8][pose];
        let scallop = if tip + 17 > 35 { 35 } else { tip + 17 };
        // Ribbed, scalloped wings wrap around the viewer-facing shoulders.
        // The face/chest remain fixed while the wings fold through seven poses.
        outlined(
            art,
            &[
                (26, 21),
                (19, elbow),
                (reach + 6, tip),
                (reach, tip + 1),
                (reach + 3, tip + 10),
                (reach + 8, tip + 7),
                (reach + 10, scallop),
                (18, 22 + tip / 5),
                (22, 28),
                (27, 24),
            ],
            b'M',
            b'U',
        );
        polygon(
            art,
            &[(26, 21), (19, elbow), (reach + 10, scallop), (22, 28)],
            b'j',
        );
        line(art, (26, 21), (19, elbow), b'K');
        line(art, (19, elbow), (reach + 6, tip), b'K');
        line(art, (reach + 6, tip), (reach, tip + 1), b'h');
        line(art, (19, elbow), (reach + 3, tip + 10), b'D');
        line(art, (19, elbow), (reach + 10, scallop), b'D');
        line(art, (19, elbow), (22, 28), b'D');
        line(art, (18, elbow), (17, elbow - 2), b'h');
        let mut y = 0;
        while y < HEIGHT {
            let mut x = 0;
            while x < WIDTH / 2 {
                art[y][WIDTH - 1 - x] = art[y][x];
                x += 1;
            }
            y += 1;
        }

        // A foreshortened tail and rear haunches give the hovering torso depth.
        outlined(
            art,
            &[(28, 29), (35, 29), (35, 34), (33, 39), (30, 39), (28, 34)],
            b'c',
            b'U',
        );
        line(art, (31, 32), (31, 38), b'V');
        outlined(
            art,
            &[(26, 28), (29, 32), (26, 36), (19, 37), (21, 33)],
            b'Q',
            b'U',
        );
        outlined(
            art,
            &[(37, 28), (34, 32), (37, 36), (44, 37), (42, 33)],
            b'Q',
            b'U',
        );
        paired_line(art, (21, 34), (25, 33), b'K');
        paired_line(art, (20, 36), (20, 37), b'w');
        paired_line(art, (23, 35), (23, 37), b'w');
        paired_line(art, (26, 35), (26, 36), b'w');

        // Broad shoulder armor, overlapping breast plates, and forward talons.
        outlined(
            art,
            &[
                (25, 19),
                (38, 19),
                (41, 25),
                (38, 31),
                (35, 34),
                (28, 34),
                (25, 31),
                (22, 25),
            ],
            b'Q',
            b'U',
        );
        polygon(
            art,
            &[(27, 22), (36, 22), (36, 28), (33, 33), (30, 33), (27, 28)],
            b'c',
        );
        paired_line(art, (25, 23), (29, 25), b'K');
        paired_line(art, (27, 26), (30, 28), b'V');
        paired_line(art, (28, 29), (31, 31), b'K');
        outlined(
            art,
            &[(25, 23), (28, 25), (26, 28), (25, 30), (21, 29), (22, 26)],
            b'Q',
            b'U',
        );
        outlined(
            art,
            &[(38, 23), (35, 25), (37, 28), (38, 30), (42, 29), (41, 26)],
            b'Q',
            b'U',
        );
        paired_line(art, (22, 26), (25, 26), b'K');
        paired_line(art, (22, 28), (22, 30), b'w');
        paired_line(art, (24, 28), (24, 30), b'w');

        // Swept horns frame a heavy wedge skull rather than a flat bat face.
        outlined(
            art,
            &[(25, 13), (23, 10), (20, 3), (25, 6), (28, 11)],
            b'h',
            b'p',
        );
        outlined(
            art,
            &[(38, 13), (40, 10), (43, 3), (38, 6), (35, 11)],
            b'h',
            b'p',
        );
        paired_line(art, (23, 6), (26, 11), b'K');
        outlined(
            art,
            &[
                (25, 10),
                (29, 8),
                (34, 8),
                (38, 10),
                (41, 14),
                (39, 19),
                (35, 23),
                (28, 23),
                (24, 19),
                (22, 14),
            ],
            b'Q',
            b'U',
        );
        polygon(
            art,
            &[(29, 10), (34, 10), (35, 14), (32, 17), (28, 14)],
            b'c',
        );
        paired_line(art, (25, 11), (29, 13), b'K');
        paired_line(art, (24, 13), (29, 15), b'U');
        paired_line(art, (26, 14), (29, 15), b'e');
        paired_line(art, (26, 15), (28, 15), b'e');
        paired_line(art, (24, 16), (26, 18), b'K');

        // Dark open maw, paired fangs, nostrils and a ridged chin.
        polygon(
            art,
            &[
                (25, 17),
                (29, 18),
                (34, 18),
                (38, 17),
                (37, 22),
                (34, 24),
                (29, 24),
                (26, 22),
            ],
            b'X',
        );
        // A lit top plane and darker front give the muzzle forward projection.
        polygon(
            art,
            &[(28, 16), (35, 16), (38, 18), (35, 20), (28, 20), (25, 18)],
            b'Q',
        );
        polygon(art, &[(28, 16), (35, 16), (36, 17), (27, 17)], b'K');
        paired_line(art, (28, 18), (29, 18), b'X');
        paired_line(art, (27, 20), (28, 22), b'w');
        paired_line(art, (29, 23), (29, 24), b'w');
        paired_line(art, (28, 24), (31, 25), b'K');
        pose += 1;
    }
    frames
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flapping_changes_wings_without_jittering_the_body_or_eyes() {
        let raised = frame(0.0, false);
        let lowered = frame(11.0, false);
        assert_ne!(raised, lowered);
        for pose in &FRAMES {
            for (row, expected) in pose[27..30].iter().zip(&raised[27..30]) {
                assert_eq!(&row[31..44], &expected[31..44]);
            }
            assert_eq!(&pose[22][55..58], b"eee");
            assert_eq!(&pose[36][45..50], b"hhhhh");
        }
        assert_eq!(frame(0.0, false), frame(12.0 / 0.55, false));
        assert_ne!(frame(5.0, false), frame(5.0, true));
    }
}
