#!/usr/bin/env python3
"""Build a single-platform, offline Waypost plugin bundle from pinned source."""
import argparse
import hashlib
import json
import os
from pathlib import Path, PurePosixPath
import platform
import shutil
import struct
import subprocess
import sys
import tempfile
import tomllib
import tarfile

COMMIT = "9f70cbf3779211544739ff7998b93895b8618944"
# New monorepo builds are unreleased; COMMIT remains the legacy upstream pin only.
BRIDGE_TREE = "763e6d567f192cf073940d6c37eae049b3f9a578"
VERSION = "0.4.0"
ROOT = Path(__file__).resolve().parents[2]
TARGETS = {
    ("Linux", "x86_64"): "x86_64-unknown-linux-gnu",
    ("Linux", "aarch64"): "aarch64-unknown-linux-gnu",
    ("Darwin", "x86_64"): "x86_64-apple-darwin",
    ("Darwin", "arm64"): "aarch64-apple-darwin",
}


def git(source, *args):
    return subprocess.check_output(["git", "-C", str(source), *args], text=True).strip()


def valid_git_id(value):
    return isinstance(value, str) and len(value) == 40 and all(c in "0123456789abcdef" for c in value)


def valid_provenance(metadata):
    """Do not let new monorepo bytes masquerade as the old upstream commit."""
    if not isinstance(metadata, dict):
        return False
    if metadata.get("source_kind") == "monorepo":
        return (metadata.get("bridge_tree") == BRIDGE_TREE
                and valid_git_id(metadata.get("source_commit"))
                and metadata.get("source_commit") != COMMIT
                and metadata.get("monorepo_commit") == metadata.get("source_commit")
                and metadata.get("release_status") == "unreleased")
    # Explicit compatibility for the already pinned upstream artifact format.
    if metadata.get("source_kind") in (None, "upstream"):
        return (metadata.get("source_commit") == COMMIT
                and "bridge_tree" not in metadata and "monorepo_commit" not in metadata
                and metadata.get("release_status") in (None, "unreleased"))
    return False


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


def validate_monorepo(source):
    if git(source, "rev-parse", "HEAD:services/waypost/bridge") != BRIDGE_TREE:
        raise ValueError("Monorepo bridge tree does not match the pinned bridge source")
    if git(source, "status", "--porcelain", "--untracked-files=all", "--", "services/waypost/bridge"):
        raise ValueError("Monorepo bridge checkout must be clean")


def export_monorepo(source, destination):
    archive = destination.parent / "bridge.tar"
    with archive.open("wb") as stream:
        subprocess.run(["git", "-C", str(source), "archive", "--format=tar", BRIDGE_TREE],
                       stdout=stream, check=True)
    bridge = destination / "bridge"
    bridge.mkdir(parents=True)
    with tarfile.open(archive) as tar:
        for entry in tar.getmembers():
            path = PurePosixPath(entry.name)
            if path.is_absolute() or ".." in path.parts or not (entry.isdir() or entry.isfile()):
                raise ValueError("Unsafe entry in source archive")
        tar.extractall(bridge, filter="data")


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


def export_source(source, destination):
    """Build from committed bytes, excluding ignored and untracked worktree files."""
    archive = destination.parent / "source.tar"
    with archive.open("wb") as stream:
        subprocess.run(["git", "-C", str(source), "archive", "--format=tar", COMMIT],
                       stdout=stream, check=True)
    destination.mkdir()
    with tarfile.open(archive) as tar:
        for entry in tar.getmembers():
            path = PurePosixPath(entry.name)
            if path.is_absolute() or ".." in path.parts or not (entry.isdir() or entry.isfile()):
                raise ValueError("Unsafe entry in source archive")
        tar.extractall(destination, filter="data")


def build(source, output, monorepo=False):
    source = source.resolve()
    if output.is_symlink():
        raise ValueError("Output must not already exist")
    output = output.resolve()
    target = TARGETS.get((platform.system(), platform.machine()))
    if not target:
        raise ValueError("Unsupported native build platform")
    if output.exists() or output.is_symlink():
        raise ValueError("Output must not already exist")
    if source == output or source in output.parents:
        raise ValueError("Output must be outside the Waypost checkout")
    validate = validate_monorepo if monorepo else validate_source
    validate(source)
    plugin = ROOT / "plugins/waypost"
    for path in [plugin / "plugin.json", plugin / ".claude-plugin/plugin.json"]:
        if json.loads(path.read_text())["version"] != VERSION:
            raise ValueError("Plugin version mismatch")
    with tempfile.TemporaryDirectory(prefix="waypost-bridge-build-") as tmp:
        tmp_root = Path(tmp)
        exported = tmp_root / "source"
        (export_monorepo if monorepo else export_source)(source, exported)
        env = os.environ.copy()
        env["CARGO_TARGET_DIR"] = str(tmp_root / "target")
        subprocess.run(["cargo", "build", "--release", "--locked", "--offline", "--target", target,
                        "--manifest-path", str(exported / "bridge/Cargo.toml")], env=env, check=True)
        binary = tmp_root / "target" / target / "release/waypost-bridge"
        check_binary(binary, target)
        validate(source)
        # Stage first, then reserve the destination without replacing an existing path.
        output.parent.mkdir(parents=True, exist_ok=True)
        with tempfile.TemporaryDirectory(prefix="waypost-bundle-", dir=output.parent) as staging:
            bundle = Path(staging) / "waypost"
            shutil.copytree(plugin, bundle, ignore=shutil.ignore_patterns("__pycache__", "artifacts"))
            artifacts = bundle / "skills/waypost/scripts/artifacts" / target
            artifacts.mkdir(parents=True)
            dest = artifacts / "waypost-bridge"
            shutil.copyfile(binary, dest)
            dest.chmod(0o755)
            metadata = {"source_kind": "monorepo" if monorepo else "upstream",
                        "source_commit": git(source, "rev-parse", "HEAD") if monorepo else COMMIT,
                        "release_status": "unreleased", "plugin_version": VERSION, "bridge_version": "0.1.0",
                        "target": target, "sha256": hashlib.sha256(dest.read_bytes()).hexdigest(),
                        "cargo_version": subprocess.check_output(["cargo", "--version"], text=True).strip(),
                        "rustc_version": subprocess.check_output(["rustc", "--version"], text=True).strip()}
            if monorepo:
                metadata["bridge_tree"] = BRIDGE_TREE
                metadata["monorepo_commit"] = metadata["source_commit"]
            (artifacts / "manifest.json").write_text(json.dumps(metadata, indent=2) + "\n")
            output.mkdir()
            try:
                shutil.copytree(bundle, output, dirs_exist_ok=True)
            except BaseException:
                shutil.rmtree(output)
                raise
    print("Built offline bundle:", output, "target:", target)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source", type=Path, required=True, help="clean Waypost checkout at the pinned commit")
    parser.add_argument("--monorepo", action="store_true", help="source is a personal-plugins checkout containing services/waypost")
    parser.add_argument("--output", type=Path, required=True, help="new plugin directory outside the source checkout")
    args = parser.parse_args()
    try:
        build(args.source, args.output, args.monorepo)
    except (OSError, ValueError, KeyError, subprocess.CalledProcessError) as exc:
        print("Bundle build failed: " + str(exc), file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
