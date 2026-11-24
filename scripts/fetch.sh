#!/usr/bin/env bash
# Downloads every file the live entries list (`registry list` reads only mods/; archived
# entries are never downloaded) into files/<sha256>: from its own release (<id>-<version>)
# once published, else from the `staging` prerelease. FILES_KEPT=<dir> names files a
# previous publish verified (<dir>/<sha256>), used instead of a download when they still
# match. Every file, kept or downloaded, is rehashed against its entry's sha256; one that
# doesn't match is fetched again, so nothing kept is trusted.
# FILES_FROM=<dir> reads local packages for a trial.
set -euo pipefail
checkout="${1:-.}"
mkdir -p files
matches() { printf '%s  %s\n' "$1" "$2" | sha256sum --check --status; }
cargo horizon-mod registry list "$checkout" | {
kept=0 downloaded=0
while read -r sha id version file; do
  [[ "$sha" =~ ^[0-9a-f]{64}$ ]] || exit 1
  [[ "$id" =~ ^[a-z0-9_]{1,64}$ ]] || exit 1
  [[ "$version" =~ ^[A-Za-z0-9][A-Za-z0-9._-]{0,63}$ ]] || exit 1
  [[ "$file" != .* && ${#file} -le 128 && "$file" =~ ^[A-Za-z0-9._+-]+$ ]] || exit 1
  if [ -f "files/$sha" ] && [ ! -L "files/$sha" ] && matches "$sha" "files/$sha"; then
    kept=$((kept + 1))
    continue
  fi
  tmp="$(mktemp -d)"
  trap 'rm -rf "$tmp"' EXIT
  if [ -n "${FILES_KEPT:-}" ] && [ -f "$FILES_KEPT/$sha" ] && [ ! -L "$FILES_KEPT/$sha" ]; then
    cp "$FILES_KEPT/$sha" "$tmp/package"
    if matches "$sha" "$tmp/package"; then
      mv "$tmp/package" "files/$sha"
      rm -rf "$tmp"
      trap - EXIT
      kept=$((kept + 1))
      continue
    fi
    rm -f "$tmp/package"
  fi
  ext="${file##*.}"
  if [ -n "${FILES_FROM:-}" ]; then
    cp "$FILES_FROM/$file" "$tmp/package"
  elif gh release download "$id-$version" --pattern "$file" --dir "$tmp" 2>/dev/null; then
    mv "$tmp/$file" "$tmp/package"
  elif gh release download staging --pattern "$sha.$ext" --dir "$tmp"; then
    mv "$tmp/$sha.$ext" "$tmp/package"
  else
    echo "::error::$id $version: reviewed package is absent from its release and staging"
    exit 1
  fi
  matches "$sha" "$tmp/package" || {
    echo "::error::$id $version: package SHA-256 mismatch"
    exit 1
  }
  mv "$tmp/package" "files/$sha"
  rm -rf "$tmp"
  trap - EXIT
  downloaded=$((downloaded + 1))
done
echo "files: $kept kept (verified), $downloaded downloaded"
}
