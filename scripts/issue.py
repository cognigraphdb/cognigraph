#!/usr/bin/env python3
"""Allocate and check CogniGraph issue records.

Issue identities are stable. The only sanctioned way to create one is

    python3 scripts/issue.py new "Title" --priority P2 --area "Server / jobs"

which allocates the next unused CG-N from every source that can hold one
(working tree, index, committed history, and the registry's "Next available identifier"),
creates the file with O_EXCL so an existing file is never overwritten, appends
the registry row and bumps the counter in one step.

    python3 scripts/issue.py next      # print the next identifier only
    python3 scripts/issue.py check     # registry/file consistency; used by CI

`check` fails when an issue file's title differs from the committed one
(an identity was reused), when a file and its registry row disagree, when a
row or file is missing, when identifiers are not contiguous, or when the
counter is stale.
"""
from __future__ import annotations

import argparse
import datetime as dt
import fcntl
import json
import os
import re
import subprocess
import sys
import tempfile
from pathlib import Path

ROOT = Path(os.environ.get("COGNIGRAPH_ROOT") or Path(__file__).resolve().parents[1])
ISSUES = ROOT / "docs" / "issues"
REGISTRY = ISSUES / "README.md"

FILE_RE = re.compile(r"^CG-(\d+)\.md$")
H1_RE = re.compile(r"^# CG-(\d+): (.+?)\s*$")
ROW_RE = re.compile(r"^\| \[CG-(\d+)\]\(CG-(\d+)\.md\) \| (P[0-3]) \| ([^|]+?) \| (.+?) \|\s*$")
NEXT_RE = re.compile(r"^Next available identifier: \*\*CG-(\d+)\*\*\.\s*$", re.M)
STATUS_RE = re.compile(r"^- Status: (.+?)\s*$", re.M)
STATUSES = {"Open", "In progress", "Blocked", "Resolved", "Closed without change"}

TEMPLATE = """# CG-{n}: {title}

- Status: Open
- Priority: {priority}
- Area: {area}
- Found: {today}
- Verification: Pending

## Problem and evidence

Describe the affected behavior, the source or live evidence, and the
reproduction or failure sequence. Source-confirmed findings are distinct from
live reproductions.

## Acceptance

- [ ] State the observable behavior that closes this issue.
- [ ] Name the gate that proves it (tests, real-runtime regression, or link/example verification for documentation).
"""


def git(*args: str) -> str:
    result = subprocess.run(["git", *args], cwd=ROOT, capture_output=True, text=True, check=False)
    if result.returncode:
        raise SystemExit(f"git {args[0]} failed; issue history could not be verified")
    return result.stdout


def ids_in_working_tree() -> set[int]:
    return {int(m.group(1)) for p in ISSUES.iterdir() if (m := FILE_RE.match(p.name))}


def ids_in_git() -> set[int]:
    """Identifiers in committed history, the index, and untracked files."""
    if git("rev-parse", "--is-shallow-repository").strip() == "true":
        raise SystemExit("issue checks require full history; run git fetch --unshallow")
    found: set[int] = set()
    for line in git("log", "--format=", "--name-only", "--diff-filter=A", "--", "docs/issues/").splitlines():
        if m := FILE_RE.match(Path(line).name):
            found.add(int(m.group(1)))
    for line in git("ls-files", "--cached", "--others", "--exclude-standard", "docs/issues/").splitlines():
        if m := FILE_RE.match(Path(line).name):
            found.add(int(m.group(1)))
    return found


def registry_text() -> str:
    return REGISTRY.read_text(encoding="utf-8")


def registry_rows(text: str) -> dict[int, tuple[str, str, str]]:
    rows: dict[int, tuple[str, str, str]] = {}
    for line in text.splitlines():
        if m := ROW_RE.match(line):
            n = int(m.group(1))
            if n != int(m.group(2)):
                raise SystemExit(f"registry link for CG-{n} points to another issue")
            if n in rows:
                raise SystemExit(f"registry lists CG-{n} twice")
            rows[n] = (m.group(3), m.group(4).strip(), m.group(5).strip())
        elif line.startswith("| [CG-"):
            raise SystemExit("registry contains a malformed issue row")
    return rows


def registry_next(text: str) -> int:
    m = NEXT_RE.search(text)
    if not m:
        raise SystemExit("registry has no 'Next available identifier' line")
    return int(m.group(1))


def next_identifier() -> int:
    text = registry_text()
    candidates = ids_in_working_tree() | ids_in_git() | set(registry_rows(text))
    return max(max(candidates, default=0) + 1, registry_next(text))


def original_title(n: int) -> str | None:
    path = f"docs/issues/CG-{n}.md"
    commits = git("log", "--reverse", "--diff-filter=A", "--format=%H", "--", path).splitlines()
    if not commits:
        return None
    blob = git("show", f"{commits[0]}:{path}")
    first = blob.splitlines()[0] if blob.splitlines() else ""
    m = H1_RE.match(first)
    return m.group(2) if m else first


