"""A restored vulnerable manifest or changed source must fail the vendor gate."""
import importlib.util
from pathlib import Path
import shutil
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]
spec = importlib.util.spec_from_file_location('vendored', ROOT / 'scripts/check-vendored.py')
vendored = importlib.util.module_from_spec(spec)
spec.loader.exec_module(vendored)


class VendoredIntegrity(unittest.TestCase):
    def test_reviewed_snapshot_passes(self):
        vendored.check()

    def test_unreviewed_changes_fail(self):
        mutations = {
            'Cargo.toml': lambda text: text.replace('version = "0.18.2"', 'version = "0.16.3"'),
            'src/store/reader.rs': lambda text: text + '\n// unreviewed change\n',
            'extra.rs': lambda text: 'unreviewed source',
            'LICENSE': lambda text: None,
        }
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            for name, mutate in mutations.items():
                with self.subTest(path=name):
                    shutil.copytree(ROOT / 'vendor', root / 'vendor')
                    path = root / 'vendor/tantivy-0.26.2' / name
                    changed = mutate(path.read_text() if path.exists() else '')
                    if changed is None:
                        path.unlink()
                    else:
                        path.write_text(changed)
                    with self.assertRaises(ValueError):
                        vendored.check(root)
                    shutil.rmtree(root / 'vendor')


if __name__ == '__main__':
    unittest.main()
