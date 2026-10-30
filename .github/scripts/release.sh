#!/usr/bin/env bash
# Called only after tests and GHCR publication succeed, on the validated checkout.
set -euo pipefail
: "${RELEASE_SHA:?set RELEASE_SHA to the validated commit}"
[[ "$(git rev-parse HEAD)" == "$RELEASE_SHA" ]] || { echo 'Wrong checkout' >&2; exit 1; }
git fetch origin --tags
PLAN=$(python3 .github/scripts/version.py plan)
grep -qx 'publish=true' <<<"$PLAN" || { echo 'No release needed'; exit 0; }
VERSION=$(python3 .github/scripts/version.py current)
TAG="v$VERSION"

# An API error is fatal, never interpreted as a missing release.
RELEASES=$(gh api --paginate 'repos/{owner}/{repo}/releases?per_page=100')
if jq -es --arg tag "$TAG" 'any(.[][]; .tag_name == $tag)' <<<"$RELEASES" >/dev/null; then
  git show-ref --verify --quiet "refs/tags/$TAG" || {
    echo "Release $TAG exists without its tag; refusing to recreate it" >&2
    exit 1
  }
  echo "Release $TAG already exists"
  exit 0
fi
if ! git show-ref --verify --quiet "refs/tags/$TAG"; then
  git config user.name 'github-actions[bot]'
  git config user.email '41898282+github-actions[bot]@users.noreply.github.com'
  git tag -a "$TAG" "$RELEASE_SHA" -m "oxi $TAG"
  git push origin "refs/tags/$TAG"
fi
TITLE="oxi $TAG"
[[ "$VERSION" != 0.* || "$VERSION" == *-* ]] || TITLE+=" (Beta)"
ARGS=(--verify-tag --title "$TITLE" --generate-notes)
if [[ "$VERSION" == *-* ]]; then
  ARGS+=(--prerelease --latest=false)
else
  ARGS+=(--latest)
fi
if [[ -f "docs/releases/$TAG.md" ]]; then
  ARGS+=(--notes-file "docs/releases/$TAG.md")
fi
gh release create "$TAG" "${ARGS[@]}"
