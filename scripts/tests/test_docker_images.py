"""Fail-closed publication gates; runtime coverage lives in the Docker suite."""
import contextlib
import io
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import call, patch
from urllib.error import HTTPError, URLError

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import docker_images as images

REVISION = 'a' * 40
VERSION = '1.2.3'
DIGEST = 'sha256:' + 'c' * 64
SETTINGS = {'enabled': True, 'rules': [images.VERSION_TAG_RULE]}
ENV = {'GITHUB_ACTIONS': 'true', 'GITHUB_EVENT_NAME': 'workflow_dispatch',
       'GITHUB_REPOSITORY': images.REPOSITORY, 'GITHUB_REF': 'refs/heads/main',
       'GITHUB_SHA': REVISION}


class Metadata(unittest.TestCase):
    def test_version_lock_and_changelog_must_agree(self):
        with tempfile.TemporaryDirectory() as folder, patch.object(images, 'ROOT', Path(folder)), \
                patch.object(images, 'output', return_value=REVISION):
            root = Path(folder)
            (root / 'docs/changelog').mkdir(parents=True)
            record = root / 'docs/changelog/release.md'
            record.write_text('- Status: v1.2.3\n')
            (root / 'Cargo.toml').write_text('[workspace.package]\nversion="1.2.3"\n')
            lock = root / 'Cargo.lock'
            lock.write_text('[[package]]\nname="cognigraph-server"\nversion="1.2.3"\n')
            self.assertEqual(images.metadata(), (VERSION, REVISION))
            lock.write_text('[[package]]\nname="cognigraph-server"\nversion="1.2.2"\n')
            with self.assertRaisesRegex(RuntimeError, 'Cargo.lock'):
                images.metadata()
            lock.write_text('[[package]]\nname="cognigraph-server"\nversion="1.2.3"\n')
            record.write_text('- Status: Unreleased\n')
            with self.assertRaisesRegex(RuntimeError, 'changelog'):
                images.metadata()
            (root / 'Cargo.toml').write_text('[workspace.package]\nversion="1.2.3-rc1"\n')
            with self.assertRaisesRegex(RuntimeError, 'stable'):
                images.metadata()


class Candidate(unittest.TestCase):
    def setUp(self):
        self.addCleanup(patch.stopall)
        patch.dict(os.environ, ENV, clear=True).start()
        self.tags = patch.object(images, 'available_tags').start()
        self.command = patch.object(images, 'output', side_effect=['', f'{REVISION}\trefs/heads/main']).start()

    def test_clean_current_main_passes(self):
        images.preflight(VERSION, REVISION)
        self.tags.assert_called_once_with(VERSION)

    def test_wrong_trigger_repository_ref_or_revision_blocks_before_network(self):
        for key, value in [('GITHUB_ACTIONS', 'false'), ('GITHUB_EVENT_NAME', 'pull_request'),
                           ('GITHUB_REPOSITORY', 'fork/cognigraph'), ('GITHUB_REF', 'refs/tags/v1.2.3'),
                           ('GITHUB_SHA', 'b' * 40)]:
            with self.subTest(key=key), patch.dict(os.environ, {key: value}):
                with self.assertRaisesRegex(RuntimeError, 'restricted'):
                    images.preflight(VERSION, REVISION)
        self.command.assert_not_called()
        self.tags.assert_not_called()

    def test_dirty_checkout_or_changed_main_blocks(self):
        for responses in ([' M Dockerfile'], ['', f'{"b" * 40}\trefs/heads/main'], ['', '']):
            self.command.side_effect = responses
            with self.assertRaises(RuntimeError):
                images.preflight(VERSION, REVISION)
        self.tags.assert_not_called()


