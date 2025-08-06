# Template Interpolation in bkmr

This document provides comprehensive information about template interpolation in bkmr, including when it's applied automatically and when manual flags are needed.

## Overview

bkmr uses Jinja2-style template interpolation to create dynamic content that adapts to runtime context. Template variables are processed using `{{` and `{%` syntax patterns and can include dates, environment variables, shell command outputs, and custom filters.

## Automatic vs Manual Interpolation

### Automatic Interpolation (No Configuration Needed)

Template interpolation is **automatically applied** in the following scenarios:

#### 1. FZF Mode - Always Interpolated

When using fuzzy finder mode, interpolation happens automatically:

```bash
# All of these automatically show interpolated content
bkmr search --fzf
bkmr search --fzf -t _snip_
bkmr search --fzf "deployment"
```

**FZF Features:**
- **Preview pane**: Shows resolved template variables
- **CTRL-O (copy)**: Copies interpolated content to clipboard
- **Enhanced display**: Renders dynamic values in real-time

#### 2. All Bookmark Actions - Always Interpolated

When executing any bookmark action, interpolation is applied automatically:

```bash
# These always interpolate before execution
bkmr open <id>           # URI: Opens interpolated URLs
bkmr open <id>           # Snippet: Copies interpolated content  
bkmr open <id>           # Shell: Executes interpolated scripts
bkmr open <id>           # Env: Sources interpolated variables
```

**Action-Specific Behavior:**
- **URI Action**: URLs rendered before opening in browser
- **Snippet Action**: Content interpolated before copying to clipboard
- **Shell Action**: Scripts interpolated before execution
- **Environment Action**: Variables interpolated before output

#### 3. Clipboard Operations - Always Interpolated

Clipboard operations automatically apply interpolation:

```bash
# Copy operations always interpolate
bkmr yank <id>                    # Copies interpolated content
# FZF CTRL-O also interpolates
```

### Manual Interpolation (Flag Required)

The `--interpolate` flag is **only needed** for regular search result display:

```bash
# Without flag - shows raw template syntax
bkmr search "deploy"
# Output: deploy-{{ current_date | strftime('%Y-%m-%d') }}.sh

# With flag - shows resolved values
bkmr search --interpolate "deploy" 
# Output: deploy-2025-07-05.sh
```

**When to Use `--interpolate`:**
- When you want to see resolved template values in search results list
- When using regular (non-FZF) search mode
- For debugging template expressions before using bookmarks

**When NOT to Use `--interpolate`:**
- ❌ FZF mode (automatic)
- ❌ Bookmark actions like `open` (automatic)  
- ❌ Clipboard operations (automatic)

## Content Type Interpolation Support

| Content Type | Supports Interpolation | Notes |
|--------------|----------------------|-------|
| URLs | ✅ Yes | Always interpolated during actions |
| Snippets (`_snip_`) | ✅ Yes | Interpolated when copied |
| Shell Scripts (`_shell_`) | ✅ Yes | Interpolated before execution |
| Environment Variables (`_env_`) | ✅ Yes | Interpolated when sourced |
| Markdown (`_md_`) | ❌ No | Disabled to avoid syntax conflicts |
| Text Documents (`_imported_`) | ✅ Yes | Interpolated when copied |

**Note**: Markdown content does not support interpolation to prevent conflicts with legitimate markdown syntax that may contain `{%}` patterns (e.g., code documentation).

## Template Syntax and Variables

### Available Variables

#### Date and Time
```bash
{{ current_date }}                           # Current datetime object
{{ current_date | strftime('%Y-%m-%d') }}    # 2025-07-05
{{ current_date | strftime('%B %d, %Y') }}   # July 05, 2025
{{ current_date | strftime('%H:%M:%S') }}    # 14:30:15
{{ current_date | subtract_days(7) }}        # 7 days ago
{{ current_date | add_days(30) }}            # 30 days from now
```

#### Environment Variables
```bash
{{ env('HOME') }}                            # Environment variable
{{ env('API_KEY', 'default-key') }}          # With fallback value
{{ env('USER') }}                            # Current username
{{ env('PATH') }}                            # System PATH
```

#### Shell Command Execution
```bash
{{ "whoami" | shell }}                       # Current username
{{ "git branch --show-current" | shell }}   # Current git branch
{{ "hostname" | shell }}                     # System hostname
{{ "pwd" | shell }}                          # Current directory
{{ "date '+%Y%m%d'" | shell }}              # Date via shell command
```

#### String Manipulation
```bash
{{ "hello world" | upper }}                  # HELLO WORLD
{{ "HELLO WORLD" | lower }}                  # hello world
{{ "hello world" | title }}                  # Hello World
{{ "  spaced  " | trim }}                    # spaced
```

