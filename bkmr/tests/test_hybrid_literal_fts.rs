//! Real SQLite and native CLI regressions. No model is needed by the default
//! exact-mode cases. The ignored model/old-binary cases have strict prerequisites
//! and must be selected separately; an absent prerequisite is a failure, not a skip.
use assert_cmd::cargo::cargo_bin_cmd;
use assert_cmd::Command;
use bkmr::application::services::bookmark_service::BookmarkService;
use bkmr::application::BookmarkServiceImpl;
use bkmr::domain::bookmark::Bookmark;
use bkmr::domain::embedding::Embedder;
use bkmr::domain::error::DomainResult;
use bkmr::domain::repositories::repository::BookmarkRepository;
use bkmr::domain::repositories::vector_repository::VectorRepository;
use bkmr::domain::search::{HybridSearch, RankedResult, SearchMode};
use bkmr::domain::tag::Tag;
use bkmr::infrastructure::embeddings::FastEmbedEmbedding;
use bkmr::infrastructure::repositories::file_import_repository::FileImportRepository;
use bkmr::infrastructure::repositories::sqlite::repository::SqliteBookmarkRepository;
use bkmr::infrastructure::repositories::sqlite::vector_repository::SqliteVectorRepository;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::io::Read;
use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::path::{Path, PathBuf};
use std::process::Output;
use std::sync::{Arc, Mutex, Once};
use std::time::Duration;
use tempfile::TempDir;

struct NativeRun {
    base: PathBuf,
    owned: Option<TempDir>,
    db: PathBuf,
    config: PathBuf,
    task_home: PathBuf,
}

impl NativeRun {
    fn new() -> Self {
        let requested = match std::env::var_os("BKMR_NATIVE_TEST_ROOT") {
            Some(root) => {
                let root = PathBuf::from(root);
                assert!(root.is_absolute(), "BKMR_NATIVE_TEST_ROOT must be an existing absolute owned test Run");
                root
            }
            None => {
                let root = Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("../ProjectCentral/now/tmp");
                std::fs::create_dir_all(&root).expect("prepare product-local test Run space");
                root
            }
        };
        let base = requested.canonicalize().expect("physical native test Run root");
        assert!(base.is_dir(), "test Run root must be a directory");
        let owned = tempfile::Builder::new().prefix("bkmr-literal-fts-")
            .tempdir_in(&base).expect("exclusive fixture child in native Run space");
        let db = owned.path().join("native.db");
        let task_home = owned.path().join("child-home");
        std::fs::create_dir(&task_home).unwrap();
        let config = owned.path().join("config.toml");
        std::fs::write(&config, "[embeddings]\nmodel = \"AllMiniLML6V2\"\n").unwrap();
        Self { base, owned: Some(owned), db, config, task_home }
    }

    fn root(&self) -> &Path {
        self.owned.as_ref().unwrap().path()
    }

    fn repository(&self) -> SqliteBookmarkRepository {
        static REGISTER: Once = Once::new();
        REGISTER.call_once(bkmr::infrastructure::repositories::sqlite::register_sqlite_vec);
        SqliteBookmarkRepository::from_url(self.db.to_str().expect("fixture DB has native UTF-8 coordinate"))
            .expect("actual native SQLite repository and migrations")
    }

    fn prepare_command(&self, command: &mut Command) {
        // Only this child gets an isolated home/cache/config. No global cwd/env
        // mutation and no existing private DB, config, credential or model use.
        command.env_clear().env("HOME", &self.task_home)
            .env("XDG_CONFIG_HOME", self.task_home.join("config"))
            .env("XDG_CACHE_HOME", self.task_home.join("cache"))
            .env("NO_COLOR", "1")
            .arg("--config").arg(&self.config)
            .arg("--db").arg(&self.db)
            .current_dir(self.root()).timeout(Duration::from_secs(20));
    }

    fn command(&self) -> Command {
        let mut command = cargo_bin_cmd!("bkmr");
        self.prepare_command(&mut command);
        command
    }

    fn seed(&self, title: &str, tag: &str) -> i32 {
        let repo = self.repository();
        let mut tags = HashSet::new();
        tags.insert(Tag::new(tag).unwrap());
        let mut bookmark = Bookmark::new(
            format!("https://literal-fts.invalid/{}", title.len()),
            title.to_string(), String::new(), tags).unwrap();
        bookmark.set_embeddable(false);
        repo.add(&mut bookmark).expect("actual owner repository insertion and FTS trigger");
        bookmark.id.expect("actual native allocated bookmark ID")
    }

    fn hsearch(&self, subject: &str, literal: bool, options: &[&str]) -> Output {
        let mut command = self.command();
        command.args(["hsearch", "--mode", "exact", "--json", "--np"]);
        if literal { command.arg("--literal-fts"); }
        command.args(options).arg("--").arg(subject);
        command.output().expect("finite existing native assert_cmd capture")
    }
}

