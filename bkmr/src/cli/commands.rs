use std::fs::create_dir_all;
use std::io::Write;

use crate::adapter::embeddings::{cosine_similarity, deserialize_embedding, OpenAiEmbedding};
use crate::cli::args::{Cli, Commands};
use crate::context::Context;
use crate::service::process::DisplayField;
use crate::{
    adapter::dal::Dal,
    adapter::json::{bms_to_json, read_ndjson_file_and_create_bookmarks},
    environment::CONFIG,
    load_url_details,
    model::{
        bms::Bookmarks,
        bookmark::{BookmarkBuilder, BookmarkUpdater},
        tag::Tags,
    },
    service::{
        self,
        embeddings::create_embeddings_for_non_bookmarks,
        fzf::fzf_process,
        process::{
            delete_bms, edit_bms, open_bm, show_bms, DisplayBookmark, ALL_FIELDS, DEFAULT_FIELDS,
        },
    },
};
use anyhow::{anyhow, Context as _};
use camino::Utf8Path;
use crossterm::style::Stylize;
use diesel::connection::SimpleConnection;
use diesel::result::DatabaseErrorKind;
use diesel::result::Error::DatabaseError;
use diesel_migrations::MigrationHarness;
use itertools::Itertools;
use termcolor::{Color, ColorSpec, StandardStream, WriteColor};
use tracing::{debug, info, instrument};
use crate::adapter::dal::migration::{init_db, MIGRATIONS};
use crate::util::helper::{confirm, ensure_int_vector};

// Type alias for commonly used Result type
type Result<T> = anyhow::Result<T>;

pub fn execute_command(stderr: StandardStream, cli: Cli) -> anyhow::Result<()> {
    match cli.command {
        Some(Commands::Search {
            fts_query,
            tags_exact,
            tags_all,
            tags_all_not,
            tags_any,
            tags_any_not,
            tags_prefix,
            order_desc,
            order_asc,
            non_interactive,
            is_fuzzy,
            is_json,
            limit,
        }) => search_bookmarks(
            tags_prefix,
            tags_all,
            fts_query,
            tags_any,
            tags_all_not,
            tags_any_not,
            tags_exact,
            order_desc,
            order_asc,
            is_fuzzy,
            is_json,
            limit,
            non_interactive,
            stderr,
        ),
        Some(Commands::SemSearch {
            query,
            limit,
            non_interactive,
        }) => sem_search(query, limit, non_interactive, stderr),
        Some(Commands::Open { ids }) => open_bookmarks(ids),
        Some(Commands::Add {
            url,
            tags,
            title,
            desc,
            no_web,
            edit,
        }) => add_bookmark(url, tags, title, desc, no_web, edit),
        Some(Commands::Delete { ids }) => delete_bookmarks(ids),
        Some(Commands::Update {
            ids,
            tags,
            tags_not,
            force,
        }) => update_bookmarks(force, tags, tags_not, ids),
        Some(Commands::Edit { ids }) => edit_bookmarks(ids),
        Some(Commands::Show { ids }) => show_bookmarks(ids),
        Some(Commands::Tags { tag }) => show_tags(tag),
        Some(Commands::CreateDb { path }) => create_db(path),
        Some(Commands::Surprise { n }) => randomized(n),
        Some(Commands::Backfill { dry_run }) => backfill_embeddings(dry_run),
        Some(Commands::LoadTexts { dry_run, path }) => load_texts(dry_run, path),
        Some(Commands::Xxx { ids, tags }) => {
            eprintln!(
                "ids: {:?}, tags: {:?}",
                ids,
                tags
            );
            Ok(())
        }
        None => Ok(()),
    }
}
// Helper function to get and validate IDs
fn get_ids(ids: String) -> Result<Vec<i32>> {
    ensure_int_vector(&ids.split(',').map(String::from).collect())
        .ok_or_else(|| anyhow!("Invalid input, only numbers allowed"))
}

