"""CG-98: the embedding recall benchmark selects distinct drugs
deterministically, chunks labels without losing text, refuses queries that
leak a drug name, reads ranks only from the legs being measured and reports
metrics with reproducible bootstrap intervals."""
import importlib.util
from pathlib import Path
import unittest

ROOT = Path(__file__).resolve().parents[2]
spec = importlib.util.spec_from_file_location(
    'embedding_recall_benchmark', ROOT / 'docs/verification/harnesses/embedding-recall-benchmark.py')
bench = importlib.util.module_from_spec(spec)
spec.loader.exec_module(bench)


class DrugKey(unittest.TestCase):
    def test_first_ingredient_word_before_the_labeler(self):
        self.assertEqual(bench.drug_key('BETAMETHASONE DIPROPIONATE CREAM, AUGMENTED [SUN PHARMA]'), 'BETAMETHASONE')
        self.assertEqual(bench.drug_key('Lipitor (atorvastatin calcium) tablet [Pfizer]'), 'LIPITOR')
        self.assertEqual(bench.drug_key('  [ODD] '), '')

    def test_name_tokens_drop_dosage_forms_salts_and_short_words(self):
        tokens = bench.name_tokens('AMITRIPTYLINE HCL TABLET, FILM COATED [DIRECT_RX]')
        self.assertEqual(tokens, {'AMITRIPTYLINE'})
        tokens = bench.name_tokens('Lipitor (atorvastatin calcium) tablet [Pfizer]')
        self.assertEqual(tokens, {'LIPITOR', 'ATORVASTATIN'})


class Selection(unittest.TestCase):
    rows = [{'set_id': f's{i}', 'title': title} for i, title in enumerate([
        'ALPHA TABLET [A]', 'ALPHA CAPSULE [B]', 'BETA CREAM [C]', 'GAMMA HCL TABLET [D]', 'DELTA [E]'])]

    def test_rank_is_seeded_and_distinct_by_drug(self):
        first = bench.select_labels(self.rows, 'seed-1', 3, eligible=lambda row: True)
        self.assertEqual(first, bench.select_labels(list(reversed(self.rows)), 'seed-1', 3, eligible=lambda r: True))
        keys = [bench.drug_key(next(r['title'] for r in self.rows if r['set_id'] == s)) for s in first]
        self.assertEqual(len(keys), len(set(keys)))
        self.assertNotEqual(first, bench.select_labels(self.rows, 'seed-2', 3, eligible=lambda r: True))

    def test_ineligible_labels_are_skipped_and_short_supply_is_an_error(self):
        chosen = bench.select_labels(self.rows, 's', 2, eligible=lambda row: row['set_id'] != 's2')
        self.assertNotIn('s2', chosen)
        with self.assertRaises(ValueError):
            bench.select_labels(self.rows, 's', 5, eligible=lambda row: True)


class Chunking(unittest.TestCase):
    def test_sections_split_at_sentences_under_the_limit_and_keep_every_word(self):
        doc = {'set_id': 'x', 'sections': [
            {'title': '1 INDICATIONS', 'text': 'One two three. Four five six. Seven eight nine.'},
            {'title': '2 DOSAGE', 'text': 'A' * 50},
            {'title': 'EMPTY', 'text': '   '},
        ]}
        chunks = bench.chunk_label(doc, max_chars=30)
        self.assertTrue(all(len(c['text']) <= 30 for c in chunks), chunks)
        self.assertEqual([c['_key'] for c in chunks], [f'x-{i:03d}' for i in range(len(chunks))])
        self.assertTrue(all(c['document_id'] == 'labels/x' for c in chunks))
        joined = ' '.join(c['text'] for c in chunks)
        for word in ('One', 'three.', 'Four', 'nine.', '1 INDICATIONS', '2 DOSAGE'):
            self.assertIn(word, joined)
        dosage = [c['text'] for c in chunks if c['section'] == '2 DOSAGE']
        self.assertEqual(sum(t.split('\n', 1)[1].count('A') for t in dosage), 50)
        self.assertNotIn('EMPTY', joined)


class Leaks(unittest.TestCase):
    def test_word_prefix_accent_and_case_insensitive(self):
        tokens = {'ATORVASTATIN', 'LIPITOR'}
        self.assertEqual(bench.leaks('Pill like lipitor for cholesterol', tokens), ['LIPITOR'])
        self.assertEqual(bench.leaks('Comprimido de atorvastatína', tokens), ['ATORVASTATIN'])
        self.assertEqual(bench.leaks('statins for high cholesterol', tokens), [])
        self.assertEqual(bench.leaks('LIPITORS', tokens), ['LIPITOR'])
        self.assertEqual(bench.leaks('a relipitor mention', tokens), [])


class Ranks(unittest.TestCase):
    results = [
        {'document_id': 'labels/a', 'score': 0.02},
        {'document_id': 'labels/b', 'score': 0.01},
        {'document_id': 'labels/c', 'score': 0.0},
    ]

    def test_rank_is_one_based_and_zero_scores_do_not_count(self):
        self.assertEqual(bench.rank_of('b', self.results), 2)
        self.assertIsNone(bench.rank_of('c', self.results))
        self.assertIsNone(bench.rank_of('z', self.results))


class Metrics(unittest.TestCase):
    def test_recall_and_mrr_at_ten(self):
        m = bench.metrics([1, 3, None, 12])
        self.assertEqual(m['n'], 4)
        self.assertAlmostEqual(m['recall@1'], 0.25)
        self.assertAlmostEqual(m['recall@5'], 0.5)
        self.assertAlmostEqual(m['recall@10'], 0.5)
        self.assertAlmostEqual(m['mrr@10'], (1 + 1 / 3) / 4)

    def test_bootstrap_is_deterministic_and_brackets_the_mean(self):
        values = [1.0, 0.0, 0.5, 1.0, 0.25, 0.0, 1.0, 0.5]
        low, high = bench.bootstrap_ci(values, seed=7)
        self.assertEqual((low, high), bench.bootstrap_ci(values, seed=7))
        mean = sum(values) / len(values)
        self.assertLessEqual(low, mean)
        self.assertGreaterEqual(high, mean)
        self.assertEqual(bench.bootstrap_ci([0.5] * 5), (0.5, 0.5))

    def test_paired_difference_uses_the_same_queries(self):
        a, b = [1.0, 0.5, 0.0], [0.5, 0.5, 0.0]
        delta, low, high = bench.paired_delta(a, b, seed=1)
        self.assertAlmostEqual(delta, 0.5 / 3)
        self.assertLessEqual(low, delta)
        self.assertGreaterEqual(high, delta)
        with self.assertRaises(ValueError):
            bench.paired_delta([1.0], [1.0, 0.0])


class Transient(unittest.TestCase):
    def test_provider_throttling_behind_a_500_is_retried_but_real_errors_are_not(self):
        self.assertTrue(bench.transient(500, 'Backend error: Gemini API error (429 Too Many Requests)'))
        self.assertTrue(bench.transient(503, 'anything'))
        self.assertFalse(bench.transient(500, 'Backend error: collection not found'))
        self.assertFalse(bench.transient(400, 'rate limit mentioned in a validation error'))


if __name__ == '__main__':
    unittest.main()
