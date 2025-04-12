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
    # Get database backup script and run it
    bkmr search --np --tags _shell_,backup "script-name"
    # Get deployment script and run it
    bkmr search --np --tags _shell_,deploy "script-name"
}
```

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

### Example 4: Dynamic Template Search

```bash
# Find templates with dynamic content
template-search() {
    bkmr search --fzf "{{ $1 }}"
}

# Usage: template-search "current_date"
# Finds all bookmarks containing date templates
```
## Advanced Environment Variable Management

The `_env_` system tag enables powerful environment management workflows. Here are some advanced usage patterns:

### Project-Specific Environment Switcher

```bash
# Create a function to switch between project environments
project-env() {
    local project=$1
    local env=${2:-dev}  # Default to dev environment
    
    # Search for the right environment bookmark
    local env_id=$(bkmr search --np --tags-prefix _env_ -t "$project","$env" | head -n1)
    
    if [[ -n "$env_id" ]]; then
        echo "Loading $project $env environment..."
        eval "$(bkmr open "$env_id")"
        echo "Environment loaded successfully"
    else
        echo "No environment found for $project $env"
    fi
}

# Usage: project-env myapp dev
# Usage: project-env myapp prod
```

### Chained Environment Loading

You can create a system where environment variables build on each other:

```bash
# Base environment (common variables)
bkmr add "export PROJECT_ROOT=\"$HOME/projects/myapp\"
export PATH=\"\$PROJECT_ROOT/bin:\$PATH\"
export NODE_ENV=\"development\"
" base,environment --type env

# Database environment (depends on base)
bkmr add "# First load base environment
eval \"\$(bkmr search --np -t base,environment)\"

# Then add database-specific variables
export DB_HOST=\"localhost\"
export DB_PORT=\"5432\"
export DB_NAME=\"myapp_dev\"
export DB_USER=\"postgres\"
export DB_PASSWORD=\"postgres\"
" database,environment --type env
```

### Dynamic Environment Configuration

Create environment configurations that adapt based on context:

```bash
# Create dynamic environment variables
bkmr add "# Dynamic project environment
export GIT_BRANCH=\"{{ \"git rev-parse --abbrev-ref HEAD\" | shell }}\"
export IS_MAIN_BRANCH=\"{{ \"test \$(git rev-parse --abbrev-ref HEAD) = 'main'\" | shell }}\"
export LAST_TAG=\"{{ \"git describe --tags --abbrev=0\" | shell }}\"

# Configure environment based on branch
if [[ \"\$IS_MAIN_BRANCH\" == \"true\" ]]; then
  export API_URL=\"https://api.example.com/v1\"
  export FEATURE_FLAGS=\"stable\"
else
  export API_URL=\"https://dev-api.example.com/v1\"
  export FEATURE_FLAGS=\"experimental\"
fi

# Set build metadata
export BUILD_DATE=\"{{ current_date | strftime(\"%Y-%m-%d\") }}\"
export BUILD_USER=\"{{ \"whoami\" | shell }}\"
" dynamic,environment --type env
```

### Environment Management System

Combine these techniques to create a comprehensive environment management system:

```bash
# Create shell function for environment management
env-manager() {
    case "$1" in
        load)
            local project=$2
            local env=${3:-dev}
            local env_id=$(bkmr search --np --tags-prefix _env_ -t "$project","$env" | head -n1)
            if [[ -n "$env_id" ]]; then
                eval "$(bkmr open "$env_id")"
                echo "Loaded $project $env environment"
            else
                echo "No environment found for $project $env"
                return 1
            fi
            ;;
            
        save)
            local project=$2
            local env=${3:-dev}
            local description="Environment variables for $project ($env)"
            
            # Collect current environment variables
            local env_vars=$(env | grep -E '^(DB_|API_|APP_|PROJECT_)')
            
            if [[ -z "$env_vars" ]]; then
                echo "No environment variables found to save"
                return 1
            fi
            
            # Format for bkmr storage
            local formatted_vars=$(echo "$env_vars" | sed 's/^/export /')
            
            # Add timestamp and user information
            local header="# $project $env environment\n# Saved on $(date)\n# By $(whoami)\n\n"
            local content="${header}${formatted_vars}"
            
            # Save to bkmr
            bkmr add "$content" "$project","$env",environment --type env
            echo "Saved current environment as $project $env"
            ;;
            
        list)
            local project=$2
            if [[ -n "$project" ]]; then
                bkmr search -t "$project",environment
            else
                bkmr search -t environment
            fi
            ;;
            
        *)
            echo "Usage: env-manager [load|save|list] [project] [env]"
            ;;
    esac
}

# Usage examples:
# env-manager load myapp dev
# env-manager save myapp prod
# env-manager list myapp
```

This function provides a complete environment management system:
- `load` sources environment variables for a specific project and environment
- `save` captures current environment variables into a new bookmark
- `list` displays all available environment configurations

### Integration with Direnv

You can integrate bkmr with direnv for automatic environment loading:

```bash
# Create a .envrc file that sources bkmr environment variables
echo 'eval "$(bkmr search --np -t project-name,environment)"' > .envrc
direnv allow
```

This approach combines the power of bkmr's environment management with direnv's automatic directory-based activation.

### Cross-Platform Environment Synchronization

Create environment configurations that work across different platforms:

```bash
# Create platform-adaptive environment variables
bkmr add "# Cross-platform environment
# Detect platform
if [[ \"$(uname)\" == \"Darwin\" ]]; then
  # macOS specific
  export PATH=\"/usr/local/bin:/opt/homebrew/bin:\$PATH\"
  export JAVA_HOME=\"$(/usr/libexec/java_home)\"
elif [[ \"$(uname)\" == \"Linux\" ]]; then
  # Linux specific
  export PATH=\"/usr/local/bin:\$PATH\"
  export JAVA_HOME=\"/usr/lib/jvm/default-java\"
fi

# Common variables
export PROJECT_ROOT=\"{{ env('HOME') }}/projects/myapp\"
export NODE_ENV=\"development\"
" cross-platform,environment --type env
```

This approach ensures consistent environments across different development machines while adapting to platform-specific requirements.

## Combining Actions and Templates for Development Workflows

### Project Switcher

```bash
# Creates a project switcher using shell scripts
create-project-switcher() {
    bkmr add "#!/bin/bash
cd {{ env('PROJECTS_DIR', '~/projects') }}/$1
if [ -f 'package.json' ]; then
    echo 'Node.js project detected'
    npm install
elif [ -f 'requirements.txt' ]; then
    echo 'Python project detected'
    python -m venv venv
    source venv/bin/activate
    pip install -r requirements.txt
fi
echo 'Project $1 ready'" project,setup,automation --type shell
}
```

### Environment-Specific Documentation Viewer

```bash
# Create a documentation viewer that adapts to your environment
env-docs() {
    bkmr add "# {{ env('PROJECT_NAME', 'Default') }} Documentation

## Environment: {{ env('ENV', 'development') }}

### Configuration
- API URL: {{ env('API_URL', 'http://localhost:8000') }}
- Database: {{ env('DB_NAME', 'local_db') }}

### Setup Instructions
```bash
# Clone the repository
git clone {{ env('REPO_URL', 'https://github.com/example/repo.git') }}

# Install dependencies
cd {{ env('PROJECT_NAME', 'project') }}
{{ 'if [ -f package.json ]; then npm install; else pip install -r requirements.txt; fi' | shell }}
```" documentation,setup --type md
}
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