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
    from concurrent.futures import ThreadPoolExecutor
    lab = Lab().start()
    try:
        _, a = grant(lab, 'works:client')
        _, b = grant(lab, 'works:client')
        ta, tb = a['access_token'], b['access_token']
        fa = lab.access[digest(ta)][0]
        assert not call(lab, ta, 'messages_send', {'assistant_id': 'alpha'})['ok']
        for name in ('alpha', 'beta'):
            assert call(lab, ta, 'mount_request', {'assistant_id': name})['ok']
            lab.owner_approve(fa, name)
        assert not call(lab, tb, 'messages_send', {'assistant_id': 'alpha'})['ok']
        with ThreadPoolExecutor(max_workers=8) as pool:
            results = list(pool.map(lambda name: call(lab, ta, 'messages_send', {'assistant_id': name}), ['alpha', 'beta'] * 10))
        assert all(r['ok'] and r['data']['assistant_id'] == n for r, n in zip(results, ['alpha', 'beta'] * 10))
        lab.policies['alpha'] = False
        assert not call(lab, ta, 'messages_send', {'assistant_id': 'alpha'})['ok']
        assert call(lab, ta, 'messages_send', {'assistant_id': 'beta'})['ok']
        print(json.dumps({'status': 'passed', 'checks': ['family isolation', '20 concurrent same-token requests preserve explicit assistant context', 'policy tightening'], 'production': False}, indent=2))
    finally:
        lab.close()

if __name__ == '__main__':
    run()
