#!/usr/bin/env python3
"""Validate maintained local Markdown links and the changelog, without dependencies.

Historical plan snapshots and frozen evidence retain their original paths.
The sibling product checkout is optional unless --include-product is requested.
"""

import argparse
import importlib.util
import json
from pathlib import Path
import re
import subprocess
import sys
from urllib.parse import unquote, urlsplit

ROOT = Path(__file__).resolve().parents[1]
PRODUCT = ROOT.parent / "docs"
LINK = re.compile(r"!?\[[^\]\n]*\]\((<[^>]*>|[^)\n]+)\)")
spec = importlib.util.spec_from_file_location("decision_index", ROOT / "scripts/check-decision-index.py")
decision_index = importlib.util.module_from_spec(spec)
sys.dont_write_bytecode = True
spec.loader.exec_module(decision_index)


def active_lines(text):
    fence = None
    for number, line in enumerate(text.splitlines(), 1):
        marker = re.match(r"^\s{0,3}(`{3,}|~{3,})", line)
        if marker:
            if fence is None:
                fence = marker[1]
            elif marker[1][0] == fence[0] and len(marker[1]) >= len(fence):
                fence = None
        elif fence is None:
            # Backtick literals are source references, not Markdown navigation.
            yield number, re.sub(r"(`+).*?\1", "", line)


def changelog():
    records = []
    errors = []
    for path in sorted((ROOT / "docs/changelog").glob("*.md")):
        if path.name == "README.md":
            continue
        text = path.read_text()
        title = re.match(r"# (.+)\n", text)
        fields = dict(re.findall(r"^- (Date|Status|Kind): (.+)$", text, re.M))
        if not title or set(fields) != {"Date", "Status", "Kind"}:
            errors.append(f"{path.name}: missing title, Date, Status or Kind")
            continue
        date = fields["Date"]
        if date != "undated":
            from datetime import date as calendar_date
            try:
                calendar_date.fromisoformat(date)
            except ValueError:
                errors.append(f"{path.name}: invalid date {date}")
        if not path.name.startswith(date + "-"):
            errors.append(f"{path.name}: filename does not match Date")
        if not re.fullmatch(r"Unreleased|Historical|v\d+\.\d+\.\d+", fields["Status"]):
            errors.append(f"{path.name}: invalid Status")
        if not re.search(r"^## \S", text, re.M):
            errors.append(f"{path.name}: missing record body")
        records.append({"path": path, "title": title[1], **fields})
    return records, errors


def render_index(records):
    out = ["# Changelog", "", "One file per logical change; release summaries link the records they include.",
           "Historical text retains its original dates, units and limitations. A dated",
           "Unreleased record is not a claim that it shipped with the current Cargo version.", "",
           "## Adding a record", "",
           "Create `YYYY-MM-DD-short-description.md` with a title and `Date`, `Status`",
           "and `Kind` metadata. Status is `Unreleased`, a release such as `v2.6.0`,",
           "or `Historical`. Include the behavior change, relevant compatibility notes,",
           "issue/decision links and validation evidence. Use `undated` only for source",
           "history whose date cannot be established. At release time, update selected",
           "records' Status and add a dated release summary linking them. Do not edit",
           "frozen measurement conclusions when assigning a release.", "", "```sh",
           "python3 scripts/check-docs.py --write-changelog-index",
           "python3 scripts/check-docs.py", "```", "",
           "The [migration manifest](../plans/documentation-migration-2026-09-10.json)",
           "maps the original 105 records to their checkpoint source lines and hashes.", ""]
    for heading, predicate in [
        ("Unreleased", lambda r: r["Status"] == "Unreleased"),
        ("Releases", lambda r: r["Status"].startswith("v")),
        ("Historical records", lambda r: r["Status"] == "Historical"),
    ]:
        out += ["## " + heading, ""]
        selected = sorted((r for r in records if predicate(r)), key=lambda r: (r["Date"] != "undated", r["Date"], r["path"].name), reverse=True)
        out += [f'- {r["Date"]}: [{r["title"]}]({r["path"].name})' for r in selected]
        out.append("")
    return "\n".join(out)


def documents(include_product):
    paths = [ROOT / name for name in ("README.md", "CHANGELOG.md", "AGENTS.md")]
    files = subprocess.check_output(
        ["git", "ls-files", "--cached", "--others", "--exclude-standard", "-z"], cwd=ROOT
    ).decode().split("\0")
    for name in files:
        path = ROOT / name
        if path.is_file() and path.suffix == ".md" and name.startswith(("docs/", "crates/", "ui/", ".claude/skills/", ".agents/skills/")):
            relative = path.relative_to(ROOT)
            if any(part in relative.parts for part in ("node_modules", "dist", "target", "evidence")):
                continue
            if path.is_relative_to(ROOT / "docs/plans/archive"):
                # The archive index remains maintained; snapshots are historical.
                if path != ROOT / "docs/plans/archive/README.md":
                    continue
            paths.append(path)
    if include_product:
        paths.extend(p for p in PRODUCT.rglob("*.md")
                     if not p.is_relative_to(PRODUCT / "archive") or p == PRODUCT / "archive/README.md")
    return sorted(set(paths))


def check(include_product=False, write_index=False):
    records, errors = changelog()
    index = ROOT / "docs/changelog/README.md"
    expected = render_index(records)
    if write_index:
        index.write_text(expected)
    elif not index.exists() or index.read_text() != expected:
        errors.append("changelog index is stale; run with --write-changelog-index")
    if include_product and not (PRODUCT / "README.md").is_file():
        errors.append("--include-product requires the sibling product docs checkout")
    cache = {}
    links = 0
    optional_links = 0
    paths = documents(include_product)
    for source in paths:
        for number, line in active_lines(source.read_text()):
            for raw in LINK.findall(line):
                value = raw[1:raw.index(">")] if raw.startswith("<") else raw.split(' "', 1)[0].split(" '", 1)[0]
                try:
                    target = urlsplit(value)
                except ValueError:
                    errors.append(f"{source.relative_to(ROOT.parent)}:{number}: malformed link {raw}")
                    continue
                if target.scheme or target.netloc:
                    continue
                dest = (source.parent / unquote(target.path)).resolve() if target.path else source
                if dest.is_relative_to(PRODUCT) and not include_product:
                    optional_links += 1
                    continue
                links += 1
                label = f"{source.relative_to(ROOT.parent)}:{number}"
                if not dest.exists():
                    errors.append(f"{label}: missing {value}")
                elif target.fragment and dest.suffix == ".md":
                    if dest not in cache:
                        cache[dest] = decision_index.anchors(dest)
                    if unquote(target.fragment) not in cache[dest]:
                        errors.append(f"{label}: missing anchor {value}")
    return {"documents": len(paths), "local_links_checked": links,
            "optional_product_links_skipped": optional_links,
            "changelog_records": len(records), "errors": errors}


if __name__ == "__main__":
    sys.dont_write_bytecode = True
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--include-product", action="store_true")
    parser.add_argument("--write-changelog-index", action="store_true")
    args = parser.parse_args()
    report = check(args.include_product, args.write_changelog_index)
    print(json.dumps(report, indent=2))
    sys.exit(bool(report["errors"]))
