#!/usr/bin/env python3
"""Dependency-free release tooling. Run from any directory with Python 3.11+."""
import argparse
from functools import total_ordering
import json
import os
from pathlib import Path
import re
import subprocess
import sys
import tomllib

ROOT = Path(__file__).resolve().parents[2]
SEMVER = re.compile(
    r"(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)"
    r"(?:-((?:0|[1-9][0-9]*|[0-9]*[A-Za-z-][0-9A-Za-z-]*)"
    r"(?:\.(?:0|[1-9][0-9]*|[0-9]*[A-Za-z-][0-9A-Za-z-]*))*))?"
)


@total_ordering
class Version:
    def __init__(self, value):
        match = SEMVER.fullmatch(value)
        if not match:
            raise ValueError(f"Expected MAJOR.MINOR.PATCH[-PRERELEASE], got {value!r}")
        self.value = value
        self.core = tuple(map(int, match.groups()[:3]))
        self.pre = match[4].split('.') if match[4] else []

    def __eq__(self, other):
        return (self.core, self.pre) == (other.core, other.pre)

    def __lt__(self, other):
        if self.core != other.core:
            return self.core < other.core
        if not self.pre or not other.pre:
            return bool(self.pre) and not other.pre
        for left, right in zip(self.pre, other.pre):
            if left == right:
                continue
            if left.isdigit() and right.isdigit():
                return int(left) < int(right)
            if left.isdigit() != right.isdigit():
                return left.isdigit()
            return left < right
        return len(self.pre) < len(other.pre)


def git(*args):
    return subprocess.check_output(['git', *args], cwd=ROOT, text=True).strip()


def current():
    manifest = tomllib.loads((ROOT / 'backend/Cargo.toml').read_text())['package']
    version = Version(manifest['version'])
    lock = tomllib.loads((ROOT / 'backend/Cargo.lock').read_text())
    versions = [p['version'] for p in lock['package'] if p['name'] == manifest['name']]
    frontend = json.loads((ROOT / 'frontend/package.json').read_text())['version']
    if versions != [version.value] or frontend != version.value:
        raise ValueError('Cargo.toml, Cargo.lock and package.json must agree; use version.py bump')
    return version


def check(base, allow_same=False):
    version = current()
    previous = Version(tomllib.loads(git('show', f'{base}:backend/Cargo.toml'))['package']['version'])
    if version < previous or (version == previous and not allow_same):
        raise ValueError(f'Version must increase from {previous.value}; got {version.value}. '
                         'Use version.py bump (or the no-release PR label for no release).')
    print(f'Version: {previous.value} -> {version.value}')


def bump(target):
    version = current()
    if target in ('patch', 'minor', 'major'):
        major, minor, patch = version.core
        target = {'patch': f'{major}.{minor}.{patch + 1}',
                  'minor': f'{major}.{minor + 1}.0', 'major': f'{major + 1}.0.0'}[target]
    updated = Version(target)
    if updated <= version:
        raise ValueError(f'New version must exceed {version.value}')
    # Only replace the package's version; dependency versions are untouched.
    paths = ['backend/Cargo.toml', 'backend/Cargo.lock', 'frontend/package.json']
    changes = {}
    for path in paths:
        file = ROOT / path
        text = file.read_text()
        if path.endswith('.json'):
            text, count = re.subn(r'("version":\s*")[^"]+(")',
                                  lambda m: m[1] + target + m[2], text, count=1)
        else:
            name = 'oxi-email-server'
            pattern = rf'(name = "{name}"\nversion = ")[^"]+(")'
            text, count = re.subn(pattern, lambda m: m[1] + target + m[2], text, count=1)
        if count != 1:
            raise ValueError(f'Cannot locate package version in {path}')
        changes[file] = text
    for file, text in changes.items():
        file.write_text(text)
    print(f'{version.value} -> {current().value}')


def recovery_base(base):
    """Validate the original pre-push commit for a manual publication retry."""
    if not re.fullmatch(r'[0-9a-f]{40}', base):
        raise ValueError('Supply release-base as the full original pre-push commit SHA')
    head = git('rev-parse', 'HEAD')
    # Resolve as a commit, not an arbitrary ref or Git revision expression.
    if git('rev-parse', '--verify', f'{base}^{{commit}}') != base:
        raise ValueError('Recovery base must identify a commit')
    if base == head or subprocess.run(
        ['git', 'merge-base', '--is-ancestor', base, head], cwd=ROOT,
    ).returncode:
        raise ValueError('Recovery base must be a strict ancestor of the validated commit')
    previous = Version(tomllib.loads(git('show', f'{base}:backend/Cargo.toml'))['package']['version'])
    if current() <= previous:
        raise ValueError('Recovery base must precede the version bump; use the original pre-push SHA')
    return base


def plan(base=None):
    version = current()
    tag = f'v{version.value}'
    head = git('rev-parse', 'HEAD')
    tags = git('tag', '--list').splitlines()
    if tag in tags:
        tagged = git('rev-parse', f'{tag}^{{commit}}')
        if subprocess.run(['git', 'merge-base', '--is-ancestor', tagged, head], cwd=ROOT).returncode:
            raise ValueError(f'{tag} belongs to another history; tags are immutable')
        publish = tagged == head  # Retry at the original commit, never a later one.
    else:
        publish = True
        if base and set(base) != {'0'}:
            previous = Version(tomllib.loads(git('show', f'{base}:backend/Cargo.toml'))['package']['version'])
            publish = version > previous  # A later unchanged push must not release a failed bump.
    versions = []
    for existing in tags:
        if existing.startswith('v') and SEMVER.fullmatch(existing[1:]):
            versions.append(Version(existing[1:]))
    if versions and version < max(versions):
        if tag not in tags:
            raise ValueError(f'{tag} is older than an existing release; choose a higher version')
        publish = False  # An old rerun must never roll back floating Docker tags.
    outputs = {'version': version.value, 'tag': tag, 'publish': str(publish).lower(),
               'prerelease': str(bool(version.pre)).lower()}
    for key, value in outputs.items():
        print(f'{key}={value}')
    if os.environ.get('GITHUB_OUTPUT'):
        with open(os.environ['GITHUB_OUTPUT'], 'a') as output:
            output.writelines(f'{key}={value}\n' for key, value in outputs.items())


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    sub = parser.add_subparsers(dest='action', required=True)
    sub.add_parser('current')
    bump_parser = sub.add_parser('bump')
    bump_parser.add_argument('target', nargs='?', default='patch', help='patch (default), minor, major, or exact version')
    check_parser = sub.add_parser('check')
    check_parser.add_argument('--base', required=True)
    check_parser.add_argument('--allow-same', action='store_true')
    recovery_parser = sub.add_parser('recovery-base')
    recovery_parser.add_argument('--base', required=True)
    plan_parser = sub.add_parser('plan')
    plan_parser.add_argument('--base', help='pre-push commit; unchanged pushes do not release')
    args = parser.parse_args()
    if args.action == 'current':
        print(current().value)
    elif args.action == 'bump':
        bump(args.target)
    elif args.action == 'recovery-base':
        print(recovery_base(args.base))
    elif args.action == 'check':
        check(args.base, args.allow_same or 'no-release' in os.environ.get('LABELS', '').split(','))
    else:
        plan(args.base)


if __name__ == '__main__':
    try:
        main()
    except (ValueError, subprocess.CalledProcessError) as error:
        sys.exit(f'::error::{error}')
