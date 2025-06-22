# Smart Actions in bkmr

One of `bkmr`'s most powerful features is its context-aware action system. This system automatically determines the appropriate behavior for different content types, making your workflow more efficient.

## What Are Smart Actions?

Smart actions are the behaviors triggered when you "open" or interact with a bookmark. Instead of treating all content the same way, `bkmr` intelligently handles each item according to its type.

## Available Actions

| Action Type | Description | When Used |
|-------------|-------------|-----------|
| Web Browser | Opens URLs in your default browser | URLs and web resources |
| Copy to Clipboard | Copies content for easy pasting | Code snippets and text documents |
| Shell Execution | Interactive editor then runs as shell script | Shell commands and scripts |
| Markdown Rendering | Converts Markdown to HTML and displays | Documentation and notes |
| File Opening | Launches with appropriate application | Local files and directories |

## How Actions Are Resolved

When you execute `bkmr open` or select an item in the fuzzy finder, the action resolution process follows this sequence:

1. Check for system tags to identify the content type
2. Apply the corresponding action for that content type
3. Fall back to the default URI action if no specific type is detected

## Action Types in Detail

### URL/URI Action

This action opens web addresses in your default browser.

```bash
# Add a standard URL
bkmr add https://github.com github,code

# Open it (launches browser)
bkmr open 1
```

The URL action also incorporates template interpolation, allowing for dynamic URLs:

```bash
# Add a template URL with current date
bkmr add "https://reports.example.com/daily/{{ current_date | strftime('%Y-%m-%d') }}" reports,daily
```

### Snippet Action

The snippet action copies content to your clipboard, perfect for code fragments or commands you use regularly.

```bash
# Add a code snippet
bkmr add "function logDebug(msg) { console.log(`[DEBUG] ${msg}`); }" javascript,logging --type snip

# Open it (copies to clipboard)
bkmr open 2
```

Benefits for developers:
- No need to retype common code patterns
- Ensures consistency in code snippets
- Reduces errors from manual typing

### Shell Action

The shell action presents an interactive editor before executing content as a shell script, making it perfect for automation tasks with runtime customization.
See "Shell Execution: Two Different Approaches" below for more details.

```bash
# Add a shell script
bkmr add "#!/bin/bash\necho 'Generating report...'\nls -la | grep '.log' > ~/report.txt" reports,logs --type shell

# Execute it (presents interactive editor first)
bkmr open 3
Execute: #!/bin/bash
echo 'Generating report...'
ls -la | grep '.log' > ~/report.txt
# Edit command to add parameters, then press Enter to execute
```

Benefits:
- Execute complex command sequences with a single action
- Add parameters or modify commands at runtime
- Store environment setup scripts for different projects
- Standardize common operational tasks

### Markdown Action

The Markdown action renders content to HTML and displays it in your browser, perfect for documentation and notes. It can process both direct markdown content and markdown files referenced by path.

```bash
# Add markdown content directly
bkmr add "# Development Setup\n\n## Requirements\n- Node.js\n- Docker\n\n## Steps\n1. Clone repo\n2. Run npm install" 
     setup,dev --type md

# Add a reference to a markdown file
bkmr add "~/documents/project/setup.md" setup,doc --type md

# View it (renders in browser)
bkmr open 4
```

#### Markdown File References

When a bookmark contains a path to a markdown file:

1. The file path is resolved (supporting shell variables, tilde expansion, relative paths)
2. The markdown content is read from the file
3. If `--openai` is enabled and the bookmark is marked as embeddable, the content is processed for embedding
4. The content is rendered as HTML with MathJax support
5. The rendered page is opened in your browser

#### Math Rendering

The markdown action supports rendering LaTeX math formulas using MathJax:

```bash
# Add a markdown document with math formulas
bkmr add "# Statistics\n\n## Formula\n\n$$E = mc^2$$\n\nInline formula: $P(x) = \frac{1}{\sigma\sqrt{2\pi}}e^{-\frac{(x-\mu)^2}{2\sigma^2}}$" math,physics --type md
```

Benefits:
- Beautiful rendering of documentation
- Support for mathematical notation and formulas
- Keeps technical notes accessible
- Works with template variables for dynamic content
- Can load content from local files

### Text Action

The text action copies content to your clipboard, similar to snippets but intended for plain text.

```bash
# Add a text note
bkmr add "Meeting with client scheduled for Tuesday at 2pm to discuss project timeline." meetings,notes --type text

# Open it (copies to clipboard)
bkmr open 5
```

## Interactive Use with FZF

When using the fuzzy finder interface (`bkmr search --fzf`), actions are displayed in the preview panel and can be executed directly:

