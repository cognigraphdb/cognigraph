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
AMD64, ARM64 = 'linux/amd64', 'linux/arm64'
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
    def test_immutable_rule_covers_version_and_per_architecture_tags_only(self):
        import re
        for tag in ('1.2.3', '1.2.3-amd64', '1.2.3-arm64', '10.20.30-arm64'):
            self.assertRegex(tag, images.VERSION_TAG_RULE)
        for tag in ('latest', '1.2.3-x86', '1.2.3-amd64-extra', 'v1.2.3', '1.2', '1.2.3-', '1.2.3-linux-amd64'):
            self.assertIsNone(re.search(images.VERSION_TAG_RULE, tag), tag)

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
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.exports = Path(self.temp.name)
        self.addCleanup(patch.stopall)
        patch.object(images, 'EXPORTS', self.exports).start()

    def test_failed_start_removes_its_container_and_volume(self):
        with patch.object(images, 'output', side_effect=[
                'probe-id', subprocess.CalledProcessError(1, ['docker', 'start'])]), \
                patch.object(images.subprocess, 'run') as cleanup:
            with self.assertRaises(subprocess.CalledProcessError):
                images.smoke('sha256:fixture', 'community', VERSION)
        self.assertEqual(cleanup.call_args.args[0], ['docker', 'rm', '--force', '--volumes', 'probe-id'])

    @staticmethod
    def info(edition, architecture='amd64'):
        return {'Id': f'sha256:{edition}', 'Os': 'linux', 'Architecture': architecture,
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

    def test_tests_receive_pinned_ids_and_write_the_platform_receipt(self):
        responses = [json.dumps([self.info(edition, 'arm64')]) for edition in images.IMAGES]
        with patch.object(images, 'output', side_effect=responses), patch.object(images, 'smoke') as smoke:
            result = images.checked_images(VERSION, REVISION, ARM64)
        self.assertEqual(result, {'community': 'sha256:community', 'enterprise': 'sha256:enterprise'})
        self.assertEqual(smoke.call_args_list, [call(image, edition, VERSION) for edition, image in result.items()])
        receipt = json.loads((self.exports / 'arm64.json').read_text())
        self.assertEqual(receipt, {'platform': ARM64, 'version': VERSION, 'revision': REVISION,
                                   'images': result})

    def test_unknown_platform_is_refused_before_inspection(self):
        with patch.object(images, 'output') as output:
            for platform in (None, 'linux/386', 'darwin/arm64', 'amd64'):
                with self.subTest(platform=platform), self.assertRaisesRegex(RuntimeError, 'platform'):
                    images.checked_images(VERSION, REVISION, platform)
        output.assert_not_called()

    def test_export_saves_only_the_images_the_runtime_checks_covered(self):
        responses = [json.dumps([self.info(edition)]) for edition in images.IMAGES]
        with patch.object(images, 'output', side_effect=responses), patch.object(images, 'smoke'):
            images.checked_images(VERSION, REVISION, AMD64)
        responses = [json.dumps([self.info('community')]), '', json.dumps([self.info('enterprise')]), '']
        with patch.object(images, 'output', side_effect=responses) as output:
            images.export_images(VERSION, REVISION, AMD64)
        saves = [c.args for c in output.call_args_list if c.args[:2] == ('docker', 'save')]
        self.assertEqual(saves, [
            ('docker', 'save', '-o', str(self.exports / 'amd64-community.tar'), 'sha256:community'),
            ('docker', 'save', '-o', str(self.exports / 'amd64-enterprise.tar'), 'sha256:enterprise')])
        # A rebuilt tag no longer matches the tested identity.
        rebuilt = self.info('community')
        rebuilt['Id'] = 'sha256:rebuilt'
        with patch.object(images, 'output', side_effect=[json.dumps([rebuilt])]) as output:
            with self.assertRaisesRegex(RuntimeError, 'rerun the Docker suite'):
                images.export_images(VERSION, REVISION, AMD64)
        self.assertFalse(any(c.args[:2] == ('docker', 'save') for c in output.call_args_list))
        # No receipt for the platform: nothing was checked here.
        with patch.object(images, 'output') as output:
            with self.assertRaisesRegex(RuntimeError, 'receipt'):
                images.export_images(VERSION, REVISION, ARM64)
        output.assert_not_called()

    def receipts(self, revision=REVISION):
        for platform, arch in images.PLATFORMS.items():
            (self.exports / f'{arch}.json').write_text(json.dumps({
                'platform': platform, 'version': VERSION, 'revision': revision,
                'images': {edition: f'sha256:{arch}-{edition}' for edition in images.IMAGES}}))

    def loaded(self, arch, edition):
        info = self.info(edition, arch)
        info['Id'] = f'sha256:{arch}-{edition}'
        return [f'Loaded image ID: sha256:{arch}-{edition}', json.dumps([info])]

    def test_load_returns_every_platform_only_when_identities_match_the_receipts(self):
        self.receipts()
        responses = [line for arch in ('amd64', 'arm64') for edition in images.IMAGES
                     for line in self.loaded(arch, edition)]
        with patch.object(images, 'output', side_effect=responses) as output:
            tested = images.load_images(VERSION, REVISION)
        self.assertEqual(tested, {
            AMD64: {'community': 'sha256:amd64-community', 'enterprise': 'sha256:amd64-enterprise'},
            ARM64: {'community': 'sha256:arm64-community', 'enterprise': 'sha256:arm64-enterprise'}})
        self.assertEqual(output.call_args_list[0].args,
                         ('docker', 'load', '-i', str(self.exports / 'amd64-community.tar')))
        responses = ['Loaded image ID: sha256:' + 'e' * 64]
        with patch.object(images, 'output', side_effect=responses):
            with self.assertRaisesRegex(RuntimeError, 'identity'):
                images.load_images(VERSION, REVISION)
        self.receipts(revision='b' * 40)
        with patch.object(images, 'output') as output:
            with self.assertRaisesRegex(RuntimeError, 'receipt'):
                images.load_images(VERSION, REVISION)
        output.assert_not_called()
        (self.exports / 'arm64.json').unlink()
        self.receipts()
        (self.exports / 'arm64.json').unlink()
        with patch.object(images, 'output') as output:
            with self.assertRaisesRegex(RuntimeError, 'linux/arm64'):
                images.load_images(VERSION, REVISION)
        output.assert_not_called()


class Manifests(unittest.TestCase):
    def test_index_digest_and_platform_coverage_are_read_back(self):
        index = {'digest': DIGEST, 'manifests': [
            {'platform': {'os': 'linux', 'architecture': 'amd64'}},
            {'platform': {'os': 'linux', 'architecture': 'arm64'}}]}
        with patch.object(images, 'output', side_effect=['', json.dumps(index)]) as output:
            digest = images.create_manifest('cognigraph/cognigraph', VERSION, ['docker.io/cognigraph/cognigraph@sha256:' + 'a' * 64, 'docker.io/cognigraph/cognigraph@sha256:' + 'b' * 64])
        self.assertEqual(digest, DIGEST)
        self.assertEqual(output.call_args_list[0].args[:6], (
            'docker', 'buildx', 'imagetools', 'create', '-t', f'docker.io/cognigraph/cognigraph:{VERSION}'))
        index['manifests'].pop()
        with patch.object(images, 'output', side_effect=['', json.dumps(index)]):
            with self.assertRaisesRegex(RuntimeError, 'linux/arm64'):
                images.create_manifest('cognigraph/cognigraph', VERSION, ['x'])
        with patch.object(images, 'output', side_effect=['', json.dumps({'digest': 'bad', 'manifests': []})]):
            with self.assertRaisesRegex(RuntimeError, 'digest'):
                images.create_manifest('cognigraph/cognigraph', VERSION, ['x'])


class Publication(unittest.TestCase):
    TESTED = {AMD64: {'community': 'sha256:amd64-community', 'enterprise': 'sha256:amd64-enterprise'},
              ARM64: {'community': 'sha256:arm64-community', 'enterprise': 'sha256:arm64-enterprise'}}

    def setUp(self):
        self.addCleanup(patch.stopall)
        patch.dict(os.environ, {}, clear=True).start()
        self.preflight = patch.object(images, 'preflight').start()
        self.current = patch.object(images, 'current_candidate').start()
        self.tag = patch.object(images, 'tag_digest', return_value=None).start()
        self.verify = patch.object(images, 'verify_tag').start()
        self.loaded = patch.object(images, 'load_images', return_value=self.TESTED).start()
        patch.object(images, 'host_platform', return_value=AMD64).start()
        self.smoke = patch.object(images, 'smoke').start()
        self.push = patch.object(images, 'push_image', side_effect=[
            'sha256:' + c * 64 for c in 'abcd']).start()
        # Version indexes e/f, then identical alias indexes e/f.
        self.manifest = patch.object(images, 'create_manifest', side_effect=[
            'sha256:' + c * 64 for c in 'efef']).start()

    def test_host_platform_images_are_rerun_before_any_upload(self):
        images.publish(VERSION, REVISION)
        self.assertEqual(self.smoke.call_args_list, [
            call('sha256:amd64-community', 'community', VERSION),
            call('sha256:amd64-enterprise', 'enterprise', VERSION)])
        self.smoke.side_effect = RuntimeError('Enterprise failed after Community passed')
        self.push.reset_mock()
        with self.assertRaises(RuntimeError):
            images.publish(VERSION, REVISION)
        self.push.assert_not_called()

    def test_failed_load_or_changed_remote_after_testing_blocks_all_uploads(self):
        self.loaded.side_effect = RuntimeError('arm64 identity mismatch')
        with self.assertRaises(RuntimeError):
            images.publish(VERSION, REVISION)
        self.loaded.side_effect = None
        self.preflight.side_effect = [None, RuntimeError('main or tag changed')]
        with self.assertRaises(RuntimeError):
            images.publish(VERSION, REVISION)
        self.push.assert_not_called()
        self.manifest.assert_not_called()

    def test_per_architecture_tags_precede_indexes_which_precede_aliases(self):
        with tempfile.TemporaryDirectory() as folder, contextlib.redirect_stdout(io.StringIO()):
            summary = Path(folder) / 'summary'
            with patch.dict(os.environ, {'GITHUB_STEP_SUMMARY': str(summary)}):
                images.publish(VERSION, REVISION)
            text = summary.read_text()
            self.assertEqual(text.count('sha256:'), 8)
            self.assertIn(REVISION, text)
        self.assertEqual(self.push.call_args_list, [
            call('sha256:amd64-community', 'docker.io/cognigraph/cognigraph:1.2.3-amd64'),
            call('sha256:arm64-community', 'docker.io/cognigraph/cognigraph:1.2.3-arm64'),
            call('sha256:amd64-enterprise', 'docker.io/cognigraph/cognigraph-enterprise:1.2.3-amd64'),
            call('sha256:arm64-enterprise', 'docker.io/cognigraph/cognigraph-enterprise:1.2.3-arm64')])
        community = ['docker.io/cognigraph/cognigraph@sha256:' + 'a' * 64,
                     'docker.io/cognigraph/cognigraph@sha256:' + 'b' * 64]
        enterprise = ['docker.io/cognigraph/cognigraph-enterprise@sha256:' + 'c' * 64,
                      'docker.io/cognigraph/cognigraph-enterprise@sha256:' + 'd' * 64]
        self.assertEqual(self.manifest.call_args_list, [
            call('cognigraph/cognigraph', VERSION, community),
            call('cognigraph/cognigraph-enterprise', VERSION, enterprise),
            call('cognigraph/cognigraph', 'latest', community),
            call('cognigraph/cognigraph-enterprise', 'latest', enterprise)])
        self.assertEqual([c.args[1] for c in self.verify.call_args_list], [
            '1.2.3-amd64', '1.2.3-arm64', '1.2.3-amd64', '1.2.3-arm64', VERSION, VERSION, 'latest', 'latest'])

    def test_unverified_architecture_tag_prevents_every_index(self):
        self.verify.side_effect = [None, None, None, RuntimeError('registry digest mismatch')]
        with contextlib.redirect_stdout(io.StringIO()), self.assertRaises(RuntimeError):
            images.publish(VERSION, REVISION)
        self.assertEqual(self.push.call_count, 4)
        self.manifest.assert_not_called()

    def test_unverified_index_prevents_all_alias_updates(self):
        self.verify.side_effect = [None] * 5 + [RuntimeError('registry digest mismatch')]
        with contextlib.redirect_stdout(io.StringIO()), self.assertRaises(RuntimeError):
            images.publish(VERSION, REVISION)
        self.assertEqual(self.manifest.call_count, 2)

    def test_main_or_latest_drift_after_index_publication_prevents_alias_updates(self):
        for drift in ('main', 'latest'):
            self.manifest.reset_mock()
            self.manifest.side_effect = ['sha256:' + c * 64 for c in 'efef']
            self.push.side_effect = ['sha256:' + c * 64 for c in 'abcd']
            self.current.side_effect = [None, None, RuntimeError('Main changed')] if drift == 'main' else None
            self.tag.side_effect = [None, None, DIGEST] if drift == 'latest' else None
            with self.subTest(drift=drift), contextlib.redirect_stdout(io.StringIO()), \
                    self.assertRaises(RuntimeError):
                images.publish(VERSION, REVISION)
            self.assertEqual(self.manifest.call_count, 2)

    def test_alias_index_must_equal_its_version_index(self):
        self.manifest.side_effect = ['sha256:' + c * 64 for c in 'efe'] + ['sha256:' + '9' * 64]
        with contextlib.redirect_stdout(io.StringIO()), self.assertRaisesRegex(RuntimeError, 'alias differs'):
            images.publish(VERSION, REVISION)
        self.assertEqual(self.manifest.call_count, 4)
        self.assertEqual(self.verify.call_count, 7)

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
            self.assertIn(f'cognigraph/cognigraph:{VERSION}-amd64@{digest}', summary.read_text())
            self.assertNotIn('arm64', summary.read_text())

    def test_publish_host_must_be_one_of_the_tested_platforms(self):
        with patch.object(images, 'host_platform', return_value='linux/386'):
            with self.assertRaisesRegex(RuntimeError, 'linux/386'):
                images.publish(VERSION, REVISION)
        self.smoke.assert_not_called()
        self.push.assert_not_called()


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
