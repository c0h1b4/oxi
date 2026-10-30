# Release process

oxi uses SemVer and publishes from validated `main` commits, following
[Pantheon's version-gated release model](https://github.com/GustavoWidman/pantheon/blob/main/.github/workflows/release.yml).
The version in `backend/Cargo.toml` is authoritative. CI requires the backend
lockfile and frontend manifest to agree; Nix reads the frontend manifest directly.
Python 3.11+ is required for the release helper.

## Everyday changes

Prefer one small release per user-facing PR rather than batching unrelated fixes.
Before opening a PR against `main`, run from the repository root:

```bash
python3 .github/scripts/version.py bump          # patch: 0.1.2 -> 0.1.3
python3 .github/scripts/version.py bump minor    # feature: 0.1.2 -> 0.2.0
python3 .github/scripts/version.py bump major    # breaking stable API: 1.2.3 -> 2.0.0
```

Choose one command. The helper updates both manifests and the backend lockfile
without changing dependencies. Commit those changes with the feature or fix.
For breaking changes during 0.x, increment the minor version and document the
migration; the 0.x series remains beta software. After 1.0, breaking changes
require a major version bump.

PRs targeting `main` must increase the version relative to their base. Rebase and
bump again if another PR has already used your version. Maintainers can apply the
`no-release` label for documentation, tests, or maintenance changes that should
keep the existing version. The label exempts only the bump requirement: version
mismatches and downgrades still fail, and a PR that bumps the version still releases.
Protect `main` with the Version policy, backend, frontend, and Docker checks.

Keep noteworthy changes in `CHANGELOG.md`. Release notes are generated from GitHub
PR history automatically. For migration instructions or a curated announcement,
add `docs/releases/vVERSION.md`; its contents precede the generated notes.
A hand-written release announcement is optional.

## Automatic publishing

On a push to `main`, CI checks version consistency and rejects version decreases,
runs Rust lint/tests and frontend lint/tests/typecheck/build, then builds Docker.
When the version is new, it publishes the image, creates an immutable annotated
`vVERSION` tag at that exact tested commit, and publishes a GitHub release.
The same workflow performs all steps; it does not rely on a bot-created tag
triggering another Actions run. No additional PAT is required.

Docker tags:

| Tag | Meaning |
| --- | --- |
| `v0.1.3`, `0.1.3` | Exact release version; recommended for deployments |
| `0.1` | Latest stable release within that minor series |
| `1` (once 1.x exists) | Latest stable release within that major series; no `0` alias |
| `latest` | Latest release without a prerelease suffix, including numbered 0.x betas |
| `edge` | Latest successful push build of `main`, including unreleased changes |
| `sha-…` | A successful build of a specific commit |

A push that retains an already released version updates only `edge` and its SHA
image. PRs and `develop` builds validate Docker without publishing. Prereleases
publish exact version and SHA tags and are marked prerelease on GitHub; they do
not update `latest`, minor, or major aliases. Numbered 0.x releases are titled
Beta but marked Latest on GitHub.

## Overrides and recovery

Prefer the normal patch/minor/major helper over manually choosing a version.
For an intentional jump or prerelease, supply an exact version:

```bash
python3 .github/scripts/version.py bump 0.2.0-rc.1
# Later, promote with a new commit:
python3 .github/scripts/version.py bump 0.2.0
```

Exact versions must increase and follow `MAJOR.MINOR.PATCH[-PRERELEASE]`.
Build metadata (`+...`) is unsupported because Docker tags cannot contain `+`.
Do not override the version only in the Actions UI or manually push release tags;
commit the version so artifacts and manifests always agree.

If publication fails, rerun the failed CI run at the same commit. Alternatively,
run CI with **Run workflow** on `main` while that release commit is still its head.
The workflow can finish after a tag was pushed but the GitHub release failed.
An existing tag is never moved, and a later unchanged commit cannot take over an
older tag. Old reruns with a newer version tag already present do not republish
release aliases. If a newer commit has superseded a failed release, bump to a new
version instead. The full validation suite also runs for manual dispatches.

## Upgrade

Read the release notes, back up the persistent `/data` volume, and replace the
image tag in your deployment. Keep the previous image reference and data backup
until the upgrade is verified. Downgrades may require restoring that backup if
a release changes the cache schema. GitHub provides source archives; Docker is
the packaged distribution, and standalone binaries are not published.
