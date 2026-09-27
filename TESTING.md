# Testing bkmr

Single source for everything test-related: automated suites, scenario suites and manual
procedures. Other documents link here instead of repeating commands.

```
 L1 AUTOMATED               L2 SCENARIO                  L3 MANUAL
 make test                  make test-scenarios          make test-env + this document
 fast, hermetic, CI         slow, needs embedding model  human judgement
 ─────────────────────      ────────────────────────     ─────────────────────────
 unit tests (src/**)        hsearch: 9 pass/fail checks  editors: nvim, IntelliJ
 integration (tests/**)                                  hsearch: TC-01 … TC-14
 LSP over stdio                                          system-ort build
   (tests/lsp)                                           Homebrew formula

 shared fixtures: scripts/test/lib/seed.sh (datasets), scripts/test/lib/bkmr-dev (wrapper)
```

| Command | Level | What it runs |
|---|---|---|
| `make test` | L1 | all Rust unit + integration tests, single-threaded; runs in CI |
| `make test-scenarios` | L2 | builds, seeds `/tmp/bkmr_hsearch_test.db`, runs `scripts/test/hsearch/verify.sh` |
| `make test-all` | L1 + L2 | both |
| `make test-env` | L3 | builds, seeds `/tmp/bkmr-dev`, prints editor launch lines |

## L1 — Automated tests

```bash
make test

# Single test with output (from repo root)
env RUST_LOG=error BKMR_DB_URL=../db/bkmr.db \
  cargo test --manifest-path bkmr/Cargo.toml <name> -- --test-threads=1 --nocapture
```

