//! Graphchan MCP server.
//!
//! Exposes Graphchan's REST API as MCP tools over JSON-RPC on stdio so MCP
//! clients (Claude Desktop, etc.) can drive a running backend node. The
//! server is a thin transport: every tool maps directly to one or more REST
//! calls. The user must already have a Graphchan backend running.
//!
//! Configuration:
//! - `GRAPHCHAN_API_URL`  — base URL of the backend (default
//!   `http://127.0.0.1:8080`). The bundled desktop launcher exports this so
//!   running both side-by-side just works.
//! - `GRAPHCHAN_API_TOKEN` — bearer token, if the backend has auth enabled.
//!   The desktop launcher mints a per-launch token; agents launched in the
//!   same environment inherit it automatically.

use anyhow::{anyhow, Result};
use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION};
use reqwest::{Client, Url};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::io::{self, BufRead, Write};

// --- JSON-RPC envelope -----------------------------------------------------

#[derive(Serialize, Deserialize, Debug)]
struct JsonRpcRequest {
    jsonrpc: String,
    method: String,
    params: Option<Value>,
    id: Option<Value>,
}

#[derive(Serialize, Deserialize, Debug)]
struct JsonRpcResponse {
    jsonrpc: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<JsonRpcError>,
    id: Option<Value>,
}

#[derive(Serialize, Deserialize, Debug)]
struct JsonRpcError {
    code: i32,
    message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    data: Option<Value>,
}

// --- Backend client --------------------------------------------------------

/// Wraps the configured `reqwest::Client` and base URL. Every tool goes
/// through this so headers (auth) and base URL composition stay in one place.
struct Backend {
    client: Client,
    base_url: Url,
}

impl Backend {
    fn from_env() -> Result<Self> {
        let base_url_raw = std::env::var("GRAPHCHAN_API_URL")
            .unwrap_or_else(|_| "http://127.0.0.1:8080".to_string());
        let base_url = Url::parse(&base_url_raw)
            .map_err(|e| anyhow!("invalid GRAPHCHAN_API_URL '{}': {}", base_url_raw, e))?;

        let mut headers = HeaderMap::new();
        if let Ok(token) = std::env::var("GRAPHCHAN_API_TOKEN") {
            let trimmed = token.trim();
            if !trimmed.is_empty() {
                let mut value = HeaderValue::from_str(&format!("Bearer {trimmed}"))?;
                value.set_sensitive(true);
                headers.insert(AUTHORIZATION, value);
            }
        }

        let client = Client::builder()
            .user_agent("graphchan-mcp/0.1")
            .default_headers(headers)
            .timeout(std::time::Duration::from_secs(30))
            .build()?;

        Ok(Self { client, base_url })
    }

    fn url(&self, path: &str) -> Result<Url> {
        Ok(self.base_url.join(path)?)
    }

    async fn get_json(&self, path: &str) -> Result<Value> {
        let resp = self.client.get(self.url(path)?).send().await?;
        let status = resp.status();
        let body: Value = resp.json().await?;
        if !status.is_success() {
            return Err(anyhow!("backend {} returned {}: {}", path, status, body));
        }
        Ok(body)
    }

    async fn post_json(&self, path: &str, body: &Value) -> Result<Value> {
        let resp = self.client.post(self.url(path)?).json(body).send().await?;
        let status = resp.status();
        let text = resp.text().await?;
        if !status.is_success() {
            return Err(anyhow!("backend {} returned {}: {}", path, status, text));
        }
        if text.is_empty() {
            return Ok(Value::Null);
        }
        Ok(serde_json::from_str(&text)?)
    }

    async fn delete(&self, path: &str) -> Result<()> {
        let resp = self.client.delete(self.url(path)?).send().await?;
        let status = resp.status();
        if !status.is_success() {
            let text = resp.text().await.unwrap_or_default();
            return Err(anyhow!("backend {} returned {}: {}", path, status, text));
        }
        Ok(())
    }
}

// --- Tool registry ---------------------------------------------------------

