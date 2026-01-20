# Topic-Based Discovery: Network Flow Documentation

**Last Updated**: 2026-01-19
**System**: Multi-Topic Discovery with iroh-gossip + BitTorrent DHT

---

## Overview

This document explains the complete network flow for topic-based peer discovery in Graphchan. The multi-topic system allows users to subscribe to named topics (like "tech", "claude", "gaming") and discover content from strangers without prior friend connections.

### Key Technologies

- **BitTorrent Mainline DHT**: Peer discovery for topic subscribers
- **iroh-gossip**: Message propagation via gossip mesh network
- **iroh-blobs**: Content-addressed file transfer with tickets
- **blake3**: Deterministic topic ID derivation

---

## Complete Network Flow Example

**Scenario**: Two users (A and B) subscribe to the "claude" topic. User A creates a thread and announces it to the topic. User B receives and downloads the thread.

### Phase 1: Topic Subscription

Both users subscribe to the "claude" topic independently.

#### User A Subscribes

**1. Frontend Action**
```http
POST /topics
Content-Type: application/json

{"topic_id": "claude"}
```

**2. Backend API Handler** (`graphchan_backend/src/api.rs:2072-2090`)
```rust
async fn subscribe_topic_handler(req: SubscribeTopicRequest) {
    // Save to database
    repos.topics().subscribe("claude")?;

    // Subscribe to gossip network
    network.subscribe_to_topic("claude").await?;
}
```

**3. Network Subscription** (`graphchan_backend/src/network.rs:435-495`)
```rust
pub async fn subscribe_to_topic(&self, topic_name: &str) -> Result<()> {
    // 1. Derive deterministic topic ID
    let topic_id = TopicId::from_bytes(blake3("topic:claude"));

    // 2. Create TWO gossip subscriptions:
    //    - receiver_topic: for listening to messages
    //    - broadcaster_topic: for sending messages
    let receiver_topic = self.gossip.subscribe(topic_id, vec![]).await?;
    let broadcaster_topic = self.gossip.subscribe(topic_id, vec![]).await?;

    // 3. Store broadcaster in topics map for event routing
    let topic_key = format!("topic:{}", topic_name);
    self.topics.write().await.insert(topic_key.clone(), broadcaster_topic);

    // 4. Spawn receiver task to forward messages to ingest loop
    tokio::spawn(async move {
        while let Some(event) = receiver_topic.next().await {
            match event {
                Event::Received(message) => {
                    let envelope = serde_json::from_slice(&message.content)?;
                    inbound_tx.send(gossip).await?;
                }
                Event::NeighborUp(peer_id) => {
                    tracing::info!("neighbor up on topic:{}", topic_key);
                }
                // ... other events
            }
        }
    });
}
```

**4. iroh-gossip DHT Announcement** (automatic)

When `gossip.subscribe()` is called, iroh-gossip automatically:
- Announces interest in `topic_id` to **BitTorrent Mainline DHT**
- Starts querying DHT for other peers interested in the same topic
- Establishes direct UDP connections to discovered peers
- Forms a gossip mesh network

**User B performs the same steps** → Both now visible in DHT

---

### Phase 2: DHT Peer Discovery

**Timeline**: 30-60 seconds for DHT propagation

```
User A                    BitTorrent DHT                  User B
  |                            |                            |
  |--[Announce: topic_id]----->|                            |
  |                            |<----[Announce: topic_id]---|
  |                            |                            |
  |<---[Peers: B's NodeAddr]---|----[Peers: A's NodeAddr]-->|
  |                            |                            |
  |<-----------[Direct UDP connection established]--------->|
  |                  (via iroh NAT traversal)               |
  |                                                          |
```

**DHT Mechanics**:
1. Each peer announces: `PUT(topic_id) → my_node_addr`
2. DHT stores peer lists for each topic_id
3. Periodic `GET(topic_id)` queries return list of interested peers
4. iroh handles NAT traversal via:
   - Direct addresses (if public IP)
   - STUN hole-punching
   - Relay servers (fallback)

