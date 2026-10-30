"""Bundle the offline prototype for direct file opening; no remote downloads."""
from pathlib import Path
import re

root = Path(__file__).parent
html = (root / 'index.html').read_text()

def stylesheet(match):
    css = (root / match.group(1)).read_text()
    # Both imported files already have explicit link tags in index.html.
    css = re.sub(r'@import\s+[^;]+;', '', css)
    return '<style>' + css + '</style>'

def script(match):
    before, path, after = match.groups()
    source = (root / path).read_text().replace('</script', '<\\/script')
    return '<script' + before + after + '>' + source + '</script>'

html = re.sub(r'<link rel="stylesheet" href="([^"]+)">', stylesheet, html)
html = re.sub(r'<script([^>]*?) src="([^"]+)"([^>]*)></script>', script, html)
(root / 'preview.html').write_text(html)
print('Wrote offline preview.html')
