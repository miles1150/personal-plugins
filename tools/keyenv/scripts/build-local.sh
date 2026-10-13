#!/bin/sh
# Build only for this host; do not publish or install globally.
set -eu
root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
case "$(uname -s)-$(uname -m)" in
 Darwin-arm64) platform=darwin-arm64;;
 Darwin-x86_64) platform=darwin-amd64;;
 Linux-aarch64) platform=linux-arm64;;
 Linux-x86_64) platform=linux-amd64;;
 *) echo 'Unsupported host' >&2; exit 1;;
esac
out="$root/dist/$platform"
mkdir -p "$out"
cd "$root"
CGO_ENABLED=0 go build -trimpath -o "$out/keyenv" ./cmd/keyenv
cp scripts/install-local.sh scripts/checksums.sh "$out/"
printf 'dev\n' > "$out/VERSION"
. "$root/scripts/checksums.sh"
cd "$out"
for file in keyenv install-local.sh checksums.sh VERSION; do printf '%s  %s\n' "$(keyenv_sha256 "$file")" "$file"; done > SHA256SUMS
printf 'Package: %s\n' "$out"
