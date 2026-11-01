// Acceptance of the actual compiled unified Worker in isolated local D1/R2.
// Browser OAuth and Google HTTPS responses are synthetic. No external network,
// real credentials, production state, or live provider verification is used.
import assert from 'node:assert/strict';
import {readFile,writeFile,mkdtemp,rm} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import {join} from 'node:path';
import {createPrivateKey,createPublicKey,createHash,createDecipheriv,randomBytes,sign} from 'node:crypto';
import {execFileSync,spawn} from 'node:child_process';
import {createServer} from 'node:http';
import {fileURLToPath} from 'node:url';
import {Miniflare,Response,Log,LogLevel} from '../examples/mcp-client/node_modules/miniflare/dist/src/index.js';
import {JSDOM,VirtualConsole} from '../examples/mcp-client/node_modules/jsdom/lib/api.js';

const root=fileURLToPath(new URL('../',import.meta.url));
const portProbe=createServer();await new Promise(resolve=>portProbe.listen(0,'127.0.0.1',resolve));
const origin='http://127.0.0.1:'+portProbe.address().port,resource=origin+'/mcp';
await new Promise(resolve=>portProbe.close(resolve));
const issuer='https://accounts.google.com',subject='synthetic-unified-owner';
const tokenUrl='https://oauth2.googleapis.com/token',jwksUrl='https://www.googleapis.com/oauth2/v3/certs';
// Explicit legacy scope preserves coverage of pre-v2 connections.
const redirect='http://127.0.0.1:8765/callback',scope='waypost keys:read keys:write';
const privateKey=createPrivateKey(await readFile(new URL('fixtures/oidc-test-private.pem',import.meta.url)));
const jwk={...createPublicKey(privateKey).export({format:'jwk'}),kid:'unified-fixture',alg:'RS256',use:'sig'};
const messageKey=randomBytes(32),vaultKey=randomBytes(32);
const secret=()=>randomBytes(32).toString('base64url');
const encode=value=>Buffer.from(JSON.stringify(value)).toString('base64url');
const digest=value=>createHash('sha256').update(value).digest();
// Parse the real migrations with SQLite, including multiline triggers.
const statements=JSON.parse(execFileSync('python3',['-c',`
import json,pathlib,sqlite3,sys
out={}
for path in sorted(pathlib.Path(sys.argv[1]).glob('*.sql')):
 out[path.name]=[]
 pending=''
 for line in path.read_text().splitlines(True):
  pending+=line
  if sqlite3.complete_statement(pending):out[path.name].append(pending);pending=''
 if pending.strip() and not all(not l.strip() or l.lstrip().startswith('--') for l in pending.splitlines()):raise ValueError('Incomplete migration')
print(json.dumps(out))
`,root+'migrations'],{encoding:'utf8'}));

let phase='initialization',outbound=0,tokenCalls=0,jwksCalls=0,catalogCalls=0,rpcId=0,receiverServer,releaseReceiver,gateway,bridgeTemp;
const issued=new Map();
const mf=new Miniflare({
  modules:true,scriptPath:root+'build/index.js',modulesRules:[{type:'CompiledWasm',include:['**/*.wasm'],fallthrough:true}],
  compatibilityDate:'2026-07-17',log:new Log(LogLevel.NONE),
  d1Databases:{DB:'unified-control-test',KEYS_DB:'unified-vault-test'},r2Buckets:['OBJECTS'],
  bindings:{PUBLIC_ORIGIN:origin,WAYPOST_PRODUCT:'unified',WAYPOST_OPERATOR_MODE:'disabled',WAYPOST_ONBOARDING_MODE:'accounts',
    WAYPOST_CSRF_SECRET:secret(),WAYPOST_CURSOR_SECRET:secret(),WAYPOST_ACCOUNT_SIGNUP_LIMIT:'0',WORKS_OWNER_SUBJECT:subject,
    WAYPOST_ENCRYPTION_KEYS:JSON.stringify({active:'message',keys:{message:messageKey.toString('hex')}}),
    KEYS_DATA_KEYS:JSON.stringify({active:'vault',keys:{vault:vaultKey.toString('hex')}}),
    WAYPOST_OAUTH_CLIENT_DISCOVERY:JSON.stringify({dynamic_registration:true}),
    WAYPOST_OIDC_CONFIG:JSON.stringify({issuer,client_id:'synthetic-unified',authorization_endpoint:issuer+'/o/oauth2/v2/auth',token_endpoint:tokenUrl,google_jwks:true,keys:[]})},
  outboundService:async request=>{
    if(request.url===jwksUrl&&request.method==='GET') {
      jwksCalls++;
      return new Response(JSON.stringify({keys:[jwk]}),{headers:{'content-type':'application/json','cache-control':'public, max-age=3600'}});
    }
    if(request.url===tokenUrl&&request.method==='POST') {
      tokenCalls++;
      const fields=new URLSearchParams(await request.text());
      assert.equal(fields.get('client_id'),'synthetic-unified');
      assert.equal(fields.get('redirect_uri'),origin+'/account/callback');
      assert(fields.get('code_verifier'));
      const id_token=issued.get(fields.get('code'));assert(id_token);
      return new Response(JSON.stringify({id_token}),{headers:{'content-type':'application/json'}});
    }
    outbound++;throw new Error('Unexpected outbound request blocked');
  },
});

