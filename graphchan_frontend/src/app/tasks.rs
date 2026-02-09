use std::sync::mpsc::Sender;
use std::thread;
use log::error;

use crate::api::ApiClient;
use crate::importer;
use crate::models::{CreatePostInput, CreateThreadInput};

use super::messages::AppMessage;
use super::state::LoadedImage;

// Shared HTTP client logic moved to api.rs

pub fn load_threads(client: ApiClient, tx: Sender<AppMessage>) {
    thread::spawn(move || {
        let result = client.list_threads();
        if tx.send(AppMessage::ThreadsLoaded(result)).is_err() {
            error!("failed to send ThreadsLoaded message");
        }
    });
}

pub fn load_recent_posts(client: ApiClient, tx: Sender<AppMessage>) {
    thread::spawn(move || {
        let result = client.list_recent_posts(Some(50));
        if tx.send(AppMessage::RecentPostsLoaded(result)).is_err() {
            error!("failed to send RecentPostsLoaded message");
        }
    });
}

pub fn load_thread(client: ApiClient, tx: Sender<AppMessage>, thread_id: String, is_refresh: bool) {
    thread::spawn(move || {
        let result = if is_refresh {
            // For refresh, just get local data - don't trigger peer downloads
            client.get_thread(&thread_id)
        } else {
            // For initial load, try to download from peers first
            client.download_thread(&thread_id)
                .or_else(|download_err| {
                    // If download fails (e.g., no ticket available), fall back to regular get
                    log::info!("Thread download failed ({}), falling back to get_thread", download_err);
                    client.get_thread(&thread_id)
                })
        };

        let message = AppMessage::ThreadLoaded { thread_id, result };
        if tx.send(message).is_err() {
            error!("failed to send ThreadLoaded message");
        }
    });
}

pub fn create_thread(client: ApiClient, tx: Sender<AppMessage>, payload: CreateThreadInput, files: Vec<std::path::PathBuf>) {
    thread::spawn(move || {
        let result = client.create_thread(&payload, &files);
        if tx.send(AppMessage::ThreadCreated(result)).is_err() {
            error!("failed to send ThreadCreated message");
        }
    });
}

pub fn create_post(
    client: ApiClient,
    tx: Sender<AppMessage>,
    thread_id: String,
    payload: CreatePostInput,
    attachments: Vec<std::path::PathBuf>,
) {
    thread::spawn(move || {
        // 1. Create Post
        let result = client.create_post(&thread_id, &payload);
        match result {
            Ok(post) => {
                let post_id = post.id.clone();
                let mut uploaded_files = Vec::new();
                
                // 2. Upload Attachments
                for path in attachments {
                    match client.upload_file(&post_id, &path) {
                        Ok(file) => uploaded_files.push(file),
                        Err(e) => error!("Failed to upload attachment {:?}: {}", path, e),
                    }
                }
                
                // 3. Send Success Message
                let message = AppMessage::PostCreated { thread_id: thread_id.clone(), result: Ok(post) };
                if tx.send(message).is_err() {
                    error!("failed to send PostCreated message");
                }

                // 4. Send Attachments Message if any were uploaded
                if !uploaded_files.is_empty() {
                    let attach_msg = AppMessage::PostAttachmentsLoaded {
                        thread_id,
                        post_id,
                        result: Ok(uploaded_files),
                    };
                    if tx.send(attach_msg).is_err() {
                        error!("failed to send PostAttachmentsLoaded message");
                    }
                }
            }
            Err(e) => {
                let message = AppMessage::PostCreated { thread_id, result: Err(e) };
                if tx.send(message).is_err() {
                    error!("failed to send PostCreated message");
                }
            }
        }
    });
}

pub fn import_fourchan(client: ApiClient, tx: Sender<AppMessage>, url: String, topics: Vec<String>) {
    thread::spawn(move || {
        let result = importer::import_fourchan_thread(&client, &url, topics);
        if tx.send(AppMessage::ImportFinished(result)).is_err() {
            error!("failed to send ImportFinished message");
        }
    });
}

pub fn import_reddit(
    api: ApiClient,
    tx: Sender<AppMessage>,
    url: String,
    topics: Vec<String>,
) {
    std::thread::spawn(move || {
        let result = crate::importer::import_reddit_thread(&api, &url, topics);
        match result {
            Ok(thread_id) => {
                let _ = tx.send(AppMessage::ThreadImported(thread_id));
            }
            Err(err) => {
                let _ = tx.send(AppMessage::ImportError(err.to_string()));
            }
        }
    });
}

