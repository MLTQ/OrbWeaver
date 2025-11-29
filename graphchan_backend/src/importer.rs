use std::collections::HashMap;
use std::time::Duration;

use anyhow::{Context, Result};
use html2text::from_read;
use regex::Regex;
use serde::Deserialize;
use tokio::time::sleep;

use crate::api::AppState;
use crate::files::{FileService, SaveFileInput};
use crate::threading::{CreatePostInput, CreateThreadInput, ThreadService};

#[derive(Deserialize)]
struct FourChanThreadResponse {
    posts: Vec<FourChanPost>,
}


#[derive(Deserialize)]
struct FourChanPost {
    no: u64,
    #[serde(rename = "time")]
    time: Option<i64>, // Unix timestamp in seconds
    com: Option<String>,
    sub: Option<String>,
    #[serde(rename = "name")]
    _name: Option<String>,
    #[serde(default)]
    tim: Option<u64>,
    #[serde(default)]
    ext: Option<String>,
    #[serde(default)]
    filename: Option<String>,
    #[serde(default)]
    #[serde(rename = "fsize")]
    _fsize: Option<u64>,
}

pub async fn import_fourchan_thread(state: &AppState, url: &str) -> Result<String> {
    let (board, thread_id) = parse_thread_url(url)?;
    let api_url = format!("https://a.4cdn.org/{}/thread/{}.json", board, thread_id);

    let response = state.http_client
        .get(&api_url)
        .send()
        .await
        .context("failed to fetch thread metadata")?
        .error_for_status()
        .context("4chan API returned an error status")?;

    let thread: FourChanThreadResponse = response.json().await.context("failed to decode thread JSON")?;
    let mut posts_iter = thread.posts.into_iter();
    let first_post = posts_iter.next().context("thread contains no posts")?;

    let thread_service = ThreadService::new(state.database.clone());
    let file_service = FileService::new(
        state.database.clone(),
        state.config.paths.clone(),
        state.config.file.clone(),
        state.blobs.clone(),
    );

    let mut thread_input = CreateThreadInput::default();
    thread_input.title = first_post
        .sub
        .clone()
        .unwrap_or_else(|| format!("Imported /{}/{}", board, thread_id));
    let first_body = clean_body(first_post.com.clone());
    if !first_body.is_empty() {
        thread_input.body = Some(first_body.clone());
    }

    // Set creator as the local peer (GPG fingerprint is the peer ID)
    thread_input.creator_peer_id = Some(state.identity.gpg_fingerprint.clone());

    // Set timestamp from 4chan post
    if let Some(unix_time) = first_post.time {
        use chrono::{DateTime, Utc};
        let dt = DateTime::from_timestamp(unix_time, 0).unwrap_or_else(|| Utc::now());
        thread_input.created_at = Some(dt.to_rfc3339());
    }

    let details = thread_service
        .create_thread(thread_input)
        .context("failed to create thread in backend")?;
    let graph_thread_id = details.thread.id.clone();

    let mut id_map: HashMap<u64, String> = HashMap::new();
    if let Some(created_op) = details.posts.first() {
        id_map.insert(first_post.no, created_op.id.clone());

        // Upload first post's image if it has one
        if let (Some(tim), Some(ext)) = (first_post.tim, first_post.ext.as_ref()) {
            let filename = first_post
                .filename
                .clone()
                .unwrap_or_else(|| format!("{}", tim));
            if let Err(e) =
                download_and_save_image(&file_service, &state, &board, tim, ext, &filename, &created_op.id, &graph_thread_id).await
            {
                tracing::warn!("Failed to upload OP image: {}", e);
            }
        }
    }

    // Import remaining posts directly via service (not API) to avoid broadcasting each post
    for post in posts_iter {
        let body = clean_body(post.com.clone());
        if body.is_empty() && post.tim.is_none() {
            continue; // Skip posts with no text and no image
        }

        let mut payload = CreatePostInput::default();
        payload.thread_id = graph_thread_id.clone();

        // Backend requires non-empty body, use placeholder for image-only posts
        payload.body = if body.is_empty() {
            "[image]".to_string()
        } else {
            body
        };

        // Set author as the local peer (GPG fingerprint is the peer ID)
        payload.author_peer_id = Some(state.identity.gpg_fingerprint.clone());

        payload.parent_post_ids = extract_references(post.com.as_deref(), &id_map);

        // Convert Unix timestamp to ISO format
        if let Some(unix_time) = post.time {
            use chrono::{DateTime, Utc};
            let dt = DateTime::from_timestamp(unix_time, 0).unwrap_or_else(|| Utc::now());
            payload.created_at = Some(dt.to_rfc3339());
        }

        // Create post directly via service (bypasses API broadcast)
        let created = thread_service
            .create_post(payload)
            .with_context(|| format!("failed to create post {}", post.no))?;
        id_map.insert(post.no, created.id.clone());

        // Upload post's image if it has one (still broadcasts FileAvailable individually)
        if let (Some(tim), Some(ext)) = (post.tim, post.ext.as_ref()) {
            let filename = post.filename.clone().unwrap_or_else(|| format!("{}", tim));
            if let Err(e) =
                download_and_save_image(&file_service, &state, &board, tim, ext, &filename, &created.id, &graph_thread_id).await
            {
                tracing::warn!("Failed to upload image for post {}: {}", post.no, e);
            }
        }
    }

    // After importing all posts, broadcast a thread announcement
    let complete_details = thread_service
        .get_thread(&graph_thread_id)
        .context("failed to get imported thread for announcement")?
        .context("imported thread not found")?;

    if let Err(err) = state.network.publish_thread_announcement(
        complete_details,
        &state.identity.gpg_fingerprint
    ).await {
        tracing::warn!(
            error = ?err,
            thread_id = %graph_thread_id,
            "failed to broadcast thread announcement"
        );
    }

    Ok(graph_thread_id)
}

