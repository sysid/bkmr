# Content Types and Actions

bkmr handles different content types with intelligent, context-aware actions. This guide covers all supported content types and their behaviors.

## Content Type Overview

| Type | System Tag | Action | Use Case |
|------|------------|--------|----------|
| URLs | (none) | Open in browser | Web bookmarks, documentation links |
| Code Snippets | `_snip_` | Copy to clipboard | Reusable code fragments |
| Shell Scripts | `_shell_` | Interactive edit + execute | Automation, commands |
| Markdown | `_md_` | Render in browser | Documentation, notes |
| Environment Variables | `_env_` | Print for sourcing | Environment setup |
| Text Documents | `_imported_` | Copy to clipboard | Plain text content |

## URLs and Web Resources

**Default Action:** Open in default browser

```bash
# Simple URL
bkmr add https://github.com/sysid/bkmr rust,cli

# Dynamic URL with template interpolation
bkmr add "https://api.example.com/daily/{{ current_date | strftime('%Y-%m-%d') }}" api,reports
```

**Features:**
- Automatic metadata extraction (title, description)
- Template interpolation support
- Support for any URL scheme (http, https, ftp, etc.)

## Code Snippets (`_snip_`)

**Default Action:** Copy to clipboard

```bash
# JavaScript snippet
bkmr add "const user = { name: 'John', role: 'admin' };" javascript --type snip

# SQL query
bkmr add "SELECT * FROM users WHERE created_at > NOW() - INTERVAL 7 DAY;" sql --type snip

# Command line snippet
bkmr add "docker run -it --rm -v \$(pwd):/app node:18 npm test" docker,testing --type snip
```

**Benefits:**
- Instant access to common code patterns
- Reduces typing errors and ensures consistency
- Supports any programming language
- Template interpolation for dynamic values

## Shell Scripts (`_shell_`)

**Default Action:** Interactive editor then execute

```bash
# Simple shell script
bkmr add "#!/bin/bash\necho 'Deployment started'\nssh server 'sudo systemctl restart app'" deploy --type shell

# Script with template interpolation
bkmr add "#!/bin/bash\necho 'Backup for {{ current_date | strftime('%Y-%m-%d') }}'\ntar -czf backup.tar.gz /data/" backup --type shell
```

**Execution Modes:**

1. **Interactive (default):** Presents editor before execution
   ```bash
   bkmr open 123   # Opens editor with script content
   ```

2. **Direct execution:** Skip interactive editing
   ```bash
   bkmr open --no-edit 123
   ```

3. **With arguments:** Pass arguments to script
   ```bash
   bkmr open --no-edit 123 -- --env production --dry-run
   ```

**Interactive Editor Features:**
- Pre-filled with script content
- Vim/Emacs bindings based on shell configuration
- Command history saved to `~/.config/bkmr/shell_history.txt`
- Supports modification before execution

**Configuration:**
```bash
# Disable interactive mode globally
export BKMR_SHELL_INTERACTIVE=false

# Or in config.toml
[shell_opts]
interactive = false
```

## Markdown Documents (`_md_`)

**Default Action:** Render HTML and open in browser

```bash
# Inline markdown
bkmr add "# Project Notes\n\n## Tasks\n- [ ] Complete docs\n- [ ] Add tests" notes --type md

# Reference to markdown file
bkmr add "~/documents/project-specs.md" specifications --type md

# Markdown with math formulas
bkmr add "# Statistics\n\n$$E = mc^2$$\n\nInline: $P(x) = \\frac{1}{\\sigma\\sqrt{2\\pi}}$" math --type md
```

**Features:**
- Full markdown rendering with syntax highlighting
- MathJax support for LaTeX formulas
- File path resolution (supports `~`, environment variables)
- No template processing (to avoid conflicts with markdown syntax like `{%}`)
- Automatic embedding updates for file-based content

## Environment Variables (`_env_`)

**Default Action:** Print to stdout for shell sourcing

```bash
# Development environment
bkmr add "export DB_URL=postgres://localhost/dev\nexport API_KEY=dev_key\nexport DEBUG=true" dev-env --type env

# Production environment with templates
bkmr add "export TIMESTAMP={{ current_date | strftime('%Y%m%d_%H%M%S') }}\nexport GIT_BRANCH={{ \"git branch --show-current\" | shell }}" deploy-env --type env
```

