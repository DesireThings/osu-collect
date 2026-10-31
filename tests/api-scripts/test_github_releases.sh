#!/usr/bin/env bash
# tests: GitHub releases API - GET /repos/DesireThings/osu-shelf/releases/latest
# rust source: src/auto_update.rs, src/config/constants.rs (RELEASES_URL)
# model: ReleaseResponse { name, tag_name, assets: [{ name, browser_download_url }] }

set -euo pipefail
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/common.sh"

URL="https://api.github.com/repos/DesireThings/osu-shelf/releases/latest"

echo "=== GitHub releases API: GET $URL ==="

resp=$(curl -s -w "\n%{http_code}" \
    -H "User-Agent: osu-shelf/test" \
    -H "Accept: application/vnd.github+json" \
    "$URL")
status=$(echo "$resp" | tail -1)
body=$(echo "$resp" | head -n -1)

if [[ "$status" == "404" ]]; then
    echo "INFO: no releases yet (404) - this is expected for new repos"
    pass "GitHub releases endpoint reachable (404 = no releases)"
    echo ""
    echo "result: PASS (no releases published)"
    exit 0
fi

check_http_status "$status" 200 "GitHub releases fetch" || { echo "$body" | head -5; exit 1; }

check_field "$body" "name" "string" "release.name"
check_field "$body" "tag_name" "string" "release.tag_name"
check_field "$body" "assets" "array" "release.assets"

tag=$(echo "$body" | jq -r '.tag_name')
name=$(echo "$body" | jq -r '.name')
pass "release: tag=$tag name=$name"

asset_count=$(echo "$body" | jq '.assets | length')
pass "assets count: $asset_count"

if [[ "$asset_count" -gt 0 ]]; then
    first_asset=$(echo "$body" | jq '.assets[0]')
    check_field "$first_asset" "name" "string" "asset[0].name"
    check_field "$first_asset" "browser_download_url" "string" "asset[0].browser_download_url"
    pass "asset shape ok: name=$(echo "$first_asset" | jq -r .name)"
fi

echo ""
if [[ $FAILURES -eq 0 ]]; then
    echo "result: PASS"
else
    echo "result: FAIL ($FAILURES failure(s))"
    exit 1
fi
