# topics.rs

## Purpose
Renders the topic manager dialog for subscribing to and managing gossip topics. Topics are like channels/subreddits that enable content discovery across the P2P network beyond direct peer connections.

## Components

### `render_topic_manager`
- **Does**: Modal window for topic subscription management
- **Interacts with**: `subscribed_topics`, topic API endpoints
- **Controls**: `show_topic_manager` boolean toggles visibility

### Subscribe Section
- **Does**: Text input to subscribe to new topics by ID
- **Interacts with**: `new_topic_input`, subscribe API
- **Examples**: "tech", "art", "gaming"

### Subscribed Topics List
- **Does**: Scrollable list of current subscriptions with unsubscribe buttons
- **Interacts with**: `subscribed_topics` HashSet
- **Display**: 📡 emoji prefix, topic ID, unsubscribe action

## Contracts

| Dependent | Expects | Breaking changes |
|-----------|---------|------------------|
| `mod.rs` (app) | `render_topic_manager(ctx)` called each frame | Signature change |
| `catalog.rs` | Opens via `show_topic_manager = true` | Field removal |
| `dialogs.rs` | Opens via `show_topic_manager = true` from create thread | Field removal |

## Layout

```
┌─ Topic Manager ─────────────────────────────────┐
│ Manage Topics                                   │
│                                                 │
│ ┌─ Subscribe to a Topic ──────────────────────┐ │
│ │ Topic ID: [tech_______] [Subscribe]         │ │
│ │ Enter a topic name like "tech", "art"...    │ │
│ └─────────────────────────────────────────────┘ │
│                                                 │
│ ─────────────────────────────────────────────── │
│ Your Topics                                     │
│                                                 │
│ 📡 tech                        [Unsubscribe]    │
│ ───────────────────────────────────────────     │
│ 📡 rust                        [Unsubscribe]    │
│ ───────────────────────────────────────────     │
│                                                 │
│ Total: 2 topic(s)                               │
│                                                 │
│                                    [Close]      │
└─────────────────────────────────────────────────┘
```

## Topic System

Topics enable discovery beyond direct peer connections:
1. User subscribes to topic (e.g., "tech")
2. Backend joins gossip topic channel
3. Threads announced to that topic appear in catalog
4. Users can filter catalog by topic

## Notes
- Topics are simple string IDs (no hierarchy)
- Subscribing joins the gossip channel for that topic
- Threads without topics are "friends-only" (only visible to direct peers)
- Topic filter chips shown in catalog view