**Usage:**
```bash
# Source environment variables
eval "$(bkmr open 123)"

# Or use in scripts
source <(bkmr open 123)
```

## Text Documents (`_imported_`)

**Default Action:** Copy to clipboard

Primarily used for imported text files. Usually assigned automatically during file import.

```bash
# Import text files
bkmr import-files ~/documents/notes.txt

# Manually add text content
bkmr add "Important phone numbers:\nSupport: 555-0123\nEmergency: 911" contacts --type text
```

## File References

Any content type can reference local files instead of containing inline content:

```bash
# Markdown file reference
bkmr add "~/docs/api-guide.md" documentation --type md

# Shell script file reference  
bkmr add "~/scripts/deploy.sh" automation --type shell

# Text file reference
bkmr add "~/notes/meeting-notes.txt" meetings --type text
```

**File Handling:**
- Automatic file content loading
- Path resolution with environment variables
- Template interpolation in file content (except for markdown files)
- Embedding updates when files change (for `--openai` enabled bookmarks)

## Template Interpolation

Most content types support Jinja2-style template interpolation (except markdown to avoid syntax conflicts):

### Available Variables and Filters

**Date/Time:**
```bash
{{ current_date | strftime('%Y-%m-%d') }}          # 2025-06-28
{{ current_date | strftime('%B %d, %Y') }}         # June 28, 2025
{{ current_date | subtract_days(7) }}              # 7 days ago
```

**Environment:**
```bash
{{ env('HOME') }}                                   # Environment variable
{{ env('API_KEY', 'default-key') }}                # With default value
```

**Shell Commands:**
```bash
{{ "whoami" | shell }}                             # Current username
{{ "git branch --show-current" | shell }}         # Current git branch
{{ "hostname" | shell }}                           # System hostname
```

### Template Examples

**Dynamic URLs:**
```bash
bkmr add "https://reports.company.com/{{ current_date | strftime('%Y/%m') }}/summary" reports
```

**Parameterized Scripts:**
```bash
bkmr add "#!/bin/bash\necho 'Deployment on {{ current_date }}'\necho 'Branch: {{ \"git branch --show-current\" | shell }}'" deploy --type shell
```

**Environment with Context:**
```bash
bkmr add "export PROJECT_ROOT={{ env('HOME') }}/projects\nexport BUILD_TIME={{ current_date | strftime('%Y%m%d_%H%M%S') }}" build-env --type env
```

## Smart Actions in Practice

### Workflow Integration

**1. Copy-Edit-Execute Pattern:**
```bash
# Copy snippet for modification
bkmr search --fzf -t _snip_,docker
# Edit and use in your editor

# Execute automation script
bkmr search --fzf -t _shell_,deploy
# Interactive edit with parameters, then execute
```

**2. Documentation Flow:**
```bash
# Quick reference lookup
bkmr search --fzf -t _md_,api
# Opens rendered documentation in browser

# Environment setup
eval "$(bkmr search --np -t _env_,development)"
```

**3. Chained Actions:**
```bash
# Shell script that uses other bookmarks
bkmr add "#!/bin/bash\neval \"\$(bkmr open 7)\"\npsql -c \"\$(bkmr open 5)\"" db-workflow --type shell
```

### FZF Integration

When using `bkmr search --fzf`, actions are displayed and executable:

- **Enter:** Execute default action
- **Ctrl-Y:** Copy URL/content to clipboard (overrides default action)
- **Ctrl-E:** Edit bookmark
- **Ctrl-D:** Delete bookmark

## Best Practices

**1. Consistent Tagging:**
```bash
# Language + purpose
bkmr add "code here" python,authentication --type snip

# Environment + technology
bkmr add "export vars" docker,development --type env

# Action + domain  
bkmr add "script content" deploy,production --type shell
```

**2. Template Usage:**
- Use templates for dynamic content that changes based on context
- Prefer shell filters for system information
- Use date filters for time-based content

**3. Content Organization:**
- Use system tags (`_snip_`, `_shell_`, etc.) consistently
- Add descriptive tags for easy discovery
- Group related content with common tag prefixes

**4. Security Considerations:**
- Be cautious with shell scripts containing sensitive data
- Use environment variables for secrets rather than inline content
- Review scripts before execution, especially in interactive mode