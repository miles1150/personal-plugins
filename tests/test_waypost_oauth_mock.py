"""Codex interoperability with a local OAuth mock, not the Waypost prototype."""
import base64
import hashlib
import http.server
import json
import os
try:
    import pty
except ImportError:
    pty = None
import re
import select
import shutil
import socket
import subprocess
import tempfile
import threading
import time
import tomllib
import unittest
import urllib.parse
import urllib.error
import urllib.request
from pathlib import Path


class TemplateTest(unittest.TestCase):
    def test_examples_are_parseable_and_only_placeholders(self):
        root = Path(__file__).resolve().parents[1] / "examples/waypost-oauth"
        with (root / "codex.toml.example").open("rb") as file:
            codex = tomllib.load(file)["mcp_servers"]["waypost_test"]
        claude = json.loads((root / "claude.json.example").read_text())["mcpServers"]["waypost_test"]
        self.assertEqual(codex["url"], claude["url"])
        self.assertEqual(codex["oauth"]["client_id"], "REPLACE_REGISTERED_PUBLIC_CLIENT_ID")
        self.assertEqual(claude["oauth"]["clientId"], "REPLACE_REGISTERED_PUBLIC_CLIENT_ID")
        self.assertNotIn("client_secret", codex["oauth"])
        self.assertNotIn("clientSecret", claude["oauth"])


