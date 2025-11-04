use crate::api;
use crate::config::GraphchanConfig;
use crate::database::Database;
use crate::files::FileService;
use crate::identity::{decode_friendcode, IdentitySummary};
use crate::network::NetworkHandle;
use crate::peers::PeerService;
use crate::threading::{CreatePostInput, CreateThreadInput, ThreadDetails, ThreadService};
use crate::utils::now_utc_iso;
use anyhow::{anyhow, Context, Result};
use shell_words;
use std::collections::{HashMap, HashSet};
use std::io::{self, Write};
use tokio::io::{AsyncBufReadExt, BufReader};

/// Run the HTTP server mode (former default behaviour).
pub async fn run_server(
    config: GraphchanConfig,
    identity: IdentitySummary,
    database: Database,
    network: NetworkHandle,
) -> Result<()> {
    tracing::info!(
        port = config.api_port,
        "starting Graphchan backend HTTP server"
    );
    api::serve_http(config, identity, database, network).await
}

/// Run the interactive CLI used for managing friendcodes, threads, and posts.
pub async fn run_cli(
    config: GraphchanConfig,
    identity: IdentitySummary,
    database: Database,
    network: NetworkHandle,
) -> Result<()> {
    let thread_service = ThreadService::new(database.clone());
    let peer_service = PeerService::new(database.clone());
    let file_service = FileService::new(database.clone(), config.paths.clone());

    let mut session = CliSession {
        identity,
        network,
        thread_service,
        peer_service,
        file_service,
        last_seen_posts: HashMap::new(),
    };

    println!("Graphchan CLI ready. Type 'help' for a list of commands.");
    println!("Your friendcode: {}", session.identity.friendcode);
    session.print_addresses();

    let stdin = tokio::io::stdin();
    let mut reader = BufReader::new(stdin);

    loop {
        print!("graphchan> ");
        io::stdout().flush()?;

        let mut line = String::new();
        let read = reader.read_line(&mut line).await?;
        if read == 0 {
            println!("Exiting");
            break;
        }
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        let tokens = match shell_words::split(trimmed) {
            Ok(tokens) if !tokens.is_empty() => tokens,
            Ok(_) => continue,
            Err(err) => {
                println!("Unable to parse command: {err}");
                continue;
            }
        };

        match session.handle_command(&tokens).await {
            Ok(LoopAction::Continue) => {}
            Ok(LoopAction::Exit) => break,
            Err(err) => {
                println!("Error: {err:#}");
            }
        }
    }

    session.network.shutdown().await;
    Ok(())
}

struct CliSession {
    identity: IdentitySummary,
    network: NetworkHandle,
    thread_service: ThreadService,
    peer_service: PeerService,
    file_service: FileService,
    last_seen_posts: HashMap<String, String>,
}

enum LoopAction {
    Continue,
    Exit,
}

