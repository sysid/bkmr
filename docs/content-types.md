# Content Types and Smart Actions in bkmr

`bkmr` supports various content types beyond traditional web bookmarks, with each type having specific behaviors and actions. This system allows you to build a unified knowledge base with content-aware functionality.

## Supported Content Types

### URL Bookmarks (Default)
Standard web URLs pointing to websites, web applications, or online resources.

```bash
# Add a web URL
bkmr add https://example.com web,resource

# Add with explicit type
bkmr add https://example.com web,resource --type uri
```

**Default Action:** Opens in your default browser.

### Code Snippets
Store reusable code fragments, SQL queries, or command-line incantations.

```bash
# Add a code snippet
bkmr add "SELECT * FROM users WHERE active = true ORDER BY created_at DESC LIMIT 10;" sql,users --type snip
```

**Default Action:** Copies to clipboard for easy pasting into your editor or terminal.
**System Tag:** `_snip_`

### Shell Scripts
Store executable shell scripts or command sequences.

```bash
# Add a shell script
bkmr add '#!/bin/bash\necho "Hello World"\nls -la' shell,scripts --type shell
```

**Default Action:** Executes the script in a shell with proper permissions.
**System Tag:** `_shell_`

### Environment Variables
Store environment variables for sourcing in a shell.

```bash
# Add environment variables
bkmr add "export DB_USER=dev_user\nexport DB_PASSWORD=dev_pass\nexport API_KEY=test_key" dev,database --type env

# Add with template interpolation
bkmr add "export DATE_STAMP={{ current_date | strftime(\"%Y%m%d\") }}\nexport USER=$(whoami)" env,dynamic --type env
```

**Default Action:** Prints to stdout for sourcing in a shell using `eval "$(bkmr open <id>)"` or `source <(bkmr open <id>)`.
**System Tag:** `_env_`

### Markdown Documents
Store documentation, notes, or any content using Markdown formatting. Markdown can be stored directly or as a reference to a local file.

```bash
# Add a markdown document directly
bkmr add "# Project Setup\n\n## Requirements\n- Node.js v14+\n- PostgreSQL\n\n## Steps\n1. Clone repo\n2. Run npm install" 
     docs,setup --type md

# Add a reference to a local markdown file
bkmr add "/path/to/documentation.md" docs,reference --type md

# Add a relative file path
bkmr add "docs/project/setup.md" docs,reference --type md

# Add with shell variables or tilde expansion
bkmr add "~/documents/notes.md" notes,personal --type md
bkmr add "$HOME/documents/notes.md" notes,personal --type md
```

**Default Action:** 
- For direct markdown content: Renders the Markdown to HTML and opens in browser
- For file paths: Reads the file content, renders the Markdown to HTML, and opens in browser
- Supports LaTeX math formulas using MathJax rendering

**System Tag:** `_md_`

**Embedding Support:**
When using with `--openai` flag, markdown content from files can be automatically embedded for semantic search.

### Text Documents
Store plain text content such as notes, documentation, or any textual information.

```bash
# Add a text document
bkmr add "System requires Java 11 and PostgreSQL 13 for deployment." 
     deployment,requirements --type text
```

**Default Action:** Copies to clipboard.
**System Tag:** `_imported_`

### File and Directory Paths
Reference local files or directories on your filesystem.

```bash
# Add a file path
bkmr add ~/documents/important.pdf documentation,reference

# Add a directory
bkmr add ~/projects/current-project project,code
```

**Default Action:** Opens with system's default application for the file type.

## Setting the Content Type

You can specify a content type when adding a bookmark:

```bash
bkmr add CONTENT tag1,tag2 --type TYPE
```

Where `TYPE` is one of:
- `uri` (default): Web URLs and general URIs
- `snip`: Code snippets
- `shell`: Shell scripts for execution
- `md`: Markdown documents
- `text`: Plain text content
- `env`: Environment variables for sourcing

## Content Type Auto-Detection

`bkmr` attempts to detect content types automatically:

| Pattern | Detected Type |
|---------|--------------|
| URLs starting with `http://` or `https://` | `uri` |
| Content with shell shebang (`#!/bin/...`) | `shell` |
| Content starting with Markdown headers (`#`) | `md` |
| Paths starting with `/` or `~/` | File path |
| Paths containing `.md` extension | Markdown file |
| Content with multiple `export VAR=value` lines | `env` |

