# LSP Testing Scripts

This directory contains Python scripts for testing and debugging bkmr's built-in Language Server
Protocol (LSP) implementation. These scripts provide protocol-level testing that complements the
Rust integration tests.

## Prerequisites

- Python 3.6+
- bkmr installed and in PATH
- A bkmr database with snippets (tagged with `_snip_`)

## Available Scripts

### 1. list_snippets.py

**Purpose**: List snippets by language using LSP executeCommand.

**Features**:
- Lists snippets with optional language filtering
- Executes `bkmr.listSnippets` command with language parameter
- Displays results in formatted table or JSON
- Shows snippet details including ID, title, content preview, and tags
- Supports any programming language filter (sh, rust, python, javascript, etc.)
- Proper error handling and LSP protocol compliance

**Usage**:
```bash
# List shell script snippets
./scripts/lsp/list_snippets.py --language sh

# List Rust snippets in JSON format  
./scripts/lsp/list_snippets.py --language rust --json

# List all snippets (no language filter)
./scripts/lsp/list_snippets.py

# Show longer content previews
./scripts/lsp/list_snippets.py --preview 100

# Show detailed view of specific snippet
./scripts/lsp/list_snippets.py --detail-id 5

# Enable debug logging
./scripts/lsp/list_snippets.py --language sh --debug
```

**Sample Output**:
```
🔍 Querying sh snippets...
✅ Found 11 sh snippets

====================================================================================================
📋 SNIPPETS
====================================================================================================
ID    Title                     Preview                                  Tags                     
----------------------------------------------------------------------------------------------------
3021  bash-yes-no               source $TW_BINX/lib/sane_fn.sh...      plain, sh               
3148  sane-fn                   source "$TW_BINX/lib/sane_fn.sh"       plain, sh               
3150  script-default            #!/usr/bin/env bash...                  plain, sh               
----------------------------------------------------------------------------------------------------
Total: 11 snippets
====================================================================================================
```

**Environment Variables**:
- `BKMR_DB_URL`: Database path override
- `RUST_LOG`: Server logging level

### 2. get_snippet.py

**Purpose**: Get individual snippet by ID using LSP executeCommand.

**Features**:
- Retrieves specific snippet by ID
- Executes `bkmr.getSnippet` command
- Shows detailed snippet information with full content
- Displays metadata including title, tags, description, and file info
- JSON output option for programmatic use
- Proper error handling for non-existent or non-snippet bookmarks

**Usage**:
```bash
# Get snippet by ID
./scripts/lsp/get_snippet.py 3165

# Get snippet in JSON format
./scripts/lsp/get_snippet.py 3165 --json

# Enable debug logging
./scripts/lsp/get_snippet.py 3165 --debug
```

**Sample Output**:
```
🔍 Retrieving snippet ID 3165...
✅ Retrieved snippet ID 3165

================================================================================
🔍 SNIPPET DETAILS - ID: 3165
================================================================================
Title: nvim-update-bkmr-nvim
ID: 3165
Tags: 
System Tags: _snip_
Description: Update plugin

Content:
------------------------------------------------------------
nvim --headless "+Lazy! update bkmr-nvim" +qa
------------------------------------------------------------
================================================================================
```

**Parameters**:
- `snippet_id`: Snippet ID to retrieve (required)

**Error Handling**:
- Non-existent bookmarks: "Snippet with ID X not found"
- Non-snippet bookmarks: "Bookmark X is not a snippet" 
- Server errors: Displays specific error messages

### 3. show_commands.py

**Purpose**: Discover and display available LSP commands.

**Features**:
- Queries LSP server for available commands
- Shows command descriptions and parameters
- Provides usage examples for each command
- Supports JSON output for programmatic use
- Can test individual commands

**Usage**:
```bash
# Show all available commands
./scripts/lsp/show_commands.py

# Output in JSON format
./scripts/lsp/show_commands.py --json

# Test a specific command
./scripts/lsp/show_commands.py --test-command bkmr.listSnippets

# With debug logging
./scripts/lsp/show_commands.py --debug
```

**What it shows**:
- All available LSP executeCommand commands
- Parameter descriptions for each command
- Usage examples with sample JSON
- Command testing capabilities

### 4. test_lsp_client.py

**Purpose**: Comprehensive LSP protocol debugging and testing client.

**Features**:
- Complete LSP protocol implementation
- Detailed request/response logging
- Server stderr monitoring
- Timeout management
- Process lifecycle management

