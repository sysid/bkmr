# Configuration Options for bkmr

`bkmr` offers several configuration options to customize its behavior and appearance. This document covers the available settings and how to configure them.

## Configuration Methods

`bkmr` loads configuration in the following order of precedence (highest to lowest):

1. Command-line arguments (highest priority)
2. Environment variables
3. Custom config file (if specified with `--config`)
4. Default config file (`~/.config/bkmr/config.toml`)
5. Built-in default values (lowest priority)

## Configuration File

`bkmr` uses TOML configuration files. By default, it looks for a config file at:

```
~/.config/bkmr/config.toml
```

### Example Configuration File

```toml
# Main database path
db_url = "/path/to/your/bookmarks.db"

# FZF options
[fzf_opts]
height = "70%"
reverse = true
show_tags = true
no_url = false

# Shell script execution options
[shell_opts]
interactive = true
```

### Generating a Default Config

You can generate a default configuration file with:

```bash
bkmr --generate-config > ~/.config/bkmr/config.toml
```

### Using a Custom Config File

You can specify a custom configuration file:

```bash
bkmr --config /path/to/your/custom-config.toml search
```

## Environment Variables

### Core Settings

| Variable | Description | Default |
|----------|-------------|---------|
| `BKMR_DB_URL` | Path to SQLite database file | `~/.config/bkmr/bkmr.db` |
| `BKMR_SHELL_INTERACTIVE` | Enable/disable interactive shell editing | `true` |
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

### Example Environment Variables

Add these to your shell profile (`.bashrc`, `.zshrc`, etc.):

```bash
# Core bkmr configuration
export BKMR_DB_URL="$HOME/.local/share/bkmr/bookmarks.db"
export BKMR_SHELL_INTERACTIVE="true"  # Enable interactive shell editing (default)
export OPENAI_API_KEY="your-openai-key"  # Only if using semantic search
export EDITOR="code -w"  # Use VS Code for editing

# FZF display options
export BKMR_FZF_OPTS="--height 70% --reverse --show-tags"
```

## Shell Script Configuration

`bkmr` provides special configuration options for shell script execution behavior.

### Interactive Shell Editing

By default, shell scripts (`_shell_` content type) present an interactive editor before execution. This allows you to:

- Add parameters to the command
- Modify the script on-the-fly
- Review the command before execution

### Configuration Options

#### Via Configuration File

```toml
[shell_opts]
# Enable interactive editing (default: true)
interactive = true
```

#### Via Environment Variable

```bash
# Enable interactive mode (default)
export BKMR_SHELL_INTERACTIVE=true

# Disable interactive mode for direct execution
export BKMR_SHELL_INTERACTIVE=false
```

### Editor Features

When interactive mode is enabled, the shell editor provides:

- **Automatic editor detection**: Detects vim or emacs mode from your shell configuration
- **Readline compatibility**: Supports `.inputrc` settings
- **History persistence**: Commands saved to `~/.config/bkmr/shell_history.txt`
- **Shell integration**: Respects `$ZSH_VI_MODE`, `$BASH_VI_MODE` environment variables

### Use Cases

**Interactive Mode (Default)**: Best for development and exploration
```bash
# Execute shell bookmark - presents editor first
bkmr open 123
Execute: ./deploy.sh production
# Edit to add parameters: ./deploy.sh staging --dry-run
# Press Enter to execute
```

**Direct Mode**: Best for automation and scripts
```bash
# Execute immediately without editing
bkmr open --no-edit 123

# Execute with arguments
bkmr open --no-edit 123 -- --env staging --verbose
```

**Environment Variable Control**:
```bash
export BKMR_SHELL_INTERACTIVE=false
# All shell executions will skip interactive editing
bkmr open 123  # Runs directly without editor
```

## Command-Line Options

### Global Options

These options apply to all commands:

| Option | Description |
|--------|-------------|
| `--debug`, `-d` | Enable debug output (use multiple times for more verbosity) |
| `--openai` | Enable OpenAI integration for semantic features |
| `--config FILE`, `-c FILE` | Use a custom config file |
| `--generate-config` | Output a default configuration to stdout |

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

## Creating a Database

When running `bkmr create-db`, the command will:

1. Check for existing configuration (file or environment variables)
2. If no configuration is found, warn that default settings will be used
3. Ask for confirmation before proceeding with default database location

To specify a custom database location:

```bash
bkmr create-db /path/to/your/bookmarks.db
```

You can pre-fill the database with example entries:

```bash
bkmr create-db --pre-fill
```

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
$HOME/.config/bkmr/
├── config.toml         # Configuration file
├── bkmr.db             # Main database file (default location)
└── shell_history.txt   # Shell command history (when interactive mode is enabled)
```

If the home directory is not available, `bkmr` will fall back to:
1. Platform-specific local data directory
2. Current working directory with `.bkmr` subfolder

## Database Migrations

When upgrading to a new version, `bkmr` will automatically:

1. Check if database migrations are needed
2. Create a backup of your database with date suffix
3. Apply necessary migrations

Backups are saved in the same directory as your database with a date suffix (e.g., `bkmr_backup_20250406.db`).

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

Just make sure to point your configuration to the synchronized database location on each device, either through the config file or `BKMR_DB_URL` environment variable.
