# bkmr LSP Server Documentation

The bkmr Language Server Protocol (LSP) server provides intelligent snippet completion and command execution for any LSP-compatible editor.

## Quick Start

```bash
# Start LSP server (usually configured in your editor)
bkmr lsp

# Start without template interpolation
bkmr lsp --no-interpolation
```

## Features

### Core Capabilities

- **Smart Snippet Completion**: Context-aware snippet suggestions based on file type
- **Language-Aware Filtering**: Automatically filters snippets by programming language
- **Universal Snippets**: Write once, adapt to any language automatically
- **Template Interpolation**: Server-side template processing with Jinja2
- **LSP Commands**: Execute bkmr operations directly from your editor

### Available LSP Commands

| Command | Description | Parameters | Example |
|---------|-------------|------------|---------|
| `bkmr.createSnippet` | Create new snippet | `url` (required)<br>`title` (required)<br>`description` (optional)<br>`tags` (optional) | `{"url": "console.log('test')", "title": "Log", "tags": ["js"]}` |
| `bkmr.updateSnippet` | Update existing snippet | `id` (required)<br>`url` (optional)<br>`title` (optional)<br>`description` (optional)<br>`tags` (optional) | `{"id": 123, "title": "Updated"}` |
| `bkmr.getSnippet` | Get snippet by ID | `id` (required) | `{"id": 123}` |
| `bkmr.deleteSnippet` | Delete snippet | `id` (required) | `{"id": 123}` |
| `bkmr.listSnippets` | List snippets | `language` (optional) | `{"language": "rust"}` |
| `bkmr.insertFilepathComment` | Insert file path comment | Direct string URI | `"file:///path/to/file.rs"` |

## Editor Integration

### Recommended: bkmr-nvim Plugin

