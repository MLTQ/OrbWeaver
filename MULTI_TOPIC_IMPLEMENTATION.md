# Multi-Topic Discovery System - Implementation Summary

**Status**: Backend Complete ✅ | Frontend In Progress 🚧
**Date**: 2026-01-16

## Overview

Implemented a complete multi-topic discovery system that replaces the single "global" topic with user-defined topics. Users can now subscribe to any topic (like "tech", "art", "gaming") and announce threads to multiple topics simultaneously.

## ✅ Completed Backend Implementation

### 1. Database Schema (`graphchan_backend/src/database/mod.rs`)

Added two new tables:

```sql
CREATE TABLE user_topics (
    topic_id TEXT PRIMARY KEY,
    subscribed_at TEXT NOT NULL
);

CREATE TABLE thread_topics (
    thread_id TEXT NOT NULL,
    topic_id TEXT NOT NULL,
    PRIMARY KEY (thread_id, topic_id),
    FOREIGN KEY (thread_id) REFERENCES threads(id) ON DELETE CASCADE
);
```

### 2. Repository Layer (`graphchan_backend/src/database/repositories.rs`)

Implemented `TopicRepository` trait with methods:
- `subscribe(topic_id)` - Subscribe to a topic
- `unsubscribe(topic_id)` - Unsubscribe from a topic
- `list_subscribed()` - List all subscribed topics
- `is_subscribed(topic_id)` - Check if subscribed
- `add_thread_topic(thread_id, topic_id)` - Associate thread with topic
- `list_thread_topics(thread_id)` - Get topics for a thread
- `list_threads_for_topic(topic_id)` - Get threads in a topic

### 3. Topic Derivation (`graphchan_backend/src/network/topics.rs`)

Added `derive_topic_id(topic_name)` function:
- Topics are deterministic blake3 hashes of `"topic:{name}"`
- Anyone who knows the topic name can subscribe
- No secrets required - truly public discovery

### 4. REST API Endpoints (`graphchan_backend/src/api.rs`)

```
GET /topics              # List subscribed topics
POST /topics             # Subscribe to topic (body: {topic_id})
DELETE /topics/:topic_id # Unsubscribe from topic
```

### 5. Thread Creation (`graphchan_backend/src/threading.rs`)

Updated `CreateThreadInput`:
```rust
pub struct CreateThreadInput {
    pub title: String,
    pub body: Option<String>,
    pub topics: Vec<String>,  // NEW: List of topic IDs
    // ... other fields
}
```

Thread creation now saves topic associations to `thread_topics` table.

### 6. Network Layer (`graphchan_backend/src/network.rs`)

#### Auto-Subscribe on Startup
```rust
// Subscribe to all user topics for public discovery
if let Ok(topics) = database.with_repositories(|repos| repos.topics().list_subscribed()) {
    for topic_id in topics {
        handle.subscribe_to_topic(&topic_id).await?;
    }
}
```

#### New Method: `subscribe_to_topic(topic_name)`
- Creates gossip subscriptions with topic ID derived from name
- Stores broadcaster in topics map with `"topic:{name}"` key
- Spawns receiver task to forward messages to ingest loop
- Uses DHT for peer discovery (no bootstrap peers needed!)

### 7. Thread Announcements (`graphchan_backend/src/network/events.rs`)

Updated `ThreadAnnouncement`:
```rust
pub struct ThreadAnnouncement {
    pub topics: Vec<String>,  // NEW: List of topics to announce on
    // ... other fields
}
```

#### Multi-Topic Broadcasting
Event loop now broadcasts thread announcements to ALL topics:
```rust
if let EventPayload::ThreadAnnouncement(ref announcement) = payload {
    if !announcement.topics.is_empty() {
        for topic_id in &announcement.topics {
            let topic_name = format!("topic:{}", topic_id);
            broadcast_to_topic(&gossip, &topics, &topic_name, payload.clone()).await;
        }
    }
}
```

## 🏗️ Architecture Highlights

### Topic-Based Discovery Model

```
┌─────────────┐
│  User A     │ subscribes to "tech", "rust"
└─────────────┘
       │
       ▼
┌─────────────────────────────────────┐
│  Gossip Network (iroh-gossip)       │
│  - topic:tech (DHT-discovered)      │
│  - topic:rust (DHT-discovered)      │
└─────────────────────────────────────┘
       ▲
       │
┌─────────────┐
│  User B     │ subscribes to "tech", "gaming"
└─────────────┘
       │
       ▼
Creates thread "Cool Rust Project"
Announces to ["tech", "rust"]
       │
       ▼
User A receives announcement (subscribed to both)
User B receives announcement (subscribed to "tech")
```

### Key Design Decisions

1. **Well-Known Topic Hashes**: Topics use deterministic hashes (`blake3("topic:{name}")`), enabling anyone to subscribe without prior coordination

2. **DHT-Based Discovery**: Nodes discover each other through BitTorrent mainline DHT, no centralized bootstrap needed

