import {JSDOM} from '../examples/mcp-client/node_modules/jsdom/lib/api.js';
import {readFileSync} from 'node:fs';
import assert from 'node:assert/strict';
import test from 'node:test';

const wait = () => new Promise(resolve => setTimeout(resolve, 30));
const dashboardFile = name => readFileSync(new URL('../dashboard/' + name, import.meta.url), 'utf8');
const space = {space_id: 'space-existing', display_name: 'Sign in with Google'};
const assistant = {actor_id: 'actor-existing', kind: 'agent', display_name: 'Sign out', disabled: false};
const consumer = {actor_id: assistant.actor_id, consumer_id: 'consumer-existing', disabled: false};

function fixture(config = {}) {
  const state = {
    signedIn: config.signedIn ?? true,
    spaces: structuredClone(config.spaces ?? [space]),
    selected: config.selected === undefined ? space.space_id : config.selected,
    actors: structuredClone(config.actors ?? [assistant]),
    consumers: structuredClone(config.consumers ?? [consumer]),
    oauthError: config.oauthError ?? null,
  };
  const url = 'https://example.test/account' + (config.connect === false ? '' : '?oauth_request=oauth_test');
  const dom = new JSDOM(dashboardFile('account.html'), {url, runScripts: 'outside-only'});
  const {window} = dom, doc = window.document, calls = [];
  Object.defineProperty(window.navigator, 'language', {value: config.language ?? 'zh-CN'});
  if (config.savedLanguage) window.localStorage.setItem('miles-works.language', config.savedLanguage);
  const reply = data => ({ok: true, status: 200, json: async () => ({ok: true, data})});
  window.fetch = async (path, options = {}) => {
    const body = options.body ? JSON.parse(options.body) : null;
    calls.push({path, method: options.method ?? 'GET', body});
    if (path === '/account/session') {
      if (!state.signedIn) return {ok: false, status: 401, json: async () => ({ok: false, error: {message: 'A valid credential is required.'}})};
      return reply({csrf_token: 'csrf-test', product: 'unified', selected_space_id: state.selected, spaces: state.spaces});
    }
    if (path === '/account/identities/list') return reply({
      space_id: state.selected,
      items: body.kind === 'actors' ? state.actors : state.consumers,
      primary_actor_id: config.primary ?? null, primary_revision: 1, has_more: false,
    });
    if (path.startsWith('/oauth/consent')) {
      if (state.oauthError) return {ok: false, status: 400, json: async () => ({error: state.oauthError})};
      return {ok: true, status: 200, json: async () => ({
        client_name: 'Refresh <img src=x onerror=alert(1)>', client_id: 'test-client', client_source: 'dcr',
        redirect_uri: 'http://127.0.0.1:8765/callback', selected_space_id: state.selected,
        scope: 'waypost keys:read keys:write', product: 'unified',
        vault: {entries: [{name: 'Read', purpose: 'Change', fields: ['TOKEN'], disabled: false}]},
      })};
    }
    if (path === '/account/spaces/create') {
      const created = {space_id: 'space-created', display_name: body.display_name};
      state.spaces.push(created); state.selected = created.space_id;
      return reply(created);
    }
    if (path === '/account/spaces/select') { state.selected = body.space_id; return reply({}); }
    if (path === '/account/agents/create') {
      const created = {actor_id: 'actor-created', consumer_id: 'consumer-created'};
      state.actors.push({actor_id: created.actor_id, kind: 'agent', display_name: body.display_name, disabled: false});
      state.consumers.push({...created, disabled: false});
      return reply(created);
    }
    throw new Error('Unexpected fixture request: ' + path);
  };
  for (const file of ['account-i18n.js', 'key-permissions.js', 'account.js']) window.eval(dashboardFile(file));
  return {dom, doc, window, calls, state};
}

const writes = calls => calls.filter(call => ['/account/spaces/create', '/account/spaces/select', '/account/agents/create'].includes(call.path) || call.path.startsWith('/oauth/consent') && call.method === 'POST');
const requests = (calls, path) => calls.filter(call => call.path === path);
function chooseLanguage(window, doc, language) {
  const select = doc.getElementById('account-language');
  select.value = language;
  select.dispatchEvent(new window.Event('change', {bubbles: true}));
}

test('signed-out Chinese account opens the login view without creating or granting anything', async () => {
  const {dom, doc, calls} = fixture({signedIn: false, connect: false});
  try {
    // The synchronous dynamic heading must survive the later static binding pass.
    assert.equal(doc.getElementById('account-title').textContent, '欢迎使用 Miles Works');
    await wait();
    assert.equal(doc.documentElement.lang, 'zh-CN');
    assert.equal(doc.body.dataset.view, 'login');
    assert.equal(doc.getElementById('account-title').textContent, '欢迎使用 Miles Works');
    assert.equal(doc.getElementById('account-intro').textContent, '登录以查看你的消息、密钥和连接。');
    assert.equal(doc.getElementById('login').textContent, '使用 Google 登录');
    assert.equal(doc.getElementById('login').hidden, false);
    assert.equal(doc.getElementById('logout').hidden, true);
    assert.equal(doc.getElementById('status').textContent, '请使用 Google 登录以继续。');
    assert.deepEqual(calls.map(call => call.path), ['/account/session']);
    assert.equal(writes(calls).length, 0);
  } finally { dom.window.close(); }
});