/// JSON Schema for the tool catalog returned by `mcp.list_tools`. Tools are
/// grouped by feature (read / write / peers / dms / moderation) so consumers
/// can discover the surface area at a glance.
fn list_tools() -> Value {
    json!({
        "tools": [
            // -- reading ----
            {
                "name": "list_threads",
                "description": "List recent threads on this node (most recent first).",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "limit": { "type": "integer", "description": "Max threads to return (default 50)" }
                    }
                }
            },
            {
                "name": "read_thread",
                "description": "Read a thread including all posts, peer metadata, and attached files.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "thread_id": { "type": "string" }
                    },
                    "required": ["thread_id"]
                }
            },
            {
                "name": "read_latest_posts",
                "description": "Get the N most recent posts in a thread.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "thread_id": { "type": "string" },
                        "n": { "type": "integer", "description": "How many posts (default 10)" }
                    },
                    "required": ["thread_id"]
                }
            },
            {
                "name": "read_recent_posts",
                "description": "Get recent posts across all threads (cross-thread feed).",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "limit": { "type": "integer", "description": "Max posts (default 50)" }
                    }
                }
            },
            {
                "name": "read_parents",
                "description": "Get the parent posts (reply targets) of a specific post.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "thread_id": { "type": "string" },
                        "post_id": { "type": "string" }
                    },
                    "required": ["thread_id", "post_id"]
                }
            },
            {
                "name": "search",
                "description": "Full-text search across posts and file names.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "query": { "type": "string" },
                        "limit": { "type": "integer", "description": "Max results (default 20)" }
                    },
                    "required": ["query"]
                }
            },
            // -- writing ----
            {
                "name": "create_thread",
                "description": "Create a new thread. The local node becomes the creator and the thread is broadcast to peers.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "title": { "type": "string" },
                        "body": { "type": "string", "description": "Optional initial post body" },
                        "topics": {
                            "type": "array",
                            "items": { "type": "string" },
                            "description": "Optional list of topic IDs to announce on for public discovery"
                        },
                        "agent_name": {
                            "type": "string",
                            "description": "Optional agent identity to record in post metadata"
                        }
                    },
                    "required": ["title"]
                }
            },
            {
                "name": "create_post",
                "description": "Reply in a thread. Optionally specify parent_post_ids to fork or thread under existing posts.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "thread_id": { "type": "string" },
                        "body": { "type": "string" },
                        "parent_post_ids": {
                            "type": "array",
                            "items": { "type": "string" }
                        },
                        "agent_name": {
                            "type": "string",
                            "description": "Optional agent identity recorded in post metadata"
                        }
                    },
                    "required": ["thread_id", "body"]
                }
            },
            {
                "name": "react",
                "description": "Add an emoji reaction to a post.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "post_id": { "type": "string" },
                        "emoji": { "type": "string", "description": "A single emoji, e.g. '👍'" }
                    },
                    "required": ["post_id", "emoji"]
                }
            },
            {
                "name": "unreact",
                "description": "Remove a previously-added reaction.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "post_id": { "type": "string" },
                        "emoji": { "type": "string" }
                    },
                    "required": ["post_id", "emoji"]
                }
            },
            // -- peers ----
            {
                "name": "get_self",
                "description": "Get this node's identity (peer id, friend code, profile, addresses).",
                "parameters": { "type": "object", "properties": {} }
            },
            {
                "name": "list_peers",
                "description": "List known peers (followed, discovered, or imported via friendcode).",
                "parameters": { "type": "object", "properties": {} }
            },
            {
                "name": "add_peer",
                "description": "Add a peer by friendcode. Establishes a follow relationship and subscribes to their announcements.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "friendcode": { "type": "string" }
                    },
                    "required": ["friendcode"]
                }
            },
            // -- direct messages ----
            {
                "name": "list_conversations",
                "description": "List active DM conversations.",
                "parameters": { "type": "object", "properties": {} }
            },
            {
                "name": "read_messages",
                "description": "Read decrypted DMs from a specific peer.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "peer_id": { "type": "string" },
                        "limit": { "type": "integer" }
                    },
                    "required": ["peer_id"]
                }
            },
            {
                "name": "send_dm",
                "description": "Send an encrypted direct message to a peer.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "to_peer_id": { "type": "string" },
                        "body": { "type": "string" }
                    },
                    "required": ["to_peer_id", "body"]
                }
            },
            {
                "name": "get_unread_count",
                "description": "Get the total number of unread DMs across all conversations.",
                "parameters": { "type": "object", "properties": {} }
            },
            // -- moderation ----
            {
                "name": "block_peer",
                "description": "Block a peer's posts and DMs from this node.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "peer_id": { "type": "string" },
                        "reason": { "type": "string" }
                    },
                    "required": ["peer_id"]
                }
            },
            {
                "name": "unblock_peer",
                "description": "Reverse a peer block.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "peer_id": { "type": "string" }
                    },
                    "required": ["peer_id"]
                }
            },
            {
                "name": "list_blocked_peers",
                "description": "List peers currently blocked on this node.",
                "parameters": { "type": "object", "properties": {} }
            },
            // -- health ----
            {
                "name": "health",
                "description": "Get backend health, identity summary, and DHT status.",
                "parameters": { "type": "object", "properties": {} }
            }
        ]
    })
}

// --- Tool dispatch ---------------------------------------------------------

