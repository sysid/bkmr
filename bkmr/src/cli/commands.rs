// src/cli/commands.rs
use crate::adapter::dal::Dal;
use crate::adapter::embeddings::{cosine_similarity, deserialize_embedding};
use crate::adapter::json::{bms_to_json, read_ndjson_file_and_create_bookmarks};
use crate::environment::CONFIG;
use crate::helper::{confirm, ensure_int_vector, init_db, MIGRATIONS};
use crate::model::bms::Bookmarks;
use crate::model::bookmark::{Bookmark, BookmarkBuilder, BookmarkUpdater};
use crate::model::tag::Tags;
use crate::service::embeddings::create_embeddings_for_non_bookmarks;
use crate::service::fzf::fzf_process;
use crate::service::process::{delete_bms, edit_bms, open_bm, show_bms, DisplayBookmark, DisplayField, ALL_FIELDS, DEFAULT_FIELDS};
use crate::{dlog2, load_url_details, service, CTX};
use anyhow::anyhow;
use camino::Utf8Path;
use crossterm::style::Stylize;
use diesel::connection::SimpleConnection;
use diesel::result::DatabaseErrorKind;
use diesel::result::Error::DatabaseError;
use diesel_migrations::MigrationHarness;
use inquire::Confirm;
use itertools::Itertools;
use log::{debug, error};
use std::fs::create_dir_all;
use std::io::Write;
use std::process;
use stdext::function_name;
use termcolor::{Color, ColorSpec, StandardStream, WriteColor};

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
) -> Option<()> {
    let mut fields = DEFAULT_FIELDS.to_vec(); // Convert array to Vec
    let _tags_all = if let Some(tags_prefix) = tags_prefix {
        if let Some(tags_all) = tags_all {
            format!("{},{}", tags_all, tags_prefix)
        } else {
            tags_prefix
        }
    } else {
        tags_all.clone().unwrap_or_default()
    };
    debug!("({}:{}) tags: {:?}", function_name!(), line!(), _tags_all);
    let fts_query = fts_query.unwrap_or_default();
    let mut bms = Bookmarks::new(fts_query);
    bms.filter(
        Some(_tags_all),
        tags_any,
        tags_all_not,
        tags_any_not,
        tags_exact,
    );
    if order_desc {
        debug!(
            "({}:{}) order_desc {:?}",
            function_name!(),
            line!(),
            order_desc
        );
        bms.bms.sort_by_key(|bm| bm.last_update_ts);
        bms.bms.reverse();
        fields.push(DisplayField::LastUpdateTs); // Add the new enum variant
    } else if order_asc {
        debug!(
            "({}:{}) order_asc {:?}",
            function_name!(),
            line!(),
            order_asc
        );
        bms.bms.sort_by_key(|bm| bm.last_update_ts);
        fields.push(DisplayField::LastUpdateTs); // Add the new enum variant
    } else {
        debug!("({}:{}) order_by_metadata", function_name!(), line!());
        bms.bms.sort_by_key(|bm| bm.metadata.to_lowercase())
    }
    if let Some(limit) = limit {
        debug!("({}:{}) limit: {:?}", function_name!(), line!(), limit);
        bms.bms.truncate(limit as usize);
    }
    if is_fuzzy {
        fzf_process(&bms.bms);
        return Some(());
    }
    debug!("({}:{})\n{:?}\n", function_name!(), line!(), bms.bms);
    if is_json {
        bms_to_json(&bms.bms);
        return None;
    }
    let d_bms: Vec<DisplayBookmark> = bms.bms.iter()
        .map(DisplayBookmark::from).collect();
    show_bms(&d_bms, &fields);
    eprintln!("Found {} bookmarks", bms.bms.len());

    if non_interactive {
        debug!("Non Interactive. Exiting..");
        let ids: Vec<String> = bms
            .bms
            .iter()
            .map(|bm| bm.id)
            .sorted()
            .map(|id| id.to_string())
            .collect();
        println!("{}", ids.join(","));
    } else {
        stderr
            .set_color(ColorSpec::new().set_fg(Some(Color::Green)))
            .unwrap();
        writeln!(&mut stderr, "Selection: ").unwrap();
        stderr.reset().unwrap();
        service::process::process(&bms.bms);
    }
    None
}

pub fn open_bookmarks(ids: String) {
    let mut dal = Dal::new(CONFIG.db_url.clone());
    let ids = get_ids(ids);
    for id in ids.unwrap() {
        let bm = dal.get_bookmark_by_id(id);
        match bm {
            Ok(bm) => {
                debug!("({}:{}) Opening {:?}", function_name!(), line!(), bm);
                open_bm(&bm).unwrap();
                // open::that(bm.URL).unwrap();
            }
            Err(_) => {
                error!(
                    "({}:{}) Bookmark with id {} not found",
                    function_name!(),
                    line!(),
                    id
                );
            }
        }
    }
}