pub fn refresh_thread_source(client: ApiClient, tx: Sender<AppMessage>, thread_id: String) {
    thread::spawn(move || {
        let result = client.refresh_thread(&thread_id);
        if tx.send(AppMessage::ThreadSourceRefreshed { thread_id, result }).is_err() {
            error!("failed to send ThreadSourceRefreshed message");
        }
    });
}

pub fn load_attachments(
    client: ApiClient,
    tx: Sender<AppMessage>,
    thread_id: String,
    post_id: String,
) {
    thread::spawn(move || {
        let result = client.list_post_files(&post_id);
        let message = AppMessage::PostAttachmentsLoaded {
            thread_id,
            post_id,
            result,
        };
        if tx.send(message).is_err() {
            error!("failed to send PostAttachmentsLoaded message");
        }
    });
}

pub fn download_image(tx: Sender<AppMessage>, file_id: String, url: String) {
    thread::spawn(move || {
        // Log the URL being requested for debugging
        log::info!("Downloading image from URL: {}", url);

        let result = (|| {
            let client = crate::api::get_shared_client().map_err(|e| format!("HTTP client error: {}", e))?;

            // Retry up to 3 times for transient errors
            let mut last_error = String::new();
            for attempt in 1..=3 {
                match client.get(&url).send() {
                    Ok(resp) => {
                        // Check status code before reading body
                        let status = resp.status();
                        if !status.is_success() {
                            return Err(format!("HTTP error: {} (URL: {})", status, url));
                        }

                        match resp.bytes() {
                            Ok(bytes) => {
                                let dyn_img = image::load_from_memory(&bytes).map_err(|e| format!("Image decode error: {}", e))?;
                                let rgba = dyn_img.to_rgba8();
                                let size = [dyn_img.width() as usize, dyn_img.height() as usize];
                                return Ok(LoadedImage {
                                    size,
                                    pixels: rgba.as_flat_samples().as_slice().to_vec(),
                                });
                            }
                            Err(e) => {
                                last_error = format!("Download error: {}", e);
                                if attempt < 3 {
                                    log::warn!("Image download attempt {} failed for {}: {}, retrying...", attempt, file_id, last_error);
                                    std::thread::sleep(std::time::Duration::from_millis(500 * attempt as u64));
                                    continue;
                                }
                            }
                        }
                    }
                    Err(e) => {
                        last_error = format!("Request error: {}", e);
                        if attempt < 3 {
                            log::warn!("Image download attempt {} failed for {}: {}, retrying...", attempt, file_id, last_error);
                            std::thread::sleep(std::time::Duration::from_millis(500 * attempt as u64));
                            continue;
                        }
                    }
                }
            }
            Err(last_error)
        })();

        let message = AppMessage::ImageLoaded { file_id, result };

        if tx.send(message).is_err() {
            error!("failed to send ImageLoaded message");
        }
    });
}

pub fn load_identity(client: ApiClient, tx: Sender<AppMessage>) {
    thread::spawn(move || {
        let result = client.get_self_peer();
        if tx.send(AppMessage::IdentityLoaded(result)).is_err() {
            error!("failed to send IdentityLoaded message");
        }
    });
}

pub fn upload_avatar(client: ApiClient, tx: Sender<AppMessage>, path: String) {
    thread::spawn(move || {
        let path = std::path::Path::new(&path);
        let result = client.upload_avatar(path);
        if tx.send(AppMessage::AvatarUploaded(result)).is_err() {
            error!("failed to send AvatarUploaded message");
        }
    });
}

pub fn update_profile(client: ApiClient, tx: Sender<AppMessage>, username: Option<String>, bio: Option<String>) {
    thread::spawn(move || {
        let result = client.update_profile(username, bio);
        if tx.send(AppMessage::ProfileUpdated(result)).is_err() {
            error!("failed to send ProfileUpdated message");
        }
    });
}

pub fn add_peer(client: ApiClient, tx: Sender<AppMessage>, friendcode: String) {
    thread::spawn(move || {
        let result = client.add_peer(&friendcode);
        if tx.send(AppMessage::PeerAdded(result)).is_err() {
            error!("failed to send PeerAdded message");
        }
    });
}

