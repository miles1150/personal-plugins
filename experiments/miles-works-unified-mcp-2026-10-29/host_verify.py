#!/usr/bin/env python3
"""Exercise real installed hosts against the loopback fixture, no model turn.

All host credentials generated here are synthetic and live in a fresh 0700 dir.
Never reads the user's host credentials. Does not invoke a model API.
"""
import argparse
import json
import os
import pty
from pathlib import Path
import queue
import re
import select
import shutil
import subprocess
import tempfile
import threading
import time
import urllib.parse
from lab import Lab
from verify import request

HOST_TOOLS = Path(__file__).resolve().parent / 'node_modules/.bin'

def lines(proc):
    q = queue.Queue()
    def read(stream):
        for line in stream:
            q.put(line)
    threading.Thread(target=read, args=(proc.stdout,), daemon=True).start()
    return q

def login(command, env, lab, terminal=False):
    if terminal:
        return login_terminal(command, env, lab)
    master, slave = pty.openpty() if terminal else (None, None)
    proc = subprocess.Popen(command, env=env, stdin=slave if terminal else subprocess.PIPE,
        stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True, bufsize=1)
    if terminal:
        os.close(slave)
    q = lines(proc)
    deadline = time.monotonic() + 25
    transcript = []
    try:
        while time.monotonic() < deadline:
            if proc.poll() is not None and q.empty():
                raise RuntimeError('login exited before authorization URL: ' + ''.join(transcript)[-600:])
            try:
                line = q.get(timeout=.2)
            except queue.Empty:
                continue
            transcript.append(line)
            match = re.search(re.escape(lab.origin) + r'/authorize\?[^\s\x00-\x20\x7f]+', line)
            if match:
                u = urllib.parse.urlsplit(match.group())
                s, h, _ = request(lab.origin, u.path + '?' + u.query)
                if s != 302:
                    raise RuntimeError('fixture consent returned ' + str(s))
                callback = h['Location']
                # --no-browser host accepts callback URL at stdin; newline needed.
                if terminal:
                    os.write(master, (callback + '\n').encode())
                else:
                    proc.stdin.write(callback + '\n')
                    proc.stdin.flush()
                proc.wait(timeout=15)
                if proc.returncode:
                    tail = []
                    while not q.empty():
                        tail.append(q.get())
                    raise RuntimeError('host token exchange failed: ' + ''.join(tail)[-600:])
                return
        raise RuntimeError('login timed out before authorization URL')
    finally:
        if proc.poll() is None:
            proc.kill()
            proc.wait()
        if master is not None:
            os.close(master)

def login_terminal(command, env, lab):
    master, slave = pty.openpty()
    proc = subprocess.Popen(command, env=env, stdin=slave, stdout=slave, stderr=slave, close_fds=True)
    os.close(slave)
    transcript, callback, sent = '', None, False
    deadline = time.monotonic() + 35
    try:
        while time.monotonic() < deadline:
            if select.select([master], [], [], .2)[0]:
                try:
                    data = os.read(master, 65536)
                except OSError:
                    break
                transcript += data.decode(errors='replace')
                clean = re.sub(r'\x1b\[[0-9;?]*[a-zA-Z]', '', transcript)
                if callback is None:
                    match = re.search(re.escape(lab.origin) + r'/authorize\?[^\s\x00-\x20\x7f]+', clean)
                    if match:
                        u = urllib.parse.urlsplit(match.group())
                        s, h, _ = request(lab.origin, u.path + '?' + u.query)
                        if s != 302:
                            raise RuntimeError('fixture consent returned ' + str(s))
                        callback = h['Location']
                # Claude's readline prompt does not end in a newline.
                if callback and not sent and ('paste' in clean.lower() or 'redirect url' in clean.lower()):
                    time.sleep(.2)
                    os.write(master, (callback + '\r').encode())
                    sent = True
            if proc.poll() is not None:
                break
        if proc.poll() is None:
            raise RuntimeError('terminal login timed out: ' + re.sub(r'https?://\S+', '<redacted-url>', transcript)[-900:])
        if proc.returncode:
            raise RuntimeError('terminal login failed: ' + re.sub(r'https?://\S+', '<redacted-url>', transcript)[-900:])
    finally:
        if proc.poll() is None:
            proc.kill()
        proc.wait()
        os.close(master)

