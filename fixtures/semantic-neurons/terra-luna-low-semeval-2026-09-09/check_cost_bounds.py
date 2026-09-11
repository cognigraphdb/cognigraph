"""Verify that partial cache counters enclose every tested compatible true cost."""
import argparse
import json
from pathlib import Path
import random
from common import ROOT, digest, save
from accounting import usage_cost


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    arm = json.loads((ROOT/'protocol.json').read_text())['arms'][1]
    rate = arm['pricing']['regular']
    rng = random.Random(20260909)
    checks = 0
    for _ in range(1000):
        total = rng.randrange(10000)
        cached = rng.randrange(total+1)
        writes = rng.randrange(total-cached+1)
        ordinary = total-cached-writes
        output = rng.randrange(8193)
        actual = (ordinary*rate['input'] + cached*rate['cached_input'] + writes*rate['cache_write'] + output*rate['output'])/1e6
        for mask in range(4):
            details = {}
            if mask & 1:
                details['cached_tokens'] = cached
            if mask & 2:
                details['cache_write_tokens'] = writes
            result = usage_cost({'prompt_tokens': total, 'completion_tokens': output, 'prompt_tokens_details': details}, arm, '')
            assert result['estimated_usd_lower']-1e-12 <= actual <= result['estimated_usd_upper']+1e-12
            checks += 1
    save(args.output, {'seed': 20260909, 'random_complete_allocations': 1000,
                       'missing_counter_masks_per_allocation': 4,
                       'compatible_actual_cost_within_reported_bounds': checks, 'status': 'passed',
                       'accounting_sha256': digest(ROOT/'accounting.py'),
                       'generator_sha256': digest(Path(__file__))})
    print(f'{checks} compatible-allocation bounds passed.')


if __name__ == '__main__':
    main()
