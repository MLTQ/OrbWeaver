# Database Schema Documentation

This document describes the database schema for the Graphchan application. The schema is implemented using SQLite and defines the core data structures for the distributed messaging system.

## Overview

The database schema consists of several key tables that manage threads, posts, files, peer relationships, and system metadata. The schema is designed to support a distributed peer-to-peer messaging system with content management and peer discovery capabilities.

## Tables

### Schema Version
```sql
CREATE TABLE schema_version (
    version INTEGER PRIMARY KEY,
    applied_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
)
```
Tracks database schema versions and when they were applied, enabling schema migrations and version control.

### Threads
```sql
CREATE TABLE threads (
    id INTEGER PRIMARY KEY,
    title TEXT NOT NULL,
    created_at DATETIME NOT NULL,
    files_directory TEXT NOT NULL,
    source_url TEXT,
    source_board TEXT,
    source_thread_id TEXT,
    creator_pubkey BLOB,
    last_active TIMESTAMP,
    pinned BOOLEAN DEFAULT FALSE,
    locked BOOLEAN DEFAULT FALSE,
    hidden BOOLEAN DEFAULT FALSE,
    deleted BOOLEAN DEFAULT FALSE,
    tags TEXT,
    signature BLOB,
    metadata TEXT
)
```
The main discussion container. Notable features:
- Uses INTEGER for id to support SQLite optimization
- Tracks thread state (pinned, locked, hidden, deleted)
- Includes cryptographic elements (creator_pubkey, signature)
- Supports source tracking for imported content
- Flexible metadata field for future extensions

### Posts
```sql
CREATE TABLE posts (
    id INTEGER PRIMARY KEY,
    thread_id INTEGER NOT NULL,
    content TEXT,
    created_at DATETIME NOT NULL,
    source_post_id TEXT,
    title TEXT,
    author_id TEXT,
    friendcode TEXT,
    last_modified TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    position_x REAL DEFAULT 0,
    position_y REAL DEFAULT 0,
    likes INTEGER DEFAULT 0,
    pinned BOOLEAN DEFAULT FALSE,
    deleted BOOLEAN DEFAULT FALSE,
    signature BLOB,
    metadata TEXT,
    FOREIGN KEY (thread_id) REFERENCES threads(id) ON DELETE CASCADE
)
```
Individual messages within threads. Notable features:
- Supports spatial positioning (position_x, position_y)
- Implements thread relationships via foreign key
- Tracks message history and modifications
- Supports imported content with source tracking
- Includes engagement metrics (likes)

### Files
```sql
CREATE TABLE files (
    id INTEGER PRIMARY KEY,
    post_id INTEGER NOT NULL,
    filename TEXT NOT NULL,
    original_filename TEXT NOT NULL,
    file_path TEXT NOT NULL,
    thumbnail_path TEXT,
    mimetype TEXT,
    size INTEGER,
    width INTEGER,
    height INTEGER,
    hash BLOB,
    signature BLOB,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (post_id) REFERENCES posts(id) ON DELETE CASCADE
)
```
Manages file attachments with:
- Complete file metadata
- Content verification (hash)
- Thumbnail support
- Cryptographic signatures
- Support for any file type, not just images

### Post Relationships
```sql
CREATE TABLE post_relationships (
    child_id INTEGER NOT NULL,
    parent_id INTEGER NOT NULL,
    PRIMARY KEY (child_id, parent_id),
    FOREIGN KEY (child_id) REFERENCES posts(id) ON DELETE CASCADE,
    FOREIGN KEY (parent_id) REFERENCES posts(id) ON DELETE CASCADE
)
```
Maps reply relationships between posts, enabling threaded discussions and reply chains.

### Peers
```sql
CREATE TABLE peers (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    friendcode TEXT NOT NULL UNIQUE,
    pubkey BLOB NOT NULL,
    last_ip TEXT,
    last_seen TIMESTAMP,
    known_ips TEXT,
    username TEXT,
    first_seen TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    total_posts INTEGER DEFAULT 0,
    connection_success_rate REAL DEFAULT 0,
    trust_score INTEGER DEFAULT 0,
    blocked BOOLEAN DEFAULT FALSE,
    notes TEXT,
    metadata TEXT
)
```
Manages direct peer relationships with:
- Peer identification (friendcode, pubkey)
- Connection tracking
- Trust and reliability metrics
- Blocking capability

### Peers of Peers
```sql
CREATE TABLE peers_of_peers (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    friendcode TEXT NOT NULL UNIQUE,
    pubkey BLOB NOT NULL,
    last_ip TEXT,
    last_seen TIMESTAMP,
    known_ips TEXT,
    username TEXT,
    first_seen TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    total_posts INTEGER DEFAULT 0,
    source_peers TEXT NOT NULL,
    last_verified TIMESTAMP,
    hop_distance INTEGER DEFAULT 1,
    routing_preference INTEGER DEFAULT 0,
    trust_score INTEGER DEFAULT 0,
    blocked BOOLEAN DEFAULT FALSE,
    metadata TEXT
)
```
Tracks indirect peer relationships, supporting network discovery with:
- Network distance tracking (hop_distance)
- Routing preferences
- Trust propagation
- Source peer tracking