pub fn load_peers(client: ApiClient, tx: Sender<AppMessage>) {
    thread::spawn(move || {
        let result = client.list_peers();
        if tx.send(AppMessage::PeersLoaded(result)).is_err() {
            error!("failed to send PeersLoaded message");
        }
    });
}

pub fn pick_files(tx: Sender<AppMessage>) {
    thread::spawn(move || {
        if let Some(files) = rfd::FileDialog::new().pick_files() {
            if tx.send(AppMessage::ThreadFilesSelected(files)).is_err() {
                error!("failed to send ThreadFilesSelected message");
            }
        }
    });
}

pub fn download_text_file(tx: Sender<AppMessage>, file_id: String, url: String) {
    thread::spawn(move || {
        let result = (|| {
            let client = crate::api::get_shared_client().map_err(|e| e.to_string())?;
            let resp = client.get(&url).send().map_err(|e| e.to_string())?;
            let text = resp.text().map_err(|e| e.to_string())?;
            Ok(text)
        })();

        let message = AppMessage::TextFileLoaded { file_id, result };
        if tx.send(message).is_err() {
            error!("failed to send TextFileLoaded message");
        }
    });
}

pub fn download_media_file(tx: Sender<AppMessage>, file_id: String, url: String) {
    thread::spawn(move || {
        let result = (|| {
            let client = crate::api::get_shared_client().map_err(|e| e.to_string())?;
            let resp = client.get(&url).send().map_err(|e| e.to_string())?;
            let bytes = resp.bytes().map_err(|e| e.to_string())?.to_vec();
            Ok(bytes)
        })();

        let message = AppMessage::MediaFileLoaded { file_id, result };
        if tx.send(message).is_err() {
            error!("failed to send MediaFileLoaded message");
        }
    });
}

pub fn download_pdf_file(tx: Sender<AppMessage>, file_id: String, url: String) {
    thread::spawn(move || {
        let result = (|| {
            let client = crate::api::get_shared_client().map_err(|e| e.to_string())?;
            let resp = client.get(&url).send().map_err(|e| e.to_string())?;
            let bytes = resp.bytes().map_err(|e| e.to_string())?.to_vec();
            Ok(bytes)
        })();

        let message = AppMessage::PdfFileLoaded { file_id, result };
        if tx.send(message).is_err() {
            error!("failed to send PdfFileLoaded message");
        }
    });
}

pub fn save_file_as(tx: Sender<AppMessage>, file_id: String, url: String, suggested_name: String) {
    thread::spawn(move || {
        let result = (|| {
            let client = crate::api::get_shared_client().map_err(|e| e.to_string())?;
            let resp = client.get(&url).send().map_err(|e| e.to_string())?;
            let bytes = resp.bytes().map_err(|e| e.to_string())?;

            // Open save dialog
            if let Some(path) = rfd::FileDialog::new()
                .set_file_name(&suggested_name)
                .save_file()
            {
                std::fs::write(path, bytes).map_err(|e| e.to_string())?;
                Ok(())
            } else {
                Err("Save cancelled".to_string())
            }
        })();

        let message = AppMessage::FileSaved { file_id, result };
        if tx.send(message).is_err() {
            error!("failed to send FileSaved message");
        }
    });
}

pub fn delete_thread(client: ApiClient, tx: Sender<AppMessage>, thread_id: String) {
    thread::spawn(move || {
        let result = client.delete_thread(&thread_id);
        if result.is_ok() {
            // Reload threads after successful deletion
            let threads_result = client.list_threads();
            if tx.send(AppMessage::ThreadsLoaded(threads_result)).is_err() {
                error!("failed to send ThreadsLoaded message after delete");
            }
        } else if let Err(e) = result {
            error!("Failed to delete thread: {}", e);
        }
    });
}

pub fn ignore_thread(client: ApiClient, tx: Sender<AppMessage>, thread_id: String, ignored: bool) {
    thread::spawn(move || {
        let result = client.set_thread_ignored(&thread_id, ignored);
        if result.is_ok() {
            // Reload threads after successful ignore
            let threads_result = client.list_threads();
            if tx.send(AppMessage::ThreadsLoaded(threads_result)).is_err() {
                error!("failed to send ThreadsLoaded message after ignore");
            }
        } else if let Err(e) = result {
            error!("Failed to ignore thread: {}", e);
        }
    });
}