async function request(path,{method='GET',body,headers={},form=false}={}) {
  const r=await mf.dispatchFetch(origin+path,{method,redirect:'manual',headers:{'content-type':form?'application/x-www-form-urlencoded':'application/json',...headers},...(body===undefined?{}:{body:form?new URLSearchParams(body):JSON.stringify(body)})});
  const raw=await r.text();
  return {status:r.status,headers:r.headers,value:raw?JSON.parse(raw):null};
}
async function browserWrite(path,body,headers) {
  const r=await request(path,{method:'POST',headers,body});
  assert.equal(r.status,200);assert.equal(r.value.ok,true);return r.value.data;
}
async function rpc(access,method,params={}) {
  if(method==='tools/list')catalogCalls++;
  const r=await request('/mcp',{method:'POST',headers:{authorization:'Bearer '+access,'mcp-protocol-version':'2025-11-25'},body:{jsonrpc:'2.0',id:++rpcId,method,params}});
  assert.equal(r.status,200);assert.equal(r.value.error,undefined);return r.value.result;
}
async function tool(access,name,args={},ok=true) {
  const r=await rpc(access,'tools/call',{name,arguments:args});
  assert.equal(r.isError,!ok);assert.equal(r.structuredContent.ok,ok);
  return ok?r.structuredContent.data:r.structuredContent.error;
}
function openField(row,key,field=row.name) {
  const cipher=Buffer.from(row.ciphertext,'base64url');
  const decipher=createDecipheriv('aes-256-gcm',key,Buffer.from(row.nonce,'base64url'));
  decipher.setAAD(Buffer.from(JSON.stringify(['keyenv-remote-field-v1',row.account_id,row.entry_id,field,row.kid])));
  decipher.setAuthTag(cipher.subarray(-16));
  return Buffer.concat([decipher.update(cipher.subarray(0,-16)),decipher.final()]).toString();
}

