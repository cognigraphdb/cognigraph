#!/usr/bin/env python3
"""Install this checkout's tracked Git hooks without replacing another hook setup."""
from pathlib import Path
import subprocess

root = Path(__file__).resolve().parents[1]
result = subprocess.run(['git', 'config', '--get', 'core.hooksPath'], cwd=root,
                        capture_output=True, text=True, check=False)
if result.returncode not in (0, 1):
    raise SystemExit('Cannot inspect existing Git hook configuration')
current = result.stdout.strip()
if current and current != '.githooks':
    raise SystemExit('Existing core.hooksPath differs; integrate it before installing')
if not current:
    hook_dir = Path(subprocess.check_output(['git', 'rev-parse', '--git-path', 'hooks'],
                                           cwd=root, text=True).strip())
    if not hook_dir.is_absolute():
        hook_dir = root / hook_dir
    active = [p.name for p in hook_dir.iterdir()
              if p.is_file() and not p.name.endswith('.sample')] if hook_dir.exists() else []
    if active:
        raise SystemExit('Existing Git hooks need integration: ' + ', '.join(active))
hook = root / '.githooks/pre-push'
if not hook.is_file() or not hook.stat().st_mode & 0o111:
    raise SystemExit('Tracked pre-push hook is missing or not executable')
subprocess.run(['git', 'config', '--local', 'core.hooksPath', '.githooks'], cwd=root, check=True)
print('Installed .githooks/pre-push for this checkout')
