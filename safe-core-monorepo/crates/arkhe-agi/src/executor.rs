//! Tool advertisement and dispatch — the `ToolCall` half of the turn.
//!
//! An [`InferenceEngine`](arkhe_inference::InferenceEngine) can answer in two
//! ways: with content, or with a request to call a tool. `AgiCoordinator`
//! handles the second by advertising every registered [`Tool`] to the engine
//! on each request (`InferenceRequest::tools`) and dispatching whatever
//! comes back:
//!
//! - a tool that ran has its output appended to the response as
//!   `[tool:<name>] <output>`;
//! - a tool that failed has its error appended as `[tool:<name> failed]
//!   <reason>` and the turn *continues* — a broken tool must not turn into a
//!   failed turn, because the model's own answer is still a valid answer.
//!
//! That second path is why [`ToolRegistry::execute`] returns a `Result`
//! rather than panicking on an unknown name: `does_not_exist` is something a
//! model does, not something a program should crash on.
//!
//! # Why the registry has interior mutability
//!
//! `AgiCoordinator` holds a [`ToolRegistry`] as an ordinary field and shares
//! itself across tasks, so `AgiCoordinator::register_tool` takes `&self` and
//! `ToolRegistry::register` has to as well. The registry wraps its tool list
//! in an `RwLock` and never holds that lock across a tool's own `await` — a
//! slow HTTP fetch inside one call must not block registering another tool.
//!
//! # Signature, and what it does not do
//!
//! A [`Tool`] describes itself ([`Tool::definition`], fresh on every call)
//! and executes against the arguments of one call ([`Tool::execute`]). It is
//! *not* given the [`ToolCall`]'s `id`: that belongs to the engine that
//! minted it, and a tool has nothing to say about which turn the call came
//! from. Validating arguments is each tool's own job — there is no schema
//! validator here, so a tool that advertises a `url` parameter must also be
//! the one that rejects a call without one, as
//! [`FetchUrlTool`] does with [`ToolError::InvalidArguments`].

use arkhe_inference::{ToolCall, ToolDefinition};
use serde_json::Value;
use std::sync::Arc;
use std::time::Duration;
use thiserror::Error;

/// How long one [`FetchUrlTool`] fetch may take before it is abandoned.
///
/// A turn waits for its tool calls, so a server that accepts a connection and
/// then goes quiet must not be able to hold the turn open indefinitely. 30
/// seconds is longer than any fetch worth blocking a conversation on.
pub const DEFAULT_FETCH_TIMEOUT: Duration = Duration::from_secs(30);

/// One capability the coordinator can advertise and dispatch to.
///
/// Object-safe, `Send + Sync + 'static`, and async by way of `async-trait`:
/// a registry stores `Arc<dyn Tool>` and executes tools from whichever task
/// is processing the turn.
///
/// ```no_run
/// use arkhe_agi::executor::{Tool, ToolError, ToolRegistry};
/// use arkhe_inference::{ToolCall, ToolDefinition};
/// use serde_json::Value;
///
/// struct Echo;
///
/// #[async_trait::async_trait]
/// impl Tool for Echo {
///     fn definition(&self) -> ToolDefinition {
///         ToolDefinition {
///             name: "echo".to_string(),
///             description: "Repeats its `text` argument".to_string(),
///             parameters: serde_json::json!({
///                 "type": "object",
///                 "properties": { "text": { "type": "string" } },
///                 "required": ["text"],
///             }),
///         }
///     }
///
///     async fn execute(&self, arguments: &Value) -> Result<String, ToolError> {
///         let text = arguments
///             .get("text")
///             .and_then(Value::as_str)
///             .ok_or_else(|| ToolError::InvalidArguments {
///                 tool: "echo".to_string(),
///                 reason: "expected a `text` string argument".to_string(),
///             })?;
///         Ok(text.to_uppercase())
///     }
/// }
///
/// # #[tokio::main(flavor = "current_thread")]
/// # async fn main() {
/// let registry = ToolRegistry::new();
/// registry.register(Echo).await;
///
/// let call = ToolCall {
///     id: "call-1".to_string(),
///     name: "echo".to_string(),
///     arguments: serde_json::json!({ "text": "hi" }),
/// };
/// assert_eq!(registry.execute(&call).await.unwrap(), "HI");
/// # }
/// ```
#[async_trait::async_trait]
pub trait Tool: Send + Sync + 'static {
    /// How this tool is advertised to the inference engine.
    ///
    /// Called on every request rather than cached at registration, so a tool
    /// is free to describe itself differently per call (a parameter list
    /// that depends on configuration, say). `name` is what a `ToolCall` is
    /// matched against, and it is what the coordinator prints in
    /// `[tool:<name>]`.
    fn definition(&self) -> ToolDefinition;

    /// Runs this tool against the arguments one `ToolCall` carried.
    ///
    /// `Ok` is the tool's output as text — it is appended to the turn's
    /// response verbatim, so it should read as something worth showing a
    /// person, not as a debug dump. `Err` is everything else: unusable
    /// arguments ([`ToolError::InvalidArguments`]) or the tool running and
    /// failing ([`ToolError::Failed`]). Neither is fatal to the turn.
    async fn execute(&self, arguments: &Value) -> Result<String, ToolError>;
}

