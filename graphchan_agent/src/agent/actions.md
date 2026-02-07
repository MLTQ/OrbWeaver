# agent/actions.rs

## Purpose
Action execution for the agent. Provides a simple interface for posting replies to threads via the Graphchan API.

## Components

### `ActionExecutor`
- **Does**: Wraps GraphchanClient for action execution
- **Fields**: client (GraphchanClient)

### `ActionExecutor::new`
- **Does**: Create executor with client
- **Inputs**: GraphchanClient

### `ActionExecutor::post_reply`
- **Does**: Create a reply post in a thread
- **Inputs**: thread_id, body, parent_ids
- **Returns**: Result<String> (new post ID)
- **Note**: Does not include agent metadata (caller should add)

## Contracts

| Dependent | Expects | Breaking changes |
|-----------|---------|------------------|
| (currently unused - agent/mod.rs posts directly) | N/A | N/A |

## Notes
- Minimal implementation - most action logic is in agent/mod.rs
- Could be extended for other actions (reactions, file uploads, etc.)
- Metadata handling moved to caller for flexibility
