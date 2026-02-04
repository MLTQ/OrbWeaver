# files.rs

## Purpose
File storage and retrieval service handling uploads, Iroh blob integration, and MIME type detection. Manages the dual storage model: local filesystem + content-addressed blob store.

## Components

### `FileService`
- **Does**: Manages file uploads, storage, and retrieval
- **Fields**: `database`, `paths`, `config`, `blobs` (FsStore)
- **Pattern**: Async methods for I/O operations

### `save_post_file`
- **Does**: Saves uploaded file to disk and blob store
- **Interacts with**: FileRepository, FsStore, filesystem
- **Flow**:
  1. Validate file size against config limit
  2. Write to `files/uploads/{uuid}.{ext}`
  3. Add to Iroh blob store (content-addressed)
  4. Detect MIME type via `infer` crate
  5. Store metadata in database

### `get_file`
- **Does**: Retrieves file metadata by ID
- **Interacts with**: FileRepository
- **Returns**: `FileView` with download_url

### `get_file_bytes`
- **Does**: Reads file content from disk
- **Interacts with**: Filesystem
- **Returns**: `Vec<u8>` file content

### `export_blob_to_downloads`
- **Does**: Exports blob from store to downloads directory
- **Interacts with**: FsStore, filesystem
- **Use case**: Making received P2P files available locally

### Helper Functions

#### `sanitize_filename`
- **Does**: Removes path traversal and invalid characters
- **Rationale**: Security - prevent directory escaping

#### `infer_mime`
- **Does**: Detects MIME type from file magic bytes
- **Interacts with**: `infer` crate

## Data Types

### `FileView`
- **Fields**: id, original_name, mime, size_bytes, blob_id, download_url, present
- **Note**: `download_url` is relative path for API access

### `SaveFileInput`
- **Fields**: post_id, data (bytes), original_name, mime (optional)

## Contracts

| Dependent | Expects | Breaking changes |
|-----------|---------|------------------|
| `api.rs` | `save_post_file`, `get_file`, `get_file_bytes` | Method changes |
| `network/events.rs` | `export_blob_to_downloads` for P2P files | Path changes |

## Storage Layout

```
{base}/
├── files/
│   ├── uploads/     # Original uploads
│   │   └── {uuid}.{ext}
│   └── downloads/   # Exported from P2P
│       └── {file_id}
└── blobs/           # Iroh FsStore (content-addressed)
```

## Notes
- Files stored twice: original path + blob store (for P2P sharing)
- Blake3 hash used as blob ID and checksum
- `temp_tag.leak()` prevents blob cleanup after upload
- MIME detection falls back to provided MIME or unknown
- Size limit configurable via `FileConfig.max_upload_bytes`
