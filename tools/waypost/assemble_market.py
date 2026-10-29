#!/usr/bin/env python3
"""Assemble a local dual-host marketplace from three CI bridge bundles."""
import argparse
import hashlib
import json
import os
from pathlib import Path, PurePosixPath
import shutil
import sys
import tarfile
import tempfile

import build_bundle

TARGETS = tuple(build_bundle.TARGETS.values())
TARGETS = tuple(t for t in TARGETS if t != "aarch64-unknown-linux-gnu")
ROOT = Path(__file__).resolve().parents[2]


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def unpack(input_dir, target, dest):
    filename = f"waypost-plugin-{target}.tar.gz"
    archive = input_dir / target / filename
    checksum = input_dir / target / (filename + ".sha256")
    pieces = checksum.read_text().split()
    if len(pieces) != 2 or pieces[1] != filename or pieces[0] != digest(archive):
        raise ValueError(f"archive checksum mismatch: {target}")
    dest.mkdir()
    with tarfile.open(archive) as tar:
        for member in tar.getmembers():
            p = PurePosixPath(member.name)
            if p.is_absolute() or ".." in p.parts or not p.parts or p.parts[0] != "waypost-plugin" or not (member.isdir() or member.isfile()):
                raise ValueError(f"unsafe archive member: {target}")
        tar.extractall(dest, filter="data")
    plugin = dest / "waypost-plugin"
    for file in (plugin / "plugin.json", plugin / ".claude-plugin/plugin.json"):
        manifest = json.loads(file.read_text())
        if manifest.get("name") != "waypost" or manifest.get("version") != build_bundle.VERSION:
            raise ValueError(f"plugin identity mismatch: {target}")
    artifact_root = plugin / "skills/waypost/scripts/artifacts"
    if {p.name for p in artifact_root.iterdir()} != {target}:
        raise ValueError(f"unexpected artifact targets: {target}")
    artifact = artifact_root / target
    if {p.name for p in artifact.iterdir()} != {"manifest.json", "waypost-bridge"}:
        raise ValueError(f"unexpected artifact contents: {target}")
    data = json.loads((artifact / "manifest.json").read_text())
    binary = artifact / "waypost-bridge"
    if (data.get("source_commit") != build_bundle.COMMIT or data.get("plugin_version") != build_bundle.VERSION
            or data.get("bridge_version") != "0.1.0" or data.get("target") != target
            or data.get("sha256") != digest(binary) or not os.access(binary, os.X_OK)):
        raise ValueError(f"artifact metadata or checksum mismatch: {target}")
    build_bundle.check_binary(binary, target)
    return plugin


def file_hashes(plugin):
    return {str(p.relative_to(plugin)): digest(p) for p in plugin.rglob("*")
            if p.is_file() and "artifacts" not in p.relative_to(plugin).parts and "__pycache__" not in p.relative_to(plugin).parts}


def assemble(input_dir, output):
    if output.exists() or output.is_symlink():
        raise ValueError("output must not already exist")
    with tempfile.TemporaryDirectory(prefix="waypost-market-") as d:
        work = Path(d)
        plugins = [unpack(input_dir, target, work / target) for target in TARGETS]
        canonical = file_hashes(ROOT / "plugins/waypost")
        if any(file_hashes(plugin) != canonical for plugin in plugins):
            raise ValueError("bundle plugin files do not match this source checkout")
        market = work / "market"
        for name in (".agents/plugins/marketplace.json", ".claude-plugin/marketplace.json",
                     "plugins/keyenv/plugin.json", "plugins/keyenv/.claude-plugin/plugin.json",
                     "plugins/keyenv/skills/keyenv/SKILL.md",
                     "plugins/keyenv-remote/plugin.json", "plugins/keyenv-remote/.claude-plugin/plugin.json",
                     "plugins/keyenv-remote/.cursor-plugin/plugin.json", "plugins/keyenv-remote/.mcp.json",
                     "plugins/keyenv-remote/skills/keyenv-remote/SKILL.md"):
            source = ROOT / name
            if source.is_symlink() or not source.is_file():
                raise ValueError(f"missing or linked marketplace input: {name}")
            dest = market / name
            dest.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(source, dest)
        # The distributable includes the plugin, so it must install offline and
        # never fetch the source checkout's separately pinned Git entry.
        for name in (".agents/plugins/marketplace.json", ".claude-plugin/marketplace.json"):
            file = market / name
            catalog = json.loads(file.read_text())
            for plugin_name in ("waypost", "miles-works"):
                entries = [p for p in catalog["plugins"] if p["name"] == plugin_name]
                if len(entries) != 1:
                    raise ValueError(f"marketplace must contain exactly one {plugin_name} entry")
                path = "./plugins/" + plugin_name
                entries[0]["source"] = ({"source": "local", "path": path}
                                        if name.startswith(".agents/") else path)
            file.write_text(json.dumps(catalog, indent=2) + "\n")
        shutil.copytree(plugins[0], market / "plugins/waypost")
        artifacts = market / "plugins/waypost/skills/waypost/scripts/artifacts"
        for target, plugin in zip(TARGETS[1:], plugins[1:]):
            shutil.copytree(plugin / "skills/waypost/scripts/artifacts" / target, artifacts / target)
        # The umbrella package has the same Waypost launcher and matching
        # version. Reuse the already validated platform artifacts, not PATH.
        umbrella = ROOT / "plugins/miles-works"
        if any(p.is_symlink() for p in umbrella.rglob("*")):
            raise ValueError("umbrella plugin must not contain symlinks")
        for manifest in (umbrella / "plugin.json", umbrella / ".claude-plugin/plugin.json"):
            identity = json.loads(manifest.read_text())
            if identity.get("name") != "miles-works" or identity.get("version") != build_bundle.VERSION:
                raise ValueError("umbrella plugin identity mismatch")
        for module in ("waypost", "keyenv", "keyenv-remote"):
            if file_hashes(umbrella / "skills" / module) != file_hashes(ROOT / "plugins" / module / "skills" / module):
                raise ValueError(f"umbrella {module} skill differs from module source")
        for name in ("mcp.json", ".mcp.json"):
            configured = json.loads((umbrella / name).read_text())["mcpServers"]
            waypost = json.loads((ROOT / "plugins/waypost" / name).read_text())["mcpServers"]["waypost"]
            remote = json.loads((ROOT / "plugins/keyenv-remote/.mcp.json").read_text())["mcpServers"]["keyenv-remote"]
            if name == "mcp.json":
                remote = {**remote, "type": "streamable-http"}
            if configured != {"waypost": waypost, "keyenv-remote": remote}:
                raise ValueError("umbrella MCP differs from module sources")
        shutil.copytree(umbrella, market / "plugins/miles-works", ignore=shutil.ignore_patterns("__pycache__", "artifacts"))
        shutil.copytree(artifacts, market / "plugins/miles-works/skills/waypost/scripts/artifacts")
        output.parent.mkdir(parents=True, exist_ok=True)
        output.mkdir()
        try:
            shutil.copytree(market, output, dirs_exist_ok=True)
        except BaseException:
            shutil.rmtree(output)
            raise
    print("Assembled marketplace:", output)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--input", required=True, type=Path, help="directories named for each target with tar and sha256")
    parser.add_argument("--output", required=True, type=Path, help="new marketplace directory")
    args = parser.parse_args()
    try:
        assemble(args.input.resolve(), args.output.absolute())
    except (OSError, ValueError, KeyError, tarfile.TarError) as exc:
        print("Marketplace assembly failed:", str(exc), file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
