// Copyright (c) Pavel Sich.
// Licensed under the MIT License.

//! A minimal Language Server Protocol client, used for code completion.
//!
//! Each server runs as a child process. A reader thread parses the `Content-Length`
//! framed messages from its stdout and a writer thread feeds its stdin,
//! so the UI thread never blocks on a slow or hung server.

use std::collections::HashMap;
use std::fmt::Write as _;
use std::io::{BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::mpsc::{self, Receiver, Sender, TryRecvError};
use std::time::Duration;
use std::{env, fs, mem, process, ptr, thread};

use edit::arena::scratch_arena;
use edit::json::{self, Object, Value};

/// How often the main loop checks for a response while a request is in flight.
const POLL_INTERVAL_BUSY: Duration = Duration::from_millis(16);
/// How often the main loop refreshes the progress shown in the status bar.
const POLL_INTERVAL_PROGRESS: Duration = Duration::from_millis(250);
/// The main loop only wakes up for input or timeouts, so while a server is running
/// we still need to check periodically for messages it sends on its own, like progress.
const POLL_INTERVAL_IDLE: Duration = Duration::from_millis(500);

struct ServerSpec {
    /// Shown in the status bar.
    name: &'static str,
    /// Executables and their arguments. The first one found on `PATH` is used.
    commands: &'static [(&'static str, &'static [&'static str])],
    extensions: &'static [&'static str],
    /// Files that mark the project root. `*.ext` matches any file with that extension.
    root_markers: &'static [&'static str],
    /// Returns the server's `initializationOptions` if `path` isn't part of a project
    /// and needs a server of its own, which treats it as a standalone file.
    standalone_options: Option<fn(&Path) -> Option<String>>,
}

const SERVERS: &[ServerSpec] = &[
    ServerSpec {
        name: "rust-analyzer",
        commands: &[("rust-analyzer", &[])],
        extensions: &["rs"],
        root_markers: &["Cargo.toml"],
        standalone_options: Some(rust_standalone_options),
    },
    ServerSpec {
        name: "gopls",
        commands: &[("gopls", &[])],
        extensions: &["go"],
        root_markers: &["go.work", "go.mod"],
        standalone_options: None,
    },
    ServerSpec {
        name: "typescript-language-server",
        commands: &[("typescript-language-server", &["--stdio"])],
        extensions: &["ts", "tsx", "mts", "cts", "js", "jsx", "mjs", "cjs"],
        root_markers: &["tsconfig.json", "jsconfig.json", "package.json"],
        standalone_options: None,
    },
    ServerSpec {
        name: "pyright",
        commands: &[
            ("pyright-langserver", &["--stdio"]),
            ("basedpyright-langserver", &["--stdio"]),
            ("pylsp", &[]),
        ],
        extensions: &["py", "pyi"],
        root_markers: &["pyproject.toml", "setup.py", "setup.cfg", "requirements.txt"],
        standalone_options: None,
    },
    ServerSpec {
        name: "csharp-ls",
        commands: &[("csharp-ls", &[]), ("OmniSharp", &["-lsp"])],
        extensions: &["cs"],
        root_markers: &["*.sln", "*.csproj"],
        standalone_options: None,
    },
];

fn language_id(path: &Path) -> &'static str {
    match extension(path).as_str() {
        "rs" => "rust",
        "go" => "go",
        "ts" | "mts" | "cts" => "typescript",
        "tsx" => "typescriptreact",
        "js" | "mjs" | "cjs" => "javascript",
        "jsx" => "javascriptreact",
        "py" | "pyi" => "python",
        "cs" => "csharp",
        _ => "plaintext",
    }
}

fn extension(path: &Path) -> String {
    path.extension().map(|e| e.to_string_lossy().to_ascii_lowercase()).unwrap_or_default()
}

fn spec_for(path: &Path) -> Option<&'static ServerSpec> {
    let extension = extension(path);
    SERVERS.iter().find(|spec| spec.extensions.contains(&extension.as_str()))
}

