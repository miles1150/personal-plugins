import {JSDOM} from '../examples/mcp-client/node_modules/jsdom/lib/api.js';
import {readFileSync} from 'node:fs';
import assert from 'node:assert/strict';
import test from 'node:test';
const wait=async()=>{await new Promise(r=>setTimeout(r,20));};
function fixture(url='https://example.test/account', hint=null, config={}){
 const dom=new JSDOM(readFileSync('dashboard/account.html','utf8'),{url,runScripts:'outside-only'});
 const calls=[];
 if(hint) dom.window.sessionStorage.setItem('waypost.oauth.request',hint);
 dom.window.fetch=async(path,options)=>{
  calls.push({path,options});
  if(path.startsWith('/oauth/')) return {ok:true,json:async()=>({request_id:'oauth_test',client_name:'<img onerror=alert(1)>',client_source:'dcr',redirect_uri:config.combined && options.method==='POST'?'https://example.test/account#approved':'http://127.0.0.1:8765/callback',scope:config.combined?'waypost keys:read keys:write':'waypost',selected_space_id:'s',approved:true,...(config.combined?{product:'unified',vault:{entries:[{name:'test-entry',fields:['TOKEN'],disabled:false}]}}:{})})};
  if(['/account/connections/list','/account/oauth/list','/account/identities/list'].includes(path)) {
   const items=path.includes('/connections/')?[{token_id:'legacy',actor_id:'actor',consumer_id:'consumer',credential_kind:'legacy',revoked_at:null}]:path.includes('/oauth/')?[{family_id:'family',client_id:'client',revoked_at:null}]:[{actor_id:'actor',consumer_id:'consumer',kind:'agent',display_name:'<img onerror=alert(1)>',disabled:false}];
   return {ok:true,json:async()=>({ok:true,data:{space_id:'s',items,has_more:false,primary_actor_id:null,primary_revision:3}})};
  }
  const data=path==='/account/session' ?{account_id:'a',csrf_token:'csrf',...(config.selected?{selected_space_id:'s'}:{}),...(config.combined?{product:'unified'}:{}),spaces:[{space_id:'s',display_name:'<img onerror=alert(1)>'}]}:
   path.endsWith('/inspect')?{display_name:'<script>untrusted</script>'}:
   path.endsWith('/list')?{devices:[{device_id:'d',display_name:'Device',state:'consumed'}],has_more:false}:{};
  return {ok:true,json:async()=>({ok:true,data})};
 };
 dom.window.eval(readFileSync('dashboard/key-permissions.js','utf8'));
 dom.window.eval(readFileSync('dashboard/account.js','utf8'));
 // Existing tests explicitly select stable IDs through the technical fields.
 dom.window.document.getElementById('actor-id').value='actor';
 dom.window.document.getElementById('consumer-id').value='consumer';
 return {dom,calls,doc:dom.window.document};
}
test('account refresh actually refetches session and renders names as text',async()=>{
 const {dom,calls,doc}=fixture();await wait();const n=calls.length;
 doc.getElementById('refresh').click();await wait();assert.equal(calls.length,n+1);assert.equal(calls.at(-1).path,'/account/session');
 assert.equal(doc.querySelectorAll('#spaces img').length,0);assert.match(doc.getElementById('spaces').textContent,/<img/);dom.window.close();
});
test('approval requires inspection and device code edits invalidate it',async()=>{
 const {dom,calls,doc}=fixture();await wait();const input=doc.getElementById('device-code');
 assert.equal(doc.getElementById('approve').disabled,true);input.value='1111';doc.getElementById('inspect').click();await wait();
 assert.equal(doc.getElementById('approve').disabled,false);assert.equal(doc.querySelectorAll('#device-info script').length,0);
 input.value='2222';input.dispatchEvent(new dom.window.Event('input'));assert.equal(doc.getElementById('approve').disabled,true);
 assert.equal(calls.at(-1).options.headers['X-Waypost-CSRF'],'csrf');dom.window.close();
});
test('device listing revokes only selected device without secrets',async()=>{
 const {dom,calls,doc}=fixture();await wait();doc.getElementById('devices').click();await wait();
 doc.querySelector('#device-list button').click();await wait();assert.equal(calls.at(-1).path,'/account/devices/revoke');
 assert.deepEqual(JSON.parse(calls.at(-1).options.body),{device_id:'d'});dom.window.close();
});

