#!/usr/bin/env python3
"""Local acceptance of the keys product on the native runtime.

Starts `waypost serve` with WAYPOST_PRODUCT=keys on a disposable data directory
and a synthetic, seeded account session (no real identity provider), then:

  1. acts as an agent host that registers dynamically (RFC 7591), acting as the
     browser that approves each consent page;
  2. seeds entries through a write grant over MCP;
  3. lists and fetches them with a read grant, and checks writes never echo values;
  4. checks a read-only grant cannot write and a write grant cannot read.

Usage: python3 scripts/verify_keys_native.py --waypost target/debug/waypost
   or, against the compiled Worker started by
   `node scripts/serve_local_workerd.mjs --product keys --port 8795 --dcr`:
       python3 scripts/verify_keys_native.py --base http://127.0.0.1:8795
Never prints secret values or tokens.
"""
import argparse
import base64
import hashlib
import http.cookiejar
import json
import os
import secrets
import socket
import sqlite3
import subprocess
import sys
import tempfile
import time
import urllib.error
import urllib.parse
import urllib.request
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
SESSION = secrets.token_urlsafe(32)[:43]
VALUE = 'kv_' + secrets.token_urlsafe(24)
IMPORTED = 'imp_' + secrets.token_urlsafe(24)
AGENT_WRITE = 'agw_' + secrets.token_urlsafe(24)


class NoRedirect(urllib.request.HTTPRedirectHandler):
    def redirect_request(self, *args):
        return None


opener = urllib.request.build_opener(NoRedirect, urllib.request.ProxyHandler({}))


def request(base, path, body=None, form=False, headers=None, method=None):
    headers = dict(headers or {})
    data = None
    if body is not None:
        headers['Content-Type'] = 'application/x-www-form-urlencoded' if form else 'application/json'
        data = (urllib.parse.urlencode(body) if form else json.dumps(body)).encode()
    req = urllib.request.Request(base + path, data=data, headers=headers, method=method)
    try:
        r = opener.open(req, timeout=20)
    except urllib.error.HTTPError as e:
        r = e
    raw = r.read()
    try:
        value = json.loads(raw) if raw else None
    except ValueError:
        value = None
    return r.status, r.headers, value, raw


def free_port():
    with socket.socket() as s:
        s.bind(('127.0.0.1', 0))
        return s.getsockname()[1]