## System Tags and Action Resolution

Content types are marked with internal system tags, which `bkmr` uses to determine the appropriate action:

1. `_snip_`: Snippets are copied to clipboard
2. `_shell_`: Shell scripts are executed in terminal
3. `_md_`: Markdown is rendered and viewed in browser
4. `_imported_`: Text documents are copied to clipboard
5. `_env_`: Environment variables are printed to stdout for sourcing

These system tags are mostly hidden from normal tag operations but can be viewed with detailed display.

## Customizing Default Actions

The behavior when accessing content follows this resolution sequence:

1. Check for system tags to determine content type
2. Apply the appropriate action for that content type
3. Fall back to default URI behavior if no specific type is detected

## Working with Templates in Content

All content types support template interpolation (see [Template Interpolation](./template-interpolation.md)):

```bash
# A markdown document with dynamic date
bkmr add "# Meeting Notes: {{ current_date | strftime('%B %d, %Y') }}\n\n## Agenda\n1. Project status\n2. Next steps" meeting,notes --type md --title "Meeting Notes"

# A shell script with environment variables (auto-detected as SystemTag `_shell_`)
bkmr add '#!/bin/bash\ncd {{ env("PROJECT_DIR", "~/projects") }}\ngit status' git,status --type shell --title "Git Status"

# Environment variables with dynamic content
bkmr add "export TIMESTAMP={{ current_date | strftime('%Y%m%d_%H%M%S') }}\nexport GIT_BRANCH={{ \"git branch --show-current\" | shell }}" deploy,env --type env --title "Deployment Environment"
```
Of course more convenient is to use the interactive, editor-based input:
```
bkmr add --help
bkmr add -e -t uri|snip|text|shell|md|env
```


## Benefits for Developer Workflow

The content-aware system provides several advantages:

1. **Context-appropriate handling** - Content is processed according to its type
2. **Workflow acceleration** - Snippets and commands are immediately available
3. **Documentation on demand** - Markdown renders beautifully when needed
4. **Environment management** - Shell environments can be sourced quickly
5. **Unified interface** - All knowledge is accessible through the same commands

## Content Examples

### Environment Variables Example

```bash
# Development environment variables
export DB_HOST=localhost
export DB_PORT=5432
export DB_NAME=dev_db
export DB_USER=dev_user
export DB_PASSWORD=dev_password

# Set PATH to include project binaries
export PATH="$HOME/projects/myapp/bin:$PATH"

# Add dynamic timestamp for deployments
export BUILD_TIMESTAMP={{ current_date | strftime('%Y%m%d_%H%M%S') }}

echo "Development environment loaded"
```

Usage:
```bash
# Source environment variables
eval "$(bkmr open 123)"

# Or with process substitution
source <(bkmr open 123)
```

### Markdown Document Examples

#### Direct Markdown Content

```markdown
# Project Setup

## Prerequisites
- Node.js 14+
- Docker

## Steps
1. Clone repository
2. Run `npm install`
3. Start with `npm run dev`

## Troubleshooting
See the [wiki](https://example.com/wiki).
```

#### Markdown Math Support

Markdown documents with LaTeX math formulas will render properly:

```markdown
# Binary Classification Metrics

The F1 score is the harmonic mean of precision and recall, giving both metrics equal weight. The formula for the F1 score is:

$$
F1 = 2 \times \left( \frac{precision \times recall}{precision + recall} \right)
$$

## Precision

$$
\text{Precision} = \frac{\text{True Positives}}{\text{True Positives} + \text{False Positives}}
$$

## Recall

$$
\text{Recall} = \frac{\text{True Positives (TP)}}{\text{True Positives (TP)} + \text{False Negatives (FN)}}
$$
```

#### File Reference 

```
bkmr add "~/documents/project-notes.md" project,documentation --type md
```

### Shell Script Example

```bash
#!/bin/bash
# Database backup script

echo "Starting backup..."
pg_dump mydb > ~/backups/mydb_$(date +%Y%m%d).sql
echo "Backup complete!"
```

### Code Snippet Example

```python
# Python snippet for data processing
import pandas as pd

def clean_data(df):
    # Drop duplicates
    df = df.drop_duplicates()
    # Handle missing values
    df = df.fillna({'numeric_col': 0, 'text_col': ''})
    return df
```