### Template Examples

#### Dynamic URLs
```bash
# Daily reports
bkmr add "https://reports.company.com/{{ current_date | strftime('%Y/%m/%d') }}/summary" reports

# API endpoints with timestamps
bkmr add "https://api.example.com/data?timestamp={{ current_date | strftime('%s') }}" api

# User-specific URLs
bkmr add "https://dashboard.company.com/users/{{ env('USER') }}" dashboard
```

#### Dynamic Shell Scripts
```bash
# Backup with timestamp
bkmr add "#!/bin/bash
echo 'Starting backup at {{ current_date | strftime('%Y-%m-%d %H:%M:%S') }}'
tar -czf backup-{{ current_date | strftime('%Y%m%d_%H%M%S') }}.tar.gz /data/
echo 'Backup completed on {{ \"hostname\" | shell }}'" backup --type shell

# Git workflow
bkmr add "#!/bin/bash
echo 'Current branch: {{ \"git branch --show-current\" | shell }}'
echo 'Current user: {{ env('USER') }}'
git commit -m 'Auto-commit {{ current_date | strftime('%Y-%m-%d') }}'" git-commit --type shell
```

#### Environment Variables with Context
```bash
# Development environment
bkmr add "export PROJECT_ROOT={{ env('HOME') }}/projects
export BUILD_TIME={{ current_date | strftime('%Y%m%d_%H%M%S') }}
export GIT_BRANCH={{ \"git branch --show-current\" | shell }}
export HOSTNAME={{ \"hostname\" | shell }}" dev-env --type env

# Deployment environment
bkmr add "export DEPLOY_DATE={{ current_date | strftime('%Y-%m-%d') }}
export DEPLOY_USER={{ env('USER') }}
export DEPLOY_HOST={{ \"hostname\" | shell }}" deploy-env --type env
```

#### Code Snippets with Dynamic Values
```bash
# SQL with timestamps
bkmr add "SELECT * FROM logs WHERE created_at >= '{{ current_date | subtract_days(7) | strftime('%Y-%m-%d') }}'" sql-recent --type snip

# Configuration with user context
bkmr add "{
  \"user\": \"{{ env('USER') }}\",
  \"timestamp\": \"{{ current_date | strftime('%Y-%m-%dT%H:%M:%SZ') }}\",
  \"hostname\": \"{{ \"hostname\" | shell }}\"
}" config-json --type snip
```

## Implementation Architecture

### Core Pattern: `render_if_needed()`

bkmr uses a consistent pattern for interpolation via `InterpolationHelper::render_if_needed()`:

```rust
// Only applies interpolation if template syntax is detected
if content.contains("{{") || content.contains("{%") {
    // Apply Jinja2 template rendering
    service.render_bookmark_url(bookmark)
} else {
    // Return content unchanged
    Ok(content.to_string())
}
```

This ensures:
- ✅ Performance: No processing overhead for non-template content
- ✅ Safety: Regular content is never accidentally modified
- ✅ Consistency: Same logic across all content types

### Error Handling

Template interpolation errors are handled gracefully:

- **Search display**: Warnings logged, original content preserved
- **Action execution**: Errors reported, action may fail safely
- **FZF mode**: Fallback to original content if rendering fails

## Usage Patterns and Best Practices

### 1. Search and Discovery

```bash
# See raw templates in search results
bkmr search "backup"
# Output: backup-{{ current_date | strftime('%Y%m%d') }}.sh

# See resolved templates in search results  
bkmr search --interpolate "backup"
# Output: backup-20250705.sh

# FZF automatically shows resolved values
bkmr search --fzf "backup"
# Preview shows: backup-20250705.sh
```

### 2. Development Workflow

```bash
# Store dynamic API endpoints
bkmr add "https://api.dev.company.com/{{ env('USER') }}/data" api-dev
bkmr add "https://api.prod.company.com/data?key={{ env('API_KEY') }}" api-prod

# Quick access with automatic interpolation
bkmr search --fzf "api"  # Shows resolved URLs
bkmr open <id>          # Opens interpolated URL
```

### 3. Script Management

```bash
# Store parameterized scripts
bkmr add "#!/bin/bash
echo 'Deploying to {{ env('ENVIRONMENT', 'development') }} at {{ current_date }}'
kubectl apply -f deployment-{{ env('ENVIRONMENT', 'dev') }}.yaml" deploy --type shell

# Execute with automatic interpolation
bkmr open <id>  # Script runs with resolved variables
```

### 4. Environment Management