impl Drop for NativeRun {
    fn drop(&mut self) {
        if let Some(owned) = self.owned.take() {
            let path = owned.path().to_path_buf();
            if let Err(error) = owned.close() {
                eprintln!("owned native test Run cleanup failed; retained {}: {error}", path.display());
                if !std::thread::panicking() {
                    panic!("owned native test Run cleanup failed: {error}");
                }
            }
        }
    }
}

fn successful_json(output: &Output) -> Vec<Value> {
    assert!(output.status.success(), "native status={:?}; stderr={}",
        output.status.code(), String::from_utf8_lossy(&output.stderr));
    // A post-capture profile, not a live output/storage or descendant bound.
    assert!(output.stdout.len() + output.stderr.len() <= 1024 * 1024);
    serde_json::from_slice::<Value>(&output.stdout).expect("actual native JSON")
        .as_array().expect("native result array").clone()
}

fn ids(rows: &[Value]) -> BTreeSet<i32> {
    rows.iter().map(|row| i32::try_from(row["id"].as_i64().unwrap()).unwrap()).collect()
}

#[test]
fn given_punctuation_when_literal_fts_then_actual_sqlite_matches_without_changing_subject() {
    let run = NativeRun::new();
    let subject = "M-PRIME-PHYSICAL-MUSIC-WORK-PACKETS P physical form source branches";
    let id = run.seed(subject, "native-proof");
    let repo = run.repository();
    assert!(repo.get_bookmarks_fts_ranked(subject, None).is_err(), "real raw SQLite grammar characterization");
    for query in [subject, "M-PRIME", "\"physical\"", "(source)", "P:physical", "form*", "  M-PRIME\tphysical  "] {
        // P:physical is a quoted phrase, not a column selector; its native
        // tokenizer phrase P physical occurs in the declared source title.
        let mut search = HybridSearch::new(query);
        search.literal_fts = true;
        let found = repo.get_bookmarks_fts_ranked(&search.fts_query(), None).unwrap();
        assert_eq!(found.iter().map(|row| row.bookmark_id).collect::<Vec<_>>(), vec![id], "actual literal subject {query:?}");
        assert_eq!(search.query, query, "lexical rendering may not rewrite the embedding input");
    }
}

#[test]
fn given_raw_boolean_grammar_when_opt_in_changes_then_actual_sqlite_keeps_default_grammar() {
    let run = NativeRun::new();
    let alpha = run.seed("alpha", "native-proof");
    let beta = run.seed("beta", "native-proof");
    let both = run.seed("alpha OR beta", "native-proof");
    let repo = run.repository();
    let mut search = HybridSearch::new("alpha OR beta");
    assert!(!search.literal_fts);
    assert_eq!(search.fts_query(), "alpha OR beta");
    let raw = repo.get_bookmarks_fts_ranked(&search.fts_query(), None).unwrap();
    assert_eq!(raw.iter().map(|row| row.bookmark_id).collect::<BTreeSet<_>>(), BTreeSet::from([alpha, beta, both]));
    search.literal_fts = true;
    let literal = repo.get_bookmarks_fts_ranked(&search.fts_query(), None).unwrap();
    assert_eq!(literal.iter().map(|row| row.bookmark_id).collect::<Vec<_>>(), vec![both]);
}

#[test]
fn given_native_exact_cli_when_literal_subject_then_real_id_tags_hydration_and_rrf_are_preserved() {
    let run = NativeRun::new();
    let subject = "M-PRIME-PHYSICAL-MUSIC-WORK-PACKETS P physical form source branches";
    let id = run.seed(subject, "native-proof");
    let raw = run.hsearch(subject, false, &[]);
    assert_eq!(raw.status.code(), Some(bkmr::exitcode::USAGE));
    assert!(raw.stdout.is_empty());
    assert!(String::from_utf8_lossy(&raw.stderr).contains("PRIME"), "actual SQLite error is retained, not fabricated as JSON success");
    let rows = successful_json(&run.hsearch(subject, true, &["--tags", "native-proof", "--limit", "1"]));
    assert_eq!(ids(&rows), BTreeSet::from([id]));
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0]["title"], subject);
    assert_eq!(rows[0]["tags"], ",native-proof,");
    assert_eq!(rows[0]["description"], "");
    assert!(rows[0]["url"].as_str().unwrap().starts_with("https://literal-fts.invalid/"));
    assert!((rows[0]["rrf_score"].as_f64().unwrap() - 1.0 / 61.0).abs() < 1e-12);
    assert_eq!(successful_json(&run.hsearch(subject, true, &["--Tags", "native-proof"])).len(), 0);
}

#[test]
fn given_actual_cli_when_raw_or_literal_then_defaults_and_advertised_opt_in_remain_distinct() {
    let run = NativeRun::new();
    let alpha = run.seed("alpha", "native-proof");
    let beta = run.seed("beta", "native-proof");
    let both = run.seed("alpha OR beta", "native-proof");
    assert_eq!(ids(&successful_json(&run.hsearch("alpha OR beta", false, &[]))), BTreeSet::from([alpha, beta, both]));
    assert_eq!(ids(&successful_json(&run.hsearch("alpha OR beta", true, &[]))), BTreeSet::from([both]));
    let help = run.command().args(["hsearch", "--help"]).output().unwrap();
    assert!(help.status.success());
    assert!(String::from_utf8(help.stdout).unwrap().contains("--literal-fts"));
}

