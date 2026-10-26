#!/usr/bin/env bash
# tests: official osu! api v2 (osu.ppy.sh) — the "osu! official" mirror (MirrorKind::OsuApi)
# rust source: src/auth/mod.rs (token refresh + client credentials), osu-downloader/src/mirrors/mod.rs (OsuApi template)
#
# what this validates:
#   1. OAuth token endpoint shape          POST https://osu.ppy.sh/oauth/token  -> { access_token, expires_in, token_type }
#   2. bearer auth works for metadata       GET  /api/v2/beatmapsets/{id}        -> 200
#   3. the download endpoint's lazer gate   GET  /api/v2/beatmapsets/{id}/download with a client_credentials token -> 401/403
#
# (3) is why osu! official is a last-resort, default-off mirror: a headless
# client_credentials token authenticates but CANNOT download .osz. Only
# osu!lazer's own client gets a token that can, and this app doesn't use it.
#
# (1)-(3) require OSU_CLIENT_ID + OSU_CLIENT_SECRET (a public-data oauth app)
# and SKIP without them — no creds are ever committed.

set -euo pipefail
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/common.sh"

TOKEN_URL="https://osu.ppy.sh/oauth/token"
API_BASE="https://osu.ppy.sh/api/v2"

echo "=== osu! official api v2 (beatmapset $BEATMAPSET_ID) ==="
echo ""

have_app_creds=1
if [[ -z "${OSU_CLIENT_ID:-}" || -z "${OSU_CLIENT_SECRET:-}" ]]; then
    have_app_creds=0
    echo "SKIP: OSU_CLIENT_ID / OSU_CLIENT_SECRET not set (skipping client_credentials probes 1-3)"
    echo "      (create a public oauth app at https://osu.ppy.sh/home/account/edit#new-oauth-application)"
    echo ""
fi

if [[ "$have_app_creds" == "1" ]]; then
# ── 1. client_credentials token (the fallback grant in src/auth::ensure_valid) ──
echo "--- token endpoint (client_credentials, scope=public) ---"
token_resp=$(curl -s \
    -H "Content-Type: application/json" \
    -H "User-Agent: $UA" \
    --max-time 20 \
    -d "{\"client_id\":\"$OSU_CLIENT_ID\",\"client_secret\":\"$OSU_CLIENT_SECRET\",\"grant_type\":\"client_credentials\",\"scope\":\"public\"}" \
    "$TOKEN_URL" 2>/dev/null) || true

check_field "$token_resp" "access_token" string "token endpoint"
check_field "$token_resp" "expires_in" number "token endpoint"
check_field "$token_resp" "token_type" string "token endpoint"

access_token=$(echo "$token_resp" | jq -r '.access_token // empty' 2>/dev/null)
if [[ -z "$access_token" ]]; then
    fail "could not obtain access token; skipping authenticated probes"
    echo ""
    echo "result: FAIL ($FAILURES failure(s))"
    exit 1
fi
pass "obtained client_credentials access token"

# ── 2. metadata lookup with the bearer token ──
echo ""
echo "--- beatmapset metadata (bearer auth) ---"
meta_status=$(curl -s -o /dev/null -w "%{http_code}" \
    -H "Authorization: Bearer $access_token" \
    -H "User-Agent: $UA" \
    --max-time 20 \
    "$API_BASE/beatmapsets/$BEATMAPSET_ID" 2>/dev/null) || true

if [[ "$meta_status" == "200" ]]; then
    pass "metadata lookup ok (HTTP 200)"
elif [[ "$meta_status" == "429" ]]; then
    echo "  SKIP: metadata: rate limited (HTTP 429)"
else
    fail "metadata lookup: expected HTTP 200, got $meta_status"
fi

# ── 3. download endpoint — the lazer gate ──
# A client_credentials token (no user behind it) must NOT be able to download.
# Expect 401 (unauthenticated for this route) or 403 (forbidden / lazer-only).
echo ""
echo "--- download endpoint (expected to be gated for client_credentials) ---"
dl_status=$(curl -s -o /dev/null -w "%{http_code}" \
    -H "Authorization: Bearer $access_token" \
    -H "User-Agent: $UA" \
    --max-time 20 \
    "$API_BASE/beatmapsets/$BEATMAPSET_ID/download" 2>/dev/null) || true

case "$dl_status" in
    401 | 403)
        pass "download correctly gated for client_credentials (HTTP $dl_status)"
        ;;
    200 | 302)
        # Unexpected: this token grant should never be able to download. If osu!
        # ever allows it, the OsuApi mirror's auth assumptions need revisiting.
        fail "download unexpectedly allowed for client_credentials (HTTP $dl_status) — revisit the lazer-scope assumption"
        ;;
    429)
        echo "  SKIP: download: rate limited (HTTP 429)"
        ;;
    *)
        fail "download: unexpected status (HTTP $dl_status)"
        ;;
esac
fi # have_app_creds

echo ""
if [[ $FAILURES -eq 0 ]]; then
    echo "result: PASS"
else
    echo "result: FAIL ($FAILURES failure(s))"
    exit 1
fi