#[instrument]
pub fn search_bookmarks(
    tags_prefix: Option<String>,
    tags_all: Option<String>,
    fts_query: Option<String>,
    tags_any: Option<String>,
    tags_all_not: Option<String>,
    tags_any_not: Option<String>,
    tags_exact: Option<String>,
    order_desc: bool,
    order_asc: bool,
    is_fuzzy: bool,
    is_json: bool,
    limit: Option<i32>,
    non_interactive: bool,
    mut stderr: StandardStream,
) -> Result<()> {
    let mut fields = DEFAULT_FIELDS.to_vec();

    // Combine prefix tags with tags_all if present
    let tags_all = tags_prefix.map_or(tags_all.clone().unwrap_or_default(), |prefix| {
        tags_all.map_or(prefix.clone(), |all| format!("{},{}", all, prefix))
    });

    let mut bms = Bookmarks::new(fts_query.unwrap_or_default());
    bms.filter(
        Some(tags_all),
        tags_any,
        tags_all_not,
        tags_any_not,
        tags_exact,
    );

    // Sort bookmarks based on order flags
    match (order_desc, order_asc) {
        (true, false) => {
            bms.bms
                .sort_by(|a, b| b.last_update_ts.cmp(&a.last_update_ts));
            fields.push(DisplayField::LastUpdateTs);
        }
        (false, true) => {
            bms.bms
                .sort_by(|a, b| a.last_update_ts.cmp(&b.last_update_ts));
            fields.push(DisplayField::LastUpdateTs);
        }
        _ => bms.bms.sort_by_key(|bm| bm.metadata.to_lowercase()),
    }

    // Apply limit if specified
    if let Some(limit) = limit {
        bms.bms.truncate(limit as usize);
    }

    // Handle different output modes
    match (is_fuzzy, is_json) {
        (true, _) => {
            fzf_process(&bms.bms);
            return Ok(());
        }
        (_, true) => {
            bms_to_json(&bms.bms);
            return Ok(());
        }
        _ => {
            let d_bms: Vec<DisplayBookmark> = bms.bms.iter().map(DisplayBookmark::from).collect();
            show_bms(&d_bms, &fields);
            eprintln!("Found {} bookmarks", bms.bms.len());

            if non_interactive {
                let ids = bms
                    .bms
                    .iter()
                    .map(|bm| bm.id.to_string())
                    .sorted()
                    .join(",");
                println!("{}", ids);
            } else {
                stderr.set_color(ColorSpec::new().set_fg(Some(Color::Green)))?;
                writeln!(&mut stderr, "Selection: ")?;
                stderr.reset()?;
                service::process::process(&bms.bms);
            }
        }
    }
    Ok(())
}

#[instrument]
pub fn open_bookmarks(ids: String) -> Result<()> {
    let mut dal = Dal::new(CONFIG.db_url.clone());
    for id in get_ids(ids)? {
        open_bm(&dal.get_bookmark_by_id(id)?)?;
    }
    Ok(())
}

#[instrument]
pub fn add_bookmark(
    url: String,
    tags: Option<String>,
    title: Option<String>,
    desc: Option<String>,
    no_web: bool,
    edit: bool,
) -> Result<()> {
    let mut dal = Dal::new(CONFIG.db_url.clone());

    // Check for unknown tags
    let unknown_tags = Bookmarks::new(String::new())
        .check_tags(Tags::normalize_tag_string(tags.clone()))
        .context("Failed to check tags")?;

    if !unknown_tags.is_empty() && !confirm(&format!("Unknown tags: {:?}, create?", unknown_tags)) {
        return Err(anyhow!("Operation aborted by user"));
    }

    // Get web details if needed
    let (web_title, web_desc, _) = if !no_web {
        load_url_details(&url).unwrap_or_default()
    } else {
        Default::default()
    };

    let mut bm = BookmarkBuilder::new()
        .id(1)
        .URL(url.clone())
        .metadata(title.unwrap_or(web_title))
        .tags(Tags::create_normalized_tag_string(tags))
        .desc(desc.unwrap_or(web_desc))
        .flags(0)
        .build();
    bm.update();

    let result = dal.insert_bookmark(bm.convert_to_new_bookmark());
    match result {
        Ok(bms) => {
            if edit {
                edit_bms(vec![1], bms.clone()).context("Failed to edit bookmark")?;
            }
            println!("Added bookmark: {}", bms[0].id);
            show_bms(
                &bms.iter().map(DisplayBookmark::from).collect::<Vec<_>>(),
                &DEFAULT_FIELDS,
            );
            Ok(())
        }
        Err(e) => {
            if let Some(DatabaseError(DatabaseErrorKind::UniqueViolation, _)) =
                e.downcast_ref::<diesel::result::Error>()
            {
                Err(anyhow!("Bookmark already exists: {}", url))
            } else {
                Err(e)
            }
        }
    }
}

