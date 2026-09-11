"""Scoring invariants which would have caught the old mixed-grain CUAD bug."""
import unittest

from prepare import prepare
from score import identity, metrics


class Scoring(unittest.TestCase):
    def test_partial_category_match_counts_each_missed_triple(self):
        a, b, c = [('chunk', x, 'R', 'z') for x in ('a', 'b', 'c')]
        result = metrics({a, b, c}, {a})
        self.assertEqual((result['tp'], result['fp'], result['fn']), (1, 0, 2))
        self.assertEqual(result['recall'], 1 / 3)

    def test_chunk_and_direction_are_part_of_identity(self):
        gold = {identity('one', 'A', 'R', 'B')}
        predicted = {identity('two', 'A', 'R', 'B'), identity('one', 'B', 'R', 'A')}
        self.assertEqual(metrics(gold, predicted)['fp'], 2)
        self.assertEqual(metrics(gold, predicted)['fn'], 1)

    def test_no_fuzzy_substring_credit(self):
        gold = {identity('one', 'Alpha', 'R', 'B')}
        self.assertEqual(metrics(gold, {identity('one', 'Alph', 'R', 'B')})['tp'], 0)

    def test_nfc_case_and_whitespace_policy(self):
        self.assertEqual(identity('one', 'CAFE\u0301', 'R', ' A  B '),
                         identity('one', 'café', 'R', 'a b'))

    def test_undefined_rates_are_null(self):
        self.assertEqual(metrics(set(), set()), {'tp': 0, 'fp': 0, 'fn': 0,
            'precision': None, 'recall': None, 'f1': None})

    def test_fixture_counts_and_negative_coverage(self):
        corpus = prepare()
        self.assertEqual(len(corpus['cases']), 48)
        self.assertEqual(sum(len(c['gold']) for c in corpus['cases']), 32)
        self.assertEqual(sum(not c['gold'] for c in corpus['cases']), 20)
        self.assertEqual(len({c['id'] for c in corpus['cases']}), 48)


if __name__ == '__main__':
    unittest.main()
