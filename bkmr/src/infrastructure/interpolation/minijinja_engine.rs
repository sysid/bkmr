// src/infrastructure/interpolation/minijinja_engine.rs
use crate::domain::bookmark::Bookmark;
use crate::domain::interpolation::{
    errors::InterpolationError,
    interface::{InterpolationEngine, ShellCommandExecutor},
};
use chrono::{DateTime, Utc};
use minijinja::{Environment, Error, ErrorKind, Value};
use std::collections::HashMap;
use std::io::Read;
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::{Duration, Instant};
use tracing::{debug, info};

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

            // Handle the optional created_at
            if let Some(created_at) = bm.created_at {
                context.insert(
                    "created_at".to_string(),
                    Value::from(created_at.to_rfc3339()),
                );
            } else {
                // If created_at is None, use null value
                context.insert("created_at".to_string(), Value::from(()));
            }

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

        debug!("Rendering template");
        let template = self.env.template_from_str(url).map_err(|e| {
            debug!(error = %e, "Template syntax error (content may not be a MiniJinja template)");
            InterpolationError::Syntax(e.to_string())
        })?;

        let context = self.create_context(bookmark);

        template.render(context).map_err(|e| {
            debug!(error = %e, "Template rendering failed");
            InterpolationError::Rendering(e.to_string())
        })
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

/// How long one `shell` filter command may run. Templates render while the fuzzy finder builds
/// its list and on every LSP completion, so one slow command must not hang them.
const SHELL_TIMEOUT: Duration = Duration::from_secs(5);

/// Runs the `shell` filter's command with `sh -c`. Templates are trusted code, like a shell
/// script you stored: there is no filter on what a command may do. A substring blocklist cannot
/// stop a determined command (a newline got past it) and rejected ordinary shell syntax.
#[derive(Clone, Debug)]
pub struct SystemShellExecutor {
    timeout: Duration,
}

impl Default for SystemShellExecutor {
    fn default() -> Self {
        Self::new()
    }
}

impl SystemShellExecutor {
    pub fn new() -> Self {
        Self::with_timeout(SHELL_TIMEOUT)
    }

    pub fn with_timeout(timeout: Duration) -> Self {
        Self { timeout }
    }
}

/// Reads a child's pipe to the end on its own thread, so a full pipe cannot block the child.
fn read_to_end_in_background<R: Read + Send + 'static>(pipe: Option<R>) -> JoinHandle<Vec<u8>> {
    std::thread::spawn(move || {
        let mut buffer = Vec::new();
        if let Some(mut pipe) = pipe {
            let _ = pipe.read_to_end(&mut buffer);
        }
        buffer
    })
}

impl ShellCommandExecutor for SystemShellExecutor {
    fn execute(&self, cmd: &str) -> Result<String, InterpolationError> {
        info!("Executing shell command: {}", cmd);

        let mut child = Command::new("sh")
            .arg("-c")
            .arg(cmd)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| InterpolationError::Shell(format!("Failed to execute command: {}", e)))?;
        let stdout = read_to_end_in_background(child.stdout.take());
        let stderr = read_to_end_in_background(child.stderr.take());

        // ponytail: polls every 10 ms and kills only `sh`. A grandchild that outlives it keeps
        // the pipes open, so the readers are left behind rather than joined; they end with the
        // process. Killing the process group needs `setsid` (libc or command-group).
        let deadline = Instant::now() + self.timeout;
        let status = loop {
            match child.try_wait() {
                Ok(Some(status)) => break status,
                Ok(None) if Instant::now() >= deadline => {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err(InterpolationError::Shell(format!(
                        "Command timed out after {} s: {}",
                        self.timeout.as_secs_f32(),
                        cmd
                    )));
                }
                Ok(None) => std::thread::sleep(Duration::from_millis(10)),
                Err(e) => {
                    return Err(InterpolationError::Shell(format!(
                        "Failed to wait for command: {}",
                        e
                    )))
                }
            }
        };

        let stdout = stdout.join().unwrap_or_default();
        let stderr = stderr.join().unwrap_or_default();
        if !status.success() {
            return Err(InterpolationError::Shell(format!(
                "Command failed: {}",
                String::from_utf8_lossy(&stderr)
            )));
        }

        Ok(String::from_utf8_lossy(&stdout).trim().to_string())
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};

    #[test]
    fn given_command_list_with_variables_when_execute_then_runs_all_of_it() {
        // Templates are trusted code: shell syntax such as `;`, `$` and `$(( ))` is allowed
        let executor = SystemShellExecutor::new();

        let output = executor.execute("echo one; echo \"$((1 + 1))\"").unwrap();

        assert_eq!(output, "one\n2");
    }

    #[test]
    fn given_documented_fallback_command_when_execute_then_returns_fallback() {
        // The example from the wiki's Template-Interpolation page
        let executor = SystemShellExecutor::new();

        let output = executor
            .execute("no-such-command-bkmr 2>/dev/null || echo 'unknown'")
            .unwrap();

        assert_eq!(output, "unknown");
    }

    #[test]
    fn given_failing_command_when_execute_then_returns_error() {
        let executor = SystemShellExecutor::new();

        let result = executor.execute("echo broken >&2; exit 3");

        let err = result
            .expect_err("a failing command is an error")
            .to_string();
        assert!(err.contains("broken"), "got: {err}");
    }

    #[test]
    fn given_command_slower_than_timeout_when_execute_then_stops_it_with_error() {
        // A slow command must not hang the fuzzy finder, the LSP or an action
        let executor = SystemShellExecutor::with_timeout(Duration::from_millis(200));
        let started = Instant::now();

        let result = executor.execute("sleep 5");

        let err = result.expect_err("a slow command is an error").to_string();
        assert!(err.contains("timed out"), "got: {err}");
        assert!(started.elapsed() < Duration::from_secs(2));
    }
}
