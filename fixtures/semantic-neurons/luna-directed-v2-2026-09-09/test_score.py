import unittest
from copy import deepcopy

from common import canonical_relation, normalize
from score import aliases, metrics, prediction_keys, reference_keys, resolve


class ScoringTests(unittest.TestCase):
    def setUp(self):
        self.reference = {'entities': [
            {'id': 0, 'aliases': ['Alpine', 'Alpine Ltd']},
            {'id': 1, 'aliases': ['Bramble', 'Bramble Inc']}],
            'relations': [{'head': 0, 'relation': 'OWNED_BY', 'tail': 1}]}

    def test_direction_is_not_interchangeable(self):
        predicted = prediction_keys([{'source': 'Bramble', 'relation': 'OWNED_BY', 'target': 'Alpine'}], self.reference, set())
        self.assertEqual(metrics(reference_keys(self.reference, set()), predicted)['matched'], 0)
        self.assertEqual(metrics(reference_keys(self.reference, set()), predicted)['missed'], 1)

    def test_aliases_and_duplicates_match_one_reference(self):
        predicted = prediction_keys([{'source': source, 'relation': 'OWNED_BY', 'target': 'bramble inc'}
                                     for source in ['Alpine', 'ALPINE LTD']], self.reference, set())
        self.assertEqual(len(predicted), 1)
        self.assertEqual(metrics(reference_keys(self.reference, set()), predicted)['reference_f1'], 1)

    def test_ambiguous_alias_is_not_resolved_using_gold(self):
        reference = deepcopy(self.reference)
        reference['entities'][1]['aliases'].append('Alpine')
        self.assertEqual(resolve('Alpine', aliases(reference)), 'ambiguous:alpine')
        self.assertEqual(resolve('Alpin', aliases(reference)), 'unmapped:alpin')

    def test_only_declared_symmetric_relation_can_reverse(self):
        self.assertEqual(canonical_relation('e:1', 'SPOUSE_OF', 'e:0', {'SPOUSE_OF'}), ('e:0', 'SPOUSE_OF', 'e:1'))
        self.assertEqual(canonical_relation('e:1', 'HAS_CHILD', 'e:0', {'SPOUSE_OF'}), ('e:1', 'HAS_CHILD', 'e:0'))

    def test_empty_and_failed_output_keep_gold_denominator(self):
        result = metrics(reference_keys(self.reference, set()), set())
        self.assertIsNone(result['reference_precision'])
        self.assertEqual(result['reference_recall'], 0)
        self.assertEqual(result['missed'], 1)

    def test_extra_prediction_is_counted(self):
        predicted = prediction_keys([{'source': 'Unknown', 'relation': 'OWNED_BY', 'target': 'Bramble'}], self.reference, set())
        self.assertEqual(metrics(reference_keys(self.reference, set()), predicted)['unmatched'], 1)

    def test_unicode_normalization_does_not_change_direction(self):
        self.assertEqual(normalize('  Cafe\u0301\nNord  '), normalize('Café Nord'))


if __name__ == '__main__':
    unittest.main()
