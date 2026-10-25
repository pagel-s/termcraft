"""Package identity, tamper, official schema and signed APT regressions."""
import gzip
import io
import json
import os
from pathlib import Path
import shutil
import subprocess
import tarfile
import tempfile
import unittest
from unittest.mock import patch
import zipfile

import distribution as d


class Distribution(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory(prefix="termcraft-distribution-")
        self.addCleanup(self.tmp.cleanup)
        self.root = Path(self.tmp.name)
        self.native = self.root / "native"
        self.native.mkdir()
        self.version = "0.2.0-beta.1"
        self.commit = "a" * 40
        self.paths = {}
        for target in d.TARGETS:
            self.paths[target] = self.archive(target)

    def archive(self, target, revision=None):
        name = f"termcraft-{self.version}-{target}"
        binary = "termcraft.exe" if "windows" in target else "termcraft"
        files = {binary: b"test executable payload", "LICENSE": b"license",
                 "README.md": b"README", "RELEASE_NOTES.md": b"notes"}
        manifest = dict(version=self.version, target=target, prerelease=True,
                        source_commit=revision or self.commit,
                        files={n: d.digest(data) for n, data in files.items()})
        files["MANIFEST.json"] = json.dumps(manifest).encode()
        path = self.native / (name + (".zip" if "windows" in target else ".tar.gz"))
        if path.suffix == ".zip":
            with zipfile.ZipFile(path, "w") as archive:
                for n, data in files.items():
                    archive.writestr(name + "/" + n, data)
        else:
            with tarfile.open(path, "w:gz") as archive:
                for n, data in files.items():
                    info = tarfile.TarInfo(name + "/" + n)
                    info.size = len(data)
                    info.mode = 0o755 if n == binary else 0o644
                    archive.addfile(info, io.BytesIO(data))
        self.checksum(path)
        return path

    def checksum(self, path):
        Path(str(path) + ".sha256").write_text(d.digest(path.read_bytes()) + "  " + path.name + "\n")

    def prepared(self):
        output = self.root / "prepared"
        return output, d.prepare(self.native, output, self.commit, "v" + self.version)

    def test_archive_and_bundled_hash_tampering_is_rejected(self):
        path = self.paths["x86_64-pc-windows-msvc"]
        path.write_bytes(path.read_bytes() + b"tamper")
        with self.assertRaisesRegex(ValueError, "checksum"):
            d.read_native(path)
        self.archive("x86_64-pc-windows-msvc")
        with zipfile.ZipFile(path) as original:
            files = {n: original.read(n) for n in original.namelist()}
        binary = next(n for n in files if n.endswith("/termcraft.exe"))
        files[binary] = b"changed executable"
        with zipfile.ZipFile(path, "w") as archive:
            for name, data in files.items():
                archive.writestr(name, data)
        self.checksum(path)
        with self.assertRaisesRegex(ValueError, "bundled file"):
            d.read_native(path)

    def test_unsafe_archive_paths_and_mixed_sources_cannot_be_released(self):
        path = self.paths["x86_64-pc-windows-msvc"]
        with zipfile.ZipFile(path, "a") as archive:
            archive.writestr("../outside", b"no")
        self.checksum(path)
        with self.assertRaisesRegex(ValueError, "unsafe"):
            d.read_native(path)
        self.archive("x86_64-pc-windows-msvc", revision="b" * 40)
        with self.assertRaisesRegex(ValueError, "different sources"):
            self.prepared()

    @unittest.skipUnless(shutil.which("dpkg-deb"), "Debian package tools required")
    def test_prepared_package_preserves_payload_version_and_immutable_urls(self):
        output, manifest = self.prepared()
        d.validate_manifest(manifest)
        deb = output / manifest["assets"]["deb-amd64"]["name"]
        fields = subprocess.check_output(["dpkg-deb", "--field", str(deb)], text=True)
        self.assertIn("Version: 0.2.0~beta.1", fields)
        self.assertIn("Architecture: amd64", fields)
        extracted = self.root / "extracted"
        subprocess.run(["dpkg-deb", "--extract", str(deb), str(extracted)], check=True)
        self.assertEqual((extracted / "usr/bin/termcraft").read_bytes(), b"test executable payload")
        self.assertTrue((extracted / "usr/bin/termcraft").stat().st_mode & 0o111)
        self.assertFalse(list(extracted.rglob("postinst")))
        self.assertIn("on_arm", d.homebrew(manifest))
        self.assertNotIn("/actions/", d.homebrew(manifest))
        bad = json.loads(json.dumps(manifest))
        bad["assets"]["deb-amd64"]["url"] = "https://example.com/payload.deb"
        with self.assertRaisesRegex(ValueError, "identity"):
            d.validate_manifest(bad)
        bad = json.loads(json.dumps(manifest))
        bad["assets"]["deb-amd64"]["name"] = r"..\outside.deb"
        with self.assertRaises(ValueError):
            d.validate_manifest(bad)

    @unittest.skipUnless(shutil.which("dpkg-deb"), "Debian package tools required")
    def test_generated_winget_manifests_pass_pinned_microsoft_schemas(self):
        import jsonschema
        _, manifest = self.prepared()
        directory = Path(__file__).resolve().parent.parent / "tests/fixtures/winget-schema-1.10.0"
        source = json.loads((directory / "SOURCE.json").read_text())
        for kind, record in source["files"].items():
            self.assertEqual(d.digest((directory / (kind + ".json")).read_bytes()), record["sha256"])
        for record in d.winget(manifest).values():
            kind = record["ManifestType"]
            schema = json.loads((directory / (kind + ".json")).read_text())
            jsonschema.validate(record, schema)
        installer = d.winget(manifest)[d.IDENTIFIER + ".installer.yaml"]
        self.assertEqual(installer["NestedInstallerFiles"][0]["RelativeFilePath"],
                         f"termcraft-{self.version}-x86_64-pc-windows-msvc/termcraft.exe")

    def test_missing_supported_release_is_an_explicit_noop(self):
        with patch.object(d, "download", return_value=b'[{"draft":false,"assets":[]}]'):
            result = d.fetch(self.root / "candidate", "homebrew")
        self.assertFalse(result["available"])

    def test_winget_extraction_rejects_unexpected_files_and_changed_installer(self):
        output, manifest = self.prepared()
        d.write_json(output / "latest.json", manifest)
        directory = d.unpack_winget(output, self.root / "unpacked")
        self.assertEqual(len(list(directory.glob("*.yaml"))), 3)
        archive = output / "termcraft-winget-manifests.zip"
        with zipfile.ZipFile(archive, "a") as z:
            z.writestr("../outside.yaml", "{}")
        manifest["assets"]["winget-manifests"]["sha256"] = d.digest(archive.read_bytes())
        d.write_json(output / "latest.json", manifest)
        with self.assertRaisesRegex(ValueError, "layout"):
            d.unpack_winget(output, self.root / "unsafe")
        self.assertFalse((self.root / "outside.yaml").exists())

    @unittest.skipUnless(shutil.which("dpkg-deb") and shutil.which("gpg"), "Signing and Debian tools required")
    def test_signed_apt_index_verifies_and_matches_the_debian_payload(self):
        output, manifest = self.prepared()
        candidate = self.root / "candidate"
        (candidate / "manifests").mkdir(parents=True)
        d.write_json(candidate / "latest.json", manifest)
        d.write_json(candidate / "manifests" / (manifest["tag"] + ".json"), manifest)
        d.write_json(candidate / "release.json", dict(tag=manifest["tag"], manifest_sha256="c" * 64, generated_at=0))
        name = manifest["assets"]["deb-amd64"]["name"]
        shutil.copyfile(output / name, candidate / name)
        private = self.root / "signing"
        private.mkdir(mode=0o700)
        gpg = ["gpg", "--homedir", str(private), "--batch", "--no-tty"]
        self.addCleanup(lambda: subprocess.run(["gpgconf", "--homedir", str(private), "--kill", "gpg-agent"], capture_output=True))
        subprocess.run(gpg + ["--pinentry-mode", "loopback", "--passphrase", "",
                             "--quick-generate-key", "Termcraft disposable signing test", "ed25519", "sign", "1d"],
                       check=True, capture_output=True)
        key = subprocess.check_output(gpg + ["--export-secret-keys"], stderr=subprocess.DEVNULL)
        site = self.root / "site"
        d.build_apt(candidate, site, key)
        apt = site / "apt"
        subprocess.run(["gpgv", "--keyring", str(apt / "termcraft-archive-keyring.gpg"),
                        str(apt / "dists/beta/InRelease")], check=True, capture_output=True)
        packages = (apt / "dists/beta/main/binary-amd64/Packages").read_bytes()
        self.assertEqual(gzip.decompress((apt / "dists/beta/main/binary-amd64/Packages.gz").read_bytes()), packages)
        self.assertIn(manifest["assets"]["deb-amd64"]["sha256"].encode(), packages)
        self.assertEqual(d.digest((apt / "pool/main/t/termcraft" / name).read_bytes()),
                         manifest["assets"]["deb-amd64"]["sha256"])
        self.assertFalse(any(p.suffix == ".asc" for p in site.rglob("*")))


if __name__ == "__main__":
    unittest.main()
