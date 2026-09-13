#!/usr/bin/env python3
"""Check lexical and resolved edit paths; never read the target file's content."""
import json
import os
from pathlib import Path
import sys


def protected(path):
    parts = path.parts
    if any(part in ('secrets', '.git') for part in parts):
        return True
    return any(part == '.env' or (part.startswith('.env.')
               and not (index == len(parts) - 1 and part == '.env.example'))
               for index, part in enumerate(parts))


def validate(payload):
    value = payload.get('tool_input', {}).get('file_path')
    if not isinstance(value, str) or not value:
        raise ValueError('Edit hook requires a non-empty file_path')
    path = Path(value).expanduser()
    if not path.is_absolute():
        path = Path(payload.get('cwd') or os.environ.get('CLAUDE_PROJECT_DIR') or os.getcwd()) / path
    if protected(path) or protected(path.resolve()):
        raise ValueError('Target is protected; do not edit environment, secrets or Git metadata directly')


if __name__ == '__main__':
    try:
        validate(json.load(sys.stdin))
    except (ValueError, TypeError, AttributeError, OSError, RuntimeError) as error:
        print(f'Blocked: {error}', file=sys.stderr)
        raise SystemExit(2)
