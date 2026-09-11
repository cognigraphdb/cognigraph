#!/usr/bin/env python3
"""Run directed construction (D12) over a CUAD sample and pull extractions.

Slices each contract into <=28-chunk requests (one completion call each,
per the route's contract), ingests into a per-set space, then reads the
grounded facts back and writes extractions.json for score.py.

Taxonomy: authored against CUAD's own category definitions; the restraint
vocabulary is the deterministic floor — the model classifies, the gates
refuse anything the evidence sentence does not carry.
"""
import argparse
import json
import os
import pathlib
import sys
import time
import urllib.request

ROOT = pathlib.Path(__file__).parent
WORK = ROOT / "work"

TAXONOMY = [
    # FROZEN 2026-07-22 after 3 design-set calibration rounds (F1 0.627 /
    # 0.605 / 0.579): round 1 won, further tuning was fitting run-to-run
    # variance on N=10. Do not edit before quoting holdout numbers.
    {"relation": "GRANTS_EXCLUSIVITY",
     "description": "One party receives exclusive rights — an exclusive license, "
        "exclusive dealing, or a promise not to license/appoint others for the "
        "same scope. Source = the granting/bound party, target = the party "
        "receiving exclusivity or the exclusive scope.",
     "require_in_sentence": ["exclusiv", "solely", "sole "]},
    {"relation": "NON_COMPETE",
     "description": "A party is restricted from competing — operating, selling, "
        "or engaging in a competing business or products.",
     "require_in_sentence": ["compet"]},
    {"relation": "TERMINATES_FOR_CONVENIENCE",
     "description": "A party may terminate WITHOUT cause — for convenience, at "
        "will, or on notice alone. Not termination for breach.",
     "require_in_sentence": ["convenience", "without cause", "at any time",
                              "for any reason", "with or without"]},
    {"relation": "ANTI_ASSIGNMENT",
     "description": "Assignment of the agreement or its rights requires consent "
        "or notice, or is prohibited.",
     "require_in_sentence": ["assign"]},
    {"relation": "AUDIT_RIGHTS",
     "description": "A party may audit or inspect the other's books, records, or "
        "compliance.",
     "require_in_sentence": ["audit", "inspect", "books and records", "examine"]},
    {"relation": "CAP_ON_LIABILITY",
     "description": "Liability is capped or limited — to an amount, to fees "
        "paid, or by excluding damage categories.",
     "require_in_sentence": ["liab"]},
    {"relation": "ASSIGNS_IP",
     "description": "Intellectual property created under or covered by the "
        "agreement becomes the counterparty's property (assignment or "
        "work-made-for-hire), or ownership is expressly allocated.",
     "require_in_sentence": ["intellectual property", "ownership", "work product",
                              "work made for hire", "title to", "shall own",
                              "property of"]},
    {"relation": "GOVERNED_BY",
     "description": "The agreement names its governing law or jurisdiction.",
     "require_in_sentence": ["governed by", "laws of", "law of", "construed"]},
]

CATEGORY_OF = {
    "GRANTS_EXCLUSIVITY": "Exclusivity",
    "NON_COMPETE": "Non-Compete",
    "TERMINATES_FOR_CONVENIENCE": "Termination For Convenience",
    "ANTI_ASSIGNMENT": "Anti-Assignment",
    "AUDIT_RIGHTS": "Audit Rights",
    "CAP_ON_LIABILITY": "Cap On Liability",
    "ASSIGNS_IP": "Ip Ownership Assignment",
    "GOVERNED_BY": "Governing Law",
}

SLICE = 28


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--set", dest="which", choices=["design", "holdout"], required=True)
    ap.add_argument("--url", default="http://localhost:3001")
    ap.add_argument("--extract-only", action="store_true",
                    help="skip ingestion; just pull facts and write extractions")
    args = ap.parse_args()
    space = f"cuad_directed_{args.which}"

    password = os.environ.get("COGNIGRAPH_ADMIN_PASSWORD")
    if not password:
        sys.exit("set COGNIGRAPH_ADMIN_PASSWORD")

    def call(method, path, body=None, t=180):
        req = urllib.request.Request(
            args.url.rstrip("/") + path,
            data=json.dumps(body).encode() if body is not None else None,
            headers={"Content-Type": "application/json",
                     **({"Authorization": f"Bearer {token}"} if path != "/api/auth/login" else {})},
            method=method)
        with urllib.request.urlopen(req, timeout=t) as r:
            return json.loads(r.read())

    token = None
    token = call("POST", "/api/auth/login",
                 {"username": "admin", "password": password})["token"]

    chunks = [json.loads(l) for l in open(WORK / f"{args.which}_chunks.jsonl")]
    by_contract = {}
    for c in chunks:
        by_contract.setdefault(c["title"], []).append(c)

    if not args.extract_only:
        t0 = time.time()
        totals = {"requests": 0, "proposed": 0, "grounded": 0, "skips": 0}
        skip_kinds = {}
        for n, (contract, cs) in enumerate(sorted(by_contract.items()), 1):
            for i in range(0, len(cs), SLICE):
                body = {"space_type": space, "taxonomy": TAXONOMY,
                        "chunks": cs[i:i + SLICE]}
                for attempt in (1, 2):
                    try:
                        res = call("POST", "/api/construct/directed", body)
                        break
                    except Exception as e:  # noqa: BLE001 — retry once, then surface
                        if attempt == 2:
                            raise
                        print(f"   retry {contract[:30]} slice {i}: {e}", file=sys.stderr)
                        time.sleep(5)
                totals["requests"] += 1
                totals["proposed"] += res.get("proposed", 0)
                totals["grounded"] += res.get("facts_grounded", 0)
                for skip in res.get("skips", []):
                    totals["skips"] += 1
                    kind = skip.split(":")[-1].strip()[:40]
                    skip_kinds[kind] = skip_kinds.get(kind, 0) + 1
            print(f"[{n:2d}/{len(by_contract)}] {contract[:52]:52s} "
                  f"grounded so far: {totals['grounded']}")
        print(f"\ningest done in {time.time()-t0:.0f}s: {json.dumps(totals)}")
        print("skip kinds:", json.dumps(skip_kinds, indent=1))

    rows = call("POST", "/api/query", {
        "query": f'FOR f IN facts FILTER f.space_id == "{space}" '
                 'RETURN { relation: f.relation_type, chunk: f.evidence_chunk_id, '
                 '         trigger: f.trigger, key: f._key }',
        "bind_vars": {}}, t=600)
    facts = rows.get("results") or rows.get("result") or []
    suspects = call("POST", "/api/query", {
        "query": 'FOR s IN fact_semantics RETURN s.fact_key', "bind_vars": {}}, t=600)
    suspect_keys = set((suspects.get("results") or suspects.get("result") or []))

    extractions = {}
    n_suspect = 0
    for f in facts:
        contract = f["chunk"].split("::")[0]
        category = CATEGORY_OF.get(f["relation"])
        if not category:
            continue
        if f["key"] in suspect_keys:
            n_suspect += 1
        extractions.setdefault(contract, {}).setdefault(category, []).append(
            {"evidence": f["trigger"], "suspect": f["key"] in suspect_keys})
    out = WORK / f"extractions_{args.which}.json"
    json.dump(extractions, open(out, "w"), indent=1)
    print(f"\nfacts: {len(facts)} ({n_suspect} semantics-flagged) -> {out}")


if __name__ == "__main__":
    main()