test('an existing sole assistant is selected for review without creating an identity or approving OAuth', async () => {
  const {dom, doc, calls} = fixture();
  try {
    assert.equal(doc.getElementById('account-title').textContent, '连接你的助手');
    await wait();
    assert.equal(doc.body.dataset.view, 'connect');
    assert.equal(doc.getElementById('account-title').textContent, '连接你的助手');
    assert.equal(doc.getElementById('account-intro').textContent, '连接助手前，请确认访问权限。');
    assert.equal(doc.getElementById('actor-id').value, assistant.actor_id);
    assert.equal(doc.getElementById('consumer-id').value, consumer.consumer_id);
    assert.equal(doc.getElementById('connect-setup').hidden, true);
    assert.equal(doc.getElementById('connection-settings').open, false);
    assert.equal(doc.getElementById('oauth-approve').disabled, false);
    assert.equal(doc.getElementById('broad-ack').checked, false);
    for (const id of ['all-read', 'all-write', 'can-create']) assert.equal(doc.getElementById(id).checked, true);
    assert.equal(doc.getElementById('connection-summary').textContent, '以 Sign out 的身份连接到 Sign in with Google。');
    assert.equal(doc.getElementById('status').textContent, '');
    assert.match(doc.getElementById('oauth-info').textContent, /客户端名称（未经验证）：Refresh <img/);
    assert.equal(doc.querySelector('#oauth-info img'), null);
    assert.equal(writes(calls).length, 0);
    assert.equal(calls.filter(call => call.path.startsWith('/oauth/consent')).length, 1);
  } finally { dom.window.close(); }
});

test('Chinese-English-Chinese switching preserves custom access, acknowledgement and selected identity without requests', async () => {
  const {dom, doc, window, calls} = fixture();
  try {
    await wait();
    doc.getElementById('all-write').checked = false;
    doc.getElementById('all-write').dispatchEvent(new window.Event('change'));
    const entryRead = doc.querySelector('#entries .read');
    entryRead.checked = true; entryRead.dispatchEvent(new window.Event('change'));
    doc.getElementById('broad-ack').checked = true;
    const snapshot = () => ({
      actor: doc.getElementById('actor-id').value, consumer: doc.getElementById('consumer-id').value,
      checks: [...doc.querySelectorAll('#key-access input')].map(input => [input.id || input.className, input.checked, input.disabled]),
      preset: doc.getElementById('preset-custom').getAttribute('aria-pressed'),
      approveDisabled: doc.getElementById('oauth-approve').disabled,
    });
    const before = snapshot(), count = calls.length;
    chooseLanguage(window, doc, 'en');
    assert.equal(doc.documentElement.lang, 'en');
    assert.equal(doc.getElementById('account-title').textContent, 'Connect your assistant');
    assert.equal(doc.getElementById('assistant-select').options[0].textContent, 'Choose an assistant');
    assert.match(doc.getElementById('permission-summary').textContent, /Read: read every field/);
    assert.equal(doc.getElementById('oauth-review').textContent, 'After allowing access, return to Refresh <img src=x onerror=alert(1)>.');
    assert.match(doc.getElementById('oauth-info').textContent, /Actual redirect URI: http:\/\/127\.0\.0\.1:8765\/callback/);
    assert.match(doc.getElementById('oauth-info').textContent, /Loopback callback/);
    assert.deepEqual(snapshot(), before);
    chooseLanguage(window, doc, 'zh');
    assert.equal(doc.getElementById('assistant-select').options[0].textContent, '选择助手');
    assert.match(doc.getElementById('permission-summary').textContent, /Read：读取所有字段/);
    assert.equal(doc.getElementById('account-title').textContent, '连接你的助手');
    assert.equal(doc.querySelector('#entries .read'), entryRead);
    assert.deepEqual(snapshot(), before);
    assert.equal(calls.length, count);
    assert.equal(window.localStorage.getItem('miles-works.language'), 'zh');
    assert.equal(doc.querySelector('#oauth-review img'), null);
  } finally { dom.window.close(); }
});

