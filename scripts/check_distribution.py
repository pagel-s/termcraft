#!/usr/bin/env python3
"""Exercise prepared manager packages; only macOS disposable CI installs a formula."""
import argparse
from functools import partial
import getpass
import http.server
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import threading

import distribution as d


def check_schema(prepared):
    import jsonschema
    manifest = json.loads((prepared / "termcraft-distribution.json").read_text())
    d.validate_manifest(manifest)
    schemas = Path(__file__).resolve().parent.parent / "tests/fixtures/winget-schema-1.10.0"
    manifests = list((prepared / "winget" / manifest["version"]).glob("*.yaml"))
    assert len(manifests) == 3
    for path in manifests:
        record = d.read_winget(path.read_bytes())
        assert record["PackageVersion"] == manifest["version"] and record["PackageIdentifier"] == d.IDENTIFIER
        schema = json.loads((schemas / (record["ManifestType"] + ".json")).read_text())
        jsonschema.validate(record, schema)
    return manifest


def check_linux(prepared, manifest, root):
    deb = prepared / manifest["assets"]["deb-amd64"]["name"]
    extracted = root / "extracted"
    subprocess.run(["dpkg-deb", "--extract", str(deb), str(extracted)], check=True)
    actual = subprocess.check_output([str(extracted / "usr/bin/termcraft"), "--version"], text=True).strip()
    assert actual == "termcraft " + manifest["version"]
    private = root / "private"
    private.mkdir(mode=0o700)
    gpg = ["gpg", "--homedir", str(private), "--batch", "--no-tty"]
    try:
        subprocess.run(gpg + ["--pinentry-mode", "loopback", "--passphrase", "",
                             "--quick-generate-key", "Termcraft disposable CI test", "ed25519", "sign", "1d"],
                       check=True, capture_output=True)
        key = subprocess.check_output(gpg + ["--export-secret-keys"], stderr=subprocess.DEVNULL)
        candidate = root / "candidate"
        (candidate / "manifests").mkdir(parents=True)
        d.write_json(candidate / "latest.json", manifest)
        d.write_json(candidate / "manifests" / (manifest["tag"] + ".json"), manifest)
        d.write_json(candidate / "release.json", dict(tag=manifest["tag"], manifest_sha256="0" * 64, generated_at=0))
        shutil.copyfile(deb, candidate / deb.name)
        site = root / "site"
        d.build_apt(candidate, site, key)
        apt = site / "apt"
        subprocess.run(["gpgv", "--keyring", str(apt / "termcraft-archive-keyring.gpg"),
                        str(apt / "dists/beta/InRelease")], check=True)
        lists = root / "lists"
        (lists / "partial").mkdir(parents=True)
        cache = root / "cache"
        (cache / "archives/partial").mkdir(parents=True)
        source = root / "termcraft.sources"
        source.write_text("Types: deb\nURIs: " + apt.as_uri() +
                          "\nSuites: beta\nComponents: main\nArchitectures: amd64\nSigned-By: " +
                          str(apt / "termcraft-archive-keyring.gpg") + "\n")
        options = ["-o", "Dir::Etc::sourcelist=" + str(source),
                   "-o", "Dir::Etc::sourceparts=-",
                   "-o", "Dir::State::lists=" + str(lists),
                   "-o", "Dir::Cache=" + str(cache),
                   "-o", "APT::Sandbox::User=" + getpass.getuser(),
                   "-o", "APT::Get::List-Cleanup=0"]
        subprocess.run(["apt-get", *options, "update"], check=True)
        policy = subprocess.check_output(["apt-cache", *options, "policy", "termcraft"], text=True)
        assert "Candidate: " + manifest["version"].replace("-", "~", 1) in policy, policy
        # Only a simulation: never write to the machine's dpkg database.
        subprocess.run(["apt-get", *options, "--simulate", "install", "termcraft"], check=True)
    finally:
        subprocess.run(["gpgconf", "--homedir", str(private), "--kill", "gpg-agent"], capture_output=True)


