use clap::Parser;
use once_cell::sync::{Lazy, OnceCell};
use serde::Deserialize;
use std::env;
use std::path::PathBuf;
use std::sync::RwLock;
use thiserror::Error;
use tracing::{debug, info};

// Default height for FZF window
const DEFAULT_HEIGHT: &str = "50%";

static SETTINGS: OnceCell<RwLock<Settings>> = OnceCell::new();

// For backwards compatibility
pub static CONFIG: Lazy<Settings> = Lazy::new(|| {
    match Settings::read_global() {
        Ok(guard) => guard.clone(),
        Err(e) => {
            eprintln!("Warning: Failed to read global settings: {}", e);
            Settings::new()
        }
    }
});

#[derive(Error, Debug)]
pub enum ConfigError {
    #[error("Failed to acquire settings lock: {0}")]
    LockError(String),
}

pub type ConfigResult<T> = std::result::Result<T, ConfigError>;

#[derive(Parser, Debug, Clone, Deserialize)]
pub struct FzfEnvOpts {
    #[clap(long, default_value = DEFAULT_HEIGHT)]
    pub height: String,

    #[clap(long, default_value_t = false)]
    pub reverse: bool,

    #[clap(long, default_value_t = false)]
    pub show_tags: bool,

    #[clap(long, default_value_t = false)]
    pub no_url: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Settings {
    pub db_url: String,
    pub fzf_opts: FzfEnvOpts,
}

impl Default for FzfEnvOpts {
    fn default() -> Self {
        Self {
            height: DEFAULT_HEIGHT.to_string(),
            reverse: false,
            show_tags: false,
            no_url: false, // Default to showing URLs
        }
    }
}

impl Default for Settings {
    fn default() -> Self {
        Self::new()
    }
}

impl Settings {
    pub fn new() -> Self {
        let db_url = env::var("BKMR_DB_URL").unwrap_or_else(|_| {
            debug!("BKMR_DB_URL not set, using default: ../db/bkmr.db");
            "../db/bkmr.db".to_string()
        });

        let fzf_opts = env::var("BKMR_FZF_OPTS")
            .map(|options_string| {
                if options_string.trim().is_empty() {
                    return FzfEnvOpts::default();
                }

                let mut args = options_string
                    .split_whitespace() // Better handling of multiple spaces
                    .filter(|s| !s.is_empty())
                    .collect::<Vec<_>>();

                if args.is_empty() {
                    return FzfEnvOpts::default();
                }

                args.insert(0, "");
                match FzfEnvOpts::try_parse_from(&args) {
                    Ok(opts) => opts,
                    Err(e) => {
                        eprintln!(
                            "Error: Failed to parse BKMR_FZF_OPTS: {:?}\nError: {}\nPlease check bkmr documentation.",
                            args, e
                        );
                        FzfEnvOpts::default()
                    }
                }
            })
            .unwrap_or_default();

        let settings = Settings { db_url, fzf_opts };
        debug!("Settings initialized: {:?}", settings);
        settings
    }

    pub fn global() -> &'static RwLock<Settings> {
        SETTINGS.get_or_init(|| RwLock::new(Self::new()))
    }

    pub fn read_global() -> ConfigResult<std::sync::RwLockReadGuard<'static, Settings>> {
        Self::global()
            .read()
            .map_err(|e| ConfigError::LockError(e.to_string()))
    }

    pub fn update_global(new_settings: Settings) -> ConfigResult<()> {
        let mut settings = Self::global()
            .write()
            .map_err(|e| ConfigError::LockError(e.to_string()))?;
        *settings = new_settings;
        Ok(())
    }

    pub fn reload() -> ConfigResult<()> {
        if let Some(lock) = SETTINGS.get() {
            let mut settings = lock
                .write()
                .map_err(|e| ConfigError::LockError(e.to_string()))?;
            *settings = Self::new();
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::*;
    use std::env;
    use serial_test::serial;

    // Helper for reliable environment variable cleanup
    struct EnvGuard {
        db_url: Option<String>,
        fzf_opts: Option<String>,
    }

    impl EnvGuard {
        fn new() -> Self {
            Self {
                db_url: env::var("BKMR_DB_URL").ok(),
                fzf_opts: env::var("BKMR_FZF_OPTS").ok(),
            }
        }
    }

    impl Drop for EnvGuard {
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

    #[test]
    #[serial]
    fn test_default_config() {
        // Guard environment variables
        let _guard = EnvGuard::new();
        env::remove_var("BKMR_DB_URL");
        env::remove_var("BKMR_FZF_OPTS");

        let settings = Settings::new();
        assert_eq!(settings.db_url, "../db/bkmr.db");
        assert_eq!(settings.fzf_opts.height, DEFAULT_HEIGHT);
        assert!(!settings.fzf_opts.reverse);
        assert!(!settings.fzf_opts.show_tags);
    }

    #[test]
    #[serial]
    fn test_environment_override() {
        // Guard environment variables
        let _guard = EnvGuard::new();

        // Set environment variables
        env::set_var("BKMR_DB_URL", "/custom/db.db");
        env::set_var("BKMR_FZF_OPTS", "--height 75% --reverse --show-tags");

        let settings = Settings::new();
        assert_eq!(settings.db_url, "/custom/db.db");
        assert_eq!(settings.fzf_opts.height, "75%");
        assert!(settings.fzf_opts.reverse);
        assert!(settings.fzf_opts.show_tags);
    }

    #[test]
    #[serial]
    fn test_empty_fzf_opts() {
        // Guard environment variables
        let _guard = EnvGuard::new();
        env::remove_var("BKMR_DB_URL");

        // Test with no env var
        env::remove_var("BKMR_FZF_OPTS");
        let settings = Settings::new();
        assert_eq!(settings.fzf_opts.height, DEFAULT_HEIGHT);

        // Test with empty string
        env::set_var("BKMR_FZF_OPTS", "");
        let settings = Settings::new();
        assert_eq!(settings.fzf_opts.height, DEFAULT_HEIGHT);

        // Test with whitespace
        env::set_var("BKMR_FZF_OPTS", "   ");
        let settings = Settings::new();
        assert_eq!(settings.fzf_opts.height, DEFAULT_HEIGHT);
    }

    #[test]
    #[serial]
    fn test_global_instance() -> ConfigResult<()> {
        // Guard environment variables
        let _guard = EnvGuard::new();
        env::set_var("BKMR_DB_URL", "/test-global/db.db");

        // Update settings and check global access
        let settings = Settings::new();
        Settings::update_global(settings)?;

        let global = Settings::read_global()?;
        assert_eq!(global.db_url, "/test-global/db.db");

        Ok(())
    }

    #[test]
    #[serial]
    fn test_reload_settings() -> ConfigResult<()> {
        // Guard environment variables
        let _guard = EnvGuard::new();

        // Set initial value and update global
        env::set_var("BKMR_DB_URL", "/before-reload/db.db");
        let initial = Settings::new();
        Settings::update_global(initial)?;

        // Change environment and reload
        env::set_var("BKMR_DB_URL", "/after-reload/db.db");
        Settings::reload()?;

        // Verify reload took effect
        let reloaded = Settings::read_global()?;
        assert_eq!(reloaded.db_url, "/after-reload/db.db");

        Ok(())
    }
}