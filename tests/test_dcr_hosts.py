"""Actual installed host CLIs against a loopback-only synthetic DCR service."""
import base64
import hashlib
import http.server
import json
import os
from pathlib import Path
import pty
import re
import select
import shutil
import subprocess
import tempfile
import threading
import time
import unittest
import urllib.parse
import urllib.request

ROOT = Path(__file__).resolve().parents[1]
CLAUDE = os.environ.get('WAYPOST_TEST_CLAUDE', '/tmp/waypost-host-tooling/node_modules/@anthropic-ai/claude-code-linux-x64/claude')

class ConfigTest(unittest.TestCase):
    def test_host_manifests_use_one_endpoint_without_secrets_or_client_ids(self):
        root = ROOT / 'plugins/waypost'
        portable = json.loads((root / 'mcp.json').read_text())['mcpServers']['waypost']
        claude = json.loads((root / '.mcp.json').read_text())['mcpServers']['waypost']
        self.assertEqual(portable, {'type': 'streamable-http', 'url': 'https://waypost.miles1150.top/mcp'})
        self.assertEqual(claude, {'type': 'http', 'url': portable['url']})

    @unittest.skipUnless(shutil.which('codex') and os.path.isfile(CLAUDE), 'requires both host CLIs')
    def test_isolated_plugin_install_and_component_discovery(self):
        with tempfile.TemporaryDirectory(prefix='waypost-plugin-host-') as d:
            root = Path(d)
            market = root / 'market'
            plugin = market / 'plugins/waypost'
            plugin.parent.mkdir(parents=True)
            shutil.copytree(ROOT / 'plugins/waypost', plugin)
            for name in ('mcp.json', '.mcp.json'):
                path = plugin / name
                value = json.loads(path.read_text())
                value['mcpServers']['waypost']['url'] = 'http://127.0.0.1:43199/mcp'
                path.write_text(json.dumps(value))
            (market / '.agents/plugins').mkdir(parents=True)
            (market / '.claude-plugin').mkdir()
            index = market / '.agents/plugins/marketplace.json'
            index.write_text(json.dumps({'name': 'fixture', 'plugins': [{'name': 'waypost', 'source': {'source': 'local', 'path': './plugins/waypost'}}]}))
            (market / '.claude-plugin/marketplace.json').write_text(json.dumps({'name': 'fixture', 'owner': {'name': 'fixture'}, 'plugins': [{'name': 'waypost', 'source': './plugins/waypost'}]}))
            (root / 'codex').mkdir()
            (root / 'claude').mkdir()
            env = {**os.environ, 'HOME': d, 'CODEX_HOME': d + '/codex', 'CLAUDE_CONFIG_DIR': d + '/claude', 'XDG_CONFIG_HOME': d + '/config', 'XDG_CACHE_HOME': d + '/cache'}
            def run(*args):
                result = subprocess.run(args, env=env, text=True, capture_output=True, timeout=20)
                self.assertEqual(result.returncode, 0, result.stderr[-500:])
                return result.stdout
            run('codex', 'plugin', 'marketplace', 'add', str(market))
            run('codex', 'plugin', 'add', 'waypost@fixture')
            run(CLAUDE, 'plugin', 'validate', str(plugin))
            run(CLAUDE, 'plugin', 'marketplace', 'add', str(market))
            run(CLAUDE, 'plugin', 'install', 'waypost@fixture')
            self.assertIn('MCP servers (1)  waypost', run(CLAUDE, 'plugin', 'details', 'waypost@fixture'))
            proc = subprocess.Popen(['codex', 'app-server'], env=env, stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.DEVNULL, text=True)
            try:
                def rpc(method, ident, params):
                    proc.stdin.write(json.dumps({'method': method, 'id': ident, 'params': params}) + '\n')
                    proc.stdin.flush()
                    while select.select([proc.stdout], [], [], 15)[0]:
                        line = proc.stdout.readline()
                        if not line:
                            break
                        item = json.loads(line)
                        if item.get('id') == ident:
                            return item
                    self.fail(f'No app-server response to {method}')
                rpc('initialize', 1, {'clientInfo': {'name': 'fixture', 'version': '0'}, 'capabilities': {'experimentalApi': True}})
                result = rpc('plugin/read', 2, {'marketplacePath': str(index), 'pluginName': 'waypost'})
                self.assertEqual(result['result']['plugin']['mcpServers'], ['waypost'])
            finally:
                proc.terminate()
                proc.wait(timeout=5)
                proc.stdin.close()
                proc.stdout.close()