impl CliSession {
    async fn handle_command(&mut self, tokens: &[String]) -> Result<LoopAction> {
        let command = tokens[0].as_str();
        match command {
            "help" => {
                self.print_help();
                Ok(LoopAction::Continue)
            }
            "friendcode" => {
                println!("{}", self.identity.friendcode);
                self.print_addresses();
                Ok(LoopAction::Continue)
            }
            "add-friend" | "subscribe" => {
                if tokens.len() < 2 {
                    println!("Usage: add-friend <friendcode>");
                    return Ok(LoopAction::Continue);
                }
                self.add_friend(&tokens[1]).await?;
                Ok(LoopAction::Continue)
            }
            "list-friends" | "friends" => {
                self.list_friends().await?;
                Ok(LoopAction::Continue)
            }
            "list-threads" | "threads" => {
                let limit = tokens
                    .get(1)
                    .and_then(|v| v.parse::<usize>().ok())
                    .unwrap_or(20);
                self.list_threads(limit)?;
                Ok(LoopAction::Continue)
            }
            "view-thread" | "thread" => {
                if tokens.len() < 2 {
                    println!("Usage: view-thread <thread_id>");
                    return Ok(LoopAction::Continue);
                }
                self.view_thread(&tokens[1])?;
                Ok(LoopAction::Continue)
            }
            "new-thread" | "create-thread" => {
                if tokens.len() < 2 {
                    println!("Usage: new-thread \"title\" [\"initial body\"]");
                    return Ok(LoopAction::Continue);
                }
                let title = tokens[1].clone();
                let body = if tokens.len() > 2 {
                    Some(tokens[2..].join(" "))
                } else {
                    None
                };
                self.create_thread(title, body).await?;
                Ok(LoopAction::Continue)
            }
            "post" | "reply" => {
                if tokens.len() < 3 {
                    println!("Usage: post <thread_id> \"message\"");
                    return Ok(LoopAction::Continue);
                }
                let thread_id = tokens[1].clone();
                let body = tokens[2..].join(" ");
                self.create_post(thread_id, body).await?;
                Ok(LoopAction::Continue)
            }
            "check" | "refresh" => {
                self.check_new_posts().await?;
                Ok(LoopAction::Continue)
            }
            "quit" | "exit" => Ok(LoopAction::Exit),
            "clear" => {
                print!("\x1B[2J\x1B[1;1H");
                Ok(LoopAction::Continue)
            }
            other => {
                println!("Unknown command '{other}'. Type 'help' for a list of commands.");
                Ok(LoopAction::Continue)
            }
        }
    }

    fn print_help(&self) {
        println!("Available commands:");
        println!("  help                 Show this help message");
        println!("  friendcode           Print your friendcode and known addresses");
        println!("  add-friend <code>    Register a remote friendcode and attempt connection");
        println!("  list-friends         Show known peers and online status");
        println!("  list-threads [N]     List recent threads (default 20)");
        println!("  view-thread <id>     Display posts within a thread");
        println!("  new-thread TITLE [BODY]  Create a new thread with optional initial post");
        println!("  post <thread_id> MSG Post a reply to an existing thread");
        println!("  check                Poll for new messages across all threads");
        println!("  clear                Clear the screen");
        println!("  exit                 Quit the CLI");
    }

    fn print_addresses(&self) {
        let addr = self.network.current_addr();
        let addresses = advertised_addresses(&addr);
        if !addresses.is_empty() {
            println!("Known addresses:");
            for entry in addresses {
                println!("  - {entry}");
            }
        }
    }

    async fn add_friend(&mut self, friendcode: &str) -> Result<()> {
        let peer = self
            .peer_service
            .register_friendcode(friendcode)
            .with_context(|| "failed to register friendcode")?;
        println!("Registered peer {}", peer.id);
        if let Ok(payload) = decode_friendcode(friendcode) {
            self.network
                .connect_friendcode(&payload)
                .await
                .inspect_err(|err| tracing::warn!(error = ?err, "failed to connect to peer"))
                .ok();
        }
        Ok(())
    }

    async fn list_friends(&self) -> Result<()> {
        let peers = self.peer_service.list_peers()?;
        let connected: HashSet<String> = self
            .network
            .connected_peer_ids()
            .await
            .into_iter()
            .collect();
        if peers.is_empty() {
            println!("No peers registered yet.");
            return Ok(());
        }
        println!("Peers:");
        for peer in peers {
            let peer_id = peer
                .iroh_peer_id
                .clone()
                .unwrap_or_else(|| "(unknown peer id)".into());
            let alias = peer.alias.unwrap_or_else(|| peer.id.clone());
            let status = if connected.contains(&peer_id) {
                "online"
            } else {
                "offline"
            };
            println!("  {} [{}] - {}", alias, peer_id, status);
        }
        Ok(())
    }