def parse_file(path: Path) -> tuple[int | None, str, str | None, list[str]]:
    errors: list[str] = []
    text = path.read_text(encoding="utf-8")
    first = text.splitlines()[0] if text.strip() else ""
    m = H1_RE.match(first)
    n = int(m.group(1)) if m else None
    title = m.group(2) if m else first
    if not m:
        errors.append(f"{path.name}: first line must be '# CG-N: Title'")
    sm = STATUS_RE.search(text)
    status = sm.group(1) if sm else None
    if status is None:
        errors.append(f"{path.name}: missing '- Status:' line")
    elif status not in STATUSES:
        errors.append(f"{path.name}: unknown status '{status}'")
    return n, title, status, errors


def cmd_check(_: argparse.Namespace) -> int:
    errors: list[str] = []
    text = registry_text()
    rows = registry_rows(text)
    files = {int(FILE_RE.match(p.name).group(1)): p for p in ISSUES.iterdir() if FILE_RE.match(p.name)}
    for n in sorted(ids_in_git() - files.keys()):
        errors.append(f"CG-{n}: recorded issue is missing from the working tree")

    for n, path in sorted(files.items()):
        fid, title, status, errs = parse_file(path)
        errors.extend(errs)
        if fid is not None and fid != n:
            errors.append(f"{path.name}: heading says CG-{fid}")
        if n not in rows:
            errors.append(f"{path.name}: no registry row in docs/issues/README.md")
        else:
            row_priority, row_status, row_title = rows[n]
            priority = re.search(r"^- Priority: (P[0-3])\s*$", path.read_text(encoding="utf-8"), re.M)
            if not priority or priority.group(1) != row_priority:
                errors.append(f"CG-{n}: file priority is missing or differs from registry")
            if status and row_status != status:
                errors.append(f"CG-{n}: file status '{status}' but registry says '{row_status}'")
            if row_title != title:
                errors.append(f"CG-{n}: registry title differs from file title")
        committed = original_title(n)
        if committed is not None and committed != title:
            errors.append(
                f"CG-{n}: title changed from the committed record ('{committed}' -> '{title}'). "
                "Issue identities are stable; allocate a new one with scripts/issue.py new."
            )

    for n in sorted(rows):
        if n not in files:
            errors.append(f"registry row CG-{n} has no docs/issues/CG-{n}.md")

    if files:
        expected = set(range(1, max(files) + 1))
        for gap in sorted(expected - set(files)):
            errors.append(f"CG-{gap} is missing; identifiers must be contiguous")
        want = max(files) + 1
        have = registry_next(text)
        if have != want:
            errors.append(f"registry says next is CG-{have}; highest existing is CG-{max(files)} so it must be CG-{want}")

    print(json.dumps({"issues": len(files), "registry_rows": len(rows), "errors": errors}, indent=2))
    return 1 if errors else 0


def cmd_next(_: argparse.Namespace) -> int:
    print(f"CG-{next_identifier()}")
    return 0


def cmd_new(args: argparse.Namespace) -> int:
    for label, value in (("title", args.title), ("area", args.area)):
        if not value.strip() or "|" in value or any(ord(c) < 32 or ord(c) == 127 for c in value):
            raise SystemExit(f"{label} must be nonempty single-line text without control characters or pipes")
    # Keep a stable lock inode: replacing or unlinking it permits parallel writers.
    with (ISSUES / ".registry.lock").open("a") as lock:
        fcntl.flock(lock, fcntl.LOCK_EX)
        return create_issue(args)


def create_issue(args: argparse.Namespace) -> int:
    n = next_identifier()
    path = ISSUES / f"CG-{n}.md"
    today = dt.date.today().isoformat()
    body = TEMPLATE.format(n=n, title=args.title.strip(), priority=args.priority, area=args.area, today=today)
    text = registry_text()
    row = f"| [CG-{n}](CG-{n}.md) | {args.priority} | Open | {args.title.strip()} |\n"
    lines = text.splitlines(keepends=True)
    last_row = max(i for i, line in enumerate(lines) if ROW_RE.match(line))
    lines.insert(last_row + 1, row)
    text = "".join(lines)
    text = NEXT_RE.sub(f"Next available identifier: **CG-{n + 1}**.", text, count=1)
    fd = os.open(path, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o644)  # never overwrite
    temporary = None
    try:
        with os.fdopen(fd, "w", encoding="utf-8") as fh:
            fh.write(body)
        with tempfile.NamedTemporaryFile(mode="w", encoding="utf-8", dir=ISSUES,
                                         prefix=".registry-", suffix=".tmp", delete=False) as fh:
            temporary = Path(fh.name)
            fh.write(text)
            fh.flush()
            os.fsync(fh.fileno())
        os.chmod(temporary, REGISTRY.stat().st_mode & 0o777)
        os.replace(temporary, REGISTRY)
    except BaseException:
        path.unlink()  # only the file this invocation created with O_EXCL
        raise
    finally:
        if temporary is not None:
            temporary.unlink(missing_ok=True)
    print(f"created {path.relative_to(ROOT)} and registry row; next is CG-{n + 1}")
    return 0


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    sub = parser.add_subparsers(dest="cmd", required=True)
    sub.add_parser("check").set_defaults(fn=cmd_check)
    sub.add_parser("next").set_defaults(fn=cmd_next)
    new = sub.add_parser("new")
    new.add_argument("title")
    new.add_argument("--priority", choices=["P0", "P1", "P2", "P3"], default="P2")
    new.add_argument("--area", default="Unassigned")
    new.set_defaults(fn=cmd_new)
    args = parser.parse_args()
    return args.fn(args)


if __name__ == "__main__":
    sys.exit(main())
