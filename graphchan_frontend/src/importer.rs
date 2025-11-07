use std::collections::HashMap;

use anyhow::{Context, Result};
use html2text::from_read;
use regex::Regex;
use reqwest::blocking::Client;
use serde::Deserialize;

use crate::api::ApiClient;
use crate::models::{CreatePostInput, CreateThreadInput};

#[derive(Deserialize)]
struct FourChanThreadResponse {
    posts: Vec<FourChanPost>,
}

#[derive(Deserialize)]
struct FourChanPost {
    no: u64,
    time: Option<i64>,
    com: Option<String>,
    sub: Option<String>,
    name: Option<String>,
    #[serde(default)]
    tim: Option<u64>,
    #[serde(default)]
    ext: Option<String>,
    #[serde(default)]
    filename: Option<String>,
    #[serde(default)]
    fsize: Option<u64>,
}

pub fn import_fourchan_thread(api: &ApiClient, url: &str) -> Result<String> {
    let (board, thread_id) = parse_thread_url(url)?;
    let api_url = format!("https://a.4cdn.org/{}/thread/{}.json", board, thread_id);

    let client = Client::builder()
        .user_agent("GraphchanFrontend/0.1")
        .build()
        .context("failed to build HTTP client")?;

    let response = client
        .get(api_url)
        .send()
        .context("failed to fetch thread metadata")?
        .error_for_status()
        .context("4chan API returned an error status")?;

    let thread: FourChanThreadResponse = response.json().context("failed to decode thread JSON")?;
    let mut posts_iter = thread.posts.into_iter();
    let first_post = posts_iter.next().context("thread contains no posts")?;

    let mut thread_input = CreateThreadInput::default();
    thread_input.title = first_post
        .sub
        .clone()
        .unwrap_or_else(|| format!("Imported /{}/{}", board, thread_id));
    let first_body = clean_body(first_post.com.clone());
    if !first_body.is_empty() {
        thread_input.body = Some(first_body.clone());
    }

    let details = api
        .create_thread(&thread_input)
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
                upload_4chan_image(api, &board, tim, ext, &filename, &created_op.id, &client)
            {
                log::warn!("Failed to upload OP image: {}", e);
            }
        }
    }

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

        payload.parent_post_ids = extract_references(post.com.as_deref(), &id_map);

        let created = api
            .create_post(&graph_thread_id, &payload)
            .with_context(|| format!("failed to create post {}", post.no))?;
        id_map.insert(post.no, created.id.clone());

        // Upload post's image if it has one
        if let (Some(tim), Some(ext)) = (post.tim, post.ext.as_ref()) {
            let filename = post.filename.clone().unwrap_or_else(|| format!("{}", tim));
            if let Err(e) =
                upload_4chan_image(api, &board, tim, ext, &filename, &created.id, &client)
            {
                log::warn!("Failed to upload image for post {}: {}", post.no, e);
            }
        }
    }

    Ok(graph_thread_id)
}

fn upload_4chan_image(
    api: &ApiClient,
    board: &str,
    tim: u64,
    ext: &str,
    filename: &str,
    post_id: &str,
    client: &Client,
) -> Result<()> {
    // 4chan image URL format: https://i.4cdn.org/{board}/{tim}{ext}
    let image_url = format!("https://i.4cdn.org/{}/{}{}", board, tim, ext);

    log::info!("Downloading image: {}", image_url);

    let response = client
        .get(&image_url)
        .send()
        .context("failed to fetch image")?
        .error_for_status()
        .context("image download failed")?;

    let bytes = response.bytes().context("failed to read image bytes")?;

    // Determine MIME type from extension
    let mime = match ext {
        ".jpg" | ".jpeg" => "image/jpeg",
        ".png" => "image/png",
        ".gif" => "image/gif",
        ".webm" => "video/webm",
        _ => "application/octet-stream",
    };

    // Create multipart form
    let form = reqwest::blocking::multipart::Form::new().part(
        "file",
        reqwest::blocking::multipart::Part::bytes(bytes.to_vec())
            .file_name(format!("{}{}", filename, ext))
            .mime_str(mime)?,
    );

    // Upload to backend
    let upload_url = format!("{}/posts/{}/files", api.base_url(), post_id);
    let upload_client = Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .context("failed to build upload client")?;

    upload_client
        .post(&upload_url)
        .multipart(form)
        .send()
        .context("failed to upload file")?
        .error_for_status()
        .context("file upload failed")?;

    log::info!("Uploaded image {} for post {}", filename, post_id);
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
                    log::debug!("Reference >>{} not yet mapped", ref_no);
                }
            }
        }
    }
    refs.sort();
    refs.dedup();
    if !refs.is_empty() {
        log::debug!(
            "Resolved references {:?} from raw reply snippet: {:?}",
            refs,
            content
        );
    }
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
