# Configuration Options for bkmr

`bkmr` offers several configuration options to customize its behavior and appearance. This document covers the available settings and how to use them.

## Environment Variables

### Core Settings

| Variable | Description | Default |
|----------|-------------|---------|
| `BKMR_DB_URL` | Path to SQLite database file | `../db/bkmr.db` |
| `OPENAI_API_KEY` | API key for OpenAI (needed for semantic search) | None |
| `EDITOR` | Text editor for editing bookmarks | `vim` |

### FZF Interface Settings

These settings control the appearance and behavior of the fuzzy finder interface:

| Variable | Description | Default Format | Example |
|----------|-------------|----------------|---------|
| `BKMR_FZF_OPTS` | All FZF options in a single string | | `--height 70% --reverse --show-tags` |

Individual FZF options can include:

- `--height VALUE`: Height of the FZF window (e.g., `50%`)
- `--reverse`: Display results in reverse order
- `--show-tags`: Display tags in the result list
- `--no-url`: Hide URLs in the result list

### Example Configuration

Add these to your shell profile (`.bashrc`, `.zshrc`, etc.):

```bash
# Core bkmr configuration
export BKMR_DB_URL="$HOME/.local/share/bkmr/bookmarks.db"
export OPENAI_API_KEY="your-openai-key"  # Only if using semantic search
export EDITOR="code -w"  # Use VS Code for editing

# FZF display options
export BKMR_FZF_OPTS="--height 70% --reverse --show-tags"
```

## Command-Line Options

### Global Options

These options apply to all commands:

| Option | Description |
|--------|-------------|
| `--debug`, `-d` | Enable debug output (use multiple times for more verbosity) |
| `--openai` | Enable OpenAI integration for semantic features |
| `--config FILE` | Use a custom config file |

### Command-Specific Options

Many commands have specific options. Here are some common ones:

#### Search Options

| Option | Description | Example |
|--------|-------------|---------|
| `-t, --tags` | Filter by all tags | `--tags python,web` |
| `-n, --ntags` | Filter by any tag | `--ntags api,frontend` |
| `-T, --Tags` | Exclude all tags | `--Tags deprecated,old` |
| `-N, --Ntags` | Exclude any tag | `--Ntags experimental,draft` |
| `-o, --descending` | Order by age, descending | |
| `-O, --ascending` | Order by age, ascending | |
| `-l, --limit` | Limit number of results | `--limit 10` |
| `--fzf` | Use fuzzy finder interface | |
| `--json` | Output results as JSON | |

## Enhanced FZF Experience

The FZF interface has keyboard shortcuts for common actions:

| Key | Action |
|-----|--------|
| `Enter` | Open selected bookmark |
| `Ctrl-O` | Open selected bookmark and record access |
| `Ctrl-Y` | Copy URL to clipboard |
| `Ctrl-E` | Edit selected bookmark |
| `Ctrl-D` | Delete selected bookmark |
| `Esc` | Quit FZF interface |

## Application Directory Structure

By default, `bkmr` uses the following directory structure:

```
$HOME/.local/share/bkmr/
├── bookmarks.db    # Main database file
└── backups/        # Database backups (created during migrations)
```

## Database Migrations

When upgrading to a new version, `bkmr` will automatically:

1. Check if database migrations are needed
2. Create a backup of your database with date suffix
3. Apply necessary migrations

You can disable automatic migrations by setting `BKMR_NO_AUTO_MIGRATE=1` if you prefer to manage migrations manually.

## Advanced Configuration

### Custom Shell Integration

For optimal workflow integration, add these functions to your shell profile:

```bash
# Quick bookmark search
bk() {
    bkmr search --fzf "$@"
}

# Quick bookmark addition
bka() {
    bkmr add "$@"
}

# Search and immediately open the first result
bko() {
    local ids=$(bkmr search "$@" --np)
    if [[ -n "$ids" ]]; then
        bkmr open "$ids"
    fi
}
```

### Using bkmr with tmux

When using `bkmr` in tmux, you may want to ensure the FZF preview window works correctly:

```bash
export BKMR_FZF_OPTS="--height 80% --reverse --preview-window=right:60%"
```

### Sync Between Devices

Since `bkmr` uses a SQLite database, you can sync between devices using:

- Git (recommended for version tracking)
- Syncthing
- Dropbox, Google Drive, etc.

Just make sure to point `BKMR_DB_URL` to the synchronized location on each device.