- `Enter`: Execute the default action for the selected bookmark
- `Ctrl-o`: Same as Enter, also records access
- `Ctrl-y`: Copy URL/content to clipboard
- `Ctrl-e`: Edit the selected bookmark
- `Ctrl-d`: Delete the selected bookmark

## Creating Custom Action Sequences

You can combine actions by creating shell scripts that operate on bookmark content:

```bash
# Add a shell script that uses another bookmark's content
bkmr add "#!/bin/bash\n# Get SQL query from bookmark ID 5 and run it\nquery=\$(bkmr open 5)\npsql -d mydb -c \"\$query\"" database,run --type shell

# Add a shell script that sources environment variables before running commands
bkmr add "#!/bin/bash\n# Source environment variables\neval \"\$(bkmr open 7)\"\n# Run database migration\nnpx prisma migrate dev" migrate,database --type shell
```

## Using Template Interpolation with Actions

All actions support template interpolation, allowing for powerful dynamic content:

```bash
# Markdown with dynamic date and user information
bkmr add "# Report: {{ current_date | strftime('%B %d, %Y') }}\n\nGenerated by: {{ \"whoami\" | shell }}" report,template --type md

# Shell script with environment variable
bkmr add "#!/bin/bash\ncd {{ env('PROJECT_DIR', '~/projects') }}\ngit pull" git,update --type shell

# Environment variables with dynamic date
bkmr add "export TIMESTAMP={{ current_date | strftime('%Y%m%d_%H%M%S') }}\nexport GIT_BRANCH={{ \"git branch --show-current\" | shell }}" deploy,env --type env
```

## Embedding Support for Markdown Files

When using the `--openai` flag, the Markdown action can automatically process file content for semantic search:

```bash
# Add a markdown file with embedding enabled
bkmr --openai add "~/documents/project-specs.md" specifications,project --type md

# Enable embedding for an existing bookmark
bkmr set-embeddable 5 --enable

# Open the markdown (reads file, updates embedding if changed, renders HTML)
bkmr open 5
```

Benefits:
- Content is automatically embedded when it changes
- No need to manually update embeddings when files are modified
- Enables semantic search of your documentation

## Shell Execution: Two Different Approaches

`bkmr` provides two distinct mechanisms for shell execution, each serving different purposes:

### 1. Shell Action (`_shell_` system tag)
The primary way to execute shell scripts.

**Characteristics:**
- Presents interactive editor before execution (configurable)
- Uses vim/emacs bindings based on your shell configuration (detected from `.inputrc`, etc.)
- Commands saved to `~/.config/bkmr/shell_history.txt` for reuse
- Uses the user's preferred shell (`$SHELL` environment variable or `/bin/sh` as fallback)
- Script runs with inherited stdio (connected to user's terminal)
- Supports full interactive scripts
- Executed as standalone scripts with proper permissions
- Uses the entire bookmark content as the script
- Output and errors are displayed in the terminal
- Can be configured for direct execution via `BKMR_SHELL_INTERACTIVE=false`

### 2. Template Shell Filter (`{{ "command" | shell }}`)
For embedding command output within templates/interpolated content.

**Characteristics:**
- Always uses `/bin/sh` regardless of user's default shell
- Restricted by security filters that block potentially harmful commands
- Output is captured and inserted into the template
- Limited to small, non-interactive commands
- Cannot redirections or complex shell constructs

**Example Use Cases:**
- Including current username: `{{ "whoami" | shell }}`
- Git branch in documentation: `{{ "git branch --show-current" | shell }}`
- Inserting system information: `{{ "uname -a" | shell }}`

### Choosing Between Them

- Use `_shell_` tag for standalone scripts that need to be executed with full shell capabilities
- Use `{{ "command" | shell }}` for embedding command output within other content types
- For security-sensitive operations, prefer the template approach as it includes safety checks
- For interactive scripts or those requiring user input, always use the `_shell_` system tag approach

## Developer Workflow Benefits

The smart action system transforms how you work in the terminal:

1. **Reduced context switching** - Execute commands and access information without leaving your terminal
2. **Workflow automation** - Turn repetitive tasks into simple bookmark operations
3. **Documentation flow** - View rendered documentation when needed, copy code when required
4. **Environment management** - Switch between different environment configurations quickly
5. **Consistent environment** - Use the same commands across different projects and contexts

## Tips for Effective Action Use

- **Tag thoughtfully** - Use descriptive tags to easily find the right action
- **Use system tags explicitly** - Add `--type shell` or similar to ensure the correct action
- **Combine with templates** - Make your actions dynamic with template variables
- **Create task-specific collections** - Group related actions with common tags
- **Use shell functions** - Create shell functions that combine multiple actions for complex workflows