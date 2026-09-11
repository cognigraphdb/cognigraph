#!/usr/bin/env python3
"""Offline CUAD evidence replay. Never contacts a model or a CogniGraph server."""

import collections
import hashlib
import importlib.util
import json
from pathlib import Path
import sys

sys.dont_write_bytecode = True
ROOT = Path(__file__).resolve().parent


def read(name):
    return json.loads((ROOT / name).read_text())


def legacy_scorer():
    spec = importlib.util.spec_from_file_location("cuad_legacy_score", ROOT / "historical/score.py")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def rates(counts):
    tp, fp, fn = (counts[key] for key in ("tp", "fp", "fn"))
    return dict(counts, precision=tp / (tp + fp) if tp + fp else None,
                recall=tp / (tp + fn) if tp + fn else None,
                f1=2 * tp / (2 * tp + fp + fn) if 2 * tp + fp + fn else None)


def score_rows(predictions, gold, contracts, matches):
    """Preserve historical greedy matching/order; use one gold-entry grain for FN."""
    categories = collections.defaultdict(lambda: {"tp": 0, "fp": 0, "fn": 0})
    ledger = []
    matched_categories = 0
    for contract in contracts:
        for category, spans in gold[contract].items():
            preds = predictions.get(contract, {}).get(category, [])
            used = set()
            assignments = []
            for index, prediction in enumerate(preds):
                hit = next((i for i, span in enumerate(spans)
                            if i not in used and matches(prediction["evidence"], span)), None)
                if hit is not None:
                    used.add(hit)
                assignments.append({"prediction_index": index, "gold_index": hit})
            counts = {"tp": len(used), "fp": len(preds) - len(used), "fn": len(spans) - len(used)}
            for key, value in counts.items():
                categories[category][key] += value
            matched_categories += bool(used)
            ledger.append(dict(contract=contract, category=category, gold_entries=len(spans),
                               predictions=len(preds), assignments=assignments,
                               unmatched_gold_indices=[i for i in range(len(spans)) if i not in used],
                               **counts))
    total = {key: sum(row[key] for row in categories.values()) for key in ("tp", "fp", "fn")}
    return {"overall": rates(total), "per_category": {key: rates(value) for key, value in sorted(categories.items())},
            "matched_contract_categories": matched_categories, "ledger": ledger}


def validate_inputs(predictions, gold, contracts, categories):
    if len(contracts) != len(set(contracts)):
        raise ValueError("duplicate split contract")
    if set(predictions) - set(contracts):
        raise ValueError("prediction outside the selected split")
    for contract in contracts:
        if contract not in gold or set(gold[contract]) != categories:
            raise ValueError("missing gold contract/category")
        if set(predictions.get(contract, {})) - categories:
            raise ValueError("unknown predicted category")
        for spans in gold[contract].values():
            if not isinstance(spans, list) or any(not isinstance(s, str) or not s.strip() for s in spans):
                raise ValueError("invalid gold span")
        for rows in predictions.get(contract, {}).values():
            if not isinstance(rows, list):
                raise ValueError("invalid predictions")
            for row in rows:
                if (not isinstance(row, dict) or set(row) != {"evidence", "suspect"}
                        or not isinstance(row["evidence"], str) or not row["evidence"].strip()
                        or type(row["suspect"]) is not bool):
                    raise ValueError("invalid prediction projection")


def replay():
    raise ValueError("This evidence package was partially withdrawn after data removal; original reproduction is retired.")
    manifest = read("provenance.json")
    if hashlib.sha256((ROOT / "taxonomy.json").read_bytes()).hexdigest() != manifest["derived_taxonomy_sha256"]:
        raise ValueError("frozen taxonomy changed")
    for name, expected in manifest["historical_files"].items():
        data = (ROOT / "historical" / name).read_bytes()
        if len(data) != expected["bytes"] or hashlib.sha256(data).hexdigest() != expected["sha256"]:
            raise ValueError(f"historical artifact changed: {name}")
    scorer = legacy_scorer()
    sample = read("historical/work/sample.json")
    gold = read("historical/work/gold.json")
    categories = set(read("taxonomy.json")["category_of"].values())
    if set(sample) != {"design", "holdout"} or len(sample["design"]) != 10 or len(sample["holdout"]) != 40:
        raise ValueError("unexpected split shape")
    if set(sample["design"]) & set(sample["holdout"]) or set(gold) != set(sample["design"] + sample["holdout"]):
        raise ValueError("split overlap or gold coverage mismatch")
    results = {"policy": "legacy-greedy-evidence-matching-with-gold-entry-FN-v1",
               "model_calls": 0, "splits": {}}
    for split, contracts in sample.items():
        predictions = read(f"historical/work/extractions_{split}.json")
        validate_inputs(predictions, gold, contracts, categories)
        chunks = [json.loads(line) for line in (ROOT / f"historical/work/{split}_chunks.jsonl").read_text().splitlines()]
        if len({c["id"] for c in chunks}) != len(chunks) or {c["title"] for c in chunks} != set(contracts):
            raise ValueError("chunk identity/split mismatch")
        profile = {
            "contracts": len(contracts), "chunks": len(chunks), "categories": len(categories),
            "gold_entries": sum(len(spans) for c in contracts for spans in gold[c].values()),
            "positive_contract_categories": sum(bool(spans) for c in contracts for spans in gold[c].values()),
            "exact_duplicate_gold_entries_within_category": sum(len(spans) - len(set(spans)) for c in contracts for spans in gold[c].values()),
            "prediction_rows": sum(len(rows) for cats in predictions.values() for rows in cats.values()),
            "contracts_without_predictions": [c for c in contracts if c not in predictions],
        }
        lanes = {}
        for lane in ("all", "without_semantics_flags"):
            selected = predictions if lane == "all" else {
                c: {cat: [row for row in rows if not row["suspect"]] for cat, rows in cats.items()}
                for c, cats in predictions.items()}
            legacy, _ = scorer.score(selected, gold, contracts)
            legacy_total = {key: sum(row[key] for row in legacy.values()) for key in ("tp", "fp", "fn")}
            corrected = score_rows(selected, gold, contracts, scorer.matches)
            if corrected["overall"]["tp"] != legacy_total["tp"] or corrected["overall"]["fp"] != legacy_total["fp"]:
                raise ValueError("replay changed historical assignments")
            if corrected["overall"]["tp"] + corrected["overall"]["fn"] != profile["gold_entries"]:
                raise ValueError("gold-entry denominator mismatch")
            lanes[lane] = {"legacy_mixed_units": {"overall": rates(legacy_total), "per_category": legacy},
                           "consistent_gold_entry_units": corrected}
        results["splits"][split] = {"profile": profile, "lanes": lanes}
    return results


if __name__ == "__main__":
    import argparse
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, help="write full count/assignment ledger to a new file")
    args = parser.parse_args()
    result = replay()
    if args.output:
        with args.output.open("x") as out:
            json.dump(result, out, indent=2)
            out.write("\n")
    for split, data in result["splits"].items():
        print(split, json.dumps(data["profile"]))
        for lane, scores in data["lanes"].items():
            for policy, score in scores.items():
                print(lane, policy, json.dumps(score["overall"]))
