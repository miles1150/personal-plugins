import hashlib
import importlib.util
import io
import json
import os
from pathlib import Path
import shutil
import struct
import sys
import tarfile
import tempfile
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "tools/waypost"))
import assemble_market as assembly
import build_bundle


class AssemblyTest(unittest.TestCase):
    def fixture(self, root):
        inputs = root / "inputs"
        for target in assembly.TARGETS:
            plugin = root / target / "waypost-plugin"
            shutil.copytree(ROOT / "plugins/waypost", plugin)
            art = plugin / "skills/waypost/scripts/artifacts" / target
            art.mkdir(parents=True)
            binary = art / "waypost-bridge"
            if target.endswith("linux-gnu"):
                data = bytearray(64)
                data[:6] = b"\x7fELF\x02\x01"
                struct.pack_into("<H", data, 18, 62)
            else:
                data = bytearray(64)
                data[:4] = b"\xcf\xfa\xed\xfe"
                struct.pack_into("<I", data, 4, 0x01000007 if target.startswith("x86") else 0x0100000c)
            binary.write_bytes(data)
            binary.chmod(0o755)
            (art / "manifest.json").write_text(json.dumps({"source_commit": build_bundle.COMMIT,
                "plugin_version": build_bundle.VERSION, "bridge_version": "0.1.0", "target": target,
                "sha256": hashlib.sha256(data).hexdigest()}))
            d = inputs / target
            d.mkdir(parents=True)
            name = f"waypost-plugin-{target}.tar.gz"
            with tarfile.open(d / name, "w:gz") as tar:
                tar.add(plugin, arcname="waypost-plugin")
            (d / (name + ".sha256")).write_text(hashlib.sha256((d / name).read_bytes()).hexdigest() + "  " + name + "\n")
        return inputs

    def test_catalog_order_does_not_change_waypost_rewrite(self):
        with tempfile.TemporaryDirectory() as d:
            root = Path(d)
            inputs = self.fixture(root)
            source = root / "source"
            for name in ("plugins/waypost", "plugins/keyenv", "plugins/keyenv-remote", "plugins/miles-works", ".agents/plugins", ".claude-plugin"):
                dest = source / name
                dest.parent.mkdir(parents=True, exist_ok=True)
                shutil.copytree(ROOT / name, dest)
            for name in (".agents/plugins/marketplace.json", ".claude-plugin/marketplace.json"):
                path = source / name
                catalog = json.loads(path.read_text())
                catalog["plugins"].reverse()
                path.write_text(json.dumps(catalog))
            out = root / "market"
            with patch.object(assembly, "ROOT", source):
                assembly.assemble(inputs, out)
            for name in (".agents/plugins/marketplace.json", ".claude-plugin/marketplace.json"):
                catalog = json.loads((out / name).read_text())
                waypost = next(entry for entry in catalog["plugins"] if entry["name"] == "waypost")
                expected = {"source": "local", "path": "./plugins/waypost"} if name.startswith(".agents/") else "./plugins/waypost"
                self.assertEqual(waypost["source"], expected)

    def test_assemble_all_platforms_and_refuse_overwrite(self):
        with tempfile.TemporaryDirectory() as d:
            root = Path(d)
            inputs = self.fixture(root)
            out = root / "market"
            assembly.assemble(inputs, out)
            self.assertTrue((out / ".agents/plugins/marketplace.json").is_file())
            self.assertTrue((out / ".claude-plugin/marketplace.json").is_file())
            codex = json.loads((out / ".agents/plugins/marketplace.json").read_text())
            claude = json.loads((out / ".claude-plugin/marketplace.json").read_text())
            self.assertEqual(next(x for x in codex["plugins"] if x["name"] == "waypost")["source"], {"source": "local", "path": "./plugins/waypost"})
            self.assertEqual(next(x for x in claude["plugins"] if x["name"] == "waypost")["source"], "./plugins/waypost")
            for target in assembly.TARGETS:
                self.assertTrue((out / "plugins/waypost/skills/waypost/scripts/artifacts" / target / "waypost-bridge").is_file())
                self.assertTrue((out / "plugins/miles-works/skills/waypost/scripts/artifacts" / target / "waypost-bridge").is_file())
            for entry in codex["plugins"]:
                self.assertTrue((out / entry["source"]["path"]).is_dir(), entry["name"])
            self.assertTrue((out / "plugins/keyenv-remote/.mcp.json").is_file())
            with self.assertRaisesRegex(ValueError, "already exist"):
                assembly.assemble(inputs, out)

    def test_bad_archive_digest_is_rejected(self):
        with tempfile.TemporaryDirectory() as d:
            root = Path(d)
            inputs = self.fixture(root)
            target = assembly.TARGETS[0]
            archive = inputs / target / f"waypost-plugin-{target}.tar.gz"
            with archive.open("ab") as f:
                f.write(b"corrupt")
            with self.assertRaisesRegex(ValueError, "checksum mismatch"):
                assembly.assemble(inputs, root / "market")
            self.assertFalse((root / "market").exists())

    def test_archive_path_traversal_rejected(self):
        with tempfile.TemporaryDirectory() as d:
            root = Path(d)
            target = assembly.TARGETS[0]
            src = root / target
            src.mkdir()
            name = f"waypost-plugin-{target}.tar.gz"
            with tarfile.open(src / name, "w:gz") as tar:
                info = tarfile.TarInfo("waypost-plugin/../../escape")
                info.size = 1
                tar.addfile(info, io.BytesIO(b"x"))
            (src / (name + ".sha256")).write_text(assembly.digest(src / name) + "  " + name + "\n")
            with self.assertRaisesRegex(ValueError, "unsafe archive"):
                assembly.unpack(root, target, root / "out")


if __name__ == "__main__":
    unittest.main()