#[test]
fn given_empty_native_results_when_json_requested_then_success_is_an_actual_empty_array() {
    let run = NativeRun::new();
    run.repository();
    for literal in [false, true] {
        let output = run.hsearch("unmatchedsubject", literal, &[]);
        assert!(successful_json(&output).is_empty());
        assert_eq!(output.stdout, b"[]\n");
    }
    let id = run.seed("actual matched subject", "native-proof");
    assert_eq!(ids(&successful_json(&run.hsearch("matched", false, &[]))), BTreeSet::from([id]));
    let filtered = run.hsearch("matched", true, &["--tags", "not-this-native-tag"]);
    assert!(successful_json(&filtered).is_empty());
    assert_eq!(filtered.stdout, b"[]\n");
    let plain = run.command().args(["hsearch", "--mode", "exact", "--np", "unmatchedsubject"]).output().unwrap();
    assert!(plain.status.success());
    assert!(plain.stdout.is_empty());
    assert!(String::from_utf8(plain.stderr).unwrap().contains("No bookmarks found"));
}

#[test]
fn given_whitespace_literal_when_native_tag_prefilter_is_empty_then_validation_still_refuses() {
    let run = NativeRun::new();
    run.repository();
    let output = run.hsearch(" \t\n ", true, &["--tags", "absent-tag"]);
    assert_eq!(output.status.code(), Some(bkmr::exitcode::USAGE));
    assert!(output.stdout.is_empty(), "invalid input is not a successful empty JSON result");
    assert!(String::from_utf8(output.stderr).unwrap().contains("literal FTS search requires at least one non-whitespace term"));
}

#[test]
#[ignore = "requires explicitly pinned actual unmodified native bkmr7.6.7 binary; must be selected, never skipped"]
fn given_actual_old_native_binary_when_literal_flag_requested_then_unsupported_is_explicit_without_downgrade() {
    let run = NativeRun::new();
    let old = PathBuf::from(std::env::var_os("BKMR_LITERAL_FTS_BEFORE_BIN")
        .expect("actual pinned native7.6.7 prerequisite; no green skip"));
    assert!(old.is_absolute() && old.is_file());
    let expected_sha = std::env::var("BKMR_LITERAL_FTS_BEFORE_SHA256")
        .expect("actual before-binary digest from the qualified native7.6.7 artifact receipt");
    assert_eq!(expected_sha.len(), 64);
    assert!(expected_sha.bytes().all(|byte| byte.is_ascii_hexdigit()));
    let mut file = std::fs::File::open(&old).unwrap();
    let mut digest = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    let mut observed_bytes = 0_u64;
    loop {
        let count = file.read(&mut buffer).unwrap();
        if count == 0 { break; }
        observed_bytes += count as u64;
        assert!(observed_bytes <= 512 * 1024 * 1024, "explicit native before-artifact observation capacity");
        digest.update(&buffer[..count]);
    }
    assert_eq!(format!("{:x}", digest.finalize()), expected_sha.to_ascii_lowercase());
    let version = Command::new(&old).arg("--version").timeout(Duration::from_secs(20)).output().unwrap();
    assert!(version.status.success());
    assert_eq!(String::from_utf8(version.stdout).unwrap().trim(), "bkmr 7.6.7");
    let subject = "M-PRIME-PHYSICAL-MUSIC-WORK-PACKETS P physical form source branches";
    let id = run.seed(subject, "native-proof");
    let mut old_command = Command::new(&old);
    run.prepare_command(&mut old_command);
    let raw = old_command.args(["hsearch", "--mode", "exact", "--json", "--np", "--", subject]).output().unwrap();
    assert_eq!(raw.status.code(), Some(bkmr::exitcode::USAGE));
    assert!(raw.stdout.is_empty());
    assert!(String::from_utf8(raw.stderr).unwrap().contains("PRIME"));
    let mut unsupported = Command::new(&old);
    run.prepare_command(&mut unsupported);
    let unsupported = unsupported.args(["hsearch", "--literal-fts", "--mode", "exact", "--json", "--np", "--", subject]).output().unwrap();
    assert_eq!(unsupported.status.code(), Some(2), "actual Clap unsupported-option outcome");
    assert!(unsupported.stdout.is_empty());
    assert!(String::from_utf8(unsupported.stderr).unwrap().contains("--literal-fts"));
    let quoted = HybridSearch { literal_fts: true, ..HybridSearch::new(subject) }.fts_query().into_owned();
    for mut command in [run.command(), { let mut command = Command::new(&old); run.prepare_command(&mut command); command }] {
        let rows = successful_json(&command.args(["search", "--json", "--np", "--", &quoted]).output().unwrap());
        assert_eq!(ids(&rows), BTreeSet::from([id]), "independent native Fulltext positive control; no Hybrid downgrade");
    }
    assert_eq!(ids(&successful_json(&run.hsearch(subject, true, &[]))), BTreeSet::from([id]));
    let mut empty_before = Command::new(&old);
    run.prepare_command(&mut empty_before);
    let empty_before = empty_before.args(["hsearch", "--mode", "exact", "--json", "--np", "unmatchedsubject"]).output().unwrap();
    assert!(empty_before.status.success());
    assert!(empty_before.stdout.is_empty(), "real native7.6.7 empty-JSON characterization");
    assert!(String::from_utf8(empty_before.stderr).unwrap().contains("No bookmarks found"));
    let empty_after = run.hsearch("unmatchedsubject", false, &[]);
    assert!(successful_json(&empty_after).is_empty());
    assert_eq!(empty_after.stdout, b"[]\n");
}