#[instrument]
pub fn delete_bookmarks(ids: String) -> Result<()> {
    let ids = get_ids(ids)?;
    delete_bms(ids, Bookmarks::new(String::new()).bms).context("Failed to delete bookmarks")
}

#[instrument]
pub fn update_bookmarks(
    force: bool,
    tags: Option<String>,
    tags_not: Option<String>,
    ids: String,
) -> Result<()> {
    // Validate force update requirements
    if force && (tags.is_none() || tags_not.is_some()) {
        return Err(anyhow!("Force update requires tags but no ntags"));
    }

    let ids = get_ids(ids)?;
    let tags = Tags::normalize_tag_string(tags);
    let tags_not = Tags::normalize_tag_string(tags_not);

    crate::update_bookmarks(ids, tags, tags_not, force).context("Failed to update bookmarks")
}

#[instrument]
pub fn edit_bookmarks(ids: String) -> Result<()> {
    edit_bms(get_ids(ids)?, Bookmarks::new(String::new()).bms).context("Failed to edit bookmarks")
}

#[instrument]
pub fn show_bookmarks(ids: String) -> Result<()> {
    let mut dal = Dal::new(CONFIG.db_url.clone());
    let mut bms = Vec::new();

    for id in get_ids(ids)? {
        if let Ok(bm) = dal.get_bookmark_by_id(id) {
            bms.push(bm);
        } else {
            eprintln!("Bookmark with id {} not found", id);
        }
    }

    show_bms(
        &bms.iter().map(DisplayBookmark::from).collect::<Vec<_>>(),
        &ALL_FIELDS,
    );
    Ok(())
}

#[instrument]
pub fn show_tags(tag: Option<String>) -> Result<()> {
    let mut dal = Dal::new(CONFIG.db_url.clone());
    let tags = match tag {
        Some(ref tag) => dal.get_related_tags(tag),
        None => dal.get_all_tags(),
    }?;

    for tag in tags {
        println!("{}: {}", tag.n, tag.tag);
    }
    Ok(())
}

#[instrument]
pub fn create_db(path: String) -> Result<()> {
    let path = Utf8Path::new(&path);
    if path.exists() {
        return Err(anyhow!("Database already exists at {:?}", path));
    }

    if let Some(parent) = path.parent() {
        create_dir_all(parent).context("Failed to create parent directories")?;
    }

    let mut dal = Dal::new(path.to_string());
    init_db(&mut dal.conn).context("Failed to initialize database")?;
    dal.clean_table().context("Failed to clean table")?;

    println!("Database created at {:?}", path);
    Ok(())
}

#[instrument]
pub fn randomized(n: i32) -> Result<()> {
    let mut dal = Dal::new(CONFIG.db_url.clone());
    let bms = dal.get_randomized_bookmarks(n)?;

    for bm in &bms {
        open::that(&bm.URL).with_context(|| format!("Failed to open URL: {}", bm.URL))?;
    }
    Ok(())
}

