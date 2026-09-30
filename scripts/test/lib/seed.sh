#!/usr/bin/env bash
# Create a fresh bkmr test database with a named dataset.
#
# Usage: seed.sh <lsp|hsearch> <db-path>
#   BKMR_BIN  binary to use (default: repo debug build)
#
# Datasets:
#   lsp      snippets exercising LSP language filtering; no embeddings (offline, fast)
#   hsearch  13 bookmarks in 5 groups for hybrid search; embeds (downloads model once)

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/../../.." && pwd)"
BKMR="${BKMR_BIN:-$REPO_ROOT/bkmr/target/debug/bkmr}"

usage() {
    echo "Usage: $(basename "$0") <lsp|hsearch> <db-path>" >&2
    exit 2
}

[ $# -eq 2 ] || usage
DATASET="$1"
DB="$2"

if [ ! -x "$BKMR" ]; then
    echo "Building debug binary..."
    cargo build --manifest-path "$REPO_ROOT/bkmr/Cargo.toml"
fi

mkdir -p "$(dirname "$DB")"
rm -f "$DB" "$DB-shm" "$DB-wal"
export BKMR_DB_URL="$DB"
"$BKMR" create-db "$DB" >/dev/null

add() {
    "$BKMR" add "$@" </dev/null >/dev/null
}

# ${1:...} are LSP snippet placeholders and must stay literal
# shellcheck disable=SC2016
seed_lsp() {
    # Expected completion per languageId is documented in TESTING.md (LSP section)
    add 'console.log(${1:msg})'                   js,_snip_            --title "JS alias log"       --no-embed --no-web
    add 'console.error(${1:err})'                 javascript,_snip_    --title "JS full-name error" --no-embed --no-web
    add 'println!("{}", ${1:v});'                 rust,_snip_          --title "Rust println"       --no-embed --no-web
    add '"rust-analyzer.check.command": "clippy"' rust-analyzer,_snip_ --title "RA clippy setting"  --no-embed --no-web
    add 'https://example.com'                     snip,rust            --title "Not a snippet"      --no-embed --no-web
    add 'echo "${1:hello}"'                       bash,_snip_          --title "Bash echo"          --no-embed --no-web
    add '// TODO: ${1:what}'                      universal,_snip_     --title "Universal TODO"     --no-embed --no-web

    # Completion session behaviour (TESTING.md "Completion sessions"):
    # every render runs the shell filter, which logs "Executing shell command: date …"
    # and shows a fresh timestamp in the completion preview.
    add '// rendered at {{ "date +%H:%M:%S" | shell }}' rust,_snip_ --title "Render counter" --no-embed --no-web
    # More python snippets than max_completions (50): the list is truncated and incomplete.
    local i
    for i in $(seq -w 1 55); do
        add "print(\"filler $i\")" python,_snip_ --title "Py filler $i" --no-embed --no-web
    done
}

seed_hsearch() {
    # Group A: Pure FTS matches (exact strings, no semantic similarity)
    add "https://docs.rs/error-code-8443" "error-code-8443,_snip_" \
        --title "Error Code 8443: TLS handshake failure on port 8443"
    add "https://github.com/rust-lang/rust/issues/12345" "rust,compiler,bugs" \
        --title "ICE: internal compiler error in rustc_mir_transform"
    add "https://stackoverflow.com/q/999999" "linux,networking,debug" \
        --title "iptables rule for blocking port 5432 on eth0"

    # Group B: Semantic matches (related concepts, different wording)
    add "https://kubernetes.io/docs/tasks/configure-pod-container/" "kubernetes,containers,_procedure_" \
        --title "Configuring liveness and readiness probes for pods"
    add "https://docs.docker.com/engine/security/" "docker,security,containers" \
        --title "Securing containerized applications with user namespaces"
    add "https://www.cncf.io/blog/service-mesh/" "networking,microservices" \
        --title "Understanding service mesh architecture patterns"

    # Group C: Both FTS and semantic matches (should rank highest via RRF)
    add "https://kubernetes.io/docs/concepts/security/" "kubernetes,security,_procedure_" \
        --title "Kubernetes cluster security best practices and health checks"
    add "https://helm.sh/docs/intro/" "kubernetes,helm,deployment" \
        --title "Kubernetes deployment with Helm charts and health check configuration"

    # Group D: Tagged entries for filter testing
    add "https://ansible.com/docs/" "ansible,automation,_procedure_" \
        --title "Ansible playbook for server provisioning and health monitoring"
    add "https://terraform.io/docs/" "terraform,iac,_procedure_" \
        --title "Terraform infrastructure as code for cloud deployment"
    add "https://python.org/docs/" "python,language,_snip_" \
        --title "Python standard library reference documentation"

    # Group E: Non-embeddable entries (should still appear in FTS)
    add "shell::echo 'kubernetes health check running'" "kubernetes,_shell_" \
        --title "Quick k8s health check shell command"
    add "SELECT * FROM pods WHERE status = 'Running'" "kubernetes,sql,_snip_" \
        --title "SQL query for kubernetes pod status"
}

case "$DATASET" in
    lsp) seed_lsp ;;
    hsearch) seed_hsearch ;;
    *) usage ;;
esac

echo "Seeded '$DATASET' dataset: $DB"
