# Release process

oxi uses Semantic Versioning with Git tags named `vMAJOR.MINOR.PATCH`.
The 0.x series is beta software and is published as GitHub prereleases. Breaking
changes are possible before 1.0; document them in each release's notes.

- Patch releases contain fixes; minor releases add features.
- For release candidates or additional beta iterations, use a SemVer suffix such
  as `v0.2.0-beta.1` and use the same version without `v` in package manifests.
- Never move or overwrite a published release tag. Publish a new version instead.

## Prepare

1. Start a release branch from the current `main`.
2. Set the same version in `backend/Cargo.toml` and `frontend/package.json`.
   Also update the frontend version in `nix/frontend.nix`.
   Update the `oxi-email-server` entry in `backend/Cargo.lock` with Cargo and
   commit any resulting lockfile changes. Do not update dependencies incidentally.
3. Move the relevant `CHANGELOG.md` entries out of `Unreleased` into a dated
   version section. Add release notes under `docs/releases/vVERSION.md`, including
   upgrade steps and known limitations. Update the current release and pinned
   Docker examples in `README.md`.
4. Open a pull request against `main`. Merge only after CI passes (Rust lint and
   tests, frontend lint/tests/build, and Docker build).

## Tag and publish

Run these commands from a clean checkout after merging the release pull request,
replacing `0.1.0` with the version being released:

```bash
git fetch origin --tags
git switch main
git pull --ff-only origin main
git tag -a v0.1.0 -m "oxi v0.1.0 (Beta)"
git push origin v0.1.0
```

The tag triggers CI again. It verifies that the tag matches both package versions
and the backend lockfile, then runs all tests and builds. After those checks pass,
it publishes Docker images to GHCR with both `v0.1.0` and `0.1.0` tags (and a SHA
tag). Releases do not update `latest`; that tag continues to track `main`.
No floating minor or major release tags are published.

Wait for the tag's CI run to succeed and verify that the image exists:

```bash
gh run list --workflow ci.yml --branch v0.1.0
docker buildx imagetools inspect ghcr.io/c0h1b4/oxi:v0.1.0
```

Then publish the GitHub release against the existing tag:

```bash
gh release create v0.1.0 --verify-tag \
  --title "oxi v0.1.0 (Beta)" \
  --notes-file docs/releases/v0.1.0.md \
  --prerelease --latest=false
```

Use `--prerelease` for all 0.x releases and any version with a prerelease suffix.
For a stable release at or after 1.0, omit `--prerelease` and use `--latest`.
GitHub provides source archives automatically. Standalone binaries are not
currently built or attached; the supported packaged distribution is the Docker
image. If tag CI fails, fix the issue on `main` and prepare a new version rather
than moving the failed tag.

## Upgrade

Read the release notes, back up the persistent `/data` volume, and replace the
image tag in your deployment. Keep the previous image reference and data backup
until the upgrade is verified. Downgrades may require restoring that backup if
a release changes the cache schema.