    fn list_threads(&self, limit: usize) -> Result<()> {
        let summaries = self.thread_service.list_threads(limit)?;
        if summaries.is_empty() {
            println!("No threads yet. Use 'new-thread' to create one.");
            return Ok(());
        }
        println!("Threads:");
        for summary in summaries {
            let details = self.thread_service.get_thread(&summary.id)?;
            let (post_count, latest_post_id) = details
                .as_ref()
                .map(|d| {
                    let latest = d.posts.last().map(|p| p.id.clone());
                    (d.posts.len(), latest)
                })
                .unwrap_or((0, None));
            let unread = latest_post_id
                .as_ref()
                .map(|id| self.is_unread(&summary.id, id))
                .unwrap_or(false);
            let marker = if unread { " *new" } else { "" };
            println!(
                "  [{}] {} (posts: {}){}",
                summary.id, summary.title, post_count, marker
            );
        }
        Ok(())
    }

    fn view_thread(&mut self, thread_id: &str) -> Result<()> {
        let Some(details) = self.thread_service.get_thread(thread_id)? else {
            println!("Thread {thread_id} not found");
            return Ok(());
        };

        println!("Thread: {}", details.thread.title);
        println!("Created at {}", details.thread.created_at);
        if details.posts.is_empty() {
            println!("  (no posts yet)");
        }
        for (index, post) in details.posts.iter().enumerate() {
            println!();
            println!("Post #{} ({})", index + 1, post.id);
            if let Some(author) = &post.author_peer_id {
                println!("Author: {author}");
            }
            println!("Created: {}", post.created_at);
            println!("Body: {}", post.body);
            let files = self.file_service.list_post_files(&post.id)?;
            if !files.is_empty() {
                println!("Attachments:");
                for file in files {
                    println!(
                        "  - {} ({} bytes)",
                        file.original_name.unwrap_or_else(|| file.id.clone()),
                        file.size_bytes.unwrap_or(0)
                    );
                }
            }
        }

        if let Some(last) = details.posts.last() {
            self.last_seen_posts
                .insert(details.thread.id.clone(), last.id.clone());
        }
        Ok(())
    }

    async fn create_thread(&mut self, title: String, body: Option<String>) -> Result<()> {
        let input = CreateThreadInput {
            title,
            body: body.clone(),
            creator_peer_id: Some(self.identity.gpg_fingerprint.clone()),
            pinned: Some(false),
        };
        let details = self.thread_service.create_thread(input)?;
        println!("Created thread {}", details.thread.id);
        self.network
            .publish_thread_snapshot(details.clone())
            .await
            .inspect_err(|err| tracing::warn!(error = ?err, "failed to gossip thread"))
            .ok();
        if let Some(last) = details.posts.last() {
            self.last_seen_posts
                .insert(details.thread.id.clone(), last.id.clone());
        }
        Ok(())
    }

    async fn create_post(&mut self, thread_id: String, body: String) -> Result<()> {
        let input = CreatePostInput {
            thread_id: thread_id.clone(),
            author_peer_id: Some(self.identity.gpg_fingerprint.clone()),
            body,
            parent_post_ids: vec![],
        };
        let post = self.thread_service.create_post(input)?;
        println!("Posted message {}", post.id);
        self.network
            .publish_post_update(post.clone())
            .await
            .inspect_err(|err| tracing::warn!(error = ?err, "failed to gossip post"))
            .ok();
        self.last_seen_posts.insert(thread_id, post.id);
        Ok(())
    }

    async fn check_new_posts(&mut self) -> Result<()> {
        let threads = self.thread_service.list_threads(100)?;
        let mut updates = Vec::new();
        for summary in threads {
            let Some(details) = self.thread_service.get_thread(&summary.id)? else {
                continue;
            };
            if let Some(last) = details.posts.last() {
                if self.is_unread(&summary.id, &last.id) {
                    updates.push((summary.title.clone(), last.created_at.clone()));
                    self.last_seen_posts
                        .insert(summary.id.clone(), last.id.clone());
                }
            }
        }
        if updates.is_empty() {
            println!("No new messages.");
        } else {
            println!("New activity:");
            for (title, created_at) in updates {
                println!("  - {title} (latest at {created_at})");
            }
        }
        Ok(())
    }

    fn is_unread(&self, thread_id: &str, latest_post_id: &str) -> bool {
        match self.last_seen_posts.get(thread_id) {
            Some(previous) => previous != latest_post_id,
            None => true,
        }
    }
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