/// An observer delegates EVERY request to the actual native provider. It returns
/// no fabricated embedding, error, JSON or rank. The retained input is evidence
/// about the real call, not a tool-count improvement or replacement provider.
#[derive(Debug)]
struct ObservedNativeEmbedder {
    native: FastEmbedEmbedding,
    actual_queries: Mutex<Vec<String>>,
}
impl Embedder for ObservedNativeEmbedder {
    fn embed_document(&self, text: &str) -> DomainResult<Option<Vec<f32>>> {
        self.native.embed_document(text)
    }
    fn embed_query(&self, text: &str) -> DomainResult<Option<Vec<f32>>> {
        self.actual_queries.lock().unwrap().push(text.to_string());
        self.native.embed_query(text)
    }
    fn dimensions(&self) -> usize { self.native.dimensions() }
}

#[test]
#[ignore = "requires explicitly selected genuine cached FastEmbed model in admitted native Run; required hybrid contribution gate, no skip"]
fn given_genuine_native_model_when_literal_hybrid_then_original_input_and_actual_vector_contribution_survive() {
    let run = NativeRun::new();
    let model_name = std::env::var("BKMR_LITERAL_FTS_MODEL")
        .expect("explicit actual native model selection prerequisite; no green skip");
    let cache = PathBuf::from(std::env::var_os("FASTEMBED_CACHE_DIR")
        .expect("explicit admitted native model cache prerequisite; no ambient model"));
    assert!(cache.is_absolute());
    let cache = cache.canonicalize().expect("actual existing native model cache");
    assert!(cache.starts_with(&run.base) && cache != run.base, "actual model cache must belong to the selected native test Run, never private ambient cache");
    std::fs::read_dir(&cache).unwrap().next()
        .expect("genuine prequalified native model cache must be nonempty")
        .expect("actual model cache entry must be readable");
    let model = FastEmbedEmbedding::model_from_name(&model_name).unwrap();
    let native = Arc::new(ObservedNativeEmbedder { native: FastEmbedEmbedding::new(model), actual_queries: Mutex::new(Vec::new()) });
    assert!(native.dimensions() > 0);
    let repo = Arc::new(run.repository());
    let vectors = Arc::new(SqliteVectorRepository::new(run.db.to_str().unwrap()).unwrap());
    vectors.init_vec_table(native.dimensions()).unwrap();
    let service = BookmarkServiceImpl::new(repo.clone(), native.clone(), vectors.clone(), Arc::new(FileImportRepository::new()));
    let subject = "M-PRIME-PHYSICAL-MUSIC-WORK-PACKETS P physical form source branches";
    let lexical = service.add_bookmark("https://literal-fts.invalid/lexical", Some(subject), Some("native source and physical form"), None, false, true, None).unwrap();
    let semantic = service.add_bookmark("https://literal-fts.invalid/semantic", Some("material embodiment and branches"), Some("source form and physical work"), None, false, true, None).unwrap();
    let lexical_id = lexical.id.unwrap();
    let semantic_id = semantic.id.unwrap();
    assert_ne!(lexical_id, semantic_id);
    assert!(vectors.has_embeddings().unwrap(), "actual native embeddings must have been committed");
    let original = native.native.embed_query(subject).unwrap().expect("actual selected model query vector");
    assert_eq!(original.len(), native.dimensions());
    assert!(original.iter().all(|value| value.is_finite()));
    let ranked = vectors.search_nearest_filtered(&original, 20, None).unwrap();
    assert_eq!(ranked.iter().map(|row| row.0).collect::<BTreeSet<_>>(), BTreeSet::from([lexical_id, semantic_id]));
    let mut search = HybridSearch::new(subject);
    search.literal_fts = true;
    search.limit = Some(10);
    let fts = repo.get_bookmarks_fts_ranked(&search.fts_query(), None).unwrap();
    assert_eq!(fts.iter().map(|row| row.bookmark_id).collect::<Vec<_>>(), vec![lexical_id]);
    let mut expected = BTreeMap::<i32, f64>::new();
    for RankedResult { bookmark_id, rank } in fts {
        *expected.entry(bookmark_id).or_default() += 1.0 / (60.0 + rank as f64 + 1.0);
    }
    for (rank, (id, _)) in ranked.into_iter().enumerate() {
        *expected.entry(id).or_default() += 1.0 / (60.0 + rank as f64 + 1.0);
    }
    let result = service.hybrid_search(&search).unwrap();
    assert_eq!(*native.actual_queries.lock().unwrap(), vec![subject.to_string()], "observed actual provider gets verbatim original input, not the quoted FTS representation");
    assert_eq!(result.iter().map(|row| row.bookmark.id.unwrap()).collect::<BTreeSet<_>>(), BTreeSet::from([lexical_id, semantic_id]));
    for row in &result {
        let id = row.bookmark.id.unwrap();
        assert!((row.rrf_score - expected[&id]).abs() < 1e-12, "actual RRF includes real SQLite vector contribution");
    }
    let hybrid_score = result.iter().find(|row| row.bookmark.id == Some(lexical_id)).unwrap().rrf_score;
    search.mode = SearchMode::Exact;
    let exact = service.hybrid_search(&search).unwrap();
    assert_eq!(exact.len(), 1);
    assert_eq!(exact[0].bookmark.id, Some(lexical_id));
    assert!(hybrid_score > exact[0].rrf_score, "actual semantic contribution is consequential to the lexical score");
    assert_eq!(native.actual_queries.lock().unwrap().len(), 1, "exact mode preserves semantic branch exclusion");
    eprintln!("actual selected native model={model_name}; dimensions={}; cache={}; native IDs={lexical_id},{semantic_id}; hybrid lexical score={hybrid_score}; exact lexical score={}", native.dimensions(), cache.display(), exact[0].rrf_score);
}

