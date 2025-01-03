// src/util/testing.rs
use std::env;
use std::fs;
use std::path::PathBuf;

use anyhow::{Context as _, Result};
use camino::{Utf8Path, Utf8PathBuf};
use camino_tempfile::tempdir;
use fs_extra::{copy_items, dir};
use lazy_static::lazy_static;
use tracing::{debug, info};
use tracing_subscriber::{
    filter::filter_fn,
    fmt::{self, format::FmtSpan},
    prelude::*,
    EnvFilter,
};

use crate::adapter::dal::{migration, Dal};
use crate::adapter::embeddings::DummyEmbedding;
use crate::context::Context;

// Constants
pub const TEST_ENV_VARS: &[&str] = &["BKMR_DB_URL", "RUST_LOG", "NO_CLEANUP"];

lazy_static! {
    pub static ref TEST_DB_PATH: PathBuf = PathBuf::from("../db/bkmr.db");
    pub static ref TEST_RESOURCES: Vec<&'static str> = vec![
        "tests/resources/bkmr.v1.db",
        "tests/resources/bkmr.v2.db",
        "tests/resources/bkmr.v2.noembed.db"
    ];
}

pub fn init_test_setup() -> Result<()> {
    // Set up logging first
    setup_test_logging();

    debug!("Initializing test context with DummyEmbedding");
    Context::update_global(Context::new(Box::new(DummyEmbedding)))?;

    // Set up environment variables
    set_test_env_vars();

    info!("Test Setup complete");
    Ok(())
}

fn setup_test_logging() {
    debug!("INIT: Attempting logger init from testing.rs");
    if env::var("RUST_LOG").is_err() {
        env::set_var("RUST_LOG", "trace");
    }

    env::set_var("SKIM_LOG", "info");
    env::set_var("TUIKIT_LOG", "info");

    // Create a filter for noisy modules
    let noisy_modules = ["skim", "html5ever", "reqwest", "mio", "want", "tuikit"];
    let module_filter = filter_fn(move |metadata| {
        !noisy_modules
            .iter()
            .any(|name| metadata.target().starts_with(name))
    });

    // Set up the subscriber with environment filter
    let env_filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("debug"));

    // Build and set the subscriber
    let subscriber = tracing_subscriber::registry().with(
        fmt::layer()
            .with_writer(std::io::stderr)
            .with_target(true)
            .with_thread_names(false)
            .with_span_events(FmtSpan::ENTER)
            .with_span_events(FmtSpan::CLOSE)
            .with_filter(module_filter)
            .with_filter(env_filter),
    );

    // Only set if we haven't already set a global subscriber
    if tracing::dispatcher::has_been_set() {
        debug!("Tracing subscriber already set");
    } else {
        subscriber.try_init().unwrap_or_else(|e| {
            eprintln!("Error: Failed to set up logging: {}", e);
        });
    }
}

fn set_test_env_vars() {
    // Set default environment variables for testing
    env::set_var("BKMR_DB_URL", TEST_DB_PATH.to_str().unwrap());
}

pub fn setup_test_db() -> Result<Dal> {
    let mut dal = Dal::new(TEST_DB_PATH.to_string_lossy().to_string());
    migration::init_db(&mut dal.conn).context("Failed to initialize test database")?;
    Ok(dal)
}

pub fn setup_temp_dir() -> Result<Utf8PathBuf> {
    let tempdir = tempdir().context("Failed to create temp directory")?;
    let options = dir::CopyOptions::new().overwrite(true);

    copy_items(&TEST_RESOURCES, "../db", &options).context("Failed to copy test resources")?;

    Ok(tempdir.into_path())
}

pub fn print_active_env_vars() {
    for var in TEST_ENV_VARS {
        if let Ok(value) = env::var(var) {
            println!("{var}={value}");
        } else {
            println!("{var} is not set");
        }
    }
}

pub fn teardown_temp_dir(temp_dir: &Utf8Path) {
    if env::var("NO_CLEANUP").is_err() && temp_dir.exists() {
        let _ = fs::remove_dir_all(temp_dir);
    } else {
        debug!("Test artifacts left at: {}", temp_dir);
    }
}

#[cfg(test)]
mod tests {
    use crate::context::{Context, CTX};
    use super::*;
    

    #[ctor::ctor]
    fn init() {
        init_test_setup().expect("Failed to initialize test setup");
    }

    #[test]
    fn test_setup_test_db() {
        let result = setup_test_db();
        assert!(result.is_ok(), "Failed to setup test DB: {:?}", result);
    }

    #[test]
    fn test_setup_temp_dir() {
        let temp_dir = setup_temp_dir().expect("Failed to create temp dir");
        assert!(temp_dir.exists(), "Temp dir should exist");
        teardown_temp_dir(&temp_dir);
    }

    #[test]
    fn test_context_initialization() {
        assert!(CTX.get().is_some(), "Context should be initialized");
        // Verify we're using DummyEmbedding for tests
        let embedding = Context::read_global().get_embedding("test");
        assert!(embedding.is_none(), "DummyEmbedding should return None");
    }
}