### Thread Subscriptions
```sql
CREATE TABLE thread_subscriptions (
    thread_id BLOB NOT NULL,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    notification_level INTEGER DEFAULT 1,
    PRIMARY KEY (thread_id),
    FOREIGN KEY (thread_id) REFERENCES threads(thread_id) ON DELETE CASCADE
)
```
Manages user thread subscriptions with configurable notification levels.

### Thread Views
```sql
CREATE TABLE thread_views (
    thread_id BLOB NOT NULL,
    last_viewed TIMESTAMP NOT NULL,
    last_post_seen INTEGER,
    PRIMARY KEY (thread_id),
    FOREIGN KEY (thread_id) REFERENCES threads(thread_id) ON DELETE CASCADE
)
```
Tracks user interaction with threads, supporting read tracking and position memory.

### Thread Tags
```sql
CREATE TABLE thread_tags (
    thread_id BLOB NOT NULL,
    tag TEXT NOT NULL,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (thread_id, tag),
    FOREIGN KEY (thread_id) REFERENCES threads(thread_id) ON DELETE CASCADE
)
```
Implements thread categorization and searchability through tags.

### Peer Sync
```sql
CREATE TABLE peer_sync (
    peer_id INTEGER NOT NULL,
    thread_id BLOB NOT NULL,
    last_sync TIMESTAMP NOT NULL,
    last_post_id INTEGER,
    sync_status TEXT,
    PRIMARY KEY (peer_id, thread_id),
    FOREIGN KEY (peer_id) REFERENCES peers(id) ON DELETE CASCADE,
    FOREIGN KEY (thread_id) REFERENCES threads(thread_id) ON DELETE CASCADE
)
```
Manages synchronization state between peers, tracking:
- Last sync timestamp
- Sync progress
- Per-thread sync status

### Content Blocks
```sql
CREATE TABLE content_blocks (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    block_type TEXT NOT NULL,
    block_value TEXT NOT NULL,
    reason TEXT,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    expires_at TIMESTAMP,
    UNIQUE (block_type, block_value)
)
```
Implements content moderation with:
- Flexible blocking types
- Expiration support
- Block reasoning

## Indexes

The schema includes several performance-optimizing indexes:
- `idx_posts_thread_id`: Optimizes post lookup by thread
- `idx_posts_author_id`: Optimizes post lookup by author
- `idx_images_hash`: Enables quick image deduplication
- `idx_peers_pubkey`: Optimizes peer lookup by public key
- `idx_peers_last_seen`: Supports peer activity queries
- `idx_peers_of_peers_pubkey`: Optimizes indirect peer lookup
- `idx_thread_tags_tag`: Enables efficient tag-based thread search
- `idx_peer_sync_last_sync`: Optimizes sync status queries
- `idx_content_blocks_type_value`: Enables efficient content blocking lookups

## Schema Initialization

The schema is initialized through the `InitSchema(db *sql.DB)` function, which:
1. Begins a transaction
2. Executes all schema creation statements
3. Commits the transaction or rolls back on error

## Design Considerations

The schema implements several key design patterns:
1. **Distributed Identity**: Uses BLOBs for IDs and public keys to support distributed systems
2. **Content Integrity**: Implements signatures and hashes for content verification
3. **Flexible Evolution**: Includes metadata fields for future extensions
4. **Peer Management**: Comprehensive peer tracking and trust metrics
5. **Content Moderation**: Supports content blocking and thread management
6. **Performance**: Strategic indexes for common query patterns
7. **Data Consistency**: Foreign key constraints ensure referential integrity 

## Recent Changes

### Version 1 (Current)
- Added schema versioning support
- Transitioned from images-specific to general file attachments
- Added source tracking fields for imported content
- Enhanced thread and post metadata
- Added cryptographic signature support
- Improved peer relationship tracking

## Schema Management

The schema includes several key features for maintenance and updates:
1. Version tracking through `schema_version` table
2. Safe schema reinitialization through `reinitialize_database()`
3. Automatic index creation for performance
4. Foreign key constraints for data integrity

## Design Considerations

1. **File Storage**:
   - Files are stored on disk in thread-specific directories
   - File paths are relative to the application data directory
   - Thumbnails are supported but optional

2. **Data Integrity**:
   - Foreign key constraints ensure referential integrity
   - Cascading deletes prevent orphaned records
   - Timestamps track all important events

3. **Performance**:
   - Indexes on frequently queried fields
   - Efficient file organization by thread
   - Optimized relationship tables

4. **Flexibility**:
   - Generic file support instead of image-specific
   - Metadata fields for future extensions
   - Source tracking for imported content 