3. **Multi-Topic Announcements**: Threads can announce to multiple topics simultaneously, maximizing discoverability

4. **Database-Backed Subscriptions**: Topic subscriptions persist across restarts, automatically re-subscribed on startup

5. **Backward Compatible**: Old `visibility` field deprecated but still supported

## 🔬 Testing Notes

### DHT Requirements
- **Won't work on localhost**: DHT needs internet-facing nodes
- **Requires different networks**: Test on separate machines or separate networks
- **Bootstrap time**: Allow 30-60 seconds for DHT to propagate topic interest

### Expected Behavior
1. User A subscribes to "tech": `POST /topics {"topic_id": "tech"}`
2. User B subscribes to "tech": `POST /topics {"topic_id": "tech"}`
3. DHT connects them (may take 30-60s)
4. User A creates thread with `topics: ["tech"]`
5. Thread announces on `topic:tech`
6. User B receives announcement via gossip
7. User B can download thread via blob ticket

## 📋 Remaining Frontend Work

### 1. Frontend Models
- Update `CreateThreadInput` in `graphchan_frontend/src/models.rs`
- Add topic list response models

### 2. Topic Management UI
- New settings screen or panel for topic management
- List subscribed topics with unsubscribe buttons
- Text input to subscribe to new topics
- Display topic subscription count

### 3. Create Thread Dialog
- Add topic selector (checkboxes or multi-select dropdown)
- Search/filter for topics when user has many subscriptions
- Default to no topics selected (friends-only)
- Visual indicator for selected topics

### 4. Catalog Filtering
- Add topic filter chips/tags
- Filter threads by selected topics
- Show "All Topics" vs individual topic views
- Display which topics a thread belongs to

## 🎯 Benefits Over Single Global Topic

1. **Topic Discrimination**: Separate tech discussions from memes
2. **Better Signal/Noise**: Subscribe only to relevant topics
3. **Scalability**: Hundreds of specialized topics vs one noisy global feed
4. **Natural Communities**: Popular topics emerge organically
5. **Privacy Options**: Can still post friends-only (no topics)
6. **Spam Resistance**: Still requires peer connections via DHT

## 🚀 Next Steps

1. **Frontend Implementation**: Build the UI components listed above
2. **Testing**: Deploy to separate machines and test DHT discovery
3. **Default Topics**: Consider auto-subscribing to a starter topic like "graphchan-general"
4. **Topic Discovery**: Maybe add a "popular topics" endpoint showing active topics
5. **Topic Metadata**: Consider adding descriptions, creator, subscriber count

## 📝 API Usage Examples

### Subscribe to a topic
```bash
curl -X POST http://localhost:8080/topics \
  -H "Content-Type: application/json" \
  -d '{"topic_id": "tech"}'
```

### List subscribed topics
```bash
curl http://localhost:8080/topics
# Returns: ["tech", "rust", "art"]
```

### Create thread with topics
```bash
curl -X POST http://localhost:8080/threads \
  -H "Content-Type: application/json" \
  -d '{
    "title": "My Cool Project",
    "body": "Check this out!",
    "topics": ["tech", "rust"]
  }'
```

### Unsubscribe from topic
```bash
curl -X DELETE http://localhost:8080/topics/rust
```

## 🔍 Code Locations

| Component | File | Lines |
|-----------|------|-------|
| Database Schema | `graphchan_backend/src/database/mod.rs` | 785-817 |
| Topic Repository | `graphchan_backend/src/database/repositories.rs` | 125-134, 1970-2043 |
| Topic Derivation | `graphchan_backend/src/network/topics.rs` | 15-21 |
| API Endpoints | `graphchan_backend/src/api.rs` | 2055-2104 |
| Network Subscribe | `graphchan_backend/src/network.rs` | 417-483 |
| Thread Creation | `graphchan_backend/src/threading.rs` | 182-190 |
| Multi-Topic Broadcast | `graphchan_backend/src/network/events.rs` | 154-167 |

## 💡 Implementation Insights

### Why This Design Works

The multi-topic system leverages iroh-gossip's DHT capabilities perfectly:

1. **Deterministic Topics**: `blake3("topic:{name}")` means anyone can derive the same topic ID from a name
2. **DHT Peer Discovery**: BitTorrent mainline DHT connects nodes interested in the same topic ID
3. **Gossip Mesh**: Once connected, messages propagate efficiently through the gossip mesh
4. **No Central Authority**: Completely decentralized - no topic registry needed

### Comparison to Old "Global" System

| Aspect | Old Global | New Multi-Topic |
|--------|-----------|-----------------|
| Topics | 1 ("graphchan-global-v1") | Unlimited user-defined |
| Discovery | Friend-of-friend only | True DHT-based global |
| Signal/Noise | All content mixed | Filtered by interest |
| Scalability | Limited | Scales to hundreds of topics |
| Privacy | "Global" or "Friends" | Per-thread topic selection |

---

**Feel better soon!** 🌟 The backend is solid and ready for frontend integration. When you're back, we can build the UI and test with your friends!
