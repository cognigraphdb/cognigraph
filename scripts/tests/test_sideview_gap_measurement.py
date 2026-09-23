"""CG-97: the side-view gap measurement harness ablates exactly the rules
behind a kit's expected facts, classifies candidates against the kit's
evaluation, judges review survival from evaluation deltas, keeps secrets out
of saved logs and aggregates repetitions without hiding variance."""
import copy
import importlib.util
from pathlib import Path
import unittest

ROOT = Path(__file__).resolve().parents[2]
spec = importlib.util.spec_from_file_location(
    'sideview_gap_measurement', ROOT / 'docs/verification/harnesses/sideview-gap-measurement.py')
harness = importlib.util.module_from_spec(spec)
spec.loader.exec_module(harness)


def rule(source, relation, target, triggers=('a trigger',)):
    return {'source': source, 'relation': relation, 'target': target, 'when_any': list(triggers)}


SPACE = {
    'id': 'forensics',
    'entities': [],
    'relation_rules': [
        rule('A', 'USES', 'B'),
        rule('A', 'USES', 'B', ('second copy',)),
        rule('B', 'RUNS', 'C'),
        rule('C', 'SEES', 'D', ('{source} sees {target}',)),
    ],
}
SPEC = {'space_id': 'case:x', 'questions': [
    {'id': 'q1', 'expected_facts': ['A --USES--> B', 'C --SEES--> D'], 'forbidden_facts': ['B --RUNS--> C']},
    {'id': 'q2', 'expected_facts': ['A --USES--> B'], 'forbidden_facts': []},
]}


class Ablation(unittest.TestCase):
    def test_blanks_every_rule_behind_an_expected_fact_and_nothing_else(self):
        original = copy.deepcopy(SPACE)
        ablated, report = harness.ablate(SPACE, SPEC)
        self.assertEqual(SPACE, original, 'input must not be mutated')
        triggers = [(r['source'], r['relation'], r['target'], r['when_any']) for r in ablated['relation_rules']]
        self.assertEqual(triggers, [
            ('A', 'USES', 'B', []), ('A', 'USES', 'B', []),
            ('B', 'RUNS', 'C', ['a trigger']), ('C', 'SEES', 'D', []),
        ])
        self.assertEqual(report, {'ablated_rules': 3, 'expected_facts': 2, 'expected_without_rule': []})

    def test_an_expected_fact_without_a_rule_is_reported_not_invented(self):
        spec = {'space_id': 's', 'questions': [{'id': 'q', 'expected_facts': ['X --R--> Y']}]}
        ablated, report = harness.ablate(SPACE, spec)
        self.assertEqual(ablated['relation_rules'], SPACE['relation_rules'])
        self.assertEqual(report['expected_without_rule'], ['X --R--> Y'])

    def test_malformed_fact_lines_are_refused(self):
        spec = {'space_id': 's', 'questions': [{'id': 'q', 'expected_facts': ['not a fact']}]}
        with self.assertRaises(ValueError):
            harness.ablate(SPACE, spec)


class Classification(unittest.TestCase):
    def test_each_candidate_lands_in_exactly_one_class(self):
        missing = {'A --USES--> B'}
        self.assertEqual(harness.classify('A --USES--> B', SPEC, missing), 'missing_expected')
        self.assertEqual(harness.classify('C --SEES--> D', SPEC, missing), 'present_expected')
        self.assertEqual(harness.classify('B --RUNS--> C', SPEC, missing), 'forbidden')
        self.assertEqual(harness.classify('D --NEAR--> A', SPEC, missing), 'outside_evaluation')


