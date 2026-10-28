#!/usr/bin/env python3
"""Validate and explicitly invoke a bridge bundled in this skill."""
import hashlib
import json
import os
from pathlib import Path
import platform
import sys

SOURCE_COMMIT = "9f70cbf3779211544739ff7998b93895b8618944"
PLUGIN_VERSION = "0.2.0"
TARGETS = {
    ("Linux", "x86_64"): "x86_64-unknown-linux-gnu",
    ("Linux", "aarch64"): "aarch64-unknown-linux-gnu",
    ("Darwin", "x86_64"): "x86_64-apple-darwin",
    ("Darwin", "arm64"): "aarch64-apple-darwin",
}


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
            if plugin["name"] != "waypost" or plugin["version"] != PLUGIN_VERSION:
                raise ValueError("Plugin version or identity does not match bridge launcher")
        metadata = json.loads((root / "manifest.json").read_text(encoding="utf-8"))
        expected = metadata["sha256"]
        if (metadata["source_commit"] != SOURCE_COMMIT or metadata["plugin_version"] != PLUGIN_VERSION
                or metadata["target"] != target or metadata["bridge_version"] != "0.1.0"
                or not isinstance(expected, str) or len(expected) != 64
                or any(c not in "0123456789abcdef" for c in expected)):
            raise ValueError("Bridge manifest does not match this plugin or platform")
        if not binary.is_file() or binary.is_symlink():
            raise ValueError("Bundled bridge is missing or is a symlink")
        digest = hashlib.sha256(binary.read_bytes()).hexdigest()
    except (OSError, KeyError, TypeError, json.JSONDecodeError) as exc:
        raise ValueError("Bundled bridge or manifest unavailable; build a platform bundle first") from exc
    if digest != expected:
        raise ValueError("Bundled bridge checksum mismatch")
    if not os.access(binary, os.X_OK):
        raise ValueError("Bundled bridge is not executable")
    return target, binary


def main():
    args = sys.argv[1:]
    if not args or args[0] not in ("doctor", "bridge") or (args[0] == "doctor" and len(args) != 1):
        print("Usage: python3 bridge.py doctor | bridge [bridge arguments...]", file=sys.stderr)
        return 2
    if args[0] == "bridge" and len(args) == 1:
        print("Specify bridge arguments explicitly; no bridge was started.", file=sys.stderr)
        return 2
    try:
        target, binary = verified_binary()
    except ValueError as exc:
        print(str(exc), file=sys.stderr)
        return 1
    if args[0] == "doctor":
        print("Verified bundled bridge: " + target + " (plugin " + PLUGIN_VERSION + ")")
        print("Checksum verifies package consistency, not the publisher's identity.")
        return 0
    try:
        os.execv(str(binary), [str(binary), *args[1:]])
    except OSError as exc:
        print("Could not execute bundled bridge: " + str(exc), file=sys.stderr)
        return 1


if __name__ == "__main__":
    sys.exit(main())
