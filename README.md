# bkmr

![Crates.io](https://img.shields.io/crates/v/bkmr)
![Crates.io](https://img.shields.io/crates/d/bkmr)
[![Docs.rs](https://docs.rs/bkmr/badge.svg)](https://docs.rs/bkmr)
[![Build Status][build-image]][build-url]

# Beyond Bookmarks: A CLI Knowledge Management System

[bkmr reborn](https://sysid.github.io/bkmr-reborn/)

`bkmr` is a blazing-fast, feature-rich command-line tool that extends far beyond traditional bookmark management. Store, organize, find, and interact with:

- Web URLs with automatic metadata extraction
- Snippets of code or text
- Local files and directories
- Shell commands for automation
- Documentation with embeddings for semantic search

## Why bkmr?

- **Lightning-fast**: 20x faster than similar Python tools
- **Intuitive**: Built for developer workflows with powerful search
- **Flexible**: Handle any type of content — not just web URLs
- **Intelligent**: Full-text and semantic search capabilities
- **Privacy-focused**: Local database, no cloud dependencies

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
| `add` | Add new content (URLs, snippets, files, shell commands) |
| `open` | Launch or interact with stored items |
| `edit` | Modify existing items |
| `tags` | View and manage your tag taxonomy |

## Advanced Features

- **Template interpolation**: Use Jinja-style templates in URLs and commands
- **Content embedding**: Store semantic representations for AI-powered search
- **Custom actions**: Configure custom behaviors for different content types
- **Multiple output formats**: Terminal display, clipboard, or JSON export

For detailed documentation on advanced features:
- [Configuration Options](./docs/configuration.md)
- [Content Types](./docs/content-types.md)
- [Template Interpolation](./docs/template-interpolation.md)
- [Advanced Usage](./docs/advanced_usage.md)

## Workflow Integration

`bkmr` shines as the central hub for your technical knowledge and daily workflow:

1. **Store information once, find it instantly** - Never lose important URLs, commands, or snippets
2. **Reduce context switching** - Launch applications, files, and commands directly from search
3. **Build a personal knowledge base** - Accumulate and organize technical references
4. **Automate repetitive tasks** - Turn complex command sequences into simple bookmarks

## Upgrading from Previous Versions

If you're upgrading from a previous version, `bkmr` will automatically handle database migration to add support for newer features.

## Community and Contributions

We welcome contributions! Please check our [Contributing Guidelines](./CONTRIBUTING.md) to get started.

<!-- Badges -->
[build-image]: https://github.com/sysid/bkmr/actions/workflows/release_wheels.yml/badge.svg
[build-url]: https://github.com/sysid/bkmr/actions/workflows/release_wheels.yml
