#!/usr/bin/env bash
# Builds THIRD-PARTY-LICENSES.txt for a release: osu-shelf's own MIT notice,
# the licences of every Rust crate built in (cargo-about, see about.toml /
# about.hbs), then realm-core's licence and third-party notices, since cargo
# can't see that C++ submodule.
#
# --desktop builds the desktop app's file instead: the crates of gui/src-tauri
# (the engine plus Tauri) and the JavaScript packages bundled into its window.
# The release bundles it with the app as THIRD-PARTY-LICENSES.txt.
#
# usage: scripts/third-party-licenses.sh [--desktop] [output]
#        (default output THIRD-PARTY-LICENSES.txt, or THIRD-PARTY-LICENSES-desktop.txt)
# needs: cargo-about 0.9.2 (cargo install cargo-about --version 0.9.2 --locked --features cli)
#        and the vendor/realm-cpp submodule checked out.
#        --desktop also needs Node and the GUI's packages (pnpm -C gui install --frozen-lockfile).
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
desktop=false
if [[ "${1:-}" == "--desktop" ]]; then
    desktop=true
    shift
fi
if $desktop; then
    out="${1:-THIRD-PARTY-LICENSES-desktop.txt}"
else
    out="${1:-THIRD-PARTY-LICENSES.txt}"
fi
realm="$root/vendor/realm-cpp/realm-core"

for f in "$realm/LICENSE" "$realm/THIRD-PARTY-NOTICES"; do
    if [[ ! -f "$f" ]]; then
        echo "missing $f (git submodule update --init --recursive)" >&2
        exit 1
    fi
done

rule='================================================================================'

# The header and osu-shelf's own licence. The MIT notice of osu-shelf
# itself has to travel with every copy too, not only the dependencies'.
{
    if $desktop; then
        printf 'THIRD-PARTY LICENCES FOR THE OSU-COLLECT DESKTOP APP\n'
        printf '=====================================================\n\n'
        printf 'This file lists the licences of everything built into the osu-shelf\n'
        printf 'desktop app: osu-shelf itself, the Rust crates, the JavaScript packages\n'
        printf 'in its window, and realm-core, the C++ database library it links to read\n'
        printf "osu!lazer's library.\n\n"
    else
        printf 'THIRD-PARTY LICENCES FOR OSU-COLLECT\n'
        printf '====================================\n\n'
        printf 'This file lists the licences of everything built into osu-shelf:\n'
        printf 'osu-shelf itself, the Rust crates, and realm-core, the C++ database\n'
        printf "library it links to read osu!lazer's library.\n\n"
    fi
    printf '%s\nosu-shelf: MIT OR Apache-2.0 (the MIT notice is below; LICENSE-APACHE\n' "$rule"
    printf 'is in the source repository, https://github.com/DesireThings/osu-shelf)\n%s\n\n' "$rule"
    cat "$root/LICENSE-MIT"
    printf '\n'
} > "$out"

if $desktop; then
    manifest="$root/gui/src-tauri/Cargo.toml"
else
    manifest="$root/Cargo.toml"
fi
scratch="$(mktemp -d)"
trap 'rm -rf "$scratch"' EXIT
cargo about generate --locked --fail -c "$root/about.toml" -m "$manifest" "$root/about.hbs" -o "$scratch/crates.txt"
cat "$scratch/crates.txt" >> "$out"

# Some crates carry notices for code they adapted from elsewhere in a separate
# file (crossbeam-channel's CC-BY-3.0 attribution, uds_windows' notices, ...).
# cargo-about only reads the crate's own licence, so append those files for
# every crate it listed. Registry crates only: our own path crates are covered
# above, and the repository root may hold an old copy of this very file.
printf '{{#each crates}}{{package.name}}\t{{package.version}}\t{{package.manifest_path}}\n{{/each}}' \
    > "$scratch/crates.hbs"
cargo about generate --locked -c "$root/about.toml" -m "$manifest" "$scratch/crates.hbs" -o "$scratch/crates.tsv"
notices_header=false
while IFS=$'\t' read -r name version manifest_path; do
    [[ "$manifest_path" == */registry/src/* ]] || continue
    dir="$(dirname "$manifest_path")"
    for f in "$dir"/*; do
        [[ -f "$f" ]] || continue
        case "$(basename "$f" | tr '[:lower:]' '[:upper:]')" in
            NOTICE | NOTICE.* | LICENSE-THIRD-PARTY* | THIRD_PARTY* | THIRD-PARTY* | THIRDPARTYNOTICES*) ;;
            *) continue ;;
        esac
        if ! $notices_header; then
            printf '\nNotices inside crates\n---------------------\n\n' >> "$out"
            printf 'Files some crates ship next to their licence, for code they adapted\n' >> "$out"
            printf 'from other projects.\n' >> "$out"
            notices_header=true
        fi
        printf '\n%s\n%s %s: %s\n%s\n\n' "$rule" "$name" "$version" "$(basename "$f")" "$rule" >> "$out"
        cat "$f" >> "$out"
    done
done < "$scratch/crates.tsv"

if $desktop; then
    # Build the frontend once more with source maps, into a scratch folder,
    # so the listing names exactly the packages that ended up in the bundle.
    maps="$scratch/maps"
    pnpm -C "$root/gui" exec vite build --sourcemap --outDir "$maps" --emptyOutDir --logLevel warn
    printf '\n' >> "$out"
    node "$root/scripts/js-bundle-licenses.mjs" "$root/gui" "$maps" >> "$out"

    # On Windows, webview2-com-sys links Microsoft's WebView2LoaderStatic.lib
    # (from the Microsoft.Web.WebView2 SDK) into the app; its BSD-style
    # licence asks for the notice with binary copies.
    {
        printf '\n%s\n' "$rule"
        printf 'Microsoft WebView2 loader (Windows build only; WebView2LoaderStatic.lib from\n'
        printf 'the Microsoft.Web.WebView2 SDK, linked in by webview2-com-sys)\n'
        printf '%s\n\n' "$rule"
        cat "$root/scripts/notices/webview2-loader.txt"
    } >> "$out"
fi

{
    printf '\n%s\n' "$rule"
    printf 'realm-core (vendor/realm-cpp/realm-core)\n'
    printf 'Apache License 2.0\n'
    printf '%s\n\n' "$rule"
    cat "$realm/LICENSE"
    printf '\n%s\n' "$rule"
    printf 'realm-core third-party notices\n'
    printf '%s\n\n' "$rule"
    cat "$realm/THIRD-PARTY-NOTICES"
    # realm-core downloads zlib and links it statically on Windows only (see
    # build.rs); elsewhere the system's zlib is used, or none at all.
    printf '\n%s\n' "$rule"
    printf 'zlib (Windows build only; downloaded and linked in by realm-core)\n'
    printf '%s\n\n' "$rule"
    cat "$root/scripts/notices/zlib.txt"
} >> "$out"

echo "wrote $out"
