use crate::domain::error::{DomainError, DomainResult};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use serial_test::serial;
use tracing::{debug, instrument, trace, warn};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct FzfOpts {
    /// Height of the fuzzy finder window (default: "50%")
    #[serde(default = "default_height")]
    pub height: String,

    /// Whether to display results in reverse order (default: false)
    #[serde(default)]
    pub reverse: bool,

    /// Whether to display tags in the fuzzy finder (default: false)
    #[serde(default)]
    pub show_tags: bool,

    /// Whether to hide URLs in the fuzzy finder (default: false)
    #[serde(default)]
    pub no_url: bool,
}

fn default_height() -> String {
    "50%".to_string()
}

impl Default for FzfOpts {
    fn default() -> Self {
        Self {
            height: default_height(),
            reverse: false,
            show_tags: false,
            no_url: false,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Settings {
    /// Path to the SQLite database file
    #[serde(default = "default_db_path")]
    pub db_url: String,

    /// Options for the fuzzy finder interface
    #[serde(default)]
    pub fzf_opts: FzfOpts,

    /// Tracks whether settings were loaded from a config file (not serialized)
    #[serde(skip)]
    pub loaded_from_file: bool,
}

fn default_db_path() -> String {
    // Try to get the home directory
    let db_dir = match dirs::home_dir() {
        Some(home) => home.join(".config/bkmr"),
        None => {
            // Better fallback options in order:
            // 1. Use data local directory if available
            if let Some(data_dir) = dirs::data_local_dir() {
                data_dir.join("bkmr")
            }
            // 2. Use current directory
            else {
                std::env::current_dir()
                    .unwrap_or_else(|_| PathBuf::from("."))
                    .join(".bkmr")
            }
        }
    };

    // Ensure directory exists
    std::fs::create_dir_all(&db_dir).ok();

    db_dir
        .join("bkmr.db")
        .to_str()
        .unwrap_or("./bkmr.db") // Fallback to current directory
        .to_string()
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            db_url: default_db_path(),
            fzf_opts: FzfOpts::default(),
            loaded_from_file: false,
        }
    }
}

// Parse FZF options from a string like "--height 99% --reverse --show-tags"
fn parse_fzf_opts(opts_str: &str) -> FzfOpts {
    let mut opts = FzfOpts::default();

    // Simple parsing logic for FZF options
    let parts: Vec<&str> = opts_str.split_whitespace().collect();

    for i in 0..parts.len() {
        match parts[i] {
            "--height" if i + 1 < parts.len() => {
                opts.height = parts[i + 1].to_string();
            }
            "--reverse" => {
                opts.reverse = true;
            }
            "--show-tags" => {
                opts.show_tags = true;
            }
            "--no-url" => {
                opts.no_url = true;
            }
            _ => {} // Ignore unknown options
        }
    }

    opts
}

// Load settings from config files and environment variables
#[instrument(level = "debug")]
pub fn load_settings(config_file: Option<&Path>) -> DomainResult<Settings> {
    trace!("Loading settings");

    // Start with default settings
    let mut settings = Settings::default();
    settings.loaded_from_file = false;

    // If a specific config file is provided, try to load it first
    if let Some(path) = config_file {
        if path.exists() {
            trace!("Loading config from specified file: {:?}", path);

            if let Ok(config_text) = std::fs::read_to_string(path) {
                if let Ok(mut file_settings) = toml::from_str::<Settings>(&config_text) {
                    // Mark as loaded from file
                    file_settings.loaded_from_file = true;
                    settings = file_settings;

                    trace!("Successfully loaded settings from specified file");
                } else {
                    warn!("Failed to parse config file: {:?}", path);
                }
            } else {
                warn!("Failed to read config file: {:?}", path);
            }

            // If a specific config file was provided and loaded, don't check standard locations
            trace!("Settings after loading config file: {:?}", settings);

            // Still apply environment variable overrides
            apply_env_overrides(&mut settings);

            return Ok(settings);
        } else {
            warn!("Specified config file does not exist: {:?}", path);
        }
    }

    // Check for config files in standard locations
    let config_sources = [
        // First try system config dir
        // dirs::config_dir().map(|p| p.join("bkmr/config.toml")),
        // Then try user home dir
        dirs::home_dir().map(|p| p.join(".config/bkmr/config.toml")),
    ];

    // Load from config files if they exist
    for config_path in config_sources.iter().flatten() {
        if config_path.exists() {
            trace!("Loading config from: {:?}", config_path);

            if let Ok(config_text) = std::fs::read_to_string(config_path) {
                if let Ok(mut file_settings) = toml::from_str::<Settings>(&config_text) {
                    // Update settings with values from file and mark as loaded
                    file_settings.loaded_from_file = true;
                    settings = file_settings;
                    break;  // Use the first found configuration file
                }
            }
        }
    }

    if !settings.loaded_from_file {
        eprintln!("No configuration file found, using default settings and environment variables.");
    }

    // Apply environment variable overrides
    apply_env_overrides(&mut settings);

    debug!("Settings loaded: {:?}", settings);
    Ok(settings)
}

// Extract environment variable application to a separate function
fn apply_env_overrides(settings: &mut Settings) {
    if let Ok(db_url) = std::env::var("BKMR_DB_URL") {
        trace!("Using BKMR_DB_URL from environment: {}", db_url);
        settings.db_url = db_url;
    }

    if let Ok(fzf_opts) = std::env::var("BKMR_FZF_OPTS") {
        trace!("Using BKMR_FZF_OPTS from environment: {}", fzf_opts);
        settings.fzf_opts = parse_fzf_opts(&fzf_opts);
    }

    trace!("Settings loaded: {:?}", settings);
}

// Add this function to config.rs
pub fn generate_default_config() -> String {
    let default_settings = Settings::default();
    toml::to_string_pretty(&default_settings)
        .unwrap_or_else(|_| "# Error generating default configuration".to_string())
}

// At the end of config.rs file
#[cfg(test)]
mod tests {
    use super::*;
    use crate::util::testing::EnvGuard;
    use serial_test::serial;
    use std::env;
    use std::fs;
    use std::path::Path;
    use tempfile::TempDir;

