import {JSDOM} from '../examples/mcp-client/node_modules/jsdom/lib/api.js';
import {readFileSync} from 'node:fs';
import assert from 'node:assert/strict';
import test from 'node:test';
const wait=async()=>new Promise(r=>setTimeout(r,20));

test('consent shows untrusted client text and grants only selected entry',async()=>{
  const dom=new JSDOM(readFileSync('dashboard/keys.html','utf8'),{url:'https://keys.example/account?oauth_request=oauth_test',runScripts:'outside-only'});
  const calls=[];
  dom.window.fetch=async(path,options)=>{
    calls.push({path,options});
    if(path==='/account/session')return {status:200,json:async()=>({data:{csrf_token:'csrf'}})};
    if(path.startsWith('/oauth/consent?'))return {status:200,json:async()=>({client_name:'<img src=x>',client_kind:'dynamic_registration',client_id:'dcr_id',client_host:'',redirect_host:'127.0.0.1',scope:'keys:read',client_warnings:['unverified'],warning:'Secrets enter the model',vault:{entries:[{name:'entry',purpose:'<script>x</script>',fields:['TOKEN'],disabled:false}]}})};
    return {status:400,json:async()=>({error:'invalid_request'})};
  };
  dom.window.eval(readFileSync('dashboard/key-permissions.js','utf8'));
  dom.window.eval(readFileSync('dashboard/keys.js','utf8'));
  await wait();
  const doc=dom.window.document;
  assert.equal(doc.querySelectorAll('img,script:not([src])').length,0);
  assert.equal(doc.querySelector('#entries .read').checked,false);
  assert.equal(doc.querySelector('#entries .write').disabled,true);
  const all=doc.getElementById('all-read');all.checked=true;all.dispatchEvent(new dom.window.Event('change'));
  all.checked=false;all.dispatchEvent(new dom.window.Event('change'));
  assert.equal(doc.querySelector('#entries .read').checked,false);
  doc.querySelector('#entries .read').checked=true;
  doc.getElementById('approve').click();await wait();
  const request=calls.find(c=>c.path==='/oauth/consent' && c.options.method==='POST');
  assert.deepEqual(JSON.parse(request.options.body),{request_id:'oauth_test',decision:'approve',all_read:false,all_write:false,can_create:false,entries:[{name:'entry',read:true,write:false}]});
  assert.equal(request.options.headers['x-waypost-csrf'],'csrf');
  dom.window.close();
});

function permissionsFixture(scope='keys:read keys:write') {
  const dom = new JSDOM(readFileSync('dashboard/keys.html','utf8'),{url:'https://keys.example/account',runScripts:'outside-only'});
  dom.window.eval(readFileSync('dashboard/key-permissions.js','utf8'));
  const doc=dom.window.document;
  const control=dom.window.MilesKeyPermissions(doc.getElementById('consent'),{scope,vault:{entries:[{name:'existing',fields:['TOKEN'],disabled:false}]}});
  return {dom,doc,control};
}

test('trusted preset requires explicit existing/future approval and includes read, write and create',()=>{
  const {dom,doc,control}=permissionsFixture();
  try {
    assert.deepEqual(JSON.parse(JSON.stringify(control.payload())),{all_read:false,all_write:false,can_create:false,entries:[]});
    doc.getElementById('preset-trusted').click();
    assert.throws(()=>control.validate(),/Confirm access to all existing and future entries/);
    assert.match(doc.getElementById('permission-summary').textContent,/Read all secret values/);
    assert.equal(doc.getElementById('key-advanced').open,false);
    assert.equal(doc.getElementById('preset-note').hidden,true);
    assert.match(doc.getElementById('broad-confirm').textContent,/all existing and future entries/);
    assert.match(doc.getElementById('permission-summary').textContent,/Change values and remove fields/);
    assert.equal(doc.getElementById('permission-summary').children.length,3);
    doc.getElementById('broad-ack').checked=true;
    assert.deepEqual(JSON.parse(JSON.stringify(control.validate())),{all_read:true,all_write:true,can_create:true,entries:[]});
    const write=doc.getElementById('all-write');write.checked=false;write.dispatchEvent(new dom.window.Event('change'));
    assert.equal(doc.getElementById('broad-ack').checked,false);
    assert.throws(()=>control.validate(),/Confirm/);
    doc.getElementById('preset-custom').click();
    assert.equal(doc.getElementById('key-advanced').open,true);
    assert.deepEqual(JSON.parse(JSON.stringify(control.payload())),{all_read:false,all_write:false,can_create:false,entries:[]});
    assert.equal(doc.getElementById('broad-confirm').hidden,true);
  } finally {dom.window.close();}
});

for (const scope of ['keys:read','keys:write']) test(`trusted preset respects requested ${scope} scope`,()=>{
  const {dom,doc,control}=permissionsFixture(scope);
  try {
    doc.getElementById('preset-trusted').click();doc.getElementById('broad-ack').checked=true;
    // Programmatic changes to disabled controls cannot smuggle unrequested scope.
    for(const id of ['all-read','all-write','can-create']) doc.getElementById(id).checked=true;
    const selected=control.validate();
    assert.equal(selected.all_read,scope==='keys:read');
    assert.equal(selected.all_write,scope==='keys:write');
    assert.equal(selected.can_create,scope==='keys:write');
  }finally{dom.window.close();}
});
