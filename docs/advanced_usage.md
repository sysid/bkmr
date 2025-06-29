# Advanced Usage Guide for bkmr

This document covers advanced usage patterns and techniques to maximize your productivity with bkmr.

## Tag Prefix Filtering

One of bkmr's most powerful features is its tag prefix filtering system, which allows you to create reusable search patterns.

### Understanding Tag Prefixes

Tag prefix options allow you to specify a set of tags that will be combined with command-line specified tags. This creates a union of the same tag types from both sources.

Available prefix options:
- `--tags-prefix`: Combined with `--tags` (all tags must match)
- `--ntags-prefix`: Combined with `--ntags` (any tag may match)
- `--Tags-prefix`: Combined with `--Tags` (none of these tags must match)
- `--Ntags-prefix`: Combined with `--Ntags` (none of these tags may match)

### Why Prefix Filtering is Powerful

Prefix filtering enables you to:

1. **Create specialized search contexts**: Pre-filter for certain content types
2. **Enforce content boundaries**: Automatically exclude certain content categories
3. **Simplify complex queries**: Combine multiple filtering strategies in shell functions
4. **Create domain-specific mini-applications**: Build custom tools for different use cases

## Smart Actions in Advanced Workflows

bkmr's content-aware action system can be leveraged in advanced workflows to create powerful automation.

### Content Type Filtering with System Tags

System tags like `_snip_`, `_shell_`, `_md_`, and `_imported_` can be used with tag filtering to create type-specific searches:

```bash
# Find only shell scripts
bkmr search --tags _shell_ "deployment"

# Find markdown documentation
bkmr search --tags _md_ "project setup"
```

### Action-based Workflow Functions

You can create shell functions for specific workflow needs:

```bash
# Execute shell scripts related to a specific project
run-project-script() {
    bkmr search --fzf --tags _shell_,project-"$1"
}

# Execute script with arguments (new in v4.26+)
run-script-with-args() {
    local script_id="$1"
    shift
    bkmr open --no-edit "$script_id" -- "$@"
}

# View documentation for a technology
view-docs() {
    bkmr search --fzf --tags _md_,"$1"
}

# Get code snippets for a language
get-snippets() {
    bkmr search --fzf --tags _snip_,"$1"
}
```

### Chaining Actions Together

Create powerful workflows by chaining multiple actions:

```bash
# Deploy application function
deploy-app() {
    local env="${1:-staging}"
    local dry_run="${2:-false}"
    
    # Run backup script with environment parameter
    echo "Running backup for $env environment..."
    bkmr open --no-edit 101 -- --env "$env"
    
    # Run deployment script with parameters
    echo "Deploying to $env..."
    if [[ "$dry_run" == "true" ]]; then
        bkmr open --no-edit 102 -- --env "$env" --dry-run
    else
        bkmr open --no-edit 102 -- --env "$env"
    fi
}

# Usage: deploy-app production false
```

## Shell Function Stubs - Direct Script Access

The `create-shell-stubs` command provides a powerful way to create shell functions for all your bookmarked shell scripts, enabling direct execution with natural argument passing.

### Basic Shell Stubs Generation

```bash
# View all shell function stubs that would be created
bkmr create-shell-stubs

# Example output:
# backup-database() { bkmr open --no-edit 123 -- "$@"; }
# export -f backup-database
# deploy-app() { bkmr open --no-edit 124 -- "$@"; }
# export -f deploy-app
# monitoring-setup() { bkmr open --no-edit 125 -- "$@"; }
# export -f monitoring-setup
```

### Integration Strategies

#### Method 1: Dynamic Loading (Recommended for Development)

```bash
# Source directly into current shell - always fresh
source <(bkmr create-shell-stubs)

# Add to your shell profile for automatic loading
echo 'source <(bkmr create-shell-stubs)' >> ~/.bashrc
echo 'source <(bkmr create-shell-stubs)' >> ~/.zshrc
```

**Benefits:**
- Always reflects current bookmarks
- Automatically includes new shell script bookmarks
- No maintenance required

**Considerations:**
- Small startup delay (typically <100ms)
- Requires bkmr to be available in PATH

#### Method 2: Static Caching (Recommended for Production)

