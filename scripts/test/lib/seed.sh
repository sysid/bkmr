#!/usr/bin/env bash
# Create a fresh bkmr test database with a named dataset.
#
# Usage: seed.sh <lsp|hsearch> <db-path>
#   BKMR_BIN  binary to use (default: repo debug build)
#
# Datasets live in bkmr/tests/resources/datasets/<name>.json, shared with cargo tests
# (bkmr/tests/lsp). Each entry carries `group`/`why` fields documenting its purpose;
# the importer ignores them.
#   lsp      snippets exercising LSP language filtering; no embeddings (offline, fast)
#   hsearch  13 bookmarks in 5 groups for hybrid search; embeds (downloads model once)

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/../../.." && pwd)"
BKMR="${BKMR_BIN:-$REPO_ROOT/bkmr/target/debug/bkmr}"
DATASETS="$REPO_ROOT/bkmr/tests/resources/datasets"

usage() {
    echo "Usage: $(basename "$0") <lsp|hsearch> <db-path>" >&2
    exit 2
}

[ $# -eq 2 ] || usage
DATASET="$1"
DB="$2"

case "$DATASET" in
    lsp | hsearch) ;;
    *) usage ;;
esac

if [ ! -x "$BKMR" ]; then
    echo "Building debug binary..."
    cargo build --manifest-path "$REPO_ROOT/bkmr/Cargo.toml"
fi

mkdir -p "$(dirname "$DB")"
rm -f "$DB" "$DB-shm" "$DB-wal"
export BKMR_DB_URL="$DB"
"$BKMR" create-db "$DB" >/dev/null
if [ "$DATASET" = lsp ]; then
    "$BKMR" load-json --no-embed "$DATASETS/$DATASET.json" </dev/null >/dev/null
else
    "$BKMR" load-json "$DATASETS/$DATASET.json" </dev/null >/dev/null
fi

echo "Seeded '$DATASET' dataset: $DB"