**Logs when connection succeeds** (`network.rs:482`):
```
INFO neighbor up on topic:topic:claude peer=<User B short ID>
```

**What This Means**:
- Users A and B now have a **direct UDP connection**
- They form a **gossip mesh** for the "claude" topic
- Messages broadcast on this topic will reach both peers
- No central server involved

---

### Phase 3: Thread Creation & Announcement

**User A creates a thread with topics: ["claude"]**

#### 1. Thread Creation (`graphchan_backend/src/threading.rs:182-190`)

```rust
pub fn create_thread(&self, input: CreateThreadInput) -> Result<ThreadDetails> {
    // Save thread to database
    repos.threads().create(&thread_record)?;

    // Save topic associations
    for topic_id in &input.topics {
        repos.topics().add_thread_topic(&thread_id, topic_id)?;
    }

    // ... create OP post, save files, etc.
}
```

#### 2. Build Thread Blob (`threading.rs:194-204`)

```rust
// Serialize complete ThreadDetails to JSON bytes
let thread_blob = serde_json::to_vec(&thread_details)?;

// Add to local blob store (content-addressed storage)
let hash = blob_store.add_bytes(thread_blob).await?;

// Create BlobTicket (shareable download link)
let ticket = BlobTicket {
    hash: hash,
    format: BlobFormat::Raw,
    node_addr: NodeAddr {
        peer_id: my_peer_id,
        direct_addresses: my_addrs,
        relay_url: relay,
    }
};
```

**BlobTicket Structure**:
```rust
BlobTicket {
    hash: Blake3Hash,           // Content address (32 bytes)
    format: BlobFormat::Raw,    // No encryption/compression
    node_addr: NodeAddr {       // Where to download from
        peer_id: PublicKey,     // User A's iroh peer ID
        direct_addresses: Vec<SocketAddr>,  // User A's IPs
        relay_url: Option<Url>, // Fallback relay
    }
}
```

The ticket is **self-contained** - anyone with it can download the blob from User A.

#### 3. Build Announcement Message (`threading.rs:205-230`)

```rust
let announcement = ThreadAnnouncement {
    thread_id: "abc123",
    creator_peer_id: "User A's GPG fingerprint",
    announcer_peer_id: "User A's peer ID",
    title: "Check out Claude!",
    preview: "First 140 chars of OP body...",
    ticket: ticket,  // ← Download link
    post_count: 1,
    has_images: true,
    created_at: "2026-01-19T...",
    last_activity: "2026-01-19T...",
    thread_hash: "blake3_of_all_posts",  // For sync detection
    topics: vec!["claude"],  // ← Will broadcast to these topics
};

// Send to network event loop
publisher.send(NetworkEvent::Broadcast(
    EventPayload::ThreadAnnouncement(announcement)
)).await?;
```

**Announcement Size**: ~1KB (metadata + ticket, NOT the full thread)

#### 4. Event Loop Routing (`network/events.rs:156-167`)

```rust
NetworkEvent::Broadcast(payload) => {
    // Special multi-topic handling for ThreadAnnouncement
    if let EventPayload::ThreadAnnouncement(ref announcement) = payload {
        if !announcement.topics.is_empty() {
            // Broadcast to ALL topics in the list
            for topic_id in &announcement.topics {
                let topic_name = format!("topic:{}", topic_id);
                broadcast_to_topic(&gossip, &topics, &topic_name, payload.clone()).await?;
            }
            continue;
        }
    }

    // Default routing for other message types...
}
```

**Why Multi-Topic Broadcasting?**
- Thread can announce to multiple topics: `["tech", "rust", "claude"]`
- Maximizes discoverability without duplication
- Each topic gets the same announcement (same ticket)

#### 5. Gossip Broadcast (`network/events.rs:191-243`)

