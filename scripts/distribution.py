#!/usr/bin/env python3
"""Build and verify release packages and feeds. Publishing is handled by CI."""
import argparse
import datetime as dt
import gzip
import hashlib
import io
import json
import os
from pathlib import Path, PurePosixPath
import re
import shutil
import stat
import subprocess
import tarfile
import tempfile
import urllib.error
import urllib.parse
import urllib.request
import zipfile

REPOSITORY = "pagel-s/termcraft"
HOME = "https://github.com/" + REPOSITORY
VERSION = re.compile(r"[0-9]+\.[0-9]+\.[0-9]+(?:-[0-9A-Za-z.-]+)?")
TARGETS = {"x86_64-unknown-linux-gnu", "aarch64-apple-darwin",
           "x86_64-apple-darwin", "x86_64-pc-windows-msvc"}
IDENTIFIER = "PagelS.Termcraft"
SCHEMA_VERSION = "1.10.0"
MAX_DOWNLOAD = 128 * 1024 * 1024


def digest(data):
    return hashlib.sha256(data).hexdigest()


def version_tag(version):
    if not VERSION.fullmatch(version):
        raise ValueError("invalid release version")
    return "v" + version


def asset_url(tag, name):
    if tag != version_tag(tag.removeprefix("v")) or not re.fullmatch(r"[A-Za-z0-9][A-Za-z0-9._~+-]*", name):
        raise ValueError("invalid release asset")
    return f"{HOME}/releases/download/{tag}/{name}"


def write_json(path, value):
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(value, indent=2) + "\n", encoding="utf-8")


def read_native(path):
    """Read regular files only, checking every member and every bundled hash."""
    checksum = Path(str(path) + ".sha256").read_text().split()
    if len(checksum) != 2 or checksum[1] != path.name or digest(path.read_bytes()) != checksum[0]:
        raise ValueError(f"archive checksum mismatch: {path.name}")
    files = {}
    total = 0
    if path.name.endswith(".zip"):
        with zipfile.ZipFile(path) as source:
            if any(m.is_dir() or stat.S_IFMT(m.external_attr >> 16) not in (0, stat.S_IFREG) for m in source.infolist()):
                raise ValueError("non-regular ZIP member")
            records = [(m.filename, m.file_size, lambda m=m: source.read(m)) for m in source.infolist()]
            for name, size, read in records:
                total += size
                if total > MAX_DOWNLOAD or name in files:
                    raise ValueError("oversized archive or duplicate member")
                files[name] = read()
    else:
        with tarfile.open(path) as source:
            for member in source:
                total += member.size
                if not member.isfile() or total > MAX_DOWNLOAD or member.name in files:
                    raise ValueError("non-regular, oversized or duplicate archive member")
                files[member.name] = source.extractfile(member).read()
    for name in files:
        p = PurePosixPath(name)
        if p.is_absolute() or ".." in p.parts or "\\" in name or len(p.parts) < 2:
            raise ValueError("unsafe archive path")
    manifests = [n for n in files if n.endswith("/MANIFEST.json")]
    if len(manifests) != 1:
        raise ValueError("archive must have one manifest")
    prefix = manifests[0].removesuffix("MANIFEST.json")
    manifest = json.loads(files[manifests[0]])
    if manifest["target"] not in TARGETS:
        raise ValueError("unsupported target")
    version_tag(manifest["version"])
    if manifest.get("prerelease") is not ("-" in manifest["version"]):
        raise ValueError("incorrect native prerelease flag")
    expected_prefix = f"termcraft-{manifest['version']}-{manifest['target']}/"
    if prefix != expected_prefix or set(files) != {prefix + p for p in manifest["files"]} | {prefix + "MANIFEST.json"}:
        raise ValueError("archive layout does not match its manifest")
    for name, expected in manifest["files"].items():
        if digest(files[prefix + name]) != expected:
            raise ValueError("bundled file checksum mismatch")
    return manifest, {n.removeprefix(prefix): data for n, data in files.items()}