class Survival(unittest.TestCase):
    base = {'missing': ['A --USES--> B', 'C --SEES--> D'], 'violations': []}

    def test_a_proposal_survives_only_by_adding_expected_facts_without_violations(self):
        after = {'missing': ['C --SEES--> D'], 'violations': []}
        self.assertEqual(harness.judge(self.base, after),
                         {'added_expected': ['A --USES--> B'], 'new_violations': [], 'survived': True})

    def test_no_gain_or_any_new_violation_fails_review(self):
        self.assertFalse(harness.judge(self.base, dict(self.base))['survived'])
        after = {'missing': [], 'violations': ['B --RUNS--> C']}
        verdict = harness.judge(self.base, after)
        self.assertEqual(verdict['new_violations'], ['B --RUNS--> C'])
        self.assertFalse(verdict['survived'])

    def test_a_pre_existing_violation_is_not_blamed_on_the_proposal(self):
        base = {'missing': ['A --USES--> B'], 'violations': ['B --RUNS--> C']}
        after = {'missing': [], 'violations': ['B --RUNS--> C']}
        self.assertTrue(harness.judge(base, after)['survived'])


class LogRedaction(unittest.TestCase):
    def test_keeps_only_provider_lines_and_masks_secret_shaped_tokens(self):
        log = '\n'.join([
            'INFO Completion provider configured model="gpt-5.6-luna"',
            'INFO Side-view generation provider configured model="gpt-5.6-luna"',
            'INFO Embedding provider: OpenAI',
            'DEBUG request header authorization=Bearer sk-proj-abcdefghijklmnop',
            'INFO something unrelated sk-live-0123456789abcdef',
            'WARN Completion provider configured key=sk-abcdefghijklmnopqrstu',
        ])
        lines = harness.provider_lines(log)
        self.assertEqual(len(lines), 4)
        self.assertTrue(all('sk-' not in line for line in lines), lines)
        self.assertIn('[redacted]', lines[-1])

    def test_ordinary_words_are_not_mistaken_for_keys(self):
        for text in ('skyscrapers-and-bridges', 'risk-assessment-report', 'pk-12', 'AIzaleas'):
            self.assertIsNone(harness.SECRET_SHAPED.search(text), text)
        self.assertIsNotNone(harness.SECRET_SHAPED.search('AIza' + 'x' * 35))

    def test_terminal_colour_codes_are_stripped_before_matching(self):
        log = ('\x1b[2m2026\x1b[0m \x1b[32m INFO\x1b[0m Completion provider configured '
               '\x1b[3mmodel\x1b[0m\x1b[2m=\x1b[0m"gpt-5.6-luna"\n'
               '\x1b[32m INFO\x1b[0m Side-view generation provider configured '
               '\x1b[3mmodel\x1b[0m\x1b[2m=\x1b[0m"gpt-5.6-luna"')
        lines = harness.provider_lines(log)
        self.assertTrue(all('\x1b' not in line for line in lines))
        self.assertTrue(harness.models_match(lines, 'gpt-5.6-luna'))

    def test_model_check_requires_both_generation_lanes_on_the_authorized_model(self):
        good = ['Completion provider configured model="gpt-5.6-luna"',
                'Side-view generation provider configured model=gpt-5.6-luna']
        self.assertTrue(harness.models_match(good, 'gpt-5.6-luna'))
        self.assertFalse(harness.models_match(good[:1], 'gpt-5.6-luna'))
        self.assertFalse(harness.models_match(
            [good[0], 'Side-view generation provider configured model=gpt-5.6-terra'], 'gpt-5.6-luna'))


class Summary(unittest.TestCase):
    def test_reports_every_repetition_and_the_range(self):
        runs = [{'kit': 'bravo', 'metrics': {'candidates': 3, 'survived': 2}},
                {'kit': 'bravo', 'metrics': {'candidates': 5, 'survived': 2}},
                {'kit': 'alpha', 'metrics': {'candidates': 1, 'survived': 0}}]
        summary = harness.summarize(runs)
        self.assertEqual(summary['bravo']['candidates'], {'runs': [3, 5], 'min': 3, 'max': 5, 'mean': 4.0})
        self.assertEqual(summary['bravo']['survived']['runs'], [2, 2])
        self.assertEqual(summary['alpha']['candidates']['mean'], 1.0)
        self.assertEqual(harness.summarize([]), {})


if __name__ == '__main__':
    unittest.main()
