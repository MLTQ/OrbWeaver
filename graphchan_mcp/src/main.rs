use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::io::{self, BufRead};
use reqwest::Client;

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
    result: Option<Value>,
    error: Option<JsonRpcError>,
    id: Option<Value>,
}

#[derive(Serialize, Deserialize, Debug)]
struct JsonRpcError {
    code: i32,
    message: String,
    data: Option<Value>,
}

#[derive(Serialize, Deserialize, Debug)]
struct ThreadSummary {
    id: String,
    title: String,
    creator_peer_id: Option<String>,
    created_at: String,
}

#[derive(Serialize, Deserialize, Debug)]
struct ThreadDetails {
    thread: ThreadSummary,
    posts: Vec<PostView>,
}

#[derive(Serialize, Deserialize, Debug)]
struct PostView {
    id: String,
    thread_id: String,
    author_peer_id: Option<String>,
    body: String,
    created_at: String,
    parent_post_ids: Vec<String>,
}

const API_URL: &str = "http://127.0.0.1:8080";

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let client = Client::new();
    let stdin = io::stdin();
    
    // Simple line-based JSON-RPC processing
    for line in stdin.lock().lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }

        let request: JsonRpcRequest = match serde_json::from_str(&line) {
            Ok(req) => req,
            Err(e) => {
                eprintln!("Failed to parse request: {}", e);
                continue;
            }
        };

        let response = handle_request(&client, request).await;
        let response_json = serde_json::to_string(&response)?;
        println!("{}", response_json);
    }

    Ok(())
}

async fn handle_request(client: &Client, req: JsonRpcRequest) -> JsonRpcResponse {
    let result = match req.method.as_str() {
        "mcp.list_tools" => Ok(list_tools()),
        "mcp.call_tool" => call_tool(client, req.params).await,
        _ => Err(JsonRpcError {
            code: -32601,
            message: "Method not found".to_string(),
            data: None,
        }),
    };

    match result {
        Ok(val) => JsonRpcResponse {
            jsonrpc: "2.0".to_string(),
            result: Some(val),
            error: None,
            id: req.id,
        },
        Err(err) => JsonRpcResponse {
            jsonrpc: "2.0".to_string(),
            result: None,
            error: Some(err),
            id: req.id,
        },
    }
}

fn list_tools() -> Value {
    serde_json::json!({
        "tools": [
            {
                "name": "read_thread",
                "description": "Read a thread and its posts",
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
                "description": "Read the latest N posts of a thread",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "thread_id": { "type": "string" },
                        "n": { "type": "integer" }
                    },
                    "required": ["thread_id", "n"]
                }
            },
            {
                "name": "read_parents",
                "description": "Read the parent posts of a specific post",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "post_id": { "type": "string" },
                        "n": { "type": "integer", "description": "Number of generations to go up (optional, default 1)" }
                    },
                    "required": ["post_id"]
                }
            },
            {
                "name": "list_threads",
                "description": "List all available threads",
                "parameters": {
                    "type": "object",
                    "properties": {},
                }
            }
        ]
    })
}

async fn call_tool(client: &Client, params: Option<Value>) -> Result<Value, JsonRpcError> {
    let params = params.ok_or(JsonRpcError {
        code: -32602,
        message: "Invalid params".to_string(),
        data: None,
    })?;

    let name = params.get("name").and_then(|v| v.as_str()).ok_or(JsonRpcError {
        code: -32602,
        message: "Missing tool name".to_string(),
        data: None,
    })?;

    let default_args = serde_json::json!({});
    let args = params.get("arguments").unwrap_or(&default_args);

    match name {
        "list_threads" => {
            let threads: Vec<ThreadSummary> = client.get(format!("{}/threads", API_URL))
                .send().await.map_err(map_req_err)?
                .json().await.map_err(map_req_err)?;
            Ok(serde_json::to_value(threads).unwrap())
        }
        "read_thread" => {
            let thread_id = args.get("thread_id").and_then(|v| v.as_str()).ok_or(JsonRpcError {
                code: -32602,
                message: "Missing thread_id".to_string(),
                data: None,
            })?;
            
            let details: ThreadDetails = client.get(format!("{}/threads/{}", API_URL, thread_id))
                .send().await.map_err(map_req_err)?
                .json().await.map_err(map_req_err)?;
            
            Ok(serde_json::to_value(details).unwrap())
        }
        "read_latest_posts" => {
            let thread_id = args.get("thread_id").and_then(|v| v.as_str()).ok_or(JsonRpcError {
                code: -32602,
                message: "Missing thread_id".to_string(),
                data: None,
            })?;
            let n = args.get("n").and_then(|v| v.as_u64()).unwrap_or(10) as usize;

            let details: ThreadDetails = client.get(format!("{}/threads/{}", API_URL, thread_id))
                .send().await.map_err(map_req_err)?
                .json().await.map_err(map_req_err)?;
            
            let mut posts = details.posts;
            posts.sort_by(|a, b| b.created_at.cmp(&a.created_at)); // Newest first
            posts.truncate(n);
            
            Ok(serde_json::to_value(posts).unwrap())
        }
        "read_parents" => {
            // This is harder because we need to find the thread first or use an API that gets a post directly.
            // Assuming we don't have a direct "get post" API, we might need to search.
            // But wait, the backend usually has /posts/{id} or similar?
            // Let's assume we have to scan threads or the user provides thread_id.
            // For now, let's implement a naive search or error if not supported.
            // Actually, `read_parents` implies we know the post.
            // If the backend doesn't support fetching a post by ID directly, this is expensive.
            // Let's assume we can fetch the thread if we knew it.
            // But we don't.
            // Let's return an error for now saying "Not implemented efficiently".
            Err(JsonRpcError {
                code: -32603,
                message: "read_parents not implemented yet (requires backend support for direct post lookup)".to_string(),
                data: None,
            })
        }
        _ => Err(JsonRpcError {
            code: -32601,
            message: format!("Tool {} not found", name),
            data: None,
        }),
    }
}

fn map_req_err(e: reqwest::Error) -> JsonRpcError {
    JsonRpcError {
        code: -32000,
        message: format!("Request failed: {}", e),
        data: None,
    }
}
