#!/usr/bin/env python3
"""Run isolated HTTP/OAuth boundary tests; output contains no token/value."""
import json
import secrets
import subprocess
import sys
from pathlib import Path
import urllib.error
import urllib.parse
import urllib.request
from lab import Lab, challenge, digest

class NoRedirect(urllib.request.HTTPRedirectHandler):
    def redirect_request(self, *_):
        return None

HTTP = urllib.request.build_opener(NoRedirect, urllib.request.ProxyHandler({}))

def request(base, path, body=None, token=None, form=False, headers=None):
    h = dict(headers or {})
    if token:
        h['Authorization'] = 'Bearer ' + token
    data = None
    if body is not None:
        data = (urllib.parse.urlencode(body) if form else json.dumps(body)).encode()
        h['Content-Type'] = 'application/x-www-form-urlencoded' if form else 'application/json'
    try:
        r = HTTP.open(urllib.request.Request(base + path, data=data, headers=h), timeout=10)
    except urllib.error.HTTPError as e:
        r = e
    raw = r.read()
    return r.status, r.headers, json.loads(raw) if raw else None

def grant(lab, scope='messages keys:read'):
    base = lab.origin
    redirect = 'http://127.0.0.1:19099/callback'
    s, _, client = request(base, '/register', {'redirect_uris': [redirect],
                         'client_name': 'isolated-test', 'token_endpoint_auth_method': 'none'})
    assert s == 201
    v = secrets.token_urlsafe(48)
    params = {'client_id': client['client_id'], 'redirect_uri': redirect, 'resource': lab.resource,
              'response_type': 'code', 'scope': scope, 'state': 'fixture-state',
              'code_challenge': challenge(v), 'code_challenge_method': 'S256'}
    s, h, _ = request(base, '/authorize?' + urllib.parse.urlencode(params))
    assert s == 302
    code = urllib.parse.parse_qs(urllib.parse.urlsplit(h['Location']).query)['code'][0]
    form = {'grant_type': 'authorization_code', 'client_id': client['client_id'], 'redirect_uri': redirect,
            'resource': lab.resource, 'code': code, 'code_verifier': v}
    assert request(base, '/token', {**form, 'code_verifier': 'wrong'}, form=True)[0] == 400
    s, _, tokens = request(base, '/token', form, form=True)
    assert s == 200
    assert request(base, '/token', form, form=True)[0] == 400
    return client, tokens

def rpc(lab, token, method, params=None):
    return request(lab.origin, '/mcp', {'jsonrpc': '2.0', 'id': 1, 'method': method,
                                      'params': params or {}}, token)

def call(lab, token, name, args=None):
    s, _, data = rpc(lab, token, 'tools/call', {'name': name, 'arguments': args or {}})
    assert s == 200
    return data['result']['structuredContent']

def run():
    lab = Lab().start()
    checks = []
    try:
        s, h, _ = rpc(lab, None, 'initialize')
        assert s == 401 and 'resource_metadata=' in h['WWW-Authenticate']
        checks.append('unauthenticated catalog denied; resource discovery challenge')
        c, t = grant(lab)
        token = t['access_token']
        tools = rpc(lab, token, 'tools/list')[2]['result']['tools']
        assert {'waypost_send', 'fetch_entry'} <= {x['name'] for x in tools}
        checks.append('one endpoint / one DCR grant exposes both catalogs; PKCE/code replay checks')
        assert call(lab, token, 'fetch_entry', {'name': 'demo/allowed'})['ok']
        assert not call(lab, token, 'fetch_entry', {'name': 'demo/forbidden'})['ok']
        assert not call(lab, token, 'update_entry', {'name': 'demo/allowed', 'value': 'SYNTHETIC_UPDATE'})['ok']
        checks.append('entry ACL and read/write scopes enforced on direct tools/call')
        args = {'request_id': 'stable-1', 'space_id': 'fixture-space', 'text': 'synthetic-message'}
        first = call(lab, token, 'waypost_send', args)
        replay = call(lab, token, 'waypost_send', args)
        assert first['data']['message_id'] == replay['data']['message_id'] and replay['data']['replayed']
        assert not call(lab, token, 'waypost_send', {**args, 'text': 'changed'})['ok']
        assert not call(lab, token, 'waypost_send', {**args, 'space_id': 'another-space'})['ok']
        checks.append('message idempotency, body conflict and cross-space denial')
        proposed = call(lab, token, 'connection_request', {'request_id': 'new-connection'})
        assert proposed['data']['status'] == 'awaiting_owner_consent' and len(lab.families) == 1
        checks.append('MCP connection request creates pending proposal only')
        f = {'grant_type': 'refresh_token', 'client_id': c['client_id'],
             'refresh_token': t['refresh_token'], 'resource': lab.resource}
        assert request(lab.origin, '/token', {**f, 'resource': 'http://other.invalid/mcp'}, form=True)[0] == 400
        assert request(lab.origin, '/token', {**f, 'scope': 'messages keys:read keys:write'}, form=True)[0] == 400
        s, _, fresh = request(lab.origin, '/token', f, form=True)
        assert s == 200 and fresh['refresh_token'] != t['refresh_token']
        assert rpc(lab, fresh['access_token'], 'ping')[0] == 200
        assert request(lab.origin, '/token', f, form=True)[0] == 400
        assert rpc(lab, fresh['access_token'], 'ping')[0] == 401
        checks.append('refresh rotation; audience/scope rejection; refresh replay revokes family')
        c2, t2 = grant(lab)
        family = lab.families[lab.access[digest(t2['access_token'])][0]]
        family['entries'].clear()
        assert not call(lab, t2['access_token'], 'fetch_entry', {'name': 'demo/allowed'})['ok']
        checks.append('grant removed after issuance immediately blocks read')
        request(lab.origin, '/revoke', {'client_id': c2['client_id'], 'token': t2['refresh_token']}, form=True)
        assert rpc(lab, t2['access_token'], 'ping')[0] == 401
        assert request(lab.origin, '/token', {'grant_type': 'refresh_token', 'client_id': c2['client_id'],
            'refresh_token': t2['refresh_token'], 'resource': lab.resource}, form=True)[0] == 400
        checks.append('revoke fences both access and refresh')
        _, msg = grant(lab, 'messages')
        assert not call(lab, msg['access_token'], 'list_entries')['ok']
        assert rpc(lab, msg['access_token'], 'ping')[0] == 200
        s, _, _ = request(lab.origin, '/mcp', {'jsonrpc': '2.0', 'id': 1, 'method': 'ping'},
                           msg['access_token'], headers={'Origin': 'http://untrusted.invalid'})
        assert s == 403
        checks.append('message-only grant cannot read keys; untrusted Origin denied')
        if '--sdk' in sys.argv:
            _, sdk = grant(lab)
            done = subprocess.run(['node', str(Path(__file__).with_name('sdk_verify.mjs'))],
                input=json.dumps({'resource': lab.resource, 'token': sdk['access_token']}),
                capture_output=True, text=True, timeout=20, check=True)
            assert json.loads(done.stdout)['status'] == 'passed'
            checks.append('official MCP TypeScript SDK initialize/list/call with mixed catalog and ACL denial')
        result = {'status': 'passed', 'checks': checks, 'count': len(checks),
                  'real_idp': False, 'production': False, 'implementation': 'isolated fixture'}
        print(json.dumps(result, ensure_ascii=False, indent=2))
        return result
    finally:
        lab.close()

if __name__ == '__main__':
    run()
