"""Isolation guarantees for the browser runner; live behavior is in ui/e2e."""
import os
import sys
from pathlib import Path
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import ui_browser


class BrowserIsolation(unittest.TestCase):
    def test_server_and_browser_environment_excludes_user_configuration(self):
        with patch.dict(os.environ, {
            'PATH': '/synthetic/bin', 'HOME': '/synthetic/home',
            'COGNIGRAPH_NATIVE_PATH': '/must/not/open',
            'COGNIGRAPH_AUTH_ENABLED': 'false', 'OPENAI_API_KEY': 'synthetic',
            'ARANGO_PASSWORD': 'synthetic', 'UNKNOWN_PROVIDER_KEY': 'synthetic',
            'CG_UI_TEST_ORIGIN': 'https://must-not-contact.invalid',
            'NODE_OPTIONS': '--require=/must/not/load',
        }, clear=True):
            self.assertEqual(ui_browser.isolated_env(), {
                'PATH': '/synthetic/bin', 'HOME': '/synthetic/home',
            })

    def test_missing_production_candidate_fails_before_building_or_starting_server(self):
        with patch.object(sys, 'argv', ['ui_browser.py', '--dist', '/missing-cg60-candidate']), \
             patch.object(ui_browser, 'run_edition') as run:
            with self.assertRaisesRegex(RuntimeError, 'Build the production UI first'):
                ui_browser.main()
        run.assert_not_called()


if __name__ == '__main__':
    unittest.main()