// Native repository-error successor: the original eight definitions above are
// retained byte for byte. These fixtures only change their own SQLite state.
#[derive(Debug, PartialEq, Eq)]
struct NativeDatabaseBytes {
    database: Vec<u8>,
    wal: Option<Vec<u8>>,
}

fn native_database_bytes(run: &NativeRun) -> NativeDatabaseBytes {
    let mut wal_name = run.db.as_os_str().to_os_string();
    wal_name.push("-wal");
    let wal = match std::fs::read(PathBuf::from(wal_name)) {
        Ok(bytes) => Some(bytes),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
        Err(error) => panic!("actual owned SQLite WAL read failed: {error}"),
    };
    NativeDatabaseBytes {
        database: std::fs::read(&run.db).expect("actual owned native database bytes"),
        wal,
    }
}

fn actual_presence_error_message(error: &bkmr::domain::error::DomainError) -> &str {
    match error.presentation() {
        bkmr::domain::error::DomainError::BookmarkOperationFailed(message) => message,
        other => panic!("actual native SQLite presence error had another variant: {other:?}"),
    }
}

#[test]
fn given_real_empty_vector_table_when_checked_then_absence_is_healthy_and_restart_stable() {
    let run = NativeRun::new();
    let id = run.seed("native healthy absence source", "native-proof");
    let repo = run.repository();
    let before = repo.get_by_id(id).unwrap().unwrap();
    let vectors = SqliteVectorRepository::new(run.db.to_str().unwrap()).unwrap();
    // Only a native empty-table width is declared here. No embedding, model,
    // semantic result or artificial vector is supplied by this default case.
    vectors.init_vec_table(384).unwrap();
    let database = rusqlite::Connection::open(&run.db).unwrap();
    let count: i64 = database.query_row(
        "SELECT COUNT(*) FROM vec_bookmarks", [], |row| row.get(0)).unwrap();
    assert_eq!(count, 0);
    let bytes = native_database_bytes(&run);
    assert!(!vectors.has_embeddings().unwrap(), "healthy native absence is Ok(false)");
    drop(vectors);
    let reopened = SqliteVectorRepository::new(run.db.to_str().unwrap()).unwrap();
    assert!(!reopened.has_embeddings().unwrap());
    assert_eq!(native_database_bytes(&run), bytes);
    assert!(repo.get_by_id(id).unwrap().unwrap() == before);
}

#[test]
fn given_actual_missing_vector_table_when_checked_then_native_repository_error_survives_recovery() {
    let run = NativeRun::new();
    let id = run.seed("native repository failure source", "native-proof");
    let repo = run.repository();
    let before = repo.get_by_id(id).unwrap().unwrap();
    let vectors = SqliteVectorRepository::new(run.db.to_str().unwrap()).unwrap();
    vectors.init_vec_table(384).unwrap();
    let database = rusqlite::Connection::open(&run.db).unwrap();
    database.execute_batch("DROP TABLE vec_bookmarks").unwrap();
    let sqlite_error = database.query_row(
        "SELECT COUNT(*) FROM vec_bookmarks", [], |row| row.get::<_, i64>(0))
        .unwrap_err();
    let bytes = native_database_bytes(&run);
    let native_error = vectors.has_embeddings().unwrap_err();
    assert_eq!(actual_presence_error_message(&native_error),
        format!("Failed to check vec_bookmarks count: {sqlite_error}"));
    let retained = std::error::Error::source(&native_error).unwrap()
        .downcast_ref::<rusqlite::Error>().expect("original actual missing-table SQLite cause");
    assert_eq!(retained.sqlite_error_code(), sqlite_error.sqlite_error_code());
    assert_eq!(retained.to_string(), sqlite_error.to_string());
    assert_eq!(native_database_bytes(&run), bytes);
    assert!(repo.get_by_id(id).unwrap().unwrap() == before);
    // Recovery is the existing native table operation, not a new repository or
    // an error-to-empty substitute. The original source record stays in place.
    vectors.init_vec_table(384).unwrap();
    assert!(!vectors.has_embeddings().unwrap());
    assert!(repo.get_by_id(id).unwrap().unwrap() == before);
}

