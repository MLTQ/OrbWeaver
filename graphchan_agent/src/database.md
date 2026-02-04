# database.rs

## Purpose
SQLite-based persistence for agent memory, persona evolution, working memory, and private chat. Implements the storage layer for "Ludonarrative Assonantic Tracing" - tracking personality evolution over time.

## Components

### `AgentDatabase`
- **Does**: Main database handle with thread-safe connection
- **Fields**: conn (Mutex<Connection>)
- **Thread safety**: Uses Mutex for safe concurrent access

### `ImportantPost`
- **Does**: Posts marked as formative experiences
- **Fields**: id, post_id, thread_id, post_body, why_important, importance_score (0.0-1.0), marked_at

### `PersonaSnapshot`
- **Does**: Captures agent personality at a point in time
- **Fields**: id, captured_at, traits (PersonaTraits), system_prompt, trigger, self_description, inferred_trajectory, formative_experiences
- **Use case**: Core data for personality trajectory tracking

### `PersonaTraits`
- **Does**: Dynamic personality dimensions map
- **Fields**: dimensions (HashMap<String, f64>)
- **Design**: Not hardcoded - defined by guiding_principles and LLM

### `ReflectionRecord`
- **Does**: Records of self-reflection events
- **Fields**: id, reflected_at, old_prompt, new_prompt, reasoning, guiding_principles

### `CharacterCard`
- **Does**: Imported character card storage
- **Fields**: id, imported_at, format, original_data, derived_prompt, name, description

### `WorkingMemoryEntry`
- **Does**: Agent's persistent scratchpad
- **Fields**: key, content, updated_at

### `ChatMessage`
- **Does**: Private operator↔agent messages
- **Fields**: id, role ("operator"/"agent"), content, created_at, processed

## Key Methods

### Memory Operations
- `save_important_post`, `get_recent_important_posts`, `delete_important_post`
- `set_working_memory`, `get_working_memory`, `get_working_memory_context`

### Persona Operations
- `save_persona_snapshot`, `get_persona_history`, `get_latest_persona`
- `save_reflection`, `get_reflection_history`

### Chat Operations
- `add_chat_message`, `get_unprocessed_operator_messages`, `mark_message_processed`
- `get_chat_history`, `get_chat_context`

### State Operations
- `get_state`, `set_state` - Generic key-value storage
- `get_current_system_prompt`, `set_current_system_prompt`
- `get_last_reflection_time`, `set_last_reflection_time`

## Contracts

| Dependent | Expects | Breaking changes |
|-----------|---------|------------------|
| `agent/mod.rs` | All CRUD methods | Method signatures |
| `agent/trajectory.rs` | `PersonaSnapshot`, `PersonaTraits` | Struct changes |
| `ui/app.rs` | `get_chat_history` | Return type |

## Schema

```sql
-- Important posts (formative experiences)
CREATE TABLE important_posts (
    id TEXT PRIMARY KEY,
    post_id TEXT NOT NULL,
    thread_id TEXT NOT NULL,
    post_body TEXT NOT NULL,
    why_important TEXT NOT NULL,
    importance_score REAL NOT NULL,
    marked_at TEXT NOT NULL
);

-- Persona history for trajectory tracking
CREATE TABLE persona_history (
    id TEXT PRIMARY KEY,
    captured_at TEXT NOT NULL,
    traits_json TEXT NOT NULL,
    system_prompt TEXT NOT NULL,
    trigger TEXT NOT NULL,
    self_description TEXT NOT NULL,
    inferred_trajectory TEXT,
    formative_experiences_json TEXT NOT NULL
);

-- Working memory (scratchpad)
CREATE TABLE working_memory (
    key TEXT PRIMARY KEY,
    content TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

-- Private chat
CREATE TABLE chat_messages (
    id TEXT PRIMARY KEY,
    role TEXT NOT NULL,
    content TEXT NOT NULL,
    created_at TEXT NOT NULL,
    processed INTEGER NOT NULL DEFAULT 0
);
```

## Notes
- Uses WAL mode for concurrent read access from UI thread
- Traits stored as JSON for flexible personality dimensions
- Working memory provides context to LLM during reasoning
- Chat enables operator to guide agent behavior
