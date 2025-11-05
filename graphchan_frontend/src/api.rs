use std::time::Duration;

use anyhow::{Context, Result};
use reqwest::blocking::Client;
use reqwest::Url;

use crate::models::{
    CreatePostInput, CreateThreadInput, FileResponse, PostResponse, PostView, ThreadDetails,
    ThreadSummary,
};

#[derive(Clone)]
pub struct ApiClient {
    base_url: String,
    client: Client,
}

impl ApiClient {
    pub fn new(base_url: impl Into<String>) -> Result<Self> {
        let base = sanitize_base_url(base_url.into())?;
        let client = Client::builder()
            .timeout(Duration::from_secs(15))
            .build()
            .context("failed to build HTTP client")?;
        Ok(Self {
            base_url: base,
            client,
        })
    }

    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    pub fn set_base_url(&mut self, base_url: impl Into<String>) -> Result<()> {
        self.base_url = sanitize_base_url(base_url.into())?;
        Ok(())
    }

    pub fn list_threads(&self) -> Result<Vec<ThreadSummary>> {
        let url = self.url("/threads")?;
        let response = self.client.get(url).send()?.error_for_status()?;
        Ok(response.json()?)
    }

    pub fn get_thread(&self, thread_id: &str) -> Result<ThreadDetails> {
        let url = self.url(&format!("/threads/{thread_id}"))?;
        let response = self.client.get(url).send()?.error_for_status()?;
        Ok(response.json()?)
    }

    pub fn create_thread(&self, input: &CreateThreadInput) -> Result<ThreadDetails> {
        let url = self.url("/threads")?;
        let response = self
            .client
            .post(url)
            .json(input)
            .send()?
            .error_for_status()?;
        Ok(response.json()?)
    }

    pub fn create_post(&self, thread_id: &str, input: &CreatePostInput) -> Result<PostView> {
        let mut payload = input.clone();
        payload.thread_id = thread_id.to_string();
        let url = self.url(&format!("/threads/{thread_id}/posts"))?;
        let response = self
            .client
            .post(url)
            .json(&payload)
            .send()?
            .error_for_status()?;
        let wrapper: PostResponse = response.json()?;
        Ok(wrapper.post)
    }

    pub fn list_post_files(&self, post_id: &str) -> Result<Vec<FileResponse>> {
        let url = self.url(&format!("/posts/{post_id}/files"))?;
        let response = self.client.get(url).send()?.error_for_status()?;
        Ok(response.json()?)
    }

    pub fn download_url(&self, file_id: &str) -> String {
        format!("{}/files/{}", self.base_url, file_id)
    }

    fn url(&self, path: &str) -> Result<Url> {
        let mut url = Url::parse(&self.base_url).context("invalid base URL")?;
        url.set_path(path.trim_start_matches('/'));
        Ok(url)
    }
}

fn sanitize_base_url(mut base: String) -> Result<String> {
    if !base.starts_with("http://") && !base.starts_with("https://") {
        base = format!("http://{base}");
    }
    // Remove trailing slash for consistency
    while base.ends_with('/') {
        base.pop();
    }
    // Validate once
    let _ = Url::parse(&base).context("invalid base URL")?;
    Ok(base)
}
