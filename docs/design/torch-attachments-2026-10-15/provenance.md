# Torch support and presentation, 15 October 2026

Correction after `187171b50af152b4533893364e4032d8e67f8a10`, prompted by the
floating-torch playtest report. [Gallery](index.html) contains actual renderer
output, software rasterized from ANSI. Attachment poses and day/night times are
staged; village interiors use fresh ordinary generation. These are not terminal
emulator screenshots or concept images.

Placement, support removal and art now share `World::torch_attachment`. Full
blocks, lower slabs and stair treads use their collision surface. Side mounts
have a shaft and bracket meeting the wall; inferred enclosed-room mounts have a
metal back plate. Deferred drawing keeps terrain from painting over the stem.
The two-pixel wide color view has its own legible flame/shaft silhouette.

Unsupported player placement does not consume an item. Losing support drops one
torch, including at negative chunk seams. A review found that replacing a wooden
roof with a solid material outside the cutaway enclosure rules also needs to
invalidate sconces; the regression covers both removal and replacement. Doors
can open without deleting the room's mounted lights.

Fresh village ground lights move onto their floor. Stronghold lights find the
actual floor after overlapping rooms are carved, avoiding nominal surfaces that
have become air. Director structures apply torches after all their new supports.
Loaded legacy lights are checked once neighboring chunks are present; valid room
mounts remain, and unsupported torches become dropped items. There are no new
serialized variants, save fields or protocol changes. Redstone torch rules are
unchanged.

The full test run also exposed villagers leaving shelter as soon as the roof made
them dry. Shelter intent now lasts while local precipitation continues. Four
behavior fixtures use isolated known terrain/rules rather than random shared saves.

```sh
cargo test --locked
cargo clippy --locked --all-targets -- -D warnings
cargo build --release --locked --bins --examples
env -u NO_COLOR target/release/examples/torch_review /tmp/torch-ansi
python3 scripts/capture_terminal_art.py /tmp/torch-ansi /tmp/torch-png
env -u NO_COLOR target/release/examples/village_review /tmp/village-ansi
python3 scripts/test_village_walk.py --binary target/release/termcraft --fixture /tmp/village-ansi/village.json --report /tmp/village-walk.json
python3 scripts/test_lantern_trial.py --binary target/release/termcraft --report /tmp/lantern.json
```

[Exact source/binary hashes and validation](../../assets/torch-attachments-2026-10-15.json)
record the completed checks. CPU profiles measure local work, excluding emulator,
network and model latency. The torch-cave renderer fixture now puts lights on its
floor, so it is not geometrically identical to the previous floating-light fixture.

The back wall remains an inferred small-room cutaway with bounded roof/floor/side
searches; this does not add arbitrary saved walls or qualify large halls. The
staged showcase stays within that existing enclosure size. A lower slab extends
the small shaft to its surface. Human judgment of the art, actual terminal
smoothness, Windows/macOS and remote play remain separate open quality gates.
