# Comprehensive bkmr LSP Integration Plan

## Current State Analysis

**Main bkmr Project:**
- Version 4.31.0 with mature CLI architecture
- Clean layered structure: domain → application → infrastructure → cli
- Rich feature set: search, semantic search, tags, CRUD operations
- Diesel/SQLite database with migrations
- Connection pooling via r2d2 with OnceLock singleton pattern
- Extensive configuration and embedding support

**bkmr-lsp Project:**
- Version 0.8.0 with tower-lsp framework
- Clean architecture with domain/services/repositories layers
- Currently spawns bkmr CLI as subprocess for all operations (performance bottleneck)
- LSP-specific features: completion, workspace edits, language translation
- Each request creates new subprocess - no direct database access

## Consolidation Strategy: Multi-Entrypoint Architecture

### Phase 1: Repository Restructuring (1-2 days)
1. **Merge Projects**: Move bkmr-lsp into bkmr/src/lsp/
2. **Update Cargo.toml**: Add LSP dependencies as optional features
3. **Create Entrypoint Router**: Extend main.rs to handle `bkmr lsp` command
4. **Preserve CLI**: Ensure 100% backward compatibility for existing commands

### Phase 2: Core Library Integration (2-3 days)
1. **Extract Shared Services**: Leverage existing service factory pattern from `application/services/factory.rs`
2. **Create LSP Adapter**: Bridge LSP domain models to bkmr domain models
3. **Remove CLI Subprocess**: Replace BkmrRepository subprocess calls with direct service layer access
4. **Reuse Connection Pool**: LSP will use the same `OnceLock<Arc<SqliteBookmarkRepository>>` pattern
5. **Implement CRUD API**: Add create/update/delete operations for LSP editing if needed

### Phase 3: Feature Integration (1-2 days)
1. **Snippet Editing**: Add `bkmr.editSnippet`, `bkmr.createSnippet` LSP commands
2. **Virtual Documents**: Implement snippet editing via LSP workspace documents
3. **Language Translation**: Keep existing language-specific comment/indent translation
4. **Configuration Sharing**: Use same config.toml for both CLI and LSP modes

### Phase 4: Testing & Migration (1 day)
1. **Preserve Tests**: Migrate all existing tests from both projects
2. **Integration Testing**: Verify CLI commands work identically
3. **LSP Protocol Testing**: Ensure LSP functionality is preserved
4. **Performance Validation**: Confirm no CLI performance regressions

## New Architecture Structure

```
bkmr/
├── src/
│   ├── main.rs              # Entrypoint router: CLI vs LSP mode
│   ├── lib.rs               # Shared library exports
│   ├── domain/              # Core business logic (shared)
│   ├── application/         # Services & use cases (shared)
│   ├── infrastructure/      # Database, HTTP, etc. (shared)
│   ├── cli/                 # CLI-specific code
│   └── lsp/                 # LSP-specific code
│       ├── backend.rs       # LSP protocol implementation
│       ├── completion.rs    # Completion logic
│       ├── editing.rs       # Snippet editing commands
│       └── translation.rs   # Language translation
├── Cargo.toml               # Unified dependencies with features
└── tests/                   # Combined test suite
```

## Key Benefits
- **Single Binary**: Users install one `bkmr` that does everything
- **Shared Logic**: No code duplication between CLI and LSP
- **Direct Integration**: LSP uses native APIs, not subprocess calls
- **Better Performance**: Eliminate CLI spawning overhead (each LSP request currently spawns new process)
- **Resource Efficiency**: Single connection pool instead of new connections per subprocess
- **Unified Config**: Same configuration for both modes
- **Rich Editing**: Full CRUD operations via LSP commands

## Migration Commands
```bash
# Existing CLI (unchanged)
bkmr search "query"
bkmr add --url "..." --tags "..."

# New LSP mode
bkmr lsp                     # Start LSP server
bkmr lsp --no-interpolation  # LSP with raw templates
```

## Implementation Details

### Feature Flag Strategy
```toml
[features]
default = ["cli"]
cli = []
lsp = ["tower-lsp", "tokio", "async-trait"]
all = ["cli", "lsp"]
```
This allows users to compile only what they need, reducing binary size.

### Async/Sync Bridge
Since LSP requires async (tower-lsp/tokio) but CLI is sync:
- Use `tokio::runtime::Handle` for selective async blocks in LSP mode
- Services remain sync with async wrappers for LSP usage
- No changes needed to existing CLI code paths

### Entry Point Router
```rust
// main.rs modification
fn main() {
    let cli = Cli::parse();
    match cli.command {
        Some(Commands::Lsp { no_interpolation }) => {
            #[cfg(feature = "lsp")]
            bkmr::lsp::run_lsp_server(no_interpolation);
            #[cfg(not(feature = "lsp"))]
            eprintln!("LSP support not compiled. Build with --features lsp");
        }
        _ => execute_command(stderr, cli) // Existing CLI logic
    }
}
```

### Performance Improvements Expected
- **Current LSP**: Each completion request spawns `bkmr search --json` subprocess (~50-100ms overhead)
- **After Integration**: Direct service call (~1-5ms)
- **Expected speedup**: 10-50x faster response times for LSP operations

## Backward Compatibility Guarantee
- All existing CLI commands work identically
- Same configuration file format and locations
- Same database schema and data
- Same output formats and behavior
- Zero breaking changes for existing users

## Timeline Estimate
- **Phase 1**: Repository Restructuring (1 day)
- **Phase 2**: Core Library Integration (2-3 days)
- **Phase 3**: Feature Integration (1-2 days)
- **Phase 4**: Testing & Validation (1 day)
- **Total**: 5-7 days (reduced from initial 7-10 day estimate)