def homebrew(manifest, base_url=None):
    """Our tap uses already-tested native binaries; no Rust is needed."""
    def source(target):
        asset = manifest["assets"][target]
        url = base_url.rstrip("/") + "/" + asset["name"] if base_url else asset["url"]
        return f'      url {json.dumps(url)}\n      sha256 "{asset["sha256"]}"'
    return f'''class Termcraft < Formula
  desc "Terminal pixel-art sandbox survival game"
  homepage "{HOME}"
  version "{manifest["version"]}"
  license "AGPL-3.0-only"
  depends_on :macos

  on_macos do
    on_arm do
{source("aarch64-apple-darwin")}
    end
    on_intel do
{source("x86_64-apple-darwin")}
    end
  end

  def install
    bin.install "termcraft"
    doc.install "README.md", "RELEASE_NOTES.md", "LICENSE"
    pkgshare.install "docs", "hosting"
  end

  test do
    assert_equal "termcraft #{{version}}", shell_output("#{{bin}}/termcraft --version").strip
  end
end
'''


def winget(manifest):
    version = manifest["version"]
    archive = manifest["assets"]["x86_64-pc-windows-msvc"]
    shared = dict(PackageIdentifier=IDENTIFIER, PackageVersion=version,
                  ManifestVersion=SCHEMA_VERSION)
    return {
        f"{IDENTIFIER}.yaml": dict(shared, DefaultLocale="en-US", ManifestType="version"),
        f"{IDENTIFIER}.installer.yaml": dict(shared, ManifestType="installer",
            InstallerType="zip", NestedInstallerType="portable",
            NestedInstallerFiles=[dict(
                RelativeFilePath=f"termcraft-{version}-x86_64-pc-windows-msvc/termcraft.exe",
                PortableCommandAlias="termcraft")],
            Scope="user", UpgradeBehavior="install", Commands=["termcraft"],
            Installers=[dict(Architecture="x64", InstallerUrl=archive["url"],
                             InstallerSha256=archive["sha256"].upper())]),
        f"{IDENTIFIER}.locale.en-US.yaml": dict(shared, ManifestType="defaultLocale",
            PackageLocale="en-US", Publisher="Sebastian Pagel",
            PublisherUrl="https://github.com/pagel-s", PublisherSupportUrl=HOME + "/issues",
            PackageName="Termcraft", PackageUrl=HOME, License="AGPL-3.0-only",
            LicenseUrl=HOME + f"/blob/{manifest['tag']}/LICENSE",
            ShortDescription="Terminal pixel-art sandbox survival game",
            Description="Explore, build, farm and survive in a terminal, alone or with friends.",
            ReleaseNotesUrl=HOME + f"/releases/tag/{manifest['tag']}",
            Tags=["terminal", "game", "sandbox", "survival", "multiplayer"])
    }


def build_deb(files, version, output):
    debversion = version.replace("-", "~", 1)
    filename = f"termcraft_{debversion}_amd64.deb"
    with tempfile.TemporaryDirectory(prefix="termcraft-deb-") as directory:
        root = Path(directory)
        payload = {"usr/bin/termcraft": files["termcraft"],
                   "usr/share/doc/termcraft/copyright": files["LICENSE"],
                   "usr/share/doc/termcraft/README.md": files["README.md"],
                   "usr/share/doc/termcraft/RELEASE_NOTES.md": files["RELEASE_NOTES.md"]}
        for name, data in files.items():
            if name.startswith("docs/"):
                payload["usr/share/doc/termcraft/" + name] = data
            if name.startswith("hosting/"):
                payload["usr/share/termcraft/" + name] = data
        for name, data in payload.items():
            path = root / name
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_bytes(data)
            path.chmod(0o755 if name == "usr/bin/termcraft" else 0o644)
        (root / "DEBIAN").mkdir()
        (root / "DEBIAN/control").write_text(
            f"Package: termcraft\nVersion: {debversion}\nArchitecture: amd64\n"
            "Maintainer: Sebastian Pagel <pagel.sebastian.1@gmail.com>\n"
            "Section: games\nPriority: optional\n"
            "Depends: libc6 (>= 2.35), libgcc-s1\n"
            f"Homepage: {HOME}\n"
            f"Installed-Size: {(sum(map(len, payload.values())) + 1023) // 1024}\n"
            "Description: Terminal pixel-art sandbox survival game\n"
            " Explore, build, farm and survive, alone or with friends.\n",
            encoding="utf-8")
        destination = output / filename
        env = dict(os.environ, SOURCE_DATE_EPOCH=os.environ.get("SOURCE_DATE_EPOCH", "0"))
        for path in sorted(root.rglob("*"), reverse=True):
            os.utime(path, (0, 0))
        subprocess.run(["dpkg-deb", "--root-owner-group", "-Zxz", "--build", str(root), str(destination)],
                       check=True, env=env, stdout=subprocess.PIPE)
    return destination


