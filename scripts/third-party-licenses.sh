#!/usr/bin/env bash
# Builds THIRD-PARTY-LICENSES.txt for a release: the licences of every Rust
# crate in the CLI binary (cargo-about, see about.toml / about.hbs), then
# realm-core's licence and third-party notices, since cargo can't see that
# C++ submodule.
#
# usage: scripts/third-party-licenses.sh [output]   (default THIRD-PARTY-LICENSES.txt)
# needs: cargo-about 0.9.2 (cargo install cargo-about --version 0.9.2 --locked --features cli)
#        and the vendor/realm-cpp submodule checked out.
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
out="${1:-THIRD-PARTY-LICENSES.txt}"
realm="$root/vendor/realm-cpp/realm-core"

for f in "$realm/LICENSE" "$realm/THIRD-PARTY-NOTICES"; do
    if [[ ! -f "$f" ]]; then
        echo "missing $f (git submodule update --init --recursive)" >&2
        exit 1
    fi
done

cargo about generate --locked --fail -m "$root/Cargo.toml" "$root/about.hbs" -o "$out"

{
    printf '\n================================================================================\n'
    printf 'realm-core (vendor/realm-cpp/realm-core)\n'
    printf 'Apache License 2.0\n'
    printf '================================================================================\n\n'
    cat "$realm/LICENSE"
    printf '\n================================================================================\n'
    printf 'realm-core third-party notices\n'
    printf '================================================================================\n\n'
    cat "$realm/THIRD-PARTY-NOTICES"
} >> "$out"

echo "wrote $out"