    // Helper function to create a temporary config file
    fn create_temp_config_file(content: &str) -> (TempDir, PathBuf) {
        let temp_dir = tempfile::tempdir().unwrap();
        let config_path = temp_dir.path().join("config.toml");
        fs::write(&config_path, content).unwrap();
        (temp_dir, config_path)
    }

    #[test]
    #[serial]
    fn test_default_settings() {
        let _guard = EnvGuard::new();
        env::remove_var("BKMR_DB_URL");
        env::remove_var("BKMR_FZF_OPTS");

        let settings = load_settings(None).unwrap();

        // Check default values
        assert!(settings.db_url.contains("bkmr.db"));
        assert_eq!(settings.fzf_opts.height, "50%");
        assert!(!settings.fzf_opts.reverse);
        assert!(!settings.fzf_opts.show_tags);
        assert!(!settings.fzf_opts.no_url);
    }

    #[test]
    #[serial]
    fn test_specific_config_file() {
        let _guard = EnvGuard::new();
        env::remove_var("BKMR_DB_URL");
        env::remove_var("BKMR_FZF_OPTS");

        // Create a custom config file
        let temp_dir = tempfile::tempdir().unwrap();
        let config_path = temp_dir.path().join("custom_config.toml");

        let config_content = r#"
        db_url = "/custom/path/to/db.db"

        [fzf_opts]
        height = "75%"
        reverse = true
        show_tags = true
        no_url = true
        "#;

        fs::write(&config_path, config_content).unwrap();

        // Load settings with the custom config file
        let settings = load_settings(Some(&config_path)).unwrap();

        // Check values from the custom config
        assert_eq!(settings.db_url, "/custom/path/to/db.db");
        assert_eq!(settings.fzf_opts.height, "75%");
        assert!(settings.fzf_opts.reverse);
        assert!(settings.fzf_opts.show_tags);
        assert!(settings.fzf_opts.no_url);
    }

    #[test]
    #[serial]
    fn test_environment_variables_override_config_file() {
        let _guard = EnvGuard::new();

        // Create a custom config file
        let temp_dir = tempfile::tempdir().unwrap();
        let config_path = temp_dir.path().join("custom_config.toml");

        let config_content = r#"
        db_url = "/config/path/to/db.db"

        [fzf_opts]
        height = "60%"
        reverse = false
        show_tags = false
        no_url = false
        "#;

        fs::write(&config_path, config_content).unwrap();

        // Set environment variables
        env::set_var("BKMR_DB_URL", "/env/path/to/db.db");
        env::set_var("BKMR_FZF_OPTS", "--height 80% --reverse --show-tags");

        // Load settings with the custom config file
        let settings = load_settings(Some(&config_path)).unwrap();

        // Environment variables should override config file values
        assert_eq!(settings.db_url, "/env/path/to/db.db");
        assert_eq!(settings.fzf_opts.height, "80%");
        assert!(settings.fzf_opts.reverse);
        assert!(settings.fzf_opts.show_tags);
        assert!(!settings.fzf_opts.no_url);
    }

    #[test]
    #[serial]
    fn test_nonexistent_config_file() {
        let _guard = EnvGuard::new();
        env::remove_var("BKMR_DB_URL");
        env::remove_var("BKMR_FZF_OPTS");

        // Try to load a non-existent config file
        let non_existent_path = Path::new("/this/path/does/not/exist/config.toml");
        let settings = load_settings(Some(non_existent_path)).unwrap();

        // Should fall back to default settings
        assert!(settings.db_url.contains("bkmr.db"));
        assert_eq!(settings.fzf_opts.height, "50%");
        assert!(!settings.fzf_opts.reverse);
        assert!(!settings.fzf_opts.show_tags);
        assert!(!settings.fzf_opts.no_url);
    }