pub fn load_reactions(client: ApiClient, tx: Sender<AppMessage>, post_id: String) {
    thread::spawn(move || {
        let result = client.get_reactions(&post_id);
        let message = AppMessage::ReactionsLoaded {
            post_id,
            result,
        };
        if tx.send(message).is_err() {
            error!("failed to send ReactionsLoaded message");
        }
    });
}

pub fn add_reaction(client: ApiClient, tx: Sender<AppMessage>, post_id: String, emoji: String) {
    thread::spawn(move || {
        let result = client.add_reaction(&post_id, &emoji);
        let message = AppMessage::ReactionAdded {
            post_id,
            result,
        };
        if tx.send(message).is_err() {
            error!("failed to send ReactionAdded message");
        }
    });
}

pub fn remove_reaction(client: ApiClient, tx: Sender<AppMessage>, post_id: String, emoji: String) {
    thread::spawn(move || {
        let result = client.remove_reaction(&post_id, &emoji);
        let message = AppMessage::ReactionRemoved {
            post_id,
            result,
        };
        if tx.send(message).is_err() {
            error!("failed to send ReactionRemoved message");
        }
    });
}

pub fn unfollow_peer(client: ApiClient, tx: Sender<AppMessage>, peer_id: String) {
    thread::spawn(move || {
        let result = client.unfollow_peer(&peer_id);
        if result.is_ok() {
            // Reload peers after successful unfollow
            let peers_result = client.list_peers();
            if tx.send(AppMessage::PeersLoaded(peers_result)).is_err() {
                error!("failed to send PeersLoaded message after unfollow");
            }
        } else if let Err(e) = result {
            error!("Failed to unfollow peer: {}", e);
        }
    });
}

// Direct Message tasks

pub fn load_conversations(client: ApiClient, tx: Sender<AppMessage>) {
    thread::spawn(move || {
        let result = client.list_conversations();
        if tx.send(AppMessage::ConversationsLoaded(result)).is_err() {
            error!("failed to send ConversationsLoaded message");
        }
    });
}

pub fn load_messages(client: ApiClient, tx: Sender<AppMessage>, peer_id: String) {
    thread::spawn(move || {
        let result = client.get_messages(&peer_id, 50);
        let message = AppMessage::MessagesLoaded { peer_id, result };
        if tx.send(message).is_err() {
            error!("failed to send MessagesLoaded message");
        }
    });
}

pub fn send_dm(client: ApiClient, tx: Sender<AppMessage>, to_peer_id: String, body: String) {
    thread::spawn(move || {
        let result = client.send_dm(&to_peer_id, &body);
        let message = AppMessage::DmSent {
            to_peer_id,
            result,
        };
        if tx.send(message).is_err() {
            error!("failed to send DmSent message");
        }
    });
}

pub fn load_blocked_peers(client: ApiClient, tx: Sender<AppMessage>) {
    thread::spawn(move || {
        let result = client.list_blocked_peers();
        let message = AppMessage::BlockedPeersLoaded(result);
        if tx.send(message).is_err() {
            error!("failed to send BlockedPeersLoaded message");
        }
    });
}

pub fn block_peer(client: ApiClient, tx: Sender<AppMessage>, peer_id: String, reason: Option<String>) {
    thread::spawn(move || {
        let result = client.block_peer(&peer_id, reason);
        let message = AppMessage::PeerBlocked { peer_id, result };
        if tx.send(message).is_err() {
            error!("failed to send PeerBlocked message");
        }
    });
}

pub fn unblock_peer(client: ApiClient, tx: Sender<AppMessage>, peer_id: String) {
    thread::spawn(move || {
        let result = client.unblock_peer(&peer_id);
        let message = AppMessage::PeerUnblocked { peer_id, result };
        if tx.send(message).is_err() {
            error!("failed to send PeerUnblocked message");
        }
    });
}

pub fn load_blocklists(client: ApiClient, tx: Sender<AppMessage>) {
    thread::spawn(move || {
        let result = client.list_blocklists();
        let message = AppMessage::BlocklistsLoaded(result);
        if tx.send(message).is_err() {
            error!("failed to send BlocklistsLoaded message");
        }
    });
}

