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
Protect `main` with the single required **CI Gate** check. Remove the old
Version policy, backend, frontend, and Docker requirements when migrating.

Keep noteworthy changes in `CHANGELOG.md`. Release notes are generated from GitHub
PR history automatically. For migration instructions or a curated announcement,
add `docs/releases/vVERSION.md`; its contents precede the generated notes.
A hand-written release announcement is optional.

## Automatic publishing

`ci-gate.yml` calls reusable `ci-version.yml`, `ci-backend.yml`,
`ci-frontend.yml`, and `ci-nix.yml` workflows in parallel. The final **CI Gate**
job runs even when a dependency fails and requires every workflow to succeed;
cancelled or skipped checks cannot pass the gate. Pull requests, merge queues,
`develop`/`main` pushes, and manual CI dispatches use this gate. CI has read-only
repository permissions and never receives registry credentials.

`cd-gate.yml` starts only after successful main push/manual CI runs from this
repository. It calls reusable workflows to plan the version, build and publish
the container to **GHCR and Docker Hub**, then tag the exact tested commit and
publish a GitHub release. It checks out the triggering CI run's SHA, rather than
`workflow_run`'s default-branch SHA, and retrieves the original push boundary
from that run's `release-context` artifact. This preserves version planning for
multi-commit pushes. Superseded main runs are skipped before building. CD is
serialized separately from CI so an old rerun cannot roll image aliases back.
The workflows must be merged into the default branch before `workflow_run`
can trigger CD. Bot-created tags do not need to trigger another Actions run;
no additional PAT is required.

Configure these repository settings before enabling publication:

| Setting | Type | Value |
| --- | --- | --- |
| `DOCKERHUB_USERNAME` | Actions secret | Docker Hub login/user |
| `DOCKERHUB_TOKEN` | Actions secret | Access token with push access |
| `DOCKERHUB_IMAGE` | Actions variable | Lowercase `namespace/image`, e.g. `c0h1b4/oxi` |

GHCR uses the built-in `GITHUB_TOKEN` with package write permission. Both
registries receive the same build and tags. Missing Docker Hub settings fail CD
before a build; the GitHub release is published only after both registry pushes
succeed. Partial publication can be recovered by rerunning CD at the same SHA.
A future Nix binary-cache publisher belongs in CD, alongside the container job.

### CI speed and Nix coverage

Required CI targets **under five minutes**: leaf jobs have four-minute timeouts,
the final gate has a one-minute timeout, and obsolete CI runs are cancelled.
GitHub queue delays, runner startup and cold dependency compilation prevent an
absolute wall-clock guarantee; timeouts fail checks rather than silently dropping
coverage. Measure hosted runs before treating five minutes as an achieved SLA.
Docker compilation and publication are outside the required CI critical path.

Rust lint and tests run in parallel with separate Cargo caches and debug symbols
disabled. Frontend quality checks and static export run in parallel, caching Bun
downloads and Next's compiler cache by lockfile with source-commit updates.
Docker uses its own GitHub Actions BuildKit cache in CD.

Required Nix CI first evaluates the package drvPath to materialize sources
needed by the pinned bun2nix, then runs `nix flake check --no-build` with lockfile mutation disabled
on x86_64 Linux. This evaluates package/check derivations and NixOS module
assertions; it does **not** compile Rust/Next or execute the static asset checks.
The Nix store is cached by OS, architecture, evaluation/build mode, lockfile and
commit, with fallback to earlier caches. See the
[cache action documentation](https://github.com/nix-community/cache-nix-action)
for restore behavior. No external cache credentials are needed.

For full validation, run **Nix build (manual)** (`nix-build.yml`) on the desired
branch. It calls the same reusable Nix workflow with `build: true`, runs the
actual flake checks (including both frontend base paths), and builds `.#default`.
This has a separate 30-minute limit and store cache, outside the five-minute
required CI budget. Crane already separates Rust dependency artifacts from
application sources, so warm full builds can reuse dependency compilation.
Cold full builds are deliberately not required until measured timings and a
published binary cache support that budget. Darwin/aarch64 builds are not covered.

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
image. PRs and `develop` runs validate the application and Nix evaluation;
container builds run only in main CD. Prereleases
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

If publication fails, rerun the failed **CD Gate** run at the same commit while
it is still main HEAD. The CI `release-context` artifact is retained for 14 days.
If it has expired, run **CI Gate** with **Run workflow** on `main` to revalidate
and regenerate it; successful CI triggers CD again.
The workflow can finish after a tag was pushed but the GitHub release failed.
An existing tag is never moved, and a later unchanged commit cannot take over an
older tag. Old reruns with a newer version tag already present do not republish
release aliases. If a newer commit has superseded a failed release, bump to a new
version instead. Required CI validation also runs for manual CI dispatches; full Nix builds
remain a separate manual workflow.

## Upgrade

Read the release notes, back up the persistent `/data` volume, and replace the
image tag in your deployment. Keep the previous image reference and data backup
until the upgrade is verified. Downgrades may require restoring that backup if
a release changes the cache schema. GitHub provides source archives; Docker is
the packaged distribution, and standalone binaries are not published.