pub fn add_bookmark(
    url: String,
    tags: Option<String>,
    title: Option<String>,
    desc: Option<String>,
    no_web: bool,
    edit: bool,
) {
    let mut dal = Dal::new(CONFIG.db_url.clone());
    dlog2!(
        "Add {:?}, {:?}, {:?}, {:?}, {:?}, {:?}",
        url,
        tags,
        title,
        desc,
        no_web,
        edit
    );

    let unknown_tags =
        match Bookmarks::new("".to_string()).check_tags(Tags::normalize_tag_string(tags.clone())) {
            Ok(tags) => tags,
            Err(e) => {
                eprintln!("Error checking tags: {:?}", e);
                return;
            }
        };

    if !unknown_tags.is_empty() {
        dlog2!("unknown_tags: {:?}", unknown_tags);
        eprintln!("Unknown tags: {:?}", unknown_tags);
        let ans = Confirm::new(format!("Unknown tags: {:?}, create?", unknown_tags).as_str())
            .with_default(false)
            .with_help_message("Make sure the new tags are really necessary.")
            .prompt();

        match ans {
            Ok(true) => {}
            Ok(false) => {
                eprintln!("Aborted");
                return;
            }
            Err(_) => {
                eprintln!("Error, try again later");
                return;
            }
        }
    }

    let (_title, _description, _keywords) = if !no_web {
        let result = load_url_details(&url);
        result.unwrap_or_else(|e| {
            debug!("Cannot enrich URL details from web: {:?}", e);
            eprintln!("Cannot enrich URL data from web.");
            Default::default()
        })
    } else {
        Default::default()
    };
    let title = title.unwrap_or(_title);
    let description = desc.unwrap_or(_description);
    dlog2!("title: {:?}, description: {:?}", title, description);

    let mut bm = BookmarkBuilder::new()
        .id(1)
        .URL(url.to_string())
        .metadata(title)
        .tags(Tags::create_normalized_tag_string(tags))
        .desc(description)
        .flags(0)
        .build();
    bm.update(); // update embeddings

    match dal.insert_bookmark(bm.convert_to_new_bookmark()) {
        Ok(bms) => {
            if edit {
                edit_bms(vec![1], bms.clone()).unwrap_or_else(|e| {
                    error!(
                        "({}:{}) Error editing bookmark: {:?}",
                        function_name!(),
                        line!(),
                        e
                    );
                });
            }
            println!("Added bookmark: {:?}", bms[0].id);
            let d_bms: Vec<DisplayBookmark> = bms.iter()
                .map(DisplayBookmark::from).collect();
            show_bms(&d_bms, &DEFAULT_FIELDS);
        }
        Err(e) => {
            if let DatabaseError(DatabaseErrorKind::UniqueViolation, _) = e {
                eprintln!("Bookmark already exists: {}", url);
            } else {
                error!(
                    "({}:{}) Error adding bookmark: {:?}",
                    function_name!(),
                    line!(),
                    e
                );
            }
        }
    }
}

pub fn delete_bookmarks(ids: String) {
    let ids = get_ids(ids);
    let bms = Bookmarks::new("".to_string());
    delete_bms(ids.unwrap(), bms.bms).unwrap_or_else(|e| {
        eprintln!(
            "Error ({}:{}) Deleting Bookmarks: {:?}",
            function_name!(),
            line!(),
            e
        );
        process::exit(1);
    });
}

pub fn update_bookmarks(force: bool, tags: Option<String>, tags_not: Option<String>, ids: String) {
    if force && (tags.is_none() || tags_not.is_some()) {
        eprintln!(
            "({}:{}) Force update requires tags but no ntags.",
            function_name!(),
            line!()
        );
        process::exit(1);
    }
    let ids = get_ids(ids);
    let tags = Tags::normalize_tag_string(tags);
    let tags_not = Tags::normalize_tag_string(tags_not);
    println!("Update {:?}, {:?}, {:?}, {:?}", ids, tags, tags_not, force);
    crate::update_bookmarks(ids.unwrap(), tags, tags_not, force).unwrap_or_else(|e| {
        eprintln!(
            "Error ({}:{}) Updating Bookmarks: {:?}",
            function_name!(),
            line!(),
            e
        );
        process::exit(1);
    });
}

pub fn edit_bookmarks(ids: String) {
    let ids = get_ids(ids);
    let bms = Bookmarks::new("".to_string());
    edit_bms(ids.unwrap(), bms.bms).unwrap_or_else(|e| {
        eprintln!(
            "Error ({}:{}) Editing Bookmarks: {:?}",
            function_name!(),
            line!(),
            e
        );
        process::exit(1);
    });
}