async fn download_and_save_image(
    file_service: &FileService,
    state: &AppState,
    board: &str,
    tim: u64,
    ext: &str,
    filename: &str,
    post_id: &str,
    thread_id: &str,
) -> Result<()> {
    // 4chan image URL format: https://i.4cdn.org/{board}/{tim}{ext}
    let image_url = format!("https://i.4cdn.org/{}/{}{}", board, tim, ext);

    tracing::info!("Downloading image: {}", image_url);
    
    // Add delay to avoid hitting 4chan's rate limit (429 errors)
    // Increased to 1500ms to be safer
    sleep(Duration::from_millis(1500)).await;

    let response = state.http_client
        .get(&image_url)
        .send()
        .await
        .context("failed to download image from 4chan")?;

    if !response.status().is_success() {
        anyhow::bail!("failed to download image: status {}", response.status());
    }

    let bytes = response.bytes().await.context("failed to read image bytes")?;

    // Determine MIME type from extension
    let mime = match ext {
        ".jpg" | ".jpeg" => "image/jpeg",
        ".png" => "image/png",
        ".gif" => "image/gif",
        ".webm" => "video/webm",
        _ => "application/octet-stream",
    };

    let save_input = SaveFileInput {
        post_id: post_id.to_string(),
        original_name: Some(format!("{}{}", filename, ext)),
        mime: Some(mime.to_string()),
        data: bytes.to_vec(),
    };

    let mut file_view = file_service.save_post_file(save_input).await?;
    
    // Publish file availability
    let ticket = file_view
        .blob_id
        .as_deref()
        .and_then(|blob| state.network.make_blob_ticket(blob));
    file_view.ticket = ticket.as_ref().map(|t: &iroh_blobs::ticket::BlobTicket| t.to_string());
    
    let announcement = crate::network::FileAnnouncement {
        id: file_view.id.clone(),
        post_id: file_view.post_id.clone(),
        thread_id: thread_id.to_string(),
        original_name: file_view.original_name.clone(),
        mime: file_view.mime.clone(),
        size_bytes: file_view.size_bytes,
        checksum: file_view.checksum.clone(),
        blob_id: file_view.blob_id.clone(),
        ticket: ticket.clone(),
    };
    
    if let Err(err) = file_service.persist_ticket(&file_view.id, ticket.as_ref()) {
        tracing::warn!(error = ?err, file_id = %file_view.id, "failed to persist blob ticket");
    }
    
    if let Err(err) = state.network.publish_file_available(announcement).await {
        tracing::warn!(
            error = ?err,
            post_id = %post_id,
            file_id = %file_view.id,
            "failed to publish file availability over network"
        );
    }

    tracing::info!("Saved image {} for post {}", filename, post_id);
    Ok(())
}

