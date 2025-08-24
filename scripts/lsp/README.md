# LSP Testing Scripts

This directory contains Python scripts for testing and debugging bkmr's built-in Language Server
Protocol (LSP) implementation. These scripts provide protocol-level testing that complements the
Rust integration tests.

## Prerequisites

- Python 3.6+
- bkmr installed and in PATH
- A bkmr database with snippets (tagged with `_snip_`)

## Available Scripts

### 1. show_commands.py

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

### 2. test_lsp_client.py

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

### 3. test_lsp_filtering.py

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

### 4. test_lsp_language_filtering.py

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
   bkmr search --ntags-prefix _snip_
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
