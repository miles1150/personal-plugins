#!/usr/bin/env python3
"""Check the native Miles Works dashboard assets and localization keys."""
import json
import re
from pathlib import Path

root = Path(__file__).resolve().parent
errors = []
for name in ('index.html', 'account.html'):
    html = (root / name).read_text()
    if '<meta name="viewport"' not in html:
        errors.append(f'{name}: missing viewport')
    if re.search(r'<script(?![^>]*\bsrc=)[^>]*>', html, re.I):
        errors.append(f'{name}: inline script is incompatible with CSP')
    if re.search(r'\son\w+\s*=', html, re.I):
        errors.append(f'{name}: inline event handler is incompatible with CSP')
for name in ('app.js', 'account.js'):
    source = (root / name).read_text()
    if re.search(r'\beval\s*\(|\bnew\s+Function\s*\(', source):
        errors.append(f'{name}: dynamic code evaluation')
for file in ('app.js', 'account.js', 'style.css', 'account.css', 'tokens.css'):
    if not (root / file).is_file():
        errors.append(f'missing {file}')
print(json.dumps({'errors': errors}, ensure_ascii=False))
raise SystemExit(bool(errors))
