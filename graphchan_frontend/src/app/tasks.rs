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

pub fn load_thread(client: ApiClient, tx: Sender<AppMessage>, thread_id: String) {
    thread::spawn(move || {
        // First try to download the thread (this will fetch the full content from peers if available)
        let result = client.download_thread(&thread_id)
            .or_else(|download_err| {
                // If download fails (e.g., no ticket available), fall back to regular get
                log::info!("Thread download failed ({}), falling back to get_thread", download_err);
                client.get_thread(&thread_id)
            });

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

pub fn import_fourchan(client: ApiClient, tx: Sender<AppMessage>, url: String) {
    thread::spawn(move || {
        let result = importer::import_fourchan_thread(&client, &url);
        if tx.send(AppMessage::ImportFinished(result)).is_err() {
            error!("failed to send ImportFinished message");
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
            let resp = client.get(&url).send().map_err(|e| format!("Request error: {}", e))?;
            let bytes = resp.bytes().map_err(|e| format!("Download error: {}", e))?;
            let dyn_img = image::load_from_memory(&bytes).map_err(|e| format!("Image decode error: {}", e))?;
            let rgba = dyn_img.to_rgba8();
            let size = [dyn_img.width() as usize, dyn_img.height() as usize];
            Ok(LoadedImage {
                size,
                pixels: rgba.as_flat_samples().as_slice().to_vec(),
            })
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