pub fn subscribe_blocklist(
    client: ApiClient,
    tx: Sender<AppMessage>,
    blocklist_id: String,
    maintainer_peer_id: String,
    name: String,
    description: Option<String>,
    auto_apply: bool,
) {
    thread::spawn(move || {
        let request = crate::models::SubscribeBlocklistRequest {
            maintainer_peer_id,
            name,
            description,
            auto_apply,
        };
        let result = client.subscribe_blocklist(&request);
        let message = AppMessage::BlocklistSubscribed { blocklist_id, result };
        if tx.send(message).is_err() {
            error!("failed to send BlocklistSubscribed message");
        }
    });
}

pub fn unsubscribe_blocklist(client: ApiClient, tx: Sender<AppMessage>, blocklist_id: String) {
    thread::spawn(move || {
        let result = client.unsubscribe_blocklist(&blocklist_id);
        let message = AppMessage::BlocklistUnsubscribed { blocklist_id, result };
        if tx.send(message).is_err() {
            error!("failed to send BlocklistUnsubscribed message");
        }
    });
}

pub fn load_blocklist_entries(client: ApiClient, tx: Sender<AppMessage>, blocklist_id: String) {
    thread::spawn(move || {
        let result = client.list_blocklist_entries(&blocklist_id);
        let message = AppMessage::BlocklistEntriesLoaded { blocklist_id, result };
        if tx.send(message).is_err() {
            error!("failed to send BlocklistEntriesLoaded message");
        }
    });
}

// IP Blocking tasks

pub fn load_ip_blocks(client: ApiClient, tx: Sender<AppMessage>) {
    thread::spawn(move || {
        let result = client.list_ip_blocks();
        let message = AppMessage::IpBlocksLoaded(result);
        if tx.send(message).is_err() {
            error!("failed to send IpBlocksLoaded message");
        }
    });
}

pub fn load_ip_block_stats(client: ApiClient, tx: Sender<AppMessage>) {
    thread::spawn(move || {
        let result = client.get_ip_block_stats();
        let message = AppMessage::IpBlockStatsLoaded(result);
        if tx.send(message).is_err() {
            error!("failed to send IpBlockStatsLoaded message");
        }
    });
}

pub fn add_ip_block(client: ApiClient, tx: Sender<AppMessage>, ip_or_range: String, reason: Option<String>) {
    thread::spawn(move || {
        let result = client.add_ip_block(&ip_or_range, reason);
        let message = AppMessage::IpBlockAdded { result };
        if tx.send(message).is_err() {
            error!("failed to send IpBlockAdded message");
        }
    });
}

pub fn remove_ip_block(client: ApiClient, tx: Sender<AppMessage>, block_id: i64) {
    thread::spawn(move || {
        let result = client.remove_ip_block(block_id);
        let message = AppMessage::IpBlockRemoved { block_id, result };
        if tx.send(message).is_err() {
            error!("failed to send IpBlockRemoved message");
        }
    });
}

pub fn export_peer_blocks(client: ApiClient, tx: Sender<AppMessage>) {
    thread::spawn(move || {
        let result = client.export_peer_blocks();
        let message = AppMessage::PeerBlocksExported { result };
        if tx.send(message).is_err() {
            error!("failed to send PeerBlocksExported message");
        }
    });
}

pub fn import_peer_blocks(client: ApiClient, tx: Sender<AppMessage>, import_text: String) {
    thread::spawn(move || {
        let result = client.import_peer_blocks(&import_text);
        let message = AppMessage::PeerBlocksImported { result };
        if tx.send(message).is_err() {
            error!("failed to send PeerBlocksImported message");
        }
    });
}

pub fn import_ip_blocks(client: ApiClient, tx: Sender<AppMessage>, import_text: String) {
    thread::spawn(move || {
        let result = client.import_ip_blocks(&import_text);
        let message = AppMessage::IpBlocksImported { result };
        if tx.send(message).is_err() {
            error!("failed to send IpBlocksImported message");
        }
    });
}

pub fn export_ip_blocks(client: ApiClient, tx: Sender<AppMessage>) {
    thread::spawn(move || {
        let result = client.export_ip_blocks();
        let message = AppMessage::IpBlocksExported { result };
        if tx.send(message).is_err() {
            error!("failed to send IpBlocksExported message");
        }
    });
}

