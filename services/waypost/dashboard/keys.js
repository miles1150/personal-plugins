'use strict';
// keyenv-remote consent and connection management. All text is set with
// textContent; nothing from the server is parsed as HTML.
const el = (id) => document.getElementById(id);
const hintKey = 'keyenv-remote.oauth.request';
let csrf = null;
let request = null;
let keyPermissions = null;
let loginPending = false, authRevision = 0, bootstrapController = null;

function status(text) { el('status').textContent = text || ''; }
function readHint() {
  const incoming = new URL(location.href).searchParams.get('oauth_request');
  let hint = incoming;
  try { hint = incoming || sessionStorage.getItem(hintKey); } catch (_) {}
  if (hint && /^oauth_[A-Za-z0-9_-]{1,128}$/.test(hint)) {
    try { sessionStorage.setItem(hintKey, hint); } catch (_) {}
    return hint;
  }
  return null;
}
function clearHint() {
  try { sessionStorage.removeItem(hintKey); } catch (_) {}
  const url = new URL(location.href); url.searchParams.delete('oauth_request'); history.replaceState(null, '', url);
}
// The deadline includes response-body parsing, not only response headers.
async function requestJSON(path, options = {}) {
  const controller = new AbortController(), outerSignal = options.signal;
  const cancel = () => controller.abort();
  if (outerSignal?.aborted) cancel();
  else outerSignal?.addEventListener('abort', cancel, {once:true});
  let timedOut = false;
  const timer = setTimeout(() => { timedOut = true; controller.abort(); }, 15000);
  try {
    const response = await fetch(path, {...options, signal:controller.signal});
    const value = await response.json();
    return {response, value};
  } catch (error) {
    if (timedOut) throw new Error('Request timed out. Check your connection and try again.');
    throw error;
  } finally {
    clearTimeout(timer);
    outerSignal?.removeEventListener('abort', cancel);
  }
}
async function api(path, body, method, signal) {
  const headers = {};
  if (body !== undefined) headers['content-type'] = 'application/json';
  if (csrf && (method || 'GET') !== 'GET') headers['x-waypost-csrf'] = csrf;
  const {response, value} = await requestJSON(path, {method: method || 'GET', signal, headers, credentials: 'same-origin', body: body === undefined ? undefined : JSON.stringify(body)});
  return {status: response.status, data:value};
}
async function showConsent() {
  const revision = authRevision;
  const {status: code, data} = await api('/oauth/consent?request_id=' + encodeURIComponent(request));
  if (revision !== authRevision) return;
  if (code !== 200) { status('This authorization request is no longer valid. Start the connection again from your app.'); clearHint(); return; }
  el('client-name').textContent = data.client_name;
  el('client-detail').textContent = data.client_kind === 'metadata_document' ? data.client_host + ' (' + data.client_id + ')' : data.client_id + ' (' + data.client_kind.replace(/_/g, ' ') + ')';
  el('redirect-host').textContent = data.redirect_host || '';
  el('scope').textContent = data.scope;
  el('warnings').replaceChildren(...data.client_warnings.concat([data.warning]).map((w) => { const li = document.createElement('li'); li.textContent = w; return li; }));
  keyPermissions = window.MilesKeyPermissions(el('consent'), data);
  el('consent').hidden = false;
}
async function decide(decision) {
  const body = {request_id: request, decision};
  if (decision === 'approve') {
    try { Object.assign(body, keyPermissions.validate()); }
    catch (error) { status(error.message); el('broad-ack').focus(); return; }
    if (!body.all_read && !body.all_write && !body.can_create && !body.entries.length) { status('Choose at least one permission.'); return; }
  }
  status(''); el('approve').disabled = true; el('deny').disabled = true;
  const {status: code, data} = await api('/oauth/consent', body, 'POST');
  if (code === 200 && data && data.redirect_uri) { clearHint(); location.assign(data.redirect_uri); return; }
  el('approve').disabled = false; el('deny').disabled = false;
  if (data && data.error === 'invalid_request' && decision === 'approve') { status('Choose at least one permission.'); return; }
  status('Approval needs a sign-in from the last 15 minutes. Sign in again; if the request has expired, start the connection again from your app.');
  el('signin').hidden = false;
}
async function showGrants() {
  const revision = authRevision;
  const {status: code, data} = await api('/account/keys/grants');
  if (revision !== authRevision) return;
  if (code !== 200) { status('Connections could not be loaded. Sign in again and retry.'); return; }
  const items = data.data.grants.map((g) => {
    const li = document.createElement('li');
    const title = document.createElement('strong'); title.textContent = g.client_id;
    const scope = document.createElement('small'); scope.textContent = g.scope;
    const summary = document.createElement('p');
    summary.textContent = [g.all_read ? 'Read all existing and future entries' : '', g.all_write ? 'Change all existing and future entries' : '', g.can_create ? 'Create entries' : '', ...g.entries.map(e => e.name + ': ' + [e.read ? 'read' : '', e.write ? 'change' : ''].filter(Boolean).join(' and '))].filter(Boolean).join('; ') || 'No keys permissions';
    li.append(title, scope, summary);
    const revoke = document.createElement('button'); revoke.type = 'button'; revoke.className = 'secondary'; revoke.textContent = 'Revoke keys access';
    revoke.onclick = async () => {
      revoke.disabled = true;
      try {
        const result = await api('/account/keys/grants/revoke', {grant_id: g.grant_id}, 'POST');
        if (result.status !== 200) throw new Error('Keys access could not be revoked. Sign in again and retry.');
        status('Keys access revoked. Values already returned cannot be recalled.'); await showGrants();
      } catch (error) { status(error.message); revoke.disabled = false; }
    };
    li.append(revoke); return li;
  });
  el('grant-list').replaceChildren(...items);
  if (!items.length) { const empty = document.createElement('li'); empty.textContent = 'No clients have keys access.'; el('grant-list').append(empty); }
  el('grants').hidden = false;
}
el('login').onclick = async () => {
  if (loginPending) return;
  loginPending = true;
  const revision = ++authRevision;
  bootstrapController?.abort(); bootstrapController = null;
  const button = el('login'), label = button.textContent;
  button.disabled = true; button.textContent = 'Opening Google…';
  status('Opening Google sign-in…');
  let navigating = false;
  try {
    const {status: code, data} = await api('/account/login', {}, 'POST');
    if (revision !== authRevision) return;
    if (code !== 200 || typeof data?.data?.authorization_url !== 'string' || !data.data.authorization_url.trim()) throw new Error('Could not start sign-in. Please try again.');
    location.assign(data.data.authorization_url); navigating = true;
  } catch (error) {
    if (revision === authRevision) status(error.message === 'Failed to fetch' ? 'Could not start sign-in. Check your connection and try again.' : error.message);
  } finally {
    if (revision === authRevision && !navigating) { loginPending = false; button.disabled = false; button.textContent = label; }
  }
};
el('approve').onclick = () => decide('approve').catch(error => { status('Connection failed. Check your network and retry.'); el('approve').disabled = false; el('deny').disabled = false; });
el('deny').onclick = () => decide('deny').catch(error => { status('Could not deny the request. Check your network and retry.'); el('approve').disabled = false; el('deny').disabled = false; });
el('logout').onclick = async () => { await api('/account/session', undefined, 'DELETE'); location.reload(); };

(async () => {
  const revision = authRevision, controller = new AbortController();
  bootstrapController = controller;
  request = readHint();
  try {
    const session = await api('/account/session', undefined, 'GET', controller.signal);
    if (revision !== authRevision) return;
    if (session.status === 401) { csrf = null; el('signin').hidden = false; status('Sign in with Google to continue.'); return; }
    if (session.status !== 200) throw new Error('Unable to check sign-in. Please refresh or try again.');
    csrf = session.data.data.csrf_token;
    if (request) await showConsent(); else await showGrants();
  } catch (error) {
    if (revision === authRevision) { el('signin').hidden = false; status(error.message || 'Unable to load this page. Check your connection and refresh.'); }
  } finally { if (bootstrapController === controller) bootstrapController = null; }
})();
