// Acceptance of design sharing on the actual compiled unified Worker in local
// Miniflare/Workerd with isolated D1/R2. Google login is synthetic and signed by
// a fixture key; no external network, real credentials or production state.
//   node tests/shares_worker.mjs <snapshot.html> [--serve state.json]
// --serve keeps the Worker listening for browser review and writes the link,
// owner cookie and URLs to state.json; stop it with Ctrl-C.
import assert from 'node:assert/strict';
import {readFile,writeFile} from 'node:fs/promises';
import {createPrivateKey,createPublicKey,createHash,randomBytes,sign} from 'node:crypto';
import {execFileSync} from 'node:child_process';
import {createServer} from 'node:http';
import {fileURLToPath} from 'node:url';

const root=fileURLToPath(new URL('../',import.meta.url));
const miniflarePath=process.env.MINIFLARE_MODULE||root+'examples/mcp-client/node_modules/miniflare/dist/src/index.js';
const {Miniflare,Response,Log,LogLevel}=await import(miniflarePath);
const [snapshotPath,flag,statePath]=process.argv.slice(2);
assert(snapshotPath,'usage: node tests/shares_worker.mjs <snapshot.html> [--serve state.json]');
const serve=flag==='--serve';
const uploadScript=fileURLToPath(new URL('../../../plugins/miles-works/skills/design-share/scripts/upload.mjs',import.meta.url));

