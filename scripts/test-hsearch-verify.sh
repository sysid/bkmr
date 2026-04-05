#!/bin/bash
# Quick automated verification of hsearch functionality.
# Run after test-hsearch-setup.sh, from the project root.

set -euo pipefail
export BKMR_DB_URL="/tmp/bkmr_hsearch_test.db"
BKMR="${BKMR_BIN:-./bkmr/target/debug/bkmr}"
PASS=0
FAIL=0

if [ ! -f "$BKMR_DB_URL" ]; then
    echo "ERROR: Test database not found. Run scripts/test-hsearch-setup.sh first."
    exit 1
fi

check() {
    local name="$1"
    shift
    if eval "$@" 2>/dev/null; then
        echo "  PASS: $name"
        ((PASS++)) || true
    else
        echo "  FAIL: $name"
        ((FAIL++)) || true
    fi
}

echo "=== hsearch verification ==="

# Basic search returns results
check "basic search" \
    "$BKMR hsearch 'kubernetes' --json --np 2>/dev/null | python3 -c 'import json,sys; r=json.load(sys.stdin); assert len(r)>0'"

# JSON has rrf_score
check "json rrf_score" \
    "$BKMR hsearch 'kubernetes' --json --np 2>/dev/null | python3 -c 'import json,sys; r=json.load(sys.stdin); assert all(\"rrf_score\" in x for x in r)'"

# JSON sorted descending by score
check "json sorted" \
    "$BKMR hsearch 'kubernetes' --json --np 2>/dev/null | python3 -c 'import json,sys; r=json.load(sys.stdin); s=[x[\"rrf_score\"] for x in r]; assert s==sorted(s,reverse=True)'"

# Tag filter works
check "tag filter" \
    "$BKMR hsearch 'kubernetes' --tags _procedure_ --json --np 2>/dev/null | python3 -c 'import json,sys; r=json.load(sys.stdin); assert all(\"_procedure_\" in x[\"tags\"] for x in r)'"

# Empty tag filter returns nothing
check "empty tag filter" \
    "$BKMR hsearch 'kubernetes' --tags nonexistent --np 2>&1 >/dev/null | grep -q 'No bookmarks found'"

# Exact mode works
check "exact mode" \
    "$BKMR hsearch 'iptables' --mode exact --json --np 2>/dev/null | python3 -c 'import json,sys; r=json.load(sys.stdin); assert len(r)>0'"

# Limit works
check "limit" \
    "$BKMR hsearch 'kubernetes' --limit 2 --json --np 2>/dev/null | python3 -c 'import json,sys; r=json.load(sys.stdin); assert len(r)<=2'"

# Piped output is tab-delimited
check "piped output" \
    "$BKMR hsearch 'kubernetes' --np 2>/dev/null | head -1 | python3 -c 'import sys; assert \"\\t\" in sys.stdin.read()'"

# search command still works (regression)
check "search regression" \
    "$BKMR search kubernetes --np 2>&1 >/dev/null | grep -q 'bookmarks'"

echo ""
echo "=== Results: $PASS passed, $FAIL failed ==="
[ "$FAIL" -eq 0 ] || exit 1
