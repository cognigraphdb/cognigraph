"""Regression checks for counting units and rejected malformed evidence."""

import copy
import unittest

from replay import legacy_scorer, score_rows, validate_inputs


class ReplayChecks(unittest.TestCase):
    def setUp(self):
        self.gold = {"contract": {"clause": ["alpha clause", "beta clause", "gamma clause"]}}
        self.pred = {"contract": {"clause": [{"evidence": "alpha clause", "suspect": False}]}}
        self.contracts = ["contract"]

    def score(self, predictions):
        return score_rows(predictions, self.gold, self.contracts, legacy_scorer().matches)["overall"]

    def test_partial_category_retains_unmatched_gold_entries(self):
        result = self.score(self.pred)
        self.assertEqual((result["tp"], result["fp"], result["fn"]), (1, 0, 2))
        legacy, _ = legacy_scorer().score(self.pred, self.gold, self.contracts)
        self.assertEqual(legacy["clause"]["fn"], 0)

    def test_empty_predictions_count_every_gold_entry(self):
        self.assertEqual(self.score({})["fn"], 3)

    def test_duplicate_prediction_cannot_reuse_a_gold_entry(self):
        self.pred["contract"]["clause"] *= 2
        result = self.score(self.pred)
        self.assertEqual((result["tp"], result["fp"], result["fn"]), (1, 1, 2))

    def test_unmatched_prediction_counts_both_fp_and_gold_fn(self):
        self.pred["contract"]["clause"][0]["evidence"] = "unrelated words"
        result = self.score(self.pred)
        self.assertEqual((result["tp"], result["fp"], result["fn"]), (0, 1, 3))

    def test_absent_category_prediction_is_false_positive(self):
        self.gold["contract"]["clause"] = []
        result = self.score(self.pred)
        self.assertEqual((result["tp"], result["fp"], result["fn"]), (0, 1, 0))

    def test_historical_matching_is_containment_or_twelve_word_run(self):
        matches = legacy_scorer().matches
        self.assertTrue(matches("alpha clause", "the alpha clause applies"))
        run = "one two three four five six seven eight nine ten eleven twelve"
        self.assertTrue(matches("before " + run, run + " after"))
        self.assertFalse(matches("before " + run.rsplit(" ", 1)[0], run.rsplit(" ", 1)[0] + " after"))
        self.assertFalse(matches("!!!", "alpha"))

    def test_malformed_inputs_fail_before_scoring(self):
        cases = []
        p = copy.deepcopy(self.pred); p["other"] = {}; cases.append((p, self.gold, self.contracts))
        p = copy.deepcopy(self.pred); p["contract"]["unknown"] = []; cases.append((p, self.gold, self.contracts))
        p = copy.deepcopy(self.pred); p["contract"]["clause"][0]["suspect"] = "false"; cases.append((p, self.gold, self.contracts))
        p = copy.deepcopy(self.pred); p["contract"]["clause"][0]["evidence"] = ""; cases.append((p, self.gold, self.contracts))
        cases.extend([(self.pred, {}, self.contracts), (self.pred, self.gold, self.contracts * 2)])
        for inputs in cases:
            with self.subTest(inputs=inputs), self.assertRaises(ValueError):
                validate_inputs(*inputs, {"clause"})


if __name__ == "__main__":
    unittest.main()
