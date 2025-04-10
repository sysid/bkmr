# bkmr

![Crates.io](https://img.shields.io/crates/v/bkmr)
![Crates.io](https://img.shields.io/crates/d/bkmr)
[![Docs.rs](https://docs.rs/bkmr/badge.svg)](https://docs.rs/bkmr)
[![Build Status][build-image]][build-url]

# Beyond Bookmarks: A CLI Knowledge Management System

[bkmr reborn](https://sysid.github.io/bkmr-reborn/)

`bkmr` is a blazing-fast, feature-rich command-line tool that extends far beyond traditional bookmark management. Store, organize, find, and interact with:

- Web URLs with automatic metadata extraction
- Code snippets for quick access and reuse
- Shell commands with execution capabilities
- Markdown documents with live rendering
- Plain text with template interpolation
- Local files and directories
- Semantic embeddings for AI-powered search

## Why bkmr?

- **Lightning-fast**: 20x faster than similar Python tools
- **Developer-focused**: Integrates seamlessly with your workflow and toolchain
- **Multifunctional**: Handles any type of content with context-aware actions
- **Intelligent**: Full-text and semantic search capabilities
- **Privacy-focused**: Local database, no cloud dependencies unless enabled

## Core Features

```bash
# Quick fuzzy search with interactive selection
bkmr search --fzf

# Advanced filtering with tags
bkmr search -t python,security "authentication"

# Add web URLs with automatic metadata
bkmr add https://example.com tag1,tag2  # title, description, etc will be loaded automatically

# Store code snippets
bkmr add "SELECT * FROM users WHERE role = 'admin'" sql,snippet --type snip

# Store shell scripts for execution
bkmr add "#!/bin/bash\necho 'System status:'\ndf -h\nfree -m" sysadmin,utils --type shell

# Store markdown documents with rendering
bkmr add "# Project Notes\n\n## Tasks\n- [ ] Complete documentation\n- [ ] Write tests" notes,project --type md

# Execute shell commands via bookmark
bkmr add "shell::find ~/projects -name '*.go' | xargs grep 'func main'" tools,search

# Semantic search with AI
bkmr --openai sem-search "containerized application security"
```
### Bookmarks
<img src="./docs/bkmr4-bookmarks.png" alt="bookmarks" width="600"/>

### Snippets
<img src="./docs/bkmr4-fzf-snippets.png" alt="fzf-snippets" width="800"/>

## Demos

See bkmr in action:

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
   brew tap sysid/bkmr
   brew info bkmr

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
| `open` | Launch or interact with stored items |
| `edit` | Modify existing items |
| `tags` | View and manage your tag taxonomy |
| `set-embeddable` | Configure items for semantic search |

## Smart Content Actions

`bkmr` intelligently handles different content types with appropriate actions:

| Content Type | Default Action | System Tag |
|--------------|----------------|------------|
| URLs | Open in browser | (none) |
| Snippets | Copy to clipboard | `_snip_` |
| Shell Scripts | Execute in terminal | `_shell_` |
| Markdown | Render and view in browser | `_md_` |
| Text Documents | Copy to clipboard | `_imported_` |
| Local Files | Open with default application | (none) |

## Advanced Features

- **Template interpolation**: Use Jinja-style templates in URLs and commands
- **Content embedding**: Store semantic representations for AI-powered search
- **Context-aware actions**: Different behaviors based on content type
- **Multiple output formats**: Terminal display, clipboard, or JSON export

For detailed documentation on advanced features:
- [Configuration Options](./docs/configuration.md)
- [Content Types](./docs/content-types.md)
- [Smart Actions](./docs/smart-actions.md)
- [Template Interpolation](./docs/template-interpolation.md)
- [Semantic Search](./docs/semantic-search.md)
- [Advanced Usage](./docs/advanced_usage.md)

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

## Community and Contributions

We welcome contributions! Please check our [Contributing Guidelines](./CONTRIBUTING.md) to get started.

<!-- Badges -->
[build-image]: https://github.com/sysid/bkmr/actions/workflows/release_wheels.yml/badge.svg
[build-url]: https://github.com/sysid/bkmr/actions/workflows/release_wheels.yml
