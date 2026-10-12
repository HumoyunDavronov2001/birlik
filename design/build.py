"""Build concept.html from concept.src.html: inline tokens.css, living-ring.js and colors.json."""
import json
import pathlib

here = pathlib.Path(__file__).parent
page = (here / 'concept.src.html').read_text(encoding='utf-8')
parts = {
    '/*TOKENS*/': (here / 'tokens.css').read_text(encoding='utf-8'),
    '/*LIVING_RING*/': (here / 'living-ring.js').read_text(encoding='utf-8').replace('export function livingRing', 'function livingRing', 1),
    '/*COLORS*/null': json.dumps(json.loads((here / 'colors.json').read_text()), ensure_ascii=False),
}
for marker, content in parts.items():
    assert page.count(marker) == 1, marker
    page = page.replace(marker, content)
(here / 'concept.html').write_text(page, encoding='utf-8')
print('concept.html', len(page), 'bytes')
