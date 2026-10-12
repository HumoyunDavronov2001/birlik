#!/usr/bin/env python3
"""Counts the screens and functions of SPEC.md and writes the summary between its count markers.

    python3 .github/scripts/spec_count.py SPEC.md            # rewrite the count
    python3 .github/scripts/spec_count.py SPEC.md --check    # fail if the count is out of date (CI)

A counted row is a table row "| XX-00 | what | screen or function | req | stage | design | ✓, ◐ or ○ |"; the part is
the "## N." section it sits in. A stage written "B? → B4" counts under its proposed stage.
"""
import collections
import pathlib
import re
import sys

ROW = re.compile(r'^\| ([A-Z]{2}-\d\d) \| (.+?) \| (screen|function) \| (.*?) \| (.*?) \| (.*?) \| (✓|◐|○) \|$')
BLOCK = re.compile(r'(<!-- count:begin[^\n]*-->\n).*?(<!-- count:end -->)', re.S)


def ready(c):
    total = c['screen'] + c['function']
    return f"{round(100 * (c['✓'] + c['◐'] / 2) / total)} %" if total else '—'


def count(text):
    parts = collections.OrderedDict()
    stages = collections.OrderedDict((f'B{i}', collections.Counter()) for i in range(10))
    seen = set()
    section = None
    for line in text.splitlines():
        heading = re.match(r'^## \d+\. (.+)$', line)
        if heading:
            section = heading.group(1)
            continue
        row = ROW.match(line)
        if not row:
            continue
        item, _, kind, _, stage, _, state = row.groups()
        if item in seen:
            sys.exit(f'{item} appears twice')
        seen.add(item)
        part = parts.setdefault(section, collections.Counter())
        part[kind] += 1
        part[state] += 1
        found = re.search(r'→ (B\d)', stage) or re.search(r'(B\d)', stage)
        if not found:
            sys.exit(f'{item}: no stage in "{stage}"')
        stages[found.group(1)][state] += 1
        stages[found.group(1)]['all'] += 1

    lines = ['| Part | Screens | Functions | ✓ done | ◐ partly | ○ not started | Ready |',
             '|---|---|---|---|---|---|---|']
    total = collections.Counter()
    for name, c in parts.items():
        total.update(c)
        lines.append(f"| {name} | {c['screen']} | {c['function']} | {c['✓']} | {c['◐']} | {c['○']} | {ready(c)} |")
    lines.append(f"| **All** | **{total['screen']}** | **{total['function']}** | **{total['✓']}** | **{total['◐']}** "
                 f"| **{total['○']}** | **{ready(total)}** |")
    lines += ['', '"Ready" counts a partly done item as half. By stage — what each stage has to deliver:', '',
              '| Stage | Items | ✓ | ◐ | ○ |', '|---|---|---|---|---|']
    lines += [f"| {s} | {c['all']} | {c['✓']} | {c['◐']} | {c['○']} |" for s, c in stages.items() if c['all']]
    return '\n'.join(lines)


def main():
    path = pathlib.Path(sys.argv[1])
    text = path.read_text(encoding='utf-8')
    block = count(text)
    updated = BLOCK.sub(lambda m: m.group(1) + block + '\n' + m.group(2), text)
    if '--check' in sys.argv[2:]:
        if updated != text:
            sys.exit(f'{path}: the count is out of date; run python3 .github/scripts/spec_count.py {path}')
        print(f'{path}: the count is up to date')
        return
    path.write_text(updated, encoding='utf-8')
    print(block)


if __name__ == '__main__':
    main()
