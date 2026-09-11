import unittest
from adapter import pair_input
from evaluate import prediction, classification, INVALID, FAILED

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
