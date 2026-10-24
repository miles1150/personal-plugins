import io
import json
import threading
import types
import unittest

from server import Bridge, guidance, runtime_env
from verification.calculator_approval import decision


def bridge_fixture():
    bridge = Bridge.__new__(Bridge)
    bridge.session_id = "independent-session"
    bridge.turn_id = "independent-turn"
    bridge.lock = threading.RLock()
    bridge.pending_finishes = set()
    bridge.cleanup_id = "internal-cleanup"
    bridge.cleanup_done = threading.Event()
    bridge.reader_done = threading.Event()
    bridge.guidance = guidance(("browser", "computer"))
    bridge.record = lambda *_: None
    bridge.sent = []
    bridge.received = []
    bridge.to_child = bridge.sent.append
    bridge.to_client = bridge.received.append
    return bridge


class BridgeTests(unittest.TestCase):
    def test_combined_surfaces_register_both_services_without_security_overrides(self):
        env = runtime_env({"env": {"EXISTING_POLICY": "keep"}}, ("browser", "computer"))
        self.assertEqual(env["CUA_REPL_ENABLED_SURFACES"], "browser,computer")
        self.assertEqual(env["BROWSER_USE_AVAILABLE_BACKENDS"], "chrome")
        self.assertEqual(json.loads(env["NODE_REPL_TRUSTED_SERVICES"]), {
            "browser": "@oai/browser-desktop/service", "sky": "@oai/sky/service"
        })
        self.assertEqual(env["EXISTING_POLICY"], "keep")

    def test_browser_only_mode_does_not_register_desktop_service(self):
        env = runtime_env({"env": {}}, ("browser",))
        self.assertEqual(json.loads(env["NODE_REPL_TRUSTED_SERVICES"]), {
            "browser": "@oai/browser-desktop/service"
        })

    def test_injects_own_identity_without_changing_other_metadata(self):
        bridge = bridge_fixture()
        request = {"jsonrpc": "2.0", "id": 3, "method": "tools/call", "params": {
            "name": "js", "arguments": {"code": "await cua.listBrowsers();"},
            "_meta": {"progressToken": 7, "x-codex-turn-metadata": {"session_id": "another-chat"}},
        }}
        bridge.from_client(request)
        meta = bridge.sent[0]["params"]["_meta"]
        self.assertEqual(meta["progressToken"], 7)
        self.assertEqual(meta["x-codex-turn-metadata"], {
            "session_id": "independent-session", "turn_id": "independent-turn"
        })
        self.assertEqual(request["params"]["_meta"]["x-codex-turn-metadata"]["session_id"], "another-chat")

    def test_client_approval_response_is_forwarded_without_auto_accept(self):
        bridge = bridge_fixture()
        response = {"jsonrpc": "2.0", "id": 0, "result": {"action": "decline"}}
        bridge.from_client(response)
        self.assertEqual(bridge.sent, [response])

    def test_unknown_tool_is_not_forwarded(self):
        bridge = bridge_fixture()
        bridge.from_client({"jsonrpc": "2.0", "id": 9, "method": "tools/call",
                            "params": {"name": "turn_ended", "arguments": {}}})
        self.assertFalse(bridge.sent)
        self.assertEqual(bridge.received[0]["error"]["code"], -32602)

    def test_finish_maps_to_vendor_lifecycle_and_rotates_only_after_success(self):
        bridge = bridge_fixture()
        bridge.from_client({"jsonrpc": "2.0", "id": 4, "method": "tools/call",
                            "params": {"name": "finish_turn", "arguments": {}}})
        params = bridge.sent[0]["params"]
        self.assertEqual(params["name"], "turn_ended")
        self.assertEqual(params["arguments"]["turn_id"], "independent-turn")
        self.assertEqual(bridge.turn_id, "independent-turn")
        bridge.child = types.SimpleNamespace(stdout=io.BytesIO(
            (json.dumps({"jsonrpc": "2.0", "id": 4, "result": {"isError": False, "content": []}}) + "\n").encode()
        ))
        bridge.read_child()
        self.assertNotEqual(bridge.turn_id, "independent-turn")

    def test_reverse_request_id_collision_does_not_finish_pending_call(self):
        bridge = bridge_fixture()
        bridge.pending_finishes.add(4)
        reverse_request = {"jsonrpc": "2.0", "id": 4, "method": "elicitation/create",
                           "params": {"message": "Confirm?"}}
        bridge.child = types.SimpleNamespace(stdout=io.BytesIO(
            (json.dumps(reverse_request) + "\n").encode()
        ))
        bridge.read_child()
        self.assertEqual(bridge.received, [reverse_request])
        self.assertIn(4, bridge.pending_finishes)
        self.assertEqual(bridge.turn_id, "independent-turn")

    def test_failed_cleanup_keeps_turn_id_for_retry(self):
        bridge = bridge_fixture()
        bridge.pending_finishes.add(4)
        bridge.child = types.SimpleNamespace(stdout=io.BytesIO(
            (json.dumps({"jsonrpc": "2.0", "id": 4, "result": {"isError": True, "content": []}}) + "\n").encode()
        ))
        bridge.read_child()
        self.assertEqual(bridge.turn_id, "independent-turn")

    def test_js_is_not_advertised_as_read_only(self):
        bridge = bridge_fixture()
        bridge.child = types.SimpleNamespace(stdout=io.BytesIO(
            (json.dumps({"jsonrpc": "2.0", "id": 1, "result": {"tools": [{
                "name": "js", "description": "Execute", "annotations": {"readOnlyHint": True}
            }]}}) + "\n").encode()
        ))
        bridge.read_child()
        self.assertFalse(bridge.received[0]["result"]["tools"][0]["annotations"]["readOnlyHint"])


class CalculatorFixtureTests(unittest.TestCase):
    def request(self):
        return {"hook_event_name": "Elicitation", "mcp_server_name": "codex-cu",
                "mode": "form", "message": 'Allow Computer Use to use "Calculator"?',
                "requested_schema": {"type": "object", "properties": {}}}

    def test_accepts_only_explicit_calculator_form(self):
        self.assertEqual(decision(self.request()), "accept")
        self.assertEqual(decision(self.request(), deny=True), "decline")

    def test_rejects_other_apps_servers_and_forms(self):
        for changes in [
            {"message": 'Allow Computer Use to use "Mail"?'},
            {"mcp_server_name": "another-server"},
            {"mode": "url"},
            {"requested_schema": {"type": "object", "properties": {"password": {"type": "string"}}}},
        ]:
            with self.subTest(changes=changes):
                self.assertEqual(decision({**self.request(), **changes}), "decline")


if __name__ == "__main__":
    unittest.main()