```bash
# Generate static functions file
bkmr create-shell-stubs > ~/.config/bkmr/shell-functions.sh

# Source the cached file in your profile
echo 'source ~/.config/bkmr/shell-functions.sh' >> ~/.bashrc

# Update when you add new shell script bookmarks
alias update-shell-stubs='bkmr create-shell-stubs > ~/.config/bkmr/shell-functions.sh'
```

**Benefits:**
- Faster shell startup
- Works without bkmr in PATH
- Explicit control over updates

**Considerations:**
- Manual refresh needed when bookmarks change
- Potential for stale functions

### Advanced Usage Patterns

#### Selective Function Loading

```bash
# Create functions only for specific tag patterns
create-dev-functions() {
    bkmr search --tags _shell_,development --json | \
    jq -r '.[].id' | \
    while read id; do
        local title=$(bkmr show "$id" | grep "Title:" | cut -d: -f2- | xargs)
        local func_name=$(echo "$title" | tr '[:upper:]' '[:lower:]' | sed 's/[^a-z0-9-]/_/g')
        echo "${func_name}() { bkmr open --no-edit $id -- \"\$@\"; }"
        echo "export -f $func_name"
    done
}
```

#### Function Namespace Management

```bash
# Prefix all functions to avoid conflicts
bkmr create-shell-stubs | sed 's/^/bkmr_/' > ~/.config/bkmr/namespaced-functions.sh

# Creates: bkmr_backup-database(), bkmr_deploy-app(), etc.
```

#### Integration with Function Managers

```bash
# For bash-completion integration
_bkmr_shell_functions() {
    local functions=$(bkmr create-shell-stubs | grep '^[a-zA-Z]' | cut -d'(' -f1)
    COMPREPLY=($(compgen -W "$functions" -- "${COMP_WORDS[COMP_CWORD]}"))
}

# Auto-complete your bookmark functions
complete -F _bkmr_shell_functions your-bookmark-function
```

### Real-World Workflow Examples

#### DevOps Toolkit

```bash
# Add to ~/.bashrc or ~/.zshrc
source <(bkmr create-shell-stubs)

# Now your bookmarked scripts become part of your shell environment:
backup-database production --incremental
deploy-microservice user-auth staging --canary-percentage 10
scale-cluster monitoring --nodes 5
update-certificates *.example.com --dry-run

# All with full argument support and tab completion (if configured)
```

#### Project-Specific Workflows

```bash
# Create project-specific shell stub files
project-stubs() {
    local project="$1"
    bkmr search --tags _shell_,"$project" --json | \
    jq -r '.[].id' | \
    while read id; do
        local bookmark=$(bkmr show "$id")
        local title=$(echo "$bookmark" | grep "Title:" | cut -d: -f2- | xargs)
        local func_name=$(echo "$title" | tr '[:upper:]' '[:lower:]' | sed 's/[^a-z0-9-]/_/g')
        echo "${func_name}() { bkmr open --no-edit $id -- \"\$@\"; }"
        echo "export -f $func_name"
    done > ".${project}-stubs.sh"
    
    echo "Created .${project}-stubs.sh - source it with: source .${project}-stubs.sh"
}

# Usage
project-stubs myapp
source .myapp-stubs.sh
```

### Function Name Conventions

The `create-shell-stubs` command follows these naming rules:

- **Preserves hyphens**: `"backup-database"` → `backup-database()`
- **Converts spaces to underscores**: `"Deploy Script"` → `deploy_script()`
- **Handles special characters**: `"My Awesome Script!"` → `my_awesome_script()`
- **Prevents numeric start**: `"2fa-setup"` → `script-2fa-setup()`
- **Fallback for invalid names**: `"!@#$%"` → `shell_script()`

### Best Practices

1. **Use Descriptive Bookmark Titles**: Since function names derive from titles, use clear, concise names
2. **Tag Consistently**: Use consistent tagging for easier filtering and organization
3. **Test Function Names**: Preview output before sourcing to ensure no conflicts
4. **Document Complex Functions**: Add comments to your shell profile explaining complex workflows
5. **Regular Cleanup**: Periodically review and clean up unused bookmarks to keep function list manageable

### Troubleshooting

#### Function Name Conflicts
```bash
# Check for conflicts before sourcing
bkmr create-shell-stubs | grep '^[a-zA-Z]' | cut -d'(' -f1 | sort | uniq -d

# Rename conflicting bookmarks or use namespacing
```

