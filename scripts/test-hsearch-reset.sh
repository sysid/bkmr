#!/bin/bash
# Re-runs hybrid search test setup from scratch.
# Run from the project root.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
rm -f /tmp/bkmr_hsearch_test.db
bash "$SCRIPT_DIR/test-hsearch-setup.sh"
