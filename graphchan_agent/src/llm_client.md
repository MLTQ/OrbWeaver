# llm_client.rs

## Purpose
Generic OpenAI-compatible LLM client for chat completions. Supports Ollama, LM Studio, vLLM, OpenAI, and any OpenAI-compatible API. Handles JSON extraction from LLM responses and vision model integration.

## Components

### `LlmClient`
- **Does**: HTTP client for LLM API calls
- **Fields**: api_url, api_key, model, client (reqwest)

### `LlmClient::generate`
- **Does**: Generate completion from messages
- **Inputs**: Vec<Message>
- **Returns**: Result<String>

### `LlmClient::generate_with_model`
- **Does**: Generate with specific model override
- **Inputs**: messages, model name
- **Returns**: Result<String>

### `LlmClient::decide_to_respond`
- **Does**: Ask LLM to decide if agent should respond
- **Returns**: Result<DecisionResponse>

### `LlmClient::generate_json<T>`
- **Does**: Generate and parse typed JSON response
- **Handles**: Markdown code blocks, raw JSON extraction

### `LlmClient::evaluate_image`
- **Does**: Vision model image evaluation
- **Returns**: Result<ImageEvaluation>
- **Note**: Placeholder implementation - needs provider-specific adaptation

### `DecisionResponse`
- **Fields**: should_respond (bool), reasoning (String)

### `ImageEvaluation`
- **Fields**: satisfactory (bool), reasoning, suggested_prompt_refinement

### `Message`
- **Does**: Chat message for API
- **Fields**: role ("system"/"user"/"assistant"), content

## Contracts

| Dependent | Expects | Breaking changes |
|-----------|---------|------------------|
| `agent/reasoning.rs` | Similar API (uses own client) | N/A (parallel impl) |
| `agent/image_gen.rs` | `evaluate_image` for quality check | Return type |

## API Format

```json
POST /chat/completions
{
  "model": "llama3.2",
  "messages": [
    {"role": "system", "content": "..."},
    {"role": "user", "content": "..."}
  ],
  "temperature": 0.7,
  "max_tokens": 2000
}
```

## Notes
- API key only sent if non-empty (local models don't need it)
- JSON extraction handles markdown blocks and raw JSON
- Temperature 0.7 for balanced creativity/consistency
- Vision support requires provider-specific implementation
