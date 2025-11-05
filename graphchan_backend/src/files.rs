use crate::config::{FileConfig, GraphchanPaths};
use crate::database::models::FileRecord;
use crate::database::repositories::{FileRepository, PostRepository};
use crate::database::Database;
use anyhow::{anyhow, Context, Result};
use bytes::Bytes;
use infer::Infer;
use iroh_blobs::store::fs::FsStore;
use iroh_blobs::ticket::BlobTicket;
use serde::Serialize;
use std::path::{Path, PathBuf};
use tokio::fs;
use uuid::Uuid;

#[derive(Clone)]
pub struct FileService {
    database: Database,
    paths: GraphchanPaths,
    config: FileConfig,
    blobs: FsStore,
}

impl FileService {
    pub fn new(
        database: Database,
        paths: GraphchanPaths,
        config: FileConfig,
        blobs: FsStore,
    ) -> Self {
        Self {
            database,
            paths,
            config,
            blobs,
        }
    }

    pub async fn save_post_file(&self, input: SaveFileInput) -> Result<FileView> {
        if input.data.is_empty() {
            return Err(anyhow!("file data may not be empty"));
        }

        if let Some(limit) = self.config.max_upload_bytes {
            if (input.data.len() as u64) > limit {
                return Err(anyhow!(
                    "file exceeds configured maximum of {} bytes",
                    limit
                ));
            }
        }

        let post_id = input.post_id.clone();
        self.ensure_post_exists(&post_id)?;

        let file_id = Uuid::new_v4().to_string();
        let original_name = input.original_name.as_deref().map(sanitize_filename);

        let stored_name = match original_name
            .as_deref()
            .and_then(|name| Path::new(name).extension().and_then(|ext| ext.to_str()))
        {
            Some(ext) if !ext.is_empty() => format!("{file_id}.{ext}"),
            _ => file_id.clone(),
        };

        let relative_path = format!("files/uploads/{stored_name}");
        let absolute_path = self.paths.base.join(&relative_path);
        if let Some(parent) = absolute_path.parent() {
            fs::create_dir_all(parent).await.with_context(|| {
                format!("failed to create upload directory {}", parent.display())
            })?;
        }
        let bytes = Bytes::from(input.data.clone());
        fs::write(&absolute_path, bytes.as_ref())
            .await
            .with_context(|| {
                format!(
                    "failed to write uploaded file to {}",
                    absolute_path.display()
                )
            })?;

        let mut temp_tag = self
            .blobs
            .add_bytes(bytes.clone())
            .temp_tag()
            .await
            .context("failed to store blob in iroh-blobs store")?;
        let hash_info = temp_tag.hash_and_format();
        let blob_hex = hash_info.hash.to_hex().to_string();
        temp_tag.leak();

        let size_bytes = bytes.len() as i64;
        let checksum = Some(format!("blake3:{}", blob_hex));
        let blob_id = Some(blob_hex.clone());
        let detected_mime = input.mime.clone().or_else(|| infer_mime(bytes.as_ref()));

        let record = FileRecord {
            id: file_id.clone(),
            post_id,
            path: relative_path.clone(),
            original_name: original_name.clone(),
            mime: detected_mime,
            blob_id: blob_id.clone(),
            size_bytes: Some(size_bytes),
            checksum: checksum.clone(),
            ticket: None,
        };

        self.database.with_repositories(|repos| {
            repos.files().attach(&record)?;
            Ok(())
        })?;

        Ok(FileView::from_record(record))
    }

    pub fn list_post_files(&self, post_id: &str) -> Result<Vec<FileView>> {
        let base = self.paths.base.clone();
        self.database.with_repositories(|repos| {
            let files = repos.files().list_for_post(post_id)?;
            Ok(files
                .into_iter()
                .map(|record| {
                    let mut view = FileView::from_record(record.clone());
                    let absolute = base.join(&record.path);
                    view.present = Some(absolute.exists());
                    view
                })
                .collect())
        })
    }

    pub async fn prepare_download(&self, id: &str) -> Result<Option<FileDownload>> {
        let record = self
            .database
            .with_repositories(|repos| repos.files().get(id))?;
        let Some(record) = record else {
            return Ok(None);
        };
        let absolute_path = self.paths.base.join(&record.path);
        if fs::metadata(&absolute_path).await.is_err() {
            tracing::warn!(path = %absolute_path.display(), "file metadata missing on disk");
            return Ok(None);
        }
        let mut view = FileView::from_record(record);
        view.present = Some(true);
        Ok(Some(FileDownload {
            metadata: view,
            absolute_path,
        }))
    }

    pub fn persist_ticket(&self, file_id: &str, ticket: Option<&BlobTicket>) -> Result<()> {
        let ticket_value = ticket.map(|t| t.to_string());
        self.database.with_repositories(|repos| {
            if let Some(mut record) = repos.files().get(file_id)? {
                record.ticket = ticket_value.clone();
                repos.files().upsert(&record)?;
            }
            Ok(())
        })
    }

    fn ensure_post_exists(&self, post_id: &str) -> Result<()> {
        self.database.with_repositories(|repos| {
            if repos.posts().get(post_id)?.is_none() {
                return Err(anyhow!("post not found"));
            }
            Ok(())
        })
    }
}

#[derive(Debug, Clone)]
pub struct SaveFileInput {
    pub post_id: String,
    pub original_name: Option<String>,
    pub mime: Option<String>,
    pub data: Vec<u8>,
}

