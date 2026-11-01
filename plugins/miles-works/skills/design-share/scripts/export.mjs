#!/usr/bin/env node
// Save a canvas or deck as one HTML file through its open editor's own export.
//   node export.mjs <editor-url> <output-dir>
// Uses Playwright when it is already installed (never installs anything). Without it,
// prints how the person exports by hand.
import { mkdir } from 'node:fs/promises';
import { createRequire } from 'node:module';
import { join, resolve } from 'node:path';

const out = (value, code = 0) => { process.stdout.write(JSON.stringify(value, null, 2) + '\n'); process.exit(code); };
const manual = 'In the open editor: on a canvas choose Export, then HTML; on a deck choose Download, then Web page (.html). Then give the saved file’s path.';
const [editor, dir] = process.argv.slice(2);
if (!editor || !dir) out({ ok: false, error: 'Usage: node export.mjs <editor-url> <output-dir>' }, 1);
let url;
try { url = new URL(editor); } catch { out({ ok: false, error: 'Give the editor URL that artifact.mjs open printed.' }, 1); }
if (!['127.0.0.1', 'localhost'].includes(url.hostname)) out({ ok: false, error: 'Only a local editor (127.0.0.1) can be exported.' }, 1);

let playwright;
for (const base of [process.cwd() + '/', import.meta.url]) {
  try { playwright = createRequire(base)('playwright'); break; } catch { /* try next */ }
}
if (!playwright) out({ ok: false, error: 'Playwright is not installed here.', next: manual }, 2);

const target = resolve(dir);
await mkdir(target, { recursive: true });
const browser = await playwright.chromium.launch();
try {
  const page = await browser.newPage({ viewport: { width: 1440, height: 900 }, acceptDownloads: true });
  await page.goto(url.href, { waitUntil: 'networkidle', timeout: 60000 });
  await page.waitForTimeout(1500);
  const menu = page.getByRole('button', { name: /^(Export|Download)$/ }).first();
  if (!(await menu.count())) out({ ok: false, error: 'This editor has no Export or Download button.', next: manual }, 2);
  await menu.click();
  const item = page.getByRole('menuitem', { name: /HTML|Web page/i }).first();
  const choice = (await item.count()) ? item : page.getByText(/^(HTML|Web page \(\.html\))/).first();
  if (!(await choice.count())) out({ ok: false, error: 'This editor offers no HTML export.', next: manual }, 2);
  const [download] = await Promise.all([page.waitForEvent('download', { timeout: 120000 }), choice.click()]);
  const file = join(target, download.suggestedFilename().replace(/[\\/]/g, '-'));
  await download.saveAs(file);
  out({ ok: true, file });
} catch (error) {
  out({ ok: false, error: String(error && error.message || error).slice(0, 300), next: manual }, 2);
} finally {
  await browser.close();
}
