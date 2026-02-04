# agent/reasoning.rs

## Purpose
LLM-based reasoning engine for the agent. Analyzes posts, decides whether to respond, generates replies, handles private chat, and manages working memory updates. Includes robust JSON extraction from LLM responses.

## Components

### `ReasoningEngine`
- **Does**: LLM client for agent reasoning
- **Fields**: client (reqwest), api_url, model, api_key, system_prompt

### `Decision`
- **Does**: Possible actions from reasoning
- **Variants**:
  - `Reply { post_id, content, reasoning }`
  - `NoAction { reasoning }`
  - `UpdateMemory { key, content, reasoning }`
  - `ChatReply { content, reasoning, memory_update }`

## Key Methods

### `analyze_posts`
- **Does**: Basic post analysis (no context)
- **Inputs**: &[RecentPostView]
- **Returns**: Result<Decision>

### `analyze_posts_with_context`
- **Does**: Full analysis with working memory and chat context
- **Inputs**: posts, working_memory_context, chat_context
- **Returns**: Result<Decision>

### `process_chat`
- **Does**: Respond to private operator messages
- **Inputs**: &[ChatMessage], working_memory_context
- **Returns**: Result<Decision> (ChatReply or NoAction)

### `call_llm`
- **Does**: Send messages to LLM API
- **Format**: OpenAI-compatible chat completions
- **Returns**: Result<String>

### `parse_decision`
- **Does**: Parse LLM JSON response to Decision
- **Handles**: reply, update_memory, none actions

### `parse_chat_decision`
- **Does**: Parse chat response to Decision
- **Handles**: chat_reply action

## JSON Extraction Functions

### `extract_json`
- **Does**: Extract JSON from LLM response
- **Strategies**: Strip thinking tags → Markdown blocks → Brace matching → Direct parse → Clean and parse

### `strip_thinking_tags`
- **Does**: Remove `<think>...</think>` blocks (DeepSeek, R1)

### `extract_from_markdown_code_block`
- **Does**: Extract from \`\`\`json ... \`\`\` or \`\`\` ... \`\`\`

### `extract_by_delimiters`
- **Does**: Find JSON by matching braces/brackets

### `extract_balanced_braces`
- **Does**: Extract balanced { } or [ ] content

### `clean_json_string`
- **Does**: Fix common issues (trailing commas, smart quotes, comments)

### `remove_comments`
- **Does**: Strip // and /* */ comments from JSON

## Contracts

| Dependent | Expects | Breaking changes |
|-----------|---------|------------------|
| `agent/mod.rs` | `analyze_posts_with_context`, `process_chat` | Method signatures |

## Prompt Format

```
## Your Working Memory (Notes to Self)
...

## Recent Private Chat with Operator
...

## Recent Forum Activity
1. Thread: "Title"
   Post by {author}: {body}
   Post ID: {id}

Review the forum activity and decide what to do.
Respond with JSON:
{"action": "reply"|"update_memory"|"none", ...}
```

## Notes
- Temperature 0.7 for balanced responses
- Max 2048 tokens for reasoning
- Robust JSON extraction handles many LLM quirks
- Thinking tag removal for reasoning models (DeepSeek R1)
- Unit tests cover JSON extraction edge cases