#[test]
#[ignore = "requires explicitly selected genuine cached FastEmbed model in admitted native Run; required repository-error service gate, no skip"]
fn given_genuine_native_model_when_vector_presence_query_fails_then_hybrid_preserves_error_and_healthy_fallback() {
    use bkmr::application::error::ApplicationError;
    use bkmr::cli::error::CliError;
    use bkmr::domain::error::DomainError;
    use bkmr::domain::error_context::CliErrorContext;

    let run = NativeRun::new();
    let model_name = std::env::var("BKMR_LITERAL_FTS_MODEL")
        .expect("explicit genuine native model prerequisite; no green skip");
    let cache = PathBuf::from(std::env::var_os("FASTEMBED_CACHE_DIR")
        .expect("explicit admitted native model cache prerequisite"));
    assert!(cache.is_absolute());
    let cache = cache.canonicalize().expect("actual existing native model cache");
    assert!(cache.starts_with(&run.base) && cache != run.base);
    std::fs::read_dir(&cache).unwrap().next()
        .expect("actual prequalified model cache prerequisite, not inference proof")
        .expect("actual model cache member is readable");
    let model = FastEmbedEmbedding::model_from_name(&model_name).unwrap();
    let native = Arc::new(ObservedNativeEmbedder {
        native: FastEmbedEmbedding::new(model), actual_queries: Mutex::new(Vec::new()),
    });
    assert!(native.dimensions() > 0, "only a genuine selected semantic provider reaches this branch");
    let subject = "physical source branches";
    let repo = Arc::new(run.repository());
    let mut tags = HashSet::new();
    tags.insert(Tag::new("native-proof").unwrap());
    let mut bookmark = Bookmark::new(
        "https://literal-fts.invalid/presence-error", subject, "", tags).unwrap();
    assert!(bookmark.embeddable, "the actual positive source permits semantic participation");
    repo.add(&mut bookmark).unwrap();
    let id = bookmark.id.expect("actual native source identity");
    let before = repo.get_by_id(id).unwrap().unwrap();
    let vectors = Arc::new(SqliteVectorRepository::new(run.db.to_str().unwrap()).unwrap());
    vectors.init_vec_table(native.dimensions()).unwrap();
    let service = BookmarkServiceImpl::new(
        repo.clone(), native.clone(), vectors.clone(), Arc::new(FileImportRepository::new()));
    let mut search = HybridSearch::new(subject);
    search.limit = Some(10);

    for literal in [false, true] {
        search.literal_fts = literal;
        assert!(!vectors.has_embeddings().unwrap());
        let healthy = service.hybrid_search(&search).unwrap();
        assert_eq!(healthy.len(), 1);
        assert_eq!(healthy[0].bookmark.id, Some(id));
        assert!(healthy[0].bookmark == before);
        assert!((healthy[0].rrf_score - 1.0 / 61.0).abs() < 1e-12);
    }
    assert!(native.actual_queries.lock().unwrap().is_empty());

    let database = rusqlite::Connection::open(&run.db).unwrap();
    database.execute_batch("DROP TABLE vec_bookmarks").unwrap();
    let sqlite_error = database.query_row(
        "SELECT COUNT(*) FROM vec_bookmarks", [], |row| row.get::<_, i64>(0))
        .unwrap_err();
    let direct_error = vectors.has_embeddings().unwrap_err();
    let expected_message = actual_presence_error_message(&direct_error).to_string();
    assert_eq!(expected_message, format!("Failed to check vec_bookmarks count: {sqlite_error}"));
    let bytes = native_database_bytes(&run);

    for literal in [false, true] {
        search.literal_fts = literal;
        let failed = service.hybrid_search(&search)
            .expect_err("actual repository failure must not acknowledge healthy lexical-only Hybrid");
        match &failed {
            ApplicationError::Domain(error) => {
                assert_eq!(actual_presence_error_message(error), expected_message);
            }
            other => panic!("native repository cause changed at service boundary: {other:?}"),
        }
        let source = std::error::Error::source(&failed).unwrap();
        let source = source.downcast_ref::<DomainError>().unwrap();
        assert_eq!(actual_presence_error_message(source), expected_message);
        let sqlite = std::error::Error::source(source).unwrap()
            .downcast_ref::<rusqlite::Error>().expect("original native SQLite cause at service boundary");
        assert_eq!(sqlite.sqlite_error_code(), sqlite_error.sqlite_error_code());
        assert_eq!(sqlite.to_string(), sqlite_error.to_string());
        let native_cause = sqlite as *const rusqlite::Error;
        // This is the SAME existing handler context formatter applied to the
        // actual failed native result. It is not a claimed CLI child replay.
        let contextual = Result::<(), ApplicationError>::Err(failed)
            .cli_context("performing hybrid search on bookmarks").unwrap_err();
        match &contextual {
            CliError::Application(ApplicationError::Domain(error)) => {
                assert_eq!(actual_presence_error_message(error),
                    format!("performing hybrid search on bookmarks: {expected_message}"));
            }
            other => panic!("actual native CLI context changed cause class: {other:?}"),
        }
        let domain = std::error::Error::source(&contextual).unwrap()
            .downcast_ref::<ApplicationError>().unwrap();
        let domain = std::error::Error::source(domain).unwrap().downcast_ref::<DomainError>().unwrap();
        let contextual_sqlite = std::error::Error::source(domain).unwrap()
            .downcast_ref::<rusqlite::Error>().unwrap();
        assert_eq!(contextual_sqlite as *const rusqlite::Error, native_cause,
            "handler context carries the identical native source allocation");
        assert_eq!(native_database_bytes(&run), bytes);
        assert!(repo.get_by_id(id).unwrap().unwrap() == before);
        assert!(native.actual_queries.lock().unwrap().is_empty());
    }

    // Exact is an intentionally independent lexical operation. It must remain
    // useful even when the unrequested vector-presence operation would fail.
    search.mode = SearchMode::Exact;
    let exact = service.hybrid_search(&search).unwrap();
    assert_eq!(exact.len(), 1);
    assert!(exact[0].bookmark == before);
    assert!((exact[0].rrf_score - 1.0 / 61.0).abs() < 1e-12);
    assert_eq!(native_database_bytes(&run), bytes);
    assert!(native.actual_queries.lock().unwrap().is_empty());

    vectors.init_vec_table(native.dimensions()).unwrap();
    assert!(!vectors.has_embeddings().unwrap());
    search.mode = SearchMode::Hybrid;
    let restored = service.hybrid_search(&search).unwrap();
    assert_eq!(restored.len(), 1);
    assert!(restored[0].bookmark == before);
    assert!((restored[0].rrf_score - 1.0 / 61.0).abs() < 1e-12);
    assert!(native.actual_queries.lock().unwrap().is_empty());

    // Actual inference, native storage and an actual semantic request are
    // required before this gate passes; cache presence alone proves none of it.
    let embedding = native.embed_document(subject).unwrap()
        .expect("genuine selected model must perform actual document inference");
    assert_eq!(embedding.len(), native.dimensions());
    assert!(embedding.iter().all(|value| value.is_finite()));
    vectors.upsert_embedding(id, &embedding).unwrap();
    assert!(vectors.has_embeddings().unwrap());
    let positive = service.hybrid_search(&search).unwrap();
    assert_eq!(positive.len(), 1);
    assert!(positive[0].bookmark == before);
    assert!((positive[0].rrf_score - 2.0 / 61.0).abs() < 1e-12);
    assert_eq!(*native.actual_queries.lock().unwrap(), vec![subject.to_string()]);
    assert!(repo.get_by_id(id).unwrap().unwrap() == before);
    eprintln!("actual native repository failure/recovery and genuine model contribution: model={model_name}; dimensions={}; id={id}", native.dimensions());
}