/// Everything that can go wrong while dispatching a tool call.
///
/// The distinction between variants is for the operator reading the log: a
/// missing tool and a broken tool are different problems with different
/// fixes. The coordinator treats them identically — the reason is appended
/// to the response and the turn goes on.
#[derive(Debug, Error)]
pub enum ToolError {
    /// No registered tool has this name. A model naming a tool that was
    /// never advertised, or misspelling one that was.
    #[error("no tool named `{name}` is registered")]
    NotFound {
        /// The name the call asked for, as it arrived.
        name: String,
    },

    /// The tool exists but could not read the arguments it was called with.
    #[error("tool `{tool}` could not read its arguments: {reason}")]
    InvalidArguments {
        /// The tool's name.
        tool: String,
        /// What was wrong with the arguments, in the tool's own words. It
        /// may quote the arguments back — they came from a model, not from
        /// a trusted source, and this string is only ever logged or shown,
        /// never parsed.
        reason: String,
    },

    /// The tool ran and failed: a transport error, a non-2xx status, a
    /// timeout, a backend that is down.
    #[error("tool `{tool}` failed: {reason}")]
    Failed {
        /// The tool's name.
        tool: String,
        /// What went wrong, including whatever the underlying library said.
        reason: String,
    },
}

/// The tools one coordinator can advertise and dispatch to.
///
/// Empty by default (`ToolRegistry::new`), which is the right default for a
/// coordinator whose engine cannot emit tool calls: advertising nothing
/// costs nothing.
pub struct ToolRegistry {
    /// In registration order, so `definitions` is stable across calls and a
    /// prompt sees the same tool list turn after turn.
    tools: tokio::sync::RwLock<Vec<Arc<dyn Tool>>>,
}

impl ToolRegistry {
    /// A registry with no tools.
    pub fn new() -> Self {
        Self {
            tools: tokio::sync::RwLock::new(Vec::new()),
        }
    }

    /// Registers `tool` under the name in its [`Tool::definition`],
    /// replacing in place any tool already registered under that name.
    ///
    /// Replacing rather than appending keeps the name-to-tool map a map: two
    /// tools with the same name would make `execute` dispatch to whichever
    /// happened to be found first. Everything else keeps the order it was
    /// registered in, and a re-registration does not move a tool in the
    /// advertised list.
    pub async fn register(&self, tool: impl Tool + 'static) {
        let name = tool.definition().name;
        let mut tools = self.tools.write().await;

        match tools.iter_mut().find(|t| t.definition().name == name) {
            Some(slot) => *slot = Arc::new(tool),
            None => tools.push(Arc::new(tool)),
        }
    }

    /// Definitions of every registered tool, in registration order.
    ///
    /// This is what goes into `InferenceRequest::tools` — the model can only
    /// call what it has been told about.
    pub async fn definitions(&self) -> Vec<ToolDefinition> {
        self.tools.read().await.iter().map(|t| t.definition()).collect()
    }

    /// How many tools are registered.
    pub async fn len(&self) -> usize {
        self.tools.read().await.len()
    }