**Always single-threaded, always with `BKMR_DB_URL`.** Tests share one SQLite file
(`db/bkmr.db`, recreated by `make test`) and mutate environment variables; parallel runs
fail with lock errors and races. `RUST_LOG` works under `cargo test` (the test harness reads
it); the `bkmr` binary itself ignores it (see [Debug logging](#debug-logging)).

| Location | Kind | Notes |
|---|---|---|
| `bkmr/src/**` `#[cfg(test)]` | unit | services via `TestServiceContainer` (`src/util/test_service_container.rs`) or `TestContext` |
| `bkmr/tests/{cli,application,infrastructure}` | integration | CLI tests spawn the binary with `assert_cmd` |
| `bkmr/tests/lsp` | LSP black-box | spawns `bkmr lsp` over stdio against a temp DB; checks framing, stdout purity, completion per languageId, commands |
| `bkmr/src/lsp/tests` | LSP in-process | backend and services without a process |

Conventions:

- Names: `given_X_when_Y_then_Z()`; structure Arrange / Act / Assert.
- Tests read like specifications (DAMP over DRY).
- Never `ServiceContainer::new()` in tests — use `TestServiceContainer::new()`.
- Isolate environment changes with `EnvGuard`.
- Tests needing their own data create it; black-box tests use a `TempDir` database seeded
  with `bkmr add --no-embed --no-web` (offline, no model download).

### Ignored tests

Excluded from `make test` because they open a browser, print for visual inspection, need the
network, or wait for an editor. Run one explicitly:

```bash
env BKMR_DB_URL=../db/bkmr.db cargo test --manifest-path bkmr/Cargo.toml <name> -- --ignored --nocapture --test-threads=1
```

| Test | Why ignored |
|---|---|
| `given_valid_url_when_load_details_then_returns_title` (`infrastructure/http.rs`) | network |
| `given_bookmark_when_edit_with_template_then_returns_modified_bookmark` (`template_service.rs`) | manual editor interaction |
| `given_markdown_*_when_execute_then_renders_*` (3×, `markdown_action.rs`) | opens a browser |
| `given_bookmarks_when_write_as_json_then_creates_valid_file` (`infrastructure/json.rs`) | visual stdout check |
| `given_path_when_creating_database_then_creates_successfully` (`tests/test_main.rs`) | not implemented |

### CI

`.github/workflows/test.yml` runs `make test` on Ubuntu and macOS for pushes to `main` and
for pull requests. Scenario suites are not run in CI (embedding model download).

## Fixtures

| Script | Purpose |
|---|---|
| `scripts/test/lib/seed.sh <lsp\|hsearch> <db>` | fresh database with a named dataset; `BKMR_BIN` selects the binary |
| `scripts/test/lib/bkmr-dev` | runs bkmr with `-dd` against `$BKMR_DEV_DIR/test.db` (default `/tmp/bkmr-dev`), stderr → `lsp.log`; `BKMR_BIN` selects the binary |
| `scripts/test/lsp_probe.py` | ad-hoc LSP client (uv script): `capabilities`, `complete <lang…> [--prefix]`, `list [lang]`, `get <id>` |

Datasets:

- **`lsp`** — 7 bookmarks exercising language filtering (aliases, exact tag match,
  non-snippets, universal); no embeddings.
- **`hsearch`** — 13 bookmarks in 5 groups for hybrid search; embeds (model download on
  first use). See [hsearch dataset](#hsearch-dataset).

All fixtures default to the repo debug build; set `BKMR_BIN=~/bin/bkmr` (or any other binary)
to compare against a released version.

## L2 — Scenario suites

### hsearch

```bash
make test-scenarios                        # setup + 9 automated checks
bash scripts/test/hsearch/cleanup.sh       # remove test databases
```

`setup.sh` (re)creates `/tmp/bkmr_hsearch_test.db`; `verify.sh` checks result presence,
`rrf_score` in JSON, ordering, tag filter, empty filter, exact mode, limit, piped output and
`search` regression. For the manual test cases see [hsearch TC catalogue](#hsearch-tc-catalogue).

## L3 — Manual procedures

### Editor integration (LSP)

```bash
make test-env        # build, seed /tmp/bkmr-dev (dataset: lsp), print launch lines
```

`/tmp/bkmr-dev/bin/bkmr` is a symlink to the `bkmr-dev` wrapper: editors started with it use
the local build and the throwaway database, and every server message lands in
`/tmp/bkmr-dev/lsp.log`. Your real database is never touched.

**Expected completion** (empty line, manual invoke):

| Buffer | Expected items | Must NOT appear | Checks |
|---|---|---|---|
| `t.rs` | Rust println, Universal TODO | Not a snippet (tag `snip`, no `_snip_`), RA clippy setting (tag `rust-analyzer`) | exact tag matching |
| `t.js` | JS alias log (`js`), JS full-name error (`javascript`), Universal TODO | – | language aliases |
| `t.sh` | Bash echo (`bash`), Universal TODO | – | shell aliases |
| `listSnippets javascript` | JS alias log, JS full-name error | Universal TODO | shared alias table |

`scripts/test/lsp_probe.py --bin /tmp/bkmr-dev/bin/bkmr complete rust javascript shellscript`
prints the same table without an editor.

**Before/after comparison:** run the same steps with `BKMR_BIN=~/bin/bkmr` (nvim) or Binary
Path pointing to the released binary (IntelliJ).

#### Neovim (bkmr-nvim)

1. `cd /tmp/bkmr-dev/proj && PATH=/tmp/bkmr-dev/bin:$PATH nvim t.rs`
   — bkmr-nvim's default `lsp.cmd = {"bkmr","lsp"}` resolves to the wrapper. If your
   `setup{}` sets an absolute `lsp.cmd`, temporarily use `{ "/tmp/bkmr-dev/bin/bkmr", "lsp" }`.
2. `:checkhealth vim.lsp` (or `:LspInfo`) → `bkmr_lsp` attached to the buffer.
3. Insert mode on an empty line: `<C-Space>` (nvim-cmp / blink.cmp) or `<C-x><C-o>` →
   compare with the table. Repeat with `:e t.js` and `:e t.sh`.
4. `:BkmrEdit javascript` → both JS snippets listed.
5. `grep -E "Document opened|Returning .* completion items|ERROR" /tmp/bkmr-dev/lsp.log`
   — `Document opened: … (language: X)` shows the languageId nvim sent.
6. Snippet expansion: accept an item, type into `${1}`, `<Tab>` to the next stop
   (requires snippet engine and select-mode mappings, see wiki Editor-Integration).

A filetype missing from bkmr-nvim's `lsp.filetypes` means the server never attaches; use
`lsp.extra_filetypes` to add one.

#### IntelliJ (bkmr-intellij-plugin)

Use the plugin's sandbox IDE; it keeps its own settings under `build/idea-sandbox/`.

1. `cd bkmr-intellij-plugin && make test-ide`
2. Open `/tmp/bkmr-dev/proj` as a project (scratch files get no completion).
3. Settings → Tools → bkmr: enable LSP Integration; Binary Path = `/tmp/bkmr-dev/bin/bkmr`
   (must be absolute and executable).
4. Close and reopen `t.rs` — the server starts on file open.
5. `Ctrl+Space` on an empty line → compare with the table; repeat for `t.js`, `t.sh`.
6. `grep "Document opened" /tmp/bkmr-dev/lsp.log` — check which languageId the IDE sends.
   If it is not `rust` / `javascript` / `shellscript`-like, only Universal TODO can match:
   that is a plugin-side mapping gap, not a server regression.
7. IDE-side logs: `make log-plugin-raw` in the plugin repo. The plugin's "Debug Logging"
   setting sets `RUST_LOG`, which the bkmr binary ignores; server logs come only from the
   wrapper's `-dd`.

Cleanup: `rm -rf /tmp/bkmr-dev`; reset Binary Path in the sandbox IDE.

### hsearch TC catalogue

Setup: `bash scripts/test/hsearch/setup.sh && export BKMR_DB_URL=/tmp/bkmr_hsearch_test.db`

#### hsearch dataset

| Group | IDs | Purpose | Tags |
|-------|-----|---------|------|
| A: FTS-only | 1-3 | Exact strings, port numbers — no semantic match | mixed |
| B: Semantic-only | 4-6 | Related concepts, different wording — no exact FTS hit | kubernetes, containers |
| C: Both engines | 7-8 | Match FTS + semantic — should rank highest via RRF | kubernetes, _procedure_ |
| D: Tag testing | 9-11 | Various tags for filter testing | _procedure_, _snip_ |
| E: Non-embeddable | 12-13 | Shell/snippet — FTS only, no embeddings | kubernetes, _shell_, _snip_ |

| TC | Command | Expected |
|---|---|---|
| 01 Basic hybrid | `bkmr hsearch "kubernetes health check" --np` | ordered by RRF score; group C (IDs 7, 8) highest; group B/E also appear |
| 02 RRF boosting | `bkmr hsearch "kubernetes health check" --json --np \| python3 -c "import json,sys; [print(r['id'], round(r['rrf_score'],6), r['title'][:60]) for r in json.load(sys.stdin)[:5]]"` | top results ~2× the score of single-engine matches |
| 03 FTS boosted by semantic | `bkmr hsearch "port 8443 TLS" --np` | ID 1 first with ~2× score; others lower (semantic-only); `--mode exact` isolates the FTS hit |
| 04 Semantic-only | `bkmr hsearch "container application isolation" --np` | Docker/Kubernetes security entries appear without verbatim match |
| 05 Tag filter (all) | `bkmr hsearch "kubernetes" --tags _procedure_ --json --np` | every result has `_procedure_` |
| 06 Tag filter (exclude) | `bkmr hsearch "kubernetes" --Tags _snip_ --np` | no `_snip_` results; other kubernetes entries remain |
| 07 Filter excludes all | `bkmr hsearch "kubernetes" --tags nonexistenttag --np` | "No bookmarks found", clean exit |
| 08 Exact mode | `time bkmr hsearch "iptables" --mode exact --np` vs `--mode hybrid` | exact noticeably faster; both return the iptables entry |
| 09 No embeddings | `D=/tmp/bkmr_hsearch_noembeddings.db; rm -f $D; BKMR_DB_URL=$D bkmr create-db $D --pre-fill; BKMR_DB_URL=$D bkmr hsearch "rust" --np` | FTS-only results, no error |
| 10 JSON `rrf_score` | `bkmr hsearch "deployment" --json --np` | every result has float `rrf_score`, sorted descending |
| 11 Piped output | `bkmr hsearch "kubernetes" --np 2>/dev/null \| head -3` | tab-separated `id title url rrf_score`, no color codes |
| 12 Limit | `bkmr hsearch "kubernetes" --limit 2 --json --np` | ≤ 2 results |
| 13 `search` regression | `bkmr search kubernetes --np`; `bkmr search --tags _procedure_ --np` | ordered by ID ascending, tag filter works, format unchanged |
| 14 Over-fetching quality | `bkmr hsearch "deployment automation" --limit 3 --json --np` | cross-engine boosted results in top 3 |

TC-05, 07, 08, 10, 11, 12 and 13 are also covered automatically by `verify.sh`.

### `system-ort` build (dynamic ONNX Runtime)

Goal: a build with `--no-default-features --features system-ort` loads `libonnxruntime` at
runtime from Homebrew and generates embeddings.

```bash
brew install onnxruntime
ls "$(brew --prefix onnxruntime)/lib/libonnxruntime.dylib"

cd bkmr
cargo build --release --no-default-features --features system-ort   # no ort-sys download in output

otool -L target/release/bkmr | grep onnx      # expected: nothing (dlopen, not linked)
ls -lh target/release/bkmr                    # smaller than a default release build

export BKMR_DB_URL=/tmp/bkmr_system_ort_test.db
target/release/bkmr create-db "$BKMR_DB_URL" --pre-fill
target/release/bkmr add "https://example.com" test --title "ONNX Runtime test"
target/release/bkmr backfill                  # embeddings generated → runtime loaded
target/release/bkmr search "ONNX" --np

brew uninstall onnxruntime
target/release/bkmr backfill 2>&1             # expected: dlopen error for libonnxruntime
brew install onnxruntime                      # restore
```

If `dlopen` cannot find the library:
`export ORT_DYLIB_PATH=$(brew --prefix onnxruntime)/lib/libonnxruntime.dylib`.
On Linux use `ldd` instead of `otool -L` and `LD_LIBRARY_PATH` instead of `DYLD_LIBRARY_PATH`.

### Homebrew formula

```bash
brew install onnxruntime
cd /opt/homebrew/Library/Taps/homebrew/homebrew-core
git checkout bkmr-rpath                       # branch with the bkmr formula change

HOMEBREW_NO_INSTALL_FROM_API=1 brew install --build-from-source bkmr
brew test bkmr
otool -l "$(brew --prefix)/bin/bkmr" | grep -A2 LC_RPATH   # rpath embedded
bkmr backfill --help                                        # no dlopen error
```

Compile check without installing (simulates the formula):

```bash
cd bkmr
RUSTFLAGS="-C link-args=-Wl,-rpath,/opt/homebrew/lib" \
  cargo build --release --no-default-features --features system-ort
otool -l target/release/bkmr | grep -A2 LC_RPATH
```

The formula's `RUSTFLAGS` are set inside the install block and do not appear in brew's
output summary; verify via `LC_RPATH` instead.

## Debug logging

The binary's log level comes only from `-d` (info), `-dd` (debug), `-ddd` (trace) — one
global level, no per-module filter. **The binary ignores `RUST_LOG`**; it is honoured only
under `cargo test`. Logs go to stderr; stdout is reserved for command output and, for
`bkmr lsp`, the LSP protocol (enforced by `tests/lsp`).

```bash
bkmr -dd search "test" 2>/tmp/bkmr-debug.log
bkmr -dd lsp 2>/tmp/bkmr-lsp.log            # or use the bkmr-dev wrapper
```

LSP log lines worth grepping: `Document opened: <uri> (language: <id>)`,
`Returning N completion items`, `Failed to get completions`, `Template interpolation failed`.
