// src/util/testing.rs

use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use tracing::{debug, info, instrument};
use tracing_subscriber::{
    filter::filter_fn,
    fmt::{self, format::FmtSpan},
    prelude::*,
    EnvFilter,
};

use crate::app_state::AppState;
use crate::infrastructure::repositories::sqlite::migration;
use crate::infrastructure::repositories::sqlite::repository::SqliteBookmarkRepository;

/// A struct that holds global test configuration and paths.
/// Everything is initialized exactly once via OnceLock.
#[derive(Debug)]
pub struct TestEnv {
    /// Path to your test database
    pub db_path: PathBuf,
    /// Paths to resource files
    pub resources: Vec<&'static str>,
}

impl TestEnv {
    /// Creates the default test configuration (paths, etc.).
    fn new() -> Self {
        Self {
            db_path: PathBuf::from("../db/bkmr.db"),
            resources: vec![
                "tests/resources/bkmr.v1.db",
                "tests/resources/bkmr.v2.db",
                "tests/resources/bkmr.v2.noembed.db",
            ],
        }
    }
}

/// Global OnceLock holding the TestEnv data.
static TEST_ENV: OnceLock<TestEnv> = OnceLock::new();

/// Initializes the global test environment exactly once.
/// - Sets up logging
/// - Updates global AppState
/// - Sets BKMR_DB_URL to match `TestEnv::db_path`
/// Returns a reference to the fully-initialized TestEnv.
pub fn init_test_env() -> &'static TestEnv {
    // Initialize test environment config, storing it in TEST_ENV exactly once.
    let env_data = TEST_ENV.get_or_init(|| {
        let data = TestEnv::new();
        setup_test_logging(); // set up logger only once
        AppState::update_global(AppState::default()).expect("Failed to update global AppState");
        info!("Test environment initialized with DummyEmbedding");
        data
    });
    env_data
}

/// Logging setup only runs once; subsequent calls do nothing if `tracing` is already set.
fn setup_test_logging() {
    debug!("Attempting logger init from testing.rs");
    if tracing::dispatcher::has_been_set() {
        debug!("Tracing subscriber already set");
        return;
    }

    if env::var("RUST_LOG").is_err() {
        env::set_var("RUST_LOG", "trace");
    }

    // Silence spammy modules
    env::set_var("SKIM_LOG", "info");
    env::set_var("TUIKIT_LOG", "info");

    let noisy_modules = [
        "skim",
        "html5ever",
        "reqwest",
        "mio",
        "want",
        "tuikit",
        "hyper_util",
    ];
    let module_filter = filter_fn(move |metadata| {
        !noisy_modules
            .iter()
            .any(|name| metadata.target().starts_with(name))
    });

    let env_filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("debug"));

    let subscriber = tracing_subscriber::registry().with(
        fmt::layer()
            .with_writer(std::io::stderr)
            .with_target(true)
            .with_thread_names(false)
            .with_span_events(FmtSpan::CLOSE)
            .with_filter(module_filter)
            .with_filter(env_filter),
    );

    subscriber.try_init().unwrap_or_else(|e| {
        eprintln!("Error: Failed to set up logging: {}", e);
    });
}

#[derive(Debug, Clone)]
pub struct EnvGuard {
    db_url: Option<String>,
    fzf_opts: Option<String>,
}

impl Default for EnvGuard {
    fn default() -> Self {
        Self::new()
    }
}

impl EnvGuard {
    pub fn new() -> Self {
        Self {
            db_url: env::var("BKMR_DB_URL").ok(),
            fzf_opts: env::var("BKMR_FZF_OPTS").ok(),
        }
    }
}

impl Drop for EnvGuard {
    #[instrument(level = "trace")]
    fn drop(&mut self) {
        env::remove_var("BKMR_DB_URL");
        env::remove_var("BKMR_FZF_OPTS");
        if let Some(val) = &self.db_url {
            env::set_var("BKMR_DB_URL", val);
        }
        if let Some(val) = &self.fzf_opts {
            env::set_var("BKMR_FZF_OPTS", val);
        }
    }
}

/// Creates a new repository with an initialized DB for testing.
pub fn setup_test_db() -> SqliteBookmarkRepository {
    let env_data = init_test_env();
    let repository =
        SqliteBookmarkRepository::from_url(env_data.db_path.to_string_lossy().as_ref())
            .expect("Failed to create SqliteBookmarkRepository");
    let mut conn = repository
        .get_connection()
        .expect("Failed to get connection from SqliteBookmarkRepository");
    migration::init_db(&mut conn).expect("Failed to initialize DB schema");
    repository
}

/// Creates a temporary directory and copies test resources into `../db`.
pub fn setup_temp_dir() -> PathBuf {
    use fs_extra::dir::CopyOptions;
    use tempfile::tempdir;

    let env_data = init_test_env(); // ensure global is initialized
    let tempdir = tempdir().expect("Failed to create temp dir");
    let options = CopyOptions::new().overwrite(true);

    fs_extra::copy_items(&env_data.resources, "../db", &options)
        .expect("Failed to copy test resources into ../db");

    tempdir.keep()
}

/// Removes the temp directory if NO_CLEANUP is not set; otherwise leaves artifacts.
pub fn teardown_temp_dir(temp_dir: &Path) {
    if env::var("NO_CLEANUP").is_err() && temp_dir.exists() {
        let _ = fs::remove_dir_all(temp_dir);
    } else {
        info!("Test artifacts left at: {}", temp_dir.display());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;

    #[test]
    fn my_test() {
        let test_env = init_test_env();
        let _guard = EnvGuard::new();
        assert!(test_env.db_path.exists());
        info!("test logic here");
    }

    #[test]
    #[serial]
    fn test_setup_test_db() {
        let _ = init_test_env();
        let repo = setup_test_db();
        assert!(repo.get_connection().is_ok());
    }

    #[test]
    fn test_setup_temp_dir() {
        let _ = init_test_env();
        let temp_dir = setup_temp_dir();
        assert!(temp_dir.exists());
        teardown_temp_dir(&temp_dir);
    }
}