    /// Whether no tools are registered.
    pub async fn is_empty(&self) -> bool {
        self.tools.read().await.is_empty()
    }

    /// Dispatches `call` to the registered tool with that name.
    ///
    /// [`ToolError::NotFound`] for a name nothing was registered under;
    /// otherwise the tool's own result. The registry's lock is released
    /// before the tool runs, so the lookup above is not held across the
    /// tool's I/O.
    pub async fn execute(&self, call: &ToolCall) -> Result<String, ToolError> {
        let tool = {
            let tools = self.tools.read().await;
            tools
                .iter()
                .find(|t| t.definition().name == call.name)
                .cloned()
        };

        match tool {
            Some(tool) => tool.execute(&call.arguments).await,
            None => Err(ToolError::NotFound {
                name: call.name.clone(),
            }),
        }
    }
}

impl Default for ToolRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// `GET {base_url}{url}` — the one tool that ships with the coordinator.
///
/// Proof that the dispatch path works end to end against a real network
/// stack (`reqwest`), and a template for the tools that do not ship yet. It
/// is pointed at a base URL — a mock server in tests, an internal API in a
/// deployment — and fetches a path relative to it.
///
/// The base URL is the tool's whole authority: the argument is concatenated
/// onto it, never parsed as a URL of its own, so a call can only ever reach
/// the origin the tool was constructed with. An argument that looks absolute
/// (it contains `://`) is refused with [`ToolError::InvalidArguments`]
/// instead of being concatenated into a URL that would not parse.
///
/// ```
/// # #[tokio::main(flavor = "current_thread")]
/// # async fn main() {
/// use arkhe_agi::executor::{FetchUrlTool, Tool, ToolRegistry};
/// use arkhe_inference::ToolCall;
///
/// let mut server = mockito::Server::new_async().await;
/// let _mock = server
///     .mock("GET", "/weather")
///     .with_status(200)
///     .with_body("sunny, 22C")
///     .create_async()
///     .await;
///
/// let registry = ToolRegistry::new();
/// registry.register(FetchUrlTool::with_base_url(server.url())).await;
///
/// let call = ToolCall {
///     id: "call-1".to_string(),
///     name: FetchUrlTool::NAME.to_string(),
///     arguments: serde_json::json!({ "url": "/weather" }),
/// };
/// assert_eq!(registry.execute(&call).await.unwrap(), "sunny, 22C");
///
/// // An unknown name is an error, not a panic — `Failed`, never a crash.
/// let unknown = ToolCall {
///     id: "call-2".to_string(),
///     name: "nope".to_string(),
///     arguments: serde_json::json!({}),
/// };
/// assert!(registry.execute(&unknown).await.is_err());
/// # }
/// ```
#[derive(Clone, Debug)]
pub struct FetchUrlTool {
    /// Without a trailing slash, so concatenation cannot double it up.
    base_url: String,
    /// Cloned per tool, pooled internally by `reqwest`.
    http: reqwest::Client,
}

impl FetchUrlTool {
    /// The name this tool advertises and is dispatched by.
    pub const NAME: &'static str = "fetch_url";

    /// A tool that fetches paths relative to `url`, with any trailing slash
    /// trimmed and a [`DEFAULT_FETCH_TIMEOUT`] per request.
    pub fn with_base_url(url: impl Into<String>) -> Self {
        Self::with_timeout(url, DEFAULT_FETCH_TIMEOUT)
    }

    /// The same, with an explicit per-request timeout.
    ///
    /// Exists so a test need not wait out the default, and so a caller
    /// fetching something knowingly slow can say so. If the underlying
    /// client rejects the configuration, the timeout is dropped and a
    /// default client is used: this is a constructor that cannot return an
    /// error, and a misconfigured TLS backend is not a reason to panic
    /// inside it.
    pub fn with_timeout(url: impl Into<String>, timeout: Duration) -> Self {
        let http = reqwest::Client::builder()
            .timeout(timeout)
            .build()
            .unwrap_or_else(|_| reqwest::Client::new());

        Self {
            base_url: url.into().trim_end_matches('/').to_string(),
            http,
        }
    }

