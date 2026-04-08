#!/bin/bash
# Creates a test database for hybrid search manual testing.
# Run from the project root.

set -euo pipefail

TEST_DB="/tmp/bkmr_hsearch_test.db"
BKMR="${BKMR_BIN:-./bkmr/target/debug/bkmr}"

if [ ! -f "$BKMR" ]; then
    echo "Building release binary..."
    cargo build --release --manifest-path bkmr/Cargo.toml
fi

rm -f "$TEST_DB"
export BKMR_DB_URL="$TEST_DB"

$BKMR create-db "$TEST_DB"

echo "--- Adding test bookmarks ---"

# Group A: Pure FTS matches (exact strings, no semantic similarity)
$BKMR add "https://docs.rs/error-code-8443" "error-code-8443,_snip_" \
    --title "Error Code 8443: TLS handshake failure on port 8443"
$BKMR add "https://github.com/rust-lang/rust/issues/12345" "rust,compiler,bugs" \
    --title "ICE: internal compiler error in rustc_mir_transform"
$BKMR add "https://stackoverflow.com/q/999999" "linux,networking,debug" \
    --title "iptables rule for blocking port 5432 on eth0"

# Group B: Semantic matches (related concepts, different wording)
$BKMR add "https://kubernetes.io/docs/tasks/configure-pod-container/" "kubernetes,containers,_procedure_" \
    --title "Configuring liveness and readiness probes for pods"
$BKMR add "https://docs.docker.com/engine/security/" "docker,security,containers" \
    --title "Securing containerized applications with user namespaces"
$BKMR add "https://www.cncf.io/blog/service-mesh/" "networking,microservices" \
    --title "Understanding service mesh architecture patterns"

# Group C: Both FTS and semantic matches (should rank highest via RRF)
$BKMR add "https://kubernetes.io/docs/concepts/security/" "kubernetes,security,_procedure_" \
    --title "Kubernetes cluster security best practices and health checks"
$BKMR add "https://helm.sh/docs/intro/" "kubernetes,helm,deployment" \
    --title "Kubernetes deployment with Helm charts and health check configuration"

# Group D: Tagged entries for filter testing
$BKMR add "https://ansible.com/docs/" "ansible,automation,_procedure_" \
    --title "Ansible playbook for server provisioning and health monitoring"
$BKMR add "https://terraform.io/docs/" "terraform,iac,_procedure_" \
    --title "Terraform infrastructure as code for cloud deployment"
$BKMR add "https://python.org/docs/" "python,language,_snip_" \
    --title "Python standard library reference documentation"

# Group E: Non-embeddable entries (should still appear in FTS)
$BKMR add "shell::echo 'kubernetes health check running'" "kubernetes,_shell_" \
    --title "Quick k8s health check shell command"
$BKMR add "SELECT * FROM pods WHERE status = 'Running'" "kubernetes,sql,_snip_" \
    --title "SQL query for kubernetes pod status"

echo ""
echo "--- Setup complete (bookmarks are embeddable by default) ---"
echo "Database: $TEST_DB"
echo ""
$BKMR info
echo ""
echo "To use: export BKMR_DB_URL=$TEST_DB"