try {
  const ui = await mf.dispatchFetch(origin+"/account/i18n.js");
  assert.equal(ui.status,200); assert.match(ui.headers.get("content-type"),/javascript/);
  assert.match(await ui.text(),/MilesAccountI18n/);
  const db=await mf.getD1Database('DB'),keys=await mf.getD1Database('KEYS_DB');
  // Upgrade both isolated databases from the pre-v2 schema with an existing
  // account, then exercise it through the new Worker.
  for(const store of [db,keys])for(const [name,sqls] of Object.entries(statements))if(name<'0021_')for(const sql of sqls)await store.prepare(sql).run();
  await db.prepare('INSERT INTO accounts(account_id,issuer,subject,created_at) VALUES(?,?,?,?)').bind('account_unified',issuer,subject,Date.now()).run();
  for(const store of [db,keys])for(const [name,sqls] of Object.entries(statements))if(name>='0021_')for(const sql of sqls)await store.prepare(sql).run();
  assert.equal((await db.prepare('SELECT subject FROM accounts WHERE account_id=?').bind('account_unified').first()).subject,subject);

  phase='synthetic browser Google login';
  const start=await request('/account/login',{method:'POST',headers:{origin},body:{}});
  assert.equal(start.status,200);
  const loginCookie=start.headers.get('set-cookie').split(';')[0];
  const authorization=new URL(start.value.data.authorization_url);
  assert.equal(authorization.origin,issuer);
  const providerCode=secret(),now=Math.floor(Date.now()/1000);
  const unsigned=encode({alg:'RS256',kid:jwk.kid})+'.'+encode({iss:issuer,aud:'synthetic-unified',sub:subject,nonce:authorization.searchParams.get('nonce'),iat:now,exp:now+300});
  issued.set(providerCode,unsigned+'.'+sign('RSA-SHA256',Buffer.from(unsigned),privateKey).toString('base64url'));
  const callback=await request('/account/callback?'+new URLSearchParams({state:authorization.searchParams.get('state'),code:providerCode}),{headers:{cookie:loginCookie}});
  issued.delete(providerCode);assert.equal(callback.status,303);
  const cookie=callback.headers.get('set-cookie').split(';')[0];
  const session=await request('/account/session',{headers:{cookie}});
  assert.equal(session.status,200);assert.equal(session.value.data.account_id,'account_unified');assert.equal(session.value.data.product,'unified');
  const browserHeaders={origin,cookie,'x-waypost-csrf':session.value.data.csrf_token};
  const space=await browserWrite('/account/spaces/create',{display_name:'Synthetic unified acceptance'},browserHeaders);
  await browserWrite('/account/spaces/select',{space_id:space.space_id},browserHeaders);
  const agent=await browserWrite('/account/agents/create',{request_id:'synthetic-agent',display_name:'Acceptance assistant'},browserHeaders);

  phase='one public OAuth authorization and explicit combined consent';
  const metadata=await request('/.well-known/oauth-authorization-server');
  assert.equal(metadata.status,200);assert.equal(metadata.value.registration_endpoint,origin+'/oauth/register');
  const registration=await request('/oauth/register',{method:'POST',body:{client_name:'Synthetic unified acceptance',redirect_uris:[redirect],token_endpoint_auth_method:'none',grant_types:['authorization_code','refresh_token'],response_types:['code'],scope}});
  assert.equal(registration.status,201);
  const client_id=registration.value.client_id,verifier=secret();
  const begun=await request('/oauth/authorize?'+new URLSearchParams({response_type:'code',client_id,redirect_uri:redirect,resource,scope,state:'synthetic-client-state',code_challenge:digest(verifier).toString('base64url'),code_challenge_method:'S256'}));
  assert.equal(begun.status,303);
  const oauthCookie=begun.headers.get('set-cookie').split(';')[0];
  const request_id=new URL(begun.headers.get('location'),origin).searchParams.get('oauth_request');assert(request_id);
  const consentHeaders={...browserHeaders,cookie:cookie+'; '+oauthCookie};
  const consent=await request('/oauth/consent?'+new URLSearchParams({request_id}),{headers:consentHeaders});
  assert.equal(consent.status,200);assert.equal(consent.value.product,'unified');assert.equal(consent.value.scope,scope);assert.deepEqual(consent.value.vault.entries,[]);
  const approved=await request('/oauth/consent',{method:'POST',headers:consentHeaders,body:{request_id,decision:'approve',...agent,can_create:true,all_read:true,all_write:true,entries:[]}});
  assert.equal(approved.status,200);
  const clientCallback=new URL(approved.value.redirect_uri);
  assert.equal(clientCallback.searchParams.get('state'),'synthetic-client-state');assert.equal(clientCallback.searchParams.get('iss'),origin);
  const exchange=await request('/oauth/token',{method:'POST',form:true,body:{resource,client_id,grant_type:'authorization_code',code:clientCallback.searchParams.get('code'),code_verifier:verifier,redirect_uri:redirect}});
  assert.equal(exchange.status,200);
  const access=exchange.value.access_token;assert(access);assert(exchange.value.refresh_token);

  phase='legacy unified protocol retired';
  const retiredMcp=await request('/mcp',{method:'POST',headers:{authorization:'Bearer '+access},body:{jsonrpc:'2.0',id:1,method:'tools/list',params:{}}});
  assert.equal(retiredMcp.status,410);assert.match(retiredMcp.value.error.message,/retired protocol/i);
  for(const path of ['/v0/describe','/v0/blobs/example/content','/v0/receiver-bindings']) {
    const retired=await request(path,{method:'GET',headers:{authorization:'Bearer '+access}});
    assert.equal(retired.status,410);assert.equal(retired.value.error.code,'LEGACY_PROTOCOL_RETIRED');
  }
  assert.equal((await db.prepare('SELECT count(*) AS n FROM unified_connection_grants WHERE request_id=?').bind(request_id).first()).n,1);
  phase='v2 bootstrap OAuth and business denial before owner mount';
  await db.prepare('UPDATE account_sessions SET created_at=? WHERE account_id=?').bind(Date.now()-900_001,'account_unified').run();
  const v2scope='works:client';
  const v2registration=await request('/oauth/register',{method:'POST',body:{client_name:'Synthetic bootstrap',redirect_uris:[redirect],token_endpoint_auth_method:'none',grant_types:['authorization_code','refresh_token'],response_types:['code'],scope:v2scope}});
  assert.equal(v2registration.status,201);
  const v2client=v2registration.value.client_id,v2verifier=secret();
  const v2begun=await request('/oauth/authorize?'+new URLSearchParams({response_type:'code',client_id:v2client,redirect_uri:redirect,resource,scope:v2scope,state:'v2-state',code_challenge:digest(v2verifier).toString('base64url'),code_challenge_method:'S256'}));
  assert.equal(v2begun.status,303);
  const v2cookie=v2begun.headers.get('set-cookie').split(';')[0];
  const v2request=new URL(v2begun.headers.get('location'),origin).searchParams.get('oauth_request');assert(v2request);
  const v2headers={...browserHeaders,cookie:cookie+'; '+v2cookie};
  const v2consent=await request('/oauth/consent?'+new URLSearchParams({request_id:v2request}),{headers:v2headers});
  assert.equal(v2consent.status,200);assert.equal(v2consent.value.product,'works_v2');assert.equal(v2consent.value.scope,v2scope);
  const v2approved=await request('/oauth/consent',{method:'POST',headers:v2headers,body:{request_id:v2request,decision:'approve'}});
  assert.equal(v2approved.status,200);
  const v2code=new URL(v2approved.value.redirect_uri).searchParams.get('code');assert(v2code);
  const v2exchange=await request('/oauth/token',{method:'POST',form:true,body:{resource,client_id:v2client,grant_type:'authorization_code',code:v2code,code_verifier:v2verifier,redirect_uri:redirect}});
  assert.equal(v2exchange.status,200);assert.equal(v2exchange.value.scope,v2scope);
  const v2access=v2exchange.value.access_token;assert(v2access);
  await db.prepare('UPDATE account_sessions SET created_at=? WHERE account_id=?').bind(Date.now(),'account_unified').run();
  const v2catalog=await rpc(v2access,'tools/list'),v2names=v2catalog.tools.map(t=>t.name);
  for(const name of ['works_identity','works_assistant_request','works_mount_request','works_messages_send'])assert(v2names.includes(name));
  assert(!v2names.some(name=>name.startsWith('waypost_')));assert(!v2names.includes('disable_entry'));
  assert.deepEqual((await tool(v2access,'works_identity')).assistants,[]);
  assert.equal((await tool(v2access,'works_messages_send',{assistant_id:'unmounted',recipient_assistant_id:'unmounted',conversation_id:'synthetic',idempotency_key:'request1',body:'synthetic'},false)).code,'ASSISTANT_REQUIRED');
  assert.equal((await tool(v2access,'list_entries',{},false)).code,'ASSISTANT_REQUIRED');

  phase='v2 dashboard and assistant-scoped MCP on isolated Worker';
  // The real dashboard script calls the actual compiled Worker, with the
  // synthetic browser session above. No business endpoint is mocked.
  const dom=new JSDOM(await readFile(root+'dashboard/index.html','utf8'),{url:origin+'/dashboard',runScripts:'outside-only'});
  const w=dom.window,d=w.document;
  w.HTMLDialogElement.prototype.showModal=function(){this.open=true;};
  w.HTMLDialogElement.prototype.close=function(){this.open=false;};
  w.fetch=(path,options={})=>mf.dispatchFetch(new URL(path,origin).href,{...options,headers:{...options.headers,origin,cookie}});
  const wait=async(fn,label)=>{for(let i=0;i<500;i++){if(fn())return;await new Promise(r=>setTimeout(r,20));}throw new Error('Dashboard did not reach '+label);};
  let sender;
  try {
    w.eval(await readFile(root+'dashboard/app.js','utf8'));
    await wait(()=>!d.querySelector('#app').hidden&&d.querySelector('#header-actions button.primary'),'assistant list');
    d.querySelector('#header-actions button.primary').click();
    await wait(()=>d.querySelector('#create-form [name=name]'),'create form');
    d.querySelector('#create-form [name=name]').value='UI sender';
    d.querySelector('#create-form').dispatchEvent(new w.Event('submit',{bubbles:true,cancelable:true}));
    await wait(()=>d.querySelector('#policy-form'),'created assistant');
    const all=await request('/account/works/assistants',{headers:browserHeaders});
    sender=all.value.data.assistants.find(a=>a.name==='UI sender').assistant_id;
  } finally {w.close();}
  const receiver=(await tool(v2access,'works_assistant_request',{name:'MCP receiver'})).assistant_id;
  await browserWrite('/account/works/assistants/policy',{assistant_id:receiver,policy_revision:1,messages_enabled:true,keys_read:false,keys_write:false,keys_create:false,key_scope:'all'},browserHeaders);
  await db.prepare('UPDATE account_sessions SET created_at=? WHERE account_id=?').bind(Date.now()-900_001,'account_unified').run();
  for(const assistant_id of [sender,receiver]) {
    const mount=await tool(v2access,'works_mount_request',{assistant_id});
    assert.equal(mount.status,'approved');
    assert.equal((await tool(v2access,'works_mount_status',{request_id:mount.request_id})).status,'approved');
  }
  await db.prepare('UPDATE account_sessions SET created_at=? WHERE account_id=?').bind(Date.now(),'account_unified').run();
  // A second OAuth grant is a distinct bridge installation, not the receiver's credential.
  const bridgeVerifier=secret();
  const bridgeBegun=await request('/oauth/authorize?'+new URLSearchParams({response_type:'code',client_id:v2client,redirect_uri:redirect,resource,scope:v2scope,state:'bridge-state',code_challenge:digest(bridgeVerifier).toString('base64url'),code_challenge_method:'S256'}));
  assert.equal(bridgeBegun.status,303);
  const bridgeCookie=bridgeBegun.headers.get('set-cookie').split(';')[0];
  const bridgeRequest=new URL(bridgeBegun.headers.get('location'),origin).searchParams.get('oauth_request');
  const combinedHeaders={...browserHeaders,cookie:cookie+'; '+bridgeCookie};
  const combinedView=await request('/oauth/consent?'+new URLSearchParams({request_id:bridgeRequest}),{headers:combinedHeaders});
  assert.equal(combinedView.status,200);
  const selectedAssistant=combinedView.value.assistants.find(a=>a.assistant_id===receiver);
  assert(selectedAssistant);assert.equal(selectedAssistant.policy_revision,2);
  const noCsrf=await request('/oauth/consent',{method:'POST',headers:{origin,cookie:cookie+'; '+bridgeCookie},body:{request_id:bridgeRequest,decision:'approve',assistant_id:receiver,policy_revision:2}});
  assert.notEqual(noCsrf.status,200);
  const unknownAssistant=await request('/oauth/consent',{method:'POST',headers:combinedHeaders,body:{request_id:bridgeRequest,decision:'approve',assistant_id:'assistant_unknown',policy_revision:2}});
  assert.notEqual(unknownAssistant.status,200);
  const staleApproval=await request('/oauth/consent',{method:'POST',headers:combinedHeaders,body:{request_id:bridgeRequest,decision:'approve',assistant_id:receiver,policy_revision:1}});
  assert.notEqual(staleApproval.status,200);
  const bridgeApproved=await request('/oauth/consent',{method:'POST',headers:combinedHeaders,body:{request_id:bridgeRequest,decision:'approve',assistant_id:receiver,policy_revision:2}});
  assert.equal(bridgeApproved.status,200);
  const bridgeCode=new URL(bridgeApproved.value.redirect_uri).searchParams.get('code');
  const bridgeExchange=await request('/oauth/token',{method:'POST',form:true,body:{resource,client_id:v2client,grant_type:'authorization_code',code:bridgeCode,code_verifier:bridgeVerifier,redirect_uri:redirect}});
  assert.equal(bridgeExchange.status,200);const bridgeAccess=bridgeExchange.value.access_token;
  assert.notEqual((await tool(bridgeAccess,'works_identity')).installation_id,(await tool(v2access,'works_identity')).installation_id);
  assert((await tool(bridgeAccess,'works_identity')).assistants.some(a=>a.assistant_id===receiver));
  assert.equal((await db.prepare('SELECT count(*) AS n FROM works_mount_requests WHERE installation_id=?').bind((await tool(bridgeAccess,'works_identity')).installation_id).first()).n,0);
  const sessionV2=await tool(v2access,'works_sessions_register',{assistant_id:receiver,host_kind:'webhook',external_session_id:'synthetic-receiver',display_name:'Test session',capability:'wakeable',discoverable:true});
  const senderSession=await tool(v2access,'works_sessions_register',{assistant_id:sender,host_kind:'codex',external_session_id:'synthetic-sender',display_name:'Sender session',capability:'pull',discoverable:true});
  const executionInstallation=(await tool(v2access,'works_identity')).installation_id;
  const clientAttachments=await request('/account/works/attachments',{headers:browserHeaders});
  const executionAttachment=clientAttachments.value.data.attachments.find(x=>x.assistant_id===receiver&&x.installation_id===executionInstallation&&x.status==='active');assert(executionAttachment);
  // Same-account active assistants need no separate A2A grant.
  // An independent loopback receiver authenticates the webhook and uses its
  // own OAuth installation for the complete v2 processing protocol.
  bridgeTemp=await mkdtemp(join(tmpdir(),'miles-webhook-test-'));
  const webhookKey=secret(), seenPath=join(bridgeTemp,'receiver-seen.json'),crashPath=join(bridgeTemp,'receiver-crash.json');
  let seenEvents=new Set(),finishReceiver,failReceiver,finishCrash,crashNext=false,workExecutions=0,webhookPosts=0,lastEvent;
  const crashReached=new Promise(resolve=>{finishCrash=resolve;});
  gateway=createServer(async(req,res)=>{try {const chunks=[];for await(const chunk of req)chunks.push(chunk);const response=await mf.dispatchFetch(origin+req.url,{method:req.method,headers:{'content-type':'application/json',authorization:req.headers.authorization||''},body:Buffer.concat(chunks)});res.writeHead(response.status,{'content-type':response.headers.get('content-type')||'application/json'});res.end(Buffer.from(await response.arrayBuffer()));}catch{res.writeHead(500).end();}});
  await new Promise(resolve=>gateway.listen(new URL(origin).port,'127.0.0.1',resolve));
  const receiverFinished=new Promise((resolve,reject)=>{finishReceiver=resolve;failReceiver=reject;});
  const processingGate=new Promise(resolve=>{releaseReceiver=resolve;});
  const receiverHandler=async(req,res)=>{
    try {
      if(req.method!=='POST'||req.headers.authorization!=='Bearer '+webhookKey){res.writeHead(401).end();return;}
      const chunks=[];for await(const chunk of req)chunks.push(chunk);
      const event=JSON.parse(Buffer.concat(chunks).toString());
      assert.equal(event.assistant_id,receiver);assert.equal(event.session_id,sessionV2.session_id);
      const eventKey=event.event==='miles-works.wake.challenge'?'wppair_'+event.binding_id:event.attempt_id;
      assert.equal(req.headers['idempotency-key'],eventKey);
      if(seenEvents.has(eventKey)){res.writeHead(202).end();return;}
      seenEvents.add(eventKey);await writeFile(seenPath,JSON.stringify([...seenEvents]),{mode:0o600});
      if(event.event==='miles-works.wake.challenge'){
        await tool(v2access,'works_wake_confirm',{assistant_id:event.assistant_id,binding_id:event.binding_id,challenge:event.challenge});
        res.writeHead(202).end();return;
      }
      assert.equal(event.event,'miles-works.message.available');webhookPosts++;lastEvent=event;
      res.writeHead(202).end();
      await processingGate;
      workExecutions++;
      const inbox=await tool(v2access,'works_messages_read',{assistant_id:receiver,session_id:sessionV2.session_id});
      const delivery=inbox.deliveries.find(item=>item.message_id===event.message_id&&item.delivery_id===event.delivery_id);assert(delivery);
      assert.equal(delivery.sender_assistant_id,sender);assert.equal(delivery.sender_session_id,senderSession.session_id);
      const delegation=await tool(v2access,'works_delegation_check',{assistant_id:receiver,session_id:sessionV2.session_id,delivery_id:delivery.delivery_id,assignment_generation:event.assignment_generation});
      assert.equal(delegation.delegated,true);assert.equal(delegation.reply,true);assert.equal(delegation.sender_session_id,senderSession.session_id);
      const claim=await tool(v2access,'works_deliveries_claim',{assistant_id:receiver,session_id:sessionV2.session_id,delivery_id:delivery.delivery_id,assignment_generation:event.assignment_generation,attempt_id:event.attempt_id});
      if(crashNext){await writeFile(crashPath,JSON.stringify({event,claim,conversation_id:delivery.conversation_id,work_done:true}),{mode:0o600});finishCrash();return;}
      await tool(v2access,'works_messages_send',{assistant_id:receiver,recipient_assistant_id:sender,session_id:senderSession.session_id,sender_session_id:sessionV2.session_id,conversation_id:delivery.conversation_id,idempotency_key:'reply_'+event.attempt_id,body:'synthetic result'});
      const complete=await tool(v2access,'works_deliveries_complete',{assistant_id:receiver,session_id:sessionV2.session_id,delivery_id:delivery.delivery_id,assignment_generation:event.assignment_generation,claim_generation:claim.claim_generation,conversation_generation:claim.conversation_generation,outcome:'completed'});
      finishReceiver(complete);
    } catch(error) {if(!res.headersSent)res.writeHead(500).end();failReceiver(error);}
  };
  receiverServer=createServer(receiverHandler);
  await new Promise(resolve=>receiverServer.listen(0,'127.0.0.1',resolve));
  const webhookUrl='http://127.0.0.1:'+receiverServer.address().port+'/wake';
  const notify=event=>fetch(webhookUrl,{method:'POST',headers:{authorization:'Bearer '+webhookKey,'content-type':'application/json','idempotency-key':event.event==='miles-works.wake.challenge'?'wppair_'+event.binding_id:event.attempt_id},body:JSON.stringify(event)});
  assert.equal((await fetch(webhookUrl,{method:'POST',headers:{authorization:'Bearer wrong'},body:'{}'})).status,401);
  const urlFile=join(bridgeTemp,'url'),secretFile=join(bridgeTemp,'secret'),oauthFile=join(bridgeTemp,'oauth.json'),configFile=join(bridgeTemp,'config.json');
  const bridgeIdentity=await tool(bridgeAccess,'works_identity');
  const privateWrite=(path,value)=>writeFile(path,value,{mode:0o600});
  await privateWrite(urlFile,webhookUrl);await privateWrite(secretFile,webhookKey);
  await privateWrite(oauthFile,JSON.stringify({version:1,v2_installation_id:bridgeIdentity.installation_id,origin,client_id:v2client,resource,space_id:'v2',actor_id:'v2',consumer_id:'v2',access_token:bridgeAccess,refresh_token:bridgeExchange.value.refresh_token,expires_at:Math.floor(Date.now()/1000)+1800,refresh_pending:false}));
  await privateWrite(configFile,JSON.stringify({config_version:3,base_url:origin,state_dir:join(bridgeTemp,'state'),oauth:{credentials_file:oauthFile,client_id:v2client,redirect_port:0},sessions:[{assistant_id:receiver,session_id:sessionV2.session_id,webhook:{url_file:urlFile,secret_file:secretFile,local_loopback:true}}]}));
  const bridge=(...args)=>new Promise((resolve,reject)=>{const child=spawn(root+'bridge/target/debug/waypost-bridge',['--config',configFile,...args],{env:{...process.env,NO_PROXY:'127.0.0.1,localhost,::1',no_proxy:'127.0.0.1,localhost,::1'},stdio:['ignore','ignore','pipe']});let stderr='';child.stderr.on('data',data=>{stderr+=data;});child.on('error',reject);child.on('close',code=>code===0?resolve():reject(new Error('isolated bridge '+args[0]+' failed: '+stderr.replaceAll(bridgeAccess,'[redacted]').replaceAll(webhookKey,'[redacted]'))));});
  await bridge('pair',sessionV2.session_id);
  await db.prepare('UPDATE account_sessions SET created_at=? WHERE account_id=?').bind(Date.now()-900_001,'account_unified').run();
  const sendUi=new JSDOM(await readFile(root+'dashboard/index.html','utf8'),{url:origin+'/dashboard',runScripts:'outside-only'}),sw=sendUi.window,sd=sw.document;
  sw.confirm=()=>true;sw.fetch=(path,options={})=>mf.dispatchFetch(new URL(path,origin).href,{...options,headers:{...options.headers,origin,cookie}});
  try {
    sw.eval(await readFile(root+'dashboard/app.js','utf8'));
    await wait(()=>!sd.querySelector('#app').hidden&&sd.querySelector('[data-tab=send]'),'send navigation');
    sd.querySelector('[data-tab=send]').click();await wait(()=>sd.querySelector('#send-form'),'send form');
    sd.querySelector('[name=assistant_id]').value=sender;sd.querySelector('#recipient').value=receiver;sd.querySelector('#recipient').dispatchEvent(new sw.Event('change'));
    sd.querySelector('#recipient-session').value=sessionV2.session_id;sd.querySelector('#recipient-session').dispatchEvent(new sw.Event('change'));
    sd.querySelector('[name=content]').value='synthetic UI message';sd.querySelector('#send-form').dispatchEvent(new sw.Event('submit',{bubbles:true,cancelable:true}));
    await wait(()=>!sd.querySelector('#notice').hidden,'send confirmation');
    const uiInbox=await tool(v2access,'works_messages_read',{assistant_id:receiver,session_id:sessionV2.session_id});
    const uiDelivery=uiInbox.deliveries.find(item=>item.body==='synthetic UI message');assert(uiDelivery);
    const uiClaim=await tool(v2access,'works_deliveries_claim',{assistant_id:receiver,session_id:sessionV2.session_id,delivery_id:uiDelivery.delivery_id,assignment_generation:uiDelivery.assignment_generation,attempt_id:'ui-attempt'});
    await tool(v2access,'works_deliveries_complete',{assistant_id:receiver,session_id:sessionV2.session_id,delivery_id:uiDelivery.delivery_id,assignment_generation:uiDelivery.assignment_generation,claim_generation:uiClaim.claim_generation,conversation_generation:uiClaim.conversation_generation,outcome:'completed'});
  } finally {sw.close();}
  await db.prepare('UPDATE account_sessions SET created_at=? WHERE account_id=?').bind(Date.now(),'account_unified').run();
  const bytes=Buffer.from('hello');
  const reserved=await tool(v2access,'works_attachments_prepare',{assistant_id:sender,request_id:'attachment1',content_type:'text/plain',size_bytes:bytes.length,sha256:createHash('sha256').update(bytes).digest('hex'),filename:'note.txt'});
  await tool(v2access,'works_attachments_upload',{assistant_id:sender,attachment_id:reserved.attachment_id,content_base64:bytes.toString('base64url')});
  const v2sent=await tool(v2access,'works_messages_send',{assistant_id:sender,recipient_assistant_id:receiver,session_id:sessionV2.session_id,conversation_id:'test-conversation',idempotency_key:'test-message',sender_session_id:senderSession.session_id,body:'synthetic',attachments:[reserved.attachment_id]});
  const attachment=await tool(v2access,'works_attachments_read',{assistant_id:receiver,message_id:v2sent.message_id,attachment_id:reserved.attachment_id});
  assert.equal(attachment.content_base64,bytes.toString('base64url'));
  await bridge('once');
  assert(lastEvent);assert.equal(lastEvent.message_id,v2sent.message_id);
  assert(!JSON.stringify(lastEvent).includes('synthetic UI message'));
  assert.equal((await db.prepare('SELECT state FROM works_deliveries WHERE delivery_id=?').bind(lastEvent.delivery_id).first()).state,'assigned');
  assert.equal(webhookPosts,1);
  await bridge('once');assert.equal(webhookPosts,1); // new process, same durable bridge journal
  releaseReceiver();
  const complete=await receiverFinished;assert.equal(complete.state,'completed');assert.equal(workExecutions,1);
  const receiverPort=receiverServer.address().port;
  receiverServer.closeAllConnections?.();await new Promise(resolve=>receiverServer.close(resolve));
  seenEvents=new Set(JSON.parse(await readFile(seenPath,'utf8')));
  receiverServer=createServer(receiverHandler);await new Promise(resolve=>receiverServer.listen(receiverPort,'127.0.0.1',resolve));
  assert.equal((await notify(lastEvent)).status,202);assert.equal(workExecutions,1);assert.equal(webhookPosts,1);
  // Interrupt the receiver after it durably records the claim and synthetic
  // work result but before its completion receipt. Restart from that evidence.
  crashNext=true;
  const interrupted=await tool(v2access,'works_messages_send',{assistant_id:sender,recipient_assistant_id:receiver,session_id:sessionV2.session_id,conversation_id:'crash-recovery',idempotency_key:'crash-message',sender_session_id:senderSession.session_id,body:'synthetic crash recovery'});
  await bridge('once');await crashReached;
  assert.equal(lastEvent.message_id,interrupted.message_id);assert.equal(workExecutions,2);
  const crashDelivery=lastEvent.delivery_id;
  assert.notEqual((await db.prepare('SELECT state FROM works_deliveries WHERE delivery_id=?').bind(crashDelivery).first()).state,'completed');
  receiverServer.closeAllConnections?.();await new Promise(resolve=>receiverServer.close(resolve));
  seenEvents=new Set(JSON.parse(await readFile(seenPath,'utf8')));
  receiverServer=createServer(receiverHandler);await new Promise(resolve=>receiverServer.listen(receiverPort,'127.0.0.1',resolve));
  const saved=JSON.parse(await readFile(crashPath,'utf8'));assert.equal(saved.work_done,true);
  const recoveryGrant=await tool(v2access,'works_delegation_check',{assistant_id:receiver,session_id:sessionV2.session_id,delivery_id:saved.event.delivery_id,assignment_generation:saved.event.assignment_generation});assert.equal(recoveryGrant.reply,true);
  const responseRequest={assistant_id:receiver,recipient_assistant_id:sender,session_id:senderSession.session_id,sender_session_id:sessionV2.session_id,conversation_id:saved.conversation_id,idempotency_key:'reply_'+saved.event.attempt_id,body:'synthetic recovered result'};
  const response=await tool(v2access,'works_messages_send',responseRequest);
  assert.equal((await tool(v2access,'works_messages_send',responseRequest)).message_id,response.message_id);
  const sourceInbox=await tool(v2access,'works_messages_read',{assistant_id:sender,session_id:senderSession.session_id});
  assert(sourceInbox.deliveries.some(x=>x.message_id===response.message_id&&x.sender_session_id===sessionV2.session_id));
  const recovered=await tool(v2access,'works_deliveries_complete',{assistant_id:receiver,session_id:sessionV2.session_id,delivery_id:saved.event.delivery_id,assignment_generation:saved.event.assignment_generation,claim_generation:saved.claim.claim_generation,conversation_generation:saved.claim.conversation_generation,outcome:'completed'});
  assert.equal(recovered.state,'completed');assert.equal(workExecutions,2);
  await bridge('once');assert.equal(webhookPosts,2);
  assert.equal((await notify(saved.event)).status,202);assert.equal(workExecutions,2);
  await tool(v2access,'works_materials_publish',{assistant_id:sender,collection_id:'test-notes',request_id:'publish1',expected_revision:0,items:[{item_id:'note',path:'note.txt',content_base64:bytes.toString('base64'),metadata:{}}]});
  const material=await tool(v2access,'works_materials_read',{assistant_id:receiver,collection_id:'test-notes'});
  assert.equal(material.items[0].content_base64,bytes.toString('base64'));
  await tool(v2access,'create_entry',{assistant_id:sender,name:'v2-synthetic',fields:{TOKEN:'synthetic-only'}});
  await tool(v2access,'fetch_entry',{assistant_id:sender,name:'v2-synthetic'});
  await tool(v2access,'update_entry',{assistant_id:sender,name:'v2-synthetic',set_fields:{TOKEN:'synthetic-updated'}});
  await browserWrite('/account/works/assistants/policy',{assistant_id:sender,policy_revision:2,messages_enabled:true,keys_read:false,keys_write:false,keys_create:true,key_scope:'all'},browserHeaders);
  assert.equal((await tool(v2access,'fetch_entry',{assistant_id:sender,name:'v2-synthetic'},false)).code,'FORBIDDEN');
  assert.equal((await tool(v2access,'update_entry',{assistant_id:sender,name:'v2-synthetic',set_fields:{TOKEN:'denied'}},false)).code,'FORBIDDEN');
  const identityBeforeRefresh=await tool(v2access,'works_identity');
  const attachmentsBeforeRefresh=(await db.prepare("SELECT count(*) AS n FROM works_attachments WHERE installation_id=? AND status='active'").bind(identityBeforeRefresh.installation_id).first()).n;
  const refreshed=await request('/oauth/token',{method:'POST',form:true,body:{resource,client_id:v2client,grant_type:'refresh_token',refresh_token:v2exchange.value.refresh_token}});
  assert.equal(refreshed.status,200);
  const identityAfterRefresh=await tool(refreshed.value.access_token,'works_identity');
  assert.equal(identityAfterRefresh.installation_id,identityBeforeRefresh.installation_id);
  assert.deepEqual(identityAfterRefresh.assistants,identityBeforeRefresh.assistants);
  assert.equal((await db.prepare("SELECT count(*) AS n FROM works_attachments WHERE installation_id=? AND status='active'").bind(identityAfterRefresh.installation_id).first()).n,attachmentsBeforeRefresh);
  await tool(refreshed.value.access_token,'works_sessions_list',{assistant_id:receiver});
  const v2revoked=await request('/oauth/revoke',{method:'POST',form:true,body:{client_id:v2client,token:refreshed.value.refresh_token}});
  assert.equal(v2revoked.status,200);
  const rejected=await request('/mcp',{method:'POST',headers:{authorization:'Bearer '+refreshed.value.access_token,'mcp-protocol-version':'2025-11-25'},body:{jsonrpc:'2.0',id:++rpcId,method:'tools/list'}});
  assert.equal(rejected.status,401);
  const virtualConsole=new VirtualConsole();let navigationError;
  virtualConsole.on('jsdomError',error=>{navigationError=error;});
  const logout=new JSDOM(await readFile(root+'dashboard/index.html','utf8'),{url:origin+'/dashboard',runScripts:'outside-only',virtualConsole}),lw=logout.window,ld=lw.document;
  lw.fetch=(path,options={})=>mf.dispatchFetch(new URL(path,origin).href,{...options,headers:{...options.headers,origin,cookie}});
  try {
    lw.eval(await readFile(root+'dashboard/app.js','utf8'));await wait(()=>!ld.querySelector('#app').hidden,'logout session');ld.querySelector('#logout').click();
    for(let i=0;i<500;i++){if((await request('/account/session',{headers:{cookie}})).status===401)break;if(i===499)throw Error('Logout did not invalidate session');await new Promise(r=>setTimeout(r,20));}
    await wait(()=>navigationError,'logout reload');assert.match(navigationError.message,/Not implemented: navigation/);
  } finally {lw.close();}
  assert.equal(catalogCalls,1);assert.equal(tokenCalls,1);assert.equal(jwksCalls,1);assert.equal(outbound,0);
  console.log(JSON.stringify({result:'passed',runtime:'actual compiled Worker in local Miniflare/Workerd',identity:'synthetic signed Google owner login',separate_d1_databases:2,legacy_unified_protocol_retired:true,old_session_bootstrap_mount_and_send:true,v2_bootstrap_denies_business_before_mount:true,v2_dashboard_create_mount_send_logout:true,v2_mount_and_session:true,v2_same_account_native_tasks:true,v2_built_bridge_receiver_worker:true,v2_bridge_restart_no_replay:true,v2_receiver_restart_deduplicates:true,v2_receiver_simulated_crash_after_durable_result:true,v2_independent_receiver_confirm_read_claim_complete:true,v2_attachment_message:true,v2_shared_material:true,v2_key_policy_revocation:true,v2_refresh_and_revoke:true,unexpected_outbound_requests:outbound,live_google_verified:false,production_verified:false}));
} catch(error) {
  // Never print tokens, cookies, codes, plaintext values, or response bodies.
  console.error(JSON.stringify({result:'failed',phase,error_type:error.constructor.name,assertion_line:error.stack?.split('\n').find(line=>line.includes('unified_worker.mjs:'))?.trim(),actual:typeof error.actual==='number'?error.actual:undefined,expected:typeof error.expected==='number'?error.expected:undefined}));
  process.exitCode=1;
} finally {if(releaseReceiver)releaseReceiver();if(receiverServer){receiverServer.closeAllConnections?.();await new Promise(resolve=>receiverServer.close(resolve));}if(gateway)await new Promise(resolve=>gateway.close(resolve));if(bridgeTemp)await rm(bridgeTemp,{recursive:true,force:true});await mf.dispose();}
