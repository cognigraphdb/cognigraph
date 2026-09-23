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
        self.assertEqual(set(job['needs']), {'gates', 'docker', 'platforms'})
        script = job['steps'][0]['run']
        for gates, docker, platforms, expected in [
                ('success', 'success', 'success', 0), ('failure', 'success', 'success', 1),
                ('success', 'failure', 'success', 1), ('cancelled', 'success', 'success', 1),
                ('success', 'skipped', 'success', 1), ('success', 'success', 'failure', 1),
                ('success', 'success', 'skipped', 1), ('success', 'success', 'cancelled', 1)]:
            result = subprocess.run(['bash', '-e', '-c', script], env={
                **os.environ, 'GATES_RESULT': gates, 'DOCKER_RESULT': docker, 'PLATFORMS_RESULT': platforms})
            self.assertEqual(result.returncode, expected, (gates, docker, platforms))

    def test_docker_job_runs_natively_on_both_published_platforms(self):
        job = self.flow['jobs']['docker']
        self.assertFalse(job['strategy']['fail-fast'])
        legs = {leg['platform']: leg for leg in job['strategy']['matrix']['include']}
        self.assertEqual(set(legs), {'linux/amd64', 'linux/arm64'})
        self.assertEqual(legs['linux/amd64']['runner'], 'ubuntu-24.04')
        self.assertEqual(legs['linux/arm64']['runner'], 'ubuntu-24.04-arm')
        self.assertEqual({leg['arch'] for leg in legs.values()}, {'amd64', 'arm64'})
        self.assertEqual(job['runs-on'], '${{ matrix.runner }}')
        self.assertEqual(job['env']['DOCKER_DEFAULT_PLATFORM'], '${{ matrix.platform }}')
        # Tested images leave the runner only for an authorized publication.
        export = next(step for step in job['steps'] if step.get('run') == 'python3 scripts/docker_images.py export')
        upload = next(step for step in job['steps'] if step.get('with', {}).get('name', '').startswith('tested-images-'))
        for step in (export, upload):
            self.assertEqual(step['if'], "github.event_name == 'workflow_dispatch' && inputs.publish_images")
        self.assertEqual(upload['with']['name'], 'tested-images-${{ matrix.arch }}')
        self.assertEqual(upload['with']['if-no-files-found'], 'error')

    def test_publication_composes_manifests_from_both_tested_legs_in_one_job(self):
        job = self.flow['jobs']['publish']
        self.assertEqual(job['if'], "github.event_name == 'workflow_dispatch' && inputs.publish_images")
        self.assertIn('docker', job['needs'])
        self.assertIn('gates', job['needs'])
        self.assertEqual(job['runs-on'], 'ubuntu-24.04')
        self.assertTrue(all(value == 'read' for value in job['permissions'].values()))
        commands = [s.get('run') for s in job['steps']]
        download = next(s for s in job['steps'] if s.get('uses', '').startswith('actions/download-artifact@'))
        self.assertEqual(download['with']['pattern'], 'tested-images-*')
        self.assertTrue(download['with']['merge-multiple'])
        self.assertEqual(download['with']['path'], 'target/ci/images')
        self.assertLess(job['steps'].index(download), commands.index('python3 scripts/docker_images.py preflight'))
        self.assertLess(commands.index('python3 scripts/docker_images.py preflight'),
                        commands.index('python3 scripts/docker_images.py publish'))
        login = next(i for i, s in enumerate(job['steps']) if s.get('uses', '').startswith('docker/login-action@'))
        self.assertLess(commands.index('python3 scripts/docker_images.py preflight'), login)
        self.assertLess(login, commands.index('python3 scripts/docker_images.py publish'))
        self.assertNotIn('python3 scripts/docker_images.py publish',
                         [s.get('run') for s in self.flow['jobs']['docker']['steps']])

    def test_apple_silicon_job_builds_and_smokes_both_editions(self):
        job = self.flow['jobs']['platforms']
        self.assertEqual(job['runs-on'], 'macos-15')
        self.assertTrue(all(value == 'read' for value in job['permissions'].values()))
        self.assertIn('python3 scripts/check-platform-smoke.py', [s.get('run') for s in job['steps']])
        self.assertNotIn('needs', job)  # Runs beside the shared gates, not after them.

    def test_shared_runner_includes_live_native_helm_and_advisory_checks(self):
        commands = verify.commands('ci')
        for suite in ('native', 'helm', 'advisories', 'dependencies'):
            for command in verify.commands(suite):
                self.assertIn(command, commands)
        self.assertIn((ROOT, ['actionlint']), commands)
        self.assertIn((ROOT, [sys.executable, 'scripts/check-vendored.py']), commands)
        self.assertIn((ROOT, [sys.executable, 'scripts/check-docs.py']), commands)
        self.assertIn((ROOT, [sys.executable, 'scripts/check-public-distribution.py']), commands)
        self.assertIn((ROOT, [sys.executable, 'scripts/check-arangodump-fixtures.py']), commands)
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
        export = next(i for i, s in enumerate(steps) if s.get('run') == 'python3 scripts/docker_images.py export')
        self.assertLess(build, incoming)
        self.assertLess(incoming, export)


if __name__ == '__main__':
    unittest.main()
