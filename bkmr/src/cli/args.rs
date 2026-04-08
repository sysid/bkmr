// src/cli/args.rs
use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser, Clone)]
#[command(author, version, about, long_about = None)]
#[command(arg_required_else_help = true, disable_help_subcommand = true)]
/// Knowledge management for humans and agents — bookmarks, snippets, scripts, and semantic search
pub struct Cli {
    /// Optional name to operate on
    pub name: Option<String>,

    /// Sets a custom config file
    #[arg(short, long, value_name = "FILE")]
    pub config: Option<PathBuf>,

    /// Path to database file (overrides BKMR_DB_URL and config.toml)
    #[arg(long = "db", value_name = "FILE", global = true)]
    pub db: Option<PathBuf>,

    /// Turn debugging information on (-d=info, -dd=debug, -ddd=trace)
    #[arg(short, long, action = clap::ArgAction::Count)]
    pub debug: u8,

    #[arg(long = "no-color", help = "Disable colored output")]
    pub no_color: bool,

    #[arg(
        long = "generate-config",
        help = "bkmr --generate-config > ~/.config/bkmr/config.toml"
    )]
    pub generate_config: bool,

    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Subcommand, Clone)]
pub enum Commands {
    /// Search bookmarks with full-text search and tag filters
    Search {
        /// FTS query (full text search)
        fts_query: Option<String>,

        #[arg(
            short = 'e',
            long = "exact",
            help = "exact tag match (comma-separated)"
        )]
        tags_exact: Option<String>,

        #[arg(long = "exact-prefix", help = "prefix tags combined with --exact")]
        tags_exact_prefix: Option<String>,

        #[arg(short = 't', long = "tags", help = "must have ALL these tags (comma-separated)")]
        tags_all: Option<String>,

        #[arg(long = "tags-prefix", help = "prefix tags combined with --tags")]
        tags_all_prefix: Option<String>,

        #[arg(
            short = 'T',
            long = "Tags",
            help = "exclude if has ALL these tags (comma-separated)"
        )]
        tags_all_not: Option<String>,

        #[arg(long = "Tags-prefix", help = "prefix tags combined with --Tags")]
        tags_all_not_prefix: Option<String>,

        #[arg(short = 'n', long = "ntags", help = "must have ANY of these tags (comma-separated)")]
        tags_any: Option<String>,

        #[arg(long = "ntags-prefix", help = "prefix tags combined with --ntags")]
        tags_any_prefix: Option<String>,

        #[arg(
            short = 'N',
            long = "Ntags",
            help = "exclude if has ANY of these tags (comma-separated)"
        )]
        tags_any_not: Option<String>,

        #[arg(long = "Ntags-prefix", help = "prefix tags combined with --Ntags")]
        tags_any_not_prefix: Option<String>,

        #[arg(short = 'o', long = "descending", help = "sort descending (implies --sort modified if no --sort given)")]
        order_desc: bool,

        #[arg(short = 'O', long = "ascending", help = "sort ascending (implies --sort modified if no --sort given)")]
        order_asc: bool,

        #[arg(long = "sort", help = "sort field: id, title, modified (default: id). Without -o/-O, id/title default ascending, modified defaults descending")]
        sort_field: Option<String>,

        #[arg(long = "np", help = "no prompt")]
        non_interactive: bool,

        #[arg(
            long = "fzf",
            help = "use fuzzy finder: [CTRL-O: copy to clipboard (shell scripts: copy 'bkmr open --no-edit <id> --' command), CTRL-E: edit, CTRL-D: delete, CTRL-A: clone, CTRL-P: show details, ENTER: open]"
        )]
        is_fuzzy: bool,

        #[arg(
            long = "fzf-style",
            help = "fuzzy finder style: classic or enhanced",
            default_value = "classic"
        )]
        fzf_style: Option<String>,

        #[arg(long = "json", help = "non-interactive mode, output as json")]
        is_json: bool,

        #[arg(short = 'l', long = "limit", help = "limit number of results")]
        limit: Option<i32>,

        #[arg(
            long = "interpolate",
            help = "process template interpolation in search results display (not needed for FZF mode or bookmark actions - they interpolate automatically)"
        )]
        interpolate: bool,

        #[arg(
            long = "shell-stubs",
            help = "output shell function stubs for shell script bookmarks (automatically filters for _shell_ type)"
        )]
        shell_stubs: bool,

        #[arg(
            long = "stdout",
            help = "output selected bookmark content to stdout instead of executing (for shell wrapper integration)"
        )]
        stdout: bool,

        #[arg(long = "embeddable", help = "filter to show only embeddable bookmarks")]
        embeddable: bool,
    },
    /// Hybrid search combining full-text and semantic search with RRF fusion
    #[command(name = "hsearch")]
    HSearch {
        /// Search query text
        query: String,

        #[arg(short = 't', long = "tags", help = "must have ALL these tags (comma-separated)")]
        tags_all: Option<String>,

        #[arg(short = 'T', long = "Tags", help = "exclude if has ALL these tags (comma-separated)")]
        tags_all_not: Option<String>,

        #[arg(short = 'n', long = "ntags", help = "must have ANY of these tags (comma-separated)")]
        tags_any: Option<String>,

        #[arg(short = 'N', long = "Ntags", help = "exclude if has ANY of these tags (comma-separated)")]
        tags_any_not: Option<String>,

        #[arg(short = 'e', long = "exact", help = "exact tag match (comma-separated)")]
        tags_exact: Option<String>,

        #[arg(long = "mode", default_value = "hybrid", help = "search mode: hybrid or exact")]
        mode: String,

        #[arg(short = 'l', long = "limit", help = "limit number of results")]
        limit: Option<i32>,

        #[arg(long = "json", help = "output as JSON (includes rrf_score)")]
        is_json: bool,

        #[arg(long = "fzf", help = "use fzf for interactive selection")]
        is_fuzzy: bool,

        #[arg(long = "stdout", help = "output to stdout for piping")]
        stdout: bool,

        #[arg(long = "np", help = "no prompt")]
        non_interactive: bool,
    },
    /// Semantic search using embeddings only
    SemSearch {
        /// Search query (natural language)
        query: String,

        #[arg(short = 'l', long = "limit", help = "limit number of results")]
        limit: Option<i32>,

        #[arg(long = "np", help = "no prompt")]
        non_interactive: bool,
    },
    /// Open bookmark (smart action based on content type)
    Open {
        /// Bookmark IDs (comma-separated) or file path with --file
        ids: String,
        #[arg(long = "no-edit", help = "skip interactive editing for shell scripts")]
        no_edit: bool,
        #[arg(
            long = "file",
            help = "treat ids parameter as file path for direct viewing"
        )]
        file: bool,
        #[arg(
            last = true,
            help = "Arguments to pass to shell scripts (use -- to separate: bkmr open ID -- arg1 arg2)"
        )]
        script_args: Vec<String>,

        #[arg(
            long = "stdout",
            help = "output bookmark content to stdout instead of executing"
        )]
        stdout: bool,
    },
    /// Add a bookmark
    Add {
        /// URL or content to store
        url: Option<String>,
        /// Tags (comma-separated, no spaces)
        tags: Option<String>,
        #[arg(long = "title", help = "bookmark title")]
        title: Option<String>,
        #[arg(short = 'd', long = "description", help = "bookmark description")]
        desc: Option<String>,
        #[arg(long = "no-web", help = "do not fetch URL data")]
        no_web: bool,
        #[arg(short = 'e', long = "edit", help = "edit the bookmark while adding")]
        edit: bool,
        #[arg(
            short = 't',
            long = "type",
            help = "bookmark type (uri, snip, text, shell, md, env)",
            default_value = "uri"
        )]
        bookmark_type: String,
        #[arg(short = 'c', long = "clone", help = "clone an existing bookmark by ID")]
        clone_id: Option<i32>,
        #[arg(long = "stdin", help = "read content from stdin into url field")]
        stdin: bool,
        #[arg(
            long = "open-with",
            help = "custom command to open this bookmark (replaces default open behavior)"
        )]
        open_with: Option<String>,
        #[arg(long = "no-embed", help = "do not generate embedding for semantic search")]
        no_embed: bool,
    },
    /// Delete bookmarks by ID
    Delete {
        /// Bookmark IDs (comma-separated)
        ids: String,
    },
    /// Update bookmark fields non-interactively (tags, title, description, URL, opener)
    Update {
        /// Bookmark IDs (comma-separated)
        ids: String,
        #[arg(short = 't', long = "tags", help = "add tags to taglist")]
        tags: Option<String>,
        #[arg(short = 'n', long = "ntags", help = "remove tags from taglist")]
        tags_not: Option<String>,
        #[arg(short = 'f', long = "force", help = "overwrite taglist with tags")]
        force: bool,
        #[arg(long = "title", help = "set bookmark title")]
        title: Option<String>,
        #[arg(short = 'd', long = "description", help = "set bookmark description")]
        description: Option<String>,
        #[arg(long = "url", help = "set bookmark URL/content")]
        url: Option<String>,
        #[arg(
            long = "open-with",
            help = "set custom command to open this bookmark (use empty string to clear)"
        )]
        open_with: Option<String>,
        #[arg(long = "embed", help = "enable embedding for semantic search")]
        embed: bool,
        #[arg(long = "no-embed", help = "disable embedding for semantic search")]
        no_embed: bool,
    },
    /// Edit bookmarks interactively in $EDITOR (smart: opens source file for imports)
    Edit {
        /// Bookmark IDs (comma-separated)
        ids: String,
        #[arg(
            long = "force-db",
            help = "force edit database content instead of source file for file-imported bookmarks"
        )]
        force_db: bool,
    },
    /// Show bookmark details
    Show {
        /// Bookmark IDs (comma-separated)
        ids: String,
        #[arg(long = "json", help = "output as JSON")]
        is_json: bool,
    },
    /// Open random bookmarks for serendipitous discovery
    Surprise {
        #[arg(short = 'n', help = "number of URLs to open", default_value_t = 1)]
        n: i32,
    },
    /// List all tags (or show related tags for a given tag)
    Tags {
        /// Show tags related to this tag (omit to list all)
        tag: Option<String>,
    },
    /// Initialize bookmark database
    CreateDb {
        /// pathname to database file (optional, uses config path if not provided)
        #[arg(help = "Path where the database will be created (default: ~/.config/bkmr/bkmr.db)")]
        path: Option<String>,

        #[arg(long, help = "Pre-fill the database with demo entries")]
        pre_fill: bool,
    },
    /// Generate missing embeddings for embeddable bookmarks
    Backfill {
        #[arg(short = 'd', long = "dry-run", help = "only show what would be done")]
        dry_run: bool,

        #[arg(
            short = 'f',
            long = "force",
            help = "force recompute of all embeddings (except _imported_)"
        )]
        force: bool,
    },
    /// Clear all embeddings and content hashes (clean slate for backfill)
    ClearEmbeddings {},

    /// Bulk-create bookmarks from JSON array (skips existing Content/URLs, no update support)
    LoadJson {
        /// Path to JSON file: [{url, title, description, tags}, ...]
        #[arg(help = "Path to JSON file with an array of bookmark objects")]
        path: String,

        #[arg(short = 'd', long = "dry-run", help = "only show what would be done")]
        dry_run: bool,
        #[arg(long = "no-embed", help = "do not generate embedding for semantic search")]
        no_embed: bool,
    },

    /// Import files from directories (stores content, tracks source file for smart editing).
    ///
    /// Supported file types: .sh (shell scripts), .py (python scripts), .md (markdown files)
    ///
    /// Required frontmatter format (YAML):
    /// ---
    /// name: "Script Name"        # Required: bookmark title
    /// tags: ["tag1", "tag2"]     # Optional: comma-separated tags  
    /// type: "_shell_"            # Optional: content type (_shell_, _md_, _snip_)
    /// ---
    ///
    /// Or hash-style format (for scripts):
    /// #!/bin/bash
    /// # name: Script Name
    /// # tags: tag1, tag2
    /// # type: _shell_
    ImportFiles {
        /// Directories or files to import
        #[arg(help = "Directories or files to import")]
        paths: Vec<String>,

        #[arg(
            short = 'u',
            long = "update",
            help = "Update existing bookmarks when content differs"
        )]
        update: bool,

        #[arg(
            long = "delete-missing",
            help = "Delete bookmarks whose source files no longer exist"
        )]
        delete_missing: bool,

        #[arg(
            short = 'd',
            long = "dry-run",
            help = "Show what would be done without making changes"
        )]
        dry_run: bool,

        #[arg(
            short = 'v',
            long = "verbose",
            help = "Show detailed information about skipped files and validation issues"
        )]
        verbose: bool,

        #[arg(
            long = "base-path",
            help = "Base path variable name from config (e.g., SCRIPTS_HOME). Paths must be relative to the base path location."
        )]
        base_path: Option<String>,
        #[arg(long = "no-embed", help = "do not generate embedding for semantic search")]
        no_embed: bool,
    },

    /// Show program information and configuration details
    Info {
        #[arg(short = 's', long = "schema", help = "Show database schema")]
        show_schema: bool,
    },
    /// Generate shell completions for bash, zsh, or fish
    Completion {
        /// Shell to generate completions for (bash, zsh, fish)
        shell: String,
    },
    /// Start LSP server for editor snippet completion (VS Code, Neovim, IntelliJ)
    Lsp {
        /// Disable bkmr template interpolation (serve raw templates instead of processed content)
        #[arg(long, help = "Disable bkmr template interpolation")]
        no_interpolation: bool,
    },
    #[command(hide = true)]
    Xxx {
        /// list of ids, separated by comma, no blanks
        ids: String,
        #[arg(short = 't', long = "tags", help = "add tags to taglist")]
        tags: Option<String>,
    },
}