def check_macos(prepared, manifest, root, public=False):
    if os.environ.get("GITHUB_ACTIONS") != "true":
        raise RuntimeError("Homebrew installation checks require a disposable GitHub runner")
    env = dict(os.environ, HOMEBREW_NO_AUTO_UPDATE="1",
               GIT_AUTHOR_NAME="Sebastian Pagel", GIT_AUTHOR_EMAIL="pagel.sebastian.1@gmail.com",
               GIT_COMMITTER_NAME="Sebastian Pagel", GIT_COMMITTER_EMAIL="pagel.sebastian.1@gmail.com")
    tap = "termcraft-check/candidate"
    formula = tap + "/termcraft"
    subprocess.run(["brew", "tap-new", tap], check=True, env=env)
    location = Path(subprocess.check_output(["brew", "--repository", tap], text=True, env=env).strip())
    server = http.server.ThreadingHTTPServer(("127.0.0.1", 0),
                partial(http.server.SimpleHTTPRequestHandler, directory=str(prepared)))
    worker = threading.Thread(target=server.serve_forever, daemon=True)
    worker.start()
    try:
        url = "http://127.0.0.1:" + str(server.server_address[1])
        destination = location / "Formula/termcraft.rb"
        destination.parent.mkdir(exist_ok=True)
        destination.write_text(d.homebrew(manifest, None if public else url), encoding="utf-8")
        subprocess.run(["brew", "install", "--formula", formula], check=True, env=env)
        subprocess.run(["brew", "test", formula], check=True, env=env)
        prefix = subprocess.check_output(["brew", "--prefix", formula], env=env, text=True).strip()
        actual = subprocess.check_output([prefix + "/bin/termcraft", "--version"], text=True).strip()
        assert actual == "termcraft " + manifest["version"]
    finally:
        server.shutdown()
        worker.join(timeout=5)
        server.server_close()
        subprocess.run(["brew", "uninstall", "--formula", formula], env=env, check=False)
        subprocess.run(["brew", "untap", tap], env=env, check=False)


def check_windows(prepared, manifest, root):
    native = prepared / manifest["assets"]["x86_64-pc-windows-msvc"]["name"]
    _, files = d.read_native(native)
    binary = root / "termcraft.exe"
    binary.write_bytes(files["termcraft.exe"])
    actual = subprocess.check_output([str(binary), "--version"], text=True).strip()
    assert actual == "termcraft " + manifest["version"]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--prepared", type=Path, required=True)
    parser.add_argument("--report", type=Path)
    parser.add_argument("--homebrew-feed", action="store_true")
    args = parser.parse_args()
    prepared = args.prepared.resolve()
    if args.homebrew_feed:
        manifest = json.loads((prepared / "latest.json").read_text())
        d.validate_manifest(manifest)
        assert (prepared / "termcraft.rb").read_text() == d.homebrew(manifest)
    else:
        manifest = check_schema(prepared)
    with tempfile.TemporaryDirectory(prefix="termcraft-manager-check-") as temporary:
        root = Path(temporary)
        if sys.platform == "darwin":
            check_macos(prepared, manifest, root, args.homebrew_feed)
            checks = ["actual Homebrew install/test", "native executable version"]
        elif sys.platform == "win32":
            check_windows(prepared, manifest, root)
            checks = ["pinned WinGet schemas", "nested portable executable", "native executable version"]
        else:
            check_linux(prepared, manifest, root)
            checks = ["pinned WinGet schemas", "Debian extraction/executable", "GPG signature",
                      "isolated signed APT update", "APT version selection", "simulated APT install"]
    report = dict(status="passed", version=manifest["version"], source_commit=manifest["source_commit"],
                  platform=sys.platform, checks=checks)
    if args.report:
        d.write_json(args.report, report)
    print(json.dumps(report))


if __name__ == "__main__":
    main()
