# Run/jump continuity and sheltered weather, 15 October 2026

Correction after `7d061c6dcd48a1cfd8237c1c5b0d6d663359dc01` following a human
report that run/jump/continue failed, precipitation moved backward and appeared
in caves, and figures seemed different. [Gallery and weather clips](index.html)
use actual renderer ANSI, software rasterized. Geometry, weather and animation
ages are staged in disposable worlds, not user saves or emulator screenshots.

## Input cause and tradeoff

OS repeat can switch from a held D/A key to Space, and may not resume the original
key after Space-up. The old 96 ms fallback and two-second confirmed per-key timeout
cancelled direction even while it was physically held, which also disabled sprint
and applied release braking in midair. A longer timeout would only postpone it.

Actual control key-up now confirms stateful holds: matching release, focus loss,
menus and normal session resets clear them. The TCP connection/session safety
path is unchanged. No finite silence timeout can distinguish a held key from an
individually missing release; confirmed streams now require another release or
focus change in that rare case. The old dropped-release watchdog guarantee is
superseded, not retained alongside an incompatible hold guarantee.

Before confirmation, ASCII taps still expire. Established repeat movement gets
a 750 ms jump grace. Stale timestamps cannot acquire grace even when queued
events run before stale cleanup after a frame stall. Only fresh directional input can re-arm it for a later jump.
Space alone cannot extend the grace or revive stopped movement. Truly reliable arbitrary chords still
require terminal key-up reporting. The PTY test exercises both modes through
actual local/co-op input and checks physical jump height, continued travel,
landing, actual direction release, focus loss, restoration and mouse doors.

The full suite also exposed an existing population-bootstrap fixture discovering
unrelated villages in randomly generated terrain. It now uses the existing isolated
village test world; its assertions are unchanged. This changes no game behavior.

## Weather cause and fix

The old pixel hash added time to vertical position, moving bands upward. It used
the decorative terrain horizon as shelter, so artificial/thin roofs and cave
ceilings could leak weather. Wide color rendering changed the cell to a sky
half-block before a space-only precipitation condition, suppressing weather there.

One world-coordinate field now samples downward rain streaks and slower snow
flakes using tick age plus frame interpolation. Full-height column cover checks
are independent of the decorative sky horizon and include glass, slabs, foliage
and fluid surfaces. Above-viewport ceilings still stop precipitation. The same
field feeds overview, detail, wide color and monochrome. Snowy biomes display
`Snow` or `Snowstorm` in the HUD rather than `Rain`.

Renderer tests compare actual wet/clear frames with identical sky/lighting mixes,
require outdoor precipitation and none beneath five roof materials at all three
color scales, and compare temporal particle positions to verify downward motion.
Neither this pass nor the torch checkpoint changed actor silhouettes, palettes,
dimensions or animations. F4 changes resolution and uses glyph figures in wide
view, which remains visually different from the pixel views.

```sh
cargo test --locked
cargo clippy --locked --all-targets -- -D warnings
cargo build --release --locked --bins --examples
python3 scripts/test_terminal_input.py --binary target/release/termcraft --village-example target/release/examples/village_review --report /tmp/run-jump.json --keep-artifacts
env -u NO_COLOR target/release/examples/weather_review /tmp/weather-ansi
python3 scripts/capture_terminal_art.py /tmp/weather-ansi /tmp/weather-png
```

[Exact source, binary and validation](../../assets/run-jump-weather-2026-10-15.json)
record the completed checks. Weather clips play 48 real incremental frames at
20 Hz in fixed geometry. CPU profiles exclude emulator presentation, kernel I/O,
network, model latency and human feel. Real terminal input behavior, WAN play and
overall visual/gameplay acceptance remain open; these fixes do not establish
that the full beta quality target is met. Save format and protocol 12 are unchanged.
