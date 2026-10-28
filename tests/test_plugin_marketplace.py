import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[1]
BRIDGE = ROOT / "plugins/waypost/skills/waypost/scripts/bridge.py"


class MarketplaceTests(unittest.TestCase):
    def test_markets_point_to_same_isolated_plugins(self):
        codex = json.loads((ROOT / ".agents/plugins/marketplace.json").read_text())
        claude = json.loads((ROOT / ".claude-plugin/marketplace.json").read_text())
        self.assertEqual(codex["name"], claude["name"])
        a = {p["name"]: p["source"]["path"] for p in codex["plugins"]}
        b = {p["name"]: p["source"] for p in claude["plugins"]}
        self.assertEqual(a, b)
        self.assertEqual(set(a), {"waypost", "keyenv"})
        for name, rel in a.items():
            root = ROOT / rel
            portable = json.loads((root / "plugin.json").read_text())
            native = json.loads((root / ".claude-plugin/plugin.json").read_text())
            self.assertEqual((portable["name"], portable["version"]), (name, native["version"]))
            self.assertEqual(native["name"], name)
            self.assertTrue((root / "skills" / name / "SKILL.md").is_file())

    def test_bridge_missing_does_not_start_anything(self):
        with tempfile.TemporaryDirectory() as d:
            env = {**os.environ, "PATH": d}
            doctor = subprocess.run([sys.executable, str(BRIDGE), "doctor"], env=env, capture_output=True, text=True)
            self.assertEqual(doctor.returncode, 1)
            self.assertIn("not found on PATH", doctor.stdout)
            call = subprocess.run([sys.executable, str(BRIDGE), "bridge", "status"], env=env, capture_output=True, text=True)
            self.assertEqual(call.returncode, 127)

    @unittest.skipIf(os.name == "nt", "POSIX fixture; Windows executable handling needs native validation")
    def test_bridge_preserves_argument_boundaries(self):
        with tempfile.TemporaryDirectory() as d:
            fake = Path(d) / "waypost-bridge"
            fake.write_text("#!" + sys.executable + "\nimport json, sys\nprint(json.dumps(sys.argv[1:]))\n")
            fake.chmod(0o755)
            env = {**os.environ, "PATH": d}
            args = ["--config", "/path with spaces/config.json", "status", "--items"]
            run = subprocess.run([sys.executable, str(BRIDGE), "bridge", *args], env=env, capture_output=True, text=True)
            self.assertEqual(run.returncode, 0, run.stderr)
            self.assertEqual(json.loads(run.stdout), args)
            no_args = subprocess.run([sys.executable, str(BRIDGE), "bridge"], env=env, capture_output=True, text=True)
            self.assertEqual(no_args.returncode, 2)


if __name__ == "__main__":
    unittest.main()
