# agent/mod.rs

## Purpose
Core agent implementation. Orchestrates the agent loop that polls for posts, reasons about responses, posts replies, manages persona evolution, and handles private chat with the operator.

## Components

### `Agent`
- **Does**: Main agent struct holding all components
- **Fields**: client (GraphchanClient), config, state, event_tx, reasoning, image_gen, database, trajectory_engine

### `AgentState`
- **Does**: Runtime state tracking
- **Fields**: visual_state, paused, posts_this_hour, last_post_time, processed_posts (HashSet)

### `AgentVisualState`
- **Does**: UI display state
- **Variants**: Idle, Reading, Thinking, Writing, Happy, Confused, Paused

### `AgentEvent`
- **Does**: Events sent to UI
- **Variants**: StateChanged, Observation, ReasoningTrace, ActionTaken, Error

## Key Methods

### `Agent::new`
- **Does**: Initialize agent with all components
- **Creates**: ReasoningEngine, ImageGenerator (optional), AgentDatabase, TrajectoryEngine (optional)

### `Agent::run_loop`
- **Does**: Main infinite loop
- **Flow**: Check pause → Maybe evolve persona → Sleep → Check rate limit → Run cycle

### `Agent::run_cycle`
- **Does**: Single iteration of agent logic
- **Flow**: Process chat → Fetch posts → Filter own posts → Reason → Execute decision

### `Agent::reload_config`
- **Does**: Hot-reload configuration
- **Recreates**: ReasoningEngine, ImageGenerator, TrajectoryEngine

### `Agent::toggle_pause`
- **Does**: Pause/resume agent loop

### `Agent::process_chat_messages`
- **Does**: Handle private messages from operator
- **Flow**: Get unprocessed → Send to LLM → Save response → Update memory

### `Agent::maybe_evolve_persona`
- **Does**: Check if reflection is due, run evolution
- **Interval**: Configurable (default 24h)

### `Agent::run_persona_evolution`
- **Does**: Full Ludonarrative Assonantic Tracing cycle
- **Flow**: Capture snapshot → Get history → Infer trajectory → Save → Emit

### `Agent::capture_persona_snapshot`
- **Does**: Ask LLM to self-reflect on current state
- **Returns**: PersonaSnapshot with traits and self-description

## Contracts

| Dependent | Expects | Breaking changes |
|-----------|---------|------------------|
| `main.rs` | `Agent::new`, `Agent::run_loop` | Signatures |
| `ui/app.rs` | `Agent::toggle_pause`, `Agent::reload_config` | Method removal |

## Agent Loop Flow

```
┌─────────────────────────────────────────────────┐
│                  run_loop()                      │
└─────────────────────────────────────────────────┘
                      │
                      ▼
              ┌───────────────┐
              │ Check paused? │──Yes──► Sleep 5s ──┐
              └───────────────┘                    │
                      │ No                         │
                      ▼                            │
              ┌───────────────┐                    │
              │ Maybe evolve  │                    │
              │    persona    │                    │
              └───────────────┘                    │
                      │                            │
                      ▼                            │
              ┌───────────────┐                    │
              │ Sleep poll_   │                    │
              │   interval    │                    │
              └───────────────┘                    │
                      │                            │
                      ▼                            │
              ┌───────────────┐                    │
              │  run_cycle()  │                    │
              └───────────────┘                    │
                      │                            │
                      └────────────────────────────┘
```

## Decision Handling

| Decision | Action |
|----------|--------|
| Reply | Create post with parent reference |
| UpdateMemory | Save to working memory |
| ChatReply | Save agent response to chat |
| NoAction | Mark posts as processed |

## Notes
- Rate limiting: max_posts_per_hour from config
- Processed posts tracked to avoid re-analysis
- Agent metadata added to posts to identify own posts
- Event channel (flume) for UI updates
