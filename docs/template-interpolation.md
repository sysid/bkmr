# Template Interpolation in bkmr

`bkmr` supports powerful template interpolation for URLs and commands, allowing you to create dynamic bookmarks that adapt to context or incorporate variables.

## Basic Syntax

Templates use Jinja2-style syntax with `{{ variable }}` for expressions and `{% statement %}` for control structures.

## Available Variables

When resolving a template, the following variables are available:

### Bookmark Properties
- `id`: The bookmark's ID
- `title`: The bookmark's title
- `description`: The bookmark's description
- `tags`: List of the bookmark's tags
- `access_count`: Number of times the bookmark has been accessed
- `created_at`: When the bookmark was created (ISO format)
- `updated_at`: When the bookmark was last updated (ISO format)

### System Variables
- `current_date`: Current date and time in ISO format
- Environment variables: Access via `env_VARIABLE_NAME` (e.g., `env_HOME`)

## Filters

Custom filters can transform variables:

- `{{ created_at | strftime("%Y-%m-%d") }}`: Format dates
- `{{ created_at | subtract_days(7) }}`: Date arithmetic
- `{{ created_at | add_days(30) }}`: Date arithmetic
- `{{ "command" | shell }}`: Execute shell command and use its output

## Environment Variable Function

Access environment variables with default fallbacks:

```
{{ env("HOME", "/default/home") }}
```

## Examples

### Date-Based URLs

```
https://example.com/reports/{{ current_date | strftime("%Y-%m-%d") }}
```

### Including Query Parameters

```
https://github.com/search?q={{ "searchterm" }}&type=repositories
```

### File Paths with Environment Variables

```
{{ env_HOME }}/documents/{{ title }}.md
```

### Incorporating Shell Command Output

```
https://example.com/status/{{ "hostname" | shell }}
```

### Dynamic Documentation URLs

```
https://docs.python.org/3/library/{{ env("PYTHON_MODULE", "datetime") }}.html
```

## Security Considerations

- The `shell` filter executes commands on your system - use with caution
- A safety mechanism prevents executing potentially dangerous commands containing characters like `;`, `|`, `>`, etc.
- Environment access could expose sensitive information in bookmarks

## Advanced Usage

For very complex cases, you can create bookmark templates that combine multiple interpolation features:

```
{% if env_CI == "true" %}
https://ci.example.com/builds/{{ "git rev-parse HEAD" | shell }}
{% else %}
file://{{ env_HOME }}/projects/current
{% endif %}
```

## Troubleshooting

If template resolution fails, `bkmr` will use the original unresolved URL as a fallback and display an error message.