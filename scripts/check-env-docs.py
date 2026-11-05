#!/usr/bin/env python3
"""Fails if the server's documented settings disagree with server/src/config.rs.

Someone who copies .env.example gets exactly what it says, so a wrong default there
changes how their server behaves: it once cut the upload limit from 100 MB to 5 MB.
This reads the names and defaults from the code and checks that:

- .env.example mentions every setting, and active lines match the code's default
- rows in the README and SECURITY.md tables match the code's default

Run by .github/workflows/repo-checks.yml.

Usage: scripts/check-env-docs.py [repo-dir]
"""

import re
import sys
from pathlib import Path

root = Path(sys.argv[1] if len(sys.argv) > 1 else '.')
config = (root / 'server/src/config.rs').read_text()

names = set(re.findall(r'\b(?:var|lookup)\(\s*"([A-Z0-9_]+)"\s*\)', config))
defaults = dict(re.findall(r'\bvar\("([A-Z0-9_]+)"\)\s*\.unwrap_or_else\(\|_\| "([^"]*)"', config))
# parsed_or's default is a Rust expression, so only a plain literal can be compared
defaults |= dict(re.findall(r'parsed_or\(\s*lookup\("([A-Z0-9_]+)"\),\s*([a-z0-9]+)\s*,?\s*\)', config))

if len(names) < 20 or len(defaults) < 15:
    sys.exit(f'Found only {len(names)} settings and {len(defaults)} defaults: has config.rs changed shape?')

problems = []

example = (root / 'server/.env.example').read_text()
for name in sorted(names):
    if not re.search(rf'^#? ?{name}=', example, re.MULTILINE):
        problems.append(f'server/.env.example: {name} is missing')

for name, value in re.findall(r'^([A-Z0-9_]+)=(.*)$', example, re.MULTILINE):
    if name in defaults and value != defaults[name]:
        problems.append(f'server/.env.example: {name}={value}, but the code defaults to {defaults[name]}')

for document in ('README.md', 'SECURITY.md'):
    rows = re.findall(r'^\| `([A-Z0-9_]+)` \| `([^`]*)` \|', (root / document).read_text(), re.MULTILINE)
    for name, value in rows:
        if name in defaults and value != defaults[name]:
            problems.append(f'{document}: {name} is shown as {value}, but the code defaults to {defaults[name]}')

for problem in problems:
    print(problem)
if problems:
    sys.exit(1)
print(f'{len(names)} settings match the documentation')
