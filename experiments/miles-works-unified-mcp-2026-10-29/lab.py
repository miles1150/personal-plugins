#!/usr/bin/env python3
"""Loopback-only feasibility fixture. Synthetic identity/keys, never production.

Not a production OAuth implementation: consent is a fixture HTTP GET, clients and
tokens are in memory, no TLS, no persistence/rate limit. Only binds 127.0.0.1.
"""
import base64
import hashlib
import json
import secrets
import threading
import time
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from urllib.parse import parse_qs, urlencode, urlsplit


def digest(v):
    return hashlib.sha256(v.encode()).hexdigest()


def challenge(v):
    return base64.urlsafe_b64encode(hashlib.sha256(v.encode()).digest()).rstrip(b'=').decode()


SCOPES = {'messages', 'keys:read', 'keys:write'}
TOOLS = [
    ('works_identity', {}, []),
    ('waypost_send', {'request_id': {'type': 'string'}, 'space_id': {'type': 'string'},
                      'text': {'type': 'string'}}, ['request_id', 'space_id', 'text']),
    ('list_entries', {}, []),
    ('fetch_entry', {'name': {'type': 'string'}}, ['name']),
    ('update_entry', {'name': {'type': 'string'}, 'value': {'type': 'string'}}, ['name', 'value']),
    ('connection_request', {'request_id': {'type': 'string'}}, ['request_id']),
]