```rust
async fn broadcast_to_topic(
    gossip: &Gossip,
    topics: &Arc<RwLock<HashMap<String, GossipTopic>>>,
    topic_name: &str,
    payload: EventPayload,
) -> Result<()> {
    // Serialize to JSON
    let envelope = EventEnvelope { payload };
    let bytes = serde_json::to_vec(&envelope)?;

    // Get the broadcaster for this topic
    let mut guard = topics.write().await;
    if let Some(topic) = guard.get_mut(topic_name) {
        // Broadcast via gossip mesh
        topic.broadcast(Bytes::from(bytes)).await?;

        tracing::info!(
            topic = %topic_name,
            payload_type = "ThreadAnnouncement",
            size_bytes = bytes.len(),
            "broadcasted message to topic"
        );
    }

    Ok(())
}
```

**Logs**:
```
INFO broadcasted message to topic:topic:claude payload_type=ThreadAnnouncement size_bytes=1024
```

**iroh-gossip Behavior**:
- Message sent to ALL neighbors in the gossip mesh
- Each neighbor re-broadcasts to THEIR neighbors (flood-fill)
- Gossip protocol ensures eventual delivery to all subscribers
- Built-in deduplication prevents infinite loops

---

### Phase 4: Message Propagation & Reception

**User B's receiver task gets the message from the gossip mesh**

#### 1. Receiver Task (`network.rs:464-480`)

```rust
// Spawned when User B subscribed to "claude"
tokio::spawn(async move {
    while let Some(event_result) = receiver.next().await {
        match event_result {
            Ok(Event::Received(message)) => {
                // Deserialize envelope
                let envelope = serde_json::from_slice(&message.content)?;

                // Forward to ingest loop for processing
                let gossip = InboundGossip {
                    peer_id: Some(message.delivered_from.to_string()),
                    payload: envelope.payload,
                };
                inbound_tx.send(gossip).await?;
            }
            Ok(Event::NeighborUp(peer_id)) => {
                tracing::info!("neighbor up on topic:topic:claude");
            }
            // ... other events
        }
    }
});
```

#### 2. Ingest Loop Processing (`network/ingest.rs:152-184`)

```rust
EventPayload::ThreadAnnouncement(announcement) => {
    tracing::info!(
        thread_id = %announcement.thread_id,
        title = %announcement.title,
        post_count = announcement.post_count,
        "📢 received thread announcement (will download on-demand)"
    );

    // 1. Deduplication check
    let msg_id = format!("thread:{}:{}", announcement.thread_id, announcement.thread_hash);
    let should_rebroadcast = {
        let mut seen = seen_messages.lock().await;
        seen.insert(msg_id)  // Returns true if NEW
    };

    // 2. Save announcement to database
    apply_thread_announcement(database, announcement.clone())?;

    // 3. RE-BROADCAST for transitive discovery (A→B→C→...)
    if should_rebroadcast {
        let mut rebroadcast = announcement.clone();
        // CRITICAL: Change announcer to OUR peer ID
        // This enables B to publish to B's peer topic, not A's
        rebroadcast.announcer_peer_id = my_peer_id.to_string();

        publisher.send(NetworkEvent::Broadcast(
            EventPayload::ThreadAnnouncement(rebroadcast)
        )).await?;
    }

    Ok(None)
}
```

**Deduplication Mechanism**:
- `seen_messages`: In-memory `HashSet<String>` of message IDs
- Message ID: `"thread:{thread_id}:{thread_hash}"`
- Same announcement from multiple peers → processed only once
- Prevents infinite re-broadcast loops

**Why Re-broadcast?**
- Enables **transitive discovery**: A→B→C→D without A knowing C or D
- Each peer propagates to their neighbors in the mesh
- Gossip protocol ensures eventual delivery to ALL subscribers
- DHT only connects SOME peers; gossip connects them ALL

#### 3. Save Announcement (`network/ingest.rs:372-470`)