async fn call_tool(backend: &Backend, params: Option<Value>) -> Result<Value> {
    let params = params.ok_or_else(|| anyhow!("missing params"))?;
    let name = params
        .get("name")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow!("missing tool name"))?;
    let default_args = json!({});
    let args = params.get("arguments").unwrap_or(&default_args);

    match name {
        // -- reading ----
        "list_threads" => {
            let limit = args.get("limit").and_then(|v| v.as_u64()).unwrap_or(50);
            backend.get_json(&format!("/threads?limit={limit}")).await
        }
        "read_thread" => {
            let thread_id = require_str(args, "thread_id")?;
            backend.get_json(&format!("/threads/{thread_id}")).await
        }
        "read_latest_posts" => {
            let thread_id = require_str(args, "thread_id")?;
            let n = args.get("n").and_then(|v| v.as_u64()).unwrap_or(10) as usize;
            let details = backend.get_json(&format!("/threads/{thread_id}")).await?;
            let mut posts: Vec<Value> = details
                .get("posts")
                .and_then(|v| v.as_array())
                .cloned()
                .unwrap_or_default();
            posts.sort_by(|a, b| {
                b.get("created_at")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .cmp(a.get("created_at").and_then(|v| v.as_str()).unwrap_or(""))
            });
            posts.truncate(n);
            Ok(Value::Array(posts))
        }
        "read_recent_posts" => {
            let limit = args.get("limit").and_then(|v| v.as_u64()).unwrap_or(50);
            backend.get_json(&format!("/posts/recent?limit={limit}")).await
        }
        "read_parents" => {
            // Find the post inside the thread and return entries matching parent_post_ids.
            let thread_id = require_str(args, "thread_id")?;
            let post_id = require_str(args, "post_id")?;
            let details = backend.get_json(&format!("/threads/{thread_id}")).await?;
            let posts = details
                .get("posts")
                .and_then(|v| v.as_array())
                .cloned()
                .unwrap_or_default();
            let parent_ids: Vec<String> = posts
                .iter()
                .find(|p| p.get("id").and_then(|v| v.as_str()) == Some(post_id))
                .and_then(|p| p.get("parent_post_ids").and_then(|v| v.as_array()).cloned())
                .unwrap_or_default()
                .into_iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect();
            let parents: Vec<Value> = posts
                .into_iter()
                .filter(|p| {
                    p.get("id")
                        .and_then(|v| v.as_str())
                        .map(|id| parent_ids.iter().any(|pid| pid == id))
                        .unwrap_or(false)
                })
                .collect();
            Ok(Value::Array(parents))
        }
        "search" => {
            let query = require_str(args, "query")?;
            let limit = args.get("limit").and_then(|v| v.as_u64()).unwrap_or(20);
            let encoded = urlencoding(query);
            backend
                .get_json(&format!("/search?q={encoded}&limit={limit}"))
                .await
        }

        // -- writing ----
        "create_thread" => {
            // The /threads endpoint is multipart (json + optional files) so we
            // bypass post_json and use a plain multipart with a single json field.
            let title = require_str(args, "title")?.to_string();
            let body = args
                .get("body")
                .and_then(|v| v.as_str())
                .map(String::from);
            let topics: Vec<String> = args
                .get("topics")
                .and_then(|v| v.as_array())
                .map(|a| {
                    a.iter()
                        .filter_map(|x| x.as_str().map(String::from))
                        .collect()
                })
                .unwrap_or_default();
            let agent_name = args
                .get("agent_name")
                .and_then(|v| v.as_str())
                .map(String::from);

            let mut json_payload = json!({ "title": title });
            if let Some(b) = body {
                json_payload["body"] = json!(b);
            }
            json_payload["topics"] = json!(topics);
            if let Some(name) = agent_name {
                json_payload["metadata"] = json!({ "agent": { "name": name } });
            }

            let form = reqwest::multipart::Form::new().part(
                "json",
                reqwest::multipart::Part::text(json_payload.to_string())
                    .mime_str("application/json")?,
            );

            let resp = backend
                .client
                .post(backend.url("/threads")?)
                .multipart(form)
                .send()
                .await?;
            let status = resp.status();
            let text = resp.text().await?;
            if !status.is_success() {
                return Err(anyhow!("create_thread returned {}: {}", status, text));
            }
            Ok(serde_json::from_str(&text)?)
        }
        "create_post" => {
            let thread_id = require_str(args, "thread_id")?;
            let body = require_str(args, "body")?;
            let parent_post_ids: Vec<String> = args
                .get("parent_post_ids")
                .and_then(|v| v.as_array())
                .map(|a| {
                    a.iter()
                        .filter_map(|x| x.as_str().map(String::from))
                        .collect()
                })
                .unwrap_or_default();
            let agent_name = args
                .get("agent_name")
                .and_then(|v| v.as_str())
                .map(String::from);
            let mut payload = json!({
                "thread_id": thread_id,
                "body": body,
                "parent_post_ids": parent_post_ids,
            });
            if let Some(name) = agent_name {
                payload["metadata"] = json!({ "agent": { "name": name } });
            }
            backend
                .post_json(&format!("/threads/{thread_id}/posts"), &payload)
                .await
        }
        "react" => {
            let post_id = require_str(args, "post_id")?;
            let emoji = require_str(args, "emoji")?;
            backend
                .post_json(
                    &format!("/posts/{post_id}/react"),
                    &json!({ "emoji": emoji }),
                )
                .await
        }
        "unreact" => {
            let post_id = require_str(args, "post_id")?;
            let emoji = require_str(args, "emoji")?;
            backend
                .post_json(
                    &format!("/posts/{post_id}/unreact"),
                    &json!({ "emoji": emoji }),
                )
                .await
        }

        // -- peers ----
        "get_self" => backend.get_json("/peers/self").await,
        "list_peers" => backend.get_json("/peers").await,
        "add_peer" => {
            let friendcode = require_str(args, "friendcode")?;
            backend
                .post_json("/peers", &json!({ "friendcode": friendcode }))
                .await
        }

        // -- direct messages ----
        "list_conversations" => backend.get_json("/dms/conversations").await,
        "read_messages" => {
            let peer_id = require_str(args, "peer_id")?;
            let limit = args.get("limit").and_then(|v| v.as_u64()).unwrap_or(50);
            backend
                .get_json(&format!("/dms/{peer_id}/messages?limit={limit}"))
                .await
        }
        "send_dm" => {
            let to_peer_id = require_str(args, "to_peer_id")?;
            let body = require_str(args, "body")?;
            backend
                .post_json(
                    "/dms/send",
                    &json!({ "to_peer_id": to_peer_id, "body": body }),
                )
                .await
        }
        "get_unread_count" => backend.get_json("/dms/unread/count").await,

        // -- moderation ----
        "block_peer" => {
            let peer_id = require_str(args, "peer_id")?;
            let reason = args.get("reason").and_then(|v| v.as_str()).map(String::from);
            backend
                .post_json(
                    &format!("/blocking/peers/{peer_id}"),
                    &json!({ "reason": reason }),
                )
                .await
        }
        "unblock_peer" => {
            let peer_id = require_str(args, "peer_id")?;
            backend.delete(&format!("/blocking/peers/{peer_id}")).await?;
            Ok(json!({ "ok": true }))
        }
        "list_blocked_peers" => backend.get_json("/blocking/peers").await,

        // -- health ----
        "health" => backend.get_json("/health").await,

        other => Err(anyhow!("tool '{}' not found", other)),
    }
}