def public_key():
    pem = ROOT / 'tests/fixtures/oidc-test-private.pem'
    modulus = subprocess.run(['openssl', 'rsa', '-in', str(pem), '-noout', '-modulus'], capture_output=True, text=True, check=True).stdout
    n = bytes.fromhex(modulus.strip().split('=', 1)[1])
    return base64.urlsafe_b64encode(n).rstrip(b'=').decode()


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--waypost')
    parser.add_argument('--base', help='use an already running keys server seeded with the synthetic session')
    args = parser.parse_args()
    work = Path(tempfile.mkdtemp(prefix='keys-acceptance-'))
    if args.base:
        global SESSION
        SESSION = 'c' * 43
        return run(args, work, args.base, None, None)
    if not args.waypost:
        parser.error('--waypost or --base is required')
    port = free_port()
    base = f'http://127.0.0.1:{port}'
    oidc = work / 'oidc.json'
    oidc.write_text(json.dumps({'issuer': 'https://issuer.example', 'client_id': 'test-client', 'authorization_endpoint': 'https://issuer.example/authorize', 'token_endpoint': 'https://issuer.example/token', 'keys': [{'kid': 'test', 'n': public_key(), 'e': 'AQAB'}]}))
    oidc.chmod(0o600)
    env = {**os.environ, 'WAYPOST_PRODUCT': 'keys', 'WAYPOST_OPERATOR_MODE': 'disabled', 'WAYPOST_OIDC_CONFIG_FILE': str(oidc),
           'WAYPOST_OAUTH_CLIENT_DISCOVERY': json.dumps({'dynamic_registration': True}), 'KEYS_OWNER_SUBJECT': 'fixture', 'WAYPOST_CURSOR_SECRET': secrets.token_hex(32), 'WAYPOST_CSRF_SECRET': secrets.token_hex(32),
           'WAYPOST_ENCRYPTION_KEYS': json.dumps({'active': 'w1', 'keys': {'w1': secrets.token_hex(32)}}),
           'KEYS_DATA_KEYS': json.dumps({'active': 'k1', 'keys': {'k1': secrets.token_hex(32)}}),
           'WAYPOST_PUBLIC_ORIGIN': base, 'WAYPOST_ACCOUNT_SIGNUP_LIMIT': '1'}
    data = work / 'data'
    subprocess.run([args.waypost, 'init', '--data', str(data)], env=env, check=True, capture_output=True)
    db = sqlite3.connect(data / 'metadata.sqlite')
    now = int(time.time() * 1000)
    db.execute("INSERT INTO accounts(account_id,issuer,subject,created_at) VALUES('acct_fixture','https://issuer.example','fixture',?)", (now,))
    db.execute("INSERT INTO account_sessions(session_digest,account_id,created_at,expires_at) VALUES(?,'acct_fixture',?,?)", (hashlib.sha256(SESSION.encode()).digest(), now, now + 3600000))
    db.commit()
    db.close()
    server = subprocess.Popen([args.waypost, 'serve', '--data', str(data), '--listen', f'127.0.0.1:{port}'], env=env, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    run(args, work, base, server, data)


def run(args, work, base, server, data):
    phase = 'startup'
    try:
        for _ in range(100):
            try:
                if request(base, '/health')[0] == 200:
                    break
            except OSError:
                pass
            time.sleep(0.1)
        cookie = 'waypost_account=' + SESSION
        csrf = request(base, '/account/session', headers={'Cookie': cookie})[2]['data']['csrf_token']

        def approve(authorize_url, grant):
            parsed = urllib.parse.urlparse(authorize_url)
            status, headers, error, _ = request(base, parsed.path + '?' + parsed.query)
            assert status == 303, (status, error)
            request_id = urllib.parse.parse_qs(urllib.parse.urlparse(headers['Location']).query)['oauth_request'][0]
            browser = cookie + '; ' + headers['Set-Cookie'].split(';')[0]
            status, _, preview, _ = request(base, '/oauth/consent?request_id=' + request_id, headers={'Cookie': browser})
            assert status == 200, status
            status, _, decided, _ = request(base, '/oauth/consent', {'request_id': request_id, 'decision': 'approve', **grant}, headers={'Cookie': browser, 'Origin': base, 'X-Waypost-CSRF': csrf})
            assert status == 200, (status, decided)
            return preview, decided['redirect_uri']

        phase = 'agent host: dynamic registration and read grant'
        redirect = f'http://127.0.0.1:{free_port()}/callback'
        status, _, client, _ = request(base, '/oauth/register', {'client_name': 'Acceptance agent', 'redirect_uris': [redirect], 'token_endpoint_auth_method': 'none', 'grant_types': ['authorization_code', 'refresh_token']})
        assert status == 201, status

        def agent_token(scope, grant):
            verifier = secrets.token_urlsafe(48)
            challenge = base64.urlsafe_b64encode(hashlib.sha256(verifier.encode()).digest()).rstrip(b'=').decode()
            q = urllib.parse.urlencode({'client_id': client['client_id'], 'redirect_uri': redirect, 'response_type': 'code', 'resource': base + '/mcp', 'scope': scope, 'state': 's', 'code_challenge': challenge, 'code_challenge_method': 'S256'})
            _, callback = approve(base + '/oauth/authorize?' + q, grant)
            code = urllib.parse.parse_qs(urllib.parse.urlparse(callback).query)['code'][0]
            status, _, tokens, _ = request(base, '/oauth/token', {'grant_type': 'authorization_code', 'client_id': client['client_id'], 'code': code, 'redirect_uri': redirect, 'code_verifier': verifier, 'resource': base + '/mcp'}, form=True)
            assert status == 200, tokens
            return tokens['access_token']

        def tool(token, name, arguments):
            status, _, value, raw = request(base, '/mcp', {'jsonrpc': '2.0', 'id': 1, 'method': 'tools/call', 'params': {'name': name, 'arguments': arguments}}, headers={'Authorization': 'Bearer ' + token, 'MCP-Protocol-Version': '2025-11-25'})
            assert status == 200, status
            return value['result']['structuredContent'], raw.decode()


        phase = 'agent host: seed entries through a write grant'
        seeder = agent_token('keys:read keys:write', {'can_create': True, 'all_write': True})
        for name, fields in (('cli/token', {'API_TOKEN': VALUE}), ('imported/one', {'SECRET': IMPORTED})):
            created, raw = tool(seeder, 'create_entry', {'name': name, 'fields': fields})
            assert created['ok'] and VALUE not in raw and IMPORTED not in raw, created

        reader = agent_token('keys:read', {'all_read': True})
        listed, raw = tool(reader, 'list_entries', {})
        assert {e['name'] for e in listed['data']['entries']} == {'cli/token', 'imported/one'} and VALUE not in raw
        fetched, raw = tool(reader, 'fetch_entry', {'name': 'cli/token'})
        assert fetched['data']['fields'] == {'API_TOKEN': VALUE}
        fetched, _ = tool(reader, 'fetch_entry', {'name': 'imported/one'})
        assert fetched['data']['fields'] == {'SECRET': IMPORTED}
        denied, raw = tool(reader, 'create_entry', {'name': 'agent/new', 'fields': {'X': AGENT_WRITE}})
        assert denied['error']['code'] == 'CREATE_NOT_GRANTED' and AGENT_WRITE not in raw

        phase = 'agent host: write grant never echoes values'
        writer = agent_token('keys:read keys:write', {'can_create': True})
        created, raw = tool(writer, 'create_entry', {'name': 'agent/new', 'fields': {'X': AGENT_WRITE}})
        assert created['ok'] and AGENT_WRITE not in raw, created
        updated, raw = tool(writer, 'update_entry', {'name': 'agent/new', 'set_fields': {'X': AGENT_WRITE + '2'}})
        assert updated['ok'] and AGENT_WRITE not in raw
        hidden, _ = tool(writer, 'fetch_entry', {'name': 'cli/token'})
        assert hidden['error']['code'] == 'ENTRY_NOT_FOUND'

        if data is not None:
            phase = 'storage holds ciphertext only'
            db = sqlite3.connect(data / 'metadata.sqlite')
            dump = '\n'.join(db.iterdump())
            db.close()
            for v in (VALUE, IMPORTED, AGENT_WRITE):
                assert v not in dump
        print(json.dumps({'result': 'passed', 'runtime': 'native' if server else base, 'seeded_through_write_grant': True, 'agent_dynamic_registration': True, 'tool_contract': True, 'read_write_separation': True, 'ciphertext_at_rest': data is not None, 'real_idp': False}))
    except Exception as error:
        print(json.dumps({'result': 'failed', 'phase': phase, 'error': type(error).__name__}), file=sys.stderr)
        if os.environ.get('KEYS_ACCEPTANCE_DEBUG') == '1':
            raise
        raise SystemExit(1)
    finally:
        if server:
            server.terminate()
            server.wait(timeout=10)


if __name__ == '__main__':
    main()