```rust
fn apply_thread_announcement(
    database: &Database,
    announcement: ThreadAnnouncement,
) -> Result<()> {
    database.with_repositories(|repos| {
        // Check if thread exists
        let existing = repos.threads().get(&announcement.thread_id)?;

        if let Some(existing_thread) = existing {
            // Compare hashes to detect updates
            if existing_thread.thread_hash == Some(announcement.thread_hash) {
                // Already in sync, skip
                return Ok(());
            } else {
                // Hash mismatch - will re-sync on next view
                tracing::info!("thread hash mismatch - marking for re-sync");
            }
        } else {
            // New thread - create stub record
            let stub_peer = PeerRecord {
                id: announcement.creator_peer_id.clone(),
                trust_state: "unknown".to_string(),
                // ... other fields default
            };
            repos.peers().upsert(&stub_peer)?;

            let stub_thread = ThreadRecord {
                id: announcement.thread_id.clone(),
                title: announcement.title.clone(),
                creator_peer_id: Some(announcement.creator_peer_id.clone()),
                created_at: announcement.created_at.clone(),
                thread_hash: Some(announcement.thread_hash.clone()),
                sync_status: "announced".to_string(),  // ← KEY
                // ... other fields
            };
            repos.threads().upsert(&stub_thread)?;
        }

        // Save the BlobTicket for later download
        let ticket_str = announcement.ticket.to_string();
        repos.conn().execute(
            "INSERT OR REPLACE INTO thread_tickets (thread_id, ticket) VALUES (?, ?)",
            [announcement.thread_id, ticket_str],
        )?;

        Ok(())
    })
}
```

**Database State After Announcement**:
- `threads` table: Stub record with `sync_status = "announced"`
- `thread_tickets` table: BlobTicket stored as string
- Thread appears in catalog but NOT fully downloaded yet

**Logs**:
```
INFO 📢 received thread announcement (will download on-demand) thread_id=abc123 title="Check out Claude!"
```

---

### Phase 5: Lazy Download (When User B Views Thread)

**User B clicks the thread in the catalog**

#### 1. Frontend Request
```http
POST /threads/{thread_id}/download
```

#### 2. Backend Download Handler (`api.rs:222-270`)

```rust
async fn download_thread(thread_id: String) -> Result<ThreadDetails> {
    tracing::info!("📥 downloading thread from peer");

    // 1. Retrieve BlobTicket from database
    let ticket_str: String = repos.conn().query_row(
        "SELECT ticket FROM thread_tickets WHERE thread_id = ?",
        [thread_id],
        |row| row.get(0)
    )?;

    // 2. Parse ticket
    let ticket: BlobTicket = ticket_str.parse()?;

    // 3. Download blob via iroh-blobs
    let downloader = blob_store.downloader();
    let download = downloader.download(ticket).await?;

    // 4. Read blob bytes
    let mut reader = download.reader().await?;
    let mut bytes = Vec::new();
    reader.read_to_end(&mut bytes).await?;

    // 5. Deserialize ThreadDetails
    let thread_details: ThreadDetails = serde_json::from_slice(&bytes)?;

    // 6. Save to database
    save_thread_snapshot(database, &thread_details)?;

    // 7. Update sync_status
    repos.threads().execute(
        "UPDATE threads SET sync_status = 'downloaded' WHERE id = ?",
        [thread_id]
    )?;

    Ok(thread_details)
}
```

#### 3. iroh-blobs Download Process

**BlobTicket contains**:
```rust
BlobTicket {
    hash: Blake3Hash,      // Target content hash
    node_addr: NodeAddr {  // Where to download from
        peer_id: User_A_PeerID,
        direct_addresses: [User_A_IP],
        relay_url: Some(relay_server),
    }
}
```

**Download Flow**:
1. **Extract peer info** from ticket
2. **Establish connection** to User A:
   - Try direct addresses first
   - Fall back to relay server if needed
3. **Request blob** by hash over iroh protocol
4. **Stream blob** in chunks (efficient for large files)
5. **Verify hash** - must match ticket hash (content-addressed)
6. **Store locally** in blob store