const probe=createServer();await new Promise(r=>probe.listen(0,'127.0.0.1',r));
const port=probe.address().port;await new Promise(r=>probe.close(r));
const origin='http://127.0.0.1:'+port,resource=origin+'/mcp';
const issuer='https://accounts.google.com',subject='synthetic-share-owner';
const tokenUrl='https://oauth2.googleapis.com/token',jwksUrl='https://www.googleapis.com/oauth2/v3/certs';
const redirect='http://127.0.0.1:8765/callback';
const privateKey=createPrivateKey(await readFile(new URL('fixtures/oidc-test-private.pem',import.meta.url)));
const jwk={...createPublicKey(privateKey).export({format:'jwk'}),kid:'share-fixture',alg:'RS256',use:'sig'};
const secret=()=>randomBytes(32).toString('base64url');
const encode=v=>Buffer.from(JSON.stringify(v)).toString('base64url');
const digest=v=>createHash('sha256').update(v).digest();
const statements=JSON.parse(execFileSync('python3',['-c',`
import json,pathlib,sqlite3,sys
out=[]
for path in sorted(pathlib.Path(sys.argv[1]).glob('*.sql')):
 pending=''
 for line in path.read_text().splitlines(True):
  pending+=line
  if sqlite3.complete_statement(pending):out.append(pending);pending=''
print(json.dumps(out))
`,root+'migrations'],{encoding:'utf8'}));
const issued=new Map();let outbound=0;
const mf=new Miniflare({
  modules:true,scriptPath:root+'build/index.js',modulesRules:[{type:'CompiledWasm',include:['**/*.wasm'],fallthrough:true}],
  compatibilityDate:'2026-07-17',log:new Log(LogLevel.NONE),host:'127.0.0.1',port,
  d1Databases:{DB:'share-control-test',KEYS_DB:'share-vault-test'},r2Buckets:['OBJECTS'],
  bindings:{PUBLIC_ORIGIN:origin,WAYPOST_PRODUCT:'unified',WAYPOST_OPERATOR_MODE:'disabled',WAYPOST_ONBOARDING_MODE:'accounts',
    WAYPOST_CSRF_SECRET:secret(),WAYPOST_CURSOR_SECRET:secret(),WAYPOST_ACCOUNT_SIGNUP_LIMIT:'1',WORKS_OWNER_SUBJECT:subject,
    WAYPOST_ENCRYPTION_KEYS:JSON.stringify({active:'m',keys:{m:randomBytes(32).toString('hex')}}),
    KEYS_DATA_KEYS:JSON.stringify({active:'v',keys:{v:randomBytes(32).toString('hex')}}),
    WAYPOST_OAUTH_CLIENT_DISCOVERY:JSON.stringify({dynamic_registration:true}),
    WAYPOST_OIDC_CONFIG:JSON.stringify({issuer,client_id:'synthetic-share',authorization_endpoint:issuer+'/o/oauth2/v2/auth',token_endpoint:tokenUrl,google_jwks:true,keys:[]})},
  outboundService:async request=>{
    if(request.url===jwksUrl)return new Response(JSON.stringify({keys:[jwk]}),{headers:{'content-type':'application/json'}});
    if(request.url===tokenUrl&&request.method==='POST'){const f=new URLSearchParams(await request.text());return new Response(JSON.stringify({id_token:issued.get(f.get('code'))}),{headers:{'content-type':'application/json'}});}
    outbound++;throw new Error('Unexpected outbound request blocked');
  },
});
async function request(path,{method='GET',body,headers={},form=false,raw}={}) {
  const r=await mf.dispatchFetch(origin+path,{method,redirect:'manual',headers:{'content-type':form?'application/x-www-form-urlencoded':'application/json',...headers},...(raw!==undefined?{body:raw}:body===undefined?{}:{body:form?new URLSearchParams(body):JSON.stringify(body)})});
  const text=await r.text();let value=null;try{value=text?JSON.parse(text):null}catch{value=text}
  return {status:r.status,headers:r.headers,value};
}
let rpcId=0;
async function tool(access,name,args={},ok=true) {
  const r=await request('/mcp',{method:'POST',headers:{authorization:'Bearer '+access,'mcp-protocol-version':'2025-11-25'},body:{jsonrpc:'2.0',id:++rpcId,method:'tools/call',params:{name,arguments:args}}});
  assert.equal(r.status,200);const s=r.value.result.structuredContent;assert.equal(s.ok,ok,JSON.stringify(s));return ok?s.data:s.error;
}
let phase='setup';
try {
  await mf.ready;
  const db=await mf.getD1Database('DB'),keys=await mf.getD1Database('KEYS_DB');
  for(const store of [db,keys])for(const sql of statements)await store.prepare(sql).run();

  phase='synthetic owner login';
  const start=await request('/account/login',{method:'POST',headers:{origin},body:{}});
  const loginCookie=start.headers.get('set-cookie').split(';')[0];
  const authorization=new URL(start.value.data.authorization_url);
  const code=secret(),now=Math.floor(Date.now()/1000);
  const unsigned=encode({alg:'RS256',kid:jwk.kid})+'.'+encode({iss:issuer,aud:'synthetic-share',sub:subject,nonce:authorization.searchParams.get('nonce'),iat:now,exp:now+300});
  issued.set(code,unsigned+'.'+sign('RSA-SHA256',Buffer.from(unsigned),privateKey).toString('base64url'));
  const callback=await request('/account/callback?'+new URLSearchParams({state:authorization.searchParams.get('state'),code}),{headers:{cookie:loginCookie}});
  assert.equal(callback.status,303);
  const cookie=callback.headers.get('set-cookie').split(';')[0];
  const session=await request('/account/session',{headers:{cookie}});
  const owner={origin,cookie,'x-waypost-csrf':session.value.data.csrf_token};

  phase='v2 client and mounted assistant';
  const scope='works:client';
  const reg=await request('/oauth/register',{method:'POST',body:{client_name:'Share acceptance',redirect_uris:[redirect],token_endpoint_auth_method:'none',grant_types:['authorization_code','refresh_token'],response_types:['code'],scope}});
  const client_id=reg.value.client_id,verifier=secret();
  const begun=await request('/oauth/authorize?'+new URLSearchParams({response_type:'code',client_id,redirect_uri:redirect,resource,scope,state:'s',code_challenge:digest(verifier).toString('base64url'),code_challenge_method:'S256'}));
  const oauthCookie=begun.headers.get('set-cookie').split(';')[0];
  const request_id=new URL(begun.headers.get('location'),origin).searchParams.get('oauth_request');
  const approved=await request('/oauth/consent',{method:'POST',headers:{...owner,cookie:cookie+'; '+oauthCookie},body:{request_id,decision:'approve'}});
  assert.equal(approved.status,200,JSON.stringify(approved.value));
  const exchange=await request('/oauth/token',{method:'POST',form:true,body:{resource,client_id,grant_type:'authorization_code',code:new URL(approved.value.redirect_uri).searchParams.get('code'),code_verifier:verifier,redirect_uri:redirect}});
  const access=exchange.value.access_token;assert(access);
  const assistant=(await tool(access,'works_assistant_request',{name:'Design helper'})).assistant_id;
  const policy=await request('/account/works/assistants/policy',{method:'POST',headers:owner,body:{assistant_id:assistant,policy_revision:1,messages_enabled:true,keys_read:false,keys_write:false,keys_create:false,key_scope:'all'}});
  assert.equal(policy.status,200,JSON.stringify(policy.value));
  assert.equal((await tool(access,'works_mount_request',{assistant_id:assistant})).status,'approved');
  const names=(await request('/mcp',{method:'POST',headers:{authorization:'Bearer '+access,'mcp-protocol-version':'2025-11-25'},body:{jsonrpc:'2.0',id:++rpcId,method:'tools/list',params:{}}})).value.result.tools.map(t=>t.name);
  for(const n of ['works_shares_publish','works_shares_list','works_shares_link_create','works_shares_link_revoke','works_shares_comments','works_shares_comment_reply'])assert(names.includes(n),n);

  phase='publish and upload through the skill script';
  const info=JSON.parse(execFileSync('node',[uploadScript,'info',snapshotPath],{encoding:'utf8'}));
  const published=await tool(access,'works_shares_publish',{title:info.title||'Shared design',kind:'canvas',size_bytes:info.size_bytes,sha256:info.sha256});
  assert(published.upload.url.startsWith(origin+'/shares/upload/wsu_'));
  const tampered=await request(new URL(published.upload.url).pathname,{method:'PUT',headers:{'content-type':'text/html; charset=utf-8'},raw:Buffer.alloc(info.size_bytes,1)});
  assert.equal(tampered.status,422,JSON.stringify(tampered.value));
  const put=JSON.parse(execFileSync('node',[uploadScript,'put',snapshotPath,published.upload.url],{encoding:'utf8'}));
  assert.equal(put.status,'ready');

  phase='link is off until the owner turns it on';
  const link=await tool(access,'works_shares_link_create',{share_id:published.share_id});
  const path=new URL(link.url).pathname;
  assert.equal((await request(path+'/info')).value.error.code,'SHARE_LINK_OFF');
  assert.equal((await request(path+'/content')).status,403);
  assert.equal((await request('/account/shares/links/activate',{method:'POST',headers:{origin,cookie},body:{link_id:link.link_id}})).status,403);
  const preview=await request('/account/shares/preview/'+published.share_id,{headers:{cookie}});
  assert.equal(preview.status,200);assert.match(preview.headers.get('content-security-policy'),/^sandbox allow-scripts /);
  const viewerPage=await request(path);assert.equal(viewerPage.status,200);assert.match(viewerPage.headers.get('content-type'),/text\/html/);
  const ownerPage=await request('/account/shares');assert.equal(ownerPage.status,200);
  if(!serve){
    const on=await request('/account/shares/links/activate',{method:'POST',headers:owner,body:{link_id:link.link_id}});
    assert.equal(on.status,200,JSON.stringify(on.value));

    phase='guests view and comment; assistant reads and replies';
    assert.equal((await request(path+'/info')).value.data.role,'comment');
    const content=await request(path+'/content');
    assert.equal(content.status,200);
    const csp=content.headers.get('content-security-policy');
    assert.match(csp,/^sandbox allow-scripts /);assert.doesNotMatch(csp,/allow-same-origin/);
    assert(content.value.startsWith((await readFile(snapshotPath,'utf8')).slice(0,200)));
    assert.match(content.value,/share:ready/);
    const sandboxed=await request('/account/shares/data',{headers:{origin:'null',cookie}});
    assert.equal(sandboxed.status,403,'opaque-origin requests are refused');
    const posted=await request(path+'/comments',{method:'POST',headers:{origin},body:{name:'Ana',body:'Make the button larger',anchor:{x:0.2,y:0.4,view:'',text:'Get the menu'}}});
    assert.equal(posted.status,200,JSON.stringify(posted.value));
    const read=await tool(access,'works_shares_comments',{share_id:published.share_id});
    assert.equal(read.comments[0].body,'Make the button larger');
    await tool(access,'works_shares_comment_reply',{share_id:published.share_id,parent_id:read.comments[0].comment_id,body:'Done in version 2.'});
    assert.equal((await request(path+'/comments')).value.data.comments[1].author,'Design helper');

    phase='new version, revocation';
    const v2=await tool(access,'works_shares_publish',{share_id:published.share_id,size_bytes:info.size_bytes,sha256:info.sha256});
    assert.equal(v2.version,2);
    execFileSync('node',[uploadScript,'put',snapshotPath,v2.upload.url]);
    assert.equal((await request(path+'/info')).value.data.version,2);
    await tool(access,'works_shares_link_revoke',{link_id:link.link_id});
    assert.equal((await request(path+'/info')).value.error.code,'SHARE_LINK_REVOKED');
    assert.equal(outbound,0);
    console.log(JSON.stringify({result:'passed',runtime:'compiled Worker in local Miniflare/Workerd',owner_gate:true,sandbox_csp:true,opaque_origin_refused:true,guest_comment_and_agent_reply:true,versions:true,revocation:true,unexpected_outbound_requests:outbound,production_verified:false}));
  } else {
    await writeFile(statePath,JSON.stringify({origin,cookie,csrf:owner['x-waypost-csrf'],access,share_id:published.share_id,link,turn_on_url:link.turn_on_url},null,2));
    console.log('Serving '+origin+' — state in '+statePath);
    await new Promise(()=>{});
  }
} catch(error) {
  console.error('Failed during: '+phase);throw error;
} finally {
  if(!serve)await mf.dispose();
}
