"""Pair-aware benchmark adaptation; gold labels never enter the outgoing request."""
import copy
import json
from common import previous_providers, save

PAIR_INSTRUCTION = '''Benchmark scope: for each excerpt consider ONLY the two supplied nominals. Return at most one fact for that pair, choosing the most specific suitable relation from the taxonomy and its correct direction. Copy the supplied nominal strings exactly as source and target, in the chosen direction. If none of the nine relations holds between that pair, return no fact for that excerpt (Other). Do not extract other pairs. Quote the complete excerpt verbatim as evidence. The pair order is text order, not a claimed relation direction. Do not use outside facts. The supplied pairs by chunk id are:\n'''


def pair_input(incoming, cases):
    augmented = copy.deepcopy(incoming)
    pairs = [{'chunk_id':c['id'],'nominals':c['pair']} for c in cases]
    augmented['messages'][0]['content'] += '\n' + PAIR_INSTRUCTION + json.dumps(pairs,ensure_ascii=False,separators=(',',':'))
    return augmented


class Recorder(previous_providers.Recorder):
    def forward(self, handler, incoming):
        original = copy.deepcopy(incoming)
        super().forward(handler, pair_input(incoming, self.context['cases']))
        self.records[-1]['original_request_from_server'] = original
        save(self.output/'provider-calls.json',self.records)
