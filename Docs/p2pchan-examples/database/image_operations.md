# Image Operations Documentation

This document describes the image handling operations implemented in `image_operations.go`. This file provides the data structures and operations for managing images and their relationships with posts in the Graphchan system.

## Data Structures

### Image
```go
type Image struct {
    ID            int64     `json:"id"`
    Filename      string    `json:"filename"`
    StoragePath   string    `json:"storage_path"`
    ThumbnailPath *string   `json:"thumbnail_path,omitempty"`
    MimeType      string    `json:"mimetype"`
    Size          int64     `json:"size"`
    Width         int       `json:"width"`
    Height        int       `json:"height"`
    Hash          []byte    `json:"hash"`
    Signature     []byte    `json:"signature,omitempty"`
    CreatedAt     time.Time `json:"created_at"`
}
```

The `Image` struct represents a media attachment with:
- Basic identification (`ID`)
- File information (`Filename`, `StoragePath`, `MimeType`, `Size`)
- Thumbnail support (`ThumbnailPath` as optional)
- Dimensional data (`Width`, `Height`)
- Content verification (`Hash`)
- Cryptographic verification (`Signature`)
- Creation timestamp (`CreatedAt`)

## Core Operations

### Image Storage

#### Save Image
```go
func (db *DB) SaveImage(img *Image) error
```
Persists a new image to the database:
- Uses transaction for data integrity
- Stores all image metadata
- Sets creation timestamp
- Returns generated ID
- Handles rollback on failure

#### Attach Image to Post
```go
func (db *DB) AttachImageToPost(postID int64, imageID int64, displayOrder int) error
```
Creates post-image relationship:
- Associates image with specific post
- Maintains display ordering
- Simple atomic operation
- Returns error on constraint violation

### Image Retrieval

#### Get Thread Images
```go
func (db *DB) GetThreadImages(threadID []byte) ([]*Image, error)
```
Retrieves all images in a thread:
- Joins across posts and relationships
- Eliminates duplicates with DISTINCT
- Orders by creation time (newest first)
- Handles optional thumbnail paths
- Returns complete image metadata

#### Get Post Images
```go
func (db *DB) GetPostImages(postID int64) ([]*Image, error)
```
Fetches images for a specific post:
- Maintains display order
- Includes creation time secondary sort
- Returns full image metadata
- Handles optional thumbnail paths
- Used by post retrieval operations

#### Get Thread Thumbnail
```go
func (db *DB) GetThreadThumbnail(threadID []byte) (*Image, error)
```
Retrieves the first image in a thread:
- Orders by post creation and display order
- Returns single image or nil
- Useful for thread previews
- Includes full image metadata
- Handles case of no images

## Query Patterns

The implementation uses several SQL patterns:
1. **JOIN Operations**:
   - `post_images` for relationships
   - `posts` for thread context
   - Proper index usage

2. **Ordering**:
   - Primary: `display_order` for intended sequence
   - Secondary: `created_at` for stable ordering
   - Descending for thumbnails (newest first)

3. **Null Handling**:
   - `sql.NullString` for optional thumbnails
   - Proper conversion to pointer types
   - Maintains optional field semantics

## Error Handling

The operations implement robust error handling:
1. Transaction management for writes
2. Proper resource cleanup (rows.Close())
3. Error wrapping with context
4. Special handling for "no rows" case
5. Constraint violation detection

## Integration Points

This component integrates with:
- Post operations ([operations.md](operations.md))
- Database core ([database.md](database.md))
- Schema definition ([schema.md](schema.md))

## Performance Considerations

The implementation optimizes for:
1. **Efficient Queries**:
   - Uses appropriate indexes
   - Minimizes JOIN complexity
   - DISTINCT for deduplication

2. **Resource Management**:
   - Proper connection handling
   - Result set cleanup
   - Transaction scope control

3. **Data Loading**:
   - Ordered retrieval for predictable paging
   - Thumbnail-specific queries
   - Batch loading capabilities

## Best Practices

When using these operations:
1. Always close result sets
2. Use transactions for multiple operations
3. Handle nil thumbnail paths appropriately
4. Validate image metadata before saving
5. Consider display order in UI context
6. Implement proper error handling
7. Verify file existence before database entry 