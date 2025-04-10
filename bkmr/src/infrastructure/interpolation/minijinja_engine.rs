// src/infrastructure/interpolation/minijinja_engine.rs
use crate::domain::bookmark::Bookmark;
use crate::domain::interpolation::{
    errors::InterpolationError,
    interface::{InterpolationEngine, ShellCommandExecutor},
};
use chrono::{DateTime, Utc};
use minijinja::{Environment, Error, ErrorKind, Value};
use std::collections::HashMap;
use std::process::Command;
use std::sync::Arc;
use tracing::info;

// #[derive(Debug)]
pub struct MiniJinjaEngine {
    env: Environment<'static>,
}

impl std::fmt::Debug for MiniJinjaEngine {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MiniJinjaEngine")
            .field("env", &"<Environment>")
            .finish()
    }
}

impl MiniJinjaEngine {
    pub fn new(shell_executor: Arc<dyn ShellCommandExecutor>) -> Self {
        let mut env = Environment::new();

        // Register interpolation filters
        env.add_filter("strftime", date_format);
        env.add_filter("subtract_days", subtract_days);
        env.add_filter("add_days", add_days);

        // Create shell filter with captured executor
        let shell_executor_clone = Arc::clone(&shell_executor);
        env.add_filter("shell", move |value: Value| {
            let cmd = value.as_str().ok_or_else(|| {
                Error::new(ErrorKind::InvalidOperation, "Expected string command")
            })?;

            match shell_executor_clone.execute(cmd) {
                Ok(result) => Ok(Value::from(result)),
                Err(e) => Err(Error::new(ErrorKind::InvalidOperation, e.to_string())),
            }
        });

        // Add a global function for environment variables with defaults
        env.add_function("env", |name: String, args: &[Value]| {
            let default_value = args.first().cloned().unwrap_or_else(|| Value::from(""));
            match std::env::var(&name) {
                Ok(value) => Value::from(value),
                Err(_) => default_value,
            }
        });

        Self { env }
    }

    fn create_context(&self, bookmark: Option<&Bookmark>) -> HashMap<String, Value> {
        let mut context = HashMap::new();

        // Add current date/time
        context.insert(
            "current_date".to_string(),
            Value::from(Utc::now().to_rfc3339()),
        );

        // Add bookmark data if available
        if let Some(bm) = bookmark {
            context.insert("id".to_string(), Value::from(bm.id));
            context.insert("title".to_string(), Value::from(bm.title.clone()));
            context.insert(
                "description".to_string(),
                Value::from(bm.description.clone()),
            );

            // Convert tags to a list of strings
            let tags: Vec<String> = bm.tags.iter().map(|tag| tag.value().to_string()).collect();
            context.insert("tags".to_string(), Value::from(tags));

            context.insert("access_count".to_string(), Value::from(bm.access_count));
            context.insert(
                "created_at".to_string(),
                Value::from(bm.created_at.to_rfc3339()),
            );
            context.insert(
                "updated_at".to_string(),
                Value::from(bm.updated_at.to_rfc3339()),
            );
        }

        // Add environment variables
        for (key, value) in std::env::vars() {
            context.insert(format!("env_{}", key), Value::from(value));
        }

        context
    }
    fn render_template(
        &self,
        url: &str,
        bookmark: Option<&Bookmark>,
    ) -> Result<String, InterpolationError> {
        // Skip rendering if no template markers present
        if !url.contains("{{") && !url.contains("{%") {
            return Ok(url.to_string());
        }

        let template = self
            .env
            .template_from_str(url)
            .map_err(|e| InterpolationError::Syntax(e.to_string()))?;

        let context = self.create_context(bookmark);

        template
            .render(context)
            .map_err(|e| InterpolationError::Rendering(e.to_string()))
    }
}

impl InterpolationEngine for MiniJinjaEngine {
    fn render_url(&self, url: &str) -> Result<String, InterpolationError> {
        self.render_template(url, None)
    }

    fn render_bookmark_url(&self, bookmark: &Bookmark) -> Result<String, InterpolationError> {
        self.render_template(&bookmark.url, Some(bookmark))
    }
}

// Safe shell executor implementation
#[derive(Clone, Debug)]
pub struct SafeShellExecutor;

impl Default for SafeShellExecutor {
    fn default() -> Self {
        Self::new()
    }
}

impl SafeShellExecutor {
    pub fn new() -> Self {
        Self
    }

    fn is_command_safe(&self, cmd: &str) -> bool {
        let dangerous_patterns = [
            ";", "|", "&", ">", "<", "`", "$", "(", ")", "{", "}", "[", "]", "sudo", "rm", "mv",
            "cp", "dd", "mkfs", "fork", "kill",
        ];

        !dangerous_patterns
            .iter()
            .any(|pattern| cmd.contains(pattern))
    }
}

impl ShellCommandExecutor for SafeShellExecutor {
    fn execute(&self, cmd: &str) -> Result<String, InterpolationError> {
        info!("Executing shell command: {}", cmd);

        if !self.is_command_safe(cmd) {
            return Err(InterpolationError::Shell(
                "Command contains forbidden patterns".to_string(),
            ));
        }

        let output = Command::new("sh")
            .arg("-c")
            .arg(cmd)
            .output()
            .map_err(|e| InterpolationError::Shell(format!("Failed to execute command: {}", e)))?;

        if !output.status.success() {
            let error = String::from_utf8_lossy(&output.stderr);
            return Err(InterpolationError::Shell(format!(
                "Command failed: {}",
                error
            )));
        }

        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    }

    fn arc_clone(&self) -> Arc<dyn ShellCommandExecutor> {
        Arc::new(self.clone())
    }
}

// Filter implementations
fn date_format(value: Value, args: &[Value]) -> Result<Value, Error> {
    let date_str = value
        .as_str()
        .ok_or_else(|| Error::new(ErrorKind::InvalidOperation, "Expected string date"))?;
    let format = args.first().and_then(|v| v.as_str()).unwrap_or("%Y-%m-%d");

    let date = DateTime::parse_from_rfc3339(date_str)
        .map_err(|e| Error::new(ErrorKind::InvalidOperation, format!("Invalid date: {}", e)))?
        .with_timezone(&Utc);

    Ok(Value::from(date.format(format).to_string()))
}

fn subtract_days(value: Value, args: &[Value]) -> Result<Value, Error> {
    let date_str = value
        .as_str()
        .ok_or_else(|| Error::new(ErrorKind::InvalidOperation, "Expected date string"))?;

    let date = DateTime::parse_from_rfc3339(date_str)
        .map_err(|e| Error::new(ErrorKind::InvalidOperation, format!("Invalid date: {}", e)))?;

    let days = args.first().and_then(|v| v.as_i64()).unwrap_or(0);
    let new_date = date - chrono::Duration::days(days);

    Ok(Value::from(new_date.to_rfc3339()))
}

fn add_days(value: Value, args: &[Value]) -> Result<Value, Error> {
    let date_str = value
        .as_str()
        .ok_or_else(|| Error::new(ErrorKind::InvalidOperation, "Expected date string"))?;

    let date = DateTime::parse_from_rfc3339(date_str)
        .map_err(|e| Error::new(ErrorKind::InvalidOperation, format!("Invalid date: {}", e)))?;

    let days = args.first().and_then(|v| v.as_i64()).unwrap_or(0);
    let new_date = date + chrono::Duration::days(days);

    Ok(Value::from(new_date.to_rfc3339()))
}
