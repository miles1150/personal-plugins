'use strict';
const el = id => document.getElementById(id);
let csrf = '';
let accountSpaces = [], oauthInfo = null, identitiesLoaded = false;
const tx = (node, source, vars={}) => window.MilesAccountI18n ? window.MilesAccountI18n.text(node, source, vars) : (node.textContent=source.replace(/\{(\w+)\}/g, (_,key)=>vars[key] ?? '{'+key+'}'));
const t = (source, vars={}) => window.MilesAccountI18n?.t(source,vars) ?? source.replace(/\{(\w+)\}/g, (_,key)=>vars[key] ?? '{'+key+'}');
const translatedRenders = new Map();
function localize(node, render) { translatedRenders.set(node, render); render(); }
let inspectedCode = null;
let managementRevision = 0;
let primarySelection = null;
let identityActors = [], identityConsumers = [];
let identityRevision = 0, currentSpace = null, currentSpaceName = '', unifiedProduct = false, keyPermissions = null;
let actionPending = false;
let loginPending = false, authRevision = 0, bootstrapController = null;
let pendingIdentity = null;
function resetManagement() {
  managementRevision++; primarySelection=null;
  for(const kind of ['connections','oauth','actors','consumers']) {el('management-'+kind).replaceChildren();el('more-'+kind).hidden=true;}
  el('primary-clear').disabled=true;el('make-primary').disabled=true;tx(el('primary-info'), '');
}
let oauthPreview = false;
let oauthDeviceCode = null, oauthDeviceInfo = null;
let oauthRevision = 0;
// This ID is only a navigation hint. The server's HttpOnly cookie binds consent.
const oauthHintKey = 'waypost.oauth.request';
let oauthRequest = null;
try {
  const incoming = new URL(location.href).searchParams.get('oauth_request');
  const hint = incoming || sessionStorage.getItem(oauthHintKey);
  if (hint && /^oauth_[A-Za-z0-9_-]{1,128}$/.test(hint)) {
    oauthRequest = hint; sessionStorage.setItem(oauthHintKey, hint);
  }
} catch (_) { /* Storage may be unavailable; direct navigation still works below. */ }
el('oauth-browser').hidden = !oauthRequest;
function resetOAuthConsent() {
  oauthRevision++; oauthPreview = false; oauthDeviceCode = null;
  for (const id of ['oauth-approve','oauth-deny','oauth-device-approve','oauth-device-deny']) el(id).disabled = true;
  keyPermissions = null; el('key-access').hidden = true;
  el('oauth-inspect').hidden = !oauthRequest;
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
async function oauthApi(path, body, method = 'POST') {
  const {response, value} = await requestJSON(path, {method, credentials:'same-origin', headers:{'Content-Type':'application/json','X-Waypost-CSRF':csrf}, ...(method==='GET'?{}:{body:JSON.stringify(body)})});
  if (!response.ok || value.error) {
    const message = value.error === 'invalid_request' ? 'This connection request expired or belongs to another browser. Restart Connect in your assistant and finish in the same browser.' : 'Could not review this connection. Sign in again, then retry. No access was granted.';
    throw new Error(message);
  }
  return value;
}
function renderOAuthReview(target, info, browser = false) {
  const sources = {dcr:'Dynamic registration (client-supplied metadata)',cimd:'URL metadata document (client-supplied metadata)',legacy:'Migrated / administrator-configured client'};
  const lines = [
    t('Client name (unverified): {name}', {name: info.client_name || t('Not supplied')}),
    t('Client ID: {id}', {id: info.client_id || t('Not supplied')}),
    t('Client source: {source}', {source: t(sources[info.client_source] || 'Unknown — not verified')}),
    t('Approval identity: {actor} / {consumer}', {actor: el('actor-id').value || t('Choose an assistant above'), consumer: el('consumer-id').value || t('Choose a connection identity above')}),
  ];
  if (browser) {
    let redirect;
    try {redirect = new URL(info.redirect_uri);} catch (_) {throw new Error('Cannot review the actual redirect destination. Restart authorization.');}
    if (!['http:','https:'].includes(redirect.protocol) || redirect.username || redirect.password) throw new Error('Invalid authorization redirect destination.');
    lines.push(t('Actual redirect URI: {uri}', {uri: redirect.href}));
    const local = ['127.0.0.1','[::1]','localhost'].includes(redirect.hostname);
    tx(el('oauth-review'), 'After allowing access, return to {client}.', {client: info.client_name || t('This client')});
    if (local) lines.push(t('Loopback callback.'));
  } else lines.push(t('Device authorization: no browser callback. Verify the code came from your own client.'));
  lines.push(t('Resource: {resource}', {resource: info.resource || t('Not supplied')}), t('Scope: {scope}', {scope: info.scope || t('Not supplied')}), t('Selected Space: {id}', {id: info.selected_space_id || t('None')}), info.warning || t('Approval grants access to the selected Space, not account management.'));
  target.textContent = lines.join('\n');
}
async function reviewOAuth() {
  el('oauth-inspect').hidden = false;
  oauthPreview = false; el('oauth-approve').disabled = true; el('oauth-deny').disabled = true;
  if (!oauthRequest) return;
  const requestId = oauthRequest, revision = ++oauthRevision;
  const info = await oauthApi('/oauth/consent?request_id='+encodeURIComponent(oauthRequest), null, 'GET');
  if (oauthRequest !== requestId || oauthRevision !== revision) return;
  oauthInfo = info;
  renderOAuthReview(el('oauth-info'), info, true);
  if (info.product === 'unified' && info.vault && (info.scope || '').split(' ').some(scope => scope === 'keys:read' || scope === 'keys:write')) {
    el('key-access').hidden = false;
    keyPermissions = window.MilesKeyPermissions(el('key-access'), info);
    // The user's chosen default is read/change/create; consent stays explicit.
    el('preset-trusted').click();
  }
  el('oauth-inspect').hidden = true;
  oauthPreview = true; el('oauth-approve').disabled = !hasIdentity(); el('oauth-deny').disabled = false;
  renderConnection();
}
function clearOAuthHint() {
  oauthRequest = null; oauthInfo = null; resetOAuthConsent(); el('oauth-browser').hidden = true;
  document.body.dataset.view = csrf ? 'manage' : 'login';
  el('connection-settings').open = Boolean(csrf);
  renderConnection();
  try { sessionStorage.removeItem(oauthHintKey); } catch (_) {}
  const url = new URL(location.href); url.searchParams.delete('oauth_request'); history.replaceState(null,'',url);
}
async function api(path, body, method = 'POST', signal) {
  const {response, value} = await requestJSON(path, {method, signal, credentials: 'same-origin', headers: {'Content-Type': 'application/json', 'X-Waypost-CSRF': csrf}, ...(method === 'GET' ? {} : {body: JSON.stringify(body || {})})});
  if (!response.ok || !value.ok) {
    const error = new Error(value.error?.message || 'Request failed');
    error.status = response.status;
    throw error;
  }
  return value.data;
}
function showSignedOut() {
  document.body.dataset.view = 'login';
  csrf = ''; currentSpace = null; currentSpaceName = '';
  el('login').hidden = false; el('login').disabled = false; el('logout').hidden = true;
  el('spaces').replaceChildren(); el('device-list').replaceChildren(); el('management-keys').replaceChildren();
  el('keys-connections').hidden = true;
  tx(el('status'), 'Sign in with Google to continue.');
  renderConnection();
}
async function startLogin() {
  if (loginPending) return;
  loginPending = true;
  const revision = ++authRevision;
  identityRevision++; resetOAuthConsent();
  bootstrapController?.abort(); bootstrapController = null;
  const button = el('login');
  button.disabled = true; tx(button, 'Opening Google…');
  tx(el('status'), 'Opening Google sign-in…');
  el('main').setAttribute('aria-busy', 'true');
  let navigating = false;
  try {
    const data = await api('/account/login');
    if (revision !== authRevision) return;
    if (typeof data?.authorization_url !== 'string' || !data.authorization_url.trim()) throw new Error('Could not start sign-in. Please try again.');
    location.assign(data.authorization_url);
    navigating = true;
  } catch (error) {
    if (revision === authRevision) tx(el('status'), error.message === 'Failed to fetch' ? 'Could not start sign-in. Check your connection and try again.' : error.message);
  } finally {
    if (revision === authRevision && !navigating) {
      loginPending = false; button.disabled = false; tx(button, 'Sign in with Google');
      if (!actionPending) el('main').removeAttribute('aria-busy');
    }
  }
}
function hasIdentity() { return Boolean(el('actor-id').value.trim() && el('consumer-id').value.trim()); }
function identityChanged() {
  inspectedCode = null; el('approve').disabled = true;
  resetOAuthConsent();
  tx(el('identity-note'), hasIdentity() ? 'This identity will be used for approval. Review the client or device again after changing identities.' : 'Choose both an assistant and a connection identity.');
  renderConnection();
  el('make-primary').disabled = !primarySelection || !identityActors.some(actor => actor.actor_id === el('actor-id').value && actor.kind === 'agent' && !actor.disabled);
}
function renderIdentityChoices(preferredActor = '', preferredConsumer = '') {
  const select = el('assistant-select');
  const assistantPlaceholder = new Option('', ''); tx(assistantPlaceholder, 'Choose an assistant');
  select.replaceChildren(assistantPlaceholder);
  for (const actor of identityActors.filter(actor => actor.kind === 'agent' && !actor.disabled)) select.append(new Option(actor.display_name, actor.actor_id));
  select.value = preferredActor;
  const consumers = el('consumer-select'); const consumerPlaceholder = new Option('', ''); tx(consumerPlaceholder, 'Choose a connection identity'); consumers.replaceChildren(consumerPlaceholder);
  const matches = identityConsumers.filter(consumer => consumer.actor_id === select.value && !consumer.disabled);
  for (const consumer of matches) consumers.append(new Option(consumer.consumer_id, consumer.consumer_id));
  consumers.value = matches.some(consumer => consumer.consumer_id === preferredConsumer) ? preferredConsumer : matches.length === 1 ? matches[0].consumer_id : '';
  el('actor-id').value = select.value; el('consumer-id').value = consumers.value;
  el('consumer-choice').hidden = Boolean(oauthRequest) && matches.length <= 1;
  identityChanged();
}
function chooseIdentity(actor, consumer) {
  renderIdentityChoices(actor, consumer);
  // Management also exposes existing technical/owner identities. Preserve those
  // stable IDs even when they are intentionally outside the assistant picker.
  if (el('actor-id').value !== actor) {
    el('actor-id').value = actor;
    const matches = identityConsumers.filter(item => item.actor_id === actor && !item.disabled);
    el('consumer-id').value = consumer || (matches.length === 1 ? matches[0].consumer_id : '');
    identityChanged();
  }
  el('assistants').scrollIntoView?.({block:'start'});
}
async function loadIdentityChoices(revision, preferredActor = el('actor-id').value, preferredConsumer = el('consumer-id').value) {
  async function all(kind) {
    const items = []; let cursor = null, state;
    do {
      state = await api('/account/identities/list', {kind,...(cursor ? {cursor} : {})});
      if (revision !== identityRevision || state.space_id !== currentSpace) return null;
      items.push(...state.items); cursor = state.has_more ? state.next_cursor : null;
    } while (cursor);
    return {items,state};
  }
  const [actors, consumers] = await Promise.all([all('actors'),all('consumers')]);
  if (!actors || !consumers || revision !== identityRevision) return;
  identityActors = actors.items; identityConsumers = consumers.items;
  primarySelection = {space_id:actors.state.space_id,expected_revision:actors.state.primary_revision};
  const primary = actors.state.primary_actor_id;
  identitiesLoaded = true;
  const enabled = identityActors.filter(actor => actor.kind === 'agent' && !actor.disabled);
  const preferred = enabled.some(actor => actor.actor_id === preferredActor) ? preferredActor : primary || (enabled.length === 1 ? enabled[0].actor_id : '');
  renderIdentityChoices(preferred, preferredConsumer);
  el('primary-clear').disabled = !primary;
  const primaryName = identityActors.find(actor => actor.actor_id === primary)?.display_name || primary;
  tx(el('primary-info'), primaryName ? 'Primary assistant: {name}' : 'Primary assistant: none', {name: primaryName});
  const needsAssistant = !identityActors.some(actor => actor.kind === 'agent' && !actor.disabled);
  el('assistant-create').open = needsAssistant;
  if (needsAssistant) tx(el('identity-note'), 'Create an assistant identity to connect your first client.');
}
el('assistant-select').onchange = () => renderIdentityChoices(el('assistant-select').value);
el('consumer-select').onchange = () => { el('consumer-id').value = el('consumer-select').value; identityChanged(); };
for (const id of ['actor-id','consumer-id']) el(id).oninput = identityChanged;
el('make-primary').onclick = () => run(async () => {
  if (!primarySelection || !el('actor-id').value) throw new Error('Load and choose an existing assistant first.');
  await api('/account/primary/set',{...primarySelection,actor_id:el('actor-id').value});
  await loadIdentityChoices(identityRevision);
  tx(el('status'), 'Primary assistant updated. Existing connection recipients and permissions are unchanged.');
});
async function refresh() {
  const previousSpace = currentSpace, preferredActor = el('actor-id').value, preferredConsumer = el('consumer-id').value;
  const revision = ++identityRevision, auth = authRevision; currentSpace = null; identitiesLoaded = false;
  bootstrapController?.abort();
  const controller = new AbortController(); bootstrapController = controller;
  identityActors = []; identityConsumers = [];
  el('actor-id').value = ''; el('consumer-id').value = '';
  renderIdentityChoices();
  resetManagement();
  resetOAuthConsent(); inspectedCode = null; el('approve').disabled = true;
  el('open-dashboard').hidden = true;
  tx(el('dashboard-selection'), 'Sign in and select a Space above before opening its dashboard.');
  let data;
  try { data = await api('/account/session', null, 'GET', controller.signal); }
  catch (error) {
    if (revision !== identityRevision || auth !== authRevision) return;
    if (error.status === 401) { showSignedOut(); return; }
    throw error;
  } finally { if (bootstrapController === controller) bootstrapController = null; }
  if (revision !== identityRevision || auth !== authRevision) return;
  csrf = data.csrf_token; accountSpaces = data.spaces;
  document.body.dataset.view = oauthRequest ? 'connect' : 'manage';
  el('connection-settings').open = !oauthRequest;
  el('login').hidden = true; el('logout').hidden = false;
  unifiedProduct = data.product === 'unified';
  el('keys-connections').hidden = !unifiedProduct;
  // Unified OAuth supports browser PKCE; retain device OAuth on legacy Waypost only.
  el('oauth-device-panel').hidden = unifiedProduct;
  const selected = data.spaces.find(space => space.space_id === data.selected_space_id);
  currentSpace = selected?.space_id || null; currentSpaceName = selected?.display_name || '';
  tx(el('status'), oauthRequest ? '' : selected ? 'Signed in · {name}' : 'Signed in. Select a Space or set up your personal Space to continue.', {name: selected?.display_name});
  el('space-setup').open = !data.spaces.length;
  el('open-dashboard').hidden = !selected;
  tx(el('open-dashboard'), selected ? 'Open dashboard: {name} ({id})' : 'Open selected Space dashboard', {name: selected?.display_name, id: selected?.space_id});
  tx(el('dashboard-selection'), selected ? 'Selected Space: {name} ({id})' : 'Select a Space above before opening its dashboard.', {name: selected?.display_name, id: selected?.space_id});
  el('spaces').replaceChildren();
  for (const space of data.spaces) {
    const li = document.createElement('li');
    const button = document.createElement('button');
    tx(button, space.space_id === data.selected_space_id ? 'Selected: {name} ({id})' : 'Select: {name} ({id})', {name: space.display_name, id: space.space_id});
    if (space.space_id === data.selected_space_id) button.setAttribute('aria-current', 'true');
    button.onclick = () => run(async () => {resetManagement(); resetOAuthConsent(); el('open-dashboard').hidden = true; tx(el('dashboard-selection'), 'Selecting Space…'); await api('/account/spaces/select', {space_id: space.space_id}); await refresh();});
    li.append(button); el('spaces').append(li);
  }
  if (selected) {
    try { await loadIdentityChoices(revision, previousSpace === currentSpace ? preferredActor : '', previousSpace === currentSpace ? preferredConsumer : ''); }
    catch (error) { tx(el('identity-note'), 'Identity list unavailable. Sign in again to load your existing assistants, or use technical identity details.'); }
  }
  renderConnection();
  if (oauthRequest && revision === identityRevision) await reviewOAuth();
}
async function run(fn) {
  if (actionPending) return;
  actionPending = true;
  const revision = authRevision;
  el('main').setAttribute('aria-busy', 'true');
  try { await fn(); } catch (error) { if (revision === authRevision) tx(el('status'), error.i18nSource || error.message); }
  finally { actionPending = false; if (!loginPending) el('main').removeAttribute('aria-busy'); }
}
el('login').onclick = startLogin;
el('logout').onclick = () => run(async () => {await api('/account/session', null, 'DELETE'); csrf=''; location.reload();});
el('refresh').onclick = () => run(refresh);
el('create').onclick = () => run(async () => {await api('/account/spaces/create',{display_name:el('space-name').value}); await refresh();});
for (const action of ['claim','unlink','recover']) el(action).onclick = () => run(async () => {
  const owner = el('owner-key').value; el('owner-key').value='';
  await api(`/account/spaces/${action}`,{space_id:el('space-id').value,owner_credential:owner}); await refresh();
});
el('agent-create').onclick=()=>run(async()=>{
  if (!el('agent-name').value.trim()) throw new Error('Enter a name for the assistant.');
  const name = el('agent-name').value.trim();
  if (!pendingIdentity || pendingIdentity.space !== currentSpace || pendingIdentity.name !== name) pendingIdentity = {space:currentSpace,name,request_id:crypto.randomUUID()};
  const result=await api('/account/agents/create',{request_id:pendingIdentity.request_id,display_name:name});
  pendingIdentity = null;
  el('actor-id').value=result.actor_id; el('consumer-id').value=result.consumer_id;
  try { await loadIdentityChoices(identityRevision, result.actor_id, result.consumer_id); }
  catch (_) { tx(el('identity-note'), 'Identity created. Its stable IDs are available in technical identity details; refresh to reload the list.'); }
  tx(el('status'), 'Assistant identity created and selected. Review your client request or device code to connect it.');
});
el('device-code').oninput=()=>{inspectedCode=null;el('approve').disabled=true;};
el('inspect').onclick=()=>run(async()=>{
  inspectedCode=null;el('approve').disabled=true;
  const code=el('device-code').value;
  const info=await api('/account/devices/inspect',{user_code:code});
  el('device-info').textContent=JSON.stringify(info,null,2);
  inspectedCode=code;el('approve').disabled=!hasIdentity();
});
el('approve').onclick=()=>run(async()=>{
  if(!inspectedCode || inspectedCode!==el('device-code').value) throw new Error('Inspect the device first.');
  if (!hasIdentity()) throw new Error('Choose an assistant and connection identity first.');
  await api('/account/devices/approve',{user_code:inspectedCode,actor_id:el('actor-id').value,consumer_id:el('consumer-id').value});
  inspectedCode=null;el('approve').disabled=true;tx(el('status'), 'Approved. Return to your device to finish pairing.');
});
el('devices').onclick=()=>run(async()=>{
  el('device-list').replaceChildren();let cursor=null;
  do {
    const page=await api('/account/devices/list',cursor ? {cursor} : {});
    for (const device of page.devices) {
      const li=document.createElement('li');
      const description = document.createElement('span');
      tx(description, '{name} ({id}) {state} ', {name: device.display_name, id: device.device_id, state: device.state});
      li.append(description);
      const button=document.createElement('button');tx(button, 'Revoke');
      button.onclick=()=>run(async()=>{await api('/account/devices/revoke',{device_id:device.device_id});button.disabled=true;});
      li.append(button);el('device-list').append(li);
    }
    cursor=page.has_more?page.next_cursor:null;
  } while(cursor);
});
el('oauth-inspect').onclick = () => run(reviewOAuth);
el('oauth-dismiss').onclick = clearOAuthHint;
for (const decision of ['approve','deny']) el('oauth-'+decision).onclick = () => run(async () => {
  if (!oauthPreview || !oauthRequest) throw new Error('Review the request first.');
  if (decision === 'approve' && !hasIdentity()) throw new Error('Choose an assistant and connection identity first.');
  const grants = decision === 'approve' && keyPermissions ? keyPermissions.validate() : {};
  const requestId = oauthRequest;
  const result = await oauthApi('/oauth/consent', {request_id:requestId,decision, ...(decision==='approve'?{actor_id:el('actor-id').value,consumer_id:el('consumer-id').value,...grants}:{})});
  resetOAuthConsent();
  // The server returns only the exact registered destination after binding checks.
  const target = new URL(result.redirect_uri);
  if (!['https:','http:'].includes(target.protocol)) throw new Error('Invalid authorization redirect.');
  clearOAuthHint(); location.assign(target.href);
});
el('oauth-device-code').oninput = () => {oauthRevision++; oauthDeviceCode=null; el('oauth-device-approve').disabled=true; el('oauth-device-deny').disabled=true;};
el('oauth-device-inspect').onclick = () => run(async () => {
  el('oauth-device-code').oninput();
  const code = el('oauth-device-code').value, revision = oauthRevision;
  const info = await oauthApi('/oauth/device_consent',{action:'inspect',user_code:code});
  if (code !== el('oauth-device-code').value || revision !== oauthRevision) return;
  oauthDeviceInfo = info;
  renderOAuthReview(el('oauth-device-info'), info);
  oauthDeviceCode=code; el('oauth-device-approve').disabled=!hasIdentity(); el('oauth-device-deny').disabled=false;
});
for (const action of ['approve','deny']) el('oauth-device-'+action).onclick = () => run(async () => {
  if (!oauthDeviceCode || oauthDeviceCode!==el('oauth-device-code').value) throw new Error('Inspect the OAuth device first.');
  if (action === 'approve' && !hasIdentity()) throw new Error('Choose an assistant and connection identity first.');
  const code=oauthDeviceCode; el('oauth-device-code').oninput();
  await oauthApi('/oauth/device_consent',{action,user_code:code,...(action==='approve'?{actor_id:el('actor-id').value,consumer_id:el('consumer-id').value}:{})});
  tx(el('status'), action==='approve'?'OAuth device approved. Return to your client.':'OAuth device denied.');
});

async function managementPage(kind, cursor=null, revision=managementRevision) {
  const identities=kind==='actors'||kind==='consumers';
  const endpoint=identities?'identities':kind;
  const body={...(identities?{kind}:{}),...(cursor?{cursor}:{})};
  const data=await api('/account/'+endpoint+'/list',body);
  if(revision!==managementRevision) return;
  if(identities) {
    primarySelection={space_id:data.space_id,expected_revision:data.primary_revision};
    tx(el('primary-info'), data.primary_actor_id ? 'Primary assistant: {name}' : 'Primary assistant: none', {name: data.primary_actor_id});
    el('primary-clear').disabled=false;
  }
  for(const item of data.items) {
    const li=document.createElement('li');
    const title=document.createElement('strong'); title.className='identity-title'; title.textContent=item.display_name || item.client_id || item.actor_id || item.token_id || item.family_id || item.consumer_id;
    const detail=document.createElement('span'); detail.className='identity-meta';
    tx(detail, (item.disabled ? 'Disabled' : item.revoked_at ? 'Revoked' : 'Active') + '{kind}', {kind: item.kind ? ' · '+item.kind : ''});
    const technical=document.createElement('details'), summary=document.createElement('summary'), metadata=document.createElement('pre'); tx(summary, 'Technical details'); metadata.textContent=JSON.stringify(item,null,2); technical.append(summary,metadata); li.append(title,detail,technical);
    const button=document.createElement('button');
    const id=kind==='connections'?item.token_id:kind==='oauth'?item.family_id:kind==='actors'?item.actor_id:item.consumer_id;
    tx(button, identities?'Disable identity':'Revoke access');
    button.disabled=Boolean(item.disabled||item.revoked_at!==null && item.revoked_at!==undefined);
    button.onclick=()=>run(async()=>{
      if(revision!==managementRevision) throw new Error('Refresh the selected Space before changing access.');
      button.disabled=true;
      try {
        await api('/account/'+endpoint+(identities?'/disable':'/revoke'),{space_id:data.space_id,...(identities?{kind,identity_id:id}:kind==='connections'?{token_id:id}:{family_id:id})});
        tx(el('status'), identities?'Identity disabled.':'Access revoked.');
      } catch(error) {button.disabled=false;throw error;}
    });
    li.append(button);
    if(identities) {
      const use=document.createElement('button');tx(use, 'Use for approval');use.disabled=item.disabled;
      use.onclick=()=>{if(revision!==managementRevision)return;chooseIdentity(item.actor_id,kind==='consumers'?item.consumer_id:null);};li.append(use);
    }
    if(kind==='actors'&&item.kind==='agent'&&!item.disabled) {
      const choose=document.createElement('button');tx(choose, 'Make primary');
      choose.onclick=()=>run(async()=>{if(revision!==managementRevision)throw new Error('Refresh Actors first.');await api('/account/primary/set',{space_id:data.space_id,expected_revision:data.primary_revision,actor_id:item.actor_id});resetManagement();await loadIdentityChoices(identityRevision);await managementPage('actors');});li.append(choose);
    }
    el('management-'+kind).append(li);
  }
  const more=el('more-'+kind);more.hidden=!data.has_more;
  more.onclick=()=>run(async()=>{more.disabled=true;try{await managementPage(kind,data.next_cursor,revision);}finally{more.disabled=false;}});
}
for(const kind of ['connections','oauth','actors','consumers']) el('manage-'+kind).onclick=()=>run(async()=>{el('management-'+kind).replaceChildren();el('more-'+kind).hidden=true;await managementPage(kind);});
el('primary-clear').onclick=()=>run(async()=>{if(!primarySelection)throw new Error('Load assistant identities first.');await api('/account/primary/set',{...primarySelection,actor_id:null});resetManagement();await loadIdentityChoices(identityRevision);});
el('manage-keys').onclick = () => run(async () => {
  const data = await api('/account/keys/grants', null, 'GET');
  const items = data.grants || [];
  el('management-keys').replaceChildren(...items.map(grant => {
    const li = document.createElement('li'), title = document.createElement('strong'), description = document.createElement('p'), button = document.createElement('button');
    title.textContent = grant.client_id;
    localize(description, () => {
      description.textContent = [grant.all_read ? t('Read all existing and future entries') : '', grant.all_write ? t('Change all existing and future entries') : '', grant.can_create ? t('Create entries') : '', ...(grant.entries || []).map(entry => t('{name}: {permissions}', {name: entry.name, permissions: [entry.read ? t('read') : '', entry.write ? t('change') : ''].filter(Boolean).join(t(' and '))}))].filter(Boolean).join('; ') || t('No keys permissions');
    });
    tx(button, 'Revoke keys access');
    button.onclick = () => run(async () => { await api('/account/keys/grants/revoke',{grant_id:grant.grant_id}); button.disabled = true; translatedRenders.delete(description); tx(description, 'Keys access revoked. Message access is preserved.'); });
    li.append(title,description,button); return li;
  }));
  if (!items.length) {const li=document.createElement('li');tx(li, 'No clients have keys access.');el('management-keys').append(li);}
});

function renderConnection() {
  const connecting = Boolean(oauthRequest);
  tx(el('account-title'), connecting ? 'Connect your assistant' : csrf ? 'Your workspace' : 'Welcome to Miles Works');
  tx(el('account-intro'), connecting ? csrf ? 'Review access before connecting your assistant.' : 'Sign in, review access, then return to your assistant.' : csrf ? 'Manage the people and devices that work with your assistants.' : 'Sign in to see your messages, keys and connections.');
  const ready = Boolean(currentSpace && hasIdentity());
  el('connect-setup').hidden = !connecting || !csrf || ready;
  el('connect-continue').disabled = !oauthInfo;
  let instruction;
  if (!accountSpaces.length) instruction = 'Continue to set up your personal workspace and an assistant. You will review permissions before connecting.';
  else if (!currentSpace && accountSpaces.length === 1) instruction = 'Continue with your existing workspace. No new workspace will be created.';
  else if (!currentSpace) instruction = 'Choose the workspace for this connection below.';
  else if (!identitiesLoaded) instruction = 'Your assistants could not be loaded. Retry before creating anything.';
  else if (!identityActors.some(a=>a.kind==='agent' && !a.disabled)) instruction = 'Continue to create an assistant for this connection. This does not grant access yet.';
  else instruction = 'Choose an existing assistant below. Reuse the same identity when reconnecting.';
  tx(el('connect-setup-note'),instruction);
  const actor=identityActors.find(a=>a.actor_id===el('actor-id').value);
  tx(el('connection-summary'), ready ? 'Connecting as {assistant} in {workspace}.' : 'Complete setup above, then review the permissions below.', {assistant:actor?.display_name || t('Selected assistant'),workspace:currentSpaceName});
  if (connecting && csrf && ((!currentSpace && accountSpaces.length > 1) || (currentSpace && identitiesLoaded && !ready && identityActors.some(a=>a.kind==='agent'&&!a.disabled)))) el('connection-settings').open=true;
}
el('connect-continue').onclick=()=>run(async()=>{
  if (!oauthRequest || !oauthInfo) throw new Error('Review the request first.');
  if (!currentSpace) {
    if (!accountSpaces.length) await api('/account/spaces/create',{display_name:'Personal Space'});
    else if (accountSpaces.length===1) await api('/account/spaces/select',{space_id:accountSpaces[0].space_id});
    else {el('connection-settings').open=true;el('overview').scrollIntoView?.({block:'start'});return;}
    await refresh();
  }
  if (!currentSpace || !identitiesLoaded) throw new Error('Your assistants could not be loaded. Retry before creating anything.');
  if (!hasIdentity()) {
    const enabled=identityActors.filter(a=>a.kind==='agent'&&!a.disabled);
    if (enabled.length) {el('connection-settings').open=true;el('assistant-select').focus();return;}
    const name=(oauthInfo?.client_name || 'Assistant').slice(0,200);
    if (!pendingIdentity || pendingIdentity.space!==currentSpace || pendingIdentity.name!==name) pendingIdentity={space:currentSpace,name,request_id:crypto.randomUUID()};
    const result=await api('/account/agents/create',{request_id:pendingIdentity.request_id,display_name:name});
    pendingIdentity=null;
    await loadIdentityChoices(identityRevision,result.actor_id,result.consumer_id);
  }
  await reviewOAuth();
});
window.addEventListener('miles-language-change',()=>{
  renderConnection();
  if (oauthInfo && oauthPreview) renderOAuthReview(el('oauth-info'),oauthInfo,true);
  if (oauthDeviceCode && oauthDeviceInfo) renderOAuthReview(el('oauth-device-info'), oauthDeviceInfo);
  for (const [node, render] of translatedRenders) {
    if (node.isConnected) render(); else translatedRenders.delete(node);
  }
  keyPermissions?.render();
});

run(refresh);
