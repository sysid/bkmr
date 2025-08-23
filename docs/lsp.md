# bkmr LSP Integration

Language Server Protocol (LSP) implementation for bkmr snippet and command management.

## Overview

The bkmr LSP server provides seamless integration with any LSP-compatible editor, offering automatic snippet completion, language-aware filtering, and additional LSP commands. All processing happens server-side through direct integration with the bkmr services.

**Key Features:**
- **Language-aware filtering**: Snippets automatically filtered by file type (e.g., Rust files get Rust snippets + universal snippets)
- **Universal snippets**: Write snippets once in natural Rust syntax, automatically adapted to target languages
- **Server-side interpolation**: Templates processed using bkmr's interpolation engine
- **LSP commands**: Additional functionality like automatic filepath comment insertion

## Basic Usage

### Starting the LSP Server

```bash
# Start LSP server (typically configured in your editor like nvim)
bkmr lsp

# Disable template interpolation if needed
bkmr lsp --no-interpolation
```

### Snippet Completion

1. **Manual Completion**: Trigger completion manually (typically Ctrl+Space)
2. **Smart Filtering**: Server returns snippets tagged with your language PLUS universal snippets

## Language-Aware Filtering

### Supported Languages

| Language | File Extensions | LSP Language ID | Comment Syntax |
|----------|----------------|-----------------|----------------|
| Rust | `.rs` | `rust` | `//`, `/* */` |
| Python | `.py` | `python` | `#` |
| JavaScript | `.js` | `javascript` | `//`, `/* */` |
| TypeScript | `.ts`, `.tsx` | `typescript` | `//`, `/* */` |
| Go | `.go` | `go` | `//`, `/* */` |
| Java | `.java` | `java` | `//`, `/* */` |
| C/C++ | `.c`, `.cpp`, `.cc` | `c`, `cpp` | `//`, `/* */` |
| Shell | `.sh`, `.bash` | `shell`, `sh` | `#` |
| HTML | `.html` | `html` | `<!-- -->` |
| CSS | `.css` | `css` | `/* */` |
| YAML | `.yaml`, `.yml` | `yaml` | `#` |
| JSON | `.json` | `json` | N/A |
| Markdown | `.md` | `markdown` | `<!-- -->` |
| And many more... | | | |

### Setting up Language-Specific Snippets

```bash
# Tag snippets with language identifiers
bkmr add 'console.log("${1:message}")' javascript,_snip_ --title "JS Console Log"
bkmr add 'println!("${1:message}")' rust,_snip_ --title "Rust Print"

# Universal snippets work across languages
bkmr add '// TODO: ${1:description}' universal,_snip_ --title "TODO Comment"
```

### Query Generation

The LSP server generates optimized queries for bkmr:

```bash
# Rust file completion request:
(tags:rust AND tags:"_snip_") OR (tags:universal AND tags:"_snip_")

# With word filter "hello":
((tags:rust AND tags:"_snip_") OR (tags:universal AND tags:"_snip_")) AND metadata:hello*
```

## Universal Snippets

Universal snippets are written in Rust syntax and automatically translate to target languages:

### Comment Translation
- **Rust/JS/C++**: `// comment` (original)
- **Python/Shell**: `# comment` 
- **HTML/XML**: `<!-- comment -->`

### Block Comment Translation
- **Rust/JS/C++**: `/* block */` (original)
- **Python**: `"""block"""` or `'''block'''`
- **HTML**: `<!-- block -->`

### Indentation Adaptation
- **Rust/Python**: 4 spaces (original)
- **Go**: Tabs
- **JavaScript**: 2 spaces
- **HTML/CSS**: 2 spaces

## LSP Commands

### `bkmr.insertFilepathComment`

Inserts the relative filepath as a language-specific comment at the beginning of the file.

**Features:**
- **Project root detection**: Automatically finds project root using common indicators (Cargo.toml, package.json, .git, etc.)
- **Language-aware comments**: Uses correct comment syntax for each language
- **Relative paths**: Shows path relative to project root, falls back to filename

**Example output:**
```rust
// src/lsp/backend.rs  ← Inserted automatically
use tower_lsp::LanguageServer;
```

## Editor Configuration

### VS Code

Install an LSP extension and add to `settings.json`:

```json
{
  "languageServerExample.servers": {
    "bkmr-lsp": {
      "command": "bkmr",
      "args": ["lsp"],
      "filetypes": ["rust", "javascript", "typescript", "python", "go", "java", "c", "cpp", "html", "css", "scss", "ruby", "php", "swift", "kotlin", "shell", "yaml", "json", "markdown", "xml", "vim"]
    }
  }
}
```