fn require_str<'a>(args: &'a Value, name: &str) -> Result<&'a str> {
    args.get(name)
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow!("missing or non-string argument '{}'", name))
}

/// Minimal URL-encoding for the search query — avoids pulling a whole
/// crate just to escape a few characters.
fn urlencoding(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for byte in input.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(byte as char);
            }
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}

// --- Main loop -------------------------------------------------------------

async fn handle_request(backend: &Backend, req: JsonRpcRequest) -> JsonRpcResponse {
    let result = match req.method.as_str() {
        "mcp.list_tools" => Ok(list_tools()),
        "mcp.call_tool" => call_tool(backend, req.params).await,
        other => Err(anyhow!("method '{}' not found", other)),
    };

    match result {
        Ok(value) => JsonRpcResponse {
            jsonrpc: "2.0".into(),
            result: Some(value),
            error: None,
            id: req.id,
        },
        Err(err) => {
            // -32603 = Internal error, -32601 for not-found at the JSON-RPC level
            let code = if err.to_string().contains("not found") {
                -32601
            } else {
                -32603
            };
            JsonRpcResponse {
                jsonrpc: "2.0".into(),
                result: None,
                error: Some(JsonRpcError {
                    code,
                    message: err.to_string(),
                    data: None,
                }),
                id: req.id,
            }
        }
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let backend = Backend::from_env()?;
    eprintln!(
        "graphchan-mcp ready (api={}, auth={})",
        backend.base_url,
        if std::env::var("GRAPHCHAN_API_TOKEN")
            .map(|s| !s.trim().is_empty())
            .unwrap_or(false)
        {
            "bearer"
        } else {
            "none"
        }
    );

    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut out = stdout.lock();

    for line in stdin.lock().lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        let req: JsonRpcRequest = match serde_json::from_str(&line) {
            Ok(req) => req,
            Err(err) => {
                eprintln!("failed to parse JSON-RPC request: {err}");
                continue;
            }
        };
        let resp = handle_request(&backend, req).await;
        writeln!(out, "{}", serde_json::to_string(&resp)?)?;
        out.flush()?;
    }

    Ok(())
}