For Neovim users, the dedicated [bkmr-nvim](https://github.com/sysid/bkmr-nvim) plugin provides the best integration experience:

**Features:**
- Automatic LSP configuration
- Custom picker interface for snippet browsing
- Visual snippet editing
- Integrated with Telescope/FZF
- Zero configuration required

**Installation with lazy.nvim:**
```lua
{
  "sysid/bkmr-nvim",
  dependencies = { "nvim-lua/plenary.nvim" },
  config = function()
    require("bkmr").setup({
      -- Optional configuration
      lsp = {
        cmd = { "bkmr", "lsp" },
        filetypes = { "*" }, -- Enable for all file types
      }
    })
  end,
}
```

### Manual Neovim Configuration (nvim-lspconfig)

If you prefer manual configuration, here's a complete setup:

```lua
-- ~/.config/nvim/lua/lsp/bkmr.lua
local lspconfig = require('lspconfig')

-- Define the bkmr LSP configuration
lspconfig.configs.bkmr_lsp = {
  default_config = {
    cmd = { "bkmr", "lsp" },
    filetypes = { 
      "rust", "python", "javascript", "typescript", "go", 
      "java", "c", "cpp", "html", "css", "shell", "yaml", 
      "json", "markdown", "vim" 
    },
    root_dir = function(fname)
      return lspconfig.util.find_git_ancestor(fname) 
        or vim.fn.getcwd()
    end,
    settings = {},
  },
}

-- Setup with capabilities and custom commands
lspconfig.bkmr_lsp.setup{
  capabilities = require('cmp_nvim_lsp').default_capabilities(),
  on_attach = function(client, bufnr)
    -- Custom command for filepath insertion
    vim.api.nvim_buf_create_user_command(bufnr, 'BkmrInsertPath', 
      function()
        vim.lsp.buf.execute_command({
          command = "bkmr.insertFilepathComment",
          arguments = { vim.uri_from_bufnr(bufnr) }
        })
      end, 
      { desc = "Insert filepath comment" }
    )
    
    -- List snippets command
    vim.api.nvim_buf_create_user_command(bufnr, 'BkmrListSnippets',
      function()
        local filetype = vim.bo[bufnr].filetype
        vim.lsp.buf.execute_command({
          command = "bkmr.listSnippets",
          arguments = { { language = filetype } }
        })
      end,
      { desc = "List snippets for current language" }
    )
  end
}
```

### VS Code Configuration

Add to `settings.json`:

```json
{
  "languageServerExample.servers": {
    "bkmr-lsp": {
      "command": "bkmr",
      "args": ["lsp"],
      "filetypes": ["*"]
    }
  }
}
```

### Vim (vim-lsp)

```vim
if executable('bkmr')
  augroup LspBkmr
    autocmd!
    autocmd User lsp_setup call lsp#register_server({
      \ 'name': 'bkmr-lsp',
      \ 'cmd': {server_info->['bkmr', 'lsp']},
      \ 'allowlist': ['rust', 'javascript', 'typescript', 'python', 'go', 'java', 'c', 'cpp', 'html', 'css', 'scss', 'ruby', 'php', 'swift', 'kotlin', 'shell', 'yaml', 'json', 'markdown', 'xml', 'vim'],
      \ })
  augroup END
endif
```

## Language Support

### Automatic Language Detection

The LSP server automatically detects file types and filters snippets accordingly:

| Language | Extensions | LSP ID | Tags | Comment Style |
|----------|------------|--------|------|---------------|
| Rust | `.rs` | `rust` | `rust` | `//`, `/* */` |
| Python | `.py` | `python` | `python`, `py` | `#` |
| JavaScript | `.js`, `.mjs` | `javascript` | `javascript`, `js` | `//`, `/* */` |
| TypeScript | `.ts`, `.tsx` | `typescript` | `typescript`, `ts` | `//`, `/* */` |
| Go | `.go` | `go` | `go`, `golang` | `//`, `/* */` |
| Shell | `.sh`, `.bash` | `shell`, `sh` | `shell`, `sh`, `bash` | `#` |
| HTML | `.html` | `html` | `html` | `<!-- -->` |
| CSS | `.css` | `css` | `css` | `/* */` |
| YAML | `.yaml`, `.yml` | `yaml` | `yaml`, `yml` | `#` |
| Markdown | `.md` | `markdown` | `markdown`, `md` | `<!-- -->` |

### Creating Language-Specific Snippets

```bash
# Language-specific snippets
bkmr add 'println!("{}", $1)' rust,_snip_ --title "Rust Print"
bkmr add 'console.log($1)' javascript,_snip_ --title "JS Log"

# Universal snippets (work in any language)
bkmr add '// TODO: $1' universal,_snip_ --title "TODO"
```

## Universal Snippet System

Universal snippets automatically adapt to the target language's syntax:

### Comment Translation

**Original (Rust syntax):**
```rust
// This is a comment
/* Block comment */
```

**Auto-translated to:**
- **Python/Shell**: `# This is a comment`
- **HTML/XML**: `<!-- This is a comment -->`
- **CSS**: `/* This is a comment */`

### Indentation Adaptation

- **Rust/Python**: 4 spaces (default)
- **Go**: Tabs
- **JavaScript/TypeScript**: 2 spaces
- **HTML/CSS**: 2 spaces

### Example Universal Snippet

```bash
# Create once
bkmr add '// ${1:Function name}\n// ${2:Description}\nfn ${1}() {\n    ${3:// Implementation}\n}' \
  universal,_snip_ --title "Function Template"
```

This snippet will automatically adapt comment syntax and indentation for each language.

## Template Interpolation

The LSP server processes templates before serving snippets:

```bash
# With interpolation (default)
bkmr lsp

# Without interpolation
bkmr lsp --no-interpolation
```

Templates support:
- Environment variables: `{{ env("HOME") }}`
- Date/time: `{{ current_date }}`
- Shell commands: `{{ "date +%Y" | shell }}`

See [Template Interpolation](./template-interpolation.md) for full documentation.

## Plain Text Snippets

Tag snippets with `plain` to prevent LSP placeholder interpretation:

```bash
# Plain text - no LSP placeholders
bkmr add 'DATABASE_URL=${DATABASE_URL}' plain,_snip_ --title "Env Template"

# LSP snippet - with placeholders
bkmr add 'DATABASE_URL=${1:localhost}' _snip_ --title "Env Config"
```

## Troubleshooting

### Common Issues

**No completions appearing:**
```bash
# Check snippets exist
bkmr search -t _snip_

# Test LSP server
echo '{"jsonrpc":"2.0","method":"initialize","id":1,"params":{}}' | bkmr lsp

# Check version (needs 4.31.0+)
bkmr --version
```

**Debugging LSP:**
```bash
# Enable debug logging
RUST_LOG=debug bkmr lsp 2>/tmp/bkmr-lsp.log

# Watch log in another terminal
tail -f /tmp/bkmr-lsp.log
```

**Snippets not filtered by language:**
- Ensure snippets are tagged with language names
- Check that file type is detected correctly
- Verify LSP client sends correct language ID

### Testing LSP Commands

Use the provided test scripts:

```bash
# List all snippets
python3 scripts/lsp/list_snippets.py

# List by language
python3 scripts/lsp/list_snippets.py --language rust

# Get specific snippet
python3 scripts/lsp/get_snippet.py 123

# Show available commands
python3 scripts/lsp/show_commands.py
```

## Related Projects

- [bkmr-nvim](https://github.com/sysid/bkmr-nvim) - Neovim plugin with visual interface
- [bkmr-intellij-plugin](https://github.com/sysid/bkmr-intellij-plugin) - IntelliJ Platform integration
- [bkmr CLI](https://github.com/sysid/bkmr) - Main bkmr project

## Architecture Notes

The LSP server is built into the bkmr binary and:
- Uses tower-lsp for protocol implementation
- Integrates directly with bkmr's service layer
- Performs all filtering server-side for efficiency
- Supports async operations via tokio runtime
- Maintains stateless operation for reliability

For implementation details, see the [LSP test scripts](../scripts/lsp/README.md).