fn clean_body(html: Option<String>) -> String {
    html.map(|raw| {
        let text = from_read(raw.as_bytes(), 120);
        text.trim().replace("\u{00a0}", " ")
    })
    .unwrap_or_default()
}

fn extract_references(html: Option<&str>, id_map: &HashMap<u64, String>) -> Vec<String> {
    let Some(content) = html else {
        return Vec::new();
    };
    let normalized = content
        .replace("&gt;", ">")
        .replace("&#62;", ">")
        .replace("&nbsp;", " ");
    let re = Regex::new(r">>\s*(\d+)").unwrap();
    let mut refs = Vec::new();
    for capture in re.captures_iter(&normalized) {
        if let Some(matched) = capture.get(1) {
            if let Ok(ref_no) = matched.as_str().parse::<u64>() {
                if let Some(mapped) = id_map.get(&ref_no) {
                    refs.push(mapped.clone());
                } else {
                    tracing::debug!("Reference >>{} not yet mapped", ref_no);
                }
            }
        }
    }
    refs.sort();
    refs.dedup();
    refs
}

fn parse_thread_url(url: &str) -> Result<(String, String)> {
    let pattern = Regex::new(r"https?://boards\.4chan\.org/([a-z0-9]+)/thread/(\d+)").unwrap();
    let captures = pattern
        .captures(url)
        .context("unable to parse 4chan thread URL")?;
    let board = captures.get(1).unwrap().as_str().to_string();
    let thread_id = captures.get(2).unwrap().as_str().to_string();
    Ok((board, thread_id))
}

pub async fn import_reddit_thread(state: &AppState, url: &str) -> Result<String> {
    // 1. Validate and format URL
    // Ensure it ends with .json
    let json_url = if url.contains("?") {
        let parts: Vec<&str> = url.split('?').collect();
        format!("{}.json?{}", parts[0], parts[1])
    } else {
        format!("{}.json", url)
    };

    tracing::info!("Importing Reddit thread: {}", json_url);

    // 2. Fetch Data
    let client = &state.http_client;
    let response = client.get(&json_url)
        .header("User-Agent", "Graphchan/0.1.0") // Reddit requires a User-Agent
        .send()
        .await
        .context("failed to fetch reddit thread")?;

    if !response.status().is_success() {
        anyhow::bail!("Reddit API error: {}", response.status());
    }

    let json: serde_json::Value = response.json().await.context("failed to parse Reddit JSON")?;

    // Reddit returns an array of two listings: [Thread Listing, Comment Listing]
    let listings = json.as_array().ok_or_else(|| anyhow::anyhow!("Invalid Reddit response format"))?;
    if listings.len() < 2 {
        anyhow::bail!("Incomplete Reddit response");
    }

    let thread_data = &listings[0]["data"]["children"][0]["data"];
    let comments_data = &listings[1]["data"]["children"];

    // 3. Create Thread
    let thread_service = ThreadService::new(state.database.clone());
    let mut thread_input = CreateThreadInput::default();
    
    let title = thread_data["title"].as_str().unwrap_or("Untitled Reddit Thread");
    let selftext = thread_data["selftext"].as_str().unwrap_or("");
    let author = thread_data["author"].as_str().unwrap_or("unknown");
    let subreddit = thread_data["subreddit"].as_str().unwrap_or("unknown");
    
    thread_input.title = format!("[r/{}] {}", subreddit, title);
    thread_input.body = Some(format!("**Author:** u/{}\n\n{}", author, selftext));
    thread_input.creator_peer_id = Some(state.identity.gpg_fingerprint.clone());
    
    if let Some(created_utc) = thread_data["created_utc"].as_f64() {
        let dt = chrono::DateTime::from_timestamp(created_utc as i64, 0).unwrap_or_default();
        thread_input.created_at = Some(dt.to_rfc3339());
    }

    let details = thread_service
        .create_thread(thread_input)
        .context("failed to create thread in backend")?;
    let graph_thread_id = details.thread.id.clone();

    // Handle OP Image/Media if present
    if let Some(url) = thread_data["url"].as_str() {
        if url.ends_with(".jpg") || url.ends_with(".png") || url.ends_with(".gif") || url.ends_with(".jpeg") {
             // Download image
             let file_service = FileService::new(
                state.database.clone(),
                state.config.paths.clone(),
                state.config.file.clone(),
                state.blobs.clone(),
            );
            if let Err(e) = download_and_save_reddit_image(&file_service, state, url, &details.posts[0].id, &graph_thread_id).await {
                 tracing::warn!("Failed to download Reddit OP image: {}", e);
            }
        }
    }

    // 4. Process Comments (Recursive)
    let mut comment_queue = Vec::new();
    // (parent_graph_id, comment_json)
    let empty_vec = vec![];
    for child in comments_data.as_array().unwrap_or(&empty_vec) {
        comment_queue.push((details.posts[0].id.clone(), child));
    }

    while let Some((parent_id, child)) = comment_queue.pop() {
        let data = &child["data"];
        
        // Skip "more" objects (pagination)
        if child["kind"].as_str() == Some("more") {
            continue;
        }

        let body = data["body"].as_str().unwrap_or("[deleted]");
        let author = data["author"].as_str().unwrap_or("[deleted]");
        
        let mut payload = CreatePostInput::default();
        payload.thread_id = graph_thread_id.clone();
        payload.body = format!("**u/{}**\n\n{}", author, body);
        payload.author_peer_id = Some(state.identity.gpg_fingerprint.clone());
        payload.parent_post_ids = vec![parent_id.clone()];

        if let Some(created_utc) = data["created_utc"].as_f64() {
             let dt = chrono::DateTime::from_timestamp(created_utc as i64, 0).unwrap_or_default();
             payload.created_at = Some(dt.to_rfc3339());
        }

        if let Ok(created) = thread_service.create_post(payload) {
            // Process replies
            if let Some(replies) = data["replies"].as_object() {
                if let Some(children) = replies["data"]["children"].as_array() {
                    for reply in children {
                        comment_queue.push((created.id.clone(), reply));
                    }
                }
            }
        }
    }

    // Broadcast
    let complete_details = thread_service.get_thread(&graph_thread_id)?.context("thread not found")?;
    if let Err(err) = state.network.publish_thread_announcement(
        complete_details,
        &state.identity.gpg_fingerprint
    ).await {
        tracing::warn!("failed to broadcast thread announcement: {}", err);
    }

    Ok(graph_thread_id)
}

