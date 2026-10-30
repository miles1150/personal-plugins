#!/usr/bin/env python3
"""Validate and explicitly invoke a bridge bundled in this skill."""
import hashlib
import json
import os
from pathlib import Path
import platform
import stat
import sys

SOURCE_COMMIT = "9f70cbf3779211544739ff7998b93895b8618944"
BRIDGE_TREE = "763e6d567f192cf073940d6c37eae049b3f9a578"
PLUGIN_VERSION = "0.4.0"
TARGETS = {
    ("Linux", "x86_64"): "x86_64-unknown-linux-gnu",
    ("Linux", "aarch64"): "aarch64-unknown-linux-gnu",
    ("Darwin", "x86_64"): "x86_64-apple-darwin",
    ("Darwin", "arm64"): "aarch64-apple-darwin",
}


def valid_provenance(metadata):
    if not isinstance(metadata, dict):
        return False
    if metadata.get("source_kind") == "monorepo":
        commit = metadata.get("source_commit")
        return (metadata.get("bridge_tree") == BRIDGE_TREE
                and isinstance(commit, str) and len(commit) == 40
                and all(c in "0123456789abcdef" for c in commit) and commit != SOURCE_COMMIT
                and metadata.get("monorepo_commit") == commit
                and metadata.get("release_status") == "unreleased")
    # Legacy upstream bundles remain valid only at their original commit pin.
    if metadata.get("source_kind") in (None, "upstream"):
        return (metadata.get("source_commit") == SOURCE_COMMIT
                and "bridge_tree" not in metadata and "monorepo_commit" not in metadata
                and metadata.get("release_status") in (None, "unreleased"))
    return False


def verified_binary():
    target = TARGETS.get((platform.system(), platform.machine()))
    if not target:
        raise ValueError("Unsupported platform: " + platform.system() + " " + platform.machine())
    root = Path(__file__).resolve().parent / "artifacts" / target
    binary = root / "waypost-bridge"
    try:
        plugin_root = Path(__file__).resolve().parents[3]
        for manifest in (plugin_root / "plugin.json", plugin_root / ".claude-plugin/plugin.json"):
            plugin = json.loads(manifest.read_text(encoding="utf-8"))
            if plugin["name"] not in ("waypost", "miles-works") or plugin["version"] != {"waypost": PLUGIN_VERSION, "miles-works": "0.5.0"}.get(plugin["name"]):
                raise ValueError("Plugin version or identity does not match bridge launcher")
        metadata = json.loads((root / "manifest.json").read_text(encoding="utf-8"))
        expected = metadata["sha256"]
        if (not valid_provenance(metadata) or metadata["plugin_version"] != PLUGIN_VERSION
                or metadata["target"] != target or metadata["bridge_version"] != "0.1.0"
                or not isinstance(expected, str) or len(expected) != 64
                or any(c not in "0123456789abcdef" for c in expected)):
            raise ValueError("Bridge manifest does not match this plugin or platform")
        if any(path.is_symlink() for path in (root.parent, root, root / "manifest.json", binary)):
            raise ValueError("Bundled bridge paths must not be symlinks")
        if not binary.is_file():
            raise ValueError("Bundled bridge is missing or is a symlink")
        fd = os.open(binary, os.O_RDONLY | getattr(os, "O_NOFOLLOW", 0))
        try:
            info = os.fstat(fd)
            if not stat.S_ISREG(info.st_mode):
                raise ValueError("Bundled bridge must be a regular file")
            digest = hashlib.sha256()
            while True:
                block = os.read(fd, 1024 * 1024)
                if not block:
                    break
                digest.update(block)
            if digest.hexdigest() != expected:
                raise ValueError("Bundled bridge checksum mismatch")
            if not info.st_mode & 0o111 or not os.access(binary, os.X_OK):
                raise ValueError("Bundled bridge is not executable")
            os.lseek(fd, 0, os.SEEK_SET)
        except BaseException:
            os.close(fd)
            raise
    except (OSError, KeyError, TypeError, json.JSONDecodeError) as exc:
        raise ValueError("Bundled bridge or manifest unavailable; build a platform bundle first") from exc
    return target, binary, fd


def main():
    args = sys.argv[1:]
    if not args or args[0] not in ("doctor", "bridge") or (args[0] == "doctor" and len(args) != 1):
        print("Usage: python3 bridge.py doctor | bridge [bridge arguments...]", file=sys.stderr)
        return 2
    if args[0] == "bridge" and len(args) == 1:
        print("Specify bridge arguments explicitly; no bridge was started.", file=sys.stderr)
        return 2
    try:
        target, binary, fd = verified_binary()
    except ValueError as exc:
        print(str(exc), file=sys.stderr)
        return 1
    if args[0] == "doctor":
        os.close(fd)
        print("Verified bundled bridge: " + target + " (plugin " + PLUGIN_VERSION + ")")
        print("Checksum verifies package consistency, not the publisher's identity.")
        print("Monorepo development bundles are unreleased; legacy upstream bundles retain their original pin.")
        return 0
    try:
        if os.execve in os.supports_fd:
            os.execve(fd, [str(binary), *args[1:]], os.environ)
        # Platforms without descriptor execution still have a path replacement race.
        os.close(fd)
        os.execve(str(binary), [str(binary), *args[1:]], os.environ)
    except OSError as exc:
        try:
            os.close(fd)
        except OSError:
            pass
        print("Could not execute bundled bridge: " + str(exc), file=sys.stderr)
        return 1


if __name__ == "__main__":
    sys.exit(main())