class AppServer:
    def __init__(self, env):
        self.p = subprocess.Popen(['codex', 'app-server'], env=env, stdin=subprocess.PIPE,
            stdout=subprocess.PIPE, stderr=subprocess.DEVNULL, text=True, bufsize=1)
        self.q = lines(self.p)
        self.n = 0
        self.rpc('initialize', {'clientInfo': {'name': 'isolated-mcp-validator', 'version': '0.1'},
                               'capabilities': {'experimentalApi': True}})
        self.p.stdin.write(json.dumps({'method': 'initialized'}) + '\n')
        self.p.stdin.flush()

    def rpc(self, method, params):
        self.n += 1
        self.p.stdin.write(json.dumps({'id': self.n, 'method': method, 'params': params}) + '\n')
        self.p.stdin.flush()
        deadline = time.monotonic() + 30
        while time.monotonic() < deadline:
            try:
                line = self.q.get(timeout=.2)
            except queue.Empty:
                continue
            item = json.loads(line)
            if item.get('id') == self.n:
                if 'error' in item:
                    raise RuntimeError(method + ': ' + json.dumps(item['error'])[:500])
                return item['result']
        raise RuntimeError(method + ' timed out')

    def close(self):
        self.p.terminate()
        try:
            self.p.wait(timeout=5)
        except subprocess.TimeoutExpired:
            self.p.kill()