pub fn create_db(path: String) {
    let path = Utf8Path::new(&path);
    if !path.exists() {
        println!("Creating database at {:?}", path);
        let parent = path.parent();
        if let Some(parent) = parent {
            create_dir_all(parent).unwrap();
            debug!("({}:{}) Created {:?}", function_name!(), line!(), parent);
        }

        let mut dal = Dal::new(path.to_string());
        match init_db(&mut dal.conn) {
            Ok(_) => {
                println!("Database created at {:?}", path);
            }
            Err(e) => {
                eprintln!(
                    "Error ({}:{}) Creating database: {:?}",
                    function_name!(),
                    line!(),
                    e
                );
                process::exit(1);
            }
        }
        let _ = dal.clean_table();
    } else {
        eprintln!(
            "({}:{}) Database already exists at {:?}.",
            function_name!(),
            line!(),
            path
        );
        process::exit(1);
    }
}

pub fn show_tags(tag: Option<String>) {
    let mut dal = Dal::new(CONFIG.db_url.clone());
    let tags = match tag {
        Some(tag) => dal.get_related_tags(&tag),
        None => dal.get_all_tags(),
    };
    match tags {
        Ok(tags) => {
            for tag in tags {
                println!("{}: {}", tag.n, tag.tag);
            }
        }
        Err(e) => {
            eprintln!(
                "Error ({}:{}) Getting all tags: {:?}",
                function_name!(),
                line!(),
                e
            );
            process::exit(1);
        }
    }
}

pub fn show_bookmarks(ids: String) {
    let mut dal = Dal::new(CONFIG.db_url.clone());
    let ids = get_ids(ids);
    let mut bms = vec![];
    for id in ids.unwrap() {
        let bm = dal.get_bookmark_by_id(id);
        match bm {
            Ok(bm) => {
                debug!("({}:{}) {:?}", function_name!(), line!(), bm);
                bms.push(bm);
            }
            Err(_) => {
                eprintln!("Bookmark with id {} not found", id);
            }
        }
    }
    let d_bms: Vec<DisplayBookmark> = bms.iter()
        .map(DisplayBookmark::from).collect();
    show_bms(&d_bms, &ALL_FIELDS);
}

fn get_ids(ids: String) -> Option<Vec<i32>> {
    let ids = ensure_int_vector(&ids.split(',').map(|s| s.to_owned()).collect());
    if ids.is_none() {
        eprintln!(
            "({}:{}) Invalid input, only numbers allowed {:?}",
            function_name!(),
            line!(),
            ids
        );
        process::exit(1);
    }
    ids
}

pub fn randomized(n: i32) {
    let mut dal = Dal::new(CONFIG.db_url.clone());
    let bms = dal.get_randomized_bookmarks(n);
    match bms {
        Ok(bms) => {
            debug!("({}:{}) Opening {:?}", function_name!(), line!(), bms);
            for bm in &bms {
                // opens without updating timestamp on purpose
                open::that(&bm.URL).unwrap();
            }
        }
        Err(e) => {
            error!(
                "({}:{}) Randomizing error: {:?}",
                function_name!(),
                line!(),
                e
            );
        }
    }
}

pub fn enable_embeddings_if_required() {
    dlog2!("Database: {}", CONFIG.db_url);
    let mut dal = Dal::new(CONFIG.db_url.clone());

    let embedding_column_exists = dal.check_embedding_column_exists().unwrap_or_else(|e| {
        eprintln!("Error checking existence of embedding column: {:?}", e);
        process::exit(1);
    });
    if embedding_column_exists {
        dlog2!("Embedding column exists, no migration required.");
        return;
    }

    eprintln!("New 'bkmr' version requires an extension of the database schema.");
    eprintln!("Two new columns will be added to the 'bookmarks' table:");
    if !confirm("Please backup up your DB before continue! Do you want to continue?") {
        println!("{}", "Aborting...".red());
        process::exit(1);
    }

    if !dal.check_schema_migrations_exists().unwrap_or_else(|e| {
        eprintln!("Error checking schema migrations: {:?}", e);
        process::exit(1);
    }) {
        eprintln!("__diesel_schema_migrations table does not exist. Creating it...");

        // SQL to create the __diesel_schema_migrations table and insert the initial record
        let create_table_sql = "
            BEGIN TRANSACTION;
            CREATE TABLE IF NOT EXISTS __diesel_schema_migrations (
                version VARCHAR(50) PRIMARY KEY NOT NULL,
                run_on TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
            );
            INSERT INTO __diesel_schema_migrations (version, run_on) VALUES ('20221229110455', '2023-12-23 09:27:06');
            COMMIT;
        ";

        if let Err(e) = dal.conn.batch_execute(create_table_sql) {
            eprintln!("Error creating __diesel_schema_migrations table: {:?}", e);
            process::exit(1);
        }

        eprintln!("__diesel_schema_migrations table created.");
    }

    if let Err(e) = dal.conn.pending_migrations(MIGRATIONS) {
        eprintln!("Error checking pending Migrations: {:?}", e);
        process::exit(1);
    } else {
        dal.conn
            .pending_migrations(MIGRATIONS)
            .unwrap()
            .iter()
            .for_each(|m| {
                dlog2!("Pending Migration: {}", m.name());
            });
    }
    if let Err(e) = dal.conn.run_pending_migrations(MIGRATIONS) {
        eprintln!("Error running pending migrations: {}", e);
        process::exit(1);
    }
    eprintln!("{}", "Database schema has been extended.".blue());
}

