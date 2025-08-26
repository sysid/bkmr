// src/cli/args.rs
use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
#[command(arg_required_else_help = true, disable_help_subcommand = true)]
/// A bookmark manager for the terminal
pub struct Cli {
    /// Optional name to operate on
    pub name: Option<String>,

    /// Sets a custom config file
    #[arg(short, long, value_name = "FILE")]
    pub config: Option<PathBuf>,

    /// Turn debugging information on
    #[arg(short, long, action = clap::ArgAction::Count)]
    pub debug: u8,

    #[arg(long = "openai", help = "use OpenAI API to embed bookmarks")]
    pub openai: bool,

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

#[derive(Subcommand)]
pub enum Commands {
    /// Searches Bookmarks
    Search {
        /// FTS query (full text search)
        fts_query: Option<String>,

        #[arg(
            short = 'e',
            long = "exact",
            help = "match exact, comma separated list"
        )]
        tags_exact: Option<String>,

        #[arg(long = "exact-prefix", help = "tags to prefix the exact option")]
        tags_exact_prefix: Option<String>,

        #[arg(short = 't', long = "tags", help = "match all, comma separated list")]
        tags_all: Option<String>,

        #[arg(long = "tags-prefix", help = "tags to prefix the tags option")]
        tags_all_prefix: Option<String>,

        #[arg(
            short = 'T',
            long = "Tags",
            help = "not match all, comma separated list"
        )]
        tags_all_not: Option<String>,

        #[arg(long = "Tags-prefix", help = "tags to prefix the Tags option")]
        tags_all_not_prefix: Option<String>,

        #[arg(short = 'n', long = "ntags", help = "match any, comma separated list")]
        tags_any: Option<String>,

        #[arg(long = "ntags-prefix", help = "tags to prefix the ntags option")]
        tags_any_prefix: Option<String>,

        #[arg(
            short = 'N',
            long = "Ntags",
            help = "not match any, comma separated list"
        )]
        tags_any_not: Option<String>,

        #[arg(long = "Ntags-prefix", help = "tags to prefix the Ntags option")]
        tags_any_not_prefix: Option<String>,

        #[arg(short = 'o', long = "descending", help = "order by age, descending")]
        order_desc: bool,

        #[arg(short = 'O', long = "ascending", help = "order by age, ascending")]
        order_asc: bool,

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
    },
    /// Semantic Search with OpenAI
    SemSearch {
        /// Input for similarity search (search terms)
        query: String,

        #[arg(short = 'l', long = "limit", help = "limit number of results")]
        limit: Option<i32>,

        #[arg(long = "np", help = "no prompt")]
        non_interactive: bool,
    },
    /// Open/launch bookmarks or view files
    Open {
        /// list of ids, separated by comma, no blanks OR file path when used with --file
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
    },
    /// Add a bookmark
    Add {
        url: Option<String>,
        /// list of tags, separated by comma, no blanks in between
        tags: Option<String>,
        #[arg(long = "title", help = "title")]
        title: Option<String>,
        #[arg(short = 'd', long = "description", help = "title")]
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
    },
    /// Delete bookmarks
    Delete {
        /// list of ids, separated by comma, no blanks
        ids: String,
    },
    /// Update bookmarks
    Update {
        /// list of ids, separated by comma, no blanks
        ids: String,
        #[arg(short = 't', long = "tags", help = "add tags to taglist")]
        tags: Option<String>,
        #[arg(short = 'n', long = "ntags", help = "remove tags from taglist")]
        tags_not: Option<String>,
        #[arg(short = 'f', long = "force", help = "overwrite taglist with tags")]
        force: bool,
    },
    /// Edit bookmarks
    Edit {
        /// Edit bookmarks, list of ids, separated by comma, no blanks
        ids: String,
        #[arg(
            long = "force-db",
            help = "force edit database content instead of source file for file-imported bookmarks"
        )]
        force_db: bool,
    },
    /// Show Bookmarks (list of ids, separated by comma, no blanks)
    Show {
        ids: String,
        #[arg(long = "json", help = "output as json")]
        is_json: bool,
    },
    /// Opens n random URLs
    Surprise {
        #[arg(short = 'n', help = "number of URLs to open", default_value_t = 1)]
        n: i32,
    },
    /// Tag for which related tags should be shown. No input: all tags are printed
    Tags {
        /// Tag for which related tags should be shown. No input: all tags are shown
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
    /// Set whether a bookmark can be embedded (used for semantic search)
    SetEmbeddable {
        /// ID of the bookmark
        id: i32,

        #[arg(long = "enable", help = "Enable embedding for this bookmark")]
        enable: bool,

        #[arg(long = "disable", help = "Disable embedding for this bookmark")]
        disable: bool,
    },
    /// Backfill embeddings for bookmarks, which have been added without embeddings.
    /// E.g. when OpenAI API was not available.
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
    /// Load bookmarks from JSON array file
    LoadJson {
        /// Path to the JSON file containing an array of bookmark objects
        #[arg(help = "Path to JSON file with an array of bookmark objects")]
        path: String,

        #[arg(short = 'd', long = "dry-run", help = "only show what would be done")]
        dry_run: bool,
    },

    /// Load texts for semantic similarity search as bookmarks.
    /// The actual content of the file is not stored in the database, only the embeddings.
    LoadTexts {
        #[arg(short = 'd', long = "dry-run", help = "only show what would be done")]
        dry_run: bool,

        #[arg(
            short = 'f',
            long = "force",
            help = "force update embeddings even if content has not changed"
        )]
        force: bool,

        /// Path to NDJSON file containing text documents (one per line)
        #[arg(help = "Path to NDJSON file with text documents (one JSON object per line)")]
        path: String,
    },

    /// Import files from directories, parsing frontmatter metadata.
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
    /// Start LSP (Language Server Protocol) server for snippet completion
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
