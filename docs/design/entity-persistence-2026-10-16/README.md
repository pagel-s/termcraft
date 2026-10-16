# Entity and encounter persistence — 16 October 2026

Native checkpoint **v9** and server checkpoint **v3** save the complete runtime
of every visited dimension alongside the corresponding player inventories. Native
v1–8 and server v1/v2 saves remain readable; historical entities absent from those
files cannot be recovered. Back up worlds before upgrading: older executables
cannot read the new checkpoints. Network protocol remains **14**.

## Behavior

- All entity families retain their state: mobs/animals/villagers, boats, loose
  items/XP, arrows/fireballs, End crystals/dragon and active lightning. This uses
  complete world vectors, independent of the nearby 512-entity network projection.
- Positions, health, sheared sheep, villager homes, item/XP age, entity AI timers,
  dimension clocks/spawn timers and damaged/destroyed End encounter state resume.
  Time pauses offline. Existing live spawning, despawning and population rules stay
  in force; there is no offline simulation or new permanent-mob policy.
- Solo portal/quick travel and death/respawn preserve the departing dimension's
  entities. Inactive solo dimensions pause. Co-op dimensions retain their existing
  active ticking behavior.
- Game-master finite waves retain their event ID and surviving members. Completed
  enemies are not respawned. Pending model requests/bridge sessions are not resumed.
- Lantern Run saves credited kills together with surviving guards, replacing the
  former restart-the-entire-wave behavior. Its legacy sidecar remains readable.
- Native and shared player inventories remain separate. A checkpoint revision
  selects the newest entity state across hosting/solo transitions in one save root,
  so an older mode checkpoint does not resurrect loot already collected in another.
- Held player controls, bow/mining actions and riding/fishing references are
  released on entry. Boats themselves remain present.

## Save boundaries and review corrections

Pickups and inventories share one atomic checkpoint per mode. A failed checkpoint
write retains the previous inventory/entity pair and live state can be retried.
Terrain, director metadata and checkpoint files still do not form one whole-world
transaction; abrupt power loss can leave their timestamps inconsistent. Emergency
recovery includes the runtime checkpoint but only loaded current-dimension terrain.
It remains a partial copy requiring an intact separate backup.

Readers bound files to 16 MiB, validate all dimension/entity positions, floating
state, species sizes, door coordinates and population limits (32,768 total), and
reject invalid/unsupported records without overwriting them. A failed source
checkpoint cancels portal travel; the failed attempt no longer consumes cooldown.

Review covered serialization scope, native/server ownership, repeated reloads,
portal rollback, partial waves, End regeneration, death drops and compatibility.
The native encoder borrows its captured vectors instead of cloning them again.
The [tests](https://github.com/pagel-s/termcraft/blob/beta/terminal-reborn/src/engine/persistence_tests.rs) and cooperative dimension
regressions use disposable saves; the TCP test launches/stops/restarts the actual
server and collects loot through ordinary movement commands.

## Reproduction

```sh
cargo test --locked
cargo clippy --all-targets --locked -- -D warnings
# Unix, with local TCP and process-signal permission:
cargo test --locked --test multiplayer_tcp -- --ignored
```

Exact build identity and final results are recorded in `evidence.json`. Automated
checks do not establish human gameplay quality, all-terminal feel or whole-world
crash consistency.
