# Lantern Run review, 12 October 2026

A focused gameplay slice, combat corrections and presentation feedback. This is
an unpublished development candidate on `beta/terminal-reborn`.

## What changed and how it was reviewed

1. Physics/gameplay review found cancelled knockback, species-priority melee,
   endpoint-only projectile hits, instant attacks and chest-only save loss.
   Shared body targeting, ordered swept impacts, committed windups, interrupted
   recovery and chest dirty tracking address those findings.
2. The complete CLI route exposed a missing agent jump command, awkward recovery
   ladders, an escaping guard and missing bridge anchors. Jump now queues takeoff;
   the trial has dry recovery shafts, a normal openable gate and an abutment.
   Supplies use normal chest inventory and are seeded once, including across resume.
3. Visual review found lost boots during compact jumps, missing classic warnings,
   a swing shorter than the hit reach and effect/glass clipping differences.
   Output-pixel leg poses, markers in both views, a longer phase-linked swing and
   shared clipping address these. The idle panel clears more of the scene.
4. Final independent reviews found no blocking visual regressions in the selected
   poses. Physics review caught an ocelot loop indexing wolves; this was corrected
   and a full-update one-ocelot/zero-wolves obstacle regression added. Wolf/ocelot
   jump probes now match their solver dimensions. Arrow impulse follows flight,
   including the dragon, rather than the shooter's later position.

## Captures and reproduction

The [gallery](index.html) contains nine poses at three terminal sizes: camp,
windup, hurt, strike, follow-through, recovery, ascent, descent and ravine.
`examples/lantern_review.rs` creates disposable terrain and stages the player's
position for focused poses, then uses real AI/physics/action ticks and `Renderer`.
These are software rasterizations of actual color ANSI output. They establish
specific visible states, not emulator latency or a human playthrough.

```sh
CARGO_TARGET_DIR=/tmp/termcraft-build cargo build --release --locked --bins --examples
env -u NO_COLOR /tmp/termcraft-build/release/examples/lantern_review /tmp/lantern-ansi
python3 scripts/capture_terminal_art.py /tmp/lantern-ansi /tmp/lantern-png
python3 scripts/test_lantern_trial.py --binary /tmp/termcraft-build/release/termcraft --report /tmp/lantern.json
```

The separate CLI test creates a fresh isolated world, traverses, kills all three
guards, opens/loots storage, places eight bridge blocks, lights the signal and
saves/reopens completion. It uses normal inventory, reach, collision and mining;
no direct state edits, teleport or developer loadout. It knows map coordinates and
advances simulation between actions. It is also retained in CI/release smoke.

[Exact candidate identity and qualification](../../assets/lantern-run-2026-10-12.json)
include 611 unit tests on stable/Rust 1.88, Clippy, real TCP, terminal input,
SSH fixture, renderer CPU work and CLI completion. Earlier review reports retain
their original identities. Protocol 12 requires matching clients and server;
progression remains v7.

## Still open

Uncoached human onboarding, enjoyment, final art acceptance, real WAN smoothness,
clean Windows/macOS qualification, larger villages, fluids/boats and full survival
through the dragon. Ground shadows remain bounded contact shadows. The trial is
not a new campaign or a requirement to run a model. Trial supplies are ordinary
storage, accessible before guard victory; objective progress requires credited
kills. World and progression files are atomic separately, not a whole-world
transaction. Session-only guards restart the full unfinished wave after reopening.