// Native INIT query propagation: all preceding f1d definitions are retained.
#[test]
fn given_corrupt_native_database_when_initializing_vectors_then_schema_error_precedes_create() {
    let run = NativeRun::new();
    let corrupt = vec![0x61u8; 4096];
    std::fs::write(&run.db, &corrupt).unwrap();
    let vectors = SqliteVectorRepository::new(run.db.to_str().unwrap())
        .expect("actual SQLite open and busy_timeout before schema access");
    let error = vectors.init_vec_table(384).unwrap_err();
    let message = actual_presence_error_message(&error);
    assert!(message.starts_with("Failed to check vec_bookmarks table: "), "first real schema failure: {error}");
    assert!(!message.contains("Failed to create vec_bookmarks table"));
    let source = std::error::Error::source(&error).unwrap()
        .downcast_ref::<rusqlite::Error>().expect("original corrupt SQLite schema cause");
    assert_eq!(source.sqlite_error_code(), Some(rusqlite::ErrorCode::NotADatabase));
    assert_eq!(std::fs::read(&run.db).unwrap(), corrupt);
    assert!(!run.root().join("native.db-wal").exists());
    drop(vectors);
    // This second real attachment must still refuse the same retained corrupt
    // file. It is a restart observation, not automatic production recovery.
    let reopened = SqliteVectorRepository::new(run.db.to_str().unwrap()).unwrap();
    let error = reopened.init_vec_table(384).unwrap_err();
    assert!(actual_presence_error_message(&error).starts_with("Failed to check vec_bookmarks table: "));
    assert_eq!(std::fs::read(&run.db).unwrap(), corrupt);
}

