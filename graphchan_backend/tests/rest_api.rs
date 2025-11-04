use graphchan_backend::bootstrap;
use graphchan_backend::config::{GraphchanConfig, GraphchanPaths, NetworkConfig};
use graphchan_backend::identity::{decode_friendcode, FriendCodePayload};
use graphchan_backend::network::NetworkHandle;
use graphchan_backend::{
    api,
    threading::{CreatePostInput, CreateThreadInput},
};
use tempfile::{tempdir, TempDir};
use tokio::time::{sleep, timeout, Duration};

struct TestNode {
    _dir: TempDir,
    _config: GraphchanConfig,
    _database: graphchan_backend::database::Database,
    identity: graphchan_backend::identity::IdentitySummary,
    network: NetworkHandle,
    server: tokio::task::JoinHandle<()>,
    base_url: String,
}

impl TestNode {
    async fn shutdown(self) {
        self.network.shutdown().await;
        self.server.abort();
        let _ = self.server.await;
    }
}

fn next_port() -> u16 {
    std::net::TcpListener::bind("127.0.0.1:0")
        .expect("bind ephemeral port")
        .local_addr()
        .unwrap()
        .port()
}

async fn wait_for_health(base_url: &str) {
    let client = reqwest::Client::new();
    for _ in 0..50 {
        if let Ok(resp) = client.get(format!("{base_url}/health")).send().await {
            if resp.status().is_success() {
                return;
            }
        }
        sleep(Duration::from_millis(100)).await;
    }
    panic!("server did not become healthy in time");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "requires local networking"]
async fn rest_roundtrip_with_file_upload() {
    let temp = tempdir().expect("tempdir");
    let port = next_port();
    let config = GraphchanConfig::new(
        port,
        GraphchanPaths::from_base_dir(temp.path()).expect("paths"),
        NetworkConfig::default(),
    );

    let bootstrap = bootstrap::initialize(&config).await.expect("bootstrap");
    let identity = bootstrap.identity.clone();
    let database = bootstrap.database.clone();
    let network = NetworkHandle::start(&config.paths, &config.network, database.clone())
        .await
        .expect("start network");

    let server_network = network.clone();
    let server_config = config.clone();
    let server_identity = identity.clone();
    let server_database = database.clone();
    let server = tokio::spawn(async move {
        let _ = api::serve_http(
            server_config,
            server_identity,
            server_database,
            server_network,
        )
        .await;
    });

    let base_url = format!("http://127.0.0.1:{port}");
    wait_for_health(&base_url).await;

    let client = reqwest::Client::new();

    let thread_resp: serde_json::Value = client
        .post(format!("{}/threads", base_url))
        .json(&CreateThreadInput {
            title: "Integration Thread".to_string(),
            body: Some("hello world".to_string()),
            creator_peer_id: None,
            pinned: Some(false),
        })
        .send()
        .await
        .expect("create thread response")
        .json()
        .await
        .expect("thread json");

    let thread_id = thread_resp
        .get("thread")
        .and_then(|t| t.get("id"))
        .and_then(|id| id.as_str())
        .expect("thread id");

    let post_resp = client
        .post(format!("{}/threads/{}/posts", base_url, thread_id))
        .json(&CreatePostInput {
            thread_id: thread_id.to_string(),
            author_peer_id: None,
            body: "second post".into(),
            parent_post_ids: vec![],
        })
        .send()
        .await
        .expect("create post response")
        .json::<serde_json::Value>()
        .await
        .expect("post json");

    let post_id = post_resp
        .get("post")
        .and_then(|p| p.get("id"))
        .and_then(|id| id.as_str())
        .expect("post id");

    let form = reqwest::multipart::Form::new().part(
        "file",
        reqwest::multipart::Part::bytes("file-body".as_bytes().to_vec())
            .file_name("hello.txt")
            .mime_str("text/plain")
            .unwrap(),
    );

    let file_resp: serde_json::Value = client
        .post(format!("{}/posts/{}/files", base_url, post_id))
        .multipart(form)
        .send()
        .await
        .expect("upload resp")
        .json()
        .await
        .expect("upload json");

    let file_id = file_resp
        .get("id")
        .and_then(|id| id.as_str())
        .expect("file id");

    let download = client
        .get(format!("{}/files/{}", base_url, file_id))
        .send()
        .await
        .expect("download")
        .bytes()
        .await
        .expect("download bytes");

    assert_eq!(download.as_ref(), b"file-body");

    network.shutdown().await;
    server.abort();
    let _ = server.await;
}

fn advertised_addresses(addr: &iroh_base::EndpointAddr) -> Vec<String> {
    let mut addresses = Vec::new();
    for ip in addr.ip_addrs() {
        addresses.push(ip.to_string());
    }
    for relay in addr.relay_urls() {
        addresses.push(relay.to_string());
    }
    addresses
}

