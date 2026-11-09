#!/usr/bin/env bash
# Bumps the version, commits "Release x.y.z", tags vx.y.z, pushes, and
# starts the Release workflow, which builds the installers and creates a
# draft GitHub release with them attached.
#
#   scripts/release.sh patch                        0.3.0 -> 0.3.1, all platforms
#   scripts/release.sh minor --platforms windows    only the Windows installer
#   scripts/release.sh 1.0.0 --platforms windows,linux
#   scripts/release.sh patch --dry-run              show what would happen
#
# Platforms: windows, linux, macos (default: all).
set -euo pipefail
cd "$(dirname "$0")/.."

fail() { echo "release: $*" >&2; exit 1; }

bump="" platforms="windows,linux,macos" dry_run=0
while [ $# -gt 0 ]; do
  case "$1" in
    --platforms) platforms="${2:-}"; shift 2 ;;
    --dry-run) dry_run=1; shift ;;
    -*) fail "unknown option $1" ;;
    *) bump="$1"; shift ;;
  esac
done
[ -n "$bump" ] || fail "usage: scripts/release.sh <patch|minor|major|x.y.z> [--platforms windows,linux,macos] [--dry-run]"
platforms=$(tr -d ' ' <<< "$platforms" | tr 'A-Z' 'a-z')
for p in ${platforms//,/ }; do
  case "$p" in windows | linux | macos) ;; *) fail "unknown platform $p (use windows, linux, macos)" ;; esac
done

current=$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)
IFS=. read -r major minor patch <<< "$current"
case "$bump" in
  major) next="$((major + 1)).0.0" ;;
  minor) next="$major.$((minor + 1)).0" ;;
  patch) next="$major.$minor.$((patch + 1))" ;;
  *) [[ "$bump" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]] || fail "\"$bump\" isn't patch, minor, major or a version like 1.2.3"; next="$bump" ;;
esac
tag="v$next"

[ "$(git rev-parse --abbrev-ref HEAD)" = main ] || fail "run it from the main branch"
[ -z "$(git status --porcelain)" ] || fail "commit or stash your changes first"
[ -z "$(git tag --list "$tag")" ] || fail "tag $tag already exists"

echo "Release $current -> $next ($tag) for $platforms"
if [ "$dry_run" = 1 ]; then echo "Dry run: nothing changed."; exit 0; fi

# The first `version =` in Cargo.toml is the [package] one.
sed -i "0,/^version = \".*\"/s//version = \"$next\"/" Cargo.toml
cargo update --workspace --offline --quiet 2>/dev/null || cargo update --workspace --quiet
grep -q "^version = \"$next\"" Cargo.toml || fail "couldn't update the version in Cargo.toml"

git add Cargo.toml Cargo.lock
git commit -m "Release $next"
git tag -a "$tag" -m "Release $next"
git push origin main
git push origin "$tag"
gh workflow run release.yml -f tag="$tag" -f platforms="$platforms"

echo
echo "Building $tag. The draft release shows up when the build finishes:"
echo "  $(gh repo view --json url --jq .url)/releases"
echo "Publish it with: gh release edit $tag --draft=false"
