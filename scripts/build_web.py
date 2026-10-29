#!/usr/bin/env python3
"""Build the real shared engine for a static site; requires wasm-bindgen 0.2.105."""
import argparse
import pathlib
import shutil
import subprocess

ROOT = pathlib.Path(__file__).resolve().parent.parent
VERSION = "0.2.105"

def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--bindgen", default="wasm-bindgen")
    parser.add_argument("--node-output", type=pathlib.Path)
    args = parser.parse_args()
    tool = shutil.which(args.bindgen)
    if tool is None:
        parser.error(f"install with: cargo install wasm-bindgen-cli --version {VERSION} --locked")
    actual = subprocess.check_output([tool, "--version"], text=True).strip()
    if actual != f"wasm-bindgen {VERSION}":
        parser.error(f"expected wasm-bindgen {VERSION}, got {actual}")
    subprocess.run(["cargo", "build", "--locked", "--release", "--lib", "--target", "wasm32-unknown-unknown"], cwd=ROOT, check=True)
    binary = ROOT / "target/wasm32-unknown-unknown/release/termcraft.wasm"
    targets = [("web", ROOT / "docs/browser/pkg")]
    if args.node_output:
        targets.append(("nodejs", args.node_output.resolve()))
    for target, output in targets:
        output.mkdir(parents=True, exist_ok=True)
        subprocess.run([tool, str(binary), "--target", target, "--out-dir", str(output), "--no-typescript"], check=True)
    print("Browser game built at docs/browser/index.html")

if __name__ == "__main__":
    main()