#[instrument(level = "debug")]
pub fn enable_embeddings_if_required() -> Result<()> {
    let mut dal = Dal::new(CONFIG.db_url.clone());

    if dal.check_embedding_column_exists()? {
        info!("Embedding column already exists, no action required.");
        return Ok(());
    }

    eprintln!("New 'bkmr' version requires an extension of the database schema.");
    eprintln!("Two new columns will be added to the 'bookmarks' table:");

    if !confirm("Please backup up your DB before continue! Do you want to continue?") {
        return Err(anyhow!("Operation aborted by user"));
    }

    // Create migrations table if it doesn't exist
    if !dal.check_schema_migrations_exists()? {
        const MIGRATION_TABLE_SQL: &str = r#"
            BEGIN TRANSACTION;
            CREATE TABLE IF NOT EXISTS __diesel_schema_migrations (
                version VARCHAR(50) PRIMARY KEY NOT NULL,
                run_on TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
            );
            INSERT INTO __diesel_schema_migrations (version, run_on)
            VALUES ('20221229110455', '2023-12-23 09:27:06');
            COMMIT;
        "#;

        info!("Creating migration table... {:?}", dal);
        dal.conn
            .batch_execute(MIGRATION_TABLE_SQL)
            .context("Failed to create migrations table")?;
    }

    let pending = dal
        .conn
        .pending_migrations(MIGRATIONS)
        .map_err(|e| anyhow::anyhow!("Failed to get pending migrations: {}", e))?;

    pending.iter().for_each(|m| {
        debug!("Pending Migration: {}", m.name());
    });

    dal.conn
        .run_pending_migrations(MIGRATIONS)
        .map_err(|e| anyhow::anyhow!("Failed to run pending migrations: {}", e))?;

    eprintln!("{}", "Database schema has been extended.".blue());
    Ok(())
}

#[instrument]
pub fn backfill_embeddings(dry_run: bool) -> Result<()> {
    let mut dal = Dal::new(CONFIG.db_url.clone());
    let bms = dal.get_bookmarks_without_embedding()?;

    for bm in &bms {
        println!("Updating: {:?}", bm.metadata);
        if !dry_run {
            let mut bm = bm.clone();
            bm.update();
            dal.update_bookmark(bm)?;
        }
    }
    Ok(())
}

#[instrument]
pub fn load_texts(dry_run: bool, path: String) -> Result<()> {
    if dry_run {
        let bms = read_ndjson_file_and_create_bookmarks(&path)?;
        eprintln!("Would load {} texts for semantic search.", bms.len());
        Ok(())
    } else {
        create_embeddings_for_non_bookmarks(path)
    }
}

#[instrument]
pub fn sem_search(
    query: String,
    limit: Option<i32>,
    non_interactive: bool,
    mut stderr: StandardStream,
) -> Result<()> {
    let bms = Bookmarks::new(String::new());
    let results = find_similar(&query, &bms)?;
    let limit = limit.unwrap_or(10) as usize;

    let filtered_results: Vec<_> = results
        .iter()
        .filter_map(|(id, similarity)| {
            bms.bms.iter().find(|bm| bm.id == *id).map(|bm| {
                let mut dbm = DisplayBookmark::from(bm);
                dbm.similarity = Some(*similarity);
                (bm.clone(), dbm)
            })
        })
        .take(limit)
        .collect();

    // Display results
    let display_bookmarks: Vec<_> = filtered_results
        .iter()
        .map(|(_, dbm)| dbm)
        .cloned()
        .collect();
    show_bms(&display_bookmarks, &DEFAULT_FIELDS);

    if non_interactive {
        let ids = filtered_results
            .iter()
            .map(|(bm, _)| bm.id.to_string())
            .sorted()
            .join(",");
        println!("{}", ids);
    } else {
        stderr.set_color(ColorSpec::new().set_fg(Some(Color::Green)))?;
        writeln!(&mut stderr, "Selection: ")?;
        stderr.reset()?;
        service::process::process(
            &filtered_results
                .into_iter()
                .map(|(bm, _)| bm)
                .collect::<Vec<_>>(),
        );
    }

    Ok(())
}