class Registry(unittest.TestCase):
    def test_only_explicit_404_counts_as_missing(self):
        for code in (401, 403, 429, 500):
            with self.subTest(code=code), patch.object(images, 'urlopen', side_effect=HTTPError(
                    'https://hub.docker.com', code, 'fixture', None, None)):
                with self.assertRaises(RuntimeError):
                    images.hub_json('fixture', missing_ok=True)
        with patch.object(images, 'urlopen', side_effect=[
                HTTPError('fixture', 404, 'fixture', None, None) for _ in range(2)]):
            self.assertIsNone(images.hub_json('fixture', missing_ok=True))
            with self.assertRaises(RuntimeError):
                images.hub_json('fixture')
        with patch.object(images, 'urlopen', side_effect=URLError('offline')):
            with self.assertRaises(URLError):
                images.hub_json('fixture', missing_ok=True)

    def test_existing_tag_missing_private_or_wrong_repository_blocks(self):
        valid = {'namespace': 'cognigraph', 'name': 'cognigraph', 'is_private': False,
                 'immutable_tags_settings': SETTINGS}
        for responses in ([None], [{**valid, 'is_private': True}],
                          [{**valid, 'namespace': 'wrong'}], [valid, {'name': VERSION}]):
            with self.subTest(responses=responses), patch.object(images, 'hub_json', side_effect=responses):
                with self.assertRaises(RuntimeError):
                    images.available_tags(VERSION)

    def test_both_destinations_checked(self):
        repos = [{'namespace': 'cognigraph', 'name': repo.split('/')[1], 'is_private': False,
                  'immutable_tags_settings': SETTINGS}
                 for _, repo in images.IMAGES.values()]
        with patch.object(images, 'hub_json', side_effect=[repos[0], None, repos[1], None]) as get:
            images.available_tags(VERSION)
        self.assertEqual(get.call_count, 4)

    def test_mutable_versions_or_immutable_latest_block_publication(self):
        for settings in (None, {'enabled': False, 'rules': []}, {'enabled': True, 'rules': ['.*']}):
            with self.subTest(settings=settings), patch.object(images, 'hub_json', return_value={
                    'namespace': 'cognigraph', 'name': 'cognigraph', 'is_private': False,
                    'immutable_tags_settings': settings}):
                with self.assertRaisesRegex(RuntimeError, 'immutable x.y.z'):
                    images.available_tags(VERSION)

    def test_digest_readback_retries_absence_but_rejects_invalid_identity(self):
        with patch.object(images, 'hub_json', side_effect=[None, {'digest': DIGEST}]), \
                patch.object(images.time, 'sleep'):
            images.verify_tag('cognigraph/cognigraph', VERSION, DIGEST)
        with patch.object(images, 'hub_json', return_value={'digest': 'invalid'}):
            with self.assertRaisesRegex(RuntimeError, 'invalid registry digest'):
                images.tag_digest('cognigraph/cognigraph', VERSION)

    def test_failed_digest_readback_is_not_accepted(self):
        with patch.object(images, 'tag_digest', return_value='sha256:' + 'd' * 64), \
                patch.object(images.time, 'sleep'):
            with self.assertRaisesRegex(RuntimeError, 'registry digest mismatch'):
                images.verify_tag('cognigraph/cognigraph', 'latest', DIGEST)


class ImageIdentity(unittest.TestCase):
    def test_failed_start_removes_its_container_and_volume(self):
        with patch.object(images, 'output', side_effect=[
                'probe-id', subprocess.CalledProcessError(1, ['docker', 'start'])]), \
                patch.object(images.subprocess, 'run') as cleanup:
            with self.assertRaises(subprocess.CalledProcessError):
                images.smoke('sha256:fixture', 'community', VERSION)
        self.assertEqual(cleanup.call_args.args[0], ['docker', 'rm', '--force', '--volumes', 'probe-id'])

    @staticmethod
    def info(edition):
        return {'Id': f'sha256:{edition}', 'Os': 'linux', 'Architecture': 'amd64',
                'Config': {'User': 'cognigraph', 'Labels': {
                    'org.opencontainers.image.version': VERSION,
                    'org.opencontainers.image.revision': REVISION,
                    'org.opencontainers.image.source': images.SOURCE, 'io.cognigraph.edition': edition}}}

    def test_both_labels_and_platform_checked_before_any_container_starts(self):
        for change in ('version', 'revision', 'edition', 'root', 'platform', 'volume'):
            info = self.info('enterprise')
            if change == 'root':
                info['Config']['User'] = 'root'
            elif change == 'platform':
                info['Architecture'] = 'arm64'
            elif change == 'volume':
                info['Config']['Volumes'] = {'/data': {}}
            else:
                key = 'io.cognigraph.edition' if change == 'edition' else f'org.opencontainers.image.{change}'
                info['Config']['Labels'][key] = 'wrong'
            responses = [json.dumps([self.info('community')]), json.dumps([info])]
            with self.subTest(change=change), patch.object(images, 'output', side_effect=responses), \
                    patch.object(images, 'smoke') as smoke:
                with self.assertRaises(RuntimeError):
                    images.checked_images(VERSION, REVISION, 'linux/amd64')
                smoke.assert_not_called()

    def test_tests_receive_pinned_ids(self):
        responses = [json.dumps([self.info(edition)]) for edition in images.IMAGES]
        with patch.object(images, 'output', side_effect=responses), patch.object(images, 'smoke') as smoke:
            result = images.checked_images(VERSION, REVISION, 'linux/amd64')
        self.assertEqual(result, {'community': 'sha256:community', 'enterprise': 'sha256:enterprise'})
        self.assertEqual(smoke.call_args_list, [call(image, edition, VERSION) for edition, image in result.items()])


