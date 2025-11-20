# Database Operations Documentation

This document describes the core database operations implemented in `operations.go`. This file provides the primary data structures and CRUD operations for managing posts and their relationships in the Graphchan system.

## Data Structures

### Post
```go
type Post struct {
    ID           int64     `json:"id"`
    ThreadID     []byte    `json:"thread_id"`
    Title        string    `json:"title"`
    Content      string    `json:"content"`
    AuthorID     string    `json:"author_id"`
    Friendcode   string    `json:"friendcode,omitempty"`
    CreatedAt    time.Time `json:"created_at"`
    LastModified time.Time `json:"last_modified"`
    PositionX    float64   `json:"position_x"`
    PositionY    float64   `json:"position_y"`
    Likes        int       `json:"likes"`
    ParentIDs    []int64   `json:"parent_ids,omitempty"`
    Pinned       bool      `json:"pinned"`
    Deleted      bool      `json:"deleted"`
    Signature    []byte    `json:"signature,omitempty"`
    Metadata     string    `json:"metadata,omitempty"`
    Images       []*Image  `json:"images,omitempty"`
}
```

The `Post` struct represents a message in the system with:
- Unique identification (`ID`, `ThreadID`)
- Content fields (`Title`, `Content`)
- Author information (`AuthorID`, `Friendcode`)
- Temporal tracking (`CreatedAt`, `LastModified`)
- Spatial positioning (`PositionX`, `PositionY`)
- Social features (`Likes`, `ParentIDs` for replies)
- State flags (`Pinned`, `Deleted`)
- Cryptographic verification (`Signature`)
- Extensibility (`Metadata`)
- Media attachments (`Images`)

### PostImage
```go
type PostImage struct {
    PostID       int64 `json:"post_id"`
    ImageID      int64 `json:"image_id"`
    DisplayOrder int   `json:"display_order"`
}
```

Represents the relationship between posts and images, maintaining:
- References to both entities
- Display ordering for multiple images

## Core Operations

### Post Creation
```go
func (db *DB) CreatePost(post *Post) error
```
Creates a new post with transactional integrity:
- Begins a database transaction
- Marshals parent IDs to JSON
- Inserts the post record
- Creates image relationships with ordering
- Commits or rolls back the transaction

### Post Retrieval

#### Get Single Post
```go
func (db *DB) GetPost(id int64) (*Post, error)
```
Retrieves a complete post by ID:
- Fetches basic post data
- Parses parent IDs from JSON
- Loads associated images
- Returns nil if post not found

#### Get All Posts
```go
func (db *DB) GetAllPosts() ([]*Post, error)
```
Retrieves all posts in the system:
- Orders by creation time
- Includes all relationships and images
- Handles JSON parsing for parent IDs

#### Get Thread Posts
```go
func (db *DB) GetThreadPosts(threadID []byte) ([]*Post, error)
```
Retrieves all posts in a specific thread:
- Filters by thread ID
- Orders by creation time
- Includes full post data and relationships

### Post Updates

#### Update Position
```go
func (db *DB) UpdatePostPosition(id int64, x, y float64) error
```
Updates a post's spatial position:
- Simple coordinate update
- No transaction needed for atomic operation

#### Like Post
```go
func (db *DB) LikePost(id int64) (int, error)
```
Implements post liking with:
- Transactional increment of like count
- Returns updated like count
- Atomic operation guarantee

### Helper Functions

#### Get Post Images
```go
func (db *DB) getPostImages(post *Post) error
```
Internal helper that:
- Loads all images for a post
- Updates the post's Images field
- Used by other retrieval functions

## Error Handling

The operations implement comprehensive error handling:
1. Transaction management with proper rollback
2. Detailed error wrapping with operation context
3. SQL error translation
4. JSON marshaling/unmarshaling error handling
5. No-rows handling for queries

## Design Patterns

The implementation follows several key patterns:
1. **Transactional Integrity**: Used for multi-step operations
2. **Error Wrapping**: Contextual error information
3. **Lazy Loading**: Images loaded on demand
4. **Clean API**: Consistent method signatures
5. **Type Safety**: Strong typing for all operations

## Integration Points

This operations layer integrates with:
- Image operations ([image_operations.md](image_operations.md))
- Database core ([database.md](database.md))
- Schema definition ([schema.md](schema.md))

## Best Practices

When using these operations:
1. Always handle returned errors
2. Use transactions for multi-step operations
3. Consider using GetPost for single post operations
4. Batch operations when possible with GetAllPosts
5. Maintain proper parent-child relationships
6. Validate input data before operations 