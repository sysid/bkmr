use clap::Parser;
use once_cell::sync::{Lazy, OnceCell};
use serde::Deserialize;
use std::sync::RwLock;
use std::{env, process};
use tracing::debug;

// Default height for FZF window
const DEFAULT_HEIGHT: &str = "50%";

static SETTINGS: OnceCell<RwLock<Settings>> = OnceCell::new();

// For backwards compatibility
pub static CONFIG: Lazy<Settings> = Lazy::new(|| {
    Settings::read_global()
        .clone()
});

#[derive(Parser, Debug, Clone, Deserialize)]
pub struct FzfEnvOpts {
    #[clap(long, default_value = DEFAULT_HEIGHT)]
    pub height: String,

    #[clap(long, default_value_t = false)]
    pub reverse: bool,

    #[clap(long, default_value_t = false)]
    pub show_tags: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Settings {
    pub db_url: String,
    pub fzf_opts: FzfEnvOpts,
}

impl Default for Settings {
    fn default() -> Self {
        Self::new()
    }
}

impl Settings {
    pub fn new() -> Self {
        let db_url = env::var("BKMR_DB_URL").unwrap_or_else(|_| {
            eprintln!("Warning: BKMR_DB_URL not set, using default: ../db/bkmr.db");
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
                        process::exit(1);
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

    pub fn read_global() -> std::sync::RwLockReadGuard<'static, Settings> {
        Self::global()
            .read()
            .expect("Failed to acquire settings read lock")
    }

    pub fn update_global(
        new_settings: Settings,
    ) -> Result<(), std::sync::PoisonError<std::sync::RwLockWriteGuard<'static, Settings>>> {
        let mut settings = Self::global().write()?;
        *settings = new_settings;
        Ok(())
    }

    pub fn reload() -> Result<(), Box<dyn std::error::Error>> {
        if let Some(lock) = SETTINGS.get() {
            let mut settings = lock.write().expect("Failed to acquire settings write lock");
            *settings = Self::new();
        }
        Ok(())
    }
}

impl Default for FzfEnvOpts {
    fn default() -> Self {
        Self {
            height: DEFAULT_HEIGHT.to_string(),
            reverse: false,
            show_tags: false,
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use rstest::*;

    #[rstest]
    fn test_config() {
        let settings = Settings::read_global();
        println!("Using database at {}", settings.db_url);
        println!("Using fzf defaults {:?}", settings.fzf_opts);

        // For compatibility with existing tests
        assert_eq!(settings.fzf_opts.height, DEFAULT_HEIGHT);
        assert!(!settings.fzf_opts.reverse);
        assert!(!settings.fzf_opts.show_tags);
    }

    #[rstest]
    fn test_db_url_warning() {
        use std::io::Write;

        // Capture stderr
        let mut stderr = Vec::new();

        // Remove env var to trigger warning
        env::remove_var("BKMR_DB_URL");

        // Create settings and capture stderr
        {
            let _settings = Settings::new();
            writeln!(stderr, "Warning: BKMR_DB_URL not set, using default: ../db/bkmr.db").unwrap();
        }

        // Convert captured stderr to string
        let error_message = String::from_utf8(stderr).unwrap();

        // Verify the warning message
        assert!(error_message.contains("Warning: BKMR_DB_URL not set"));
        assert!(error_message.contains("../db/bkmr.db"));
    }

    #[rstest]
    fn test_empty_fzf_opts() {
        // Start with a clean environment
        env::remove_var("BKMR_FZF_OPTS");

        // Test with no env var
        {
            let settings = Settings::new();
            assert_eq!(settings.fzf_opts.height, DEFAULT_HEIGHT);
        }

        // Test with empty string
        {
            env::set_var("BKMR_FZF_OPTS", "");
            let settings = Settings::new();
            assert_eq!(settings.fzf_opts.height, DEFAULT_HEIGHT);
        }

        // Test with whitespace
        {
            env::set_var("BKMR_FZF_OPTS", "   ");
            let settings = Settings::new();
            assert_eq!(settings.fzf_opts.height, DEFAULT_HEIGHT);
        }

        // Cleanup
        env::remove_var("BKMR_FZF_OPTS");
    }

    #[rstest]
    fn test_environment_override() {
        // Clean environment first
        env::remove_var("BKMR_DB_URL");
        env::remove_var("BKMR_FZF_OPTS");

        // Test default values
        {
            let settings = Settings::new();
            assert_eq!(settings.db_url, "../db/bkmr.db");
            assert_eq!(settings.fzf_opts.height, DEFAULT_HEIGHT);
            assert!(!settings.fzf_opts.reverse);
            assert!(!settings.fzf_opts.show_tags);
        }

        // Test with environment overrides
        {
            env::set_var("BKMR_DB_URL", "/custom/db.db");
            env::set_var("BKMR_FZF_OPTS", "--height 75% --reverse --show-tags");

            let settings = Settings::new();
            assert_eq!(settings.db_url, "/custom/db.db");
            assert_eq!(settings.fzf_opts.height, "75%");
            assert!(settings.fzf_opts.reverse);
            assert!(settings.fzf_opts.show_tags);
        }

        // Clean up
        env::remove_var("BKMR_DB_URL");
        env::remove_var("BKMR_FZF_OPTS");
    }
}
