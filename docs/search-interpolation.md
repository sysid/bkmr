# Search Interpolation Feature

The `--interpolate` flag enables template processing in search results, ensuring that dynamic content is resolved before output.

## Purpose

This feature was primarily added to support the bkmr-lsp integration, where LSP clients need to receive processed content rather than raw template strings.

## Usage

```bash
# Basic usage
bkmr search --interpolate --json -t _snip_ "current directory"

# For LSP consumption
bkmr search --json --interpolate -t _snip_ --limit 50

# Interactive mode with interpolation
bkmr search --interpolate "template snippet"
```

## How It Works

1. **Detection**: Only processes bookmarks containing `{{` or `{%` (template markers)
2. **Processing**: Uses the same interpolation engine as `bkmr open`
3. **Error Handling**: Failed interpolations fall back to original content
4. **Performance**: Efficient - only processes content that needs it

## Examples

### Shell Command Interpolation

```bash
# Add a snippet with shell interpolation
bkmr add '{{ "pwd" | shell }}' directory,shell --type snip --title "Current Directory"

# Without interpolation (raw template)
bkmr search --json "Current Directory"
# Output: {"url": "{{ \"pwd\" | shell }}", ...}

# With interpolation (processed result)
bkmr search --json --interpolate "Current Directory"
# Output: {"url": "/Users/tw", ...}
```

### Date and Environment Interpolation

```bash
# Add a snippet with date and user
bkmr add 'Report: {{ current_date | strftime("%Y-%m-%d") }} by {{ "whoami" | shell }}' reports --type snip

# Without interpolation
bkmr search --json "Report"
# Output: {"url": "Report: {{ current_date | strftime(\"%Y-%m-%d\") }} by {{ \"whoami\" | shell }}", ...}

# With interpolation
bkmr search --json --interpolate "Report"
# Output: {"url": "Report: 2025-06-17 by tw", ...}
```

### Environment Variables

```bash
# Add environment variables with templates
bkmr add 'export PROJECT_DIR={{ env("HOME", "/tmp") }}/projects' env --type env

# Search with interpolation
bkmr search --json --interpolate -t _env_
# Output: {"url": "export PROJECT_DIR=/Users/tw/projects", ...}
```

## LSP Integration

The bkmr-lsp server uses this flag to ensure consistent behavior:

```bash
# LSP calls this internally
bkmr search --json --interpolate -t _snip_ --limit 50 "metadata:prefix*"
```

This ensures that when you trigger completion in your editor, you get the processed content (e.g., `/Users/tw`) instead of the raw template (`{{ "pwd" | shell }}`).

## Error Handling

If interpolation fails (e.g., invalid shell command), the original content is preserved:

```bash
bkmr add '{{ "nonexistent_command" | shell }}' test --type snip

# With interpolation - falls back to original on error
bkmr search --json --interpolate "test"
# Output: {"url": "{{ \"nonexistent_command\" | shell }}", ...}
```

## Performance Notes

- Only bookmarks with template syntax are processed
- Uses the same efficient interpolation engine as actions
- Minimal overhead for bookmarks without templates
- Safe error handling prevents search failures

## Compatibility

- Works with all search modes: `--json`, `--fzf`, regular output
- Compatible with all tag filtering options
- Maintains backward compatibility (default: false)
- Safe to use in scripts and automation