#### Performance Issues
```bash
# Profile shell startup time
time (source <(bkmr create-shell-stubs))

# Switch to static caching if too slow
```

#### Missing Functions
```bash
# Verify shell script bookmarks exist
bkmr search --tags _shell_ --json | jq length

# Check if functions are properly exported
declare -F | grep -E "(backup|deploy|monitoring)"
```

### File Quickview with metadata Enrichment
1. Add file as interpolation snippet like:
```bash
=== ID ===
2919
=== URL ===
{{ "cat $HOME/dev/binx/py/llm-json-schema-generate.py" | shell }}
=== TITLE ===
llm-json-schema-generate
=== TAGS ===
_snip_
=== COMMENTS ===
Arbitrary addtional metadata, e.g.
Source: https://simonwillison.net/2025/Feb/28/llm-schemas/
=== EMBEDDABLE ===
false
=== END ===
```
2. view it as snippet.


## FTS Column Prefix Filtering

bkmr supports column-specific full-text search using the `column:term` syntax. When combined with prefix filtering, this becomes extremely powerful.

### Available FTS Columns

- `url:` - Search URLs only
- `metadata:` - Search metadata only (alias for title)
- `desc:` - Search descriptions only
- `tags:` - Search tags only

### Wildcard Matching

You can use `*` as a prefix search:
- `term*` - Terms starting with "term"

## Advanced Usage Examples

### Example 1: Smart Snippet Search

```bash
b() {
    bkmr search --fzf --fzf-style enhanced --Ntags-prefix _imported_ --tags-prefix _snip_ "metadata:${1}*"
}
```

This function:
1. Searches for snippets (includes tag prefix `_snip_`)
2. Excludes imported content (excludes tag prefix `_imported_`)
3. Searches only in titles starting with your search term (`metadata:${1}*`)
4. Uses enhanced FZF interface for better display

**Use case**: Finding code snippets by title prefix

```bash
# Find all Docker-related snippets
b docker

# Find all Git-related snippets
b git
```

### Example 2: Quick Content Creation by Type

```bash
# Quick snippet creation
bs() {
    bkmr add -e -t snip "$@"
}

# Quick markdown document creation
bm() {
    bkmr add -e -t md "$@"
}

# Quick shell script creation
bsh() {
    bkmr add -e -t shell "$@"
}
```

These functions create type-specific content with the appropriate actions.

### Example 3: Documentation-specific Searches

```bash
alias d-="BKMR_DB_URL=$HOME/vimwiki/buku/bm.db bkmr search --Ntags-prefix _snip_,_imported_,_shell_,_md_ --tags-prefix doc"
alias d-aws="BKMR_DB_URL=$HOME/vimwiki/buku/bm.db bkmr search --fzf --Ntags-prefix _snip_,_imported_,_shell_,_md_ --tags-prefix doc,aws"
```

These aliases:
1. Use specific URLs for documentation purposes
2. Exclude all system-tagged content (only plain URLs)
3. Include only items tagged with "doc"
4. For AWS docs, additionally filter for "aws" tag


## Advanced Environment Variable Management

The `_env_` system tag enables powerful environment management workflows. Here are some advanced usage patterns:

### Project-Specific Environment Switcher

```bash
# Create a function to switch between project environments
project-env() {
    local project=$1
    local env=${2:-dev}  # Default to dev environment

    # Search for the right environment bookmark
    echo "Loading $project $env environment..."
    eval "$(bkmr search --fzf --fzf-style enhanced --tags-prefix _env_ -t "$project","$env")"
    echo "Environment loaded successfully"
}

# Usage: project-env myapp dev
# Usage: project-env myapp prod
```

## Creating Advanced Search Contexts

By combining prefix filtering with FTS column searches, you can create powerful search contexts for different needs:

### Project-specific References

```bash
proj-refs() {
    bkmr search --fzf --tags-prefix project,reference -t "$1" "$2"
}

# Usage: proj-refs [PROJECT-TAG] [OPTIONAL-SEARCH-TERM]
# Example: proj-refs frontend "react hooks"
```

### Language-specific Snippets

