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
bkmr add "#!/bin/bash\necho 'Hello World'\nls -la" shell,scripts --type shell

# Can also be added with shell:: prefix
bkmr add "shell::find . -name '*.rs' | xargs grep 'fn main'" rust,find
```

**Default Action:** Executes the script in a shell with proper permissions.
**System Tag:** `_shell_`

### Markdown Documents
Store documentation, notes, or any content using Markdown formatting.

```bash
# Add a markdown document
bkmr add "# Project Setup\n\n## Requirements\n- Node.js v14+\n- PostgreSQL\n\n## Steps\n1. Clone repo\n2. Run npm install" 
     docs,setup --type md
```

**Default Action:** Renders the Markdown to HTML and opens in browser.
**System Tag:** `_md_`

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

## Content Type Auto-Detection

`bkmr` attempts to detect content types automatically:

| Pattern | Detected Type |
|---------|--------------|
| URLs starting with `http://` or `https://` | `uri` |
| URLs starting with `shell::` | Shell command |
| Content with shell shebang (`#!/bin/...`) | `shell` |
| Content starting with Markdown headers (`#`) | `md` |
| Paths starting with `/` or `~/` | File path |

## System Tags and Action Resolution

Content types are marked with internal system tags, which `bkmr` uses to determine the appropriate action:

1. `_snip_`: Snippets are copied to clipboard
2. `_shell_`: Shell scripts are executed in terminal
3. `_md_`: Markdown is rendered and viewed in browser
4. `_imported_`: Text documents are copied to clipboard

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
bkmr add "# Meeting Notes: {{ current_date | strftime('%B %d, %Y') }}\n\n## Agenda\n1. Project status\n2. Next steps" 
     meeting,notes --type md

# A shell script with environment variables
bkmr add "#!/bin/bash\ncd {{ env('PROJECT_DIR', '~/projects') }}\ngit status" git,status --type shell
```

## Benefits for Developer Workflow

The content-aware system provides several advantages:

1. **Context-appropriate handling** - Content is processed according to its type
2. **Workflow acceleration** - Snippets and commands are immediately available
3. **Documentation on demand** - Markdown renders beautifully when needed
4. **Unified interface** - All knowledge is accessible through the same commands

## Content Examples

### Markdown Document Example

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