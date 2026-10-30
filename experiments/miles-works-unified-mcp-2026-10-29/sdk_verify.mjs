// Receives only synthetic loopback fixture credentials through stdin.
import assert from 'node:assert/strict';
import { Client } from '@modelcontextprotocol/sdk/client/index.js';
import { StreamableHTTPClientTransport } from '@modelcontextprotocol/sdk/client/streamableHttp.js';
let input = '';
for await (const chunk of process.stdin) input += chunk;
const { resource, token } = JSON.parse(input);
assert.equal(new URL(resource).hostname, '127.0.0.1');
const client = new Client({ name: 'isolated-validation', version: '0.1' });
const transport = new StreamableHTTPClientTransport(new URL(resource), {
  requestInit: { headers: { Authorization: `Bearer ${token}` } },
});
try {
  await client.connect(transport);
  const tools = await client.listTools();
  assert(tools.tools.some(t => t.name === 'waypost_send'));
  assert(tools.tools.some(t => t.name === 'fetch_entry'));
  const allowed = await client.callTool({ name: 'fetch_entry', arguments: { name: 'demo/allowed' } });
  assert.equal(allowed.isError, false);
  const denied = await client.callTool({ name: 'update_entry', arguments: { name: 'demo/allowed', value: 'SYNTHETIC' } });
  assert.equal(denied.isError, true);
  process.stdout.write(JSON.stringify({ status: 'passed', sdk: '@modelcontextprotocol/sdk 1.32.2',
    checks: ['streamable HTTP initialize/list/call', 'mixed tool discovery', 'read allowed; write denied'],
    production: false }) + '\n');
} finally {
  await client.close();
}
