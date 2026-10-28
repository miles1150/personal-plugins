#!/usr/bin/env python3
"""Build a single-platform, offline Waypost plugin bundle from pinned source."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import shutil
import struct
import subprocess
import sys
import tempfile
import tomllib

COMMIT = "9f70cbf3779211544739ff7998b93895b8618944"
VERSION = "0.2.0"
ROOT = Path(__file__).resolve().parents[2]
TARGETS = {
    ("Linux", "x86_64"): "x86_64-unknown-linux-gnu",
    ("Linux", "aarch64"): "aarch64-unknown-linux-gnu",
    ("Darwin", "x86_64"): "x86_64-apple-darwin",
    ("Darwin", "arm64"): "aarch64-apple-darwin",
}


def git(source, *args):
    return subprocess.check_output(["git", "-C", str(source), *args], text=True).strip()


def validate_source(source):
    if git(source, "rev-parse", "HEAD") != COMMIT:
        raise ValueError("Waypost source HEAD does not match pinned commit")
    if git(source, "status", "--porcelain", "--untracked-files=all"):
        raise ValueError("Waypost source checkout must be clean, including untracked files")
    manifest = source / "bridge/Cargo.toml"
    lock = source / "bridge/Cargo.lock"
    if not manifest.is_file() or not lock.is_file():
        raise ValueError("Waypost bridge Cargo manifest or lockfile missing")
    with manifest.open("rb") as stream:
        package = tomllib.load(stream).get("package", {})
    if package.get("name") != "waypost-bridge" or package.get("version") != "0.1.0":
        raise ValueError("Waypost bridge package version does not match this plugin")


def check_binary(path, target):
    # Reject the common error of packaging a binary for a different OS or CPU.
    data = path.read_bytes()[:64]
    if target.endswith("linux-gnu"):
        machine = {"x86_64-unknown-linux-gnu": 62, "aarch64-unknown-linux-gnu": 183}[target]
        if len(data) < 20 or data[:6] != b"\x7fELF\x02\x01" or struct.unpack_from("<H", data, 18)[0] != machine:
            raise ValueError("Built binary is not the expected 64-bit Linux ELF architecture")
    else:
        cpu = {"x86_64-apple-darwin": 0x01000007, "aarch64-apple-darwin": 0x0100000C}[target]
        if len(data) < 8 or data[:4] != b"\xcf\xfa\xed\xfe" or struct.unpack_from("<I", data, 4)[0] != cpu:
            raise ValueError("Built binary is not the expected 64-bit macOS Mach-O architecture")


def build(source, output):
    source, output = source.resolve(), output.absolute()
    target = TARGETS.get((platform.system(), platform.machine()))
    if not target:
        raise ValueError("Unsupported native build platform")
    if output.exists() or output.is_symlink():
        raise ValueError("Output must not already exist")
    if source == output or source in output.parents:
        raise ValueError("Output must be outside the Waypost checkout")
    validate_source(source)
    plugin = ROOT / "plugins/waypost"
    for path in [plugin / "plugin.json", plugin / ".claude-plugin/plugin.json"]:
        if json.loads(path.read_text())["version"] != VERSION:
            raise ValueError("Plugin version mismatch")
    with tempfile.TemporaryDirectory(prefix="waypost-bridge-build-") as tmp:
        env = os.environ.copy()
        env["CARGO_TARGET_DIR"] = tmp
        subprocess.run(["cargo", "build", "--release", "--locked", "--offline", "--target", target,
                        "--manifest-path", str(source / "bridge/Cargo.toml")], env=env, check=True)
        binary = Path(tmp) / target / "release/waypost-bridge"
        check_binary(binary, target)
        validate_source(source)
        # Build in a sibling temporary directory, then publish as one local directory.
        output.parent.mkdir(parents=True, exist_ok=True)
        with tempfile.TemporaryDirectory(prefix="waypost-bundle-", dir=output.parent) as staging:
            bundle = Path(staging) / "waypost"
            shutil.copytree(plugin, bundle, ignore=shutil.ignore_patterns("__pycache__", "artifacts"))
            artifacts = bundle / "skills/waypost/scripts/artifacts" / target
            artifacts.mkdir(parents=True)
            dest = artifacts / "waypost-bridge"
            shutil.copyfile(binary, dest)
            dest.chmod(0o755)
            metadata = {"source_commit": COMMIT, "plugin_version": VERSION, "bridge_version": "0.1.0",
                        "target": target, "sha256": hashlib.sha256(dest.read_bytes()).hexdigest(),
                        "cargo_version": subprocess.check_output(["cargo", "--version"], text=True).strip(),
                        "rustc_version": subprocess.check_output(["rustc", "--version"], text=True).strip()}
            (artifacts / "manifest.json").write_text(json.dumps(metadata, indent=2) + "\n")
            os.rename(bundle, output)
    print("Built offline bundle:", output, "target:", target)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source", type=Path, required=True, help="clean Waypost checkout at the pinned commit")
    parser.add_argument("--output", type=Path, required=True, help="new plugin directory outside the source checkout")
    args = parser.parse_args()
    try:
        build(args.source, args.output)
    except (OSError, ValueError, KeyError, subprocess.CalledProcessError) as exc:
        print("Bundle build failed: " + str(exc), file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
