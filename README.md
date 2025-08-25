# bkmr

![Crates.io](https://img.shields.io/crates/v/bkmr)
![Crates.io](https://img.shields.io/crates/d/bkmr)
[![Docs.rs](https://docs.rs/bkmr/badge.svg)](https://docs.rs/bkmr)
[![Build Status][build-image]][build-url]

> Combine any snippet (code, urls, markdown, text) with powerful search, interpolation and execution.
 
# Beyond Bookmarks and Snippets: A CLI Knowledge Management System

[bkmr reborn](https://sysid.github.io/bkmr-reborn/)

`bkmr` - selected [crate of the week 482](https://this-week-in-rust.org/blog/2023/02/15/this-week-in-rust-482/) - is a fast, feature-rich command-line tool that extends bookmark management, snippet handling, markdown rendering, script execution and more.

**Organize**, **find**, and **apply** your various content types:

- Web URLs with automatic metadata extraction
- Code snippets for quick access and reuse
- Shell commands with execution capabilities
- Markdown documents with live rendering
- Plain text with template interpolation
- Local files and directories
- Semantic embeddings for AI-powered search

## Why bkmr?

- **Developer-focused**: Integrates seamlessly with your workflow and toolchain
- **Multifunctional**: Handles many content types with context-aware actions
- **Intelligent**: Full-text and semantic search capabilities
- **Privacy-focused**: Local database, no cloud dependencies unless enabled
- **Fast**: 20x faster than similar Python tools

### NEW: [Editor Integrations!](#editor-integrations)

- **Built-in LSP server**: Use `bkmr lsp` for VS Code, Vim, Emacs - automatic snippet completion with language-aware filtering
- **[Neovim Plugin](https://github.com/sysid/bkmr-nvim)**: Visual interface with Telescope integration and zero configuration
- **[IntelliJ Plugin](https://github.com/sysid/bkmr-intellij-plugin)**: JetBrains Marketplace plugin for all IDEs

## Core Features

```bash
# Quick fuzzy search across your content with interactive selection
bkmr search --fzf

# Advanced filtering with tags
bkmr search -t python,security "authentication"

# Add web URLs, enrich with metadata automatically
bkmr add https://example.com tag1,tag2  # title, description, etc will be loaded automatically

# Store code snippets
bkmr add "SELECT * FROM users WHERE role = 'admin'" sql,snippet --type snip --title "My Sql"
bkmr search --fzf --fzf-style enhanced -t _snip_  # show interactive selection menu

# shell scripts, added via interactive editor
bkmr add sysadmin,utils --type shell
# Bookmark Template
# Lines starting with '#' are comments and will be ignored.
# Section markers (---SECTION_NAME---) are required and must not be removed.

---ID---

---URL---
#!/bin/bash
echo "Hello World!"
---TITLE---
System Status
---TAGS---
_shell_
---COMMENTS---
Show the system status
---EMBEDDABLE---
false
---END---

# Run the script (default action for this content-type is called automatically when search returns exactly one)
bkmr search -t _shell_ "System Status"
> Found 1 bookmark: System Status (ID: 22). Executing default action...
> Execute: #!/bin/bash
echo "Hello World!"
# Edit the command if needed, press Enter to execute, or Ctrl-C to cancel
> Hello World!


# Store markdown which will be rendered in the browser with Table of Contents
bkmr add "# Project Notes\n\n## Tasks\n- [ ] Complete documentation\n- [ ] Write tests" notes,project --type md --title Markdown
bkmr open <id>  # opens in browser with interactive TOC sidebar for navigation
bkmr add "/path/to/markdown.md" --type md -t "Markdown File to be rendered"  # automatically detects path instead of content

# View markdown files directly without storing as bookmarks
bkmr open --file README.md              # renders with TOC, syntax highlighting
bkmr open --file ~/docs/notes.md        # supports relative and absolute paths
bkmr open --file ./documentation.md     # automatic markdown rendering in browser

# Store environment variables for sourcing in a shell
bkmr add "export DB_USER=dev\nexport DB_PASSWORD=secret\nexport API_KEY=test_key" dev,env --type env --title 'My Environment'
bkmr search --fzf --fzf-style enhanced -t _env_  # select it for sourcing

# Import files with frontmatter parsing and base path support
bkmr import-files ~/scripts/backup.sh --base-path SCRIPTS_HOME
bkmr edit 123  # Smart editing: opens source file for file-imported bookmarks

# Execute shell commands via bookmark (deprecated, use content-type _shell_ instead)
bkmr add "shell::find ~/projects -name '*.go' | xargs grep 'func main'" tools,search --title 'Search Golang'

# Semantic search with AI
bkmr --openai sem-search "containerized application security" --limit 3
```
### Bookmarks
<img src="./docs/bkmr4-bookmarks.png" alt="bookmarks" width="800"/>

### Snippets
<img src="./docs/bkmr4-fzf-snippets.png" alt="fzf-snippets" width="800"/>

## Demos

See bkmr in action:

- <a href="https://asciinema.org/a/q4Okf4j2ja757Nav5wf0tNz8k?autoplay=1" alt="Never Context-Switch Again"><img src="https://asciinema.org/a/q4Okf4j2ja757Nav5wf0tNz8k.svg" /></a>

- <a href="https://asciinema.org/a/VTsHuw1Ugsbo10EP0tZ3PdpoG?autoplay=1&speed=2&t=3" alt="Overview">Overview</a>

- <a href="https://asciinema.org/a/wpnsTw3Cl7DK2R7jK7WVpp9OR?autoplay=1&speed=2&t=3" alt="Getting Started">Getting Started</a>
- <a href="https://asciinema.org/a/M97UJMKxw1nxnzO4SaowGZAmb?autoplay=1&speed=2&t=3" alt="Search and Filter">Search and Filter</a>
- <a href="https://asciinema.org/a/uCuNPSlqRemlcXiVQ3CIqq8uV?autoplay=1&speed=2&t=3" alt="Edit and Update">Edit and Update</a>
- <a href="https://asciinema.org/a/jNOLfhc6aFV3wPGTgOzgrM7Kc?autoplay=1&speed=2&t=3" alt="Tag Management">Tag Management</a>

## Getting Started

1. **Install:**
   ```bash
   cargo install bkmr

   # or via pip/pipx/uv
   pip install bkmr

   # or via brew
   brew install bkmr

   ```

2. **Setup:**
   ```bash
   # Configuration 
   bkmr --generate-config > ~/.config/bkmr/config.toml

   # Create database
   bkmr create-db ~/.config/bkmr/bkmr.db
   
   # Optional: Configure location (override config.toml)
   export BKMR_DB_URL=~/path/to/db
   ```

3. **Start using:**
   ```bash
   # Add your first bookmark
   bkmr add https://github.com/yourusername/yourrepo github,project
   
   # Find it again
   bkmr search github
   ```

## Command Reference

| Command | Description |
|---------|-------------|
| `search` | Search across all content with full-text and tag filtering |
| `sem-search` | AI-powered semantic search using OpenAI embeddings |
| `add` | Add new content (URLs, snippets, files, shell commands, etc.) |
| `open` | Launch or interact with stored items (supports script arguments) |
| `edit` | Smart editing: auto-detects file-imported bookmarks for source file editing |
| `import-files` | Import files/directories with frontmatter parsing and base path support |
| `create-shell-stubs` | Generate shell function stubs for all shell script bookmarks |
| `tags` | View and manage your tag taxonomy |
| `set-embeddable` | Configure items for semantic search |

## Smart Content Actions

`bkmr` intelligently handles different content types with appropriate actions:

| Content Type          | Default Action                        | System Tag   |
|-----------------------|---------------------------------------|--------------|
| URLs                  | Open in browser                       | (none)       |
| Snippets              | Copy to clipboard                     | `_snip_`     |
| Shell Scripts         | Interactive edit then execute in terminal | `_shell_`    |
| Environment Variables | Print to stdout for sourcing in shell | `_env_`      |
| Markdown              | Render in browser with TOC sidebar   | `_md_`       |
| Text Documents        | Copy to clipboard                     | `_imported_` |
| Local Files           | Open with default application         | (none)       |

## Advanced Features

- **Smart editing system**: Automatically detects file-imported bookmarks and edits source files directly
- **File import with base paths**: Import files with portable path storage using configurable base path variables
- **Interactive shell editing**: Shell scripts present an interactive editor with vim/emacs bindings before execution
- **Template interpolation**: Use Jinja-style templates in URLs and commands
- **Content embedding**: Store semantic representations for AI-powered search
- **Context-aware actions**: Different behaviors based on content type
- **Multiple output formats**: Terminal display, clipboard, or JSON export

### Shell Script Interaction

Shell scripts (`_shell_` content type) provide an interactive editing experience:

- **Pre-filled editing**: Original script appears ready for modification
- **Vim/Emacs bindings**: Automatically detects your shell's edit mode from `.inputrc`, `$ZSH_VI_MODE`, etc.
- **Parameter support**: Add arguments, modify commands, or combine multiple commands
- **History integration**: Commands are saved to `~/.config/bkmr/shell_history.txt`
- **Configurable behavior**: Can be disabled via configuration for direct execution

```bash
# Interactive mode (default) - edit before execution
bkmr search -t _shell_ "backup script"
Execute: rsync -av /home/user/docs /backup/
# Edit to add parameters: rsync -av /home/user/docs /backup/$(date +%Y%m%d)/
# Press Enter to execute

# Direct execution with arguments (skip interactive editing)
bkmr open --no-edit <id> -- arg1 arg2 arg3

# Disable interactive mode via configuration
export BKMR_SHELL_INTERACTIVE=false
# or in ~/.config/bkmr/config.toml:
# [shell_opts]
# interactive = false
```

#### Shell Function Stubs

Create shell functions for all your shell script bookmarks to enable direct execution with arguments:

```bash
# Generate shell function stubs
bkmr create-shell-stubs

# Example output:
# backup-database() { bkmr open --no-edit 123 -- "$@"; }
# export -f backup-database
# deploy-app() { bkmr open --no-edit 124 -- "$@"; }
# export -f deploy-app

# Source directly into your current shell
source <(bkmr create-shell-stubs)

# Add to your shell profile for permanent access
echo 'source <(bkmr create-shell-stubs)' >> ~/.bashrc
# or for better performance, cache the output:
bkmr create-shell-stubs >> ~/.bashrc

# Now use your bookmarked scripts directly with arguments
backup-database production --incremental
deploy-app staging --rollback
```

**Function Name Generation:**
- Preserves hyphens: `"backup-database"` → `backup-database()`
- Converts spaces to underscores: `"Deploy Script"` → `deploy_script()`
- Handles edge cases: `"2fa-setup"` → `script-2fa-setup()`

### File Import and Smart Editing

`bkmr` supports importing files with structured metadata and provides intelligent editing capabilities:

```bash
# Import files with frontmatter parsing
bkmr import-files ~/scripts/backup.sh ~/docs/notes.md

# Import with base path configuration for portable storage
bkmr import-files scripts/backup.sh --base-path SCRIPTS_HOME

# Smart editing automatically detects file-imported bookmarks
bkmr edit 123  # Opens source file in $EDITOR if file-imported, database editor otherwise

# Force database content editing
bkmr edit 123 --force-db
```

**File Import Features:**
- **Frontmatter parsing**: Supports YAML (`---` delimited) and hash-style (`# key: value`) metadata
- **Base path configuration**: Store portable paths using configurable base path variables
- **Incremental updates**: SHA-256 hash tracking for efficient re-imports
- **Directory traversal**: Recursive processing with `.gitignore` support

**Smart Editing Features:**
- **Automatic detection**: Identifies file-imported vs regular bookmarks
- **Source file editing**: Opens original files in your `$EDITOR` for file-imported bookmarks
- **Metadata synchronization**: Changes to frontmatter automatically sync to database
- **Path resolution**: Handles base path variables and environment expansion

**Configuration Example:**
```toml
# ~/.config/bkmr/config.toml
[base_paths]
SCRIPTS_HOME = "$HOME/scripts"
DOCS_HOME = "$HOME/documents"
WORK_SCRIPTS = "/work/automation/scripts"
```

For detailed documentation on advanced features:
- [Configuration Options](./docs/configuration.md)
- [Content Types](./docs/content-types.md)
- [File Import and Smart Editing](./docs/file-import-smart-editing.md)
- [Advanced Usage](./docs/advanced_usage.md)
- [Semantic Search](./docs/semantic-search.md)

## Editor Integrations

Instead of switching between editor and terminal, access snippets directly within the IDE.

### Neovim Plugin (Recommended for Neovim Users)

**[bkmr-nvim](https://github.com/sysid/bkmr-nvim)** provides the best Neovim integration experience with visual interface and zero configuration.

**Installation with lazy.nvim:**
```lua
{
  "sysid/bkmr-nvim",
  dependencies = { "nvim-lua/plenary.nvim" },
  config = function()
    require("bkmr").setup() -- Zero config required!
  end,
}
```

**Key Features:**
- **Visual snippet browser**: Telescope/FZF integration for browsing and searching
- **In-editor editing**: Create and edit snippets without leaving Neovim
- **Automatic LSP setup**: Configures bkmr LSP server automatically
- **Custom commands**: `:Bkmr`, `:BkmrEdit`, `:BkmrSearch`

### Built-in LSP Server

**bkmr** includes a built-in Language Server Protocol (LSP) server accessible via `bkmr lsp` for any LSP-compatible editor (VS Code, Vim, Emacs, Sublime, etc.).

**Usage:**
```bash
# Start the LSP server (typically configured in your editor)
bkmr lsp

# Disable template interpolation if needed
bkmr lsp --no-interpolation
```

**Key Features:**
- **Automatic completion**: Snippets appear in completion popup while typing
- **Language-aware filtering**: Shows only relevant snippets based on file type
- **Universal snippets**: Write once in Rust syntax, automatically adapt to any language
- **Template interpolation**: Server-side processing of bkmr templates
- **LSP commands**: CRUD operations and utilities directly from editor

For comprehensive documentation including editor configuration, see [LSP Documentation](./docs/lsp.md).

### IntelliJ Platform Plugin

**[bkmr-intellij-plugin](https://github.com/sysid/bkmr-intellij-plugin)** brings bkmr integration to all JetBrains IDEs.

**Key Features:**
- **Seamless LSP integration**: Automatic snippet completion with no manual triggers
- **Tab navigation**: Full snippet placeholder support with Tab/Shift+Tab navigation
- **Cross-platform**: Works in IntelliJ IDEA, PyCharm, WebStorm, CLion, RustRover, and all JetBrains IDEs
- **Filepath comments**: Insert relative filepaths as comments with smart language detection

### Universal Snippets

Both integrations support **universal snippets** - write snippets once using natural Rust syntax, and they automatically translate to your target language:

```rust
// Universal snippet (stored as Rust syntax)
// Function: {{ function_name }}
// TODO: implement
    return {{ value }};
```

**Automatic translation:**
- **Python**: `// comment` → `# comment`
- **HTML**: `// comment` → `<!-- comment -->`
- **Indentation**: Adapts to language conventions (tabs for Go, 2 spaces for JS, etc.)

## Developer Workflow Integration

`bkmr` transforms your terminal into a knowledge hub for development tasks:

1. **Unified knowledge store** - Access code snippets, documentation, and resources with one command
2. **Reduced context switching** - Launch applications and execute commands without leaving your workflow
3. **Smart clipboard management** - Quickly access common snippets without leaving the terminal
4. **Documentation at your fingertips** - Render markdown and technical notes instantly
5. **Automation shortcuts** - Turn complex command sequences into reusable bookmarks

## Upgrading from Previous Versions

If you're upgrading from a previous version, `bkmr` will automatically:
1. Check for necessary database migrations
2. Create a timestamped backup of your current database
3. Apply migrations to support newer features

## Development

### Building from Source

```bash
# Clone the repository
git clone https://github.com/sysid/bkmr.git
cd bkmr

# Build debug version
cargo build

# Build release version
cargo build --release
```

### Running Tests

**IMPORTANT**: All tests must be run single-threaded due to shared SQLite database state:

```bash
# Run tests (REQUIRED: single-threaded)
cargo test -- --test-threads=1

# Or use the Makefile (pre-configured for single-threaded execution)
make test

# Run with visible output
cargo test -- --test-threads=1 --nocapture
```

**Why single-threaded?**
- Tests share a common SQLite database and environment variables
- Parallel execution causes race conditions and intermittent failures
- Single-threaded execution ensures reliable, deterministic test results

### Development Commands

```bash
# Format code
make format

# Run linter
make lint

# Clean build artifacts
make clean

# Install development version
make install
```

## Community and Contributions

We welcome contributions! Please check our [Contributing Guidelines](./CONTRIBUTING.md) to get started.

**For developers**: Remember to always run tests with `--test-threads=1` to avoid database conflicts.

<!-- Badges -->
[build-image]: https://github.com/sysid/bkmr/actions/workflows/release_wheels.yml/badge.svg
[build-url]: https://github.com/sysid/bkmr/actions/workflows/release_wheels.yml
