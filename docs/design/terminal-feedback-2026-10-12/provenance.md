# Native terminal feedback correction, 12 October 2026

Development correction after `8c3fe4e4c701616a63879889b6f34cb25ac22526`.
The human report contradicted earlier quality confidence: endless movement, poor
sky, broken-feeling interactions, a long swing, moving layers and excessive zoom.
Earlier PTY checks used ideal key-up streams; stationary profiles missed scrolling
ANSI volume. This pass corrects concrete causes and leaves human acceptance open.

## Reproduced causes and fixes

A terminal that replied to the keyboard capability query but sent one plain ASCII
`d` kept walking beyond 1.4 seconds. Support was assumed before receiving a real
release, disabling all stale-input timeouts. Native and co-op now confirm actual
movement/jump key-up, retain bounded fallback beforehand, and apply a two-second
watchdog afterward. Repeat events renew holds; focus loss clears them. An unrelated
key release cannot promote input. Co-op repeats reactivate an expired fallback hold.
The first unconfirmed long hold can pause before OS repeat; legacy tap/hold
ambiguity remains. This is bounded input, not universal key-release support.

Native right-drag repeatedly used the hovered block, which could re-close a door
immediately after opening it. World use now happens once per right-button press;
inventory drag distribution is retained. Target feedback distinguishes obstruction
and fixture use. The loft regression now opens chest/furnace and uses beds from
the ladder with Shift, without qualifying access by jumping across furniture tops.

The renderer emitted changing camera rows without synchronized-update brackets.
One system write does not guarantee one emulator presentation. Frames now request
synchronized display and restoration ends the mode. Consecutive ANSI color runs
avoid redundant cursor moves; uniform pixel halves and invisible foregrounds are
normalized. Both native/client loops pace after flush completion. Supporting
emulators should avoid partial-frame painting; actual latency remains unmeasured.

The default 160×48 pixel view now shows 40 blocks across rather than 20. F4 cycles
overview → wide glyphs → optional detailed pixels. Sky art uses restrained designed
clouds, a stepped disc and stable stars in one screen coordinate system; the noisy
cloud field and foreground-derived distant ridge are removed. Screen-space sky is
intentionally simple and has no camera parallax. Short shaded sleeve/skin arms and
small held-tool extensions replace the long white swing rod and burst.

## Evidence and reproduction

[Gallery](index.html): sixteen frames. Sky time/player poses and action timers are
staged in fixed disposable terrain. The village uses ordinary generation; other
world samples stage geometry. Native door frames come from actual keyboard/mouse
input in copied generated saves. All PNGs are software rasterized ANSI, not emulator
screenshots or generated concept art. Historical galleries retain their old builds.

```sh
cargo build --release --locked --bins --examples
python3 scripts/test_terminal_input.py --binary target/release/termcraft --village-example target/release/examples/village_review --report /tmp/terminal-input.json --keep-artifacts
env -u NO_COLOR target/release/examples/sky_review /tmp/sky-ansi
python3 scripts/capture_terminal_art.py /tmp/sky-ansi /tmp/sky-png
env -u NO_COLOR target/release/examples/motion_profile > /tmp/moving-output.json
```

The retained PTY test first advertises protocol support but sends plain taps with
no release. It checks stopping, unrelated releases, confirmed holds, dropped key-up,
repeat renewal, focus loss and terminal restoration in native/co-op modes. Each
of three views receives right-click down, three drag events and release, then
actually walks through the door. Reports record synchronized frame boundaries.
SSH/archive smoke header detection now ignores ANSI styles instead of assuming a
visible phrase is encoded as one raw byte run. A fresh full-village fixture avoids
using the review save that the user has since played.

[Exact source, binary, validation and moving measurements](../../assets/terminal-feedback-2026-10-12.json)
record 636 tests on stable/Rust 1.88, strict Clippy, TCP, native/co-op input, village
walking, SSH and Lantern Run. Moving output is measured with color enabled using
synthetic camera pans and a counting writer. It does not measure kernel/emulator
latency or achieved FPS; estimated bandwidth is stated separately from CPU time.
The native archive has a separate qualification/hash report in `dist`.

## Open quality gates

The corrective capture review found no new blocker, but nighttime/cave figures
remain dark and wide figures retain glyph rendering. Real emulator smoothness,
repeat-only input feel, unsupported synchronization, real WAN, clean Windows/macOS,
full survival and the user's overall art/feel acceptance remain unverified. Existing
saves are preserved. Player collision/land tuning, protocol 12 and progression v7
are unchanged. This is a concrete feedback correction, not a completed quality target.
