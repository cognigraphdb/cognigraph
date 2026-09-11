import unittest

from score_v2 import attributed_prediction_keys, metrics, reference_keys


class AttributionTests(unittest.TestCase):
    def test_wrong_chunk_cannot_receive_reference_credit(self):
        reference = {'entities': [{'id': 0, 'aliases': ['A']}, {'id': 1, 'aliases': ['B']}],
                     'relations': [{'head': 0, 'relation': 'OWNED_BY', 'tail': 1}]}
        proposal = {'source': 'A', 'target': 'B', 'relation': 'OWNED_BY', 'chunk_id': 'chunk document'}
        result = metrics(reference_keys(reference, set()),
                         attributed_prediction_keys([proposal], reference, set(), 'document'))
        self.assertEqual((result['matched'], result['unmatched'], result['missed']), (0, 1, 1))
        proposal['chunk_id'] = 'document'
        result = metrics(reference_keys(reference, set()),
                         attributed_prediction_keys([proposal], reference, set(), 'document'))
        self.assertEqual((result['matched'], result['unmatched'], result['missed']), (1, 0, 0))

    def test_wrong_chunk_duplicates_do_not_inflate_denominator(self):
        reference = {'entities': [], 'relations': []}
        proposal = {'source': 'A', 'target': 'B', 'relation': 'OWNED_BY', 'chunk_id': 'wrong'}
        self.assertEqual(len(attributed_prediction_keys([proposal, proposal], reference, set(), 'document')), 1)


if __name__ == '__main__':
    unittest.main()
