"""Incoming-work gate rejects stale PR/base snapshots and unreviewed heads."""
import copy
import importlib.util
import json
from pathlib import Path
import sys
import tempfile
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / 'scripts'))
import incoming

spec = importlib.util.spec_from_file_location('check_incoming', ROOT / 'scripts/check-incoming.py')
gate = importlib.util.module_from_spec(spec)
spec.loader.exec_module(gate)
SNAPSHOT = {'repository': 'owner/repo', 'candidate': 'a' * 40, 'develop': 'b' * 40,
            'prs': [[1, 'c' * 40, 'develop']], 'details': []}


class Incoming(unittest.TestCase):
    def test_all_paginated_pr_heads_and_targets_are_accounted_for(self):
        pages = [[{'number': 1, 'head': {'sha': 'a' * 40}, 'base': {'ref': 'develop'}}],
                 [{'number': 2, 'head': {'sha': 'b' * 40}, 'base': {'ref': 'main'}}]]
        with patch.object(gate, 'output', return_value=json.dumps(pages)) as output:
            self.assertEqual(len(incoming.open_prs('owner/repo', output)), 2)
            self.assertIn('--paginate', output.call_args.args)

    def test_changed_candidate_base_head_or_target_cannot_reuse_a_snapshot(self):
        for key, value in [('candidate', 'd' * 40), ('develop', 'e' * 40),
                           ('prs', [[1, 'f' * 40, 'develop']]),
                           ('prs', [[1, 'c' * 40, 'main']]),
                           ('prs', SNAPSHOT['prs'] + [[2, 'd' * 40, 'develop']])]:
            current = {**SNAPSHOT, key: value}
            with self.subTest(key=key), self.assertRaisesRegex(RuntimeError, 'snapshot changed'):
                gate.compare(SNAPSHOT, current)
        gate.compare(SNAPSHOT, {**SNAPSHOT, 'details': [{'status': 'our own CI advanced'}]})

    def test_needs_changes_disposition_requires_exact_head_and_owner_decision(self):
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder)
            path = root / 'docs/operations/pr-dispositions.json'
            path.parent.mkdir(parents=True)
            record = {'number': 1, 'head': 'c' * 40, 'disposition': 'needs-changes',
                      'reason': 'Synthetic regression', 'decision': 'Synthetic owner decision'}
            path.write_text(json.dumps([record]))
            incoming.verify(SNAPSHOT['prs'], SNAPSHOT['candidate'], root, lambda *_: False)
            for change in ({'head': 'd' * 40}, {'decision': ''}):
                path.write_text(json.dumps([{**record, **change}]))
                with self.assertRaisesRegex(RuntimeError, 'PR #1'):
                    incoming.verify(SNAPSHOT['prs'], SNAPSHOT['candidate'], root, lambda *_: False)

    def test_unprocessed_pr_still_retains_its_review_and_check_inventory(self):
        with tempfile.TemporaryDirectory() as folder, patch.object(gate, 'ROOT', Path(folder)), \
                patch.object(gate, 'inspect', return_value=copy.deepcopy(SNAPSHOT)), \
                patch.object(gate.subprocess, 'run') as run:
            run.return_value.returncode = 1
            path = Path(folder) / 'report.json'
            with self.assertRaisesRegex(RuntimeError, 'PR #1'):
                gate.main('snapshot', path)
            self.assertEqual(json.loads(path.read_text()), SNAPSHOT)

    def test_included_pr_with_requested_changes_blocks(self):
        current = {**SNAPSHOT, 'details': [{'number': 1, 'headRefOid': 'c' * 40,
                                          'reviewDecision': 'CHANGES_REQUESTED'}]}
        with tempfile.TemporaryDirectory() as folder, patch.object(gate, 'ROOT', Path(folder)), \
                patch.object(gate, 'inspect', return_value=current), \
                patch.object(gate.subprocess, 'run') as run:
            run.return_value.returncode = 0
            with self.assertRaisesRegex(RuntimeError, 'requested changes'):
                gate.main('snapshot', Path(folder) / 'report.json')


if __name__ == '__main__':
    unittest.main()
