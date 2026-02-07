# ui/chat.rs

## Purpose
Rendering functions for the event log and private chat interface. Displays agent activity and enables operator-agent communication.

## Components

### `render_event_log`
- **Does**: Display agent activity events
- **Inputs**: ui, &[AgentEvent]
- **Events rendered**:
  - Observation → Light blue text
  - ReasoningTrace → Grouped bullet points (gray)
  - ActionTaken → Green with checkmark
  - Error → Red with X
  - StateChanged → (not shown in log)

### `render_private_chat`
- **Does**: Display operator↔agent chat history
- **Inputs**: ui, &[ChatMessage]
- **Layout**: Operator messages right-aligned, agent left-aligned
- **Colors**:
  - Operator: Blue header, dark blue background
  - Agent: Green header, dark green background

## UI Styling

### Event Log
```
📖 Checking for recent posts...

💭 Reasoning:
  • Post is interesting
  • I have something to add

✅ Posted reply: Post ID: abc123

❌ Error: Connection failed
```

### Private Chat
```
┌────────────────────────────────────────┐
│                        ┌──────────────┐│
│                        │ You    12:34 ││
│                        │ Hello agent  ││
│                        │ ⏳ Waiting...││
│                        └──────────────┘│
│ ┌──────────────┐                       │
│ │ Agent  12:35 │                       │
│ │ Hello! How   │                       │
│ │ can I help?  │                       │
│ └──────────────┘                       │
└────────────────────────────────────────┘
```

## Contracts

| Dependent | Expects | Breaking changes |
|-----------|---------|------------------|
| `ui/app.rs` | Both render functions | Signature changes |
| `agent/mod.rs` | `AgentEvent` variants | Enum changes |
| `database.rs` | `ChatMessage` struct | Field changes |

## Notes
- ScrollArea sticks to bottom for auto-scroll
- Max height calculated from available space minus input area
- Unprocessed operator messages show "Waiting for agent..." indicator
- Time displayed in HH:MM format from message timestamp