async fn download_and_save_reddit_image(
    file_service: &FileService,
    state: &AppState,
    url: &str,
    post_id: &str,
    thread_id: &str,
) -> Result<()> {
    let response = state.http_client.get(url).send().await?;
    let bytes = response.bytes().await?;
    
    let filename = url.split('/').last().unwrap_or("image.jpg");
    // Remove query params if any
    let filename = filename.split('?').next().unwrap_or("image.jpg");
    
    let mime = if filename.ends_with(".png") { "image/png" } 
               else if filename.ends_with(".gif") { "image/gif" }
               else { "image/jpeg" };

    let save_input = SaveFileInput {
        post_id: post_id.to_string(),
        original_name: Some(filename.to_string()),
        mime: Some(mime.to_string()),
        data: bytes.to_vec(),
    };

    let mut file_view = file_service.save_post_file(save_input).await?;
    
    // Publish file availability
    let ticket = file_view
        .blob_id
        .as_deref()
        .and_then(|blob| state.network.make_blob_ticket(blob));
    file_view.ticket = ticket.as_ref().map(|t: &iroh_blobs::ticket::BlobTicket| t.to_string());
    
    let announcement = crate::network::FileAnnouncement {
        id: file_view.id.clone(),
        post_id: file_view.post_id.clone(),
        thread_id: thread_id.to_string(),
        original_name: file_view.original_name.clone(),
        mime: file_view.mime.clone(),
        size_bytes: file_view.size_bytes,
        checksum: file_view.checksum.clone(),
        blob_id: file_view.blob_id.clone(),
        ticket: ticket.clone(),
    };
    
    if let Err(err) = file_service.persist_ticket(&file_view.id, ticket.as_ref()) {
        tracing::warn!("failed to persist blob ticket: {}", err);
    }
    
    if let Err(err) = state.network.publish_file_available(announcement).await {
        tracing::warn!("failed to publish file availability: {}", err);
    }

    Ok(())
}
