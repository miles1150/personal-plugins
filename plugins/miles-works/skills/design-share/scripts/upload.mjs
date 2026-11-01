#!/usr/bin/env node
// Prepare and upload a design snapshot for Miles Works sharing. Node 20+, no dependencies.
//   node upload.mjs info <file>          size, sha256 and title for works_shares_publish
//   node upload.mjs put <file> <url>     PUT the exact bytes to the upload URL it returned
import { readFile, stat } from 'node:fs/promises';
import { createHash } from 'node:crypto';
import { basename } from 'node:path';

const MAX = 50 * 1024 * 1024;
const out = (value, code = 0) => { process.stdout.write(JSON.stringify(value, null, 2) + '\n'); process.exit(code); };
const fail = (message, next) => out({ ok: false, error: message, ...(next ? { next } : {}) }, 1);

async function load(file) {
  if (!file) fail('Give the path of the exported .html file.');
  let info;
  try { info = await stat(file); } catch { fail(`No file at ${file}.`); }
  if (!info.isFile()) fail(`${file} is not a file.`);
  if (info.size === 0 || info.size > MAX) fail('The snapshot must be between 1 byte and 50 MB.');
  const bytes = await readFile(file);
  const head = bytes.subarray(0, 4096).toString('utf8').trimStart().toLowerCase();
  if (!head.startsWith('<!doctype html') && !head.startsWith('<html')) fail('This is not an HTML file. Use the editor’s HTML export.');
  return bytes;
}

const [command, file, url] = process.argv.slice(2);
if (command === 'info') {
  const bytes = await load(file);
  const title = (bytes.toString('utf8').match(/<title>([^<]{1,200})<\/title>/i) || [])[1];
  out({ ok: true, file, name: basename(file), size_bytes: bytes.length, sha256: createHash('sha256').update(bytes).digest('hex'), title: title ? title.trim() : null });
} else if (command === 'put') {
  const bytes = await load(file);
  let target;
  try { target = new URL(url); } catch { fail('Give the upload URL from works_shares_publish.'); }
  if (target.protocol !== 'https:' && !['127.0.0.1', 'localhost'].includes(target.hostname)) fail('The upload URL must use https.');
  if (!/^\/shares\/upload\/wsu_[A-Za-z0-9_-]{43}$/.test(target.pathname)) fail('This is not a Miles Works share upload URL.');
  const response = await fetch(target, { method: 'PUT', headers: { 'content-type': 'text/html; charset=utf-8', 'content-length': String(bytes.length) }, body: bytes });
  const value = await response.json().catch(() => null);
  if (!response.ok || !value || !value.ok) {
    const code = value && value.error && value.error.code;
    fail(code || `HTTP ${response.status}`, code === 'UPLOAD_EXPIRED' || code === 'NOT_FOUND' ? 'Call works_shares_publish again for a new upload URL.' : code === 'CHECKSUM_MISMATCH' ? 'The file changed after info. Run info again and publish again.' : undefined);
  }
  out({ ok: true, ...value.data });
} else {
  fail('Usage: node upload.mjs info <file> | node upload.mjs put <file> <upload-url>');
}
