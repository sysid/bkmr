// src/cli/args.rs
use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
/// A bookmark manager for the terminal
pub struct Cli {
    /// Optional name to operate on
    name: Option<String>,

    /// Sets a custom config file
    #[arg(short, long, value_name = "FILE")]
    config: Option<PathBuf>,

    /// Turn debugging information on
    #[arg(short, long, action = clap::ArgAction::Count)]
    pub debug: u8,

    #[arg(long = "openai", help = "use OpenAI API to embed bookmarks")]
    pub openai: bool,

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

        #[arg(short = 't', long = "tags", help = "match all, comma separated list")]
        tags_all: Option<String>,

        #[arg(
        short = 'T',
        long = "Tags",
        help = "not match all, comma separated list"
        )]
        tags_all_not: Option<String>,

        #[arg(short = 'n', long = "ntags", help = "match any, comma separated list")]
        tags_any: Option<String>,

        #[arg(
        short = 'N',
        long = "Ntags",
        help = "not match any, comma separated list"
        )]
        tags_any_not: Option<String>,

        #[arg(long = "prefix", help = "tags to prefix the tags option")]
        tags_prefix: Option<String>,

        #[arg(short = 'o', long = "descending", help = "order by age, descending")]
        order_desc: bool,

        #[arg(short = 'O', long = "ascending", help = "order by age, ascending")]
        order_asc: bool,

        #[arg(long = "np", help = "no prompt")]
        non_interactive: bool,

        #[arg(
        long = "fzf",
        help = "use fuzzy finder: [CTRL-O: open, CTRL-E: edit, ENTER: open]"
        )]
        is_fuzzy: bool,

        #[arg(long = "json", help = "non-interactive mode, output as json")]
        is_json: bool,

        #[arg(short = 'l', long = "limit", help = "limit number of results")]
        limit: Option<i32>,
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
    /// Open/launch bookmarks
    Open {
        /// list of ids, separated by comma, no blanks
        ids: String,
    },
    /// Add a bookmark
    Add {
        url: String,
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
    },
    /// Show Bookmarks (list of ids, separated by comma, no blanks)
    Show { ids: String },
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
        /// pathname to database file
        path: String,
    },
    /// Backfill embeddings for bookmarks, which have been added without embeddings.
    /// E.g. when OpenAI API was not available.
    Backfill {
        #[arg(short = 'd', long = "dry-run", help = "only show what would be done")]
        dry_run: bool,
    },
    /// Load texts for semantic similarity search as bookmarks.
    /// The actual content of the file is not stored in the database, only the embeddings.
    LoadTexts {
        #[arg(short = 'd', long = "dry-run", help = "only show what would be done")]
        dry_run: bool,
        /// pathname to ndjson file
        path: String,
    },
    #[command(hide = true)]
    Xxx {
        /// list of ids, separated by comma, no blanks
        ids: String,
        #[arg(short = 't', long = "tags", help = "add tags to taglist")]
        tags: Option<String>,
    },
}