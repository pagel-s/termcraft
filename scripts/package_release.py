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
    for path in sorted((root / "docs").rglob("*")):
        # Keep the current compact art review usable offline. Historical videos
        # and large gameplay recordings are deliberately outside native bundles.
        review_clip = path == root / "docs/design/pixel-refinement/movement.gif"
        if path.is_file() and (path.suffix in {".md", ".json", ".html", ".png"} or review_clip):
            files[path.relative_to(root).as_posix()] = path.read_bytes()
    for script in ("demo_director.py", "generate_world.py"):
        files[f"scripts/{script}"] = (root / "scripts" / script).read_bytes()
    launch = ".\\termcraft.exe" if binary_name.endswith(".exe") else "./termcraft"
    files["START_HERE.txt"] = (
        f"Termcraft {version} development build. Beta qualification is unfinished.\n\n"
        "No Rust installation required. Extract all files, open a terminal here,\n"
        f"and run {launch}.\n\n"
        "Local play: termcraft\n"
        "Host trusted local co-op: termcraft server\n"
        "Join locally: termcraft client --player alice\n"
        "Join through an existing SSH account: termcraft join user@host --player alice\n"
        "Game master: press T, enter /connect, then connect your LLM using\n"
        "the entry printed by termcraft mcp config. See docs/agents.md.\n\n"
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
                info.mode = 0o755 if path == binary_name or path.startswith("scripts/") else 0o644
                output.addfile(info, io.BytesIO(data))
    digest = hashlib.sha256(archive.read_bytes()).hexdigest()
    archive.with_name(archive.name + ".sha256").write_text(f"{digest}  {archive.name}\n")
    print(archive)


if __name__ == "__main__":
    main()
