#!/usr/bin/env python3
"""Score extracted clause facts against CUAD's expert annotations.

Input: extractions.json — {contract: {category: [{"evidence": "..."} ...]}}
where `evidence` is the sentence(s) the pipeline grounded the fact in.
Gold: work/gold.json from prep.py.

A prediction is a true positive when its evidence and a gold clause span
overlap materially: the normalized forms share a 12+ word contiguous run, or
one contains the other. Per category and overall we report precision, recall
and F1, plus the per-contract detail for error reading. Design and holdout
are scored separately — the holdout number is the only one that may be
quoted (same discipline as the FDA-label pilot).
"""
import json
import pathlib
import re
import sys

ROOT = pathlib.Path(__file__).parent
WORK = ROOT / "work"


def norm(s):
    return re.sub(r"\s+", " ", re.sub(r"[^a-z0-9 ]", " ", s.lower())).strip()


def shares_run(a, b, k=12):
    """Do the two normalized strings share a contiguous run of k words?"""
    aw, bw = a.split(), b.split()
    if len(aw) < k or len(bw) < k:
        return (a in b) or (b in a) if a and b else False
    grams = {" ".join(aw[i:i + k]) for i in range(len(aw) - k + 1)}
    return any(" ".join(bw[i:i + k]) in grams for i in range(len(bw) - k + 1))


def matches(evidence, gold_span):
    e, g = norm(evidence), norm(gold_span)
    if not e or not g:
        return False
    return e in g or g in e or shares_run(e, g)


def score(extractions, gold, contracts):
    per_cat = {}
    detail = []
    for contract in contracts:
        gold_cats = gold.get(contract, {})
        pred_cats = extractions.get(contract, {})
        for cat in gold_cats:
            stat = per_cat.setdefault(cat, {"tp": 0, "fp": 0, "fn": 0})
            spans = gold_cats.get(cat) or []
            preds = pred_cats.get(cat) or []
            used = set()
            for p in preds:
                hit = next((i for i, s in enumerate(spans)
                            if i not in used and matches(p.get("evidence", ""), s)), None)
                if hit is None:
                    stat["fp"] += 1
                    detail.append(("FP", contract, cat, p.get("evidence", "")[:100]))
                else:
                    used.add(hit)
                    stat["tp"] += 1
            # Category-level recall: the clause was present and we said nothing.
            if spans and not preds:
                stat["fn"] += 1
                detail.append(("FN", contract, cat, spans[0][:100]))
            elif spans and preds and not used:
                stat["fn"] += 1
    return per_cat, detail


def main():
    extractions = json.load(open(sys.argv[1]))
    which = sys.argv[2] if len(sys.argv) > 2 else "design"
    gold = json.load(open(WORK / "gold.json"))
    sample = json.load(open(WORK / "sample.json"))
    contracts = sample[which]
    per_cat, detail = score(extractions, gold, contracts)

    print(f"=== {which} set: {len(contracts)} contracts ===")
    tot = {"tp": 0, "fp": 0, "fn": 0}
    for cat, s in sorted(per_cat.items()):
        for k in tot:
            tot[k] += s[k]
        p = s["tp"] / (s["tp"] + s["fp"]) if s["tp"] + s["fp"] else None
        r = s["tp"] / (s["tp"] + s["fn"]) if s["tp"] + s["fn"] else None
        print(f"{cat:28s} tp={s['tp']:3d} fp={s['fp']:3d} fn={s['fn']:3d}"
              f"  P={p:.2f}" if p is not None else f"{cat:28s} —", end="")
        print(f"  R={r:.2f}" if r is not None else "")
    p = tot["tp"] / (tot["tp"] + tot["fp"]) if tot["tp"] + tot["fp"] else 0
    r = tot["tp"] / (tot["tp"] + tot["fn"]) if tot["tp"] + tot["fn"] else 0
    f1 = 2 * p * r / (p + r) if p + r else 0
    print(f"\nOVERALL  P={p:.3f}  R={r:.3f}  F1={f1:.3f}   (tp={tot['tp']} fp={tot['fp']} fn={tot['fn']})")
    for kind, contract, cat, text in detail[:12]:
        print(f"   {kind}  {contract[:36]:36s} {cat:24s} {text}")


if __name__ == "__main__":
    main()
