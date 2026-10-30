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
    filename = f"miles-works-plugin-{target}.tar.gz"
    archive = input_dir / target / filename
    checksum = input_dir / target / (filename + ".sha256")
    pieces = checksum.read_text().split()
    if len(pieces) != 2 or pieces[1] != filename or pieces[0] != digest(archive):
        raise ValueError(f"archive checksum mismatch: {target}")
    dest.mkdir()
    with tarfile.open(archive) as tar:
        for member in tar.getmembers():
            p = PurePosixPath(member.name)
            if p.is_absolute() or ".." in p.parts or not p.parts or p.parts[0] != "miles-works-plugin" or not (member.isdir() or member.isfile()):
                raise ValueError(f"unsafe archive member: {target}")
        tar.extractall(dest, filter="data")
    plugin = dest / "miles-works-plugin"
    for file in (plugin / "plugin.json", plugin / ".claude-plugin/plugin.json"):
        manifest = json.loads(file.read_text())
        if manifest.get("name") != "miles-works" or manifest.get("version") != build_bundle.VERSION:
            raise ValueError(f"plugin identity mismatch: {target}")
    artifact_root = plugin / "skills/waypost/scripts/artifacts"
    if {p.name for p in artifact_root.iterdir()} != {target}:
        raise ValueError(f"unexpected artifact targets: {target}")
    artifact = artifact_root / target
    if {p.name for p in artifact.iterdir()} != {"manifest.json", "waypost-bridge"}:
        raise ValueError(f"unexpected artifact contents: {target}")
    data = json.loads((artifact / "manifest.json").read_text())
    binary = artifact / "waypost-bridge"
    if (not build_bundle.valid_provenance(data) or data.get("plugin_version") != build_bundle.VERSION
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
        canonical = file_hashes(ROOT / "plugins/miles-works")
        if any(file_hashes(plugin) != canonical for plugin in plugins):
            raise ValueError("bundle plugin files do not match this source checkout")
        market = work / "market"
        for name in (".agents/plugins/marketplace.json", ".claude-plugin/marketplace.json"):
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
            for plugin_name in ("miles-works",):
                entries = [p for p in catalog["plugins"] if p["name"] == plugin_name]
                if len(entries) != 1:
                    raise ValueError(f"marketplace must contain exactly one {plugin_name} entry")
                path = "./plugins/" + plugin_name
                entries[0]["source"] = ({"source": "local", "path": path}
                                        if name.startswith(".agents/") else path)
            if len(catalog["plugins"]) != 1:
                raise ValueError("marketplace must contain only miles-works")
            file.write_text(json.dumps(catalog, indent=2) + "\n")
        shutil.copytree(plugins[0], market / "plugins/miles-works")
        artifacts = market / "plugins/miles-works/skills/waypost/scripts/artifacts"
        for target, plugin in zip(TARGETS[1:], plugins[1:]):
            shutil.copytree(plugin / "skills/waypost/scripts/artifacts" / target, artifacts / target)
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