test('OAuth request survives login as a hint, renders safely and never automatically approves',async()=>{
 const {dom,calls,doc}=fixture('https://example.test/account','oauth_test');await wait();
 assert.equal(doc.getElementById('oauth-browser').hidden,false);
 assert.equal(calls.filter(c=>c.path.startsWith('/oauth/')).length,1);
 assert.equal(calls.at(-1).options.method,'GET');
 assert.equal(doc.querySelectorAll('#oauth-info img').length,0);
 assert.match(doc.getElementById('oauth-info').textContent,/<img/);
 assert.equal(doc.getElementById('oauth-approve').disabled,false);
 doc.getElementById('oauth-dismiss').click();assert.equal(dom.window.sessionStorage.getItem('waypost.oauth.request'),null);dom.window.close();
});
test('OAuth device approval uses inspected code and explicit identity without legacy pairing',async()=>{
 const {dom,calls,doc}=fixture();await wait();
 const input=doc.getElementById('oauth-device-code');input.value='1111-2222';
 assert.equal(doc.getElementById('oauth-device-approve').disabled,true);
 doc.getElementById('oauth-device-inspect').click();await wait();
 assert.equal(doc.getElementById('oauth-device-approve').disabled,false);
 input.value='3333';input.dispatchEvent(new dom.window.Event('input'));
 assert.equal(doc.getElementById('oauth-device-approve').disabled,true);
 doc.getElementById('oauth-device-inspect').click();await wait();
 doc.getElementById('actor-id').value='actor';doc.getElementById('consumer-id').value='consumer';
 doc.getElementById('oauth-device-approve').click();await wait();
 assert.equal(calls.at(-1).path,'/oauth/device_consent');
 assert.equal(calls.at(-1).options.headers['X-Waypost-CSRF'],'csrf');
 assert.deepEqual(JSON.parse(calls.at(-1).options.body),{action:'approve',user_code:'3333',actor_id:'actor',consumer_id:'consumer'});
 assert.equal(doc.getElementById('oauth-device-approve').disabled,true);dom.window.close();
});

test('starting Space selection immediately disables OAuth consent while request is pending',async()=>{
 const {dom,doc}=fixture('https://example.test/account','oauth_test');await wait();
 assert.equal(doc.getElementById('oauth-approve').disabled,false);
 dom.window.fetch=()=>new Promise(()=>{});
 doc.querySelector('#spaces button').click();
 assert.equal(doc.getElementById('oauth-approve').disabled,true);
 assert.equal(doc.getElementById('oauth-deny').disabled,true);
 dom.window.close();
});

test('management revokes stable connection and OAuth IDs bound to the listed Space',async()=>{
 const {dom,calls,doc}=fixture();await wait();
 for(const [kind,key,id] of [['connections','token_id','legacy'],['oauth','family_id','family']]) {
  doc.getElementById('manage-'+kind).click();await wait();
  doc.querySelector('#management-'+kind+' button').click();await wait();
  assert.equal(calls.at(-1).path,'/account/'+kind+'/revoke');
  assert.deepEqual(JSON.parse(calls.at(-1).options.body),{space_id:'s',[key]:id});
  assert.equal(calls.at(-1).options.headers['X-Waypost-CSRF'],'csrf');
  assert.equal(doc.querySelector('#management-'+kind+' button').disabled,true);
 }
 dom.window.close();
});
test('management identity names are text and primary uses revision rather than owner defaults',async()=>{
 const {dom,calls,doc}=fixture();await wait();doc.getElementById('manage-actors').click();await wait();
 assert.equal(doc.querySelectorAll('#management-actors img').length,0);
 assert.match(doc.getElementById('management-actors').textContent,/<img/);
 const button=[...doc.querySelectorAll('#management-actors button')].find(b=>b.textContent==='Make primary');button.click();await wait();
 const action=calls.find(c=>c.path==='/account/primary/set');
 assert.deepEqual(JSON.parse(action.options.body),{space_id:'s',expected_revision:3,actor_id:'actor'});dom.window.close();
});
test('Space selection clears management controls and stale detached actions cannot mutate',async()=>{
 const {dom,calls,doc}=fixture();await wait();doc.getElementById('manage-connections').click();await wait();
 const stale=doc.querySelector('#management-connections button');
 doc.querySelector('#spaces button').click();await wait();
 assert.equal(doc.querySelector('#management-connections button'),null);
 const count=calls.length;stale.click();await wait();assert.equal(calls.length,count);dom.window.close();
});

