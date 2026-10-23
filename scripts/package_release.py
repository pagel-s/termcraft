#!/usr/bin/env python3
"""Package an already-tested native binary; never uploads or publishes anything."""
import argparse
import hashlib
import io
import json
import pathlib
import re
import subprocess
import tarfile
import tomllib
import zipfile


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", required=True, type=pathlib.Path)
    parser.add_argument("--target", required=True)
    parser.add_argument("--output", required=True, type=pathlib.Path)
    args = parser.parse_args()
    if not re.fullmatch(r"[a-zA-Z0-9_-]{1,80}", args.target):
        parser.error("invalid target")
    root = pathlib.Path(__file__).resolve().parent.parent
    version = tomllib.loads((root / "Cargo.toml").read_text())["package"]["version"]
    reported = subprocess.check_output([str(args.binary.resolve()), "--version"], text=True).strip()
    if reported != f"termcraft {version}":
        parser.error("binary and manifest versions differ")
    name = f"termcraft-{version}-dev-{args.target}"
    binary_name = "termcraft.exe" if args.binary.suffix == ".exe" else "termcraft"
    files = {binary_name: args.binary.read_bytes(), "LICENSE": (root / "LICENSE").read_bytes()}
    files["RELEASE_NOTES.md"] = (root / "RELEASE_NOTES.md").read_bytes()
    for path in sorted((root / "docs").rglob("*")):
        # Keep the current compact art review usable offline. Historical videos
        # and large gameplay recordings are deliberately outside native bundles.
        review_clip = path in {
            root / "docs/design/pixel-refinement/movement.gif",
            root / "docs/design/dragon-refinement-2026-10-12/flight.gif",
            root / "docs/design/2d-refinement-2026-10-12/course.gif",
            root / "docs/design/run-jump-weather-2026-10-15/rain.gif",
            root / "docs/design/run-jump-weather-2026-10-15/snow.gif",
            root / "docs/design/dragon-front-2026-10-15/side-flight.gif",
            root / "docs/design/dragon-front-2026-10-15/front-flight.gif",
            root / "docs/design/nether-refinement-2026-10-15/creatures.gif",
            root / "docs/design/world-atlas-2026-10-15/nether-creatures.gif",
            root / "docs/design/world-atlas-2026-10-15/shelter-rain.gif",
            root / "docs/design/world-atlas-2026-10-15/shelter-snow.gif",
        }
        if path.is_file() and (path.suffix in {".md", ".json", ".html", ".png"} or review_clip):
            files[path.relative_to(root).as_posix()] = path.read_bytes()
    for script in ("test_lantern_trial.py",):
        files[f"scripts/{script}"] = (root / "scripts" / script).read_bytes()
    launch = ".\\termcraft.exe" if binary_name.endswith(".exe") else "./termcraft"
    if binary_name.endswith(".exe"):
        launcher = "PLAY.cmd"
        files[launcher] = (
            '@echo off\r\nsetlocal\r\n'
            'if "%~1"=="" (\r\n'
            '  "%~dp0termcraft.exe" trial\r\n'
            ') else (\r\n'
            '  "%~dp0termcraft.exe" %*\r\n'
            ')\r\nexit /b %errorlevel%\r\n'
        ).encode()
    else:
        launcher = "PLAY.command" if "apple-darwin" in args.target else "play.sh"
        files[launcher] = (
            '#!/bin/sh\n'
            'case "$0" in */*) launch_dir=${0%/*} ;; *) launch_dir=. ;; esac\n'
            'if [ "$#" -eq 0 ]; then set -- trial; fi\n'
            'exec "$launch_dir/termcraft" "$@"\n'
        ).encode()
    launcher_command = f".\\{launcher}" if binary_name.endswith(".exe") else f"./{launcher}"
    files["START_HERE.txt"] = (
        f"Termcraft {version} development build. Beta qualification is unfinished.\n\n"
        "No Rust installation required. Extract all files and open a terminal here.\n"
        f"Start the focused combat/building route: {launcher_command}\n"
        "The launcher forwards arguments; without arguments it starts Lantern Run.\n"
        f"Classic survival: {launch}\n"
        f"Focused combat/building route: {launch} trial\n"
        f"Host trusted local co-op: {launch} server\n"
        f"Join locally: {launch} client --player alice\n"
        f"Join through an existing SSH account: {launch} join user@host --player alice\n"
        "Controls: hold A/D to walk; double-tap and hold to sprint; Space to jump.\n"
        "E: inventory/crafting; V/Shift+V: drop one/stack; =/-: zoom; F4: views.\n\n"
        "See RELEASE_NOTES.md for the proposed release highlights.\n"
        "See docs/installation.md and docs/multiplayer.md for saves and limits.\n"
        "On Windows, use Windows Terminal with OpenSSH installed for SSH joining.\n"
    ).encode()
    files["MANIFEST.json"] = json.dumps({
        "version": version, "target": args.target, "development_build": True,
        "files": {path: hashlib.sha256(data).hexdigest() for path, data in files.items()},
    }, indent=2).encode()
    args.output.mkdir(parents=True, exist_ok=True)
    archive = args.output / (name + (".zip" if binary_name.endswith(".exe") else ".tar.gz"))
    if archive.exists():
        parser.error("archive already exists; use a fresh output directory")
    if binary_name.endswith(".exe"):
        with zipfile.ZipFile(archive, "x", compression=zipfile.ZIP_DEFLATED) as output:
            for path, data in files.items():
                output.writestr(f"{name}/{path}", data)
    else:
        with tarfile.open(archive, "x:gz") as output:
            for path, data in files.items():
                info = tarfile.TarInfo(f"{name}/{path}")
                info.size = len(data)
                info.mode = 0o755 if path in {binary_name, launcher} or path.startswith("scripts/") else 0o644
                output.addfile(info, io.BytesIO(data))
    digest = hashlib.sha256(archive.read_bytes()).hexdigest()
    archive.with_name(archive.name + ".sha256").write_text(f"{digest}  {archive.name}\n")
    print(archive)


if __name__ == "__main__":
    main()
