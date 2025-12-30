use anyhow::{Context, Result};
use reqwest::Client;

use crate::models::*;

#[derive(Clone)]
pub struct GraphchanClient {
    base_url: String,
    client: Client,
}

impl GraphchanClient {
    pub fn new(base_url: String) -> Self {
        Self {
            base_url,
            client: Client::new(),
        }
    }

    pub async fn list_threads(&self) -> Result<Vec<ThreadSummary>> {
        let url = format!("{}/threads", self.base_url);
        let response = self.client.get(&url).send().await?;
        let threads = response.json().await?;
        Ok(threads)
    }

    pub async fn get_thread(&self, thread_id: &str) -> Result<ThreadDetails> {
        let url = format!("{}/threads/{}", self.base_url, thread_id);
        let response = self.client.get(&url).send().await?;
        let thread = response.json().await?;
        Ok(thread)
    }

    pub async fn get_recent_posts(&self, limit: usize) -> Result<RecentPostsResponse> {
        let url = format!("{}/posts/recent?limit={}", self.base_url, limit);
        let response = self.client.get(&url).send().await?;
        let posts = response.json().await?;
        Ok(posts)
    }

    pub async fn create_post(&self, input: CreatePostInput) -> Result<PostView> {
        let url = format!("{}/posts", self.base_url);
        let response = self.client.post(&url).json(&input).send().await?;
        
        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            anyhow::bail!("Failed to create post: {} - {}", status, body);
        }
        
        let post = response.json().await?;
        Ok(post)
    }

    pub async fn health_check(&self) -> Result<()> {
        let url = format!("{}/threads", self.base_url);
        self.client.get(&url).send().await
            .context("Failed to connect to graphchan API")?;
        Ok(())
    }
}
