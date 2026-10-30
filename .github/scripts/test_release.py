"""Test publication and failure recovery with a local remote and a fake GitHub API."""
import json
import os
from pathlib import Path
import subprocess
import unittest

from test_version import RepositoryFixture

SCRIPTS = Path(__file__).resolve().parent


class ReleaseTests(RepositoryFixture, unittest.TestCase):
    def setUp(self):
        super().setUp()
        target = self.root / '.github/scripts'
        target.mkdir(parents=True)
        for name in ['version.py', 'release.sh']:
            (target / name).write_text((SCRIPTS / name).read_text())
        self.git('init', '--bare', '-q', str(self.root / 'remote.git'))
        self.git('remote', 'add', 'origin', str(self.root / 'remote.git'))
        # Keep the bare remote out of the test commit.
        (self.root / '.gitignore').write_text('remote.git/\n__pycache__/\nbin/\n')
        self.write_version('0.1.3')
        self.commit()
        self.git('push', '-q', 'origin', 'HEAD:refs/heads/main', '--tags')
        binary = self.root / 'bin'
        binary.mkdir()
        gh = binary / 'gh'
        gh.write_text('''#!/usr/bin/env python3
import json, os, sys
from pathlib import Path
if sys.argv[1] == 'api':
    if os.environ.get('API_FAIL'):
        sys.exit(1)
    print(os.environ.get('RELEASES', '[]'))
else:
    Path('gh-args.json').write_text(json.dumps(sys.argv[1:]))
    if os.environ.get('CREATE_FAIL'):
        sys.exit(1)
''')
        gh.chmod(0o755)
        self.env = dict(os.environ, PATH=f'{binary}:{os.environ["PATH"]}',
                        RELEASE_SHA=self.git('rev-parse', 'HEAD'), RELEASES='[]')

    def release(self, **env):
        return subprocess.run(['bash', '.github/scripts/release.sh'], cwd=self.root,
                              env={**self.env, **env}, capture_output=True, text=True)

    def test_publish_and_recover_after_tag_push(self):
        result = self.release(CREATE_FAIL='1')
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual(self.git('rev-parse', 'v0.1.3^{commit}'), self.env['RELEASE_SHA'])
        result = self.release()
        self.assertEqual(result.returncode, 0, result.stderr)
        args = json.loads((self.root / 'gh-args.json').read_text())
        self.assertIn('--verify-tag', args)
        self.assertIn('--latest', args)
        self.assertIn('--generate-notes', args)

    def test_api_failure_does_not_create_a_tag(self):
        self.assertNotEqual(self.release(API_FAIL='1').returncode, 0)
        self.assertNotIn('v0.1.3', self.git('tag', '--list'))

    def test_existing_release_is_idempotent(self):
        self.assertEqual(self.release().returncode, 0)
        (self.root / 'gh-args.json').unlink()
        result = self.release(RELEASES='[{"tag_name":"v0.1.3"}]')
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertFalse((self.root / 'gh-args.json').exists())

    def test_release_without_tag_is_rejected(self):
        result = self.release(RELEASES='[{"tag_name":"v0.1.3"}]')
        self.assertNotEqual(result.returncode, 0)
        self.assertNotIn('v0.1.3', self.git('tag', '--list'))

    def test_wrong_checkout_is_rejected(self):
        self.assertNotEqual(self.release(RELEASE_SHA=self.base).returncode, 0)
        self.assertNotIn('v0.1.3', self.git('tag', '--list'))

    def test_prerelease_does_not_become_latest(self):
        self.write_version('0.2.0-rc.1')
        self.commit()
        self.env['RELEASE_SHA'] = self.git('rev-parse', 'HEAD')
        result = self.release()
        self.assertEqual(result.returncode, 0, result.stderr)
        args = json.loads((self.root / 'gh-args.json').read_text())
        self.assertIn('--prerelease', args)
        self.assertIn('--latest=false', args)


if __name__ == '__main__':
    unittest.main()
