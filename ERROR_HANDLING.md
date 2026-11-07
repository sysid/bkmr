# Error Handling

Rules for adding or changing errors in bkmr. Errors are converted at each layer boundary on their
way out to the user:

```
Infrastructure → Domain → Application → CLI / LSP → exit code
```

| Layer | Types | Location |
|---|---|---|
| Infrastructure | `SqliteRepositoryError` | `src/infrastructure/repositories/sqlite/error.rs` |
| Domain | `DomainError`, `RepositoryError`, `InterpolationError` | `src/domain/error.rs`, `src/domain/interpolation/errors.rs` |
| Application | `ApplicationError` | `src/application/error.rs` |
| CLI | `CliError` | `src/cli/error.rs` |
| LSP | `LspError`, `SnippetError` | `src/lsp/error.rs`, `src/lsp/services/snippet_service.rs` |

## Rules

- **One `thiserror` enum per layer**, with a result alias such as `DomainResult<T>`. Do not
  collapse them into one catch-all type.
- **Convert with `From` at layer boundaries**, so `?` propagates. See
  `impl From<SqliteRepositoryError> for RepositoryError` in `sqlite/error.rs`.
- **Add context where the failing operation is known**, either on the error or on the result:

  ```rust
  repository.delete(id).map_err(|e| e.context("deleting bookmark"))?;
  repository.delete(id).with_app_context(|| format!("deleting bookmark {id}"))?;
  ```

  The result helpers are in `src/domain/error_context.rs`.
- **Tell the user how to recover** when there is a clear next step, as the missing-database
  message in `service_container.rs` does.
- **Print fatal errors with `print_error` in `main.rs`**, not `.red()`: crossterm's opt-out still
  writes an empty escape sequence, which ends up in LSP client logs.

## Exit codes

Exit codes are a contract for scripts. `CliError::exit_code()` maps errors to codes, and
`bkmr/tests/test_main.rs` tests them.

| Code | Constant | Meaning | Examples |
|---|---|---|---|
| 0 | `SUCCESS` | Success | |
| 1 | `FAILURE` | Valid invocation, but the operation failed | missing database, unknown bookmark ID in `add --clone` or `delete`, failed `delete`, `delete --force` with another process reading, missing import path |
| 64 | `USAGE` | Fix the invocation or config | unknown flag, no arguments, non-numeric ID, unsupported shell, invalid config file |
| 65 | `DUP` | Duplicate name | `import-files` without `--update` |
| 130 | `CANCEL` | User declined a confirmation | `clear-embeddings`, `delete --force` (plain `delete` exits 0: it promises no removal for good) |

A new `CliError` variant exits 1 unless `exit_code()` says otherwise. Use `InvalidInput` only when
the user can fix the failure by changing the arguments; for a state problem such as an unknown ID,
use `CommandFailed`.

Leaving the fuzzy finder with Esc still exits 0.
