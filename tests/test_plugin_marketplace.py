import hashlib
import importlib.util
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import tarfile
import io
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[1]
SCRIPT_DIR = ROOT / "plugins/waypost/skills/waypost/scripts"
SPEC = importlib.util.spec_from_file_location("bundle_builder", ROOT / "tools/waypost/build_bundle.py")
builder = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(builder)


class MarketplaceTests(unittest.TestCase):
    def test_markets_point_to_same_isolated_plugins(self):
        codex = json.loads((ROOT / ".agents/plugins/marketplace.json").read_text())
        claude = json.loads((ROOT / ".claude-plugin/marketplace.json").read_text())
        self.assertEqual(codex["name"], claude["name"])
        a = {p["name"]: p["source"] for p in codex["plugins"]}
        b = {p["name"]: p["source"] for p in claude["plugins"]}
        self.assertEqual(set(a), {"waypost", "keyenv", "keyenv-remote", "miles-works"})
        self.assertEqual(set(b), set(a))
        for name in ("keyenv", "keyenv-remote"):
            self.assertEqual(a[name], {"source": "local", "path": "./plugins/" + name})
            self.assertEqual(b[name], "./plugins/" + name)
        for entry in (a["waypost"], b["waypost"]):
            self.assertEqual(entry["source"], "git-subdir")
            self.assertEqual(entry["sha"], "e040011a7b0100e21a21fff2726af5dea8046de6")
        for entry in (a["miles-works"], b["miles-works"]):
            self.assertEqual(entry["source"], "git-subdir")
            self.assertEqual(entry["url"], "https://github.com/miles1150/personal-plugins.git")
            self.assertEqual(entry["sha"], "8bf0b376f1c97b5d78dcbaeeb4e7c5d3b749f458")
        for name in a:
            root = ROOT / "plugins" / name
            portable = json.loads((root / "plugin.json").read_text())
            native = json.loads((root / ".claude-plugin/plugin.json").read_text())
            self.assertEqual((portable["name"], portable["version"]), (name, native["version"]))
            self.assertEqual(native["name"], name)
            if name != "miles-works":
                self.assertTrue((root / "skills" / name / "SKILL.md").is_file())
        for name in ("waypost", "keyenv"):
            module = ROOT / "plugins" / name / "skills" / name
            shared = ROOT / "plugins/miles-works/skills" / name
            self.assertEqual({str(p.relative_to(module)): p.read_bytes() for p in module.rglob('*') if p.is_file() and '__pycache__' not in p.parts},
                             {str(p.relative_to(shared)): p.read_bytes() for p in shared.rglob('*') if p.is_file() and '__pycache__' not in p.parts})

    def test_keyenv_remote_is_mcp_only(self):
        root = ROOT / "plugins/keyenv-remote"
        manifests = [root / "plugin.json", root / ".claude-plugin/plugin.json", root / ".cursor-plugin/plugin.json"]
        versions = set()
        for path in manifests:
            manifest = json.loads(path.read_text())
            self.assertEqual(manifest["name"], "keyenv-remote")
            self.assertEqual(manifest["mcpServers"], "./.mcp.json")
            versions.add(manifest["version"])
            # Hosts discover client identity through OAuth; manifests carry no client IDs.
            self.assertNotIn("client_id", path.read_text())
        self.assertEqual(len(versions), 1)
        servers = json.loads((root / ".mcp.json").read_text())["mcpServers"]
        self.assertEqual(servers, {"keyenv-remote": {"type": "http", "url": "https://keys.miles1150.top/mcp"}})
        files = [p for p in root.rglob("*") if p.is_file()]
        self.assertEqual(sorted(str(p.relative_to(root)) for p in files), [".claude-plugin/plugin.json", ".cursor-plugin/plugin.json", ".mcp.json", "plugin.json", "skills/keyenv-remote/SKILL.md"])
        self.assertFalse(any(os.access(p, os.X_OK) for p in files))
        skill = (root / "skills/keyenv-remote/SKILL.md").read_text()
        for rule in ("chmod 700", "chmod 600", "git check-ignore", "discipline, not a sandbox", "list_entries"):
            self.assertIn(rule, skill)

    def test_missing_binary_never_falls_back_to_path(self):
        with tempfile.TemporaryDirectory() as d:
            fake = Path(d) / "waypost-bridge"
            fake.write_text("#!/bin/sh\necho WRONG\n")
            fake.chmod(0o755)
            result = subprocess.run([sys.executable, str(SCRIPT_DIR / "bridge.py"), "bridge", "status"],
                                    env={**os.environ, "PATH": d}, capture_output=True, text=True)
            self.assertEqual(result.returncode, 1)
            self.assertNotIn("WRONG", result.stdout)
            self.assertIn("unavailable", result.stderr)

    def test_unsupported_platform_and_wrong_architecture(self):
        spec = importlib.util.spec_from_file_location("bridge_runner", SCRIPT_DIR / "bridge.py")
        runner = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(runner)
        with patch.object(runner.platform, "system", return_value="Windows"):
            with self.assertRaisesRegex(ValueError, "Unsupported platform"):
                runner.verified_binary()
        with tempfile.TemporaryDirectory() as d:
            bad = Path(d) / "binary"
            bad.write_bytes(b"not an executable")
            with self.assertRaisesRegex(ValueError, "ELF architecture"):
                builder.check_binary(bad, "x86_64-unknown-linux-gnu")

    @unittest.skipUnless(sys.platform.startswith("linux") and os.uname().machine == "x86_64" and shutil.which("cc"), "requires native Linux x86_64 C compiler")
    def test_bundled_binary_validation_and_arguments(self):
        with tempfile.TemporaryDirectory() as d:
            root = Path(d)
            plugin = root / "plugin"
            shutil.copytree(ROOT / "plugins/waypost", plugin)
            scripts = plugin / "skills/waypost/scripts"
            artifacts = scripts / "artifacts/x86_64-unknown-linux-gnu"
            artifacts.mkdir(parents=True)
            source = root / "fixture.c"
            source.write_text('#include <stdio.h>\nint main(int n,char **v){for(int i=1;i<n;i++) puts(v[i]);return 37;}\n')
            binary = artifacts / "waypost-bridge"
            subprocess.run(["cc", str(source), "-o", str(binary)], check=True)
            manifest = {"source_kind": "monorepo", "source_commit": "a" * 40,
                        "monorepo_commit": "a" * 40, "bridge_tree": builder.BRIDGE_TREE, "release_status": "unreleased",
                        "plugin_version": builder.VERSION,
                        "bridge_version": "0.1.0", "target": "x86_64-unknown-linux-gnu",
                        "sha256": hashlib.sha256(binary.read_bytes()).hexdigest()}
            meta = artifacts / "manifest.json"
            meta.write_text(json.dumps(manifest))
            runner = scripts / "bridge.py"
            env = {**os.environ, "PATH": ""}
            def run(*args):
                return subprocess.run([sys.executable, str(runner), *args], env=env, capture_output=True, text=True)
            self.assertEqual(run("doctor").returncode, 0)
            # Current artifacts need the exact tree, real monorepo commit and development label.
            for key, bad in (("bridge_tree", "b" * 40), ("monorepo_commit", "b" * 40),
                             ("source_commit", builder.COMMIT), ("release_status", "released")):
                original = manifest[key]
                manifest[key] = bad
                meta.write_text(json.dumps(manifest))
                self.assertIn("does not match", run("doctor").stderr)
                manifest[key] = original
            original_tree = manifest.pop("bridge_tree")
            meta.write_text(json.dumps(manifest))
            self.assertIn("does not match", run("doctor").stderr)
            manifest["bridge_tree"] = original_tree
            meta.write_text(json.dumps(manifest))
            umbrella = root / "umbrella"
            shutil.copytree(ROOT / "plugins/miles-works", umbrella)
            umbrella_artifact = umbrella / "skills/waypost/scripts/artifacts/x86_64-unknown-linux-gnu"
            umbrella_artifact.parent.mkdir(parents=True)
            shutil.copytree(artifacts, umbrella_artifact)
            umbrella_runner = umbrella / "skills/waypost/scripts/bridge.py"
            umbrella_check = subprocess.run([sys.executable, str(umbrella_runner), "doctor"], env=env, capture_output=True, text=True)
            self.assertEqual(umbrella_check.returncode, 0, umbrella_check.stderr)
            result = run("bridge", "--config", "/path with spaces/$(touch x).json", "status")
            self.assertEqual(result.returncode, 37)
            self.assertEqual(result.stdout.splitlines(), ["--config", "/path with spaces/$(touch x).json", "status"])
            self.assertFalse((ROOT / "x").exists())
            spec = importlib.util.spec_from_file_location("fixture_runner", runner)
            module = importlib.util.module_from_spec(spec)
            spec.loader.exec_module(module)
            _, _, fd = module.verified_binary()
            replacement = root / "replacement"
            replacement.write_bytes(b"replacement")
            old = root / "original"
            binary.rename(old)
            replacement.rename(binary)
            self.assertEqual(os.read(fd, 4), b"\x7fELF")
            os.close(fd)
            binary.unlink()
            old.rename(binary)
            binary.chmod(0o644)
            self.assertIn("not executable", run("doctor").stderr)
            binary.chmod(0o755)
            binary.rename(old)
            binary.symlink_to(old)
            self.assertIn("must not be symlinks", run("doctor").stderr)
            binary.unlink()
            old.rename(binary)
            self.assertEqual(run("bridge").returncode, 2)
            plugin_manifest = plugin / "plugin.json"
            orig = plugin_manifest.read_text()
            changed = json.loads(orig)
            changed["version"] = "0.5.0"
            plugin_manifest.write_text(json.dumps(changed))
            self.assertIn("Plugin version", run("doctor").stderr)
            plugin_manifest.write_text(orig)
            manifest["plugin_version"] = "9.9.9"
            meta.write_text(json.dumps(manifest))
            self.assertIn("does not match", run("doctor").stderr)
            manifest["plugin_version"] = builder.VERSION
            meta.write_text(json.dumps(manifest))
            manifest["bridge_version"] = "0.2.0"
            meta.write_text(json.dumps(manifest))
            self.assertIn("does not match", run("doctor").stderr)
            manifest["bridge_version"] = "0.1.0"
            meta.write_text(json.dumps(manifest))
            binary.write_bytes(binary.read_bytes() + b"corruption")
            self.assertIn("checksum mismatch", run("doctor").stderr)

    def test_source_revision_rejected(self):
        with tempfile.TemporaryDirectory() as d:
            subprocess.run(["git", "init", "-q", d], check=True)
            with self.assertRaises((ValueError, subprocess.CalledProcessError)):
                builder.validate_source(Path(d))

    def test_monorepo_bridge_is_pinned_and_exported_from_git(self):
        # A fixture commit keeps the test independent of this workspace's index/HEAD.
        with tempfile.TemporaryDirectory() as d:
            source = Path(d) / "source"
            bridge = source / "services/waypost/bridge"
            bridge.mkdir(parents=True)
            (bridge / "Cargo.toml").write_text("committed bridge")
            subprocess.run(["git", "init", "-q", str(source)], check=True)
            subprocess.run(["git", "-C", str(source), "add", "."], check=True)
            subprocess.run(["git", "-C", str(source), "-c", "user.name=Fixture", "-c", "user.email=fixture@example.invalid", "commit", "-qm", "fixture"], check=True)
            pin = builder.git(source, "rev-parse", "HEAD:services/waypost/bridge")
            with patch.object(builder, "BRIDGE_TREE", pin):
                builder.validate_monorepo(source)
                output = Path(d) / "exported"
                builder.export_monorepo(source, output)
                self.assertEqual((output / "bridge/Cargo.toml").read_bytes(), (bridge / "Cargo.toml").read_bytes())
                (bridge / "Cargo.toml").write_text("uncommitted bridge")
                with self.assertRaisesRegex(ValueError, "must be clean"):
                    builder.validate_monorepo(source)
        with patch.object(builder, "git", return_value="different-tree"):
            with self.assertRaisesRegex(ValueError, "pinned bridge"):
                builder.validate_monorepo(ROOT)

    def test_current_and_legacy_provenance_are_separate_and_pinned(self):
        current = {"source_kind":"monorepo", "source_commit":"a"*40, "monorepo_commit":"a"*40,
                   "bridge_tree":builder.BRIDGE_TREE,"release_status":"unreleased"}
        legacy = {"source_commit":builder.COMMIT}
        for plugin in ("waypost", "miles-works"):
            spec = importlib.util.spec_from_file_location("provenance_runner_"+plugin,
                ROOT / "plugins" / plugin / "skills/waypost/scripts/bridge.py")
            runner = importlib.util.module_from_spec(spec)
            spec.loader.exec_module(runner)
            self.assertEqual(runner.BRIDGE_TREE, builder.BRIDGE_TREE)
            self.assertEqual(runner.SOURCE_COMMIT, builder.COMMIT)
            for metadata in (current, legacy, {**legacy,"source_kind":"upstream"}):
                self.assertTrue(builder.valid_provenance(metadata))
                self.assertTrue(runner.valid_provenance(metadata))
            for metadata in ({**current,"bridge_tree":"b"*40}, {**current,"source_kind":None},
                             {**current,"source_commit":builder.COMMIT,"monorepo_commit":builder.COMMIT},
                             {**legacy,"bridge_tree":builder.BRIDGE_TREE}, {**current,"release_status":"released"},
                             {**current,"monorepo_commit":"invalid"}, []):
                self.assertFalse(builder.valid_provenance(metadata))
                self.assertFalse(runner.valid_provenance(metadata))

    def test_export_source_rejects_traversal(self):
        with tempfile.TemporaryDirectory() as d:
            def fake_archive(_args, stdout, check):
                with tarfile.open(fileobj=stdout, mode="w") as tar:
                    info = tarfile.TarInfo("../escape")
                    data = b"unsafe"
                    info.size = len(data)
                    tar.addfile(info, io.BytesIO(data))
            with patch.object(builder.subprocess, "run", side_effect=fake_archive):
                with self.assertRaisesRegex(ValueError, "Unsafe entry"):
                    builder.export_source(Path(d), Path(d) / "exported")
            self.assertFalse((Path(d).parent / "escape").exists())

    def test_output_rejects_symlink_into_source(self):
        with tempfile.TemporaryDirectory() as d:
            root = Path(d)
            source = root / "source"
            source.mkdir()
            alias = root / "alias"
            alias.symlink_to(source, target_is_directory=True)
            with patch.object(builder.platform, "system", return_value="Linux"), \
                 patch.object(builder.platform, "machine", return_value="x86_64"):
                with self.assertRaisesRegex(ValueError, "outside the Waypost checkout"):
                    builder.build(source, alias / "bundle")
            self.assertFalse((source / "bundle").exists())

    def test_export_uses_committed_files_only(self):
        with tempfile.TemporaryDirectory() as d:
            root = Path(d)
            source = root / "source"
            source.mkdir()
            subprocess.run(["git", "init", "-q", str(source)], check=True)
            (source / "tracked").write_text("committed")
            (source / ".gitignore").write_text("ignored\n")
            subprocess.run(["git", "-C", str(source), "add", "tracked", ".gitignore"], check=True)
            subprocess.run(["git", "-C", str(source), "-c", "user.name=Fixture", "-c", "user.email=fixture@example.invalid", "commit", "-qm", "fixture"], check=True)
            sha = subprocess.check_output(["git", "-C", str(source), "rev-parse", "HEAD"], text=True).strip()
            (source / "tracked").write_text("modified")
            (source / "ignored").write_text("ignored")
            with patch.object(builder, "COMMIT", sha):
                builder.export_source(source, root / "snapshot")
            self.assertEqual((root / "snapshot/tracked").read_text(), "committed")
            self.assertFalse((root / "snapshot/ignored").exists())

    @unittest.skipUnless(sys.platform.startswith("linux") and os.uname().machine == "x86_64" and shutil.which("cc"), "requires native Linux x86_64 C compiler")
    def test_build_pipeline_with_fake_offline_compiler(self):
        with tempfile.TemporaryDirectory() as d:
            root = Path(d)
            source = root / "source"
            (source / "bridge").mkdir(parents=True)
            (source / "bridge/Cargo.toml").write_text('[package]\nname="waypost-bridge"\nversion="0.1.0"\n')
            (source / "bridge/Cargo.lock").write_text("fixture")
            with patch.object(builder, "git", side_effect=lambda _source, *args: builder.COMMIT if args[0] == "rev-parse" else "?? file"):
                with self.assertRaisesRegex(ValueError, "checkout must be clean"):
                    builder.validate_source(source)
            (source / "bridge/Cargo.toml").write_text('[package]\nname="waypost-bridge"\nversion="9.9.9"\n')
            with patch.object(builder, "git", side_effect=lambda _source, *args: builder.COMMIT if args[0] == "rev-parse" else ""):
                with self.assertRaisesRegex(ValueError, "package version"):
                    builder.validate_source(source)
            (source / "bridge/Cargo.toml").write_text('[package]\nname="waypost-bridge"\nversion="0.1.0"\n')
            cfile = root / "fixture.c"
            cfile.write_text("int main(void){return 0;}\n")
            elf = root / "fixture"
            subprocess.run(["cc", str(cfile), "-o", str(elf)], check=True)
            def fake_git(_source, *args):
                return builder.COMMIT if args[0] == "rev-parse" else ""
            def fake_cargo(args, env, check):
                self.assertIn("--offline", args)
                self.assertIn("--locked", args)
                target = args[args.index("--target") + 1]
                dest = Path(env["CARGO_TARGET_DIR"]) / target / "release/waypost-bridge"
                dest.parent.mkdir(parents=True)
                shutil.copyfile(elf, dest)
            out = root / "waypost"
            with patch.object(builder, "git", side_effect=fake_git), \
                 patch.object(builder, "export_source", side_effect=lambda src, dest: shutil.copytree(src, dest)), \
                 patch.object(builder.subprocess, "run", side_effect=fake_cargo), \
                 patch.object(builder.subprocess, "check_output", return_value="fixture toolchain\n"):
                builder.build(source, out)
                with self.assertRaisesRegex(ValueError, "already exist"):
                    builder.build(source, out)
            result = subprocess.run([sys.executable, str(out / "skills/waypost/scripts/bridge.py"), "doctor"],
                                    capture_output=True, text=True)
            self.assertEqual(result.returncode, 0, result.stderr)
            monorepo_out = root / "monorepo-waypost"
            def fake_monorepo_git(_source, *args):
                if args[0] == "rev-parse":
                    return builder.BRIDGE_TREE if args[1] == "HEAD:services/waypost/bridge" else "a" * 40
                return ""
            with patch.object(builder, "git", side_effect=fake_monorepo_git), \
                 patch.object(builder, "export_monorepo", side_effect=lambda src, dest: shutil.copytree(src, dest)), \
                 patch.object(builder.subprocess, "run", side_effect=fake_cargo), \
                 patch.object(builder.subprocess, "check_output", return_value="fixture toolchain\n"):
                builder.build(source, monorepo_out, monorepo=True)
            metadata = json.loads((monorepo_out / "skills/waypost/scripts/artifacts/x86_64-unknown-linux-gnu/manifest.json").read_text())
            self.assertEqual(metadata["source_commit"], "a" * 40)
            self.assertEqual(metadata["monorepo_commit"], metadata["source_commit"])
            self.assertEqual(metadata["bridge_tree"], builder.BRIDGE_TREE)
            self.assertEqual(metadata["release_status"], "unreleased")
            self.assertNotEqual(metadata["source_commit"], builder.COMMIT)
            result = subprocess.run([sys.executable, str(monorepo_out / "skills/waypost/scripts/bridge.py"), "doctor"],
                                    capture_output=True, text=True)
            self.assertEqual(result.returncode, 0, result.stderr)


if __name__ == "__main__":
    unittest.main()
