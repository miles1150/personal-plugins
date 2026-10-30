// Acceptance of the actual compiled unified Worker in isolated local D1/R2.
// Browser OAuth and Google HTTPS responses are synthetic. No external network,
// real credentials, production state, or live provider verification is used.
import assert from 'node:assert/strict';
import {readFile} from 'node:fs/promises';
import {createPrivateKey,createPublicKey,createHash,createDecipheriv,randomBytes,sign} from 'node:crypto';
import {execFileSync} from 'node:child_process';
import {fileURLToPath} from 'node:url';
import {Miniflare,Response,Log,LogLevel} from '../examples/mcp-client/node_modules/miniflare/dist/src/index.js';

const root=fileURLToPath(new URL('../',import.meta.url));
const origin='http://127.0.0.1:8793',resource=origin+'/mcp';
const issuer='https://accounts.google.com',subject='synthetic-unified-owner';
const tokenUrl='https://oauth2.googleapis.com/token',jwksUrl='https://www.googleapis.com/oauth2/v3/certs';
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
out=[]
for path in sorted(pathlib.Path(sys.argv[1]).glob('*.sql')):
 pending=''
 for line in path.read_text().splitlines(True):
  pending+=line
  if sqlite3.complete_statement(pending):out.append(pending);pending=''
 if pending.strip() and not all(not l.strip() or l.lstrip().startswith('--') for l in pending.splitlines()):raise ValueError('Incomplete migration')
