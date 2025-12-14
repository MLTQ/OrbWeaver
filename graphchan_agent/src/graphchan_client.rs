use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

#[derive(Clone)]
pub struct GraphchanClient {
    base_url: String,
    client: reqwest::Client,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Thread {
    pub id: String,
    pub title: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Post {
    pub id: String,
    pub thread_id: String,
    pub author_peer_id: Option<String>,
    pub body: String,
    pub created_at: String,
    pub parent_post_id: Option<String>,
    pub reply_to_post_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThreadDetails {
    pub thread: Thread,
    pub posts: Vec<Post>,
}

#[derive(Debug, Serialize)]
pub struct CreatePostRequest {
    pub thread_id: String,
    pub body: String,
    #[serde(default)]
    pub parent_post_ids: Vec<String>,
}

impl GraphchanClient {
    pub fn new(base_url: String) -> Self {
        Self {
            base_url,
            client: reqwest::Client::new(),
        }
    }

    /// List all threads
    pub async fn list_threads(&self) -> Result<Vec<Thread>> {
        let url = format!("{}/threads", self.base_url);
        let response = self
            .client
            .get(&url)
            .send()
            .await
            .context("Failed to fetch threads")?
            .error_for_status()
            .context("API returned error for threads list")?;

        let threads = response
            .json()
            .await
            .context("Failed to parse threads response")?;

        Ok(threads)
    }

    /// Get thread details with all posts
    pub async fn get_thread(&self, thread_id: &str) -> Result<ThreadDetails> {
        let url = format!("{}/threads/{}", self.base_url, thread_id);
        let response = self
            .client
            .get(&url)
            .send()
            .await
            .with_context(|| format!("Failed to fetch thread {}", thread_id))?
            .error_for_status()
            .with_context(|| format!("API returned error for thread {}", thread_id))?;

        let details = response
            .json()
            .await
            .context("Failed to parse thread details")?;

        Ok(details)
    }

    /// Create a new post in a thread
    pub async fn create_post(
        &self,
        thread_id: &str,
        body: String,
        parent_post_ids: Vec<String>,
    ) -> Result<Post> {
        let url = format!("{}/threads/{}/posts", self.base_url, thread_id);
        let request = CreatePostRequest {
            thread_id: thread_id.to_string(),
            body,
            parent_post_ids,
        };

        let response = self
            .client
            .post(&url)
            .json(&request)
            .send()
            .await
            .with_context(|| format!("Failed to create post in thread {}", thread_id))?;

        // Check for errors and include response body
        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_else(|_| "Unable to read body".to_string());
            anyhow::bail!(
                "API returned error creating post in thread {}: {} - {}",
                thread_id,
                status,
                body
            );
        }

        // Backend returns { "post": { ... } }
        #[derive(Deserialize)]
        struct PostResponse {
            post: Post,
        }

        let response_data: PostResponse = response.json().await.context("Failed to parse post response")?;

        Ok(response_data.post)
    }
}
