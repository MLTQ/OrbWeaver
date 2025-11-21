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
        let result = client.get_thread(&thread_id);
        let message = AppMessage::ThreadLoaded { thread_id, result };
        if tx.send(message).is_err() {
            error!("failed to send ThreadLoaded message");
        }
    });
}

pub fn create_thread(client: ApiClient, tx: Sender<AppMessage>, payload: CreateThreadInput) {
    thread::spawn(move || {
        let result = client.create_thread(&payload);
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
                // 2. Upload Attachments
                for path in attachments {
                    if let Err(e) = client.upload_file(&post_id, &path) {
                        error!("Failed to upload attachment {:?}: {}", path, e);
                        // We continue trying to upload others even if one fails
                    }
                }
                
                // 3. Send Success Message
                let message = AppMessage::PostCreated { thread_id, result: Ok(post) };
                if tx.send(message).is_err() {
                    error!("failed to send PostCreated message");
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
            let client = crate::api::get_shared_client().map_err(|e| e.to_string())?;
            let resp = client.get(&url).send().map_err(|e| e.to_string())?;
            let bytes = resp.bytes().map_err(|e| e.to_string())?;
            let dyn_img = image::load_from_memory(&bytes).map_err(|e| e.to_string())?;
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