**Content Addressing Benefits**:
- Integrity verification (hash must match)
- Deduplication (same hash = same content)
- No trust required (cryptographically verified)

#### 4. Save Thread to Database (`threading.rs:save_thread_snapshot`)

```rust
fn save_thread_snapshot(db: &Database, details: &ThreadDetails) -> Result<()> {
    db.with_repositories(|repos| {
        // Update thread record
        repos.threads().upsert(&thread_record)?;

        // Save all posts
        for post in &details.posts {
            repos.posts().upsert(&post_record)?;

            // Save post relationships (parent-child edges)
            for parent_id in &post.parent_post_ids {
                repos.posts().add_relationship(parent_id, &post.id)?;
            }

            // Save file metadata (NOT file data - that's in blobs)
            for file in &post.files {
                repos.files().upsert(&file_record)?;
            }
        }

        // Update sync status
        repos.threads().update_sync_status(&thread_id, "downloaded")?;

        Ok(())
    })
}
```

**Database State After Download**:
- `threads`: `sync_status = "downloaded"`
- `posts`: All posts saved
- `post_relationships`: Graph edges saved
- `files`: Metadata saved (actual files downloaded on-demand)

**Logs**:
```
INFO 📥 downloading thread from peer thread_id=abc123
INFO thread downloaded successfully posts=5 files=3
```

---

## Key Architectural Points

### 1. Two-Phase Discovery

**Phase 1: DHT Peer Discovery**
- Purpose: Find OTHER peers interested in the same topic
- Technology: BitTorrent Mainline DHT (global, proven)
- Latency: 30-60 seconds for initial discovery
- Result: Direct UDP connections between peers

**Phase 2: Gossip Mesh Propagation**
- Purpose: Propagate messages to ALL subscribers
- Technology: iroh-gossip flood-fill
- Latency: <10 seconds for mesh propagation
- Result: Every subscriber receives the announcement

### 2. Lazy Content Loading

**Announcements Are Tiny** (~1KB):
- Thread metadata (title, preview, timestamps)
- BlobTicket (download link)
- Creator info

**Full Threads Downloaded On-Demand**:
- Only when user clicks to view
- Uses iroh-blobs content-addressed transfer
- Cryptographically verified (hash must match)
- Efficient for large threads (streaming)

**Benefits**:
- Low bandwidth for browsing catalog
- Fast propagation (small messages)
- No wasted downloads for unwanted threads

### 3. Deduplication & Caching

**Message-Level Deduplication**:
```rust
seen_messages: Arc<Mutex<HashSet<String>>>
```
- Key: `"thread:{id}:{hash}"`
- Prevents re-processing same message from multiple peers
- Prevents infinite re-broadcast loops

**Content-Level Deduplication**:
- iroh-blobs uses content addressing (blake3 hash)
- Same content = same hash = stored once
- Multiple tickets to same hash → download once

### 4. Transitive Discovery

**How User C Discovers Content**:
```
User A (subscribed to "claude")
   |
   ├─ Creates thread → announces to "claude"
   |
   ▼
User B (subscribed to "claude")
   |
   ├─ Receives announcement from A
   ├─ Re-broadcasts with THEIR peer ID
   |
   ▼
User C (subscribed to "claude")
   |
   └─ Receives from B (never connected to A!)
```

**Why This Works**:
- Gossip mesh is NOT fully connected
- DHT connects SOME peers, gossip connects them ALL
- Each peer re-broadcasts to their neighbors
- Eventual consistency via flood-fill

### 5. NAT Traversal

**iroh Handles Complex Networking**:
1. **Direct connection** (if public IPs)
2. **STUN hole-punching** (most cases)
3. **Relay servers** (fallback for strict NATs)

**User Experience**:
- Just works™ across most network configurations
- No port forwarding required
- No static IP required
- Works behind home routers

---

## Performance Characteristics