pub fn clear_all_ip_blocks(client: ApiClient, tx: Sender<AppMessage>) {
    thread::spawn(move || {
        let result = client.clear_all_ip_blocks();
        let message = AppMessage::IpBlocksCleared { result };
        if tx.send(message).is_err() {
            error!("failed to send IpBlocksCleared message");
        }
    });
}

pub fn trigger_file_download(client: ApiClient, tx: Sender<AppMessage>, file_id: String, thread_id: String) {
    thread::spawn(move || {
        let result = client.trigger_file_download(&file_id);
        if result.is_ok() {
            // Reload thread to get updated download status
            let thread_result = client.get_thread(&thread_id);
            if thread_result.is_ok() {
                let message = AppMessage::ThreadLoaded { thread_id: thread_id.clone(), result: thread_result };
                if tx.send(message).is_err() {
                    error!("failed to send ThreadLoaded message after file download trigger");
                }
            }
        } else if let Err(e) = result {
            error!("Failed to trigger file download: {}", e);
        }
    });
}

pub fn block_peer_ip(client: ApiClient, tx: Sender<AppMessage>, peer_id: String) {
    thread::spawn(move || {
        // First, fetch the peer's IP addresses
        let ips_result = client.get_peer_ips(&peer_id);

        match ips_result {
            Ok(peer_ips) => {
                if peer_ips.ips.is_empty() {
                    let message = AppMessage::PeerIpBlockFailed {
                        peer_id,
                        error: "No IP addresses found for this peer".to_string(),
                    };
                    if tx.send(message).is_err() {
                        error!("failed to send PeerIpBlockFailed message");
                    }
                    return;
                }

                // Block all IPs for this peer
                let mut blocked_count = 0;
                let mut errors = Vec::new();

                for ip in &peer_ips.ips {
                    let reason = Some(format!("Blocked peer: {}", peer_id));
                    match client.add_ip_block(ip, reason) {
                        Ok(_) => blocked_count += 1,
                        Err(e) => errors.push(format!("{}: {}", ip, e)),
                    }
                }

                if blocked_count > 0 {
                    let message = AppMessage::PeerIpBlocked {
                        peer_id,
                        blocked_ips: peer_ips.ips.clone(),
                    };
                    if tx.send(message).is_err() {
                        error!("failed to send PeerIpBlocked message");
                    }
                } else {
                    let message = AppMessage::PeerIpBlockFailed {
                        peer_id,
                        error: format!("Failed to block IPs: {}", errors.join(", ")),
                    };
                    if tx.send(message).is_err() {
                        error!("failed to send PeerIpBlockFailed message");
                    }
                }
            }
            Err(err) => {
                let message = AppMessage::PeerIpBlockFailed {
                    peer_id,
                    error: format!("Failed to fetch peer IPs: {}", err),
                };
                if tx.send(message).is_err() {
                    error!("failed to send PeerIpBlockFailed message");
                }
            }
        }
    });
}

// Topic management tasks
pub fn load_topics(client: ApiClient, tx: Sender<AppMessage>) {
    thread::spawn(move || {
        let result = client.list_topics();
        if tx.send(AppMessage::TopicsLoaded(result)).is_err() {
            error!("failed to send TopicsLoaded message");
        }
    });
}

pub fn load_theme_color(client: ApiClient, tx: Sender<AppMessage>) {
    thread::spawn(move || {
        let result = client.get_theme_color();
        if tx.send(AppMessage::ThemeColorLoaded(result)).is_err() {
            error!("failed to send ThemeColorLoaded message");
        }
    });
}

pub fn subscribe_topic(client: ApiClient, tx: Sender<AppMessage>, topic_id: String) {
    thread::spawn(move || {
        let result = client.subscribe_topic(&topic_id);
        let message = AppMessage::TopicSubscribed {
            topic_id,
            result,
        };
        if tx.send(message).is_err() {
            error!("failed to send TopicSubscribed message");
        }
    });
}

pub fn unsubscribe_topic(client: ApiClient, tx: Sender<AppMessage>, topic_id: String) {
    thread::spawn(move || {
        let result = client.unsubscribe_topic(&topic_id);
        let message = AppMessage::TopicUnsubscribed {
            topic_id,
            result,
        };
        if tx.send(message).is_err() {
            error!("failed to send TopicUnsubscribed message");
        }
    });
}