test('dashboard entry requires an explicitly selected owned Space and uses account mode',async()=>{
  const {dom,doc}=fixture();
  try {
    await wait();
    assert.equal(doc.getElementById('open-dashboard').hidden,true);
    assert.match(doc.getElementById('dashboard-selection').textContent,/Select a Space/);
    let selects=0;
    dom.window.fetch=async(path,options)=>{
      if(path==='/account/spaces/select') {selects++; assert.equal(JSON.parse(options.body).space_id,'s');}
      return {ok:true,json:async()=>({ok:true,data:path==='/account/session'?{account_id:'a',csrf_token:'csrf',selected_space_id:'s',spaces:[{space_id:'s',display_name:'Chosen Space'}]}:{}})};
    };
    doc.querySelector('#spaces button').click();await wait();
    assert.equal(selects,1);
    assert.equal(doc.getElementById('open-dashboard').hidden,false);
    assert.equal(doc.getElementById('open-dashboard').getAttribute('href'),'/dashboard?auth=account');
    assert.match(doc.getElementById('open-dashboard').textContent,/Chosen Space/);
  } finally {dom.window.close();}
});

test('OAuth consent distinguishes client metadata from actual callback and warns about loopback',async()=>{
 const {dom,doc}=fixture('https://example.test/account','oauth_test');
 try {
  await wait();
  const text=doc.getElementById('oauth-info').textContent;
  assert.match(text,/Client name \(unverified\): <img/);
  assert.match(text,/Dynamic registration/);
  assert.equal(doc.getElementById('oauth-review').textContent,'After allowing access, return to <img onerror=alert(1)>.');
  assert.equal(doc.getElementById('oauth-details').open,false);
  assert.match(text,/Actual redirect URI: http:\/\/127\.0\.0\.1:8765\/callback/);
  assert.match(text,/Loopback callback/);
  assert.equal(doc.querySelector('#oauth-info img'),null);
  for(const [source,label] of [['cimd','URL metadata document'],['legacy','Migrated / administrator-configured client']]) {
   dom.window.fetch=async()=>({ok:true,json:async()=>({client_name:'Trusted-looking <script>client</script>',client_id:'https://metadata.example/client.json',client_source:source,redirect_uri:'https://actual-callback.example/return',selected_space_id:'s'})});
   doc.getElementById('oauth-inspect').click();await wait();
   const review=doc.getElementById('oauth-info').textContent;
   assert(review.includes(label));assert.equal(doc.getElementById('oauth-review').textContent,'After allowing access, return to Trusted-looking <script>client</script>.');
   assert(review.includes('Actual redirect URI: https://actual-callback.example/return'));
   assert(!review.includes('Loopback callback'));assert.equal(doc.querySelector('#oauth-info script'),null);
  }
 }finally{dom.window.close();}
});

