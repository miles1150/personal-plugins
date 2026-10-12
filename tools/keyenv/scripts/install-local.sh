#!/bin/sh
# Explicit destination only; never modify shell startup files or replace files.
set -eu
if [ "$#" -ne 2 ]; then
 echo 'Usage: install-local.sh ABSOLUTE_PACKAGE_DIR ABSOLUTE_EXISTING_BIN_DIR' >&2
 exit 2
fi
case "$1:$2" in /*:/*) ;; *) echo 'Absolute paths required' >&2; exit 2;; esac
[ -d "$2" ] && [ ! -L "$2" ] || { echo 'Destination must be an existing directory' >&2; exit 1; }
[ -f "$1/keyenv" ] && [ ! -L "$1/keyenv" ] || exit 1
(cd "$1" && shasum -a 256 -c SHA256SUMS)
tmp=$(mktemp "$2/.keyenv-install.XXXXXX")
trap 'rm -f "$tmp"' EXIT HUP INT TERM
cp "$1/keyenv" "$tmp"
chmod 755 "$tmp"
# Hard-link creation is atomic and refuses an existing file or symlink.
ln "$tmp" "$2/keyenv"
printf 'Installed: %s/keyenv (PATH is unchanged)\n' "$2"