**To disable template interpolation:**
```json
{
  "languageServerExample.servers": {
    "bkmr-lsp": {
      "command": "bkmr",
      "args": ["lsp", "--no-interpolation"],
      "filetypes": ["rust", "javascript", "typescript", "python", "go", "java", "c", "cpp", "html", "css", "scss", "ruby", "php", "swift", "kotlin", "shell", "yaml", "json", "markdown", "xml", "vim"]
    }
  }
}
```

### Neovim with nvim-lspconfig

```lua
require'lspconfig.configs'.bkmr_lsp = {
  default_config = {
    cmd = { "bkmr", "lsp" },
    filetypes = { "rust", "javascript", "typescript", "python", "go", "java", "c", "cpp", "html", "css", "scss", "ruby", "php", "swift", "kotlin", "shell", "yaml", "json", "markdown", "xml", "vim" },
    root_dir = function(fname)
      return require'lspconfig.util'.find_git_ancestor(fname) or vim.fn.getcwd()
    end,
    settings = {},
  },
}

require'lspconfig'.bkmr_lsp.setup{
  capabilities = require('cmp_nvim_lsp').default_capabilities(),
  settings = {
    bkmr = {
      enableIncrementalCompletion = false
    }
  },
  on_attach = function(client, bufnr)
    -- Create custom command for filepath insertion
    vim.api.nvim_create_user_command('BkmrInsertPath', function()
      vim.lsp.buf_request(0, 'workspace/executeCommand', {
        command = "bkmr.insertFilepathComment",
        arguments = { vim.uri_from_bufnr(0) }
      }, function(err, result)
        if err then
          vim.notify("Error: " .. tostring(err), vim.log.levels.ERROR)
        elseif result then
          vim.lsp.util.apply_workspace_edit(result, client.offset_encoding)
        end
      end)
    end, { desc = "Insert filepath comment" })
  end
}
```

**To disable template interpolation:**
```lua
require'lspconfig'.bkmr_lsp.setup{
  cmd = { "bkmr", "lsp", "--no-interpolation" },
  -- ... rest of configuration
}
```

### Vim with vim-lsp

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

### Emacs with lsp-mode

```elisp
(with-eval-after-load 'lsp-mode
  (add-to-list 'lsp-language-id-configuration '(".*" . "text"))
  (lsp-register-client
   (make-lsp-client :new-connection (lsp-stdio-connection '("bkmr" "lsp"))
                    :major-modes '(text-mode)
                    :server-id 'bkmr-lsp)))
```

## Plain Text Snippets

Snippets tagged with "plain" are treated as plain text, preventing LSP clients from interpreting snippet syntax like `$1`, `${2:default}`, etc.

**Use plain text for:**
- **Documentation templates**: Contains `${COMPANY}` or `${VERSION}` that should appear literally
- **Configuration files**: Raw templates with placeholder syntax  
- **Shell scripts**: Variables like `$HOME` that shouldn't be LSP placeholders

```bash
# Plain text snippet (literal insertion)
bkmr add 'Config: ${DATABASE_URL}\nUser: ${USERNAME}' plain,_snip_ --title "Config Template"
```

## Template Interpolation

**Default behavior**: The LSP server uses bkmr's `--interpolate` flag to process templates before serving snippets.

For complete template documentation, see [Template Interpolation](./template-interpolation.md).

### Disabling Interpolation

```bash
# Disable interpolation
bkmr lsp --no-interpolation
```

## Troubleshooting

### No Completions Appearing

1. **Verify bkmr works**: `bkmr search --json 'tags:"_snip_"'`
2. **Check bkmr version**: `bkmr --version` (should be 4.31.0+)
3. **Test LSP server**: `echo '{"jsonrpc":"2.0","method":"initialize","id":1,"params":{}}' | bkmr lsp`

### LSP Placeholders Not Working

**Problem**: Snippet navigation (`$1`, `${2:default}`) doesn't work

**Solutions**:
1. **Check if snippet is plain**: Plain text snippets don't support LSP placeholders
2. **Verify placeholder syntax**: 
   - Simple tabstops: `$1`, `$2`, `$3`
   - Placeholders: `${1:default text}`, `${2:another default}`
   - Choices: `${1|option1,option2,option3|}`

### LSP Server Logging

The LSP server automatically adjusts log levels:
- **LSP mode**: ERROR level (avoids noise in client logs)
- **Terminal mode**: WARN level (for development)

```bash
# Debug logging (appears as ERROR in LSP client)
RUST_LOG=debug bkmr lsp

# Log to file for debugging
RUST_LOG=debug bkmr lsp 2>/tmp/bkmr-lsp.log
```

## Related Projects

- [bkmr-intellij-plugin](https://github.com/sysid/bkmr-intellij-plugin) - IntelliJ Platform integration