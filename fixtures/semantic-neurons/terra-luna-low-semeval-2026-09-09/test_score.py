import unittest
from adapter import pair_input
from evaluate import prediction, classification, INVALID, FAILED
from accounting import usage_cost


class AccountingTests(unittest.TestCase):
    arm = {'provider': 'openai', 'pricing': {'regular': {
        'input': 2, 'cached_input': .2, 'cache_write': 2.5, 'output': 12}}}

    def test_disjoint_cache_buckets_and_reasoning_not_double_billed(self):
        u = {'prompt_tokens': 1000, 'completion_tokens': 100,
             'prompt_tokens_details': {'cached_tokens': 400, 'cache_write_tokens': 300},
             'completion_tokens_details': {'reasoning_tokens': 75}}
        result = usage_cost(u, self.arm, '')
        self.assertAlmostEqual(result['estimated_usd'], .00263)
        self.assertEqual(result['estimated_usd_lower'], result['estimated_usd_upper'])
        self.assertEqual(result['reasoning_tokens'], 75)

    def test_missing_writes_are_unknown_and_bounded(self):
        u = {'prompt_tokens': 1000, 'completion_tokens': 0,
             'prompt_tokens_details': {'cached_tokens': 400}}
        result = usage_cost(u, self.arm, '')
        self.assertIsNone(result['cache_write_tokens'])
        self.assertAlmostEqual(result['estimated_usd_lower'], .00128)
        self.assertAlmostEqual(result['estimated_usd_upper'], .00158)

    def test_missing_both_counters(self):
        result = usage_cost({'prompt_tokens': 1000, 'completion_tokens': 0}, self.arm, '')
        self.assertAlmostEqual(result['estimated_usd_lower'], .0002)
        self.assertAlmostEqual(result['estimated_usd_upper'], .0025)

    def test_invalid_buckets_rejected(self):
        with self.assertRaises(AssertionError):
            usage_cost({'prompt_tokens': 1000, 'completion_tokens': 0,
                        'prompt_tokens_details': {'cached_tokens': 800, 'cache_write_tokens': 800}}, self.arm, '')

class ScoringTests(unittest.TestCase):
    def setUp(self):
        self.case={'id':'x','pair':['tool','worker'],'gold':[{'secret':'oracle'}],'label':'Instrument-Agency(e1,e2)'}
        self.fact={'chunk_id':'x','source':'tool','target':'worker','relation':'Instrument-Agency'}
        self.relations=['Instrument-Agency']
    def test_direction(self):
        self.assertEqual(prediction(self.case,[self.fact],self.relations),'Instrument-Agency(e1,e2)')
        self.assertEqual(prediction(self.case,[dict(self.fact,source='worker',target='tool')],self.relations),'Instrument-Agency(e2,e1)')
    def test_failures_not_negatives(self):
        self.assertEqual(prediction(self.case,[],self.relations,False),FAILED)
        rows=[{'gold':'Other','raw':FAILED}]
        self.assertEqual(classification(rows,'raw',self.relations)['correct_abstentions'],0)
    def test_invalid_multiple_and_wrong_pair(self):
        self.assertEqual(prediction(self.case,[self.fact,self.fact],self.relations),INVALID)
        self.assertEqual(prediction(self.case,[dict(self.fact,target='other')],self.relations),INVALID)
    def test_other(self):
        self.assertEqual(prediction(self.case,[],self.relations),'Other')
    def test_wrong_direction_fp_fn(self):
        rows=[{'gold':'Instrument-Agency(e1,e2)','raw':'Instrument-Agency(e2,e1)'}]
        stats=classification(rows,'raw',self.relations)
        self.assertEqual(stats['direction_errors'],1)
        self.assertEqual(stats['by_relation']['Instrument-Agency']['fp'],1)
        self.assertEqual(stats['by_relation']['Instrument-Agency']['fn'],1)
    def test_pair_adapter_does_not_expose_gold(self):
        original={'messages':[{'role':'system','content':'Task'}]}
        result=pair_input(original,[self.case])
        self.assertNotIn('oracle',str(result));self.assertNotIn('Instrument-Agency',str(result))
        self.assertEqual(original['messages'][0]['content'],'Task')

if __name__=='__main__': unittest.main()
