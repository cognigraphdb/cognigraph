#!/usr/bin/env python3
"""Check decision inventory coverage and local index links without dependencies."""

import collections
import json
from pathlib import Path
import re
import sys
import unicodedata
from urllib.parse import unquote, urlsplit


STATUSES = {"Active", "Amended", "Research", "Historical"}
SECTIONS = {
    "Standing conventions",
    "Platform contracts",
    "Construction and review contracts",
    "Governed operations and authority generations",
    "Research evidence and historical priorities",
    "Superseded identity history",
}
LINK = re.compile(r"\[([^\]]+)\]\(([^)]+)\)")


def anchors(path):
    """GitHub-style slugs for the ATX headings used by the owner documents."""
    result = set()
    used = collections.Counter()
    fence = None
    for line in path.read_text().splitlines():
        marker = re.match(r"^\s{0,3}(`{3,}|~{3,})", line)
        if marker:
            value = marker[1]
            if fence is None:
                fence = value
            elif value[0] == fence[0] and len(value) >= len(fence):
                fence = None
            continue
        if fence is not None:
            continue
        heading = re.match(r"^#{1,6}\s+(.+?)(?:\s+#+)?\s*$", line)
        if not heading:
            continue
        label = LINK.sub(r"\1", heading[1])
        label = re.sub(r"<[^>]*>", "", label).lower()
        slug = "".join(
            char for char in label
            if char in "-_ " or not unicodedata.category(char).startswith(("P", "S"))
        ).replace(" ", "-")
        unique = slug
        while unique in result:
            used[slug] += 1
            unique = f"{slug}-{used[slug]}"
        result.add(unique)
    return result


def check_index(root):
    index = root / "docs/decisions/README.md"
    content = index.read_text()
    errors = []
    inventory = collections.Counter()
    categories = collections.Counter()
    statuses = collections.Counter()
    section = ""
    for number, line in enumerate(content.splitlines(), 1):
        if line.startswith("## "):
            section = line[3:]
        if not line.startswith("| ["):
            continue
        cells = [cell.strip() for cell in line.strip("|").split("|")]
        first = LINK.fullmatch(cells[0])
        if not first or not re.fullmatch(r"decision_[\w]+\.md", first[2]):
            errors.append(f"line {number}: invalid decision row")
            continue
        inventory[first[2]] += 1
        categories[section] += 1
        if len(cells) != 4 or not all(cells):
            errors.append(f"line {number}: expected four nonempty decision cells")
            continue
        statuses[cells[1]] += 1
        if cells[1] not in STATUSES:
            errors.append(f"line {number}: unknown status {cells[1]!r}")
        if section not in SECTIONS:
            errors.append(f"line {number}: decision outside an inventory section")

    expected = {path.name for path in index.parent.glob("decision_*.md")}
    for name in sorted(expected - inventory.keys()):
        errors.append(f"unindexed decision: {name}")
    for name in sorted(inventory.keys() - expected):
        errors.append(f"missing decision file: {name}")
    for name, count in sorted(inventory.items()):
        if count != 1:
            errors.append(f"duplicate decision: {name} ({count} rows)")
    for name in sorted(SECTIONS - categories.keys()):
        errors.append(f"empty inventory section: {name}")

    local_links = 0
    checked_anchors = 0
    external_links = 0
    optional_product_links = 0
    anchor_cache = {}
    for _, destination in LINK.findall(content):
        target = urlsplit(destination.strip("<>"))
        if target.scheme or target.netloc:
            external_links += 1
            continue
        path = (index.parent / unquote(target.path)).resolve() if target.path else index
        if path.is_relative_to(root.resolve().parent / "docs"):
            # Business/publication docs have a separate, optional checkout.
            # `check-docs.py --include-product` validates these when requested.
            optional_product_links += 1
            continue
        local_links += 1
        if not path.exists():
            errors.append(f"missing local link: {destination}")
        elif target.fragment and path.suffix == ".md":
            checked_anchors += 1
            if path not in anchor_cache:
                anchor_cache[path] = anchors(path)
            if unquote(target.fragment) not in anchor_cache[path]:
                errors.append(f"missing Markdown anchor: {destination}")

    return {
        "decisions": len(expected),
        "inventory_rows": sum(inventory.values()),
        "categories": dict(categories),
        "statuses": dict(statuses),
        "local_links_checked": local_links,
        "markdown_anchors_checked": checked_anchors,
        "external_links_not_checked": external_links,
        "optional_product_links_not_checked": optional_product_links,
        "errors": errors,
    }


if __name__ == "__main__":
    report = check_index(Path(__file__).resolve().parents[1])
    print(json.dumps(report, indent=2))
    sys.exit(bool(report["errors"]))
