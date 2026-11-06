# bkmr
Rust CLI + built-in LSP server for knowledge management (bookmarks, snippets, shell commands,
markdown, text) with FTS5 + local semantic search; Clean Architecture with explicit dependency
injection.

## Setup
- The Cargo manifest is in `bkmr/`, NOT the repo root. Run cargo from `bkmr/` or pass `--manifest-path bkmr/Cargo.toml`.
- DB-accessing code and tests require `BKMR_DB_URL` to point at a SQLite file (tests use `../db/bkmr.db`).

## Commands
- `make test` — full suite incl. LSP; sets `BKMR_DB_URL` and forces single-threaded. Use this before reporting success.
- Single test: `env RUST_LOG=error BKMR_DB_URL=../db/bkmr.db cargo test --manifest-path bkmr/Cargo.toml <name> -- --test-threads=1 --nocapture`
- `make lint` (clippy --fix + cargo fix), `make format` (cargo fmt).
- `make bump-patch|minor|major` bumps the version AND tags and pushes — never run it unasked.
- `TESTING.md` is the single source for testing (levels, fixtures, scenario suites, manual editor tests); link to it, don't repeat it.

## Architectural Invariants
- **Layered dependency direction**: Domain has zero external deps; Application depends only on Domain traits; Infrastructure implements Domain traits; CLI and LSP depend on Application services. Never invert these arrows.
- **Single composition root**: All service wiring happens in `main.rs` / `ServiceContainer`. Services take dependencies as `Arc<dyn Trait>` constructor params. No global state, no factory or service-locator methods.
- **Explicit settings**: Pass `Settings` down as parameters; NEVER call `Settings::default()` inside a service.
- **Single-threaded DB tests**: Tests share one SQLite file, so they MUST run with `--test-threads=1` and `BKMR_DB_URL` set, or they fail with lock errors. `make test` enforces both.
- **Layered error hierarchy**: Errors use `thiserror` enums per layer with `DomainResult<T>` / `ApplicationResult<T>` aliases; do not collapse them into a single catch-all error type. See `ERROR_HANDLING.md` before adding error types.

## Strict Antipatterns
- NEVER use `ServiceContainer::new()` in tests — use `TestServiceContainer::new()`.
- NEVER expose concrete service or repository types across layers — always `Arc<dyn Trait>`.
- NEVER run `cargo test` without `--test-threads=1`.

## Gotchas
- File-import content is stored in the `url` column, not `description`.
- Embeddings live in the sqlite-vec `vec_bookmarks` virtual table, NOT `bookmarks.embedding`. Regenerate with `bkmr backfill --force`; remove stale vectors without re-embedding with `bkmr backfill --prune`.
- Two SQLite connections hit the same file: a Diesel r2d2 pool (CRUD) and one mutex-guarded rusqlite connection (sqlite-vec). WAL is set once in `init_pool()`; `busy_timeout` is per-connection. Keep pragmas consistent when adding connections.
- DB-independent commands (`--generate-config`, `completion`, `create-db`) are routed before `ServiceContainer` creation in `main.rs` and must not require a database.
- Tag filter flags are easy to misread: `-t` must have ALL, `-T` exclude if has ALL, `-n` must have ANY, `-N` exclude if has ANY.
- Test function naming convention: `given_X_when_Y_then_Z()`.
- Exit codes are a tested contract: `CliError::exit_code()` decides them, and `ERROR_HANDLING.md` has the table. Use `InvalidInput` only for errors the user fixes by changing arguments.
- An unreadable or invalid config file is fatal in `main.rs` (exit 64), since falling back to defaults would open another database. Only `--generate-config` bypasses this.
- Print fatal errors with `print_error` in `main.rs`, not `.red()`: crossterm's opt-out still writes an empty escape sequence, which ends up in LSP client logs.
- `embeddable` is decided by `[embeddings.policy]` in `BookmarkServiceImpl`: pass `None` to `add_bookmark` and friends unless the user made an explicit request, never a hardcoded `true`. A policy change does not touch existing rows, except that `always` applies on their next write.

## Domain glossary
- `_snip_`, `_shell_`, `_md_`, `_env_`, `_imported_`, `_mem_` — known system tags marking content type; a bookmark has at most one, validated in the domain setters, `BookmarkBuilder` and the repository's `add`/`update`. `record_access` skips the check so existing violating rows stay openable. Unknown `_x_` tags are allowed and don't count toward that limit.
- `hsearch` — hybrid search fusing FTS5 and semantic vectors via Reciprocal Rank Fusion.
- access — recorded when one explicitly chosen bookmark's content starts being delivered (open, execute, copy, print, show details), via `ActionService::record_access`. Listing results and changing data are not accesses. See the wiki's Basic-Usage for the table.

## Other Instructions
- Follow TDD, and keep README and the GitHub wiki in sync with behavior changes.