def prepare(native, output, revision, tag, qualifications=None):
    if not re.fullmatch(r"[0-9a-f]{40}", revision):
        raise ValueError("release needs an exact source commit")
    version = tag.removeprefix("v")
    if version_tag(version) != tag:
        raise ValueError("tag and version differ")
    archives = sorted(native.rglob("termcraft-*.tar.gz")) + sorted(native.rglob("termcraft-*.zip"))
    if len(archives) != 4:
        raise ValueError("exactly four native archives are required")
    output.mkdir(parents=True, exist_ok=True)
    manifest = dict(schema=1, repository=REPOSITORY, version=version, tag=tag,
                    source_commit=revision, prerelease="-" in version, assets={})
    linux = None
    for archive in archives:
        package, files = read_native(archive)
        target = package["target"]
        if target in manifest["assets"] or package["version"] != version or package.get("source_commit") != revision:
            raise ValueError("native archives have different sources, versions or duplicate targets")
        binary = "termcraft.exe" if "windows" in target else "termcraft"
        if qualifications is not None:
            matches = list(qualifications.rglob(f"native-qualification-{target}/native-qualification.json"))
            if len(matches) != 1:
                raise ValueError("missing native qualification")
            q = json.loads(matches[0].read_text())
            if (q["status"] != "passed" or q["archive_sha256"] != digest(archive.read_bytes())
                    or q["binary_sha256"] != digest(files[binary])):
                raise ValueError("native qualification does not match archive")
        destination = output / archive.name
        if destination.resolve() != archive.resolve():
            shutil.copyfile(archive, destination)
        manifest["assets"][target] = dict(name=archive.name, sha256=digest(archive.read_bytes()),
                                         url=asset_url(tag, archive.name))
        if target == "x86_64-unknown-linux-gnu":
            linux = files
    if set(manifest["assets"]) != TARGETS:
        raise ValueError("missing native target")
    deb = build_deb(linux, version, output)
    manifest["assets"]["deb-amd64"] = dict(name=deb.name, sha256=digest(deb.read_bytes()), url=asset_url(tag, deb.name))
    formula = output / "termcraft.rb"
    formula.write_text(homebrew(manifest), encoding="utf-8")
    manifest["assets"]["homebrew-formula"] = dict(name=formula.name, sha256=digest(formula.read_bytes()), url=asset_url(tag, formula.name))
    winget_dir = output / "winget" / version
    for name, value in winget(manifest).items():
        write_json(winget_dir / name, value)  # JSON is a strict subset of YAML.
    winget_zip = output / "termcraft-winget-manifests.zip"
    with zipfile.ZipFile(winget_zip, "w", compression=zipfile.ZIP_DEFLATED) as archive:
        for path in sorted(winget_dir.glob("*.yaml")):
            archive.write(path, f"{version}/{path.name}")
    manifest["assets"]["winget-manifests"] = dict(name=winget_zip.name, sha256=digest(winget_zip.read_bytes()), url=asset_url(tag, winget_zip.name))
    write_json(output / "termcraft-distribution.json", manifest)
    payloads = [p for p in sorted(output.iterdir()) if p.is_file() and p.name != "SHA256SUMS" and not p.name.endswith(".sha256")]
    for path in payloads:
        Path(str(path) + ".sha256").write_text(f"{digest(path.read_bytes())}  {path.name}\n")
    (output / "SHA256SUMS").write_text("".join(f"{digest(p.read_bytes())}  {p.name}\n" for p in payloads))
    return manifest


