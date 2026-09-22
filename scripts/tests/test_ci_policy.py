"""Verify workflow privilege boundaries and the actual required-check shell gate."""
import json
import os
from pathlib import Path
import re
import subprocess
import sys
import unittest

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / 'scripts'))
import verify


def workflow():
    return json.loads(subprocess.check_output(
        ['bun', '-e', 'console.log(JSON.stringify(Bun.YAML.parse(await Bun.file(process.argv[1]).text())))',
         str(ROOT / '.github/workflows/ci.yml')], text=True, cwd=ROOT))


class WorkflowPolicy(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.flow = workflow()

    def test_pull_request_validation_has_read_only_permissions_and_no_privileged_trigger(self):
        self.assertEqual(set(self.flow['on']), {'push', 'pull_request', 'workflow_dispatch'})
        self.assertEqual(self.flow['on']['pull_request']['branches'], ['main', 'develop'])
        self.assertEqual(self.flow['permissions'], {'contents': 'read'})
        self.assertFalse(self.flow['on']['workflow_dispatch']['inputs']['publish_images']['default'])
        for job in self.flow['jobs'].values():
            for step in job.get('steps', []):
                if 'uses' in step:
                    self.assertRegex(step['uses'], r'^[\w-]+/[\w-]+@[a-f0-9]{40}$')
                if step.get('uses', '').startswith('actions/checkout@'):
                    self.assertFalse(step['with']['persist-credentials'])
                if 'DOCKERHUB_TOKEN' in json.dumps(step) or 'publish' in step.get('run', '') \
                        or 'preflight' in step.get('run', ''):
                    self.assertEqual(step['if'], "github.event_name == 'workflow_dispatch' && inputs.publish_images")

    def test_dependency_updates_target_only_develop(self):
        config = json.loads((ROOT / '.github/renovate.json').read_text())
        self.assertEqual(config['baseBranchPatterns'], ['develop'])
        self.assertFalse(config['automerge'])
        self.assertFalse((ROOT / '.github/dependabot.yml').exists())
        self.assertEqual(self.flow['on']['push']['branches'], ['main', 'develop'])
        step = next(step for step in self.flow['jobs']['gates']['steps']
                    if step.get('name') == 'Require dependency PRs to target develop')
        self.assertEqual(step['if'], "github.event_name == 'pull_request'")
        for author, base, expected in [('renovate[bot]', 'main', 1),
                                       ('dependabot[bot]', 'main', 1),
                                       ('renovate[bot]', 'develop', 0),
                                       ('dependabot[bot]', 'develop', 0),
                                       ('maintainer', 'main', 0)]:
            result = subprocess.run(['bash', '-e', '-c', step['run']],
                                    env={**os.environ, 'PR_AUTHOR': author, 'PR_BASE': base})
            self.assertEqual(result.returncode, expected, (author, base))

    def test_required_check_rejects_failures_cancellations_and_skipped_jobs(self):
        job = self.flow['jobs']['required']
        self.assertEqual(job['name'], 'CI required')
        self.assertEqual(job['if'], 'always()')
        self.assertEqual(set(job['needs']), {'gates', 'docker'})
        script = job['steps'][0]['run']
        for first, second, expected in [('success', 'success', 0), ('failure', 'success', 1),
                                        ('success', 'failure', 1), ('cancelled', 'success', 1),
                                        ('success', 'skipped', 1)]:
            result = subprocess.run(['bash', '-e', '-c', script],
                                    env={**os.environ, 'GATES_RESULT': first, 'DOCKER_RESULT': second})
            self.assertEqual(result.returncode, expected, (first, second))

    def test_shared_runner_includes_live_native_helm_and_advisory_checks(self):
        commands = verify.commands('ci')
        for suite in ('native', 'helm', 'advisories', 'dependencies'):
            for command in verify.commands(suite):
                self.assertIn(command, commands)
        self.assertIn((ROOT, ['actionlint']), commands)
        self.assertIn((ROOT, [sys.executable, 'scripts/check-vendored.py']), commands)
        self.assertIn((ROOT, [sys.executable, 'scripts/check-docs.py']), commands)
        self.assertIn((ROOT, [sys.executable, 'scripts/check-public-distribution.py']), commands)
        self.assertIn((ROOT, ['cargo', 'audit', '--deny', 'unsound']), commands)
        for _, command in commands:
            if command[:2] in (['cargo', 'clippy'], ['cargo', 'test']):
                self.assertIn('--locked', command)
        docker = verify.commands('docker')
        self.assertIn((ROOT, [sys.executable, 'scripts/check-image-vulnerabilities.py']), docker)
        for _, command in docker:
            if command[:2] == ['docker', 'build']:
                self.assertIn('--pull', command)
                self.assertEqual(command[command.index('--no-cache-filter') + 1], 'runtime')
        for flags in (['--live'], ['--live', '--enterprise']):
            self.assertIn((ROOT, [sys.executable, 'scripts/check-helm.py', *flags]), docker)

    def test_incoming_work_is_checked_with_read_only_access_before_required_passes(self):
        source = self.flow['jobs']['gates']['steps']
        commands = [s.get('run') for s in source]
        self.assertLess(commands.index('python3 scripts/check-incoming.py'),
                        commands.index('python3 scripts/verify.py --suite ci'))
        self.assertLess(commands.index('python3 scripts/verify.py --suite ci'),
                        commands.index('python3 scripts/check-incoming.py --mode compare'))
        for name in ('gates', 'docker'):
            job = self.flow['jobs'][name]
            self.assertTrue(all(value == 'read' for value in job['permissions'].values()))
            self.assertEqual(job['permissions']['pull-requests'], 'read')
            checkout = next(step for step in job['steps'] if step.get('uses', '').startswith('actions/checkout@'))
            self.assertEqual(checkout['with']['fetch-depth'], 0)
        steps = self.flow['jobs']['docker']['steps']
        build = next(i for i, s in enumerate(steps) if s.get('run') == 'python3 scripts/verify.py --suite docker')
        incoming = next(i for i, s in enumerate(steps) if s.get('run') == 'python3 scripts/check-incoming.py')
        publish = next(i for i, s in enumerate(steps) if s.get('run') == 'python3 scripts/docker_images.py publish')
        self.assertLess(build, incoming)
        self.assertLess(incoming, publish)


if __name__ == '__main__':
    unittest.main()
