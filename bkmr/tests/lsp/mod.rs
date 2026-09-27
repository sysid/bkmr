//! Black-box tests of `bkmr lsp` over stdio.
//!
//! The in-process tests in `src/lsp/tests` call the backend directly and cannot
//! see JSON-RPC framing or stray output on stdout. These tests spawn the real
//! binary against a throwaway database, exactly as an editor does.

mod test_stdio;

use serde_json::{json, Value};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{self, Receiver};
use std::thread;
use std::time::Duration;
use tempfile::TempDir;

const BKMR: &str = env!("CARGO_BIN_EXE_bkmr");
const RESPONSE_TIMEOUT: Duration = Duration::from_secs(10);

/// Throwaway database seeded through the CLI, deleted on drop.
pub struct TestDb {
    _dir: TempDir,
    path: PathBuf,
}

impl TestDb {
    pub fn new() -> Self {
        let dir = TempDir::new().expect("create temp dir");
        let path = dir.path().join("lsp_test.db");
        let status = Command::new(BKMR)
            .env("BKMR_DB_URL", &path)
            .arg("create-db")
            .arg(&path)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .expect("run bkmr create-db");
        assert!(status.success(), "bkmr create-db failed");
        Self { _dir: dir, path }
    }

    /// Add a bookmark without embedding or web fetch: hermetic and offline.
    pub fn add(&self, content: &str, tags: &str, title: &str) -> &Self {
        let status = Command::new(BKMR)
            .env("BKMR_DB_URL", &self.path)
            .args([
                "add",
                content,
                tags,
                "--title",
                title,
                "--no-embed",
                "--no-web",
            ])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .expect("run bkmr add");
        assert!(status.success(), "bkmr add failed for '{}'", title);
        self
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}

/// A running `bkmr lsp` process with a minimal JSON-RPC client.
pub struct LspSession {
    child: Child,
    stdin: ChildStdin,
    frames: Receiver<Result<Value, String>>,
    next_id: i64,
}

impl LspSession {
    pub fn start(db: &TestDb, extra_args: &[&str]) -> Self {
        let mut child = Command::new(BKMR)
            .env("BKMR_DB_URL", db.path())
            .args(extra_args)
            .arg("lsp")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .expect("spawn bkmr lsp");
        let stdin = child.stdin.take().expect("stdin");
        let stdout = child.stdout.take().expect("stdout");

        let (tx, frames) = mpsc::channel();
        thread::spawn(move || {
            let mut reader = BufReader::new(stdout);
            loop {
                let frame = read_frame(&mut reader);
                let done = !matches!(frame, Ok(Some(_)));
                if tx
                    .send(frame.and_then(|f| f.ok_or_else(|| "EOF".into())))
                    .is_err()
                    || done
                {
                    break;
                }
            }
        });

        Self {
            child,
            stdin,
            frames,
            next_id: 1,
        }
    }

    /// Initialize handshake with a client that supports snippets.
    pub fn initialize(&mut self) -> Value {
        let result = self.request(
            "initialize",
            json!({
                "processId": null,
                "capabilities": {
                    "textDocument": {"completion": {"completionItem": {"snippetSupport": true}}}
                }
            }),
        );
        self.notify("initialized", json!({}));
        result
    }

    pub fn open(&mut self, uri: &str, language_id: &str, text: &str) {
        self.notify(
            "textDocument/didOpen",
            json!({"textDocument": {"uri": uri, "languageId": language_id, "version": 1, "text": text}}),
        );
    }

    /// Manually invoked completion at the start of the first line.
    pub fn completion_labels(&mut self, uri: &str) -> Vec<String> {
        let result = self.request(
            "textDocument/completion",
            json!({
                "textDocument": {"uri": uri},
                "position": {"line": 0, "character": 0},
                "context": {"triggerKind": 1}
            }),
        );
        let mut labels: Vec<String> = result["items"]
            .as_array()
            .expect("completion list items")
            .iter()
            .map(|i| i["label"].as_str().expect("label").to_string())
            .collect();
        labels.sort();
        labels
    }

    pub fn execute_command(&mut self, command: &str, arguments: Value) -> Value {
        self.request(
            "workspace/executeCommand",
            json!({"command": command, "arguments": arguments}),
        )
    }

    /// Send a request and wait for the response with the same id; notifications
    /// (e.g. window/logMessage) arriving in between are skipped.
    pub fn request(&mut self, method: &str, params: Value) -> Value {
        let id = self.next_id;
        self.next_id += 1;
        self.send(json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params}));
        loop {
            let frame = self
                .frames
                .recv_timeout(RESPONSE_TIMEOUT)
                .unwrap_or_else(|_| {
                    panic!("no response to '{}' within {:?}", method, RESPONSE_TIMEOUT)
                })
                .unwrap_or_else(|e| panic!("invalid stdout while waiting for '{}': {}", method, e));
            if frame.get("id") == Some(&json!(id)) {
                assert!(
                    frame.get("error").is_none(),
                    "'{}' returned JSON-RPC error: {}",
                    method,
                    frame["error"]
                );
                return frame["result"].clone();
            }
        }
    }

    pub fn notify(&mut self, method: &str, params: Value) {
        self.send(json!({"jsonrpc": "2.0", "method": method, "params": params}));
    }

    /// Orderly shutdown; returns every frame the server wrote after the last
    /// response, so callers can assert stdout stayed clean until exit.
    pub fn shutdown(mut self) -> Vec<Result<Value, String>> {
        self.request("shutdown", Value::Null);
        self.notify("exit", Value::Null);
        let _ = self.child.wait();
        self.frames
            .iter()
            .filter(|f| f.as_ref().err().map(String::as_str) != Some("EOF"))
            .collect()
    }

    fn send(&mut self, message: Value) {
        let body = message.to_string();
        write!(self.stdin, "Content-Length: {}\r\n\r\n{}", body.len(), body)
            .expect("write to server");
        self.stdin.flush().expect("flush to server");
    }
}

impl Drop for LspSession {
    fn drop(&mut self) {
        let _ = self.child.kill();
    }
}

/// Read one `Content-Length`-framed JSON-RPC message. Anything else on stdout
/// (e.g. a log line) is an error: it would corrupt every editor's LSP client.
fn read_frame(reader: &mut impl BufRead) -> Result<Option<Value>, String> {
    let mut content_length = None;
    loop {
        let mut line = String::new();
        if reader.read_line(&mut line).map_err(|e| e.to_string())? == 0 {
            return Ok(None);
        }
        let line = line.trim_end_matches(['\r', '\n']);
        if line.is_empty() {
            break;
        }
        match line.split_once(':') {
            Some((name, value)) if name.eq_ignore_ascii_case("content-length") => {
                content_length = Some(value.trim().parse::<usize>().map_err(|e| e.to_string())?);
            }
            Some((name, _)) if name.eq_ignore_ascii_case("content-type") => {}
            _ => return Err(format!("non-JSON-RPC output on stdout: {:?}", line)),
        }
    }
    let length = content_length.ok_or("frame without Content-Length")?;
    let mut body = vec![0; length];
    reader.read_exact(&mut body).map_err(|e| e.to_string())?;
    serde_json::from_slice(&body)
        .map(Some)
        .map_err(|e| format!("frame body is not JSON: {}", e))
}
