"""This tests a fail-closed state model, not a working OAuth client."""
import importlib.util
from pathlib import Path
import unittest

spec = importlib.util.spec_from_file_location("bridge_refresh_model", Path(__file__).resolve().parents[1] / "tools/waypost/bridge_refresh_model.py")
model = importlib.util.module_from_spec(spec)
spec.loader.exec_module(model)


class RefreshModelTest(unittest.TestCase):
    def test_success_and_concurrent_attempt(self):
        pending = model.begin(model.Phase.READY)
        with self.assertRaises(ValueError):
            model.begin(pending)
        self.assertEqual(model.complete(pending), model.Phase.READY)

    def test_uncertain_exchange_and_restart_require_new_authorization(self):
        pending = model.begin(model.Phase.READY)
        self.assertEqual(model.uncertain(pending), model.Phase.REAUTHORIZE)
        self.assertEqual(model.restart(pending), model.Phase.REAUTHORIZE)
        with self.assertRaises(ValueError):
            model.begin(model.restart(pending))

    def test_ready_and_reauthorization_survive_restart(self):
        self.assertEqual(model.restart(model.Phase.READY), model.Phase.READY)
        self.assertEqual(model.restart(model.Phase.REAUTHORIZE), model.Phase.REAUTHORIZE)
        with self.assertRaises(ValueError):
            model.complete(model.Phase.REAUTHORIZE)


if __name__ == "__main__":
    unittest.main()
