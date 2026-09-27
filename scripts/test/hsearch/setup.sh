#!/usr/bin/env bash
# Creates (or recreates) the hybrid search test database.
#   BKMR_BIN  binary to use (default: repo debug build)

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/../../.." && pwd)"
BKMR="${BKMR_BIN:-$REPO_ROOT/bkmr/target/debug/bkmr}"
TEST_DB="/tmp/bkmr_hsearch_test.db"

bash "$SCRIPT_DIR/../lib/seed.sh" hsearch "$TEST_DB"

echo ""
echo "--- Setup complete (bookmarks are embeddable by default) ---"
BKMR_DB_URL="$TEST_DB" "$BKMR" info
echo ""
echo "To use: export BKMR_DB_URL=$TEST_DB"