test('OAuth consent cannot approve when actual callback is missing or invalid',async()=>{
 const {dom,doc}=fixture('https://example.test/account','oauth_test');
 try {
  await wait();
  for(const redirect_uri of [undefined,'javascript:alert(1)','https://user:password@callback.example/']) {
   dom.window.fetch=async()=>({ok:true,json:async()=>({client_name:'Example',client_source:'dcr',redirect_uri})});
   doc.getElementById('oauth-inspect').click();await wait();
   assert.equal(doc.getElementById('oauth-approve').disabled,true);
   assert.match(doc.getElementById('status').textContent,/redirect/);
  }
 }finally{dom.window.close();}
});

test('assistant picker loads stable IDs and changing it invalidates a reviewed request',async()=>{
 const {dom,doc,calls}=fixture('https://example.test/account','oauth_test',{selected:true});
 try{
  await wait();
  assert.equal(doc.querySelectorAll('#assistant-select option').length,2);
  assert.equal(doc.querySelector('#assistant-select img'),null);
  assert.equal(calls.filter(call=>call.path==='/account/agents/create').length,0);
  const select=doc.getElementById('assistant-select');select.value='actor';select.dispatchEvent(new dom.window.Event('change'));
  assert.equal(doc.getElementById('actor-id').value,'actor');
  assert.equal(doc.getElementById('consumer-id').value,'consumer');
  assert.equal(doc.getElementById('oauth-approve').disabled,true);
  doc.getElementById('oauth-inspect').click();await wait();
  assert.equal(doc.getElementById('oauth-approve').disabled,false);
 }finally{dom.window.close();}
});

test('combined consent shows an explicit broad preset and submits exact key grant flags only after approval',async()=>{
 const {dom,doc,calls}=fixture('https://example.test/account','oauth_test',{selected:true,combined:true});
 try{
  await wait();
  const select=doc.getElementById('assistant-select');select.value='actor';select.dispatchEvent(new dom.window.Event('change'));
  doc.getElementById('oauth-inspect').click();await wait();
  assert.equal(doc.getElementById('key-access').hidden,false);
  assert.equal(doc.getElementById('keys-connections').hidden,false);
  assert.equal(doc.getElementById('oauth-device-panel').hidden,true);
  doc.getElementById('preset-trusted').click();
  assert.equal(doc.getElementById('key-advanced').open,false);
  assert.equal(doc.getElementById('permission-summary').children.length,3);
  doc.getElementById('oauth-approve').click();await wait();
  assert.equal(calls.filter(call=>call.path==='/oauth/consent'&&call.options.method==='POST').length,0);
  assert.match(doc.getElementById('status').textContent,/Confirm access to all existing and future/);
  doc.getElementById('broad-ack').checked=true;doc.getElementById('oauth-approve').click();await wait();
  const request=calls.find(call=>call.path==='/oauth/consent'&&call.options.method==='POST');
  assert.deepEqual(JSON.parse(request.options.body),{request_id:'oauth_test',decision:'approve',actor_id:'actor',consumer_id:'consumer',all_read:true,all_write:true,can_create:true,entries:[]});
 }finally{dom.window.close();}
});

function authPage(fetch, timerHooks) {
 const dom=new JSDOM(readFileSync('dashboard/account.html','utf8'),{url:'https://example.test/account',runScripts:'outside-only'});
 dom.window.fetch=fetch;
 timerHooks?.(dom.window);
 dom.window.eval(readFileSync('dashboard/key-permissions.js','utf8'));
 dom.window.eval(readFileSync('dashboard/account.js','utf8'));
 return dom;
}
const missingSession=()=>({ok:false,status:401,json:async()=>({ok:false,error:{code:'UNAUTHENTICATED',message:'A valid credential is required.'}})});

test('missing and expired account sessions restore normal sign-in without exposing credential errors',async()=>{
 for(const initiallySignedIn of [false,true]) {
  let count=0;
  const dom=authPage(async()=>initiallySignedIn && ++count===1?{ok:true,status:200,json:async()=>({ok:true,data:{csrf_token:'csrf',spaces:[]}})}:missingSession());
  try {
   await wait();
   if(initiallySignedIn) {assert.equal(dom.window.document.getElementById('login').hidden,true);dom.window.document.getElementById('refresh').click();await wait();}
   const d=dom.window.document;
   assert.equal(d.getElementById('login').hidden,false);assert.equal(d.getElementById('login').disabled,false);
   assert.equal(d.getElementById('logout').hidden,true);
   assert.equal(d.getElementById('status').textContent,'Sign in with Google to continue.');
  }finally{dom.window.close();}
 }
});