def download(url):
    request = urllib.request.Request(url, headers={"User-Agent": "termcraft-distribution/1"})
    with urllib.request.urlopen(request, timeout=60) as response:
        data = response.read(MAX_DOWNLOAD + 1)
    if len(data) > MAX_DOWNLOAD:
        raise ValueError("oversized release download")
    return data


def validate_manifest(manifest):
    version = manifest["version"]
    if (manifest.get("schema") != 1 or manifest.get("repository") != REPOSITORY
            or manifest.get("tag") != version_tag(version)
            or not re.fullmatch(r"[0-9a-f]{40}", manifest.get("source_commit", ""))):
        raise ValueError("invalid distribution manifest")
    names = {target: f"termcraft-{version}-{target}" + (".zip" if "windows" in target else ".tar.gz") for target in TARGETS}
    names.update({"deb-amd64": f"termcraft_{version.replace('-', '~', 1)}_amd64.deb",
                  "homebrew-formula": "termcraft.rb", "winget-manifests": "termcraft-winget-manifests.zip"})
    if manifest.get("prerelease") is not ("-" in version):
        raise ValueError("invalid release channel")
    for kind, asset in manifest["assets"].items():
        if asset["name"] != names.get(kind):
            raise ValueError("incorrect release asset name")
        if not re.fullmatch(r"[0-9a-f]{64}", asset["sha256"]) or asset["url"] != asset_url(manifest["tag"], asset["name"]):
            raise ValueError("invalid release asset identity")
    required = TARGETS | {"deb-amd64", "homebrew-formula", "winget-manifests"}
    if set(manifest["assets"]) != required:
        raise ValueError("incomplete distribution manifest")


def fetch(output, kind, current=None, tag=None):
    releases = json.loads(download(f"https://api.github.com/repos/{REPOSITORY}/releases?per_page=30"))
    selected = []
    for release in releases:
        if release["draft"]:
            continue
        if tag and release.get("tag_name") != tag:
            continue
        asset = next((a for a in release["assets"] if a["name"] == "termcraft-distribution.json"), None)
        if asset:
            if asset["browser_download_url"] != asset_url(release["tag_name"], asset["name"]):
                raise ValueError("unexpected distribution manifest URL")
            data = download(asset["browser_download_url"])
            manifest = json.loads(data)
            validate_manifest(manifest)
            if manifest["tag"] != release["tag_name"]:
                raise ValueError("manifest tag mismatch")
            selected.append((manifest, digest(data)))
            if len(selected) == (3 if kind == "apt" else 1):
                break
    output.mkdir(parents=True, exist_ok=True)
    if not selected:
        return dict(available=False, reason="No published release has distribution metadata")
    latest, sha = selected[0]
    if current:
        try:
            previous = json.loads(download(current) if current.startswith("https://") else Path(current).read_bytes())
        except (FileNotFoundError, urllib.error.HTTPError):
            previous = {}
        age = dt.datetime.now(dt.timezone.utc).timestamp() - previous.get("generated_at", 0)
        if previous.get("manifest_sha256") == sha and (kind != "apt" or age < 7 * 86400):
            return dict(available=False, reason="Channel already current")
    keys = {"homebrew": ["homebrew-formula"], "apt": ["deb-amd64"], "winget": ["winget-manifests"]}[kind]
    for manifest, _ in selected:
        write_json(output / "manifests" / (manifest["tag"] + ".json"), manifest)
        for key in keys:
            asset = manifest["assets"][key]
            data = download(asset["url"])
            if digest(data) != asset["sha256"]:
                raise ValueError("release asset checksum mismatch")
            destination = output / asset["name"]
            destination.write_bytes(data)
    state = dict(tag=latest["tag"], manifest_sha256=sha,
                 generated_at=int(dt.datetime.now(dt.timezone.utc).timestamp()))
    write_json(output / "release.json", state)
    write_json(output / "latest.json", latest)
    return dict(available=True, version=latest["version"], tag=latest["tag"])


