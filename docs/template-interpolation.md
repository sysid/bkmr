# Template Interpolation in bkmr

`bkmr` incorporates a powerful template system that allows your bookmarks to contain dynamic content. This enables everything from date-based URLs to environment-aware shell scripts and context-sensitive documentation.

## How It Works

Templates are processed when content is accessed (through `open`, `search --fzf`, etc.) and replaced with actual values. This happens just before the content's action is executed.

## Template Syntax

Templates use a Jinja2-inspired syntax:
- `{{ expression }}` - For outputting values
- `{% statement %}` - For control structures like conditionals
- escape backslash `\` with `\\` 
- to not parse content which contains `{{`, enclose it in: `{% raw %}...{% endraw %}`

## Available Variables and Context

### Bookmark Properties
Access information about the current bookmark:

- `id` - The bookmark's unique identifier
- `title` - The bookmark's title
- `description` - The bookmark's description
- `tags` - List of the bookmark's tags
- `access_count` - Number of times the bookmark has been accessed
- `created_at` - When the bookmark was created (ISO format)
- `updated_at` - When the bookmark was last updated (ISO format)

### System Context
Access system information:

- `current_date` - Current date and time in ISO format
- Environment variables via `env_VARIABLE_NAME` (e.g., `env_HOME`)

## Template Filters

Filters transform values and are applied using the pipe symbol (`|`):

### Date Formatting
```
{{ current_date | strftime("%Y-%m-%d") }}  -> 2025-04-11
{{ current_date | strftime("%B %d, %Y") }} -> April 11, 2025
```

### Date Arithmetic
```
{{ current_date | subtract_days(7) | strftime("%Y-%m-%d") }} -> A week ago
{{ current_date | add_days(30) | strftime("%Y-%m-%d") }}     -> 30 days from now
```

### Shell Command Execution
```
{{ "hostname" | shell }}    -> Executes hostname command, inserts the output
{{ "whoami" | shell }}      -> Current user
{{ "git branch" | shell }}  -> Current git branch
```

### Environment Variables
Access environment variables with optional defaults:
```
{{ env("API_KEY", "default-key") }}
{{ env("PROJECT_DIR", "~/projects") }}
```

## Template Examples by Content Type

### URLs with Templates
```
# Daily report URL
https://reports.example.com/{{ current_date | strftime("%Y-%m-%d") }}

# User-specific dashboard
https://dashboard.example.com/user/{{ "whoami" | shell }}

# API with authentication
https://api.example.com/data?token={{ env("API_TOKEN", "demo") }}
```

### Shell Scripts with Templates
```bash
#!/bin/bash
# Dynamic workspace switcher
cd {{ env("WORKSPACE_DIR", "~/workspaces") }}/{{ env("PROJECT", "default") }}
echo "Switched to project: {{ env("PROJECT", "default") }}"
git status
```

### Markdown with Templates
```markdown
# Meeting Notes: {{ current_date | strftime("%B %d, %Y") }}

## Attendees
- {{ "whoami" | shell }} (me)
- Team members

## Action Items
1. Review sprint ending {{ current_date | add_days(14) | strftime("%Y-%m-%d") }}
2. Prepare for next planning session
```

### Snippets with Templates
```javascript
/**
 * @file {{ title }}
 * @author {{ "whoami" | shell }}
 * @date {{ current_date | strftime("%Y-%m-%d") }}
 */
console.log("Environment: {{ env('NODE_ENV', 'development') }}");
```

## Environment Variables with Templates

The `_env_` tag enables powerful environment management with template interpolation:

```bash
# Create environment variables for development
bkmr add "# Development environment
export DB_HOST=localhost
export DB_PORT=5432
export DB_NAME=myapp_dev
export DB_USER=postgres
export DB_PASSWORD=postgres
export API_URL=http://localhost:3000
export DEBUG=true
export BUILD_DATE={{ current_date | strftime(\"%Y-%m-%d\") }}
" development,database --type env

# Create environment variables for production
bkmr add "# Production environment
export DB_HOST=db.example.com
export DB_PORT=5432
export DB_NAME=myapp_prod
export DB_USER=app_user
export DB_PASSWORD={{ env(\"PROD_DB_PASSWORD\", \"default_pwd\") }}
export API_URL=https://api.example.com
export DEBUG=false
export BUILD_DATE={{ current_date | strftime(\"%Y-%m-%d\") }}
" production,database --type env
```

### Using Environment Variables

To use environment variables stored in bkmr, you can:

```bash
# Source the variables directly
eval "$(bkmr open 123)"

# Or use process substitution
source <(bkmr open 123)

# Create an alias for quick environment switching
alias dev-env="eval \$(bkmr search --np -t development,database)"
alias prod-env="eval \$(bkmr search --np -t production,database)"
```

### Dynamic Values in Environment Variables

Environment variables can include dynamic content through template interpolation:

```bash
# Environment variables with dynamic content
export GIT_BRANCH={{ "git rev-parse --abbrev-ref HEAD" | shell }}
export GIT_COMMIT={{ "git rev-parse HEAD" | shell }}
export TIMESTAMP={{ current_date | strftime("%Y%m%d_%H%M%S") }}
export BUILD_NUMBER={{ env("BUILD_NUMBER", "local") }}
export USER_NAME={{ "whoami" | shell }}
```

This approach is similar to how tools like `z.bash` work, allowing you to source environment variables directly from your terminal.


## Conditional Logic in Templates

Use control structures for more complex templates:

```
{% if env_NODE_ENV == "production" %}
https://api.example.com/v1/data
{% else %}
https://dev-api.example.com/v1/data
{% endif %}
```

## Action-Specific Template Use Cases

Different actions benefit from templates in unique ways:

### Browser Action Templates
- Date-based report URLs
- User or environment-specific dashboards
- Authentication tokens in APIs

### Shell Action Templates
- Environment-aware scripts
- User-specific configurations
- Date-based operations

### Markdown Action Templates
- Dynamic documentation
- Date-stamped notes
- Environment-specific instructions

### Snippet Action Templates
- Context-aware code snippets
- Environment-specific configurations
- User attribution in comments

## Developer Workflow Benefits

Template interpolation enhances your workflow by:

1. **Reducing repetition** - Create one template instead of multiple similar bookmarks
2. **Adding context awareness** - Bookmarks adapt to your environment
3. **Improving automation** - Combine with shell actions for powerful scripts
4. **Enhancing documentation** - Create self-updating reference materials

## Security Considerations

- The `shell` filter executes commands on your system - use with caution
- A safety mechanism prevents executing potentially dangerous commands containing characters like `;`, `|`, `>`, etc.
- Environment access could expose sensitive information in bookmarks

## Troubleshooting

If template resolution fails, `bkmr` will:
1. Show an error message
2. Fall back to the original unresolved template
3. Continue with the bookmark's default action

Common issues:
- Missing environment variables
- Syntax errors in templates
- Shell commands that fail to execute