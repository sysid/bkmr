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

### NEW: Editor Integrations!

- **Built-in LSP server**: Use `bkmr lsp` for VS Code, Vim, Emacs - automatic snippet completion with language-aware filtering
- **[Neovim Plugin](https://github.com/sysid/bkmr-nvim)**: Visual interface with Telescope integration and zero configuration
- **[IntelliJ Plugin](https://github.com/sysid/bkmr-intellij-plugin)**: JetBrains Marketplace plugin for all IDEs

See **[Editor Integration](https://github.com/sysid/bkmr/wiki/Editor-Integration)** for complete documentation.

## Quick Examples

```bash
# Quick fuzzy search with interactive selection
bkmr search --fzf

# Add URL with automatic metadata extraction
bkmr add https://example.com tag1,tag2

# Store code snippet
bkmr add "SELECT * FROM users" sql,_snip_ --title "User Query"

# Shell script with interactive execution
bkmr add "#!/bin/bash\necho 'Hello'" utils,_shell_ --title "Greeting"

# Render markdown in browser with TOC
bkmr add "# Notes\n## Section 1" docs,_md_ --title "Project Notes"

# Import files with frontmatter
bkmr import-files ~/scripts/ --base-path SCRIPTS_HOME

# AI-powered semantic search
bkmr --openai sem-search "containerized application security"
```

### Screenshots

**Bookmarks:**
<img src="./docs/bkmr4-bookmarks.png" alt="bookmarks" width="800"/>

**Snippets:**
<img src="./docs/bkmr4-fzf-snippets.png" alt="fzf-snippets" width="800"/>

**Demos:**
- <a href="https://asciinema.org/a/q4Okf4j2ja757Nav5wf0tNz8k?autoplay=1" alt="Never Context-Switch Again"><img src="https://asciinema.org/a/q4Okf4j2ja757Nav5wf0tNz8k.svg" /></a>
- [Overview](https://asciinema.org/a/VTsHuw1Ugsbo10EP0tZ3PdpoG?autoplay=1&speed=2&t=3) | [Getting Started](https://asciinema.org/a/wpnsTw3Cl7DK2R7jK7WVpp9OR?autoplay=1&speed=2&t=3) | [Search and Filter](https://asciinema.org/a/M97UJMKxw1nxnzO4SaowGZAmb?autoplay=1&speed=2&t=3) | [Edit and Update](https://asciinema.org/a/uCuNPSlqRemlcXiVQ3CIqq8uV?autoplay=1&speed=2&t=3) | [Tag Management](https://asciinema.org/a/jNOLfhc6aFV3wPGTgOzgrM7Kc?autoplay=1&speed=2&t=3)

## Getting Started

### Installation

```bash
# Via cargo
cargo install bkmr

# Via pip/pipx/uv
pip install bkmr

# Via brew
brew install bkmr
```

See **[Installation Guide](https://github.com/sysid/bkmr/wiki/Installation)** for detailed instructions and troubleshooting.

### Initial Setup

```bash
# Generate configuration
bkmr --generate-config > ~/.config/bkmr/config.toml

# Create database
bkmr create-db ~/.config/bkmr/bkmr.db

# Optional: Configure location
export BKMR_DB_URL=~/path/to/db
```

### First Use

```bash
# Add your first bookmark
bkmr add https://github.com/yourusername/yourrepo github,project

# Search and find
bkmr search github

# Interactive fuzzy search
bkmr search --fzf
```

**Quick Start Guide**: See the **[Quick Start](https://github.com/sysid/bkmr/wiki/Quick-Start)** for a 5-minute tutorial.

## Command Reference

| Command | Description |
|---------|-------------|
| `search` | Search across all content with full-text and tag filtering |
| `sem-search` | AI-powered semantic search using OpenAI embeddings |
| `add` | Add new content (URLs, snippets, files, shell commands, etc.) |
| `open` | Launch or interact with stored items (supports script arguments) |
| `edit` | Smart editing: auto-detects file-imported bookmarks |
| `import-files` | Import files/directories with frontmatter parsing |
| `tags` | View and manage your tag taxonomy |
| `set-embeddable` | Configure items for semantic search |

**Complete command documentation**: See **[Basic Usage](https://github.com/sysid/bkmr/wiki/Basic-Usage)** for detailed examples.

## Smart Content Actions

bkmr intelligently handles different content types with appropriate actions:

| Content Type          | Default Action                        | System Tag   |
|-----------------------|---------------------------------------|--------------|
| URLs                  | Open in browser                       | (none)       |
| Snippets              | Copy to clipboard                     | `_snip_`     |
| Shell Scripts         | Interactive edit then execute         | `_shell_`    |
| Environment Variables | Print for sourcing in shell           | `_env_`      |
| Markdown              | Render in browser with TOC            | `_md_`       |
| Text Documents        | Copy to clipboard                     | `_imported_` |
| Local Files           | Open with default application         | (none)       |

Learn more: **[Content Types](https://github.com/sysid/bkmr/wiki/Content-Types)** | **[Core Concepts](https://github.com/sysid/bkmr/wiki/Core-Concepts)**

## Documentation

Comprehensive documentation is available in the **[bkmr Wiki](https://github.com/sysid/bkmr/wiki)**:

### Getting Started
- **[Home](https://github.com/sysid/bkmr/wiki/Home)** - Wiki overview and navigation
- **[Quick Start](https://github.com/sysid/bkmr/wiki/Quick-Start)** - 5-minute introduction
- **[Installation](https://github.com/sysid/bkmr/wiki/Installation)** - Installation methods and troubleshooting
- **[Core Concepts](https://github.com/sysid/bkmr/wiki/Core-Concepts)** - Understanding tags, system tags, and bookmarks

### Core Features
- **[Basic Usage](https://github.com/sysid/bkmr/wiki/Basic-Usage)** - Common daily operations
- **[Search and Discovery](https://github.com/sysid/bkmr/wiki/Search-and-Discovery)** - FTS, tags, fuzzy finder, semantic search
- **[Content Types](https://github.com/sysid/bkmr/wiki/Content-Types)** - URLs, snippets, shell scripts, markdown, environment variables
- **[Shell Scripts](https://github.com/sysid/bkmr/wiki/Shell-Scripts)** - Interactive execution and shell function stubs

### Advanced Topics
- **[Configuration](https://github.com/sysid/bkmr/wiki/Configuration)** - Complete configuration reference
- **[Template Interpolation](https://github.com/sysid/bkmr/wiki/Template-Interpolation)** - Jinja2 dynamic content
- **[File Import and Editing](https://github.com/sysid/bkmr/wiki/File-Import-and-Editing)** - Frontmatter, base paths, smart editing
- **[Semantic Search](https://github.com/sysid/bkmr/wiki/Semantic-Search)** - OpenAI-powered AI search
- **[Editor Integration](https://github.com/sysid/bkmr/wiki/Editor-Integration)** - LSP server and editor plugins
- **[Advanced Workflows](https://github.com/sysid/bkmr/wiki/Advanced-Workflows)** - Power user techniques

### Reference
- **[Troubleshooting](https://github.com/sysid/bkmr/wiki/Troubleshooting)** - Common issues and solutions
- **[Development](https://github.com/sysid/bkmr/wiki/Development)** - Contributing and building from source

## Editor Integrations

Access your snippets directly within your editor without context switching.

### Neovim Plugin (Recommended)

**[bkmr-nvim](https://github.com/sysid/bkmr-nvim)** provides visual interface with zero configuration.

```lua
{
  "sysid/bkmr-nvim",
  dependencies = { "nvim-lua/plenary.nvim" },
  config = function()
    require("bkmr").setup() -- Zero config required!
  end,
}
```

**Features**: Visual snippet browser, in-editor editing, automatic LSP setup, custom commands

### Built-in LSP Server

Compatible with VS Code, Vim, Emacs, Sublime, and any LSP-compatible editor.

```bash
# Start LSP server
bkmr lsp

# Disable template interpolation if needed
bkmr lsp --no-interpolation
```

**Features**: Automatic completion, language-aware filtering, universal snippets, template interpolation

### IntelliJ Platform Plugin

**[bkmr-intellij-plugin](https://github.com/sysid/bkmr-intellij-plugin)** for all JetBrains IDEs.

**Features**: Seamless LSP integration, Tab navigation, works in IntelliJ IDEA, PyCharm, WebStorm, CLion, RustRover, and all JetBrains IDEs

**Complete documentation**: **[Editor Integration](https://github.com/sysid/bkmr/wiki/Editor-Integration)**

## Platform Compatibility

**Wayland Support**: Native Wayland clipboard support for modern Linux desktops.
- **Supported compositors**: Hyprland, Sway, and other compositors supporting `wlr-data-control-unstable-v1`
- **Automatic detection**: Falls back to X11/XWayland if unavailable
- **Compatibility**: Check your compositor at [wayland.app](https://wayland.app)

## Development

### Building from Source

```bash
git clone https://github.com/sysid/bkmr.git
cd bkmr
cargo build --release
```

### Running Tests

**IMPORTANT**: All tests must be run single-threaded:

```bash
# Run tests (REQUIRED: single-threaded)
cargo test -- --test-threads=1

# Or use Makefile
make test
```

**Why single-threaded?** Tests share SQLite database and environment variables. Parallel execution causes race conditions.

See **[Development](https://github.com/sysid/bkmr/wiki/Development)** for complete contributor guide.

## Community and Contributions

We welcome contributions! Please check our [Contributing Guidelines](./CONTRIBUTING.md) to get started.

**Resources:**
- GitHub: https://github.com/sysid/bkmr
- Issues: https://github.com/sysid/bkmr/issues
- Wiki: https://github.com/sysid/bkmr/wiki
- Discussions: https://github.com/sysid/bkmr/discussions

**For developers**: Remember to always run tests with `--test-threads=1` to avoid database conflicts.

<!-- Badges -->
[build-image]: https://github.com/sysid/bkmr/actions/workflows/release_wheels.yml/badge.svg
[build-url]: https://github.com/sysid/bkmr/actions/workflows/release_wheels.yml
