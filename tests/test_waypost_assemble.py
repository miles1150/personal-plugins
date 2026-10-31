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
    def setUp(self):
        # Fixtures use synthetic provenance; do not inherit the runner's real SHA.
        self.github_sha = patch.dict(os.environ)
        self.github_sha.start()
        os.environ.pop("GITHUB_SHA", None)
        self.addCleanup(self.github_sha.stop)

    def fixture(self, root):
        inputs = root / "inputs"
        for target in assembly.TARGETS:
            plugin = root / target / "miles-works-plugin"
            shutil.copytree(ROOT / "plugins/miles-works", plugin)
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
            (art / "manifest.json").write_text(json.dumps({"source_kind": "monorepo", "source_commit": "a" * 40,
                "monorepo_commit": "a" * 40, "bridge_tree": "b" * 40, "release_status": "unreleased",
                "plugin_version": build_bundle.VERSION, "bridge_version": "0.1.0", "target": target,
                "sha256": hashlib.sha256(data).hexdigest()}))
            d = inputs / target
            d.mkdir(parents=True)
            name = f"miles-works-plugin-{target}.tar.gz"
            with tarfile.open(d / name, "w:gz") as tar:
                tar.add(plugin, arcname="miles-works-plugin")
            (d / (name + ".sha256")).write_text(hashlib.sha256((d / name).read_bytes()).hexdigest() + "  " + name + "\n")
        return inputs

    def test_monorepo_artifact_cannot_fall_back_to_legacy_source(self):
        with tempfile.TemporaryDirectory() as d:
            root = Path(d)
            inputs = self.fixture(root)
            target = assembly.TARGETS[0]
            extracted = root / "changed"
            archive = inputs / target / f"miles-works-plugin-{target}.tar.gz"
            with tarfile.open(archive) as tar:
                tar.extractall(extracted, filter="data")
            manifest = extracted / "miles-works-plugin/skills/waypost/scripts/artifacts" / target / "manifest.json"
            data = json.loads(manifest.read_text())
            data.pop("bridge_tree")
            manifest.write_text(json.dumps(data))
            with tarfile.open(archive, "w:gz") as tar:
                tar.add(extracted / "miles-works-plugin", arcname="miles-works-plugin")
            archive.with_name(archive.name + ".sha256").write_text(assembly.digest(archive) + "  " + archive.name + "\n")
            with self.assertRaisesRegex(ValueError, "metadata or checksum mismatch"):
                assembly.assemble(inputs, root / "market")
            self.assertFalse((root / "market").exists())

    def test_mixed_bridge_commits_are_rejected(self):
        with tempfile.TemporaryDirectory() as d:
            root = Path(d)
            inputs = self.fixture(root)
            target = assembly.TARGETS[-1]
            archive = inputs / target / f"miles-works-plugin-{target}.tar.gz"
            extracted = root / "changed"
            with tarfile.open(archive) as tar:
                tar.extractall(extracted, filter="data")
            manifest = extracted / "miles-works-plugin/skills/waypost/scripts/artifacts" / target / "manifest.json"
            data = json.loads(manifest.read_text())
            data["source_commit"] = data["monorepo_commit"] = "b" * 40
            manifest.write_text(json.dumps(data))
            with tarfile.open(archive, "w:gz") as tar:
                tar.add(extracted / "miles-works-plugin", arcname="miles-works-plugin")
            archive.with_name(archive.name + ".sha256").write_text(assembly.digest(archive) + "  " + archive.name + "\n")
            with self.assertRaisesRegex(ValueError, "different source commits"):
                assembly.assemble(inputs, root / "market")
            self.assertFalse((root / "market").exists())

    def test_catalog_order_does_not_change_waypost_rewrite(self):
        with tempfile.TemporaryDirectory() as d:
            root = Path(d)
            inputs = self.fixture(root)
            source = root / "source"
            for name in ("plugins/miles-works", ".agents/plugins", ".claude-plugin"):
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
                waypost = next(entry for entry in catalog["plugins"] if entry["name"] == "miles-works")
                expected = {"source": "local", "path": "./plugins/miles-works"} if name.startswith(".agents/") else "./plugins/miles-works"
                self.assertEqual(waypost["source"], expected)

    def test_assemble_all_platforms_and_refuse_overwrite(self):
        with tempfile.TemporaryDirectory() as d:
            root = Path(d)
            inputs = self.fixture(root)
            out = root / "market"
            assembly.assemble(inputs, out)
            refs = out / "plugins/miles-works/skills/waypost/references"
            self.assertTrue((out / "plugins/miles-works/RELEASE-NOTES.md").is_file())
            self.assertTrue((refs / "setup.md").is_file())
            self.assertIn("login keychain", (refs / "setup.md").read_text())
            template = json.loads((refs / "bridge-v3.example.json").read_text())
            self.assertEqual(template["config_version"], 3)
            self.assertIn("codex_path", template["sessions"][0])
            webhook = json.loads((refs / "bridge-v3-webhook.example.json").read_text())
            self.assertEqual(webhook["config_version"], 3)
            self.assertFalse(webhook["sessions"][0]["webhook"]["local_loopback"])
            self.assertTrue((out / ".agents/plugins/marketplace.json").is_file())
            self.assertTrue((out / ".claude-plugin/marketplace.json").is_file())
            codex = json.loads((out / ".agents/plugins/marketplace.json").read_text())
            claude = json.loads((out / ".claude-plugin/marketplace.json").read_text())
            self.assertEqual(next(x for x in codex["plugins"] if x["name"] == "miles-works")["source"], {"source": "local", "path": "./plugins/miles-works"})
            self.assertEqual(next(x for x in claude["plugins"] if x["name"] == "miles-works")["source"], "./plugins/miles-works")
            self.assertEqual(next(x for x in codex["plugins"] if x["name"] == "miles-works")["source"], {"source": "local", "path": "./plugins/miles-works"})
            self.assertEqual(next(x for x in claude["plugins"] if x["name"] == "miles-works")["source"], "./plugins/miles-works")
            for target in assembly.TARGETS:
                self.assertTrue((out / "plugins/miles-works/skills/waypost/scripts/artifacts" / target / "waypost-bridge").is_file())
                self.assertTrue((out / "plugins/miles-works/skills/waypost/scripts/artifacts" / target / "waypost-bridge").is_file())
            for entry in codex["plugins"]:
                self.assertTrue((out / entry["source"]["path"]).is_dir(), entry["name"])
            self.assertEqual({p.name for p in (out / "plugins").iterdir()}, {"miles-works"})
            with self.assertRaisesRegex(ValueError, "already exist"):
                assembly.assemble(inputs, out)

    def test_ci_rejects_artifacts_from_another_commit(self):
        with tempfile.TemporaryDirectory() as d:
            root = Path(d)
            inputs = self.fixture(root)
            with patch.dict(os.environ, {"GITHUB_SHA": "c" * 40}), \
                 patch.object(build_bundle, "git", side_effect=["c" * 40, "b" * 40]):
                with self.assertRaisesRegex(ValueError, "provenance does not match"):
                    assembly.assemble(inputs, root / "market")
            self.assertFalse((root / "market").exists())

    def test_bad_archive_digest_is_rejected(self):
        with tempfile.TemporaryDirectory() as d:
            root = Path(d)
            inputs = self.fixture(root)
            target = assembly.TARGETS[0]
            archive = inputs / target / f"miles-works-plugin-{target}.tar.gz"
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
            name = f"miles-works-plugin-{target}.tar.gz"
            with tarfile.open(src / name, "w:gz") as tar:
                info = tarfile.TarInfo("miles-works-plugin/../../escape")
                info.size = 1
                tar.addfile(info, io.BytesIO(b"x"))
            (src / (name + ".sha256")).write_text(assembly.digest(src / name) + "  " + name + "\n")
            with self.assertRaisesRegex(ValueError, "unsafe archive"):
                assembly.unpack(root, target, root / "out")


if __name__ == "__main__":
    unittest.main()
