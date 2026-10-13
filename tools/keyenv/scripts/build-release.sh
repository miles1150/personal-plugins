#!/bin/sh
# Explicit whitelist: never archive the checkout or local private state.
set -eu
root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
. "$root/scripts/checksums.sh"
tag=${1:?Usage: build-release.sh keyenv-vVERSION [ABSOLUTE_NEW_OUTPUT_DIR]}
version=${tag#keyenv-v}
printf '%s\n' "$tag" | LC_ALL=C grep -Eq '^keyenv-v[0-9]+\.[0-9]+\.[0-9]+(-[a-z0-9]+([.-][a-z0-9]+)*)?$' || { echo 'Invalid keyenv release tag' >&2; exit 2; }
out=${2:-"$root/dist/$tag"}
case "$out" in /*) ;; *) exit 2;; esac
[ ! -e "$out" ] || { echo 'Output already exists' >&2; exit 1; }
mkdir -p "$out"
stage=$(mktemp -d "$out/.stage.XXXXXX")
trap 'rm -rf "$stage"' EXIT HUP INT TERM
cd "$root"
for platform in darwin-arm64 darwin-amd64 linux-arm64 linux-amd64; do
 dir="$stage/keyenv-$version-$platform"
 mkdir "$dir"
 CGO_ENABLED=0 GOOS=${platform%-*} GOARCH=${platform#*-} go build -trimpath -ldflags "-s -w -X main.version=$version" -o "$dir/keyenv" ./cmd/keyenv
 cp scripts/install-local.sh scripts/checksums.sh "$dir/"
 printf '%s\n' "$version" > "$dir/VERSION"
 (cd "$dir"; for file in keyenv install-local.sh checksums.sh VERSION; do printf '%s  %s\n' "$(keyenv_sha256 "$file")" "$file"; done > SHA256SUMS)
 tar -czf "$out/keyenv-$version-$platform.tar.gz" -C "$stage" "keyenv-$version-$platform"
done
# Personal bootstrap resources remain external to all four CLI binaries.
dir="$stage/keyenv-$version-personal-connect"
mkdir -p "$dir/oauth"
cp scripts/connect-existing.sh "$dir/"
cp config/oauth/client.public.json config/oauth/client.secret.enc.json "$dir/oauth/"
cp scripts/checksums.sh "$dir/"
printf '%s\n' "$version" > "$dir/VERSION"
(cd "$dir"; for file in connect-existing.sh checksums.sh VERSION oauth/client.public.json oauth/client.secret.enc.json; do printf '%s  %s\n' "$(keyenv_sha256 "$file")" "$file"; done > SHA256SUMS)
tar -czf "$out/keyenv-$version-personal-connect.tar.gz" -C "$stage" "keyenv-$version-personal-connect"
(cd "$out"; for file in keyenv-*.tar.gz; do printf '%s  %s\n' "$(keyenv_sha256 "$file")" "$file"; done > SHA256SUMS)
printf 'Prepared (not published): %s\n' "$out"