async fn spawn_node(port: u16) -> TestNode {
    let dir = tempdir().expect("tempdir");
    let paths = GraphchanPaths::from_base_dir(dir.path()).expect("paths");
    let config = GraphchanConfig::new(port, paths.clone(), NetworkConfig::default());
    let bootstrap = bootstrap::initialize(&config).await.expect("bootstrap");
    let identity = bootstrap.identity.clone();
    let database = bootstrap.database.clone();
    let network = NetworkHandle::start(&config.paths, &config.network, database.clone())
        .await
        .expect("network start");

    let server_network = network.clone();
    let server_config = config.clone();
    let server_identity = identity.clone();
    let server_database = database.clone();
    let server = tokio::spawn(async move {
        let _ = api::serve_http(
            server_config,
            server_identity,
            server_database,
            server_network,
        )
        .await;
    });

    let base_url = format!("http://127.0.0.1:{port}");
    wait_for_health(&base_url).await;

    TestNode {
        _dir: dir,
        _config: config,
        _database: database,
        identity,
        network,
        server,
        base_url,
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 8)]
#[ignore = "requires local networking"]
async fn two_node_gossip_replication() {
    let port_a = next_port();
    let port_b = next_port();

    let node_a = spawn_node(port_a).await;
    let node_b = spawn_node(port_b).await;

    let payload_a = decorated_friendcode(&node_a.identity.friendcode, &node_a.network);
    let payload_b = decorated_friendcode(&node_b.identity.friendcode, &node_b.network);

    node_a
        .network
        .connect_friendcode(&payload_b)
        .await
        .expect("connect a->b");
    node_b
        .network
        .connect_friendcode(&payload_a)
        .await
        .expect("connect b->a");

    sleep(Duration::from_millis(500)).await;

    let client = reqwest::Client::new();

    // Node A creates a thread with an initial post.
    let thread_resp: serde_json::Value = client
        .post(format!("{}/threads", node_a.base_url))
        .json(&CreateThreadInput {
            title: "Replication Thread".to_string(),
            body: Some("hello from A".to_string()),
            creator_peer_id: None,
            pinned: Some(false),
        })
        .send()
        .await
        .expect("create thread response")
        .json()
        .await
        .expect("thread json");

    let thread_id = thread_resp
        .get("thread")
        .and_then(|t| t.get("id"))
        .and_then(|id| id.as_str())
        .expect("thread id")
        .to_string();

    // Wait for Node B to learn about the thread.
    wait_until(|| async {
        let resp = client
            .get(format!("{}/threads/{}", node_b.base_url, thread_id))
            .send()
            .await
            .ok()?;
        if !resp.status().is_success() {
            return None;
        }
        let json: serde_json::Value = resp.json().await.ok()?;
        let posts = json.get("posts")?.as_array()?;
        if posts.is_empty() {
            return None;
        }
        Some(json)
    })
    .await;

    // Node B creates a reply.
    let post_resp = client
        .post(format!("{}/threads/{}/posts", node_b.base_url, thread_id))
        .json(&CreatePostInput {
            thread_id: thread_id.clone(),
            author_peer_id: None,
            body: "hello from B".into(),
            parent_post_ids: vec![],
        })
        .send()
        .await
        .expect("create post response")
        .json::<serde_json::Value>()
        .await
        .expect("post json");

    let post_id = post_resp
        .get("post")
        .and_then(|p| p.get("id"))
        .and_then(|id| id.as_str())
        .expect("post id")
        .to_string();

    // Wait for Node A to observe the reply.
    wait_until(|| async {
        let resp = client
            .get(format!("{}/threads/{}", node_a.base_url, thread_id))
            .send()
            .await
            .ok()?;
        if !resp.status().is_success() {
            return None;
        }
        let json: serde_json::Value = resp.json().await.ok()?;
        let posts = json.get("posts")?.as_array()?;
        if posts.len() < 2 {
            return None;
        }
        Some(())
    })
    .await;

    // Node A uploads a file to the reply created by Node B.
    let form = reqwest::multipart::Form::new().part(
        "file",
        reqwest::multipart::Part::bytes("gossip file".as_bytes().to_vec())
            .file_name("note.txt")
            .mime_str("text/plain")
            .unwrap(),
    );

    let file_resp: serde_json::Value = client
        .post(format!("{}/posts/{}/files", node_a.base_url, post_id))
        .multipart(form)
        .send()
        .await
        .expect("upload file response")
        .json()
        .await
        .expect("file json");

    let file_id = file_resp
        .get("id")
        .and_then(|id| id.as_str())
        .expect("file id")
        .to_string();

    // Wait for Node B to download the file.
    wait_until(|| async {
        let resp = client
            .get(format!("{}/posts/{}/files", node_b.base_url, post_id))
            .send()
            .await
            .ok()?;
        if !resp.status().is_success() {
            return None;
        }
        let json: serde_json::Value = resp.json().await.ok()?;
        let files = json.as_array()?;
        let found = files.iter().any(|entry| {
            entry
                .get("id")
                .and_then(|id| id.as_str())
                .map(|value| value == file_id)
                .unwrap_or(false)
        });
        if found {
            Some(())
        } else {
            None
        }
    })
    .await;

    let downloaded = client
        .get(format!("{}/files/{}", node_b.base_url, file_id))
        .send()
        .await
        .expect("download response")
        .bytes()
        .await
        .expect("download bytes");
    assert_eq!(downloaded, "gossip file");

    node_a.shutdown().await;
    node_b.shutdown().await;
}

fn decorated_friendcode(friendcode: &str, network: &NetworkHandle) -> FriendCodePayload {
    let mut payload = decode_friendcode(friendcode).expect("decode friendcode");
    let addr = network.current_addr();
    let mut addresses = advertised_addresses(&addr);
    if addresses.is_empty() {
        if let Some(primary) = addr.ip_addrs().next() {
            addresses.push(primary.to_string());
        }
    }
    payload.addresses = addresses;
    payload
}

async fn wait_until<F, Fut, T>(mut check: F) -> T
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = Option<T>>,
{
    timeout(Duration::from_secs(10), async {
        loop {
            if let Some(value) = check().await {
                break value;
            }
            sleep(Duration::from_millis(200)).await;
        }
    })
    .await
    .expect("condition not met in time")
}