/// rust-analyzer only analyzes files that belong to a crate. Anything else, like a
/// loose `test.rs`, is passed via `linkedProjects`, which makes it a standalone file.
fn rust_standalone_options(path: &Path) -> Option<String> {
    let dir = path.parent()?;
    let in_crate = dir
        .ancestors()
        .find(|ancestor| ancestor.join("Cargo.toml").is_file())
        .is_some_and(|crate_dir| {
            let relative = path.strip_prefix(crate_dir).unwrap_or(path);
            relative == Path::new("build.rs")
                || relative.components().next().is_some_and(|first| {
                    ["src", "tests", "benches", "examples"]
                        .iter()
                        .any(|name| first.as_os_str() == *name)
                })
        });
    let path = path.to_string_lossy();
    (!in_crate).then(|| format!(r#"{{"linkedProjects":[{}]}}"#, json_string(&path)))
}

/// A position in a document, as defined by LSP: a 0-based line and a UTF-16 code unit offset.
#[derive(Clone, Copy, Default)]
pub struct Position {
    pub line: u32,
    pub character: u32,
}

#[derive(Clone, Copy, Default)]
pub struct Range {
    pub start: Position,
    pub end: Position,
}

#[derive(Clone)]
pub struct TextEdit {
    pub range: Range,
    pub new_text: String,
}

#[derive(Clone)]
pub struct CompletionItem {
    /// The text shown in the list, e.g. `new(…)`.
    pub label: String,
    /// The type or signature, e.g. `fn(String, u32) -> Person`.
    pub detail: String,
    pub kind: u32,
    pub filter_text: String,
    pub sort_text: String,
    /// Inserted when there's no `text_edit`, replacing the word being completed.
    pub insert_text: String,
    pub text_edit: Option<TextEdit>,
    /// Edits elsewhere in the document, e.g. an automatically added `use` statement.
    pub additional_edits: Vec<TextEdit>,
}

#[derive(Default)]
pub struct CompletionList {
    /// If true, the server wants to be asked again as the user keeps typing.
    pub is_incomplete: bool,
    pub items: Vec<CompletionItem>,
}

pub enum CompletionTrigger {
    /// Requested explicitly with Ctrl+Space.
    Invoked,
    /// One of the server's trigger characters was typed, e.g. `.`.
    Character(char),
    /// The previous result was incomplete and the user kept typing.
    Incomplete,
}

#[derive(Default)]
pub struct LspPoll {
    pub completion: Option<CompletionList>,
    /// Something that affects the UI changed, e.g. the status bar text.
    pub changed: bool,
}

/// Owns all running language servers.
#[derive(Default)]
pub struct LspManager {
    servers: Vec<Server>,
    /// Servers that couldn't be started and the reason why. They aren't retried.
    unavailable: Vec<(&'static ServerSpec, &'static str)>,
}

impl LspManager {
    /// Opens the document on its language server, starting the server if needed,
    /// and sends the document's text whenever `generation` changes.
    pub fn sync_document(&mut self, path: &Path, generation: u32, text: impl FnOnce() -> String) {
        if let Some(index) = self.server_index(path, true) {
            self.servers[index].sync_document(path, generation, text);
        }
    }

    /// Tells the servers about documents that were closed in the editor.
    pub fn close_documents_except(&mut self, open: &[&Path]) {
        for server in &mut self.servers {
            server.close_documents_except(open);
        }
    }

    /// Returns false if there's no server for this document.
    pub fn request_completion(
        &mut self,
        path: &Path,
        position: Position,
        trigger: CompletionTrigger,
    ) -> bool {
        match self.server_index(path, false) {
            Some(index) => self.servers[index].request_completion(path, position, trigger),
            None => false,
        }
    }

    pub fn trigger_characters(&self, path: &Path) -> &[char] {
        self.find_server(path).map_or(&[], |server| &server.trigger_characters)
    }

    /// A short status for the status bar, e.g. `rust-analyzer: Indexing 40%`.
    /// Empty if the server is ready and idle, or if there's no server for this document.
    pub fn status(&self, path: &Path) -> String {
        if let Some(server) = self.find_server(path) {
            return server.status();
        }
        let Some(spec) = spec_for(path) else {
            return String::new();
        };
        match self.unavailable.iter().find(|(s, _)| ptr::eq(*s, spec)) {
            Some((_, reason)) => format!("{}: {}", spec.name, reason),
            None => String::new(),
        }
    }

    /// Processes all messages the servers sent since the last call.
    pub fn poll(&mut self) -> LspPoll {
        let mut result = LspPoll::default();
        for server in &mut self.servers {
            server.poll(&mut result);
        }
        result
    }

    /// How long the main loop may wait for input before it needs to call [`LspManager::poll`].
    pub fn read_timeout(&self) -> Duration {
        let running = || self.servers.iter().filter(|s| !matches!(s.state, ServerState::Exited));
        if running().any(|s| s.pending_completion.is_some() || s.state != ServerState::Ready) {
            POLL_INTERVAL_BUSY
        } else if running().any(|s| !s.progress.is_empty()) {
            POLL_INTERVAL_PROGRESS
        } else if running().next().is_some() {
            POLL_INTERVAL_IDLE
        } else {
            Duration::MAX
        }
    }

    fn find_server(&self, path: &Path) -> Option<&Server> {
        self.servers.iter().find(|server| server.documents.contains_key(path))
    }

    fn server_index(&mut self, path: &Path, start: bool) -> Option<usize> {
        if let Some(index) = self.servers.iter().position(|s| s.documents.contains_key(path)) {
            return Some(index);
        }

        let spec = spec_for(path)?;
        let dir = path.parent()?;
        let standalone_options = spec.standalone_options.and_then(|options| options(path));
        let shares_server = |s: &Server| {
            ptr::eq(s.spec, spec)
                && match &s.standalone_file {
                    Some(file) => file == path,
                    None => standalone_options.is_none() && dir.starts_with(&s.root),
                }
        };
        if let Some(index) = self.servers.iter().position(shares_server) {
            return Some(index);
        }

        if !start || self.unavailable.iter().any(|(s, _)| ptr::eq(*s, spec)) {
            return None;
        }

        let launch = match standalone_options {
            Some(options) => Launch {
                root: dir.to_path_buf(),
                standalone_file: Some(path.to_path_buf()),
                initialization_options: options,
            },
            None => Launch {
                root: find_root(dir, spec.root_markers),
                standalone_file: None,
                initialization_options: "null".to_string(),
            },
        };
        match Server::start(spec, launch) {
            Ok(server) => {
                self.servers.push(server);
                Some(self.servers.len() - 1)
            }
            Err(reason) => {
                self.unavailable.push((spec, reason));
                None
            }
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ServerState {
    /// Waiting for the response to the `initialize` request with this ID.
    Initializing(i64),
    Ready,
    Exited,
}

struct OpenDocument {
    uri: String,
    version: i32,
    generation: u32,
}

struct Progress {
    token: String,
    title: String,
    percentage: Option<u32>,
}

struct Launch {
    root: PathBuf,
    standalone_file: Option<PathBuf>,
    initialization_options: String,
}

struct Server {
    spec: &'static ServerSpec,
    root: PathBuf,
    /// Set if this server was started for a single file outside of any project.
    standalone_file: Option<PathBuf>,
    process: Child,
    outgoing: Sender<Vec<u8>>,
    incoming: Receiver<String>,
    state: ServerState,
    next_id: i64,
    /// Messages held back until the server has been initialized.
    queued: Vec<String>,
    trigger_characters: Vec<char>,
    documents: HashMap<PathBuf, OpenDocument>,
    progress: Vec<Progress>,
    pending_completion: Option<i64>,
}

impl Server {
    fn start(spec: &'static ServerSpec, launch: Launch) -> Result<Self, &'static str> {
        let (program, args) = spec
            .commands
            .iter()
            .find_map(|(name, args)| Some((find_executable(name)?, *args)))
            .ok_or("not installed")?;

        // stderr must not be inherited, or the server's logging would corrupt the terminal.
        let mut process = Command::new(program)
            .args(args)
            .current_dir(&launch.root)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|_| "failed to start")?;
        let stdin = process.stdin.take().ok_or("failed to start")?;
        let stdout = process.stdout.take().ok_or("failed to start")?;

        let (outgoing, outgoing_rx) = mpsc::channel();
        let (incoming_tx, incoming) = mpsc::channel();
        thread::spawn(move || write_messages(stdin, outgoing_rx));
        thread::spawn(move || read_messages(stdout, incoming_tx));

        let mut server = Self {
            spec,
            root: launch.root,
            standalone_file: launch.standalone_file,
            process,
            outgoing,
            incoming,
            state: ServerState::Initializing(0),
            next_id: 1,
            queued: Vec::new(),
            trigger_characters: Vec::new(),
            documents: HashMap::new(),
            progress: Vec::new(),
            pending_completion: None,
        };

        let root_uri = path_to_uri(&server.root);
        let root_name = server.root.file_name().unwrap_or_default().to_string_lossy();
        let mut params = String::new();
        _ = write!(
            params,
            concat!(
                r#"{{"processId":{},"clientInfo":{{"name":"edit","version":"{}"}},"#,
                r#""rootUri":{},"workspaceFolders":[{{"uri":{},"name":{}}}],"#,
                r#""initializationOptions":{},"#,
                r#""capabilities":{{"#,
                r#""general":{{"positionEncodings":["utf-16"]}},"#,
                r#""window":{{"workDoneProgress":true}},"#,
                r#""workspace":{{"workspaceFolders":true}},"#,
                r#""textDocument":{{"#,
                r#""synchronization":{{"dynamicRegistration":false}},"#,
                r#""completion":{{"contextSupport":true,"completionItem":{{"#,
                r#""snippetSupport":false,"insertReplaceSupport":true,"#,
                r#""labelDetailsSupport":true,"documentationFormat":["plaintext"]}}}}"#,
                r#"}}}}}}"#,
            ),
            process::id(),
            env!("CARGO_PKG_VERSION"),
            json_string(&root_uri),
            json_string(&root_uri),
            json_string(&root_name),
            launch.initialization_options,
        );

        let id = server.next_request_id();
        server.write(&request_body(id, "initialize", &params));
        server.state = ServerState::Initializing(id);
        Ok(server)
    }

    fn status(&self) -> String {
        let name = self.spec.name;
        match self.state {
            ServerState::Initializing(_) => format!("{name}: starting"),
            ServerState::Exited => format!("{name}: stopped"),
            ServerState::Ready => match self.progress.last() {
                Some(Progress { title, percentage: Some(pct), .. }) => {
                    format!("{name}: {title} {pct}%")
                }
                Some(Progress { title, .. }) => format!("{name}: {title}"),
                None => String::new(),
            },
        }
    }

    fn sync_document(&mut self, path: &Path, generation: u32, text: impl FnOnce() -> String) {
        let params = match self.documents.get_mut(path) {
            Some(doc) if doc.generation == generation => return,
            Some(doc) => {
                doc.version += 1;
                doc.generation = generation;
                format!(
                    r#"{{"textDocument":{{"uri":{},"version":{}}},"contentChanges":[{{"text":{}}}]}}"#,
                    json_string(&doc.uri),
                    doc.version,
                    json_string(&text()),
                )
            }
            None => {
                let uri = path_to_uri(path);
                let params = format!(
                    r#"{{"textDocument":{{"uri":{},"languageId":"{}","version":1,"text":{}}}}}"#,
                    json_string(&uri),
                    language_id(path),
                    json_string(&text()),
                );
                self.documents
                    .insert(path.to_path_buf(), OpenDocument { uri, version: 1, generation });
                return self.notify("textDocument/didOpen", &params);
            }
        };
        self.notify("textDocument/didChange", &params);
    }

    fn close_documents_except(&mut self, open: &[&Path]) {
        let closed: Vec<PathBuf> =
            self.documents.keys().filter(|path| !open.contains(&path.as_path())).cloned().collect();
        for path in closed {
            if let Some(doc) = self.documents.remove(&path) {
                let params = format!(r#"{{"textDocument":{{"uri":{}}}}}"#, json_string(&doc.uri));
                self.notify("textDocument/didClose", &params);
            }
        }
    }

    fn request_completion(
        &mut self,
        path: &Path,
        position: Position,
        trigger: CompletionTrigger,
    ) -> bool {
        if self.state == ServerState::Exited {
            return false;
        }
        let Some(doc) = self.documents.get(path) else {
            return false;
        };

        let context = match trigger {
            CompletionTrigger::Invoked => r#"{"triggerKind":1}"#.to_string(),
            CompletionTrigger::Character(ch) => format!(
                r#"{{"triggerKind":2,"triggerCharacter":{}}}"#,
                json_string(ch.encode_utf8(&mut [0; 4]))
            ),
            CompletionTrigger::Incomplete => r#"{"triggerKind":3}"#.to_string(),
        };
        let params = format!(
            r#"{{"textDocument":{{"uri":{}}},"position":{{"line":{},"character":{}}},"context":{}}}"#,
            json_string(&doc.uri),
            position.line,
            position.character,
            context,
        );

        let id = self.next_request_id();
        self.send(request_body(id, "textDocument/completion", &params));
        self.pending_completion = Some(id);
        true
    }

    fn poll(&mut self, result: &mut LspPoll) {
        loop {
            match self.incoming.try_recv() {
                Ok(body) => self.handle_message(&body, result),
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    if self.state != ServerState::Exited {
                        self.state = ServerState::Exited;
                        self.pending_completion = None;
                        self.progress.clear();
                        result.changed = true;
                    }
                    break;
                }
            }
        }
    }

    fn handle_message(&mut self, body: &str, result: &mut LspPoll) {
        let scratch = scratch_arena(None);
        let Ok(message) = json::parse(&scratch, body) else {
            return;
        };
        let Some(message) = message.as_object() else {
            return;
        };

        match (message.get_str("method"), message.get("id")) {
            (Some(method), Some(id)) => self.respond_to_request(method, id, message),
            (Some("$/progress"), None) => {
                if let Some(params) = message.get_object("params") {
                    self.handle_progress(params);
                    result.changed = true;
                }
            }
            (None, Some(id)) => {
                let Some(id) = id.as_number().map(|id| id as i64) else {
                    return;
                };
                let response = message.get("result");

                if self.state == ServerState::Initializing(id) {
                    self.finish_initialize(response);
                    result.changed = true;
                } else if self.pending_completion == Some(id) {
                    self.pending_completion = None;
                    result.completion =
                        Some(response.map(parse_completion_list).unwrap_or_default());
                    result.changed = true;
                }
            }
            _ => {}
        }
    }

    fn finish_initialize(&mut self, response: Option<&Value>) {
        let trigger_characters = response
            .and_then(Value::as_object)
            .and_then(|result| result.get_object("capabilities"))
            .and_then(|capabilities| capabilities.get_object("completionProvider"))
            .and_then(|provider| provider.get_array("triggerCharacters"))
            .unwrap_or(&[]);
        self.trigger_characters = trigger_characters
            .iter()
            .filter_map(Value::as_str)
            .filter_map(|s| s.chars().next())
            .collect();

        self.state = ServerState::Ready;
        self.write(&notification_body("initialized", "{}"));
        for body in mem::take(&mut self.queued) {
            self.write(&body);
        }
    }

    /// Servers expect an answer to every request. We don't implement any of them,
    /// so we answer with empty results, which servers treat as "use your defaults".
    fn respond_to_request(&mut self, method: &str, id: &Value, message: Object) {
        let result = if method == "workspace/configuration" {
            let count = message
                .get_object("params")
                .and_then(|params| params.get_array("items"))
                .map_or(0, |items| items.len());
            format!("[{}]", vec!["null"; count].join(","))
        } else {
            "null".to_string()
        };
        let body = format!(r#"{{"jsonrpc":"2.0","id":{},"result":{}}}"#, json_id(id), result);
        self.write(&body);
    }

    fn handle_progress(&mut self, params: Object) {
        let token = params.get("token").map(json_id).unwrap_or_default();
        let Some(value) = params.get_object("value") else {
            return;
        };
        let percentage = value.get_number("percentage").map(|pct| pct as u32);

        match value.get_str("kind") {
            Some("begin") => {
                self.progress.retain(|p| p.token != token);
                let title = value.get_str("title").unwrap_or_default().to_string();
                self.progress.push(Progress { token, title, percentage });
            }
            Some("report") => {
                if let Some(progress) = self.progress.iter_mut().find(|p| p.token == token) {
                    progress.percentage = percentage.or(progress.percentage);
                }
            }
            Some("end") => self.progress.retain(|p| p.token != token),
            _ => {}
        }
    }

    fn next_request_id(&mut self) -> i64 {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    fn notify(&mut self, method: &str, params: &str) {
        self.send(notification_body(method, params));
    }

    fn send(&mut self, body: String) {
        match self.state {
            ServerState::Initializing(_) => self.queued.push(body),
            ServerState::Ready => self.write(&body),
            ServerState::Exited => {}
        }
    }

    fn write(&self, body: &str) {
        let mut message = format!("Content-Length: {}\r\n\r\n", body.len()).into_bytes();
        message.extend_from_slice(body.as_bytes());
        _ = self.outgoing.send(message);
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        _ = self.process.kill();
        _ = self.process.wait();
    }
}

fn read_messages(stdout: ChildStdout, incoming: Sender<String>) {
    let mut reader = BufReader::new(stdout);
    let mut line = String::new();

    loop {
        let mut content_length = None;
        loop {
            line.clear();
            match reader.read_line(&mut line) {
                Ok(0) | Err(_) => return,
                Ok(_) => {}
            }
            let header = line.trim_end();
            if header.is_empty() {
                break;
            }
            if let Some((name, value)) = header.split_once(':')
                && name.eq_ignore_ascii_case("content-length")
            {
                content_length = value.trim().parse::<usize>().ok();
            }
        }

        let Some(length) = content_length else {
            continue;
        };
        let mut body = vec![0; length];
        if reader.read_exact(&mut body).is_err() {
            return;
        }
        if incoming.send(String::from_utf8_lossy(&body).into_owned()).is_err() {
            return;
        }
    }
}

fn write_messages(mut stdin: ChildStdin, outgoing: Receiver<Vec<u8>>) {
    for message in outgoing {
        if stdin.write_all(&message).and_then(|_| stdin.flush()).is_err() {
            return;
        }
    }
}

fn request_body(id: i64, method: &str, params: &str) -> String {
    format!(r#"{{"jsonrpc":"2.0","id":{id},"method":"{method}","params":{params}}}"#)
}

fn notification_body(method: &str, params: &str) -> String {
    format!(r#"{{"jsonrpc":"2.0","method":"{method}","params":{params}}}"#)
}

fn parse_completion_list(result: &Value) -> CompletionList {
    let (is_incomplete, items) = match result {
        Value::Array(items) => (false, *items),
        Value::Object(_) => {
            let list = result.as_object().unwrap();
            (list.get_bool("isIncomplete").unwrap_or(false), list.get_array("items").unwrap_or(&[]))
        }
        _ => (false, &[][..]),
    };
    CompletionList {
        is_incomplete,
        items: items.iter().filter_map(parse_completion_item).collect(),
    }
}

fn parse_completion_item(value: &Value) -> Option<CompletionItem> {
    let item = value.as_object()?;
    let name = item.get_str("label")?;
    let label_details = item.get_object("labelDetails");
    let is_snippet = item.get_number("insertTextFormat") == Some(2.0);

    let mut label = name.to_string();
    if let Some(suffix) = label_details.and_then(|d| d.get_str("detail")) {
        label.push_str(suffix);
    }
    let detail = item
        .get_str("detail")
        .or_else(|| label_details.and_then(|d| d.get_str("description")))
        .unwrap_or_default();

    let text_edit = item.get_object("textEdit").and_then(|edit| {
        let range = edit.get_object("insert").or_else(|| edit.get_object("range"))?;
        Some(TextEdit {
            range: parse_range(range)?,
            new_text: plain_text(edit.get_str("newText")?, is_snippet),
        })
    });
    let additional_edits = item
        .get_array("additionalTextEdits")
        .unwrap_or(&[])
        .iter()
        .filter_map(|edit| {
            let edit = edit.as_object()?;
            Some(TextEdit {
                range: parse_range(edit.get_object("range")?)?,
                new_text: edit.get_str("newText")?.to_string(),
            })
        })
        .collect();

    Some(CompletionItem {
        label,
        detail: detail.lines().next().unwrap_or_default().to_string(),
        kind: item.get_number("kind").unwrap_or(0.0) as u32,
        filter_text: item.get_str("filterText").unwrap_or(name).to_string(),
        sort_text: item.get_str("sortText").unwrap_or(name).to_string(),
        insert_text: plain_text(item.get_str("insertText").unwrap_or(name), is_snippet),
        text_edit,
        additional_edits,
    })
}

fn parse_range(range: Object) -> Option<Range> {
    let position = |key: &str| {
        let position = range.get_object(key)?;
        Some(Position {
            line: position.get_number("line")? as u32,
            character: position.get_number("character")? as u32,
        })
    };
    Some(Range { start: position("start")?, end: position("end")? })
}

/// We don't advertise snippet support, but some servers send snippets anyway.
/// This turns `${1:name}` into `name` and drops tab stops like `$0`.
fn plain_text(text: &str, is_snippet: bool) -> String {
    if !is_snippet {
        return text.to_string();
    }

    let mut result = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(ch) = chars.next() {
        match ch {
            '\\' => result.extend(chars.next()),
            '$' if chars.peek().is_some_and(char::is_ascii_digit) => {
                while chars.next_if(char::is_ascii_digit).is_some() {}
            }
            '$' if chars.peek() == Some(&'{') => {
                chars.next();
                while chars.next_if(char::is_ascii_digit).is_some() {}
                if chars.next_if_eq(&':').is_none() {
                    // `${1}` or `${1|a,b|}` has no default text to keep.
                    for ch in chars.by_ref() {
                        if ch == '}' {
                            break;
                        }
                    }
                }
            }
            '}' => {}
            _ => result.push(ch),
        }
    }
    result
}

fn json_id(id: &Value) -> String {
    match id {
        Value::Number(n) => format!("{}", *n as i64),
        Value::String(s) => json_string(s),
        _ => "null".to_string(),
    }
}

fn json_string(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for ch in s.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            ch if ch < ' ' => _ = write!(out, "\\u{:04x}", ch as u32),
            ch => out.push(ch),
        }
    }
    out.push('"');
    out
}

fn path_to_uri(path: &Path) -> String {
    let path = path.to_string_lossy();
    let mut uri = String::from(if cfg!(windows) { "file:///" } else { "file://" });
    for byte in path.bytes() {
        match byte {
            b'\\' if cfg!(windows) => uri.push('/'),
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' | b'/' | b':' => {
                uri.push(byte as char)
            }
            _ => _ = write!(uri, "%{byte:02X}"),
        }
    }
    uri
}

fn find_executable(name: &str) -> Option<PathBuf> {
    let suffixes: &[&str] = if cfg!(windows) { &[".exe", ".cmd", ".bat"] } else { &[""] };
    env::split_paths(&env::var_os("PATH")?).find_map(|dir| {
        suffixes.iter().map(|suffix| dir.join(format!("{name}{suffix}"))).find(|p| p.is_file())
    })
}

fn find_root(dir: &Path, markers: &[&str]) -> PathBuf {
    let has_marker = |dir: &Path, marker: &str| match marker.strip_prefix("*.") {
        Some(ext) => fs::read_dir(dir)
            .into_iter()
            .flatten()
            .flatten()
            .any(|entry| entry.path().extension().is_some_and(|e| e == ext)),
        None => dir.join(marker).exists(),
    };
    dir.ancestors()
        .find(|ancestor| markers.iter().any(|marker| has_marker(ancestor, marker)))
        .unwrap_or(dir)
        .to_path_buf()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_plain_text() {
        assert_eq!(plain_text("foo", true), "foo");
        assert_eq!(plain_text("new(${1:name}, ${2:age})$0", true), "new(name, age)");
        assert_eq!(plain_text("println!(\"$1\")", true), "println!(\"\")");
        assert_eq!(plain_text("${1|a,b|}x", true), "x");
        assert_eq!(plain_text("\\$1", true), "$1");
        assert_eq!(plain_text("${1:name}", false), "${1:name}");
    }

    #[test]
    fn test_json_string() {
        assert_eq!(json_string("a\"b\\c\nd\u{1}"), r#""a\"b\\c\nd\u0001""#);
    }

    #[test]
    fn test_parse_completion_list() {
        let scratch = scratch_arena(None);
        let input = r#"{"isIncomplete":true,"items":[
            {"label":"name","kind":5,"detail":"String"},
            {"label":"new","labelDetails":{"detail":"(…)"},"kind":3,"insertTextFormat":2,
             "textEdit":{"insert":{"start":{"line":1,"character":2},"end":{"line":1,"character":4}},
                         "replace":{"start":{"line":1,"character":2},"end":{"line":1,"character":6}},
                         "newText":"new(${1:name})"}}
        ]}"#;
        let list = parse_completion_list(&json::parse(&scratch, input).unwrap());
        assert!(list.is_incomplete);
        assert_eq!(list.items.len(), 2);
        assert_eq!(list.items[0].label, "name");
        assert_eq!(list.items[0].detail, "String");
        assert_eq!(list.items[1].label, "new(…)");
        let edit = list.items[1].text_edit.as_ref().unwrap();
        assert_eq!(edit.new_text, "new(name)");
        assert_eq!(edit.range.end.character, 4);
    }
}
