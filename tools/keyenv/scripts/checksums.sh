#!/bin/sh
# Shared portable SHA256. Never execute filenames from a manifest.
keyenv_sha256() {
 if command -v sha256sum >/dev/null 2>&1; then sha256sum "$1" | awk '{print $1}'
 elif command -v shasum >/dev/null 2>&1; then shasum -a 256 "$1" | awk '{print $1}'
 else echo 'SHA256 requires sha256sum or shasum' >&2; return 1
 fi
}
keyenv_verify() {
 manifest=$1
 shift
 [ -f "$manifest" ] && [ ! -L "$manifest" ] || return 1
 [ "$(wc -l < "$manifest" | tr -d ' ')" -eq "$#" ] || return 1
 for file do
  [ -f "$file" ] && [ ! -L "$file" ] || return 1
  expected=$(awk -v name="$file" '$2 == name && length($1) == 64 && $1 !~ /[^0-9a-f]/ {print $1}' "$manifest")
  [ "$expected" = "$(keyenv_sha256 "$file")" ] || { echo "Checksum failed: $file" >&2; return 1; }
 done
}
