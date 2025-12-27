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

    /// Delete a thread
    pub async fn delete_thread(&self, thread_id: &str) -> Result<()> {
        let url = format!("{}/threads/{}", self.base_url, thread_id);
        let response = self
            .client
            .delete(&url)
            .send()
            .await
            .with_context(|| format!("Failed to delete thread {}", thread_id))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_else(|_| "Unable to read body".to_string());
            anyhow::bail!("Failed to delete thread {}: {} - {}", thread_id, status, body);
        }

        Ok(())
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

    /// Upload a file to a post
    pub async fn upload_file(
        &self,
        post_id: &str,
        filename: &str,
        mime_type: &str,
        file_data: Vec<u8>,
    ) -> Result<FileUploadResponse> {
        let url = format!("{}/posts/{}/files", self.base_url, post_id);

        // Create multipart form
        let file_part = reqwest::multipart::Part::bytes(file_data)
            .file_name(filename.to_string())
            .mime_str(mime_type)
            .context("Invalid MIME type")?;

        let form = reqwest::multipart::Form::new()
            .part("file", file_part);

        let response = self.client
            .post(&url)
            .multipart(form)
            .send()
            .await
            .with_context(|| format!("Failed to upload file to post {}", post_id))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_else(|_| "Unable to read body".to_string());
            anyhow::bail!(
                "API returned error uploading file to post {}: {} - {}",
                post_id,
                status,
                body
            );
        }

        let file_response: FileUploadResponse = response
            .json()
            .await
            .context("Failed to parse file upload response")?;

        Ok(file_response)
    }
}

#[derive(Debug, Deserialize)]
pub struct FileUploadResponse {
    pub id: String,
    pub post_id: String,
    pub original_name: Option<String>,
    pub mime_type: Option<String>,
    pub size_bytes: Option<i64>,
    pub blob_id: Option<String>,
}
