#!/usr/bin/env python3
"""Extract one native archive, verify hashes, then run without Rust/Cargo in PATH.

Uses only Python 3.11+ standard libraries. Unix also exercises a real PTY; Windows
checks executable/configuration and stepped MCP, leaving real terminal input to
native qualification. All game saves are temporary and removed on exit.
"""
import argparse
import hashlib
import json
import os
import re
from pathlib import Path, PurePosixPath
import subprocess
import tarfile
import tempfile
import zipfile


def digest(data):
    return hashlib.sha256(data).hexdigest()


def terminal_smoke(command, env, cwd):
    import fcntl
    import pty
    import select
    import struct
    import termios
    import time

    master, slave = pty.openpty()
    fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack("HHHH", 40, 120, 0, 0))
    before = termios.tcgetattr(slave)

    def setup():
        os.setsid()
        fcntl.ioctl(slave, termios.TIOCSCTTY, 0)

    process = subprocess.Popen(command, env=env, cwd=cwd,
                               stdin=slave, stdout=slave, stderr=slave, preexec_fn=setup)
    output = bytearray()
    started, sent = time.monotonic(), False
    try:
        while process.poll() is None and time.monotonic() - started < 10:
            if select.select([master], [], [], 0.02)[0]:
                output.extend(os.read(master, 65536))
            if not sent and time.monotonic() - started > 1:
                os.write(master, b"q")
                sent = True
        assert process.poll() == 0, "extracted native terminal did not exit cleanly"
        while select.select([master], [], [], 0)[0]:
            output.extend(os.read(master, 65536))
        assert b"Termcraft |" in re.sub(rb"\x1b\[[0-?]*[ -/]*[@-~]", b"", output) and b"\x1b[?1049l" in output
        assert before == termios.tcgetattr(slave), "terminal settings not restored"
    finally:
        if process.poll() is None:
            process.kill()
        process.wait(timeout=3)
        os.close(master)
        os.close(slave)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--directory", type=Path, default=Path("dist"))
    parser.add_argument("--report", type=Path)
    args = parser.parse_args()
    archives = sorted(p for p in args.directory.iterdir()
                      if p.name.endswith((".tar.gz", ".zip")))
    if len(archives) != 1:
        parser.error("directory must contain exactly one native archive")
    archive = archives[0]
    archive_hash = digest(archive.read_bytes())
    assert archive_hash == Path(str(archive) + ".sha256").read_text().split()[0]
    contents = {}
    executable_modes = {}
    if archive.suffix == ".zip":
        with zipfile.ZipFile(archive) as source:
            for entry in source.infolist():
                assert not entry.is_dir() and entry.filename not in contents
                contents[entry.filename] = source.read(entry)
    else:
        with tarfile.open(archive) as source:
            for entry in source:
                assert entry.isfile() and entry.name not in contents, "unexpected archive member"
                contents[entry.name] = source.extractfile(entry).read()
                executable_modes[entry.name] = entry.mode
    # Spaces and a different working directory catch launcher path mistakes.
    with tempfile.TemporaryDirectory(prefix="termcraft native archive ") as temporary:
        root = Path(temporary)
        prefixes = set()
        for name, data in contents.items():
            relative = PurePosixPath(name)
            assert not relative.is_absolute() and ".." not in relative.parts and "\\" not in name
            assert len(relative.parts) >= 2
            prefixes.add(relative.parts[0])
            path = root.joinpath(*relative.parts)
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_bytes(data)
            if name in executable_modes:
                path.chmod(executable_modes[name] & 0o777)
        assert len(prefixes) == 1
        prefix = prefixes.pop()
        bundle = root / prefix
        manifest = json.loads((bundle / "MANIFEST.json").read_text())
        assert manifest["development_build"] is True
        assert set(contents) == {f"{prefix}/{p}" for p in manifest["files"]} | {f"{prefix}/MANIFEST.json"}
        for path, expected in manifest["files"].items():
            assert digest(contents[f"{prefix}/{path}"]) == expected, path
        binary_name = "termcraft.exe" if os.name == "nt" else "termcraft"
        binary = bundle / binary_name
        launcher_name = "PLAY.cmd" if os.name == "nt" else (
            "PLAY.command" if "apple-darwin" in manifest["target"] else "play.sh")
        launcher = bundle / launcher_name
        assert binary.is_file(), "archive does not match this native platform"
        assert launcher.is_file(), "missing native launcher"
        assert (bundle / "LICENSE").is_file() and (bundle / "START_HERE.txt").is_file()
        assert (bundle / "docs" / "agents.md").is_file()
        if os.name != "nt":
            assert binary.stat().st_mode & 0o111
            assert launcher.stat().st_mode & 0o111
            assert (bundle / "scripts" / "demo_director.py").stat().st_mode & 0o111
        empty = root / "empty-path"
        empty.mkdir()
        env = dict(os.environ, PATH=str(empty), TERMCRAFT_SAVE_DIR=str(root / "saves"), TERM="xterm-256color")
        env.pop("TERMCRAFT_DIRECTOR", None)
        env.pop("TERMCRAFT_WORLD_GENERATOR", None)
        version = subprocess.check_output([str(binary), "--version"], env=env, text=True, timeout=10).strip()
        assert version == f"termcraft {manifest['version']}"
        if os.name == "nt":
            command_shell = env.get("COMSPEC", r"C:\Windows\System32\cmd.exe")
            launcher_command = f'"{command_shell}" /d /s /c ""{launcher}" --version"'
        else:
            launcher_command = [str(launcher), "--version"]
        assert subprocess.check_output(launcher_command, cwd=root, env=env, text=True,
                                       timeout=10).strip() == version
        assert "join" in subprocess.check_output([str(binary), "--help"], env=env, text=True, timeout=10)
        config = json.loads(subprocess.check_output([str(binary), "mcp", "config"], env=env, timeout=10))
        assert Path(config["mcpServers"]["termcraft"]["command"]) == binary
        recipe = json.loads(subprocess.check_output([str(binary), "world", "schema"], env=env, timeout=10))["example"]
        path = root / "recipe.json"
        path.write_text(json.dumps(recipe))
        subprocess.run([str(binary), "world", "create", "archive", str(path)], env=env,
                       check=True, capture_output=True, timeout=10)
        calls = [{"jsonrpc": "2.0", "id": 1, "method": "tools/list"}]
        for index, (name, arguments) in enumerate([
            ("enter_world", {"id": "archive"}), ("act", {"ticks": 6, "direction": "right"}),
            ("save_world", {}), ("leave_world", {}),
            ("enter_world", {"id": "archive"}), ("observe", {}), ("leave_world", {}),
        ], 2):
            calls.append({"jsonrpc": "2.0", "id": index, "method": "tools/call",
                          "params": {"name": name, "arguments": arguments}})
        result = subprocess.run([str(binary), "mcp"], env=env, input="".join(json.dumps(c) + "\n" for c in calls),
                                text=True, capture_output=True, timeout=15, check=True)
        responses = [json.loads(line)["result"] for line in result.stdout.splitlines()]
        assert len(responses) == len(calls) and len(responses[0]["tools"]) == 22
        assert "inventory_click" in {tool["name"] for tool in responses[0]["tools"]}
        assert all(not r.get("isError") for r in responses)
        assert responses[2]["structuredContent"]["player"]["x"] > 0.5
        assert responses[6]["structuredContent"]["player"]["x"] == responses[2]["structuredContent"]["player"]["x"]
        if os.name != "nt":
            terminal_smoke([str(binary), "world", "play", "archive"], env, root)
            terminal_smoke([str(launcher)], env, root)
        assert not (root / "saves" / "worlds" / "world-archive" / ".session.lock").exists()
        report = dict(status="passed", archive=archive.name, target=manifest["target"],
                      archive_sha256=archive_hash, binary_sha256=digest(binary.read_bytes()),
                      manifest_files_verified=len(manifest["files"]), empty_path_runtime=True,
                      absolute_mcp_config=True, stepped_mcp_save_reopen=True,
                      launcher_arguments_from_spaced_path=True,
                      unix_launcher_default_trial=os.name != "nt",
                      unix_native_terminal_restore=os.name != "nt",
                      limits="Automated archive/runtime check; real terminal input, clean OS compatibility and human gameplay remain separate.")
        if args.report:
            args.report.write_text(json.dumps(report, indent=2) + "\n")
        print(json.dumps(report))


if __name__ == "__main__":
    main()
