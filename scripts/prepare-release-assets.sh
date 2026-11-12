#!/usr/bin/env bash
set -euo pipefail

ASSET_DIR="${1:-release-assets}"
EXPECTED_VERSION="0.2.0"
EXPECTED_COMMIT="${2:-$(git rev-parse HEAD 2>/dev/null || true)}"

if [[ ! -d "$ASSET_DIR" ]]; then
  echo "ERROR: asset directory not found: $ASSET_DIR" >&2
  exit 1
fi

shopt -s nullglob
require_one_or_more() {
  local pattern="$1" label="$2"
  local files=("$ASSET_DIR"/$pattern)
  if (( ${#files[@]} == 0 )); then
    echo "ERROR: missing $label ($pattern)" >&2
    exit 1
  fi
  printf '%s: %s file(s)\n' "$label" "${#files[@]}"
}

require_one_or_more '*.deb' 'Linux Debian installer'
require_one_or_more '*.AppImage' 'Linux AppImage installer'
require_one_or_more '*.msi' 'Windows MSI installer'
require_one_or_more '*.exe' 'Windows NSIS installer'
require_one_or_more '*.dmg' 'macOS disk image'

for platform in windows macos linux; do
  info="$ASSET_DIR/BUILD-INFO-$platform.txt"
  if [[ ! -s "$info" ]]; then
    echo "ERROR: missing build metadata: $info" >&2
    exit 1
  fi
  grep -qx "version=$EXPECTED_VERSION" "$info" || { echo "ERROR: $info has the wrong version" >&2; exit 1; }
  grep -Eq '^commit=[0-9a-f]{40}$' "$info" || { echo "ERROR: $info has an invalid commit hash" >&2; exit 1; }
  grep -qx "platform=$platform" "$info" || { echo "ERROR: $info has the wrong platform" >&2; exit 1; }
done

commit="$(sed -n 's/^commit=//p' "$ASSET_DIR/BUILD-INFO-windows.txt")"
for platform in macos linux; do
  other="$(sed -n 's/^commit=//p' "$ASSET_DIR/BUILD-INFO-$platform.txt")"
  [[ "$other" == "$commit" ]] || { echo "ERROR: $platform build commit differs from Windows" >&2; exit 1; }
done
if [[ -n "$EXPECTED_COMMIT" && "$commit" != "$EXPECTED_COMMIT" ]]; then
  echo "ERROR: build metadata commit does not match the current Vault checkout ($EXPECTED_COMMIT)" >&2
  exit 1
fi

cd "$ASSET_DIR"
find . -maxdepth 1 -type f ! -name SHA256SUMS.txt -print0 | sort -z | xargs -0 -r sha256sum > SHA256SUMS.txt
echo "Verified v$EXPECTED_VERSION artifacts from Vault commit $commit"
echo "Wrote $ASSET_DIR/SHA256SUMS.txt"
