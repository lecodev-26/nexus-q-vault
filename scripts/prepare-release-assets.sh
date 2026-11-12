#!/usr/bin/env bash
set -euo pipefail

ASSET_DIR="${1:-release-assets}"
EXPECTED_VERSION="0.2.0"
EXPECTED_COMMIT="${2:-$(git rev-parse HEAD 2>/dev/null || true)}"
OUTPUT_DIR="${ASSET_DIR}-flat"

if [[ ! -d "$ASSET_DIR" ]]; then
  echo "ERROR: asset directory not found: $ASSET_DIR" >&2
  exit 1
fi

# GitHub artifact downloads preserve each artifact's relative directory tree.
# Collect the expected deliverables recursively and stage them at one level.
rm -rf "$OUTPUT_DIR"
mkdir -p "$OUTPUT_DIR"
shopt -s nullglob
copy_matches() {
  local pattern="$1" label="$2"
  local -a files=()
  while IFS= read -r -d '' file; do
    files+=("$file")
  done < <(find "$ASSET_DIR" -type f -name "$pattern" -print0)
  if (( ${#files[@]} == 0 )); then
    echo "ERROR: missing $label ($pattern)" >&2
    exit 1
  fi
  for file in "${files[@]}"; do
    local asset_name
    asset_name="$(basename "$file" | tr ' ' '.')"
    local target="$OUTPUT_DIR/$asset_name"
    if [[ -e "$target" ]]; then
      echo "ERROR: duplicate asset filename: $(basename "$file")" >&2
      exit 1
    fi
    cp "$file" "$target"
  done
  printf '%s: %s file(s)\n' "$label" "${#files[@]}"
}

copy_matches '*.deb' 'Linux Debian installer'
copy_matches '*.AppImage' 'Linux AppImage installer'
copy_matches '*.msi' 'Windows MSI installer'
copy_matches '*.exe' 'Windows NSIS installer'
copy_matches '*.dmg' 'macOS disk image'

for platform in windows macos linux; do
  info="BUILD-INFO-$platform.txt"
  mapfile -t matches < <(find "$ASSET_DIR" -type f -name "$info" -print)
  if (( ${#matches[@]} != 1 )); then
    echo "ERROR: expected exactly one $info, found ${#matches[@]}" >&2
    exit 1
  fi
  cp "${matches[0]}" "$OUTPUT_DIR/$info"
  grep -qx "version=$EXPECTED_VERSION" "$OUTPUT_DIR/$info" || { echo "ERROR: $info has the wrong version" >&2; exit 1; }
  grep -Eq '^commit=[0-9a-f]{40}$' "$OUTPUT_DIR/$info" || { echo "ERROR: $info has an invalid commit hash" >&2; exit 1; }
  grep -qx "platform=$platform" "$OUTPUT_DIR/$info" || { echo "ERROR: $info has the wrong platform" >&2; exit 1; }
done

commit="$(sed -n 's/^commit=//p' "$OUTPUT_DIR/BUILD-INFO-windows.txt")"
for platform in macos linux; do
  other="$(sed -n 's/^commit=//p' "$OUTPUT_DIR/BUILD-INFO-$platform.txt")"
  [[ "$other" == "$commit" ]] || { echo "ERROR: $platform build commit differs from Windows" >&2; exit 1; }
done
if [[ -n "$EXPECTED_COMMIT" && "$commit" != "$EXPECTED_COMMIT" ]]; then
  echo "ERROR: build metadata commit does not match the current Vault checkout ($EXPECTED_COMMIT)" >&2
  exit 1
fi

(
  cd "$OUTPUT_DIR"
  find . -maxdepth 1 -type f ! -name SHA256SUMS.txt -print0 | sort -z | xargs -0 -r sha256sum > SHA256SUMS.txt
)
echo "Verified v$EXPECTED_VERSION artifacts from Vault commit $commit"
echo "Prepared flat release assets in $OUTPUT_DIR"