```bash
# Context-aware environment setup
bkmr add "export WORKSPACE={{ env('HOME') }}/workspaces/{{ \"git branch --show-current\" | shell }}
export BUILD_DATE={{ current_date | strftime('%Y%m%d') }}
export HOSTNAME={{ \"hostname\" | shell }}" workspace-env --type env

# Source with automatic interpolation
eval "$(bkmr open <id>)"  # Variables resolved before sourcing
```

## Troubleshooting

### Common Issues

**1. Template Syntax Conflicts with Other Tools**

bkmr detects any `{{` or `{%` patterns and attempts Jinja2 template processing, which can cause conflicts with other templating systems:

```bash
# ❌ Problem: GitHub CLI Go templates conflict with bkmr Jinja2
gh run list --template '{{range .}}{{.name}}{{end}}'
# Error: Template syntax error: unexpected end of variable block

# ✅ Solution: Dynamic template construction to avoid {{ pattern detection
OPEN_BRACE='{'
CLOSE_BRACE='}'
TEMPLATE="${OPEN_BRACE}${OPEN_BRACE}range .${CLOSE_BRACE}${CLOSE_BRACE}..."
gh run list --template "$TEMPLATE"

# ❌ Problem: Docker Compose variable conflicts
version: '3.8'
services:
  app:
    image: myapp:{{.Tag}}  # Conflicts with bkmr template detection

# ✅ Solution: Escape or construct dynamically
TAG_VAR='{{.Tag}}'
# Or use different quoting/escaping strategies
```

**Common Tools with Template Syntax Conflicts:**
- GitHub CLI (`gh`) - Uses Go templates with `{{` syntax
- Docker Compose - Uses Go templates in some contexts  
- Helm charts - Uses Go templates extensively
- Kubernetes manifests - May contain Go template syntax
- Other CLIs using Go's `text/template` package

**2. Template Not Rendering**
```bash
# Check raw content first
bkmr search "template-name"

# Verify template syntax
# ✅ Correct: {{ current_date }}
# ❌ Wrong:   { current_date }
# ❌ Wrong:   {{current_date}}  (missing spaces)
```

**3. Environment Variable Not Found**
```bash
# Use fallback values
{{ env('MISSING_VAR', 'default-value') }}

# Check if variable exists
echo $VARIABLE_NAME
```

**4. Shell Command Failures**
```bash
# Test shell commands separately
{{ "nonexistent-command" | shell }}  # May cause errors

# Use safe commands with error handling
{{ "which git && git branch --show-current || echo 'no-git'" | shell }}
```

**5. Date Format Issues**
```bash
# Use standard strftime formats
{{ current_date | strftime('%Y-%m-%d') }}  # ✅ Good
{{ current_date | strftime('%x') }}        # ❌ May vary by locale
```

### Debugging Templates

**1. Test in Search Results:**
```bash
# See raw template
bkmr search "template-bookmark"

# See rendered result
bkmr search --interpolate "template-bookmark"
```

**2. Use FZF Preview:**
```bash
# FZF shows resolved values in preview
bkmr search --fzf "template-bookmark"
```

**3. Check Logs:**
```bash
# Enable debug logging
RUST_LOG=debug bkmr open <id>
```

## Security Considerations

### Shell Command Injection

Be cautious with shell commands in templates:

```bash
# ✅ Safe: Static commands
{{ "hostname" | shell }}
{{ "date" | shell }}

# ⚠️  Risk: Dynamic input (avoid user-controlled values)
{{ env('USER_INPUT') | shell }}  # Could be dangerous

# ✅ Better: Validate or escape input
{{ env('USER_INPUT', 'safe-default') }}
```

### Environment Variable Exposure

```bash
# ✅ Safe: Non-sensitive variables
{{ env('USER') }}
{{ env('HOME') }}

# ⚠️  Risk: Sensitive data
{{ env('API_SECRET') }}      # May expose secrets in logs/output
{{ env('DATABASE_PASSWORD') }}

# ✅ Better: Use secure secret management
# Store sensitive data separately, reference by ID
```

### Template Complexity

```bash
# ✅ Simple: Easy to understand and debug
{{ current_date | strftime('%Y-%m-%d') }}

# ⚠️  Complex: Harder to debug and maintain
{{ env('BASE_URL', 'https://default.com') }}/{{ env('API_VERSION', 'v1') }}/{{ env('USER') | lower }}/data?t={{ current_date | strftime('%s') }}

# ✅ Better: Break into multiple simpler templates or use shell scripts
```

## Performance Notes

- **Lazy Evaluation**: Templates only processed when `{{` or `{%` patterns detected
- **Caching**: Template engine reuses parsed templates where possible
- **Shell Commands**: Executed fresh each time (no caching for dynamic values)
- **Environment Variables**: Read from current environment each time

For high-frequency usage, consider pre-rendering static values or using shell scripts for complex logic.