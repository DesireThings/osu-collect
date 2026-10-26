#!/usr/bin/env bash
# Prints the licence section for the shared libraries the Linux AppImage
# carries. linuxdeploy copies WebKitGTK, GTK, GLib and the libraries they
# use from the build machine's Ubuntu packages into the AppImage, so those
# packages' copyright files (many are LGPL) and a pointer to their source
# have to ship with it. The .deb doesn't carry them; it depends on them.
#
# For each package that a bundled library comes from: the library files,
# the package and source versions, where Ubuntu publishes the source, and
# the package's copyright file. Licence texts that copyright files only
# reference (/usr/share/common-licenses/...) are appended once at the end.
#
# usage: scripts/appimage-lib-notices.sh <osu-collect.AppDir>
# Run on the Ubuntu machine that built the AppImage (needs dpkg).
set -euo pipefail

appdir="${1:?usage: appimage-lib-notices.sh <AppDir>}"
[[ -d "$appdir" ]] || { echo "no AppDir at $appdir" >&2; exit 1; }

rule='================================================================================'
release="$(. /etc/os-release && echo "$PRETTY_NAME")"

declare -A files_of   # package -> bundled library names
while IFS= read -r lib; do
    # dpkg -S also lists the -dev symlink packages; keep the first package
    # that owns a path ending in exactly this file name.
    owner="$(dpkg -S "*/$lib" 2>/dev/null | awk -F': ' -v lib="$lib" '
        { n = split($2, p, "/"); if (p[n] == lib) { print $1; exit } }')"
    if [[ -z "$owner" ]]; then
        echo "no Ubuntu package owns $lib; add its notice by hand" >&2
        exit 1
    fi
    owner="${owner%%,*}"      # "a, b: /path" when two packages share a path
    pkg="${owner%%:*}"        # drop the :amd64 architecture suffix
    files_of[$pkg]+="${files_of[$pkg]:+, }$lib"
done < <(find "$appdir" -type f -name '*.so*' -printf '%f\n' | sort -u)

printf 'Libraries bundled in the Linux AppImage\n'
printf -- '---------------------------------------\n\n'
printf 'The AppImage carries these shared libraries so it runs on most Linux\n'
printf 'distributions. They are copied unchanged from %s packages.\n' "$release"
printf 'The source code of each package is published by Ubuntu at the address\n'
printf 'given with it (https://launchpad.net/ubuntu/+source/<source>/<version>).\n'

declare -A referenced
for pkg in $(printf '%s\n' "${!files_of[@]}" | sort); do
    version="$(dpkg-query -W -f='${Version}' "$pkg")"
    source="$(dpkg-query -W -f='${source:Package}' "$pkg")"
    source_version="$(dpkg-query -W -f='${source:Version}' "$pkg")"
    copyright="/usr/share/doc/$pkg/copyright"
    printf '\n%s\n%s %s (source package %s %s)\n' "$rule" "$pkg" "$version" "$source" "$source_version"
    printf 'Files: %s\n' "${files_of[$pkg]}"
    printf 'Source: https://launchpad.net/ubuntu/+source/%s/%s\n%s\n\n' "$source" "$source_version" "$rule"
    if [[ -f "$copyright" ]]; then
        cat "$copyright"
        while IFS= read -r ref; do
            referenced[$ref]=1
        done < <(grep -o '/usr/share/common-licenses/[A-Za-z0-9.+_-]*' "$copyright" | sort -u)
    else
        echo "no copyright file for $pkg at $copyright" >&2
        exit 1
    fi
done

for ref in $(printf '%s\n' "${!referenced[@]}" | sort); do
    [[ -f "$ref" ]] || continue
    printf '\n%s\n%s (referenced by the copyright files above)\n%s\n\n' "$rule" "$ref" "$rule"
    cat "$ref"
done
