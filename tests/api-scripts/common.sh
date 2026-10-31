#!/usr/bin/env bash
# shared helpers for api test scripts

BEATMAPSET_ID=705655
BEATMAP_ID=1510355
UA="osu-shelf-api-test/0.1"

pass() { echo "PASS: $1"; }
fail() { echo "FAIL: $1"; FAILURES=$((FAILURES + 1)); }

check_field() {
    local json="$1" field="$2" expected_type="$3" label="$4"
    local val
    val=$(echo "$json" | jq -r ".$field // \"__missing__\"" 2>/dev/null)
    if [[ "$val" == "__missing__" || "$val" == "null" ]]; then
        fail "$label: field '$field' missing or null"
        return 1
    fi
    case "$expected_type" in
        number)
            if ! echo "$val" | grep -qE '^[0-9]+$'; then
                fail "$label: field '$field' not a number (got: $val)"
                return 1
            fi
            ;;
        string)
            if [[ -z "$val" ]]; then
                fail "$label: field '$field' is empty string"
                return 1
            fi
            ;;
        array)
            local len
            len=$(echo "$json" | jq ".$field | length" 2>/dev/null)
            if [[ "$len" == "null" || -z "$len" ]]; then
                fail "$label: field '$field' not an array"
                return 1
            fi
            ;;
    esac
    return 0
}

check_http_status() {
    local status="$1" expected="$2" label="$3"
    if [[ "$status" != "$expected" ]]; then
        fail "$label: expected HTTP $expected, got $status"
        return 1
    fi
    return 0
}

FAILURES=0
