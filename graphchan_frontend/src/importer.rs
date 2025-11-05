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
    }

    for post in posts_iter {
        let body = clean_body(post.com.clone());
        if body.is_empty() {
            continue;
        }
        let mut payload = CreatePostInput::default();
        payload.thread_id = graph_thread_id.clone();
        payload.body = body;
        payload.parent_post_ids = extract_references(post.com.as_deref(), &id_map);

        let created = api
            .create_post(&graph_thread_id, &payload)
            .with_context(|| format!("failed to create post {}", post.no))?;
        id_map.insert(post.no, created.id);
    }

    Ok(graph_thread_id)
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
    let re = Regex::new(r">>\s*(\d+)").unwrap();
    let mut refs = Vec::new();
    for capture in re.captures_iter(content) {
        if let Some(matched) = capture.get(1) {
            if let Ok(ref_no) = matched.as_str().parse::<u64>() {
                if let Some(mapped) = id_map.get(&ref_no) {
                    refs.push(mapped.clone());
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