print(json.dumps(out))
`,root+'migrations'],{encoding:'utf8'}));

let phase='initialization',outbound=0,tokenCalls=0,jwksCalls=0,catalogCalls=0,rpcId=0;
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
  for(const store of [db,keys])for(const sql of statements)await store.prepare(sql).run();
  // The owner exists, but no browser sessions, grants, tokens, or spaces are seeded.
  await db.prepare('INSERT INTO accounts(account_id,issuer,subject,created_at) VALUES(?,?,?,?)').bind('account_unified',issuer,subject,Date.now()).run();

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

  phase='one unified MCP catalog and message identity';
  const initialized=await rpc(access,'initialize',{protocolVersion:'2025-11-25',capabilities:{},clientInfo:{name:'acceptance',version:'1'}});
  assert.equal(initialized.serverInfo.name,'miles-works');
  const catalog=await rpc(access,'tools/list');
  const names=catalog.tools.map(t=>t.name);assert.equal(names.length,new Set(names).size);
  for(const name of ['works_identity','waypost_describe','waypost_send','waypost_receiver_claim','waypost_receiver_complete','create_entry','fetch_entry','update_entry','list_entries'])assert(names.includes(name));
  const identity=await tool(access,'works_identity');
  assert.equal(identity.account_id,'account_unified');assert.equal(identity.messages_enabled,true);assert.equal(identity.keys_enabled,true);
  assert.equal(identity.actor_id,agent.actor_id);assert.equal(identity.consumer_id,agent.consumer_id);
  const described=await tool(access,'waypost_describe');
  assert.equal(described.space_id,space.space_id);assert.equal(described.actor_id,agent.actor_id);assert.equal(described.access_model,'space_membership');

  phase='keys create fetch update through the same connection';
  const name='acceptance/service',first=secret(),updated=secret();
  const created=await tool(access,'create_entry',{name,purpose:'Synthetic local fixture',fields:{TOKEN:first}});
  assert.equal(created.created,true);assert.deepEqual(created.fields,['TOKEN']);assert(!JSON.stringify(created).includes(first));
  assert.equal((await tool(access,'fetch_entry',{name})).fields.TOKEN,first);
  const changed=await tool(access,'update_entry',{name,set_fields:{TOKEN:updated}});
  assert.equal(changed.updated,true);assert(!JSON.stringify(changed).includes(updated));
  assert.equal((await tool(access,'fetch_entry',{name})).fields.TOKEN,updated);

  phase='independent D1 storage and encryption contexts';
  const row=await keys.prepare('SELECT e.account_id,f.* FROM key_entries e JOIN key_fields f USING(entry_id) WHERE e.name=?').bind(name).first();
  assert(row);assert.equal(row.kid,'vault');assert.notEqual(row.account_id,identity.account_id);
  assert.equal(openField(row,vaultKey),updated);
  assert.throws(()=>openField(row,messageKey));assert.throws(()=>openField(row,vaultKey,'OTHER_FIELD'));
  assert(!row.ciphertext.includes(first));assert(!row.ciphertext.includes(updated));
  assert.equal((await db.prepare('SELECT count(*) AS n FROM key_entries').first()).n,0);
  assert.equal((await keys.prepare('SELECT count(*) AS n FROM oauth_families').first()).n,0);
  const localAccount=await keys.prepare('SELECT issuer,subject FROM accounts WHERE account_id=?').bind(row.account_id).first();
  assert.equal(localAccount.issuer,issuer);assert.equal(localAccount.subject,subject);
  const messageText='Synthetic private message '+secret();
  const sent=await tool(access,'waypost_send',{request_id:'synthetic-message',recipient_ids:[agent.actor_id],kind:'info',subject:'Acceptance',body:{text:messageText}});
  const message=await tool(access,'waypost_message_get',{message_id:sent.message_id});assert.equal(message.body.text,messageText);
  assert.equal((await keys.prepare('SELECT count(*) AS n FROM messages').first()).n,0);
  const stored=await db.prepare('SELECT body_ref FROM messages WHERE message_id=?').bind(sent.message_id).first();
  const bucket=await mf.getR2Bucket('OBJECTS'),object=await bucket.get(stored.body_ref);assert(object);
  assert.equal(object.httpMetadata.contentType,'application/vnd.waypost.encrypted');
  const encryptedMessage=Buffer.from(await object.arrayBuffer());
  assert.equal(encryptedMessage.subarray(0,8).toString(),'WAYPOST1');
  const header=JSON.parse(encryptedMessage.subarray(12,12+encryptedMessage.readUInt32BE(8)).toString());
  assert.equal(header.kid,'message');
  assert(!encryptedMessage.includes(Buffer.from(messageText)));

  phase='key-only revocation retains message capability';
  const grants=await request('/account/keys/grants',{headers:browserHeaders});
  assert.equal(grants.status,200);assert.equal(grants.value.data.grants.length,1);
  assert.equal(grants.value.data.grants[0].grant_id,identity.connection_id);
  const revoked=await browserWrite('/account/keys/grants/revoke',{grant_id:identity.connection_id},browserHeaders);
  assert.equal(revoked.revoked,true);assert.equal(revoked.capability,'keys');
  assert.equal((await tool(access,'fetch_entry',{name},false)).code,'FORBIDDEN');
  const remaining=await tool(access,'works_identity');assert.equal(remaining.keys_enabled,false);assert.equal(remaining.messages_enabled,true);
  assert.equal((await tool(access,'waypost_describe')).actor_id,agent.actor_id);
  assert.equal((await tool(access,'waypost_message_get',{message_id:sent.message_id})).body.text,messageText);
  assert.equal((await db.prepare('SELECT messages_enabled,keys_enabled FROM unified_connection_grants WHERE request_id=?').bind(request_id).first()).keys_enabled,0);
  assert((await keys.prepare('SELECT revoked_at FROM unified_key_grants WHERE request_id=?').bind(request_id).first()).revoked_at);
  assert.equal(catalogCalls,1);assert.equal(tokenCalls,1);assert.equal(jwksCalls,1);assert.equal(outbound,0);
  console.log(JSON.stringify({result:'passed',runtime:'actual compiled Worker in local Miniflare/Workerd',identity:'synthetic signed Google owner login',separate_d1_databases:2,explicit_combined_consent:true,single_mcp_catalog:true,key_create_fetch_update:true,separate_encryption_keys_and_aad:true,encrypted_message_roundtrip:true,key_only_revoke_retains_messages:true,unexpected_outbound_requests:outbound,live_google_verified:false,production_verified:false}));
} catch(error) {
  // Never print tokens, cookies, codes, plaintext values, or response bodies.
  console.error(JSON.stringify({result:'failed',phase,error_type:error.constructor.name,assertion_line:error.stack?.split('\n').find(line=>line.includes('unified_worker.mjs:'))?.trim(),actual:typeof error.actual==='number'?error.actual:undefined,expected:typeof error.expected==='number'?error.expected:undefined}));
  process.exitCode=1;
} finally {await mf.dispose();}