for(const lateResult of ['401','success','error']) test(`sign-in starts during bootstrap and ignores late ${lateResult}`,async()=>{
 const calls=[];let finishBootstrap,finishLogin;
 const dom=authPage((path)=>{
  calls.push(path);
  if(path==='/account/session')return new Promise((resolve,reject)=>{finishBootstrap=()=>lateResult==='error'?reject(new Error('stale network error')):resolve(lateResult==='401'?missingSession():{ok:true,status:200,json:async()=>({ok:true,data:{csrf_token:'old',spaces:[]}})});});
  return new Promise(resolve=>{finishLogin=resolve;});
 });
 try{
  const d=dom.window.document;
  d.getElementById('login').click();d.getElementById('login').click();
  assert.equal(calls.filter(path=>path==='/account/login').length,1);
  assert.equal(d.getElementById('login').disabled,true);
  finishBootstrap();await wait();
  assert.equal(d.getElementById('status').textContent,'Opening Google sign-in…');
  assert.equal(d.getElementById('main').getAttribute('aria-busy'),'true');
  finishLogin({ok:true,status:200,json:async()=>({ok:true,data:{authorization_url:'https://example.test/account#google'}})});await wait();
  assert.equal(dom.window.location.hash,'#google');
  assert(!d.getElementById('status').textContent.includes('credential'));
 }finally{dom.window.close();}
});

for(const phase of ['headers','body']) test(`login deadline covers stalled ${phase} and permits retry`,async()=>{
 const deadlines=new Map();let nextTimer=0,attempts=0,signal;
 const dom=authPage(async(path,options)=>{
  if(path==='/account/session')return missingSession();
  attempts++;
  if(attempts>1)return {ok:true,status:200,json:async()=>({ok:true,data:{authorization_url:'https://example.test/account#retried'}})};
  signal=options.signal;
  const stalled=()=>new Promise((resolve,reject)=>options.signal.addEventListener('abort',()=>reject(new Error('aborted')),{once:true}));
  return phase==='headers'?stalled():{ok:true,status:200,json:stalled};
 },w=>{w.setTimeout=(callback,ms)=>{assert.equal(ms,15000);deadlines.set(++nextTimer,callback);return nextTimer;};w.clearTimeout=id=>deadlines.delete(id);});
 try{
  await wait();const d=dom.window.document;d.getElementById('login').click();await wait();
  assert.equal(deadlines.size,1);[...deadlines.values()][0]();await wait();
  assert.equal(signal.aborted,true);
  assert.match(d.getElementById('status').textContent,/timed out.*try again/);
  assert.equal(d.getElementById('login').disabled,false);assert.equal(d.getElementById('main').hasAttribute('aria-busy'),false);
  d.getElementById('login').click();await wait();
  assert.equal(attempts,2);assert.equal(dom.window.location.hash,'#retried');
 }finally{dom.window.close();}
});

test('malformed login success never navigates and restores retry controls',async()=>{
 for(const authorization_url of [undefined,' ',42]) {
  let attempts=0;
  const dom=authPage(async(path)=>path==='/account/session'?missingSession():{ok:true,status:200,json:async()=>({ok:true,data:++attempts===1?{authorization_url}:{authorization_url:'https://example.test/account#recovered'}})});
  try{
   await wait();const d=dom.window.document;d.getElementById('login').click();await wait();
   assert.equal(dom.window.location.hash,'');assert.match(d.getElementById('status').textContent,/Could not start sign-in/);assert.equal(d.getElementById('login').disabled,false);
   d.getElementById('login').click();await wait();assert.equal(dom.window.location.hash,'#recovered');
  }finally{dom.window.close();}
 }
});