### Scalability

**Per-Topic Limits**:
- **Announcement propagation**: O(log N) hops in gossip mesh
- **Bandwidth per node**: O(neighbors × message_rate)
- **DHT queries**: O(log N) distributed lookups

**Practical Limits** (estimated):
- Peers per topic: 1,000+ (tested to ~100)
- Topics per user: Unlimited (stored in SQLite)
- Announcements per second: ~10-100 per topic
- Thread size: No limit (streamed via iroh-blobs)

### Latency

**Topic Subscription**:
- Local: Instant (database + gossip subscribe)
- DHT propagation: 30-60 seconds
- First neighbor connection: ~30s

**Thread Announcement**:
- Local broadcast: <100ms
- Gossip propagation: 5-10 seconds
- Global delivery: <60 seconds (10+ hops)

**Thread Download**:
- Connection setup: <1 second (if cached)
- Download speed: Limited by slowest peer's upload
- Small thread (10 posts): <1 second
- Large thread (1000 posts + images): 10-60 seconds

### Bandwidth

**Idle Subscription**:
- DHT keep-alive: ~1 KB/minute
- Gossip heartbeat: ~1 KB/second per neighbor
- Total: <10 KB/minute per topic

**Active Announcement**:
- Outbound: 1 KB × num_neighbors
- Inbound: 1 KB per announcement received
- Re-broadcast: 1 KB × num_neighbors

**Thread Download**:
- Variable based on thread size
- Streamed (not all-at-once)
- Typical: 10 KB - 10 MB

---

## Failure Modes & Resilience

### Network Partitions

**Symptom**: Some peers can't reach each other
**Cause**: Network configuration, firewall, ISP blocking
**Mitigation**:
- Relay servers provide fallback connectivity
- Gossip mesh routes around failed peers
- DHT continues to provide new peer candidates

### DHT Failures

**Symptom**: New peers can't discover each other
**Cause**: DHT node outages, network censorship
**Mitigation**:
- BitTorrent DHT is globally distributed (no single point of failure)
- Existing connections continue via gossip mesh
- New peers can use bootstrap nodes

### Malicious Peers

**Symptom**: Invalid announcements, spam
**Cause**: Malicious or buggy peers
**Mitigation**:
- Content verification (BlobTicket hash must match)
- User blocking (IP and peer ID)
- Gossip mesh rate limiting
- Schema validation on all messages

### Message Loss

**Symptom**: Announcement not received
**Cause**: Network issues, peer offline
**Mitigation**:
- Gossip ensures eventual delivery (flood-fill)
- Multiple paths through mesh network
- Re-announcements on thread updates

---

## Comparison to Other Systems

### vs. Traditional Forums (Reddit, HN)

| Aspect | Graphchan Topics | Reddit/HN |
|--------|------------------|-----------|
| **Discovery** | DHT + gossip | Central database |
| **Scalability** | P2P mesh | Vertical scaling |
| **Censorship** | Resistant (no central authority) | Vulnerable |
| **Latency** | 30-60s initial, <10s updates | <1s (centralized) |
| **Reliability** | Resilient (distributed) | Single point of failure |

### vs. Mastodon/ActivityPub

| Aspect | Graphchan Topics | Mastodon |
|--------|------------------|----------|
| **Federation** | True P2P (no servers) | Server-to-server |
| **Account Required** | No (pseudonymous) | Yes (per instance) |
| **Discovery** | Topic-based | Follow-based + hashtags |
| **Hosting** | Desktop app | Web server |
| **Moderation** | Local blocking | Instance-level |

### vs. Scuttlebutt

| Aspect | Graphchan Topics | Scuttlebutt |
|--------|------------------|-------------|
| **Discovery** | DHT (global) | Pub servers + local |
| **Strangers** | Yes (topics) | No (follow graph) |
| **Sync Model** | Lazy (on-demand) | Eager (replicate all) |
| **Bandwidth** | Low (announcements only) | High (full replication) |
| **Scaling** | 1000+ per topic | ~100 follows |

