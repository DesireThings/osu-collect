#!/usr/bin/env bash
# Checks that macOS executables and static libraries were built for macOS
# <min> or older, so they run on every Mac the apps say they support. A
# library compiled for the build machine's own macOS links without an
# error, but it can call system functions an older Mac doesn't have, so
# the app would crash there. MACOSX_DEPLOYMENT_TARGET is what keeps rustc,
# cc and CMake (realm-core) on <min>; this catches anything that ignores it.
#
# usage: scripts/check-macos-min.sh <min> <file or directory>...
# A file must carry a minimum macOS version. A directory is searched for
# static libraries (*.a); archives without one are listed, not failed.
# Run on macOS (needs otool).
set -euo pipefail

min="${1:?usage: check-macos-min.sh <min> <file or directory>...}"
shift
[[ $# -gt 0 ]] || { echo "usage: check-macos-min.sh <min> <file or directory>..." >&2; exit 1; }

failed=0

# Every minimum macOS version in a Mach-O file or archive, one per line.
versions() {
  otool -l "$1" | awk '
    /cmd LC_BUILD_VERSION/      { build = 1 }
    build && $1 == "minos"      { print $2; build = 0 }
    /cmd LC_VERSION_MIN_MACOSX/ { old = 1 }
    old && $1 == "version"      { print $2; old = 0 }
  ' | sort -uV
}

# newer A B: true when version A is newer than version B (11.0 = 11).
newer() {
  local a="$1" b="$2"
  while [[ "$a" == *.0 ]]; do a="${a%.0}"; done
  while [[ "$b" == *.0 ]]; do b="${b%.0}"; done
  [[ "$a" != "$b" && "$(printf '%s\n%s\n' "$a" "$b" | sort -V | tail -n1)" == "$a" ]]
}

check() {
  local file="$1" required="$2" found v
  found="$(versions "$file")"
  if [[ -z "$found" ]]; then
    if [[ "$required" == yes ]]; then
      echo "::error::$file: no minimum macOS version in it"
      failed=1
    else
      echo "$file: no minimum macOS version (skipped)"
    fi
    return
  fi
  for v in $found; do
    if newer "$v" "$min"; then
      echo "::error::$file: built for macOS $v, newer than $min"
      failed=1
    fi
  done
  echo "$file: macOS $(echo $found)"
}

for path in "$@"; do
  if [[ -d "$path" ]]; then
    count=0
    while IFS= read -r -d '' lib; do
      check "$lib" no
      count=$((count + 1))
    done < <(find "$path" -name '*.a' -type f -print0)
    [[ $count -gt 0 ]] || { echo "::error::$path: no static libraries found"; failed=1; }
  elif [[ -f "$path" ]]; then
    check "$path" yes
  else
    echo "::error::$path: not found"
    failed=1
  fi
done

[[ $failed -eq 0 ]] && echo "All built for macOS $min or older."
exit $failed
