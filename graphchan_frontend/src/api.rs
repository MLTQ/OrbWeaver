use std::sync::OnceLock;
use std::time::Duration;

use anyhow::{Context, Result};
use reqwest::blocking::Client;
use reqwest::Url;

use crate::models::{
    AddPeerRequest, BlockedPeerView, BlocklistEntryView, BlocklistSubscriptionView,
    BlockPeerRequest, ConversationView, CreatePostInput, CreateThreadInput, DirectMessageView,
    FileResponse, PeerView, PostResponse, PostView, ReactionsResponse, SearchResponse, SendDmRequest,
    SubscribeBlocklistRequest, ThreadDetails, ThreadSummary, UnreadCountResponse,
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

    pub fn list_recent_posts(&self, limit: Option<usize>) -> Result<crate::models::RecentPostsResponse> {
        let mut url = self.url("/posts/recent")?;
        if let Some(lim) = limit {
            url.set_query(Some(&format!("limit={}", lim)));
        }
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

    // Direct Message methods

    pub fn list_conversations(&self) -> Result<Vec<ConversationView>> {
        let url = self.url("/dms/conversations")?;
        let response = self.client.get(url).send()?.error_for_status()?;
        Ok(response.json()?)
    }

    pub fn send_dm(&self, to_peer_id: &str, body: &str) -> Result<DirectMessageView> {
        let url = self.url("/dms/send")?;
        let request = SendDmRequest {
            to_peer_id: to_peer_id.to_string(),
            body: body.to_string(),
        };
        let response = self.client.post(url).json(&request).send()?.error_for_status()?;
        Ok(response.json()?)
    }

    pub fn get_messages(&self, peer_id: &str, limit: usize) -> Result<Vec<DirectMessageView>> {
        let url = format!("{}/dms/{}/messages?limit={}", self.base_url(), peer_id, limit);
        let response = self.client.get(&url).send()?.error_for_status()?;
        Ok(response.json()?)
    }

    pub fn mark_message_read(&self, message_id: &str) -> Result<()> {
        let url = format!("{}/dms/messages/{}/read", self.base_url(), message_id);
        self.client.post(&url).send()?.error_for_status()?;
        Ok(())
    }

    pub fn get_unread_count(&self) -> Result<UnreadCountResponse> {
        let url = self.url("/dms/unread/count")?;
        let response = self.client.get(url).send()?.error_for_status()?;
        Ok(response.json()?)
    }

    // Blocking and Moderation methods

    pub fn list_blocked_peers(&self) -> Result<Vec<BlockedPeerView>> {
        let url = self.url("/blocking/peers")?;
        let response = self.client.get(url).send()?.error_for_status()?;
        Ok(response.json()?)
    }

    pub fn block_peer(&self, peer_id: &str, reason: Option<String>) -> Result<BlockedPeerView> {
        let url = format!("{}/blocking/peers/{}", self.base_url(), peer_id);
        let request = BlockPeerRequest { reason };
        let response = self.client.post(&url).json(&request).send()?.error_for_status()?;
        Ok(response.json()?)
    }

    pub fn unblock_peer(&self, peer_id: &str) -> Result<()> {
        let url = format!("{}/blocking/peers/{}", self.base_url(), peer_id);
        self.client.delete(&url).send()?.error_for_status()?;
        Ok(())
    }

    pub fn list_blocklists(&self) -> Result<Vec<BlocklistSubscriptionView>> {
        let url = self.url("/blocking/blocklists")?;
        let response = self.client.get(url).send()?.error_for_status()?;
        Ok(response.json()?)
    }

    pub fn subscribe_blocklist(&self, request: &SubscribeBlocklistRequest) -> Result<BlocklistSubscriptionView> {
        let url = self.url("/blocking/blocklists")?;
        let response = self.client.post(url).json(request).send()?.error_for_status()?;
        Ok(response.json()?)
    }

    pub fn unsubscribe_blocklist(&self, blocklist_id: &str) -> Result<()> {
        let url = format!("{}/blocking/blocklists/{}", self.base_url(), blocklist_id);
        self.client.delete(&url).send()?.error_for_status()?;
        Ok(())
    }

    pub fn list_blocklist_entries(&self, blocklist_id: &str) -> Result<Vec<BlocklistEntryView>> {
        let url = format!("{}/blocking/blocklists/{}/entries", self.base_url(), blocklist_id);
        let response = self.client.get(&url).send()?.error_for_status()?;
        Ok(response.json()?)
    }

    // IP Blocking methods

    pub fn list_ip_blocks(&self) -> Result<Vec<crate::models::IpBlockView>> {
        let url = self.url("/blocking/ips")?;
        let response = self.client.get(url).send()?.error_for_status()?;
        Ok(response.json()?)
    }

    pub fn get_ip_block_stats(&self) -> Result<crate::models::IpBlockStatsResponse> {
        let url = self.url("/blocking/ips/stats")?;
        let response = self.client.get(url).send()?.error_for_status()?;
        Ok(response.json()?)
    }

    pub fn add_ip_block(&self, ip_or_range: &str, reason: Option<String>) -> Result<()> {
        let url = self.url("/blocking/ips")?;
        let request = crate::models::AddIpBlockRequest {
            ip_or_range: ip_or_range.to_string(),
            reason,
        };
        self.client.post(url).json(&request).send()?.error_for_status()?;
        Ok(())
    }

    pub fn remove_ip_block(&self, block_id: i64) -> Result<()> {
        let url = format!("{}/blocking/ips/{}", self.base_url(), block_id);
        self.client.delete(&url).send()?.error_for_status()?;
        Ok(())
    }

    pub fn import_ip_blocks(&self, import_text: &str) -> Result<()> {
        let url = self.url("/blocking/ips/import")?;
        self.client.post(url).body(import_text.to_string()).send()?.error_for_status()?;
        Ok(())
    }

    pub fn export_ip_blocks(&self) -> Result<String> {
        let url = self.url("/blocking/ips/export")?;
        let response = self.client.get(url).send()?.error_for_status()?;
        Ok(response.text()?)
    }

    pub fn clear_all_ip_blocks(&self) -> Result<()> {
        let url = self.url("/blocking/ips/clear")?;
        self.client.post(url).send()?.error_for_status()?;
        Ok(())
    }

    pub fn get_peer_ips(&self, peer_id: &str) -> Result<crate::models::PeerIpResponse> {
        let url = format!("{}/peers/{}/ip", self.base_url(), peer_id);
        let response = self.client.get(&url).send()?.error_for_status()?;
        Ok(response.json()?)
    }

    pub fn search(&self, query: &str, limit: Option<usize>) -> Result<SearchResponse> {
        let limit_param = limit.unwrap_or(50);
        let url = format!("{}/search", self.base_url());
        let response = self.client
            .get(&url)
            .query(&[("q", query), ("limit", &limit_param.to_string())])
            .send()?
            .error_for_status()?;
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
