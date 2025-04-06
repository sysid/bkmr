# Content Types in bkmr

`bkmr` supports various content types beyond traditional web bookmarks, allowing you to store and manage different kinds of information in a unified system.

## Supported Content Types

### URL Bookmarks (Default)
Standard web URLs pointing to websites, web applications, or online resources.

```bash
# Add a web URL
bkmr add https://example.com web,resource

# Add with explicit type
bkmr add https://example.com web,resource --type uri
```

### Code Snippets
Store reusable code fragments, SQL queries, or command-line incantations.

```bash
# Add a code snippet
bkmr add "SELECT * FROM users WHERE active = true ORDER BY created_at DESC LIMIT 10;" 
     sql,users --type snip
```

Snippets will be copied to clipboard when opened, rather than launching a browser.

### Text Documents (tbd)
Store plain text content such as notes, documentation, or any textual information.

```bash
# Add a text document
bkmr add "System requires Java 11 and PostgreSQL 13 for deployment." 
     deployment,requirements --type text
```

Like snippets, text content will be copied to clipboard when opened.

### File and Directory Paths
Reference local files or directories on your filesystem.

```bash
# Add a file path
bkmr add ~/documents/important.pdf documentation,reference

# Add a directory
bkmr add ~/projects/current-project project,code
```

When opened, these will launch using your system's default application.

### Shell Commands
Store and execute shell commands or scripts.

```bash
# Add a shell command
bkmr add "shell::find ~/projects -name '*.js' | xargs grep 'TODO'" todos,javascript

# Add a more complex command
bkmr add "shell::docker ps --format '{{.Names}}' | sort" docker,containers
```

When opened, these commands will execute in a shell.

## Setting the Content Type

You can specify a content type when adding a bookmark:

```bash
bkmr add CONTENT tag1,tag2 --type TYPE
```

Where `TYPE` is one of:
- `uri` (default): web URLs and general URIs
- `snip`: code snippets or commands
- `text`: plain text content

Alternatively, content types are automatically inferred:
- URLs starting with `http://` or `https://` are treated as `uri`
- URLs starting with `shell::` are treated as shell commands
- Paths starting with `/` or `~/` are treated as file/directory paths

## System Tags

Content types are also marked with internal system tags:
- `_snip_`: for snippets
- `_text_`: for text documents
- `_uri_`: for URLs (often omitted as it's the default)

These system tags are used internally by `bkmr` and are generally hidden from normal tag operations.

## Behavior When Opening

Each content type has a specific behavior when opened using `bkmr open` or selected in interactive mode:

| Type | Open Action |
|------|-------------|
| URL | Opens in default web browser |
| File/Directory | Opens with system default application |
| Shell Command | Executes in terminal |
| Snippet | Copies to clipboard |
| Text | Copies to clipboard |

## Working with Mixed Content Types

All content types support the core `bkmr` features:
- Tagging and categorization
- Full-text search
- Semantic search (when enabled)
- Editing and updating

This unified approach allows you to build a comprehensive knowledge base with various types of information, all managed through the same interface.