    /// The base URL every call is resolved against, without a trailing
    /// slash.
    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    /// Reads the `url` path argument, refusing anything that is not a
    /// relative path.
    fn path_argument(&self, arguments: &Value) -> Result<String, ToolError> {
        let invalid = |reason: String| ToolError::InvalidArguments {
            tool: Self::NAME.to_string(),
            reason,
        };

        let url = arguments
            .get("url")
            .and_then(Value::as_str)
            .ok_or_else(|| invalid("expected a `url` string argument".to_string()))?
            .trim();

        if url.is_empty() {
            return Err(invalid("the `url` argument is empty".to_string()));
        }
        if url.contains("://") {
            return Err(invalid(format!(
                "`{url}` is an absolute URL; this tool only fetches paths relative to {}",
                self.base_url
            )));
        }

        Ok(if url.starts_with('/') {
            url.to_string()
        } else {
            format!("/{url}")
        })
    }
}

#[async_trait::async_trait]
impl Tool for FetchUrlTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: Self::NAME.to_string(),
            description: format!(
                "Fetches the body of a path with an HTTP GET. Paths are relative to {}, which \
                 is the only host this tool can reach.",
                self.base_url
            ),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "url": {
                        "type": "string",
                        "description": "Path to fetch, e.g. `/weather`.",
                    },
                },
                "required": ["url"],
            }),
        }
    }

    async fn execute(&self, arguments: &Value) -> Result<String, ToolError> {
        let path = self.path_argument(arguments)?;
        let url = format!("{}{}", self.base_url, path);

        let response = self
            .http
            .get(&url)
            .send()
            .await
            .map_err(|e| ToolError::Failed {
                tool: Self::NAME.to_string(),
                reason: format!("GET {url}: {e}"),
            })?;

        let status = response.status();
        if !status.is_success() {
            return Err(ToolError::Failed {
                tool: Self::NAME.to_string(),
                reason: format!("GET {url} answered HTTP {}", status.as_u16()),
            });
        }

        response.text().await.map_err(|e| ToolError::Failed {
            tool: Self::NAME.to_string(),
            reason: format!("GET {url} answered HTTP {status} but its body could not be read: {e}"),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A tool that records the arguments it was called with.
    struct Recorder {
        seen: Arc<tokio::sync::Mutex<Vec<Value>>>,
    }

    #[async_trait::async_trait]
    impl Tool for Recorder {
        fn definition(&self) -> ToolDefinition {
            ToolDefinition {
                name: "recorder".to_string(),
                description: "Records its arguments".to_string(),
                parameters: serde_json::json!({ "type": "object" }),
            }
        }

        async fn execute(&self, arguments: &Value) -> Result<String, ToolError> {
            self.seen.lock().await.push(arguments.clone());
            Ok("recorded".to_string())
        }
    }

    fn call(name: &str, arguments: Value) -> ToolCall {
        ToolCall {
            id: "call-1".to_string(),
            name: name.to_string(),
            arguments,
        }
    }

    #[tokio::test]
    async fn a_new_registry_advertises_nothing() {
        let registry = ToolRegistry::new();
        assert!(registry.is_empty().await);
        assert_eq!(registry.len().await, 0);
        assert!(registry.definitions().await.is_empty());
    }

    #[tokio::test]
    async fn registration_shows_up_in_the_advertised_definitions() {
        let registry = ToolRegistry::new();
        registry.register(Recorder { seen: Arc::new(tokio::sync::Mutex::new(Vec::new())) }).await;

        let definitions = registry.definitions().await;
        assert_eq!(definitions.len(), 1);
        assert_eq!(definitions[0].name, "recorder");
    }

    #[tokio::test]
    async fn a_registered_tool_runs_and_its_arguments_arrive_intact() {
        let seen = Arc::new(tokio::sync::Mutex::new(Vec::new()));
        let registry = ToolRegistry::new();
        registry.register(Recorder { seen: Arc::clone(&seen) }).await;

        let result = registry.execute(&call("recorder", serde_json::json!({"a": 1}))).await;

        assert_eq!(result.unwrap(), "recorded");
        let seen = seen.lock().await;
        assert_eq!(seen.len(), 1);
        assert_eq!(seen[0], serde_json::json!({"a": 1}));
    }

    #[tokio::test]
    async fn an_unknown_tool_is_an_error_and_not_a_panic() {
        let registry = ToolRegistry::new();
        let err = registry.execute(&call("does_not_exist", serde_json::json!({}))).await.unwrap_err();

        match &err {
            ToolError::NotFound { name } => assert_eq!(name, "does_not_exist"),
            other => panic!("expected NotFound, got {other:?}"),
        }
        assert!(err.to_string().contains("does_not_exist"), "{err}");
    }

    #[tokio::test]
    async fn re_registering_a_name_replaces_it_in_place() {
        let first = Arc::new(tokio::sync::Mutex::new(Vec::new()));
        let second = Arc::new(tokio::sync::Mutex::new(Vec::new()));
        let registry = ToolRegistry::new();

        registry.register(Recorder { seen: Arc::clone(&first) }).await;
        registry.register(Recorder { seen: Arc::clone(&second) }).await;

        assert_eq!(registry.len().await, 1);
        registry.execute(&call("recorder", serde_json::json!({}))).await.unwrap();
        assert!(first.lock().await.is_empty(), "the replaced tool must not run");
        assert_eq!(second.lock().await.len(), 1);
    }

    #[test]
    fn fetch_url_advertises_itself_with_a_url_parameter() {
        let tool = FetchUrlTool::with_base_url("http://127.0.0.1:1/");
        let definition = tool.definition();

        assert_eq!(definition.name, FetchUrlTool::NAME);
        assert_eq!(definition.parameters["required"][0], "url");
        // The trailing slash was trimmed, so concatenation cannot double it.
        assert_eq!(tool.base_url(), "http://127.0.0.1:1");
    }

    #[test]
    fn a_missing_or_unusable_url_argument_is_named_in_the_error() {
        let tool = FetchUrlTool::with_base_url("http://127.0.0.1:1");

        let missing = tool.path_argument(&serde_json::json!({})).unwrap_err();
        assert!(missing.to_string().contains("fetch_url"), "{missing}");

        let empty = tool.path_argument(&serde_json::json!({"url": "  "})).unwrap_err();
        assert!(empty.to_string().contains("empty"), "{empty}");

        let absolute = tool
            .path_argument(&serde_json::json!({"url": "https://example.org/steal"}))
            .unwrap_err();
        assert!(absolute.to_string().contains("absolute URL"), "{absolute}");
    }

    #[test]
    fn a_path_without_a_leading_slash_still_resolves_against_the_base() {
        let tool = FetchUrlTool::with_base_url("http://127.0.0.1:1");
        assert_eq!(tool.path_argument(&serde_json::json!({"url": "weather"})).unwrap(), "/weather");
        assert_eq!(tool.path_argument(&serde_json::json!({"url": "/weather"})).unwrap(), "/weather");
    }

    #[tokio::test]
    async fn fetch_url_returns_the_body_and_reports_a_failed_status() {
        let mut server = mockito::Server::new_async().await;
        let _ok = server.mock("GET", "/weather").with_status(200).with_body("sunny, 22C").create_async().await;
        let _missing = server.mock("GET", "/gone").with_status(404).create_async().await;

        let tool = FetchUrlTool::with_base_url(server.url());
        let arguments = serde_json::json!({"url": "/weather"});
        assert_eq!(tool.execute(&arguments).await.unwrap(), "sunny, 22C");

        let err = tool.execute(&serde_json::json!({"url": "/gone"})).await.unwrap_err();
        assert!(err.to_string().contains("404"), "{err}");
    }

    #[tokio::test]
    async fn fetch_url_reports_a_transport_failure_instead_of_panicking() {
        // Port 1 is reserved, so nothing is listening: a refused connection
        // is immediate, and the short timeout bounds the test either way.
        let tool = FetchUrlTool::with_timeout("http://127.0.0.1:1", Duration::from_millis(500));
        let err = tool.execute(&serde_json::json!({"url": "/weather"})).await.unwrap_err();
        assert!(matches!(err, ToolError::Failed { .. }), "{err:?}");
    }
}
