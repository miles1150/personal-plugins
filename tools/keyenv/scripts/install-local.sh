#!/bin/sh
# Non-root, explicit destination; preserve shell configuration and unrelated files.
set -eu
[ "$#" -ge 2 ] && [ "$#" -le 3 ] || { echo 'Usage: install-local.sh ABS_PACKAGE_DIR ABS_EXISTING_BIN_DIR [--replace]' >&2; exit 2; }
case "$1" in /*) ;; *) exit 2;; esac
case "$2" in /*) ;; *) exit 2;; esac
mode=${3:-}
case "$mode" in ''|--replace) ;; *) exit 2;; esac
[ -d "$2" ] && [ ! -L "$2" ] || exit 1
script_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
. "$script_dir/checksums.sh"
# Verify the fixed allowlist; a manifest cannot select arbitrary filesystem paths.
(cd "$1" && keyenv_verify SHA256SUMS keyenv install-local.sh checksums.sh VERSION)
target="$2/keyenv"
[ ! -L "$target" ] || { echo 'Refusing symlink destination' >&2; exit 1; }
if [ -e "$target" ]; then
 [ -f "$target" ] || exit 1
 if cmp -s "$1/keyenv" "$target"; then echo "Already installed: $target"; exit 0; fi
 [ "$mode" = --replace ] || { echo 'Different installation exists; use --replace explicitly' >&2; exit 1; }
fi
tmp=$(mktemp "$2/.keyenv-install.XXXXXX")
trap 'rm -f "$tmp"' EXIT HUP INT TERM
cp "$1/keyenv" "$tmp"
chmod 755 "$tmp"
if [ "$mode" = --replace ]; then mv -f "$tmp" "$target"; else ln "$tmp" "$target"; fi
printf 'Installed: %s (PATH and profiles unchanged)\n' "$target"