pub fn backfill_embeddings(dry_run: bool) {
    eprintln!("Database: {}", CONFIG.db_url);
    let mut dal = Dal::new(CONFIG.db_url.clone());
    let bms = dal.get_bookmarks_without_embedding().unwrap_or_else(|e| {
        eprintln!("Error getting bookmarks without embedding: {}", e);
        process::exit(1);
    });
    dlog2!("bms: {:?}", bms);
    for bm in &bms {
        println!("Updating: {:?}", bm.metadata);
        if dry_run {
            continue;
        }
        let mut bm = bm.clone();
        bm.update();
        dal.update_bookmark(bm).unwrap_or_else(|e| {
            eprintln!("Error updating bookmark: {}", e);
            process::exit(1);
        });
    }
}

pub fn load_texts(dry_run: bool, path: String) {
    eprintln!("Database: {}", CONFIG.db_url);
    if dry_run {
        eprintln!("Dry run, no changes will be made.");
        let bms = read_ndjson_file_and_create_bookmarks(path).unwrap_or_else(|e| {
            eprintln!("{}", format!("Error reading ndjson file: {}", e).red());
            process::exit(1);
        });
        eprintln!("Would load {} texts for semantic search.", bms.len());
        process::exit(0);
    }
    create_embeddings_for_non_bookmarks(path).unwrap_or_else(|e| {
        eprintln!("{}", format!("Error creating embeddings: {}", e).red());
        process::exit(1);
    });
}

pub fn sem_search(
    query: String,
    limit: Option<i32>,
    non_interactive: bool,
    mut stderr: StandardStream,
) {
    let bms = Bookmarks::new("".to_string());
    let results = match find_similar(&query, &bms) {
        Ok(value) => value,
        Err(e) => {
            eprintln!("Error finding similar: {}", e);
            process::exit(1);
        }
    };

    // Calculate limit once
    let limit = limit.unwrap_or(10) as usize;

    // todo: simplify this redundant vector generation
    let filtered_bms: Vec<Bookmark> = results.iter()
        .filter_map(|(id, _similarity)| {
            bms.bms.iter().find(|bm| bm.id == *id).cloned()
        })
        .take(limit)
        .collect();

    let display_bookmarks: Vec<DisplayBookmark> = results.iter()
        .filter_map(|(id, similarity)| {
            bms.bms.iter().find(|bm| bm.id == *id).map(|bm| {
                let mut dbm = DisplayBookmark::from(bm);
                dbm.similarity = Some(*similarity);
                dbm
            })
        })
        .take(limit)
        .collect();
    // debug!("display_bookmarks: {:?}", display_bookmarks);

    show_bms(&display_bookmarks, &DEFAULT_FIELDS);

    if non_interactive {
        debug!("Non Interactive. Exiting..");
        let ids: Vec<String> = filtered_bms
            .iter()
            .map(|bm| bm.id)
            .sorted()
            .map(|id| id.to_string())
            .collect();
        println!("{}", ids.join(","));
    } else {
        stderr
            .set_color(ColorSpec::new().set_fg(Some(Color::Green)))
            .unwrap();
        writeln!(&mut stderr, "Selection: ").unwrap();
        stderr.reset().unwrap();
        service::process::process(&filtered_bms);
    }
}

pub fn find_similar(query: &String, bms: &Bookmarks) -> anyhow::Result<Vec<(i32, f32)>> {
    let embedding = CTX
        .get()
        .ok_or_else(|| anyhow!("Error: CTX is not initialized"))?
        .execute(query)?
        .ok_or_else(|| anyhow!("Error: embedding is not set, did you use --openai?"))?;

    let ndarray_vector = ndarray::Array1::from(embedding);
    let mut results = Vec::new();
    for bm in &bms.bms {
        if let Some(embedding_data) = &bm.embedding {
            let bm_embedding = deserialize_embedding(embedding_data.clone())?;
            let bm_ndarray_vector = ndarray::Array1::from(bm_embedding);
            let similarity = cosine_similarity(&ndarray_vector, &bm_ndarray_vector);
            results.push((bm.id, similarity));
        }
        // Bookmarks without embeddings are skipped
    }
    // Sorting by similarity
    results.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
    Ok(results)
}