# File Import and Smart Editing

This document describes the file import system and smart editing capabilities in bkmr, which allow you to import files with structured metadata and seamlessly edit both source files and database content.

## Overview

The file import system enables you to:
- Import files with frontmatter metadata (YAML or hash-style comments)
- Use base path configuration for portable path storage
- Automatically track file changes with SHA-256 hashing
- Edit source files directly with automatic database synchronization

## File Import System

### Supported File Types

- **Shell scripts** (`.sh`): Executable scripts with metadata
- **Python scripts** (`.py`): Python files with metadata
- **Markdown files** (`.md`): Documentation with metadata

### Frontmatter Formats

#### YAML Frontmatter
```yaml
---
name: "Database Backup Script"
tags: ["database", "backup", "automation"]
type: "_shell_"
---
#!/bin/bash
pg_dump mydb > backup.sql
```

#### Hash-Style Frontmatter
```bash
#!/bin/bash
# name: Database Backup Script
# tags: database, backup, automation
# type: _shell_
pg_dump mydb > backup.sql
```

### Metadata Fields

- **name** (required): The title/name of the bookmark
- **tags** (optional): Comma-separated list of tags
- **type** (optional): Content type (_shell_, _snip_, _md_, _env_)

## Base Path Configuration

Base paths allow you to store portable file references that work across different environments.

### Configuration

Add base paths to your `~/.config/bkmr/config.toml`:

```toml
[base_paths]
SCRIPTS_HOME = "$HOME/scripts"
DOCS_HOME = "$HOME/documents"
WORK_SCRIPTS = "/work/automation/scripts"
PROJECT_NOTES = "$HOME/projects/notes"
```

### Environment Variable Support

Base paths support environment variable expansion:
- `$HOME` - User home directory
- `$USER` - Current username
- Any other environment variables

### Usage Examples

```bash
# Import with base path (stores as $SCRIPTS_HOME/backup/daily.sh)
bkmr import-files backup/daily.sh --base-path SCRIPTS_HOME

# Import without base path (stores absolute path)
bkmr import-files ~/scripts/backup/daily.sh

# Import multiple files with base path
bkmr import-files scripts/ docs/ --base-path WORK_SCRIPTS

# Update existing bookmarks when files change
bkmr import-files scripts/ --base-path SCRIPTS_HOME --update

# Dry run to see what would be imported
bkmr import-files scripts/ --base-path SCRIPTS_HOME --dry-run
```

## Smart Editing System

The smart editing system automatically detects whether a bookmark was imported from a file and provides appropriate editing behavior.

### Automatic Detection

When you run `bkmr edit <id>`, the system:
1. Checks if the bookmark has a `file_path` field
2. If yes → Opens the source file in your `$EDITOR`
3. If no → Opens the database content editor

### Source File Editing

For file-imported bookmarks:
- Opens the original source file in your `$EDITOR` (vim, nano, code, etc.)
- Preserves file structure and formatting
- Allows editing of both content and frontmatter
- Automatically syncs changes back to the database

### Database Synchronization

After editing a source file, the system:
- Re-parses the frontmatter metadata
- Updates the bookmark's title, tags, and content type
- Syncs the cleaned content (without frontmatter) to the database
- Updates file tracking information (modification time, hash)

### Override Options

```bash
# Smart editing (default)
bkmr edit 123

# Force database content editing
bkmr edit 123 --force-db

# Works with fuzzy finder too
bkmr search --fzf  # CTRL-E uses smart editing
```

## Advanced Features

### Incremental Updates

The system tracks file changes using SHA-256 hashes:
- Only processes files that have actually changed
- Efficient re-imports for large directories
- Preserves file relationships and metadata

### Path Resolution

The system intelligently resolves file paths:
- Expands base path variables (`$SCRIPTS_HOME` → `/home/user/scripts`)
- Resolves environment variables (`$HOME`, `$USER`)
- Handles both absolute and relative paths

### Error Handling

Robust error handling for common scenarios:
- Missing source files (falls back to database editing)
- Invalid frontmatter (logs warnings, continues processing)
- Base path validation (checks existence before import)
- Editor failures (provides helpful error messages)

## Integration with Existing Features

### Content Types

File-imported bookmarks integrate seamlessly with bkmr's content type system:
- Shell scripts execute with interactive editing
- Markdown files render in browser
- Snippets copy to clipboard
- Environment variables print for sourcing

### Search and Tags

File-imported bookmarks are fully searchable:
- Full-text search includes file content
- Tag filtering works with frontmatter tags
- Semantic search (if enabled) includes file content

### Fuzzy Finder

The fuzzy finder (--fzf) supports smart editing:
- CTRL-E triggers smart editing
- Automatically detects file-imported bookmarks
- Provides consistent editing experience

## Configuration Examples

### Complete Configuration

```toml
# ~/.config/bkmr/config.toml
db_url = "/Users/username/.config/bkmr/bkmr.db"

[fzf_opts]
height = "50%"
reverse = false
show_tags = false

[shell_opts]
interactive = true

[base_paths]
SCRIPTS_HOME = "$HOME/scripts"
DOCS_HOME = "$HOME/documents"
WORK_SCRIPTS = "/work/automation/scripts"
PROJECT_NOTES = "$HOME/projects/notes"
DOTFILES = "$HOME/.config"
```

### Workflow Example

```bash
# 1. Configure base paths
bkmr --generate-config > ~/.config/bkmr/config.toml
# Edit config.toml to add base_paths

# 2. Import your scripts directory
bkmr import-files scripts/ --base-path SCRIPTS_HOME

# 3. Search and edit
bkmr search --fzf -t _shell_
# Select a script and press CTRL-E to edit the source file

# 4. Update imports when files change
bkmr import-files scripts/ --base-path SCRIPTS_HOME --update
```

## Troubleshooting

### Common Issues

**Base path not found**: Ensure the base path is defined in your config.toml
```bash
Error: Base path 'SCRIPTS_HOME' not found in configuration
Add it to your config.toml under [base_paths]
```

**Source file missing**: The system falls back to database editing
```bash
Source file does not exist: /path/to/file.sh
Falling back to database content editing...
```

**Editor not found**: Set your EDITOR environment variable
```bash
export EDITOR=vim  # or nano, code, etc.
```

### Best Practices

1. **Use descriptive base path names**: `SCRIPTS_HOME` vs `S1`
2. **Keep frontmatter minimal**: Only include necessary metadata
3. **Use consistent naming**: Follow a naming convention for files
4. **Regular updates**: Run import with `--update` when files change
5. **Backup before bulk changes**: Create database backups before large imports

## See Also

- [Configuration Options](./configuration.md)
- [Content Types](./content-types.md)
- [Smart Actions](./smart-actions.md)
- [Advanced Usage](./advanced_usage.md)