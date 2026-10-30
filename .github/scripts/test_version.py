"""Exercise release decisions against real temporary Git histories."""
import contextlib
import io
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

import version


class SemverTests(unittest.TestCase):
    def test_precedence(self):
        ordered = ['0.1.9', '0.2.0-alpha', '0.2.0-alpha.1', '0.2.0-alpha.2',
                   '0.2.0-alpha.10', '0.2.0-beta', '0.2.0-rc.1', '0.2.0', '1.0.0']
        self.assertEqual(sorted(ordered, key=version.Version), ordered)

    def test_invalid_versions(self):
        for value in ['v1.2.3', '01.2.3', '1.2', '1.2.3-', '1.2.3-01',
                      '1.2.3-a..b', '1.2.3+build', '1.2.3-a/b']:
            with self.subTest(value=value), self.assertRaises(ValueError):
                version.Version(value)


class RepositoryFixture:
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.patch = patch.object(version, 'ROOT', self.root)
        self.patch.start()
        self.addCleanup(self.patch.stop)
        self.env_patch = patch.dict('os.environ', {'LABELS': '', 'GITHUB_OUTPUT': ''})
        self.env_patch.start()
        self.addCleanup(self.env_patch.stop)
        for directory in ['backend', 'frontend']:
            (self.root / directory).mkdir()
        self.write_version('0.1.2')
        self.git('init', '-q')
        self.git('config', 'tag.gpgSign', 'false')
        self.git('config', 'commit.gpgSign', 'false')
        self.git('config', 'core.hooksPath', '/dev/null')
        self.git('config', 'user.name', 'Test')
        self.git('config', 'user.email', 'test@example.org')
        self.commit()
        self.git('tag', 'v0.1.2')
        self.base = version.git('rev-parse', 'HEAD')

    def git(self, *args):
        return version.git(*args)

    def commit(self):
        self.git('add', '.')
        self.git('commit', '-qm', 'test')

    def write_version(self, value):
        (self.root / 'backend/Cargo.toml').write_text(
            f'[package]\nname = "oxi-email-server"\nversion = "{value}"\n')
        (self.root / 'backend/Cargo.lock').write_text(
            f'[[package]]\nname = "oxi-email-server"\nversion = "{value}"\n'
            '[[package]]\nname = "dependency"\nversion = "0.1.2"\n')
        (self.root / 'frontend/package.json').write_text(json.dumps({'version': value}))

    def plan(self, base=None):
        output = io.StringIO()
        with contextlib.redirect_stdout(output):
            version.plan(base)
        return dict(line.split('=', 1) for line in output.getvalue().splitlines())


class RepositoryTests(RepositoryFixture, unittest.TestCase):
    def test_patch_default_and_dependencies_untouched(self):
        version.bump('patch')
        self.assertEqual(version.current().value, '0.1.3')
        self.assertIn('name = "dependency"\nversion = "0.1.2"',
                      (self.root / 'backend/Cargo.lock').read_text())
        version.check(self.base)

    def test_explicit_and_prerelease_bumps(self):
        version.bump('minor')
        self.assertEqual(version.current().value, '0.2.0')
        version.bump('1.0.0-rc.1')
        self.assertEqual(self.plan()['prerelease'], 'true')
        version.bump('1.0.0')
        self.assertEqual(self.plan()['prerelease'], 'false')

    def test_no_release_and_rollback(self):
        with self.assertRaises(ValueError):
            version.check(self.base)
        version.check(self.base, allow_same=True)
        self.write_version('0.1.1')
        with self.assertRaises(ValueError):
            version.check(self.base, allow_same=True)

    def test_lock_mismatch_even_with_exemption(self):
        (self.root / 'frontend/package.json').write_text('{"version":"0.1.1"}')
        with self.assertRaises(ValueError):
            version.check(self.base, allow_same=True)

    def test_unchanged_push_does_not_release_an_untagged_failed_bump(self):
        version.bump('patch')
        self.commit()
        failed = self.git('rev-parse', 'HEAD')
        (self.root / 'README.md').write_text('later docs')
        self.commit()
        self.assertEqual(self.plan(failed)['publish'], 'false')
        self.assertEqual(self.plan(self.base)['publish'], 'true')

    def test_new_release_and_same_commit_retry(self):
        version.bump('patch')
        self.commit()
        self.assertEqual(self.plan()['publish'], 'true')
        self.git('tag', 'v0.1.3')
        self.assertEqual(self.plan()['publish'], 'true')

    def test_unchanged_version_after_release_is_not_released(self):
        (self.root / 'README.md').write_text('docs')
        self.commit()
        self.assertEqual(self.plan()['publish'], 'false')

    def test_tag_on_unrelated_history_is_rejected(self):
        self.git('checkout', '--orphan', 'other')
        (self.root / 'unrelated').write_text('different tree')
        self.commit()
        with self.assertRaises(ValueError):
            self.plan()

    def test_old_rerun_cannot_roll_back_aliases(self):
        version.bump('patch')
        self.commit()
        self.git('tag', 'v0.1.3')
        self.git('checkout', '--detach', self.base)
        self.assertEqual(self.plan()['publish'], 'false')

    def test_untagged_version_below_published_version_rejected(self):
        self.git('tag', 'v0.2.0')
        version.bump('patch')
        with self.assertRaises(ValueError):
            self.plan()


if __name__ == '__main__':
    unittest.main()