@unittest.skipUnless(os.name == 'posix', 'PTY required')
class OAuthTest(unittest.TestCase):
    def test_codex_dcr(self):
        if not shutil.which('codex'):
            self.skipTest('Codex CLI unavailable')
        self.flow('codex', shutil.which('codex'))

    def test_claude_dcr(self):
        if not os.path.isfile(CLAUDE) or not os.access(CLAUDE, os.X_OK):
            self.skipTest('Set WAYPOST_TEST_CLAUDE to Claude CLI executable')
        self.flow('claude', CLAUDE)

    def flow(self, host, binary):
        seen = {}
        class Handler(http.server.BaseHTTPRequestHandler):
            def log_message(self, *_): pass
            def reply(self, status, obj, headers=None):
                payload = json.dumps(obj).encode()
                self.send_response(status)
                self.send_header('Content-Type', 'application/json')
                for name, value in (headers or {}).items(): self.send_header(name, value)
                self.end_headers()
                self.wfile.write(payload)
            def do_GET(self):
                origin = f'http://127.0.0.1:{self.server.server_port}'
                if self.path.startswith('/.well-known/oauth-protected-resource'):
                    self.reply(200, {'resource': origin + '/mcp', 'authorization_servers': [origin], 'scopes_supported': ['waypost']})
                elif self.path.startswith('/.well-known/oauth-authorization-server'):
                    self.reply(200, {'issuer': origin, 'authorization_endpoint': origin + '/authorize', 'token_endpoint': origin + '/token', 'registration_endpoint': origin + '/register', 'response_types_supported': ['code'], 'grant_types_supported': ['authorization_code', 'refresh_token'], 'code_challenge_methods_supported': ['S256'], 'token_endpoint_auth_methods_supported': ['none'], 'authorization_response_iss_parameter_supported': True})
                else: self.reply(404, {})
            def do_POST(self):
                origin = f'http://127.0.0.1:{self.server.server_port}'
                raw = self.rfile.read(int(self.headers.get('Content-Length', '0')))
                if self.path == '/register':
                    metadata = json.loads(raw)
                    seen['registration'] = metadata
                    self.reply(201, {**metadata, 'client_id': 'fixture-dynamic-client', 'client_id_issued_at': 1})
                elif self.path == '/token':
                    params = urllib.parse.parse_qs(raw.decode())
                    if params.get('grant_type') == ['authorization_code']:
                        verifier = params.get('code_verifier', [''])[0]
                        seen['token_keys'] = sorted(params)
                        seen['verifier_length'] = len(verifier)
                        seen['pkce'] = base64.urlsafe_b64encode(hashlib.sha256(verifier.encode()).digest()).decode().rstrip('=') in seen.get('challenges', [])
                        seen['token_client'] = params.get('client_id')
                    self.reply(200, {'access_token': 'synthetic-access', 'refresh_token': 'synthetic-refresh', 'token_type': 'Bearer', 'expires_in': 300, 'scope': 'waypost'})
                else:
                    self.reply(401, {}, {'WWW-Authenticate': f'Bearer resource_metadata="{origin}/.well-known/oauth-protected-resource/mcp"'})
        server = http.server.ThreadingHTTPServer(('127.0.0.1', 0), Handler)
        threading.Thread(target=server.serve_forever, daemon=True).start()
        origin = f'http://127.0.0.1:{server.server_port}'
        try:
            with tempfile.TemporaryDirectory(prefix='waypost-dcr-host-') as d:
                home = Path(d)
                env = {**os.environ, 'HOME': d, 'CODEX_HOME': d + '/codex', 'CLAUDE_CONFIG_DIR': d + '/claude', 'XDG_CONFIG_HOME': d + '/config', 'XDG_CACHE_HOME': d + '/cache', 'BROWSER': '/bin/true', 'NO_PROXY': '127.0.0.1,localhost,::1', 'no_proxy': '127.0.0.1,localhost,::1'}
                if host == 'codex':
                    (home / 'codex').mkdir()
                    (home / 'codex/config.toml').write_text(f'mcp_oauth_credentials_store = "file"\n[mcp_servers.waypost]\nurl = "{origin}/mcp"\n')
                    cmd = [binary, 'mcp', 'login', 'waypost']
                else:
                    add = subprocess.run([binary, 'mcp', 'add', '--scope', 'user', '--transport', 'http', 'waypost', origin + '/mcp'], env=env, capture_output=True, text=True, timeout=15)
                    self.assertEqual(add.returncode, 0, add.stderr)
                    cmd = [binary, 'mcp', 'login', 'waypost', '--no-browser']
                master, slave = pty.openpty()
                proc = subprocess.Popen(cmd, stdin=slave, stdout=slave, stderr=slave, env=env)
                os.close(slave)
                output, sent = '', False
                try:
                    deadline = time.monotonic() + 25
                    while time.monotonic() < deadline and proc.poll() is None:
                        if not select.select([master], [], [], .2)[0]: continue
                        try: output += os.read(master, 65536).decode(errors='replace')
                        except OSError: break
                        match = re.search(re.escape(origin) + r'/authorize\?[^\s\x1b]+', output)
                        if match and not sent:
                            query = urllib.parse.parse_qs(urllib.parse.urlsplit(match.group()).query)
                            seen['authorization'] = query
                            seen['challenges'] = query['code_challenge']
                            callback = query['redirect_uri'][0] + '?' + urllib.parse.urlencode({'code': 'synthetic-code', 'state': query['state'][0], 'iss': origin})
                            if host == 'codex':
                                with urllib.request.urlopen(callback, timeout=5) as response:
                                    response.read()
                            else:
                                os.write(master, (callback + '\n').encode())
                            sent = True
                    if proc.poll() is None: self.fail(f'{host} timed out; output={output[-1500:]}')
                    self.assertEqual(proc.returncode, 0, output[-1000:])
                    self.assertEqual(set(seen['authorization']['client_id']), {'fixture-dynamic-client'})
                    self.assertEqual(set(seen['authorization']['code_challenge_method']), {'S256'})
                    self.assertIn(seen['authorization']['redirect_uri'][0], seen['registration']['redirect_uris'])
                    self.assertEqual(seen['token_client'], ['fixture-dynamic-client'])
                    self.assertTrue(seen['pkce'], f"PKCE mismatch: keys={seen.get('token_keys')} verifier_length={seen.get('verifier_length')} challenges={len(seen.get('challenges', []))}")
                finally:
                    if proc.poll() is None: proc.terminate(); proc.wait(timeout=5)
                    os.close(master)
        finally:
            server.shutdown(); server.server_close()

if __name__ == '__main__': unittest.main()