#[derive(Debug, Clone, Serialize)]
pub struct FileView {
    pub id: String,
    pub post_id: String,
    pub original_name: Option<String>,
    pub mime: Option<String>,
    pub size_bytes: Option<i64>,
    pub checksum: Option<String>,
    pub blob_id: Option<String>,
    pub ticket: Option<String>,
    pub path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub present: Option<bool>,
}

#[derive(Debug, Clone)]
pub struct FileDownload {
    pub metadata: FileView,
    pub absolute_path: PathBuf,
}

impl FileView {
    fn from_record(record: FileRecord) -> Self {
        Self {
            id: record.id,
            post_id: record.post_id,
            original_name: record.original_name,
            mime: record.mime,
            size_bytes: record.size_bytes,
            checksum: record.checksum,
            blob_id: record.blob_id,
            ticket: record.ticket,
            path: record.path,
            present: None,
        }
    }
}

fn sanitize_filename(name: &str) -> String {
    Path::new(name)
        .file_name()
        .and_then(|file| file.to_str())
        .unwrap_or("upload")
        .chars()
        .map(|c| match c {
            'a'..='z' | 'A'..='Z' | '0'..='9' | '.' | '-' | '_' => c,
            _ => '_',
        })
        .collect()
}

fn infer_mime(data: &[u8]) -> Option<String> {
    Infer::new()
        .get(data)
        .map(|info| info.mime_type().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::FileConfig;
    use crate::config::GraphchanPaths;
    use crate::database::models::{PostRecord, ThreadRecord};
    use crate::database::repositories::{PostRepository, ThreadRepository};
    use crate::database::Database;
    use crate::utils::now_utc_iso;
    use rusqlite::Connection;
    use tempfile::tempdir;
    use tokio::runtime::Runtime;

    #[test]
    fn save_and_list_files() {
        let rt = Runtime::new().unwrap();
        rt.block_on(async {
            let temp = tempdir().expect("tempdir");
            let paths = GraphchanPaths::from_base_dir(temp.path()).expect("paths");
            let conn = Connection::open_in_memory().expect("db");
            let db = Database::from_connection(conn, true);
            db.ensure_migrations().expect("migrations");

            // seed post
            db.with_repositories(|repos| {
                repos.threads().create(&ThreadRecord {
                    id: "thread-1".into(),
                    title: "T".into(),
                    creator_peer_id: None,
                    created_at: now_utc_iso(),
                    pinned: false,
                })?;
                repos.posts().create(&PostRecord {
                    id: "post-1".into(),
                    thread_id: "thread-1".into(),
                    author_peer_id: None,
                    body: "body".into(),
                    created_at: now_utc_iso(),
                    updated_at: None,
                })?;
                Ok(())
            })
            .unwrap();

            let blob_store = FsStore::load(&paths.blobs_dir).await.expect("blob store");
            let service =
                FileService::new(db.clone(), paths.clone(), FileConfig::default(), blob_store);
            let file = service
                .save_post_file(SaveFileInput {
                    post_id: "post-1".into(),
                    original_name: Some("example.txt".into()),
                    mime: Some("text/plain".into()),
                    data: b"hello".to_vec(),
                })
                .await
                .expect("save file");

            assert_eq!(file.original_name.as_deref(), Some("example.txt"));
            assert_eq!(file.mime.as_deref(), Some("text/plain"));
            assert_eq!(file.size_bytes, Some(5));
            assert!(file
                .checksum
                .as_deref()
                .map(|c| c.starts_with("blake3:"))
                .unwrap_or(false));
            assert_eq!(file.blob_id.as_ref().map(|s| s.len()), Some(64));

            let files = service.list_post_files("post-1").expect("list");
            assert_eq!(files.len(), 1);

            let download = service
                .prepare_download(&file.id)
                .await
                .expect("prepare download")
                .expect("download exists");
            assert!(download.absolute_path.exists());
            assert_eq!(download.metadata.blob_id, file.blob_id);
            assert_eq!(files[0].present, Some(true));
        });
    }

    #[test]
    fn reject_oversized_upload() {
        let rt = Runtime::new().unwrap();
        rt.block_on(async {
            let temp = tempdir().expect("tempdir");
            let paths = GraphchanPaths::from_base_dir(temp.path()).expect("paths");
            let conn = Connection::open_in_memory().expect("db");
            let db = Database::from_connection(conn, true);
            db.ensure_migrations().expect("migrations");

            db.with_repositories(|repos| {
                repos.threads().create(&ThreadRecord {
                    id: "thread-1".into(),
                    title: "T".into(),
                    creator_peer_id: None,
                    created_at: now_utc_iso(),
                    pinned: false,
                })?;
                repos.posts().create(&PostRecord {
                    id: "post-1".into(),
                    thread_id: "thread-1".into(),
                    author_peer_id: None,
                    body: "body".into(),
                    created_at: now_utc_iso(),
                    updated_at: None,
                })?;
                Ok(())
            })
            .unwrap();

            let blob_store = FsStore::load(&paths.blobs_dir).await.expect("blob store");
            let service = FileService::new(
                db.clone(),
                paths.clone(),
                FileConfig {
                    max_upload_bytes: Some(2),
                },
                blob_store,
            );

            let result = service
                .save_post_file(SaveFileInput {
                    post_id: "post-1".into(),
                    original_name: Some("example.txt".into()),
                    mime: Some("text/plain".into()),
                    data: b"toolarge".to_vec(),
                })
                .await;
            assert!(result.is_err());
        });
    }
}