test('first-time Continue creates one personal Space and one assistant, then stops before OAuth approval', async () => {
  const {dom, doc, calls} = fixture({spaces: [], selected: null, actors: [], consumers: []});
  try {
    await wait();
    assert.equal(writes(calls).length, 0);
    assert.equal(doc.getElementById('connect-setup').hidden, false);
    assert.equal(doc.getElementById('connect-continue').disabled, false);
    doc.getElementById('connect-continue').click();
    doc.getElementById('connect-continue').click();
    await wait();
    assert.equal(requests(calls, '/account/spaces/create').length, 1);
    assert.deepEqual(requests(calls, '/account/spaces/create')[0].body, {display_name: 'Personal Space'});
    assert.equal(requests(calls, '/account/agents/create').length, 1);
    assert.equal(requests(calls, '/account/spaces/select').length, 0);
    assert.equal(doc.getElementById('actor-id').value, 'actor-created');
    assert.equal(doc.getElementById('consumer-id').value, 'consumer-created');
    assert.equal(doc.getElementById('oauth-approve').disabled, false);
    assert.equal(doc.getElementById('broad-ack').checked, false);
    assert.equal(writes(calls).filter(call => call.path.startsWith('/oauth/consent')).length, 0);
  } finally { dom.window.close(); }
});

test('Continue selects an existing unselected sole Space without creating another Space or assistant', async () => {
  const {dom, doc, calls} = fixture({selected: null});
  try {
    await wait();
    assert.equal(doc.getElementById('connect-setup').hidden, false);
    assert.match(doc.getElementById('connect-setup-note').textContent, /已有工作区/);
    doc.getElementById('connect-continue').click();
    await wait();
    assert.equal(requests(calls, '/account/spaces/select').length, 1);
    assert.deepEqual(requests(calls, '/account/spaces/select')[0].body, {space_id: space.space_id});
    assert.equal(requests(calls, '/account/spaces/create').length, 0);
    assert.equal(requests(calls, '/account/agents/create').length, 0);
    assert.equal(doc.getElementById('actor-id').value, assistant.actor_id);
    assert.equal(doc.getElementById('oauth-approve').disabled, false);
    assert.equal(writes(calls).length, 1);
  } finally { dom.window.close(); }
});

test('multiple existing assistants require an explicit choice rather than selecting an arbitrary identity', async () => {
  const other = {...assistant, actor_id: 'actor-other', display_name: 'Other assistant'};
  const {dom, doc, calls} = fixture({actors: [assistant, other], consumers: [consumer, {...consumer, actor_id: other.actor_id, consumer_id: 'consumer-other'}]});
  try {
    await wait();
    assert.equal(doc.getElementById('assistant-select').value, '');
    assert.equal(doc.getElementById('actor-id').value, '');
    assert.equal(doc.getElementById('consumer-id').value, '');
    assert.equal(doc.getElementById('oauth-approve').disabled, true);
    assert.equal(doc.getElementById('connection-settings').open, true);
    doc.getElementById('connect-continue').click();
    await wait();
    assert.equal(doc.getElementById('actor-id').value, '');
    assert.equal(doc.activeElement.id, 'assistant-select');
    assert.equal(writes(calls).length, 0);
  } finally { dom.window.close(); }
});

test('invalid OAuth requests explain same-browser restart in Chinese and leave review available for retry', async () => {
  const {dom, doc, window, calls} = fixture({oauthError: 'invalid_request'});
  try {
    await wait();
    assert.match(doc.getElementById('status').textContent, /已过期或属于另一个浏览器/);
    assert.match(doc.getElementById('status').textContent, /重新发起连接，并在同一浏览器中完成/);
    assert.equal(doc.getElementById('oauth-inspect').hidden, false);
    assert.equal(doc.getElementById('oauth-approve').disabled, true);
    assert.equal(doc.getElementById('oauth-deny').disabled, true);
    chooseLanguage(window, doc, 'en');
    assert.match(doc.getElementById('status').textContent, /Restart Connect in your assistant and finish in the same browser/);
    const before = calls.length;
    doc.getElementById('oauth-inspect').click();
    await wait();
    assert.equal(calls.length, before + 1);
    assert.equal(calls.at(-1).method, 'GET');
    assert.equal(doc.getElementById('oauth-inspect').hidden, false);
    assert.equal(writes(calls).length, 0);
  } finally { dom.window.close(); }
});

test('saved language overrides browser preference when the account loads', async () => {
  const {dom, doc, window} = fixture({savedLanguage: 'en', signedIn: false, connect: false});
  try {
    await wait();
    assert.equal(doc.documentElement.lang, 'en');
    assert.equal(doc.getElementById('account-language').value, 'en');
    assert.equal(doc.getElementById('account-title').textContent, 'Welcome to Miles Works');
    chooseLanguage(window, doc, 'zh');
    assert.equal(doc.getElementById('account-title').textContent, '欢迎使用 Miles Works');
    assert.equal(window.localStorage.getItem('miles-works.language'), 'zh');
  } finally { dom.window.close(); }
});