**Usage**:
```bash
# Basic test
./scripts/lsp/test_lsp_client.py

# With debug logging
./scripts/lsp/test_lsp_client.py --debug

# With specific database
./scripts/lsp/test_lsp_client.py --db-path ../db/bkmr.db

# Without template interpolation
./scripts/lsp/test_lsp_client.py --no-interpolation
```

**What it tests**:
- LSP server initialization
- Server capabilities
- Document open/close
- Completion requests at various positions
- Proper shutdown sequence

### 5. test_lsp_filtering.py

**Purpose**: Determines whether the LSP server implements server-side or client-side filtering.

**Features**:
- Simulates incremental typing
- Monitors completion behavior
- Analyzes filtering patterns
- Provides clear pass/fail determination

**Usage**:
```bash
# Basic filtering test
./scripts/lsp/test_lsp_filtering.py

# With debug output
./scripts/lsp/test_lsp_filtering.py --debug

# With specific database
./scripts/lsp/test_lsp_filtering.py --db-path ../db/bkmr.db
```

**What it tests**:
- Server-side filtering (optimal): Each keystroke triggers new queries
- Client-side filtering (problematic): Only initial query, then cached results
- Document change handling
- Completion response patterns

### 6. test_lsp_language_filtering.py

**Purpose**: Tests language-aware filtering for different file types.

**Features**:
- Tests multiple programming languages
- Verifies language detection
- Analyzes snippet filtering
- Checks universal snippet inclusion

**Usage**:
```bash
# Test all languages
./scripts/lsp/test_lsp_language_filtering.py

# With debug logging
./scripts/lsp/test_lsp_language_filtering.py --debug

# With specific database
./scripts/lsp/test_lsp_language_filtering.py --db-path ../db/bkmr.db
```

**What it tests**:
- Language ID extraction from textDocument/didOpen
- Language-specific snippet filtering
- Universal snippet inclusion
- Support for: Rust, Python, JavaScript, Go, C, TypeScript

## Environment Variables

All scripts respect these environment variables:

- `BKMR_DB_URL`: Path to bkmr database (overrides --db-path)
- `RUST_LOG`: Logging level (debug, info, warn, error)

## Common Options

All scripts support these command-line options:

- `--debug`, `-d`: Enable debug logging (sets RUST_LOG=debug)
- `--db-path PATH`: Use specific database path
- `--no-interpolation`: Disable template interpolation (where applicable)
- `--help`, `-h`: Show help message

## Integration with Makefile

You can run these tests using make targets:

```bash
# Show available LSP commands
make show-lsp-commands

# Run all LSP tests
make test-lsp

# Individual tests
make test-lsp-client
make test-lsp-filtering
make test-lsp-language
```

## Troubleshooting

### No completions returned

1. Check that you have snippets in your database:
   ```bash
   bkmr search -t _snip_
   ```

2. Ensure bkmr is in your PATH:
   ```bash
   which bkmr
   ```

3. Check the database path:
   ```bash
   echo $BKMR_DB_URL
   ```

### Server crashes immediately

1. Check bkmr version:
   ```bash
   bkmr --version
   ```

2. Ensure LSP support is compiled in:
   ```bash
   bkmr lsp --help
   ```

3. Check for conflicting processes:
   ```bash
   ps aux | grep "bkmr lsp"
   ```

### Debug Mode

Enable debug mode to see detailed server logs:

```bash
RUST_LOG=debug ./scripts/lsp/test_lsp_client.py --debug
```

This will show:
- All LSP messages
- Server-side processing
- Database queries
- Error details

## CI/CD Integration

These scripts can be integrated into CI/CD pipelines:

```yaml
# Example GitHub Actions workflow
- name: Test LSP Protocol
  run: |
    python3 scripts/lsp/test_lsp_client.py
    python3 scripts/lsp/test_lsp_filtering.py
    python3 scripts/lsp/test_lsp_language_filtering.py
```

## Contributing

When adding new LSP test scripts:

1. Follow the existing naming convention: `test_lsp_*.py`
2. Include comprehensive docstrings
3. Support standard command-line options
4. Add error handling and clear output
5. Update this README with documentation

## Related Documentation

- [bkmr LSP Documentation](../../docs/lsp.md)
- [LSP Specification](https://microsoft.github.io/language-server-protocol/)
- [bkmr CLI Documentation](../../README.md)