def unpack_winget(candidate, output):
    """Extract only the exact generated manifest set; never trust ZIP paths."""
    manifest = json.loads((candidate / "latest.json").read_text())
    validate_manifest(manifest)
    asset = manifest["assets"]["winget-manifests"]
    archive = candidate / asset["name"]
    if digest(archive.read_bytes()) != asset["sha256"]:
        raise ValueError("WinGet manifest archive checksum mismatch")
    expected = winget(manifest)
    prefix = manifest["version"] + "/"
    with zipfile.ZipFile(archive) as source:
        members = source.infolist()
        if (len(members) != 3 or {m.filename for m in members} != {prefix + n for n in expected}
                or any(m.file_size > 65536 or m.is_dir() for m in members)):
            raise ValueError("unexpected WinGet archive layout")
        records = {m.filename.removeprefix(prefix): json.loads(source.read(m)) for m in members}
    if records != expected:
        raise ValueError("WinGet manifests do not match the qualified release")
    directory = output / manifest["version"]
    for name, record in records.items():
        write_json(directory / name, record)
    return directory


def build_apt(candidate, output, key, fingerprint=None):
    manifest = json.loads((candidate / "latest.json").read_text())
    validate_manifest(manifest)
    apt = output / "apt"
    pool = apt / "pool/main/t/termcraft"
    pool.mkdir(parents=True, exist_ok=True)
    packages = []
    for meta in sorted((candidate / "manifests").glob("*.json")):
        record = json.loads(meta.read_text())
        validate_manifest(record)
        asset = record["assets"]["deb-amd64"]
        source = candidate / asset["name"]
        data = source.read_bytes()
        if digest(data) != asset["sha256"]:
            raise ValueError("APT payload checksum mismatch")
        shutil.copyfile(source, pool / source.name)
        control = subprocess.check_output(["dpkg-deb", "--field", str(source)], text=True)
        if not re.search(r"(?m)^Package: termcraft$", control) or not re.search(r"(?m)^Architecture: amd64$", control):
            raise ValueError("unexpected Debian package identity")
        if not re.search(r"(?m)^Version: " + re.escape(record["version"].replace("-", "~", 1)) + "$", control):
            raise ValueError("Debian package version does not match release")
        packages.append(control.strip() + f"\nFilename: pool/main/t/termcraft/{source.name}\nSize: {len(data)}\nSHA256: {digest(data)}\n\n")
    if not packages:
        raise ValueError("no Debian packages")
    directory = apt / "dists/beta/main/binary-amd64"
    directory.mkdir(parents=True, exist_ok=True)
    package_data = "".join(packages).encode()
    (directory / "Packages").write_bytes(package_data)
    (directory / "Packages.gz").write_bytes(gzip.compress(package_data, mtime=0))
    now = dt.datetime.now(dt.timezone.utc)
    release = (
        "Origin: Termcraft\nLabel: Termcraft beta\nSuite: beta\nCodename: beta\n"
        f"Date: {now.strftime('%a, %d %b %Y %H:%M:%S +0000')}\n"
        f"Valid-Until: {(now + dt.timedelta(days=30)).strftime('%a, %d %b %Y %H:%M:%S +0000')}\n"
        "Architectures: amd64\nComponents: main\nDescription: Termcraft beta packages\nSHA256:\n")
    for path in sorted(directory.glob("Packages*")):
        data = path.read_bytes()
        release += f" {digest(data)} {len(data)} {path.relative_to(apt / 'dists/beta')}\n"
    release_path = apt / "dists/beta/Release"
    release_path.write_text(release, encoding="utf-8")
    with tempfile.TemporaryDirectory(prefix="termcraft-apt-key-") as private:
        Path(private).chmod(0o700)
        command = ["gpg", "--homedir", private, "--batch", "--no-tty"]
        try:
            subprocess.run(command + ["--import"], input=key, check=True, capture_output=True)
            listing = subprocess.check_output(command + ["--with-colons", "--list-secret-keys"], text=True, stderr=subprocess.DEVNULL)
            keys = [row.split(":")[9] for row in listing.splitlines() if row.startswith("fpr:")]
            if not keys or (fingerprint and keys[0] != fingerprint.strip().upper()):
                raise ValueError("APT signing key fingerprint mismatch")
            public = subprocess.check_output(command + ["--export", keys[0]])
            (apt / "termcraft-archive-keyring.gpg").write_bytes(public)
            for options, destination in [
                (["--clearsign"], release_path.parent / "InRelease"),
                (["--detach-sign"], release_path.parent / "Release.gpg")]:
                subprocess.run(command + ["--yes", "--local-user", keys[0], "--digest-algo", "SHA256",
                                          "--output", str(destination), *options, str(release_path)],
                               check=True, capture_output=True)
            (output / "signing-fingerprint.txt").write_text(keys[0] + "\n")
        finally:
            subprocess.run(["gpgconf", "--homedir", private, "--kill", "gpg-agent"], capture_output=True)
    shutil.copyfile(candidate / "release.json", apt / "current.json")
    (output / "index.html").write_text('<!doctype html><title>Termcraft packages</title><h1>Termcraft beta APT repository</h1><p>See <a href="' + HOME + '">installation instructions</a>.</p>\n')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    sub = parser.add_subparsers(dest="command", required=True)
    p = sub.add_parser("prepare")
    p.add_argument("--native", type=Path, required=True)
    p.add_argument("--output", type=Path, required=True)
    p.add_argument("--revision", required=True)
    p.add_argument("--tag", required=True)
    p.add_argument("--qualifications", type=Path)
    p = sub.add_parser("fetch")
    p.add_argument("--output", type=Path, required=True)
    p.add_argument("--kind", choices=["homebrew", "apt", "winget"], required=True)
    p.add_argument("--current")
    p.add_argument("--tag")
    p = sub.add_parser("apt")
    p.add_argument("--candidate", type=Path, required=True)
    p.add_argument("--output", type=Path, required=True)
    p.add_argument("--fingerprint")
    p = sub.add_parser("formula")
    p.add_argument("--manifest", type=Path, required=True)
    p.add_argument("--output", type=Path, required=True)
    p.add_argument("--base-url")
    p = sub.add_parser("unpack-winget")
    p.add_argument("--candidate", type=Path, required=True)
    p.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    if args.command == "prepare":
        prepare(args.native, args.output, args.revision, args.tag, args.qualifications)
    elif args.command == "fetch":
        result = fetch(args.output, args.kind, args.current, args.tag)
        write_json(args.output / "result.json", result)
        if "GITHUB_OUTPUT" in os.environ:
            with open(os.environ["GITHUB_OUTPUT"], "a") as output:
                output.write("available=" + str(result["available"]).lower() + "\n")
        print(json.dumps(result))
    elif args.command == "apt":
        key = os.environ.get("APT_SIGNING_PRIVATE_KEY", "").encode()
        if not key:
            raise ValueError("Configure APT_SIGNING_PRIVATE_KEY in the APT repository")
        build_apt(args.candidate, args.output, key, args.fingerprint)
    elif args.command == "unpack-winget":
        print(unpack_winget(args.candidate, args.output))
    else:
        manifest = json.loads(args.manifest.read_text())
        validate_manifest(manifest)
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(homebrew(manifest, args.base_url), encoding="utf-8")


if __name__ == "__main__":
    main()
