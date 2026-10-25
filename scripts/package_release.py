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
    parser.add_argument("--source-revision")
    args = parser.parse_args()
    if not re.fullmatch(r"[a-zA-Z0-9_-]{1,80}", args.target):
        parser.error("invalid target")
    root = pathlib.Path(__file__).resolve().parent.parent
    version = tomllib.loads((root / "Cargo.toml").read_text())["package"]["version"]
    reported = subprocess.check_output([str(args.binary.resolve()), "--version"], text=True).strip()
    if reported != f"termcraft {version}":
        parser.error("binary and manifest versions differ")
    prerelease = "-" in version
    revision = args.source_revision
    if revision is None:
        detected = subprocess.run(["git", "rev-parse", "HEAD"], cwd=root,
                                  capture_output=True, text=True)
        revision = detected.stdout.strip() if detected.returncode == 0 else "unknown"
    if revision != "unknown" and not re.fullmatch(r"[0-9a-f]{40}", revision):
        parser.error("source revision must be a full Git commit")
    build_label = "prerelease" if prerelease else "release"
    name = f"termcraft-{version}-{args.target}"
    binary_name = "termcraft.exe" if args.binary.suffix == ".exe" else "termcraft"
    files = {binary_name: args.binary.read_bytes(), "LICENSE": (root / "LICENSE").read_bytes()}
    files["README.md"] = (root / "README.md").read_bytes()
    files["RELEASE_NOTES.md"] = (root / "RELEASE_NOTES.md").read_bytes()
    # Player archives use an explicit list, so future development files cannot
    # accidentally become part of a download.
    for relative in (
        "index.md", "installation.md", "gameplay.md", "multiplayer.md",
        "distribution.md", "hosting.md", "supported-content.md", "world-format.md",
        "media-credits.md", "assets/termcraft.png",
    ):
        files[f"docs/{relative}"] = (root / "docs" / relative).read_bytes()
    for relative in ("server.py", "Containerfile"):
        files[f"hosting/{relative}"] = (root / "hosting" / relative).read_bytes()
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
        f"Termcraft {version} ({build_label}).\n\n"
        "No Rust installation required. Extract all files and open a terminal here.\n"
        f"Start the focused combat/building route: {launcher_command}\n"
        "The launcher forwards arguments; without arguments it starts Lantern Run.\n"
        f"Classic survival: {launch}\n"
        f"Focused combat/building route: {launch} trial\n"
        f"Host and play trusted local co-op in one terminal: {launch} host\n"
        "Press T in the host game for the address friends should join.\n"
        f"Run a separate dedicated server: {launch} server\n"
        f"Join locally: {launch} client --player alice\n"
        f"Join through an existing SSH account: {launch} join user@host --player alice\n"
        "Controls: hold A/D to walk; double-tap and hold to sprint; Space to jump.\n"
        "E: inventory/crafting; V/Shift+V: drop one/stack; =/-: zoom; F4: views.\n"
        "T: player chat or solo commands; O: settings (shared rules are host-only).\n\n"
        "Detail is the fresh-start default; saved view choices are respected.\n\n"
        "See RELEASE_NOTES.md for highlights, compatibility and current limits.\n"
        "See docs/installation.md and docs/multiplayer.md for saves and limits.\n"
        "Optional rootless Linux hosting: python3 hosting/server.py start\n"
        "Read docs/hosting.md before sharing remote access.\n"
        "On Windows, use Windows Terminal with OpenSSH installed for SSH joining.\n"
    ).encode()
    files["MANIFEST.json"] = json.dumps({
        "version": version, "target": args.target, "prerelease": prerelease,
        "source_commit": revision,
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
                info.mode = 0o755 if path in {binary_name, launcher} else 0o644
                output.addfile(info, io.BytesIO(data))
    digest = hashlib.sha256(archive.read_bytes()).hexdigest()
    archive.with_name(archive.name + ".sha256").write_text(f"{digest}  {archive.name}\n")
    print(archive)


if __name__ == "__main__":
    main()