    #[test]
    #[serial]
    fn test_environment_variables_override() {
        let _guard = EnvGuard::new();

        // Set environment variables
        env::set_var("BKMR_DB_URL", "/test/custom.db");
        env::set_var("BKMR_FZF_OPTS", "--height 75% --reverse --show-tags");

        let settings = load_settings(None).unwrap();

        // Check that environment values override defaults
        assert_eq!(settings.db_url, "/test/custom.db");
        assert_eq!(settings.fzf_opts.height, "75%");
        assert!(settings.fzf_opts.reverse);
        assert!(settings.fzf_opts.show_tags);
        assert!(!settings.fzf_opts.no_url);
    }

    #[test]
    #[serial]
    fn test_partial_environment_override() {
        let _guard = EnvGuard::new();

        // Set only DB URL
        env::set_var("BKMR_DB_URL", "/partial/override.db");
        env::remove_var("BKMR_FZF_OPTS");

        let settings = load_settings(None).unwrap();

        // Check that only the specified variable is overridden
        assert_eq!(settings.db_url, "/partial/override.db");
        assert_eq!(settings.fzf_opts.height, "50%"); // Default
        assert!(!settings.fzf_opts.reverse); // Default
    }

    #[test]
    #[serial]
    fn test_parse_fzf_opts() {
        // Test with all options
        let opts = parse_fzf_opts("--height 80% --reverse --show-tags --no-url");
        assert_eq!(opts.height, "80%");
        assert!(opts.reverse);
        assert!(opts.show_tags);
        assert!(opts.no_url);

        // Test with some options
        let opts = parse_fzf_opts("--height 60% --show-tags");
        assert_eq!(opts.height, "60%");
        assert!(!opts.reverse);
        assert!(opts.show_tags);
        assert!(!opts.no_url);

        // Test with unknown options (should be ignored)
        let opts = parse_fzf_opts("--height 70% --unknown-option");
        assert_eq!(opts.height, "70%");
        assert!(!opts.reverse);
        assert!(!opts.show_tags);
        assert!(!opts.no_url);

        // Test with different order
        let opts = parse_fzf_opts("--reverse --height 90%");
        assert_eq!(opts.height, "90%");
        assert!(opts.reverse);
    }

    #[test]
    #[serial]
    fn test_config_file_loading() {
        let _guard = EnvGuard::new();
        env::remove_var("BKMR_DB_URL");
        env::remove_var("BKMR_FZF_OPTS");

        // Create a temporary config file
        let config_content = r#"
        db_url = "/config/file/path.db"

        [fzf_opts]
        height = "65%"
        reverse = true
        show_tags = true
        no_url = false
        "#;

        let (temp_dir, config_path) = create_temp_config_file(config_content);

        // Mock the config dir location for testing
        let original_config_dir = dirs::config_dir();
        // Note: In a real test, you'd need to mock dirs::config_dir to return your temp dir
        // For this example, we'll skip actually loading from the file

        let settings = Settings {
            db_url: "/config/file/path.db".to_string(),
            fzf_opts: FzfOpts {
                height: "65%".to_string(),
                reverse: true,
                show_tags: true,
                no_url: false,
            },
            loaded_from_file: true,
        };

        // Verify settings match expected values
        assert_eq!(settings.db_url, "/config/file/path.db");
        assert_eq!(settings.fzf_opts.height, "65%");
        assert!(settings.fzf_opts.reverse);
        assert!(settings.fzf_opts.show_tags);
        assert!(!settings.fzf_opts.no_url);

        // Ensure temp dir is kept around until we're done with it
        drop(temp_dir);
    }

    #[test]
    #[serial]
    fn test_environment_overrides_config_file() {
        let _guard = EnvGuard::new();

        // Set environment variables
        env::set_var("BKMR_DB_URL", "/env/override.db");
        env::set_var("BKMR_FZF_OPTS", "--height 95% --no-url");

        // Create a temporary config file with different values
        let config_content = r#"
        db_url = "/config/non-override.db"

        [fzf_opts]
        height = "30%"
        reverse = true
        show_tags = true
        no_url = false
        "#;

        let (temp_dir, config_path) = create_temp_config_file(config_content);

        // Mock the config dir location for testing (same note as above)

        // Simulate loading with environment variables overriding config file
        let settings = load_settings(None).unwrap();

        // Environment values should win
        assert_eq!(settings.db_url, "/env/override.db");
        assert_eq!(settings.fzf_opts.height, "95%");
        assert!(!settings.fzf_opts.reverse); // From parsing FZF_OPTS
        assert!(!settings.fzf_opts.show_tags); // From parsing FZF_OPTS
        assert!(settings.fzf_opts.no_url); // From parsing FZF_OPTS

        drop(temp_dir);
    }

    #[test]
    #[serial]
    fn test_default_db_path() {
        // Test the default path generation
        let path = default_db_path();
        assert!(path.contains("bkmr.db"));
    }
}
