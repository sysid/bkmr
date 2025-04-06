// bkmr/src/app_state.rs
use crate::domain::embedding::Embedder;
use crate::domain::error::{DomainError, DomainResult};
use crate::infrastructure::embeddings::dummy_provider::DummyEmbedding;
use std::sync::{Arc, OnceLock, RwLock};
use std::{env, fmt};
use std::path::Path;
use tracing::{debug, instrument};

// Import our new config module
use crate::config::{Settings, FzfOpts, load_settings};

/** Global AppState (“Service Locator” Style)
Global State Management:
The AppState struct and APP_STATE static variable provide global access to application state using
a thread-safe RwLock. This pattern is used throughout the codebase.

Service Locator Pattern:
The Context struct holds the embedder service which is used throughout the application.
This follows the service locator pattern.

Settings Storage:
The current implementation holds application settings and provides methods to update them.

API Compatibility:
Many parts of the codebase call methods like AppState::read_global() to access configuration and services.

need to keep AppState struct, but modify it to use our new configuration system internally.

  read-only access:
  let app_state = AppState::read_global();
  let db_url = &app_state.settings.db_url;
  let embedder = &app_state.context.embedder;

  This returns a read guard, preventing concurrent writes while it’s in use.

  write access:
  let mut new_state = AppState::new();
  new_state.settings.db_url = "/my/new/path.db".to_string();
  AppState::update_global(new_state)?;

  refresh from environment:
  AppState::reload_settings()?;
*/

// Keep the Context struct as-is
pub struct Context {
    pub embedder: Arc<dyn Embedder>,
}

impl fmt::Debug for Context {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Context")
            .field("embedder", &"Arc<dyn Embedder>")
            .finish()
    }
}

impl Context {
    pub fn new(embedder: Arc<dyn Embedder>) -> Self {
        Self { embedder }
    }
}

#[derive(Debug)]
pub struct AppState {
    // holds infrastructure-like services
    pub context: Context,
    // holds configuration settings
    pub settings: Settings,
}

impl Default for AppState {
    fn default() -> Self {
        Self::new(Arc::new(DummyEmbedding))
    }
}

impl AppState {
    pub fn new(embedder: Arc<dyn Embedder>) -> Self {
        Self::new_with_config_file(embedder, None)
    }

    pub fn new_with_config_file(embedder: Arc<dyn Embedder>, config_file: Option<&Path>) -> Self {
        // Load settings using the new configuration system with optional config file
        let settings = load_settings(config_file).unwrap_or_else(|e| {
            debug!("Failed to load settings: {}. Using defaults.", e);
            Settings::default()
        });

        Self {
            context: Context::new(embedder),
            settings,
        }
    }
}

pub static APP_STATE: OnceLock<RwLock<AppState>> = OnceLock::new();

//
// --- Public API for global access ---
impl AppState {
    /// Returns the global AppState lock (initializing if necessary).
    pub fn global() -> &'static RwLock<AppState> {
        APP_STATE.get_or_init(|| RwLock::new(AppState::new(Arc::new(DummyEmbedding))))
    }

    /// Acquire a read guard for the global AppState.
    #[instrument(level = "debug")]
    pub fn read_global() -> std::sync::RwLockReadGuard<'static, AppState> {
        Self::global()
            .read()
            .expect("Failed to acquire read lock for AppState")
    }

    /// Acquire a write guard and replace the global AppState.
    #[instrument(level = "debug")]
    pub fn update_global(new_state: AppState) -> DomainResult<()> {
        let mut guard = Self::global()
            .write()
            .map_err(|e| DomainError::Other(format!("Write lock error: {}", e)))?;
        *guard = new_state;
        Ok(())
    }

    /// Reload settings from the configuration files and environment variables.
    pub fn reload_settings() -> DomainResult<()> {
        Self::reload_settings_with_config(None)
    }

    /// Reload settings with a specific config file
    pub fn reload_settings_with_config(config_file: Option<&Path>) -> DomainResult<()> {
        let mut guard = Self::global()
            .write()
            .map_err(|e| DomainError::Other(format!("Write lock error: {}", e)))?;

        // Use the new configuration system
        guard.settings = load_settings(config_file)?;
        Ok(())
    }
}

// Implement tests for backward compatibility
#[cfg(test)]
mod tests {
    use super::*;
    use crate::util::testing::{init_test_env, EnvGuard};
    use serial_test::serial;

    /// Helper for environment variable cleanup
    // struct EnvGuard {
    //     db_url: Option<String>,
    //     fzf_opts: Option<String>,
    // }
    //
    // impl EnvGuard {
    //     fn new() -> Self {
    //         Self {
    //             db_url: env::var("BKMR_DB_URL").ok(),
    //             fzf_opts: env::var("BKMR_FZF_OPTS").ok(),
    //         }
    //     }
    // }
    //
    // impl Drop for EnvGuard {
    //     fn drop(&mut self) {
    //         env::remove_var("BKMR_DB_URL");
    //         env::remove_var("BKMR_FZF_OPTS");
    //         if let Some(val) = &self.db_url {
    //             env::set_var("BKMR_DB_URL", val);
    //         }
    //         if let Some(val) = &self.fzf_opts {
    //             env::set_var("BKMR_FZF_OPTS", val);
    //         }
    //     }
    // }

    #[test]
    #[serial]
    fn given_no_environment_when_new_then_defaults() {
        let _ = init_test_env();
        let _guard = EnvGuard::new();
        env::remove_var("BKMR_DB_URL");
        env::remove_var("BKMR_FZF_OPTS");

        let state = AppState::default();
        // Test that default values are used
        assert!(state.settings.db_url.contains("bkmr.db"));
        assert_eq!(state.settings.fzf_opts.height, "50%");
        assert!(!state.settings.fzf_opts.reverse);
        assert!(!state.settings.fzf_opts.show_tags);
    }

    #[test]
    #[serial]
    fn given_env_vars_when_new_then_overrides() {
        let _ = init_test_env();
        let _guard = EnvGuard::new();
        env::set_var("BKMR_DB_URL", "/test/db.db");
        env::set_var("BKMR_FZF_OPTS", "--height 99% --reverse --show-tags");

        let state = AppState::default();
        assert_eq!(state.settings.db_url, "/test/db.db");
        assert_eq!(state.settings.fzf_opts.height, "99%");
        assert!(state.settings.fzf_opts.reverse);
        assert!(state.settings.fzf_opts.show_tags);
    }

    #[test]
    #[serial]
    fn given_global_state_when_update_then_state_changes() {
        let _guard = EnvGuard::new();
        let mut state = AppState::default();
        state.settings.db_url = "/some/db/path.db".to_string();
        AppState::update_global(state).unwrap();

        let global = AppState::read_global();
        assert_eq!(global.settings.db_url, "/some/db/path.db");
    }

    #[test]
    #[serial]
    fn given_modified_env_when_reload_then_settings_change() {
        let _guard = EnvGuard::new();

        let mut initial = AppState::default();
        initial.settings.db_url = "/before-reload.db".to_string();
        AppState::update_global(initial).unwrap();

        env::set_var("BKMR_DB_URL", "/after-reload.db");
        AppState::reload_settings().unwrap();

        let reloaded = AppState::read_global();
        assert_eq!(reloaded.settings.db_url, "/after-reload.db");
    }
}