#[test]
fn given_real_missing_vector_schema_when_initialized_then_create_and_existing_restart_are_healthy() {
    let run = NativeRun::new();
    let id = run.seed("native schema continuity source", "native-proof");
    let repository = run.repository();
    let source = repository.get_by_id(id).unwrap().unwrap();
    let database = rusqlite::Connection::open(&run.db).unwrap();
    let before: bool = database.query_row(
        "SELECT COUNT(*) > 0 FROM sqlite_master WHERE type='table' AND name='vec_bookmarks'",
        [], |row| row.get(0)).unwrap();
    assert!(!before, "actual missing native vector table");
    let vectors = SqliteVectorRepository::new(run.db.to_str().unwrap()).unwrap();
    vectors.init_vec_table(384).unwrap();
    let after: bool = database.query_row(
        "SELECT COUNT(*) > 0 FROM sqlite_master WHERE type='table' AND name='vec_bookmarks'",
        [], |row| row.get(0)).unwrap();
    assert!(after);
    vectors.init_vec_table(384).unwrap();
    assert!(!vectors.has_embeddings().unwrap());
    let bytes = native_database_bytes(&run);
    drop(vectors);
    let reopened = SqliteVectorRepository::new(run.db.to_str().unwrap()).unwrap();
    reopened.init_vec_table(384).unwrap();
    assert!(!reopened.has_embeddings().unwrap());
    assert_eq!(native_database_bytes(&run), bytes);
    assert!(repository.get_by_id(id).unwrap().unwrap() == source);
}


#[test]
fn given_real_missing_table_when_native_cause_crosses_context_then_same_sqlite_failure_survives() {
    use bkmr::application::error::ApplicationError;
    use bkmr::cli::error::CliError;
    use bkmr::domain::error::DomainError;
    use bkmr::domain::error_context::CliErrorContext;

    let run = NativeRun::new();
    let id = run.seed("native original cause source", "native-proof");
    let repository = run.repository();
    let before = repository.get_by_id(id).unwrap().unwrap();
    let vectors = SqliteVectorRepository::new(run.db.to_str().unwrap()).unwrap();
    vectors.init_vec_table(384).unwrap();
    let database = rusqlite::Connection::open(&run.db).unwrap();
    database.execute_batch("DROP TABLE vec_bookmarks").unwrap();
    let bytes = native_database_bytes(&run);
    let failure = vectors.has_embeddings().unwrap_err();
    let original = std::error::Error::source(&failure).unwrap()
        .downcast_ref::<rusqlite::Error>().unwrap();
    assert_eq!(original.sqlite_error_code(), Some(rusqlite::ErrorCode::Unknown));
    let pointer = original as *const rusqlite::Error;
    let sqlite_text = original.to_string();
    let expected = format!("Application error: Domain error: Bookmark operation failed: native original cause: Failed to check vec_bookmarks count: {sqlite_text}");
    let contextual = Result::<(), ApplicationError>::Err(failure.into())
        .cli_context("native original cause").unwrap_err();
    assert_eq!(contextual.to_string(), expected);
    let application = std::error::Error::source(&contextual).unwrap()
        .downcast_ref::<ApplicationError>().unwrap();
    let domain = std::error::Error::source(application).unwrap()
        .downcast_ref::<DomainError>().unwrap();
    let sqlite = std::error::Error::source(domain).unwrap()
        .downcast_ref::<rusqlite::Error>().unwrap();
    assert_eq!(sqlite as *const rusqlite::Error, pointer);
    assert_eq!(sqlite.to_string(), sqlite_text);
    assert!(matches!(contextual, CliError::Application(_)));
    assert_eq!(native_database_bytes(&run), bytes);
    assert!(repository.get_by_id(id).unwrap().unwrap() == before);
}

#[test]
fn given_corrupt_native_database_when_production_cli_initializes_then_wal_failure_and_exit_are_preserved() {
    let run = NativeRun::new();
    let corrupt = vec![0x62u8; 4096];
    std::fs::write(&run.db, &corrupt).unwrap();
    let output = run.command().arg("hsearch").arg("native-source").arg("--json").arg("--np")
        .output().expect("actual compiled native CLI captured under its declared 20s fixture timeout");
    assert_eq!(output.status.code(), Some(bkmr::exitcode::USAGE));
    assert!(output.stdout.len() + output.stderr.len() <= 1024 * 1024, "post-completion observation bound only");
    assert!(output.stdout.is_empty(), "failed native setup must not manufacture JSON search data");
    let diagnostic = std::str::from_utf8(&output.stderr).unwrap();
    assert!(diagnostic.contains("Failed to create service container"));
    assert!(diagnostic.contains("Failed to create SQLite bookmark repository"));
    assert!(diagnostic.contains("Failed to set WAL journal mode"));
    assert_eq!(std::fs::read(&run.db).unwrap(), corrupt);
}