class Lab:
    def __init__(self):
        self.lock = threading.RLock()
        self.clients, self.codes, self.access, self.refresh, self.families = {}, {}, {}, {}, {}
        self.messages, self.connection_requests, self.receipts = {}, {}, {}
        self.keys = {'demo/allowed': 'SYNTHETIC_ONLY', 'demo/forbidden': 'NEVER_RETURNED'}
        self.events = []
        self.ttl = 3600
        self.http = ThreadingHTTPServer(('127.0.0.1', 0), self.handler())
        self.origin = f'http://127.0.0.1:{self.http.server_port}'
        self.resource = self.origin + '/mcp'

    def start(self):
        threading.Thread(target=self.http.serve_forever, daemon=True).start()
        return self

    def close(self):
        self.http.shutdown()
        self.http.server_close()

    def event(self, kind, **data):
        # Never store bearer tokens, authorization codes, message or key values.
        self.events.append({'event': kind, **data})

    def issue(self, family):
        at, rt = secrets.token_urlsafe(32), secrets.token_urlsafe(32)
        self.access[digest(at)] = [family, time.time() + self.ttl]
        self.refresh[digest(rt)] = [family, False]
        return {'access_token': at, 'refresh_token': rt, 'token_type': 'Bearer',
                'expires_in': self.ttl, 'scope': ' '.join(sorted(self.families[family]['scopes']))}

    def handler(self):
        lab = self

        class Handler(BaseHTTPRequestHandler):
            protocol_version = 'HTTP/1.1'

            def log_message(self, *_):
                pass

            def respond(self, status, value=None, headers=None):
                raw = b'' if value is None else json.dumps(value).encode()
                self.send_response(status)
                for k, v in {'Content-Type': 'application/json', 'Cache-Control': 'no-store',
                             'Content-Length': str(len(raw)), **(headers or {})}.items():
                    self.send_header(k, v)
                self.end_headers()
                self.wfile.write(raw)

            def do_GET(self):
                self.route()

            def do_POST(self):
                self.route()

            def do_DELETE(self):
                self.route()

            def route(self):
                # Synchronize token rotation, revocation and tool authorization.
                with lab.lock:
                    self.route_locked()

            def route_locked(self):
                u = urlsplit(self.path)
                q = {k: v[-1] for k, v in parse_qs(u.query).items()}
                size = int(self.headers.get('Content-Length', 0))
                if size > 65536:
                    return self.respond(413, {'error': 'too_large'})
                raw = self.rfile.read(size)
                try:
                    body = (json.loads(raw) if 'application/json' in self.headers.get('Content-Type', '')
                            else {k: v[-1] for k, v in parse_qs(raw.decode()).items()}) if raw else {}
                except (ValueError, UnicodeError):
                    return self.respond(400, {'error': 'invalid_request'})
                if self.headers.get('Origin') not in (None, lab.origin):
                    return self.respond(403, {'error': 'invalid_origin'})
                if u.path.startswith('/.well-known/oauth-protected-resource'):
                    return self.respond(200, {'resource': lab.resource, 'authorization_servers': [lab.origin],
                                             'scopes_supported': sorted(SCOPES), 'bearer_methods_supported': ['header']})
                if u.path in ('/.well-known/oauth-authorization-server', '/.well-known/openid-configuration'):
                    return self.respond(200, {'issuer': lab.origin, 'authorization_endpoint': lab.origin + '/authorize',
                        'token_endpoint': lab.origin + '/token', 'registration_endpoint': lab.origin + '/register',
                        'revocation_endpoint': lab.origin + '/revoke', 'response_types_supported': ['code'],
                        'grant_types_supported': ['authorization_code', 'refresh_token'],
                        'token_endpoint_auth_methods_supported': ['none'], 'code_challenge_methods_supported': ['S256'],
                        'scopes_supported': sorted(SCOPES), 'authorization_response_iss_parameter_supported': True})
                if u.path == '/register' and self.command == 'POST':
                    redirects = body.get('redirect_uris', [])
                    if not redirects or any(urlsplit(x).scheme != 'http' or
                           urlsplit(x).hostname not in ('127.0.0.1', 'localhost') for x in redirects):
                        return self.respond(400, {'error': 'invalid_redirect_uri'})
                    cid = 'fixture_' + secrets.token_hex(8)
                    lab.clients[cid] = redirects
                    lab.event('dcr', client=cid)
                    return self.respond(201, {'client_id': cid, 'client_id_issued_at': int(time.time()),
                        'redirect_uris': redirects, 'token_endpoint_auth_method': 'none',
                        'grant_types': ['authorization_code', 'refresh_token'], 'response_types': ['code']})
                if u.path == '/authorize' and self.command == 'GET':
                    cid = q.get('client_id')
                    scopes = set(q.get('scope', 'messages').split())
                    if q.get('resource') != lab.resource:
                        return self.respond(400, {'error': 'invalid_target'})
                    if cid not in lab.clients or q.get('redirect_uri') not in lab.clients[cid]:
                        return self.respond(400, {'error': 'invalid_redirect_uri'})
                    if not scopes <= SCOPES or q.get('response_type') != 'code' or q.get('code_challenge_method') != 'S256':
                        return self.respond(400, {'error': 'invalid_request'})
                    # Synthetic owner approval, not a real sign-in or UI consent test.
                    code = secrets.token_urlsafe(32)
                    lab.codes[digest(code)] = {**q, 'expires': time.time() + 120}
                    lab.event('fixture_consent', scopes=sorted(scopes))
                    sep = '&' if '?' in q['redirect_uri'] else '?'
                    return self.respond(302, headers={'Location': q['redirect_uri'] + sep +
                        urlencode({'code': code, 'state': q.get('state', ''), 'iss': lab.origin})})
                if u.path == '/token' and self.command == 'POST':
                    if body.get('resource') != lab.resource:
                        return self.respond(400, {'error': 'invalid_target'})
                    if body.get('grant_type') == 'authorization_code':
                        key = digest(body.get('code', ''))
                        code = lab.codes.get(key)
                        if not code or code['expires'] < time.time() or any(
                            code.get(k) != body.get(k) for k in ('client_id', 'redirect_uri', 'resource')) or \
                            challenge(body.get('code_verifier', '')) != code['code_challenge']:
                            return self.respond(400, {'error': 'invalid_grant'})
                        del lab.codes[key]
                        family = secrets.token_hex(12)
                        lab.families[family] = {'account': 'fixture-owner', 'agent': 'fixture-agent',
                            'connection': family, 'space': 'fixture-space', 'consumer': 'fixture-consumer',
                            'client': body['client_id'], 'scopes': set(code.get('scope', 'messages').split()),
                            'entries': {'demo/allowed'}, 'revoked': False}
                        lab.event('code_exchanged', family=family)
                        return self.respond(200, lab.issue(family))
                    if body.get('grant_type') == 'refresh_token':
                        rt = lab.refresh.get(digest(body.get('refresh_token', '')))
                        if not rt:
                            return self.respond(400, {'error': 'invalid_grant'})
                        family = lab.families[rt[0]]
                        if family['client'] != body.get('client_id'):
                            return self.respond(400, {'error': 'invalid_grant'})
                        if rt[1]:
                            family['revoked'] = True
                            lab.event('refresh_replay_revoked', family=rt[0])
                        if family['revoked']:
                            return self.respond(400, {'error': 'invalid_grant'})
                        requested = set(body.get('scope', ' '.join(family['scopes'])).split())
                        if not requested <= family['scopes']:
                            return self.respond(400, {'error': 'invalid_scope'})
                        # Fixture deliberately allows only identical scopes on refresh.
                        if requested != family['scopes']:
                            return self.respond(400, {'error': 'invalid_scope'})
                        rt[1] = True
                        lab.event('refreshed', family=rt[0])
                        return self.respond(200, lab.issue(rt[0]))
                    return self.respond(400, {'error': 'unsupported_grant_type'})
                if u.path == '/revoke' and self.command == 'POST':
                    token = digest(body.get('token', ''))
                    entry = lab.refresh.get(token) or lab.access.get(token)
                    if entry and lab.families[entry[0]]['client'] == body.get('client_id'):
                        lab.families[entry[0]]['revoked'] = True
                        lab.event('revoked', family=entry[0])
                    return self.respond(200, {})
                if u.path != '/mcp':
                    return self.respond(404, {})
                at = lab.access.get(digest(self.headers.get('Authorization', '').removeprefix('Bearer ')))
                if not at or at[1] <= time.time() or lab.families[at[0]]['revoked']:
                    lab.event('request_denied', reason='missing_expired_or_revoked_token')
                    return self.respond(401, {'error': 'invalid_token'}, {'WWW-Authenticate':
                        f'Bearer resource_metadata="{lab.origin}/.well-known/oauth-protected-resource/mcp", scope="messages"'})
                if self.command != 'POST':
                    return self.respond(405, headers={'Allow': 'POST'})
                family = lab.families[at[0]]
                method, ident, params = body.get('method'), body.get('id'), body.get('params', {})
                lab.event('protocol', method=method)
                if ident is None:
                    return self.respond(202)
                if method == 'initialize':
                    result = {'protocolVersion': '2025-11-25', 'capabilities': {'tools': {'listChanged': False}},
                              'serverInfo': {'name': 'miles-works-isolated-lab', 'version': '0.1'}}
                elif method == 'ping':
                    result = {}
                elif method == 'tools/list':
                    result = {'tools': [{'name': name, 'description': 'Synthetic isolated fixture: ' + name,
                        'inputSchema': {'type': 'object', 'properties': props, 'required': required,
                                        'additionalProperties': False}} for name, props, required in TOOLS]}
                elif method == 'tools/call':
                    name, args = params.get('name'), params.get('arguments', {})
                    result = lab.call(family, name, args)
                elif method in ('resources/list', 'prompts/list'):
                    result = {method.split('/')[0]: []}
                else:
                    return self.respond(200, {'jsonrpc': '2.0', 'id': ident,
                                             'error': {'code': -32601, 'message': 'Method not found'}})
                return self.respond(200, {'jsonrpc': '2.0', 'id': ident, 'result': result})

        return Handler

    def call(self, f, name, args):
        data, error = None, None
        known = {t[0] for t in TOOLS}
        if name not in known:
            error = 'UNKNOWN_TOOL'
        elif name == 'works_identity':
            data = {k: f[k] for k in ('account', 'agent', 'connection', 'space', 'consumer')}
        elif name == 'waypost_send':
            if 'messages' not in f['scopes'] or args.get('space_id') != f['space']:
                error = 'FORBIDDEN'
            else:
                key = (f['account'], f['agent'], args['request_id'])
                fingerprint = digest(json.dumps(args, sort_keys=True))
                prior = self.messages.get(key)
                if prior and prior[0] != fingerprint:
                    error = 'IDEMPOTENCY_CONFLICT'
                else:
                    if not prior:
                        self.messages[key] = (fingerprint, 'message_' + secrets.token_hex(6))
                    data = {'message_id': self.messages[key][1], 'replayed': bool(prior)}
        elif name == 'connection_request':
            # A proposal cannot manufacture a connection or credentials.
            key = (f['connection'], args['request_id'])
            self.connection_requests.setdefault(key, 'pending_' + secrets.token_hex(6))
            data = {'status': 'awaiting_owner_consent', 'request_id': self.connection_requests[key]}
        else:
            scope = 'keys:write' if name == 'update_entry' else 'keys:read'
            if scope not in f['scopes']:
                error = 'SCOPE_NOT_GRANTED'
            elif name == 'list_entries':
                data = {'entries': sorted(f['entries'])}
            elif args.get('name') not in f['entries']:
                error = 'ENTRY_NOT_FOUND'
            elif name == 'fetch_entry':
                data = {'value': self.keys[args['name']]}
            else:
                self.keys[args['name']] = args['value']
                data = {'updated': args['name']}
        self.event('tool', name=name, allowed=error is None)
        payload = {'ok': error is None, **({'data': data} if error is None else {'error': {'code': error}})}
        return {'content': [{'type': 'text', 'text': json.dumps(payload)}],
                'structuredContent': payload, 'isError': error is not None}


if __name__ == '__main__':
    lab = Lab().start()
    print(json.dumps({'origin': lab.origin, 'resource': lab.resource}), flush=True)
    try:
        threading.Event().wait()
    except KeyboardInterrupt:
        lab.close()
