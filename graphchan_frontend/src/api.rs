use std::sync::OnceLock;
use std::time::Duration;

use anyhow::{Context, Result};
use reqwest::blocking::Client;
use reqwest::Url;

use crate::models::{
    AddPeerRequest, CreatePostInput, CreateThreadInput, FileResponse, PeerView, PostResponse,
    PostView, ReactionsResponse, ThreadDetails, ThreadSummary,
};

static SHARED_CLIENT: OnceLock<Client> = OnceLock::new();

pub fn get_shared_client() -> Result<&'static Client> {
    if let Some(client) = SHARED_CLIENT.get() {
        return Ok(client);
    }

    let client = Client::builder()
        .timeout(Duration::from_secs(30))
        .user_agent("GraphchanFrontend/0.1")
        .build()
        .context("failed to build HTTP client")?;

    let _ = SHARED_CLIENT.set(client);
    Ok(SHARED_CLIENT.get().unwrap())
}

#[derive(Clone)]
pub struct ApiClient {
    base_url: String,
    client: Client,
}

impl ApiClient {
    pub fn new(base_url: impl Into<String>) -> Result<Self> {
        let base = sanitize_base_url(base_url.into())?;
        let client = get_shared_client()?.clone();
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

    pub fn download_thread(&self, thread_id: &str) -> Result<ThreadDetails> {
        let url = self.url(&format!("/threads/{thread_id}/download"))?;
        let response = self.client.post(url).send()?.error_for_status()?;
        Ok(response.json()?)
    }

    pub fn create_thread(&self, input: &CreateThreadInput, files: &[std::path::PathBuf]) -> Result<ThreadDetails> {
        let url = self.url("/threads")?;
        let mut form = reqwest::blocking::multipart::Form::new();
        
        let json = serde_json::to_string(input)?;
        form = form.part("json", reqwest::blocking::multipart::Part::text(json).mime_str("application/json")?);
        
        for path in files {
             form = form.file("file", path)?;
        }

        let response = self
            .client
            .post(url)
            .multipart(form)
            .send()?
            .error_for_status()?;
        Ok(response.json()?)
    }

    pub fn get_self_peer(&self) -> Result<PeerView> {
        let url = self.url("/peers/self")?;
        let response = self.client.get(url).send()?.error_for_status()?;
        Ok(response.json()?)
    }

    pub fn list_peers(&self) -> Result<Vec<PeerView>> {
        let url = self.url("/peers")?;
        let response = self.client.get(url).send()?.error_for_status()?;
        Ok(response.json()?)
    }

    pub fn add_peer(&self, friendcode: &str) -> Result<PeerView> {
        let url = self.url("/peers")?;
        let request = AddPeerRequest {
            friendcode: friendcode.to_string(),
        };
        let response = self.client.post(url).json(&request).send()?.error_for_status()?;
        Ok(response.json()?)
    }

    pub fn upload_avatar(&self, path: &std::path::Path) -> Result<()> {
        let url = self.url("/identity/avatar")?;
        let form = reqwest::blocking::multipart::Form::new()
            .file("file", path)?;
        self.client
            .post(url)
            .multipart(form)
            .send()?
            .error_for_status()?;
        Ok(())
    }

    pub fn update_profile(&self, username: Option<String>, bio: Option<String>) -> Result<()> {
        let url = self.url("/identity/profile")?;
        let input = crate::models::UpdateProfileRequest { username, bio };
        self.client.post(url).json(&input).send()?.error_for_status()?;
        Ok(())
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

    pub fn upload_file(&self, post_id: &str, path: &std::path::Path) -> Result<FileResponse> {
        let url = self.url(&format!("/posts/{post_id}/files"))?;
        let form = reqwest::blocking::multipart::Form::new()
            .file("file", path)?;
            
        let response = self.client.post(url)
            .multipart(form)
            .send()?
            .error_for_status()?;
            
        Ok(response.json()?)
    }

    pub fn download_url(&self, file_id: &str) -> String {
        format!("{}/files/{}", self.base_url, file_id)
    }

    pub fn import_thread(&self, url: &str) -> Result<String> {
        let api_url = self.url("/import")?;
        #[derive(serde::Serialize)]
        struct ImportRequest {
            url: String,
        }
        #[derive(serde::Deserialize)]
        struct ImportResponse {
            id: String,
        }

        let request = ImportRequest {
            url: url.to_string(),
        };
        let response = self
            .client
            .post(api_url)
            .json(&request)
            .timeout(Duration::from_secs(300)) // 5 minute timeout for large imports
            .send()?
            .error_for_status()?;
        let wrapper: ImportResponse = response.json()?;
        Ok(wrapper.id)
    }

    pub fn post_json<T: serde::Serialize>(&self, path: &str, json: &T) -> Result<reqwest::blocking::Response> {
        let url = self.url(path)?;
        let response = self.client.post(url).json(json).send()?.error_for_status()?;
        Ok(response)
    }

    pub fn delete_thread(&self, thread_id: &str) -> Result<()> {
        let url = format!("{}/threads/{}/delete", self.base_url(), thread_id);
        self.client.post(&url).send()?.error_for_status()?;
        Ok(())
    }

    pub fn set_thread_ignored(&self, thread_id: &str, ignored: bool) -> Result<()> {
        let url = format!("{}/threads/{}/ignore", self.base_url(), thread_id);
        let payload = serde_json::json!({ "ignored": ignored });
        self.client.post(&url).json(&payload).send()?.error_for_status()?;
        Ok(())
    }

    pub fn unfollow_peer(&self, peer_id: &str) -> Result<()> {
        let url = format!("{}/peers/{}/unfollow", self.base_url(), peer_id);
        self.client.post(&url).send()?.error_for_status()?;
        Ok(())
    }

    pub fn add_reaction(&self, post_id: &str, emoji: &str) -> Result<()> {
        let url = format!("{}/posts/{}/react", self.base_url(), post_id);
        let payload = serde_json::json!({ "emoji": emoji });
        self.client.post(&url).json(&payload).send()?.error_for_status()?;
        Ok(())
    }

    pub fn remove_reaction(&self, post_id: &str, emoji: &str) -> Result<()> {
        let url = format!("{}/posts/{}/unreact", self.base_url(), post_id);
        let payload = serde_json::json!({ "emoji": emoji });
        self.client.post(&url).json(&payload).send()?.error_for_status()?;
        Ok(())
    }

    pub fn get_reactions(&self, post_id: &str) -> Result<ReactionsResponse> {
        let url = format!("{}/posts/{}/reactions", self.base_url(), post_id);
        let response = self.client.get(&url).send()?.error_for_status()?;
        Ok(response.json()?)
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
