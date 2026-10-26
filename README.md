<div align="center">
  <h1>termcraft</h1>
  <p><strong>Survive, build and explore a block world in your terminal. Bring friends over SSH.</strong></p>
  <p>
    <a href="https://pagel-s.github.io/termcraft/"><img alt="Docs" src="https://img.shields.io/badge/docs-project%20guide-2f6feb?style=flat-square"></a>
    <a href="https://github.com/pagel-s/termcraft/releases/tag/v0.2.0"><img alt="Beta release 0.2.0" src="https://img.shields.io/badge/beta-0.2.0-7452aa?style=flat-square"></a>
    <a href="https://www.rust-lang.org/"><img alt="Rust" src="https://img.shields.io/badge/rust-1.88%2B-93450a?style=flat-square&logo=rust"></a>
  </p>
  <p>
    <a href="docs/installation.md">Play</a> ·
    <a href="docs/gameplay.md">Controls</a> ·
    <a href="docs/multiplayer.md">Co-op</a> ·
    <a href="RELEASE_NOTES.md">Release notes</a>
  </p>
</div>

> Unofficial fan project. Not affiliated with Mojang or Microsoft.
>
> **Termcraft `0.2.0` — the beta release.**
> Download the game from [Releases](https://github.com/pagel-s/termcraft/releases/tag/v0.2.0).

[![Termcraft beta gameplay: co-op exploration, crafting, the Nether and the Ender Dragon](docs/assets/termcraft.gif)](https://youtu.be/nn37_15dLFM)

[Watch the full video](https://youtu.be/nn37_15dLFM) ·
[Music and media credits](docs/media-credits.md)

## A survival world, adapted to 2D

Gather wood, craft tools, build a shelter and survive the night. Explore caves and
villages, smelt better equipment, cross the Nether, find the stronghold and face
the Ender Dragon. Termcraft adapts that survival progression to a side-on terminal
world, with shaped terrain, room cutaways and distinct creature silhouettes.

- Mine, craft, farm and build in a procedural Overworld, Nether and End.
- Explore villages, dungeons and fortresses, then face the Ender Dragon.
- Play solo or with up to eight players, with shared building, PvP and player chat.
- Terminal pixel art, lighting and selectable views; **Detail is the default**.

## Play the beta

**Without Rust:** download your platform's `0.2.0` archive from
[Releases](https://github.com/pagel-s/termcraft/releases/tag/v0.2.0). Extract the game archive,
keeping all files together. Open a terminal in the extracted folder.

| Platform | Download `0.2.0` | Start Lantern Run |
| --- | --- | --- |
| Linux, Intel/AMD 64-bit | [Linux archive](https://github.com/pagel-s/termcraft/releases/download/v0.2.0/termcraft-0.2.0-x86_64-unknown-linux-gnu.tar.gz) | `./play.sh` |
| Windows, Intel/AMD 64-bit | [Windows archive](https://github.com/pagel-s/termcraft/releases/download/v0.2.0/termcraft-0.2.0-x86_64-pc-windows-msvc.zip) | `.\PLAY.cmd` in Windows Terminal |
| macOS, Apple silicon | [Apple silicon archive](https://github.com/pagel-s/termcraft/releases/download/v0.2.0/termcraft-0.2.0-aarch64-apple-darwin.tar.gz) | `./PLAY.command` |
| macOS, Intel | [Intel Mac archive](https://github.com/pagel-s/termcraft/releases/download/v0.2.0/termcraft-0.2.0-x86_64-apple-darwin.tar.gz) | `./PLAY.command` |

For classic survival run `./termcraft` on Linux/macOS or `.\termcraft.exe` on
Windows. The launchers in the table start **Lantern Run**, a separate short
combat, traversal and building route. macOS archives are unsigned; see
[installation](docs/installation.md) for platform details.

**Package managers:** on macOS run `brew install pagel-s/termcraft/termcraft`.
Rust users can run `cargo install termcraft --locked` (Rust 1.88+).
For Debian/Ubuntu, follow the [signed APT setup](docs/distribution.md#apt-setup).
WinGet is pending Microsoft's review; use the Windows archive above meanwhile.
See [package-manager channels](docs/distribution.md#package-manager-channels)
for details.

To build it yourself, follow [building from source](docs/installation.md#build-from-source).

## Controls

Start with a true-color terminal, a monospace font and a window around **120×40**
or larger. Mouse support is recommended.

| Action | Control |
| --- | --- |
| Walk | Hold `A` / `D` or left/right arrow |
| Sprint | Tap, release, then quickly hold the same direction |
| Jump / swim up | `W`, `Up` or `Space` |
| Mine / attack | Left mouse button |
| Place / interact | Right mouse button; `F` uses the hovered block |
| Inventory / crafting | `E` |
| Hotbar | `1`–`9` |
| Drop one / stack | `V` / `Shift+V` |
| Zoom in / out | `=` or `+` / `-` |
| Close UI / save and quit | `Q` or `Esc` |

**O** opens settings, **F3** diagnostics and **F4** cycles views. Saved view choices
are respected. See [all controls and gameplay](docs/gameplay.md) for sprint input
support, crafting, furnaces and portals.

## Play with friends

Use matching beta builds. Start hosting and playing in one terminal:

```sh
./termcraft host
```

The host starts the server and joins automatically. Press **T** to see the join
address; friends on the same network run `./termcraft client ADDRESS --player bob`.
Use a different `--player` profile for each terminal. **T** also opens player chat.
For package-manager installs, omit `./`; on Windows use `.\termcraft.exe`.
Quitting the host ends the shared session.

Follow the **[multiplayer setup guide](docs/multiplayer.md)** for one computer,
one network or separate networks, with connection checks and troubleshooting.
For remote access, follow [safe hosting](docs/hosting.md) or
[SSH joining](docs/distribution.md#join-through-ssh).

## Saves and current limits

The game autosaves in your user data directory. Back up the **whole save directory**,
including hidden files, before upgrading; older builds cannot open newer saves.
See [save locations and recovery](docs/world-format.md).

The game is in beta. See [supported content](docs/supported-content.md) and
[release notes](RELEASE_NOTES.md) for current capabilities and limits.

## Feedback

Report bugs through [GitHub issues](https://github.com/pagel-s/termcraft/issues)
or `pagel.sebastian.1@gmail.com`. Include build, OS, terminal and reproduction steps.

[Supported content](docs/supported-content.md) · [Media credits](docs/media-credits.md)
