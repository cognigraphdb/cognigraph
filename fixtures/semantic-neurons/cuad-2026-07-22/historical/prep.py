#!/usr/bin/env python3
"""CUAD evaluation prep: pick design/holdout contracts, chunk them, emit gold.

CUAD (Atticus Project, CC-BY 4.0): 510 real commercial contracts with expert
clause annotations. We evaluate governed construction on 8 categories that map
to what pharma compliance evaluators ask about, scoring extracted facts'
EVIDENCE against the expert clause spans.

Outputs (in ./work):
  gold.json            {contract: {category: [clause text, ...]}} for the sample
  design_chunks.jsonl  one chunk per line: {id, title, text}  (design set)
  holdout_chunks.jsonl same for the holdout set
  sample.json          {design: [...], holdout: [...]}
"""
import ast
import csv
import json
import pathlib
import random
import re

ROOT = pathlib.Path(__file__).parent
TXT = ROOT / "CUAD_v1" / "full_contract_txt"
WORK = ROOT / "work"
WORK.mkdir(exist_ok=True)

CATEGORIES = [
    "Exclusivity",
    "Non-Compete",
    "Termination For Convenience",
    "Anti-Assignment",
    "Audit Rights",
    "Cap On Liability",
    "Ip Ownership Assignment",
    "Governing Law",
]
DESIGN_N, HOLDOUT_N, SEED = 10, 40, 20260722


def parse_cell(cell):
    cell = (cell or "").strip()
    if not cell or cell in ("[]", "['']", '[""]'):
        return []
    try:
        val = ast.literal_eval(cell)
        return [v.strip() for v in val if isinstance(v, str) and v.strip()]
    except (ValueError, SyntaxError):
        return [cell]


def chunk(text, size=1800, overlap=200):
    """Paragraph-aware fixed-size chunking; contracts have no better natural
    unit than the paragraph, and clause spans rarely exceed one."""
    paras = re.split(r"\n\s*\n", text)
    chunks, buf = [], ""
    for p in paras:
        p = p.strip()
        if not p:
            continue
        if len(buf) + len(p) + 2 > size and buf:
            chunks.append(buf)
            buf = buf[-overlap:] + "\n\n" + p if overlap else p
        else:
            buf = (buf + "\n\n" + p) if buf else p
    if buf:
        chunks.append(buf)
    return chunks


def main():
    raise ValueError("This evidence package was partially withdrawn after data removal; original reproduction is retired.")
    with open(ROOT / "CUAD_v1" / "master_clauses.csv") as f:
        rows = list(csv.DictReader(f))
    txt_by_stem = {p.stem: p for p in TXT.glob("*.txt")}

    gold, coverage = {}, {}
    for row in rows:
        stem = pathlib.Path(row["Filename"]).stem
        if stem not in txt_by_stem:
            continue
        labels = {c: parse_cell(row.get(c)) for c in CATEGORIES}
        gold[stem] = labels
        coverage[stem] = sum(1 for c in CATEGORIES if labels[c])

    # Stratified sample: draft needs to SEE the phenomena, the holdout needs
    # negatives too. Sort by category coverage, take alternating slices.
    random.seed(SEED)
    ranked = sorted(coverage, key=lambda s: (-coverage[s], s))
    rich, mid, sparse = ranked[:120], ranked[120:300], ranked[300:]
    design = random.sample(rich, 6) + random.sample(mid, 3) + random.sample(sparse, 1)
    remaining = [s for s in ranked if s not in design]
    holdout = (random.sample([s for s in rich if s in remaining], 15)
               + random.sample([s for s in mid if s in remaining], 15)
               + random.sample([s for s in sparse if s in remaining], 10))

    for name, stems in (("design", design), ("holdout", holdout)):
        with open(WORK / f"{name}_chunks.jsonl", "w") as out:
            n = 0
            for stem in stems:
                text = txt_by_stem[stem].read_text(errors="replace")
                for i, piece in enumerate(chunk(text)):
                    out.write(json.dumps(
                        {"id": f"{stem}::{i}", "title": stem, "text": piece}) + "\n")
                    n += 1
        print(f"{name}: {len(stems)} contracts -> {n} chunks")

    json.dump({s: gold[s] for s in design + holdout},
              open(WORK / "gold.json", "w"), indent=1)
    json.dump({"design": design, "holdout": holdout},
              open(WORK / "sample.json", "w"), indent=1)
    per = {c: sum(1 for s in design + holdout if gold[s][c]) for c in CATEGORIES}
    print("category coverage in sample:", per)


if __name__ == "__main__":
    main()