#[instrument]
pub fn find_similar(query: &str, bms: &Bookmarks) -> Result<Vec<(i32, f32)>> {
    Context::update_global(Context::new(Box::new(OpenAiEmbedding::default())))?;

    let embedding = Context::read_global()
        .execute(query)?
        .ok_or_else(|| anyhow!("No embedding generated. OpenAI flag set?"))?;

    let query_vector = ndarray::Array1::from(embedding);
    let mut results = Vec::with_capacity(bms.bms.len());

    for bm in &bms.bms {
        if let Some(embedding_data) = &bm.embedding {
            let bm_embedding = deserialize_embedding(embedding_data.clone())?;
            let bm_vector = ndarray::Array1::from(bm_embedding);
            let similarity = cosine_similarity(&query_vector, &bm_vector);
            results.push((bm.id, similarity));
        }
    }

    results.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    Ok(results)
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;

    use crate::cli::commands::{find_similar, randomized, sem_search};
    use crate::model::bms::Bookmarks;
    
    use camino::Utf8PathBuf;
    use camino_tempfile::tempdir;
    use fs_extra::{copy_items, dir};
    use rstest::{fixture, rstest};
    use termcolor::ColorChoice;

    #[fixture]
    fn temp_dir() -> Utf8PathBuf {
        let tempdir = tempdir().unwrap();
        let options = dir::CopyOptions::new().overwrite(true);
        copy_items(
            &[
                "tests/resources/bkmr.v1.db",
                "tests/resources/bkmr.v2.db",
                "tests/resources/bkmr.v2.noembed.db",
            ],
            "../db",
            &options,
        )
        .expect("Failed to copy test project directory");

        tempdir.into_path()
    }

    #[allow(unused_variables)]
    #[ignore = "currently only works in isolation"]
    #[rstest]
    fn test_find_similar_when_embed_null(temp_dir: Utf8PathBuf) -> Result<()> {
        // Given: v2 database with embeddings and OpenAI context
        fs::rename("../db/bkmr.v2.noembed.db", "../db/bkmr.db").expect("Failed to rename database");
        let bms = Bookmarks::new("".to_string());
        Context::update_global(Context::new(Box::new(OpenAiEmbedding::default())))?;

        // When: find similar for "blub"
        let results = find_similar("blub", &bms)?;

        // Then: Expect no findings
        assert_eq!(results.len(), 0);
        Ok(())
    }

    #[allow(unused_variables)]
    #[rstest]
    fn test_find_similar(temp_dir: Utf8PathBuf) -> Result<()> {
        // Given: Set up test environment
        fs::rename("../db/bkmr.v2.db", "../db/bkmr.db")?;
        let bms = Bookmarks::new("".to_string());

        // Initialize CTX with proper error handling and verification
        Context::update_global(Context::new(Box::new(OpenAiEmbedding::default())))?;

        // When: find similar for "blub"
        let results = find_similar("blub", &bms)?;

        // Then: Verify results
        assert!(!results.is_empty(), "Expected non-empty results");
        assert_eq!(results.len(), 11, "Expected 11 results");

        // Verify top 3 results by ID
        let top_three_ids: Vec<_> = results.iter().take(3).map(|(id, _)| *id).collect();
        assert_eq!(
            top_three_ids,
            vec![4, 6, 5],
            "Expected top results to be IDs [4, 6, 5] in order, got {:?}",
            top_three_ids
        );

        // Verify similarities are properly ordered
        let similarities: Vec<_> = results.iter().take(3).map(|(_, sim)| *sim).collect();
        assert!(
            similarities.windows(2).all(|w| w[0] >= w[1]),
            "Expected similarities to be in descending order"
        );

        Ok(())
    }

    #[allow(unused_variables)]
    #[ignore = "interactive: visual check required"]
    #[rstest]
    fn test_sem_search_via_visual_check(temp_dir: Utf8PathBuf) -> Result<()> {
        let stderr = StandardStream::stderr(ColorChoice::Always);
        fs::rename("../db/bkmr.v2.db", "../db/bkmr.db").expect("Failed to rename database");
        // this is only visible test
        Context::update_global(Context::new(Box::new(OpenAiEmbedding::default())))?;
        // Given: v2 database with embeddings
        // When:
        sem_search("blub".to_string(), None, false, stderr)?;
        // Then: Expect the first three entries to be: blub, blub3, blub2
        Ok(())
    }

    #[ignore = "interactive: opens browser link"]
    #[test]
    fn test_randomized() -> Result<()> {
        randomized(1)?;
        Ok(())
    }
}