def run(host):
    lab = Lab().start()
    temp = Path(tempfile.mkdtemp(prefix='works-host-'))
    temp.chmod(0o700)
    env = {k: v for k, v in os.environ.items() if k in
           ('PATH', 'HTTP_PROXY', 'HTTPS_PROXY', 'http_proxy', 'https_proxy', 'SSL_CERT_FILE', 'SSL_CERT_DIR', 'NODE_EXTRA_CA_CERTS')}
    env.update({'NO_PROXY': '127.0.0.1,localhost', 'no_proxy': '127.0.0.1,localhost',
                'XDG_CONFIG_HOME': str(temp / 'xdg-config'), 'XDG_CACHE_HOME': str(temp / 'xdg-cache'),
                'XDG_DATA_HOME': str(temp / 'xdg-data')})
    checks = []
    app = None
    result = {'host': host, 'model_turns': 0, 'real_idp': False, 'production': False}
    try:
        if host == 'codex':
            env['CODEX_HOME'] = str(temp)
            (temp / 'config.toml').write_text('mcp_oauth_credentials_store = "file"\n'
                '[mcp_servers.works_lab]\nurl = ' + json.dumps(lab.resource) + '\n')
            result['version'] = subprocess.check_output(['codex', '--version'], env=env,
                                                       stderr=subprocess.DEVNULL, text=True).strip()
            login(['codex', 'mcp', 'login', 'works_lab', '--no-browser', '--oauth-client-registration', 'dcr',
                   '--scopes', 'messages,keys:read'], env, lab)
            checks.append('real Codex CLI DCR + S256 PKCE + callback + token exchange')
            app = AppServer(env)
            status = app.rpc('mcpServerStatus/list', {})
            server = next(x for x in status['data'] if x['name'] == 'works_lab')
            assert {'waypost_send', 'fetch_entry'} <= set(server['tools']), str(server)
            checks.append('real Codex app-server discovers message and key tools on one configured server')
            thread = app.rpc('thread/start', {'cwd': str(temp), 'ephemeral': True})['thread']['id']
            def call(tool, args):
                return app.rpc('mcpServer/tool/call', {'threadId': thread, 'server': 'works_lab',
                    'tool': tool, 'arguments': args})
            identity = call('works_identity', {})
            assert identity.get('isError') is not True
            value = call('fetch_entry', {'name': 'demo/allowed'})
            assert value.get('isError') is not True
            denied = call('update_entry', {'name': 'demo/allowed', 'value': 'SYNTHETIC'})
            assert denied.get('isError') is True
            sent = call('waypost_send', {'request_id': 'host-one', 'space_id': 'fixture-space', 'text': 'synthetic'})
            assert sent.get('isError') is not True
            checks.append('real Codex tool calls: key read + message send allowed, key write denied')
            # Expire only synthetic access tokens on the fixture; host must use its own saved refresh.
            for at in lab.access.values():
                at[1] = 0
            before = sum(e['event'] == 'refreshed' for e in lab.events)
            value = call('works_identity', {})
            after = sum(e['event'] == 'refreshed' for e in lab.events)
            assert value.get('isError') is not True and after > before
            checks.append('real Codex automatic refresh after server rejects expired access token')
            # Revoke fixture family server-side; subsequent refresh/read must fail.
            for family in lab.families.values():
                family['revoked'] = True
            try:
                rejected = call('works_identity', {})
                assert rejected.get('isError') is True
            except RuntimeError as e:
                if not any(x in str(e).lower() for x in ('auth', 'token', 'unauthorized', 'refresh')):
                    raise
                result['revocation_error'] = re.sub(r'https?://\S+', '<redacted-url>', str(e))[:500]
            checks.append('real Codex denied after server-side family revocation')
        else:
            exe = str(HOST_TOOLS / 'claude')
            env['CLAUDE_CONFIG_DIR'] = str(temp)
            env['CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC'] = '1'
            result['version'] = subprocess.check_output([exe, '--version'], env=env, text=True).strip()
            # User scope here is isolated by CLAUDE_CONFIG_DIR, not the real user profile.
            subprocess.run([exe, 'mcp', 'add', '--scope', 'user', '--transport', 'http', 'works_lab', lab.resource],
                           env=env, check=True, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
            login([exe, 'mcp', 'login', 'works_lab', '--no-browser'], env, lab, terminal=True)
            checks.append('real Claude Code CLI DCR + S256 PKCE + callback + token exchange')
            before = len(lab.events)
            p = subprocess.run([exe, 'mcp', 'get', 'works_lab'], env=env, capture_output=True, text=True, timeout=30)
            assert p.returncode == 0, p.stderr[-400:]
            result['health_check_connected'] = 'Connected' in p.stdout
            checks.append('real Claude Code get health check after saved OAuth login')
            # Running get may initialize/check auth but does not exercise a model tool call.
            result['health_events'] = [e['event'] for e in lab.events[before:]]
            for at in lab.access.values():
                at[1] = 0
            before = sum(e['event'] == 'refreshed' for e in lab.events)
            p = subprocess.run([exe, 'mcp', 'get', 'works_lab'], env=env, capture_output=True, text=True, timeout=30)
            after = sum(e['event'] == 'refreshed' for e in lab.events)
            result['refresh_observed'] = after > before
            assert p.returncode == 0 and 'Connected' in p.stdout and after > before
            if after > before:
                checks.append('real Claude Code refresh observed during health check')
            for family in lab.families.values():
                family['revoked'] = True
            p = subprocess.run([exe, 'mcp', 'get', 'works_lab'], env=env, capture_output=True, text=True, timeout=30)
            result['revoked_health_connected'] = 'Connected' in p.stdout
            assert p.returncode == 0 and not result['revoked_health_connected']
            checks.append('real Claude Code health check after server-side revocation')
        result['status'] = 'passed'
    except Exception as e:
        result['status'] = 'partial'
        # No raw credential data should appear in diagnostics. URLs may contain code/state.
        error = re.sub(r'https?://\S+', '<redacted-url>', str(e))
        result['blocker'] = type(e).__name__ + ': ' + error[:700]
    finally:
        result['checks'] = checks
        result['events'] = lab.events
        if app:
            app.close()
        lab.close()
        shutil.rmtree(temp)
    return result

if __name__ == '__main__':
    p = argparse.ArgumentParser()
    p.add_argument('host', choices=['codex', 'claude'])
    args = p.parse_args()
    print(json.dumps(run(args.host), ensure_ascii=False, indent=2))