```bash
lang-snippets() {
    bkmr search --fzf --tags-prefix _snip_ -t "$1" "$2"
}

# Usage: lang-snippets [LANGUAGE] [OPTIONAL-SEARCH-TERM]
# Example: lang-snippets python "decorator"
```

## Advanced Filtering Techniques

### Combining Multiple Tag Types

You can combine different tag filtering methods:

```bash
# Find Python or Rust snippets that are NOT tagged as beginner
bkmr search --tags-prefix _snip_ -n python,rust -N beginner
```

### Complex FTS Queries

Full-text search supports complex queries:

```bash
# Find Docker entries with Compose or Swarm in the description
bkmr search "tags:docker desc:compose desc:swarm"
```

### Date-based Filtering with Sort Direction

Find recently added or the oldest entries:

```bash
# Most recently added entries
bkmr search --descending --limit 10

# Oldest entries that need review
bkmr search --ascending --tags needs-review
```

## Building a Knowledge Management System

bkmr's combination of tagging, actions, and templates enables building a comprehensive knowledge management system:

### Reference Architecture

1. **URLs**: Web resources and online documentation
   - Tagged by technology, platform, purpose
   - Use standard URLs

2. **Snippets**: Reusable code fragments
   - Tagged by language, purpose, complexity
   - Use `--type snip` for clipboard action

3. **Shell Scripts**: Automation scripts
   - Tagged by function, environment, technology
   - Use `--type shell` for execution action

4. **Markdown Documents**: Comprehensive documentation
   - Tagged by topic, project, status
   - Use `--type md` for browser rendering

5. **Templates**: Dynamic content
   - Tagged by purpose, context
   - Any content type can use template variables

### Optimizing Tag Structure

Develop a consistent tagging strategy:

1. **Primary categories**: Use single-word tags like `python`, `docker`, `aws`
2. **Qualities/Properties**: Tags like `tutorial`, `reference`, `example`
3. **Projects**: Prefix with `project-` like `project-website`, `project-api`
4. **Status**: Tags like `active`, `archived`, `needs-review`
5. **Content Type**: System tags handle this automatically

### Balancing Tag Specificity

Creating too many specific tags makes it harder to maintain consistency. Strike a balance:

- Too general: `code`, `document`
- Too specific: `python3.9-asyncio-example`, `aws-lambda-python-tutorial`
- Just right: `python`, `asyncio`, `aws`, `lambda`, `tutorial`

### Bulk Tag Management
```bash
# remove tag 'dev' from list of entries, keep only 'doc,java'
bkmr update -n dev $(bkmr search -t doc,java,dev --np)
```

## Extending bkmr

### Integration with Other Tools

bkmr works well with other command-line tools:

```bash
# Use jq to process JSON output
bkmr search --json "python" | jq '.[] | {title, url}'

# Use fzf for additional filtering
bkmr search --json "programming" | jq -r '.[] | .title' | fzf
```

### Custom Output Processing

You can process search results for custom displays:

```bash
# Create a formatted HTML report of bookmarks
bkmr search --json "important" | jq -r '.[] | "<li><a href=\"\(.url)\">\(.title)</a></li>"' > bookmarks.html
```

### Backup and Version Control

Create automated backups of your bkmr databases:

```bash
# Daily backup script
backup-bkmr() {
    cp "$HOME/.config/bkmr/bkmr.db" "$HOME/backups/bkmr/bkmr-$(date +%Y%m%d).db"
    git -C "$HOME/backups/bkmr" add .
    git -C "$HOME/backups/bkmr" commit -m "Backup $(date +%Y-%m-%d)"
}
```

## Troubleshooting

### Common Issues

If your tag prefixes aren't working as expected:
- Verify your database contains the expected tags with `bkmr tags`
- Check if you're using the correct system tags (`_snip_`, `_shell_`, `_md_`, `_imported_`)

### Debugging Tips

Enable debug output to see what's happening:

```bash
bkmr -d search --tags-prefix project --ntags code
```

For action issues:
```bash
bkmr -d -d open 123  # Double debug flag for more detailed output
```

## Conclusion

By mastering tag prefix filtering, content-specific actions, and template interpolation, you can transform bkmr from a simple bookmark manager into a powerful knowledge management system tailored to your specific workflows.

The combination of these features allows you to create specialized tools for different development tasks, while maintaining a single source of truth for your technical knowledge.
