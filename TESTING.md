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

 shared fixtures: bkmr/tests/resources/datasets/*.json (datasets), loaded by
                  scripts/test/lib/seed.sh and tests/lsp; scripts/test/lib/bkmr-dev (wrapper)
```

| Command | Level | What it runs |
|---|---|---|
| `make test` | L1 | all Rust unit + integration tests, single-threaded, `--locked` (fails if `Cargo.lock` is stale); runs in CI |
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

### Test data

Every test starts from an empty database — the migrations insert no rows — and declares
the data it needs:

- **Inline** (most tests): create 1–5 rows in the test through the service or repository
  under test. Use the id it returns, never a literal id.
- **Named dataset** (only where the data *is* the scenario): see [Datasets](#datasets).
- Assert exact result sets. An assertion only inside `for b in &results` (or `<=`/`>=`
  against another query) passes on an empty result and proves nothing.
- Black-box tests use a `TempDir` database seeded with `bkmr add --no-embed --no-web` or
  `TestDb::load_dataset` (offline, no model download).

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

### CI

`.github/workflows/test.yml` runs `make test` on Ubuntu and macOS for pushes to `main` and
for pull requests. Scenario suites are not run in CI (embedding model download).

## Fixtures

| Script | Purpose |
|---|---|
| `scripts/test/lib/seed.sh <lsp\|hsearch> <db>` | fresh database: `bkmr create-db` + `bkmr load-json` of the named dataset; `BKMR_BIN` selects the binary |
| `scripts/test/lib/bkmr-dev` | runs bkmr with `-dd` against `$BKMR_DEV_DIR/test.db` (default `/tmp/bkmr-dev`), stderr → `lsp.log`; `BKMR_BIN` selects the binary |
| `scripts/test/lsp_probe.py` | ad-hoc LSP client (uv script): `capabilities`, `complete <lang…> [--prefix]`, `list [lang]`, `get <id>` |

### Datasets

One JSON file per dataset in `bkmr/tests/resources/datasets/`, the single source for all
levels: `seed.sh` (L2, L3) and `TestDb::load_dataset` in `bkmr/tests/lsp` (L1). Format is
the `bkmr load-json` format (`url`, `title`, `description`, `tags`); each entry also carries
`group` and `why` documenting its purpose, which the importer ignores. Load one by hand:

```bash
bkmr create-db /tmp/x.db
BKMR_DB_URL=/tmp/x.db bkmr load-json --no-embed bkmr/tests/resources/datasets/lsp.json
```

| Dataset | Content | Used by |
|---|---|---|
| `lsp.json` | 63 bookmarks: 7 exercising language filtering (aliases, exact tag match, non-snippets, universal), a `Render counter` template with a `shell` filter, 55 python fillers exceeding `max_completions` (50); loaded without embeddings | `make test-env`; [Completion sessions](#completion-sessions-isincomplete-and-template-rendering) — its table is asserted by `given_lsp_dataset_when_completing_then_matches_documented_table` |
| `hsearch.json` | 13 bookmarks in 5 groups for hybrid search; descriptions frozen from the pages they link to (no network); embeds (model download on first use) | `make test-scenarios`; [hsearch dataset](#hsearch-dataset) |

All fixtures default to the repo debug build; set `BKMR_BIN` to another binary (e.g.
`~/bin/bkmr7.6.8`) to compare against a released version. Check that the "released" binary
really is one: `ls -l ~/bin/bkmr*` — an `install-debug` symlink points at the debug build.

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
| `t.rs` | Rust println, Render counter, Universal TODO | Not a snippet (tag `snip`, no `_snip_`), RA clippy setting (tag `rust-analyzer`) | exact tag matching |
| `t.js` | JS alias log (`js`), JS full-name error (`javascript`), Universal TODO | – | language aliases |
| `t.sh` | Bash echo (`bash`), Universal TODO | – | shell aliases |
| `t.py` | 50 of Py filler 01–55 + Universal TODO (truncated) | – | `max_completions` limit, see [Completion sessions](#completion-sessions-isincomplete-and-template-rendering) |
| `listSnippets javascript` | JS alias log, JS full-name error | Universal TODO | shared alias table |

`scripts/test/lsp_probe.py --bin /tmp/bkmr-dev/bin/bkmr complete rust javascript shellscript python`
prints the same table without an editor.

**Before/after comparison:** run the same steps with `BKMR_BIN=~/bin/bkmr7.6.8` (nvim) or
Binary Path pointing to a released binary such as `~/bin/bkmr7.6.8` (IntelliJ).

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

GOTCHA:
- sandbox IDE config must be put in place completely and then restarted (keeps settings)
- must point to correct binary

#### Completion sessions (isIncomplete and template rendering)

**Scenario** (LSP assessment B2): a completion list is only marked `isIncomplete` when it
was truncated at `max_completions` (50). A complete list is filtered by the client while you
type; only a truncated list is re-requested per keystroke. Every request renders all
templates in its result — including `shell` filters, which execute commands — so the flag
decides how often templates run. Released 7.6.8 marked every list incomplete: one request and
one render per keystroke.

**Data** (dataset `lsp`, seeded by `make test-env`):

| Snippet | Tags | Purpose |
|---|---|---|
| Render counter: `// rendered at {{ "date +%H:%M:%S" \| shell }}` | `rust,_snip_` | each render runs `date`: logged as `Executing shell command: date +%H:%M:%S`, fresh timestamp in the completion preview |
| Py filler 01 … 55 | `python,_snip_` | 55 + Universal TODO = 56 candidates > 50 → truncated list |

**Server-side check (no editor)** — verified 2026-09-30:

```bash
scripts/test/lsp_probe.py --bin /tmp/bkmr-dev/bin/bkmr complete rust python javascript
BKMR_BIN=~/bin/bkmr7.6.8 scripts/test/lsp_probe.py --bin /tmp/bkmr-dev/bin/bkmr complete rust python javascript
```

| languageId | Current build | Released 7.6.8 |
|---|---|---|
| `rust` | 3 items, `isIncomplete=False` | 5 items (incl. Not a snippet, RA clippy setting), `isIncomplete=True` |
| `python` | 50 items, `isIncomplete=True` | 50 items, `isIncomplete=True` |
| `javascript` | 3 items, `isIncomplete=False` | 2 items, `isIncomplete=True` |

Each probe run is a single request, so it renders Render counter exactly once in both
builds; the difference only shows as re-requests in an editor.

**Editor procedure** (nvim or IntelliJ, set up as in the sections above):

| Step | Action | Expected (current build) | Released 7.6.8 |
|---|---|---|---|
| 1 | `: > /tmp/bkmr-dev/lsp.log` | log empty | |
| 2 | open `t.rs`, empty line, trigger completion (C-P); note the Render counter preview time | popup with 3 items | popup with 5 items |
| 3 | with the popup open, type `r`, `e`, `n` | popup narrows to Render counter; preview time unchanged | same narrowing, one server request per key |
| 4 | open `t.py`, empty line, trigger completion, then type `f`, `i`, `l` | one request per key: list stays server-filtered | same |

Counts after step 3:

```bash
grep -c "Completion request for file://.*/t.rs" /tmp/bkmr-dev/lsp.log   # current: 1   7.6.8: 4 (invoke + 3 keys)
grep -c "Executing shell command: date" /tmp/bkmr-dev/lsp.log           # current: 1   7.6.8: 4
```

Counts after step 4:

```bash
grep -c "Completion request for file://.*/t.py" /tmp/bkmr-dev/lsp.log   # both: 4 (truncated → incomplete)
```

The server-side flags are verified; the client-side counts follow the LSP contract and have
not been recorded for every client yet. A client may still re-request on its own (e.g. when
the word start changes or on a trigger character); more requests than expected in step 3 with
`isIncomplete=False` point at client behaviour, not at the server.

### hsearch TC catalogue

Setup: `bash scripts/test/hsearch/setup.sh && export BKMR_DB_URL=/tmp/bkmr_hsearch_test.db`

#### hsearch dataset

| Group | IDs | Purpose | Tags |
|-------|-----|---------|------|
| A: FTS-only | 1-3 | Exact strings, port numbers — no semantic match | mixed |
| B: Semantic-only | 4-6 | Related concepts, different wording — no exact FTS hit | kubernetes, containers |
| C: Both engines | 7-8 | Match FTS + semantic — boosted via RRF | kubernetes, procedure |
| D: Tag testing | 9-11 | Various tags for filter testing | procedure, _snip_ |
| E: Non-URL content | 12-13 | Shell command / SQL snippet (embedded like the rest) | kubernetes, _shell_, _snip_ |

| TC | Command | Expected |
|---|---|---|
| 01 Basic hybrid | `bkmr hsearch "kubernetes health check" --np` | ordered by RRF score; ID 12 (verbatim "kubernetes health check") first, then group C (IDs 7, 8); group B/E also appear |
| 02 RRF boosting | `bkmr hsearch "kubernetes health check" --json --np \| python3 -c "import json,sys; [print(r['id'], round(r['rrf_score'],6), r['title'][:60]) for r in json.load(sys.stdin)[:5]]"` | top results ~2× the score of single-engine matches |
| 03 FTS boosted by semantic | `bkmr hsearch "port 8443 TLS" --np` | ID 1 first with ~2× score; others lower (semantic-only); `--mode exact` isolates the FTS hit |
| 04 Semantic-only | `bkmr hsearch "container application isolation" --np` | Docker/Kubernetes security entries appear without verbatim match |
| 05 Tag filter (all) | `bkmr hsearch "kubernetes" --tags procedure --json --np` | exactly the 4 entries tagged `procedure` |
| 06 Tag filter (exclude) | `bkmr hsearch "kubernetes" --Tags _snip_ --np` | no `_snip_` results; other kubernetes entries remain |
| 07 Filter excludes all | `bkmr hsearch "kubernetes" --tags nonexistenttag --np` | "No bookmarks found", clean exit |
| 08 Exact mode | `time bkmr hsearch "iptables" --mode exact --np` vs `--mode hybrid` | exact noticeably faster; both return the iptables entry |
| 09 No embeddings | `D=/tmp/bkmr_hsearch_noembeddings.db; rm -f $D; BKMR_DB_URL=$D bkmr create-db $D --pre-fill; BKMR_DB_URL=$D bkmr hsearch "rust" --np` | FTS-only results, no error |
| 10 JSON `rrf_score` | `bkmr hsearch "deployment" --json --np` | every result has float `rrf_score`, sorted descending |
| 11 Piped output | `bkmr hsearch "kubernetes" --np 2>/dev/null \| head -3` | tab-separated `id title url rrf_score`, no color codes |
| 12 Limit | `bkmr hsearch "kubernetes" --limit 2 --json --np` | exactly 2 results |
| 13 `search` regression | `bkmr search kubernetes --np`; `bkmr search --tags procedure --np` | ordered by ID ascending, tag filter works, format unchanged |
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


### Native vector schema-query failures

`SqliteVectorRepository::init_vec_table` only treats a successful native
`sqlite_master` answer of false as table absence. A real query failure returns
before DROP or CREATE. Healthy missing, empty and existing-table behavior is
unchanged; dimension replacement still uses its existing separate DROP/CREATE
statements and is not a rollback guarantee.

The real SQLite/native CLI target `test_hybrid_literal_fts` includes corrupt-file
schema failure and missing/existing/restart controls in its default selection.
Its three previously declared model/old-binary cases still require explicit
`--ignored --exact` selection and genuine admitted model or old-binary inputs.
Run default tests single-threaded with the documented `BKMR_DB_URL` and locked
source. New definitions are not evidence of execution; qualification must retain
source/tool/host receipts and actual outputs, including failures.


### Original native SQLite causes and Rust API migration

Vector operations and the two bootstrap WAL branches retain the actual
`rusqlite::Error` in the existing error chain. Display prefixes and context text
retain their prior presentation; the original `Error::source` is moved, not
reconstructed from text. SQLite repository conversion, the production repository
and vector legs of `ServiceContainer`, and the private main command wrapper carry
that source. A process exit/stderr observation does not expose an in-process
error object or certify every other BKMR error route.

The existing publicly reachable `DomainError`, `RepositoryError` and
`ApplicationError` enums add `Caused { presentation, source }`. This is a Rust
source API migration for downstream exhaustive matches. The SQLite implementation
enum adds the same variant inside its existing crate-private module. Consumers
that need the previous classification can match `error.presentation()`; consumers
that need the original error must use `std::error::Error::source`. Context applies
the existing presentation transformation while retaining the same source box.
Repository lookup alone cannot establish external source compatibility. No release
version or provider cutover is declared by this candidate.

The real `test_hybrid_literal_fts` target now has 15 definitions: 12 default and
three explicitly ignored genuine model/old-binary definitions. The original eight
definitions are retained byte for byte. The existing strict model repository-error
case also checks actual rusqlite cause/code and identical source allocation after
CLI context; it still performs genuine document/query inference after native
recovery. New default cases cover corrupt-file schema failure, create/restart,
missing-table original cause/context, and an actual CLI corrupt-WAL failure.
The actual-readonly file case is in `sqlite::vector_repository::native_readonly_tests`;
the production DI corrupt-WAL leg is in `di::service_container::native_cause_tests`;
the private main wrapper has a genuine SQLite missing-table cause test. The
readonly fixture uses a real SQLite READ_ONLY connection to the owned file and
the same production init method, not an injected error or fabricated result.
The DI case calls the exact production repository leg before embedder/clipboard
composition; it does not use `ServiceContainer::new()` or a dummy provider.

Retain normal `make test` and select these exact native definitions as well:

```sh
cd bkmr
cargo test --locked --test test_hybrid_literal_fts -- --test-threads=1 --nocapture
cargo test --locked --lib native_readonly_tests:: -- --test-threads=1 --nocapture
cargo test --locked --lib native_cause_tests:: -- --test-threads=1 --nocapture
cargo test --locked --bin bkmr given_actual_native_vector_failure_when_main_wraps_then_display_and_original_source_are_preserved -- --test-threads=1 --nocapture
cargo test --locked --test test_hybrid_literal_fts given_genuine_native_model_when_vector_presence_query_fails_then_hybrid_preserves_error_and_healthy_fallback -- --ignored --exact --test-threads=1 --nocapture
```

The two other already-declared ignored controls (genuine model contribution and
pinned 7.6.7 old-binary characterization) remain mandatory, with their existing
strict prerequisites. An unavailable prerequisite is a failure, not a green skip.
Use the same actual maintained source/compiler/run for the default and selected
cases. The 7.6.7 cut and lock are independent and cannot qualify this 7.6.11 source
through a copied receipt. The unchanged ordinary `make test` command selects
default tests; that command alone does not establish completion of the three
ignored controls. The prior upstream workflow selected only this ordinary
command; the qualification successor below adds their mandatory selection.
These source definitions and commands are unexecuted until native hosted
qualification. Capture limits asserted after completion are not live memory or
process-lifecycle guarantees. In-process model tests need the declared hosted
owner deadline and explicit model/cache provenance.

Dimension changes still perform DROP followed by CREATE. Bookmark creation commits
before embedding storage; bookmark update may write the vector before its bookmark
update; neither cross-connection sequence nor vector DELETE/INSERT is a rollback
guarantee. A failure may follow these known partial effects; no automatic retry is
added.
INFO's defaulted vector observations, backfill aggregate reporting, unrelated
Diesel/r2d2/context conversion losses and their existing diagnostic audiences are
separate retained limitations with native-owner follow-up, not repaired by this
cause chain.


## Required native hybrid, SQLite cause and backup qualification

The existing `Test` Ubuntu 24.04/macOS 14 job retains the ordinary `make test`
command and adds mandatory explicit qualification of the fifteen definitions in
`test_hybrid_literal_fts`, the three native lib/bin INIT/WAL/main cause cases,
and six native model-free backup-observation/migration lib cases: twenty-four
unique selected definitions, twenty-one default and three ignored. The selected
roles are fifteen integration, eight lib and one bin; the complete compiled lib
roster must contain all eight selected lib names. Existing case identities 01–18
remain stable and the six backup cases append as 19–24. The integration target
still has twelve default and three ignored definitions.
The ignored definitions are selected with `--ignored --exact`; an absent model,
unqualified before binary, native error, timeout, zero-test dispatch or skipped
case is a failure, never a semantic pass. The compiled libtest rosters and exact
one-case summaries are audited by `scripts/test/verify-native-hybrid-results.py`.
The helper does not launch processes or substitute native observations.

The six backup cases use real SQLite schema absence, allocated native rows,
exclusive locking with an independent BUSY prerequisite, a broken view and
original Diesel cause, working/case/temporary relation resolution, and the
actual embedded pending-migration/backup owner. Failed schema or COUNT stays
unavailable before backup/final migration; successful absence/count keeps its
existing meaning. The migration case preserves row/schema/bytes/applied versions
on the real failure, restores the native relation, affirms committed DELETE
journal mode after restoration, and independently reopens the actual backup
before the final migration clears the retained legacy BLOB. That BLOB is ordinary
retained data, not a fabricated vector/model result. These source definitions
remain unparsed/uncompiled/unrun until genuine native host qualification. They
do not qualify production concurrent WAL copying, the size gate/date overwrite,
physical backup lifecycle or unrelated migration/pool/IO cause losses.

The current code and dependency locks are pinned separately from an unmodified
7.6.7 before executable. That prerequisite is built from the published sdist
SHA-256 `a7350a3372263fc7e89f8b80f8d4f7966a19aa5c08a50d0a89b5c9c67466b7f2`,
with its own locked source and actual compiler-produced executable. The gate
retains both source/lock identities, actual executable bytes and hashes, native
version output and Rust/Cargo/host observations. No old source patch, invented
release or dynamic embedding-provider fallback is selected.

The model is explicitly `AllMiniLML6V2` in an exclusive product-native
`ProjectCentral/now/tmp` fixture. Controlled HOME/XDG/model/DB paths avoid an
ambient private cache; the native Rust tool homes remain explicit. The actual
current BKMR `create-db`, `add --no-web` (embedding enabled) and `info --schema`
commands prepare and observe declared fixture data. Model cache availability
and this warm command alone do not qualify inference: the two mandatory native
model tests must perform actual document/query inference, vector storage,
hydration and RRF/error/recovery assertions. FastEmbed may download public model
files during that actual operation. Network/provider absence and genuine
failures remain failures with raw output. Both dependency/model generations
are distinct; the old compatibility case does not claim old-model inference.

Each host job has a 180-minute limit, with finite compile, ordinary suite,
prerequisite, case and retention step limits. The maximum step allowances are
not a guarantee that cold builds/downloads fit inside the job. Existing
`assert_cmd` direct-child timeouts and post-capture size assertions do not prove
bounded live memory, disk growth, descendant retirement or safe cancellation.
An interrupted job can have incomplete artifacts; that is not qualification.
Raw stdout/stderr/status and actual errors are retained without truncating a
semantic result. The audit refuses a parse above 16 MiB, a regular artifact
above 512 MiB, or selected fixture material above 1 GiB/8192 files/20000 entries;
refusal retains available raw files, rather than clipping success. Bounded
material enumeration prunes old build/source trees and limits its frontier.
The artifact retains the actual selected binaries, model material, source tar,
compiled rosters, case logs/results and cleanup observations. The original
`make test` outcome and complete hosted job log are separately required; repeat
ordinary-suite dispatches do not add distinct native-definition credit.

Actual exclusive root custody is published before later fixture effects, and
an actual evidence locator is published before fixture creation. Qualified
partial preparation failures remain eligible for always-retention/upload;
actual errors are recorded, while unavailable ownership stays unknown and
cannot authorize a guessed artifact path. Available attributable material is
copied into evidence; the fixture is preserved for the existing finite hosted
lifecycle even when all selected cases pass. An outer native exit/EOF does not
prove inner child retirement, so this audit performs no recursive deletion.
Failed/incomplete/unknown case, prerequisite or retention outcomes preserve
material and report their actual disposition. Host retirement is not observed
by this helper. These local fixture checks do not exclude concurrent external
mutation. Actual SQLite
operations/schema/vector observations and native locked dependencies are
retained; a Python/helper SQLite runtime is not the compiled BKMR SQLite
runtime. The native runtime library version is not inferred from a lockfile.

This workflow is contribution qualification, not provider installation,
consumer capability activation, whole-project acceptance, or a model grant.
It must be normally reviewed/applied through an admitted upstream contributor
or maintainer source route. A read-only repository account or an immutable
T-only proposal does not establish that writable route. Before-source INIT and
cause characterization remains required separately; incompatible old Rust
APIs cannot be represented as a manufactured before-test success.
