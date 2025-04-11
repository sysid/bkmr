# Template Interpolation in bkmr

`bkmr` incorporates a powerful template system that allows your bookmarks to contain dynamic content. This enables everything from date-based URLs to environment-aware shell scripts and context-sensitive documentation.

## How It Works

Templates are processed when content is accessed (through `open`, `search --fzf`, etc.) and replaced with actual values. This happens just before the content's action is executed.

## Template Syntax

Templates use a Jinja2-inspired syntax:
- `{{ expression }}` - For outputting values
- `{% statement %}` - For control structures like conditionals

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