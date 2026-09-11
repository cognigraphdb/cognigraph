import copy
import json
import unittest

from shared import ROOT, BASELINE, identity, metrics
from providers import adapt, rates_at, usage_cost


class Protocol(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.arms = {a['id']: a for a in json.loads((ROOT / 'protocol.json').read_text())['arms']}

    def test_zai_promotion_expiry_is_utc_boundary(self):
        arm = self.arms['glm-flash']
        self.assertEqual(rates_at(arm, '2026-09-09T15:59:59Z')[0], 'promotion')
        self.assertEqual(rates_at(arm, '2026-09-09T16:00:00Z')[0], 'regular')

    def test_deepseek_peak_windows_and_weekend(self):
        arm = self.arms['ds-flash']
        for stamp, expected in [('2026-09-09T01:00:00Z', 'peak'), ('2026-09-09T04:00:00Z', 'off_peak'),
            ('2026-09-09T06:00:00Z', 'peak'), ('2026-09-09T10:00:00Z', 'off_peak'), ('2026-09-12T06:00:00Z', 'off_peak')]:
            self.assertEqual(rates_at(arm, stamp)[0], expected)

    def test_cache_cost_and_no_double_count_reasoning(self):
        usage = {'prompt_tokens': 1000, 'completion_tokens': 200, 'prompt_cache_hit_tokens': 700,
                 'completion_tokens_details': {'reasoning_tokens': 100}}
        result = usage_cost(usage, self.arms['ds-flash'], '2026-09-09T11:00:00Z')
        self.assertAlmostEqual(result['estimated_usd'], (300 * .22 + 700 * .007 + 200 * .66) / 1e6)
        self.assertEqual(result['reasoning_tokens'], 100)

    def test_missing_counters_remain_unknown(self):
        result = usage_cost({'prompt_tokens': 100, 'completion_tokens': 20}, self.arms['glm-flash'], '2026-09-09T11:00:00Z')
        self.assertIsNone(result['reasoning_tokens'])
        self.assertIsNone(result['cached_input_tokens'])

    def test_adapter_preserves_original_and_schema_across_models(self):
        incoming = json.loads((BASELINE / 'live/provider-calls.json').read_text())[0]['request_from_server']
        untouched = copy.deepcopy(incoming)
        all_messages = [adapt(incoming, a, 4096)['messages'] for a in self.arms.values()]
        self.assertTrue(all(m == all_messages[0] for m in all_messages))
        self.assertEqual(incoming, untouched)
        self.assertEqual(all_messages[0][1], incoming['messages'][1])

    def test_wrong_direction_and_partial_category_grain(self):
        gold = {identity('c', 'A', 'R', 'B'), identity('c', 'C', 'R', 'D')}
        result = metrics(gold, {identity('c', 'B', 'R', 'A')})
        self.assertEqual((result['tp'], result['fp'], result['fn']), (0, 1, 2))


if __name__ == '__main__':
    unittest.main()