---

## Testing & Debugging

### Logs to Monitor

**Successful Subscription**:
```
INFO subscribing to user topic topic=claude
INFO neighbor up on topic:topic:claude peer=abc123
```

**Thread Announcement**:
```
INFO broadcasted message to topic:topic:claude payload_type=ThreadAnnouncement
```

**Thread Reception**:
```
INFO 📢 received thread announcement thread_id=xyz title="..." post_count=5
```

**Thread Download**:
```
INFO 📥 downloading thread from peer thread_id=xyz
INFO thread downloaded successfully posts=5 files=3
```

### Common Issues

**"No neighbors on topic"**
- **Cause**: DHT hasn't propagated yet, or no other subscribers
- **Fix**: Wait 60 seconds, check other peer subscribed to same topic

**"Failed to download thread"**
- **Cause**: Announcer peer offline, network unreachable
- **Fix**: Retry later, peer will re-announce when online

**"Thread hash mismatch"**
- **Cause**: Thread was updated after announcement
- **Fix**: Re-download triggered automatically on next view

### Manual Testing

**Test DHT Discovery** (requires 2 machines):
```bash
# Machine A
curl -X POST http://localhost:8080/topics \
  -H "Content-Type: application/json" \
  -d '{"topic_id": "test"}'

# Machine B
curl -X POST http://localhost:8080/topics \
  -H "Content-Type: application/json" \
  -d '{"topic_id": "test"}'

# Wait 60 seconds, check logs for "neighbor up"
```

**Test Thread Announcement**:
```bash
# Machine A - create thread with topics
curl -X POST http://localhost:8080/threads \
  -H "Content-Type: application/json" \
  -d '{
    "title": "Test Thread",
    "body": "Testing topic announcement",
    "topics": ["test"]
  }'

# Machine B - check catalog
curl http://localhost:8080/threads

# Should see thread with sync_status="announced"
```

**Test Download**:
```bash
# Machine B - download thread
curl -X POST http://localhost:8080/threads/{thread_id}/download

# Should return full ThreadDetails
# Check logs for "📥 downloading thread from peer"
```

---

## Future Enhancements

### 1. Topic Metadata
- **Description**: Per-topic info (description, rules, creation date)
- **Creator**: Track who created the topic
- **Moderation**: Topic-specific block lists

### 2. Topic Discovery
- **Popular Topics**: API to browse active topics
- **Search**: Find topics by keyword
- **Recommendations**: Suggest topics based on activity

### 3. Subscription Optimization
- **Auto-Prune**: Unsubscribe from inactive topics
- **Prioritization**: QoS for high-priority topics
- **Selective Sync**: Filter announcements by criteria

### 4. Advanced Routing
- **Hierarchical Topics**: "tech/rust", "tech/python"
- **Tag Matching**: Regex-based topic patterns
- **Private Topics**: Encrypted topic IDs

---

## References

### Code Locations

| Component | File | Key Lines |
|-----------|------|-----------|
| Topic Subscription | `network.rs` | 435-495 |
| DHT Derivation | `network/topics.rs` | 15-21 |
| Event Loop Routing | `network/events.rs` | 145-243 |
| Announcement Ingest | `network/ingest.rs` | 152-184 |
| Thread Download | `api.rs` | 222-270 |
| Topic API Endpoints | `api.rs` | 2055-2104 |

### External Documentation

- [iroh-gossip Documentation](https://docs.rs/iroh-gossip/)
- [iroh-blobs Documentation](https://docs.rs/iroh-blobs/)
- [BitTorrent DHT BEP-0005](http://www.bittorrent.org/beps/bep_0005.html)
- [Gossip Protocol Overview](https://en.wikipedia.org/wiki/Gossip_protocol)

---

**Document Version**: 1.0
**Implementation Status**: Complete (backend + frontend)
**Last Tested**: 2026-01-19