@unittest.skipUnless(pty and os.name == "posix" and shutil.which("codex"), "requires POSIX and Codex CLI")
class CodexOAuthMockTest(unittest.TestCase):
    def test_preregistered_public_client_and_issuer_bound_redirect(self):
        self.run_flow(issuer_supported=True)

    def test_no_issuer_support_uses_server_specific_callback(self):
        self.run_flow(issuer_supported=False)

    def test_advertised_issuer_missing_response_is_rejected(self):
        self.run_flow(issuer_supported=True, include_issuer=False)

    def run_flow(self, issuer_supported, include_issuer=True):
        observed = {"requests": []}

        class Handler(http.server.BaseHTTPRequestHandler):
            def log_message(self, *_args):
                pass

            def do_GET(self):
                origin = f"http://127.0.0.1:{self.server.server_port}"
                observed["requests"].append(self.path)
                if self.path.startswith("/.well-known/oauth-protected-resource"):
                    body = {"resource": origin + "/mcp", "authorization_servers": [origin]}
                elif self.path.startswith("/.well-known/oauth-authorization-server"):
                    body = {"issuer": origin, "authorization_endpoint": origin + "/authorize",
                            "token_endpoint": origin + "/token", "response_types_supported": ["code"],
                            "grant_types_supported": ["authorization_code", "refresh_token"],
                            "code_challenge_methods_supported": ["S256"],
                            "token_endpoint_auth_methods_supported": ["none"],
                            "authorization_response_iss_parameter_supported": issuer_supported}
                else:
                    self.send_error(404)
                    return
                payload = json.dumps(body).encode()
                self.send_response(200)
                self.send_header("Content-Type", "application/json")
                self.end_headers()
                self.wfile.write(payload)

            def do_POST(self):
                origin = f"http://127.0.0.1:{self.server.server_port}"
                observed["requests"].append(self.path)
                if self.path != "/token":
                    self.send_response(401)
                    self.send_header("WWW-Authenticate", f'Bearer resource_metadata="{origin}/.well-known/oauth-protected-resource/mcp"')
                    self.end_headers()
                    return
                data = urllib.parse.parse_qs(self.rfile.read(int(self.headers.get("Content-Length", "0"))).decode())
                verifier = data.get("code_verifier", [""])[0]
                challenge = base64.urlsafe_b64encode(hashlib.sha256(verifier.encode()).digest()).decode().rstrip("=")
                observed["pkce_valid"] = challenge == observed.get("challenge")
                observed["token_client_id"] = data.get("client_id")
                observed["token_redirect"] = data.get("redirect_uri")
                observed["grant_type"] = data.get("grant_type")
                payload = json.dumps({"access_token": "fixture-access", "token_type": "Bearer",
                                      "expires_in": 3600, "refresh_token": "fixture-refresh"}).encode()
                self.send_response(200)
                self.send_header("Content-Type", "application/json")
                self.end_headers()
                self.wfile.write(payload)

        server = http.server.ThreadingHTTPServer(("127.0.0.1", 0), Handler)
        threading.Thread(target=server.serve_forever, daemon=True).start()
        origin = f"http://127.0.0.1:{server.server_port}"
        try:
            with tempfile.TemporaryDirectory(prefix="waypost-codex-oauth-") as d:
                home = Path(d) / "codex"
                home.mkdir()
                with socket.socket() as probe:
                    probe.bind(("127.0.0.1", 0))
                    port = probe.getsockname()[1]
                callback = f"http://127.0.0.1:{port}/callback"
                (home / "config.toml").write_text(
                    f'[mcp_servers.mock]\nurl = "{origin}/mcp"\n'
                    f'[mcp_servers.mock.oauth]\nclient_id = "fixture-client"\n'
                    f'callback_url = "{callback}"\ncallback_port = {port}\n')
                env = {**os.environ, "CODEX_HOME": str(home), "XDG_CONFIG_HOME": d + "/config",
                       "XDG_CACHE_HOME": d + "/cache", "BROWSER": "/bin/true"}
                master, slave = pty.openpty()
                proc = subprocess.Popen(["codex", "mcp", "login", "mock"], stdin=slave, stdout=slave,
                                        stderr=slave, env=env)
                os.close(slave)
                output = ""
                sent = False
                try:
                    deadline = time.monotonic() + 20
                    while time.monotonic() < deadline and proc.poll() is None:
                        if not select.select([master], [], [], 0.25)[0]:
                            continue
                        try:
                            output += os.read(master, 65536).decode(errors="replace")
                        except OSError:
                            break
                        match = re.search(re.escape(origin) + r"/authorize\?[^\s]+", output)
                        if match and not sent:
                            query = urllib.parse.parse_qs(urllib.parse.urlsplit(match.group()).query)
                            observed["authorization"] = query
                            observed["challenge"] = query["code_challenge"][0]
                            redirect = query["redirect_uri"][0]
                            params = {"code": "fixture-code", "state": query["state"][0]}
                            if issuer_supported and include_issuer:
                                params["iss"] = origin
                            url = redirect + "?" + urllib.parse.urlencode(params)
                            try:
                                with urllib.request.urlopen(url, timeout=5) as response:
                                    response.read()
                            except urllib.error.HTTPError:
                                if include_issuer:
                                    raise
                            sent = True
                    if proc.poll() is None:
                        self.fail("Codex mock OAuth login timed out")
                    if not include_issuer:
                        self.assertNotEqual(proc.returncode, 0)
                        self.assertNotIn("/token", observed["requests"])
                        return
                    self.assertEqual(proc.returncode, 0, output[-500:])
                    query = observed["authorization"]
                    self.assertEqual(query["client_id"], ["fixture-client"])
                    if issuer_supported:
                        self.assertEqual(query["redirect_uri"], [callback])
                    else:
                        self.assertTrue(query["redirect_uri"][0].startswith(callback + "/"))
                    self.assertEqual(query["code_challenge_method"], ["S256"])
                    self.assertEqual(query["resource"], [origin + "/mcp"])
                    self.assertTrue(observed["pkce_valid"])
                    self.assertEqual(observed["grant_type"], ["authorization_code"])
                    self.assertEqual(observed["token_client_id"], ["fixture-client"])
                    self.assertEqual(observed["token_redirect"], query["redirect_uri"])
                    self.assertNotIn("/register", observed["requests"])
                finally:
                    if proc.poll() is None:
                        proc.terminate()
                        proc.wait(timeout=5)
                    os.close(master)
        finally:
            server.shutdown()
            server.server_close()


if __name__ == "__main__":
    unittest.main()