class Publication(unittest.TestCase):
    def setUp(self):
        self.addCleanup(patch.stopall)
        patch.dict(os.environ, {}, clear=True).start()
        self.preflight = patch.object(images, 'preflight').start()
        self.current = patch.object(images, 'current_candidate').start()
        self.tag = patch.object(images, 'tag_digest', return_value=None).start()
        self.verify = patch.object(images, 'verify_tag').start()
        self.checked = patch.object(images, 'checked_images', return_value={
            'community': 'sha256:tested-community', 'enterprise': 'sha256:tested-enterprise'}).start()
        self.push = patch.object(images, 'push_image', return_value='sha256:' + 'c' * 64).start()

    def test_failed_smoke_never_uploads_either_edition(self):
        self.checked.side_effect = RuntimeError('Enterprise failed after Community passed')
        with self.assertRaises(RuntimeError):
            images.publish(VERSION, REVISION)
        self.push.assert_not_called()

    def test_changed_remote_after_testing_blocks_all_uploads(self):
        self.preflight.side_effect = [None, RuntimeError('main or tag changed')]
        with self.assertRaises(RuntimeError):
            images.publish(VERSION, REVISION)
        self.push.assert_not_called()

    def test_publishes_both_versions_before_aliases_and_records_each_digest(self):
        with tempfile.TemporaryDirectory() as folder, contextlib.redirect_stdout(io.StringIO()):
            summary = Path(folder) / 'summary'
            with patch.dict(os.environ, {'GITHUB_STEP_SUMMARY': str(summary)}):
                images.publish(VERSION, REVISION)
            self.assertEqual(summary.read_text().count('sha256:'), 4)
            self.assertIn(REVISION, summary.read_text())
        self.assertEqual(self.push.call_args_list, [
            call('sha256:tested-community', 'docker.io/cognigraph/cognigraph:1.2.3'),
            call('sha256:tested-enterprise', 'docker.io/cognigraph/cognigraph-enterprise:1.2.3'),
            call('sha256:tested-community', 'docker.io/cognigraph/cognigraph:latest'),
            call('sha256:tested-enterprise', 'docker.io/cognigraph/cognigraph-enterprise:latest')])
        self.assertEqual(self.verify.call_count, 4)

    def test_unverified_version_prevents_all_alias_updates(self):
        self.verify.side_effect = [None, RuntimeError('registry digest mismatch')]
        with contextlib.redirect_stdout(io.StringIO()), self.assertRaises(RuntimeError):
            images.publish(VERSION, REVISION)
        self.assertEqual(self.push.call_count, 2)

    def test_main_or_latest_drift_after_version_publication_prevents_alias_updates(self):
        for drift in ('main', 'latest'):
            self.push.reset_mock()
            self.current.side_effect = RuntimeError('Main changed') if drift == 'main' else None
            self.tag.side_effect = [None, None, DIGEST] if drift == 'latest' else None
            with self.subTest(drift=drift), contextlib.redirect_stdout(io.StringIO()), \
                    self.assertRaises(RuntimeError):
                images.publish(VERSION, REVISION)
            self.assertEqual(self.push.call_count, 2)

    def test_alias_digest_must_equal_its_version_digest(self):
        self.push.side_effect = [DIGEST, DIGEST, 'sha256:' + 'd' * 64]
        with contextlib.redirect_stdout(io.StringIO()), self.assertRaisesRegex(RuntimeError, 'alias differs'):
            images.publish(VERSION, REVISION)
        self.assertEqual(self.push.call_count, 3)

    def test_failed_first_push_stops_second(self):
        self.push.side_effect = subprocess.CalledProcessError(1, ['docker', 'push'])
        with self.assertRaises(subprocess.CalledProcessError):
            images.publish(VERSION, REVISION)
        self.assertEqual(self.push.call_count, 1)

    def test_partial_publication_keeps_the_successful_digest(self):
        digest = 'sha256:' + 'c' * 64
        self.push.side_effect = [digest, subprocess.CalledProcessError(1, ['docker', 'push'])]
        with tempfile.TemporaryDirectory() as folder, contextlib.redirect_stdout(io.StringIO()):
            summary = Path(folder) / 'summary'
            with patch.dict(os.environ, {'GITHUB_STEP_SUMMARY': str(summary)}):
                with self.assertRaises(subprocess.CalledProcessError):
                    images.publish(VERSION, REVISION)
            self.assertIn(f'cognigraph/cognigraph:{VERSION}@{digest}', summary.read_text())
            self.assertNotIn('cognigraph-enterprise', summary.read_text())


class PushResult(unittest.TestCase):
    def test_records_registry_digest_and_rejects_ambiguous_success(self):
        digest = 'sha256:' + 'f' * 64
        with patch.object(images, 'output', side_effect=['', f'1.2.3: digest: {digest} size: 123']), \
                contextlib.redirect_stdout(io.StringIO()):
            self.assertEqual(images.push_image('sha256:tested', 'localhost:5000/probe:1.2.3'), digest)
        with patch.object(images, 'output', side_effect=['', 'no digest']), contextlib.redirect_stdout(io.StringIO()):
            with self.assertRaisesRegex(RuntimeError, 'inspect remote state'):
                images.push_image('sha256:tested', 'localhost:5000/probe:1.2.3')


if __name__ == '__main__':
    unittest.main()
