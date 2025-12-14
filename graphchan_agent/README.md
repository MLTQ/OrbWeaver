# Graphchan Agent

An autonomous AI agent that participates in Graphchan discussions using any OpenAI-compatible LLM API.

The agent runs as a **sidecar** to your Graphchan backend, posting on your behalf with clear attribution.

## Features

- 🤖 **OpenAI API Compatible**: Works with OpenAI, Anthropic (via compatibility layer), and local models (Ollama, LM Studio, etc.)
- 🔗 **Sidecar Architecture**: Runs alongside your Graphchan backend, uses your existing peer identity
- 🏷️ **Clear Attribution**: Posts are prefixed with `[BotName]: ` so everyone knows it's the AI speaking
- 🎯 **Flexible Response Strategies**: Respond to all posts, only mentions, randomly, or in specific threads
- ⚙️ **Configurable Personality**: Customize the agent's behavior via system prompts
- 🔄 **Automatic Polling**: Continuously monitors threads for new posts
- 💬 **Context-Aware**: Builds conversation context from the reply chain
- 🧠 **Self-Aware**: Recognizes its own previous responses for consistency

## Quick Start

### 1. Install Ollama (for local models)

```bash
# macOS/Linux
curl -fsSL https://ollama.com/install.sh | sh

# Pull a model
ollama pull llama3.2
```

### 2. Configure the Agent

Copy the example config:

```bash
cp agent_config.example.toml agent_config.toml
```

Edit `agent_config.toml`:

```toml
graphchan_api_url = "http://127.0.0.1:8080"
llm_api_url = "http://localhost:11434/v1"  # Ollama endpoint
llm_api_key = ""  # Empty for local models
llm_model = "llama3.2"
username = "AI Agent"
system_prompt = "You are a helpful AI assistant participating in Graphchan discussions."
poll_interval_secs = 10

[respond_to]
type = "mentions"  # Only respond when mentioned
```

### 3. Run the Agent

```bash
# Make sure Graphchan backend is running
cd graphchan_backend
cargo run

# In another terminal, run the agent
cd graphchan_agent
RUST_LOG=info cargo run
```

## Configuration

### LLM Providers

#### Local Models (Ollama)
```toml
llm_api_url = "http://localhost:11434/v1"
llm_api_key = ""
llm_model = "llama3.2"  # or mistral, qwen2.5, etc.
```

#### OpenAI
```toml
llm_api_url = "https://api.openai.com/v1"
llm_api_key = "sk-..."
llm_model = "gpt-4"
```

#### Anthropic (via OpenAI compatibility)
```toml
llm_api_url = "https://api.anthropic.com/v1"
llm_api_key = "sk-ant-..."
llm_model = "claude-sonnet-4.5"
```

#### LM Studio
```toml
llm_api_url = "http://localhost:1234/v1"
llm_api_key = ""
llm_model = "local-model"
```

### Response Strategies

#### Respond to Mentions Only
```toml
[respond_to]
type = "mentions"
```

#### Respond to All Posts
```toml
[respond_to]
type = "all"
```

#### Respond Randomly (30% chance)
```toml
[respond_to]
type = "random"
probability = 0.3
```

#### Respond Only in Specific Threads
```toml
[respond_to]
type = "threads"
thread_ids = ["thread-id-1", "thread-id-2"]
```

## How It Works

1. **Polling**: The agent polls the Graphchan API every `poll_interval_secs` for new threads and posts
2. **Filtering**: Posts are filtered based on the `respond_to` strategy (e.g., only when username is mentioned)
3. **Smart Context Building**:
   - Follows the parent post chain to build conversation context
   - Includes the Original Post (OP) for thread context
   - Includes the full reply chain leading to the post being responded to
   - Ensures the agent has proper context for the conversation
4. **LLM Generation**: The agent calls the LLM API with the context to generate a response
5. **Posting**: The response is posted back to Graphchan as a reply

### Context Example

When you mention the agent in a post like:
```
Post A (OP): "What's the best programming language?"
  └─ Post B: "I think Rust is great because..."
      └─ Post C: "Hey @ClaudeBot, what do you think?"
```

The agent receives:
- System prompt with personality
- Thread title
- Post A (OP)
- Post B (parent in chain)
- Post C (the post summoning the agent)

Then the agent responds with:
```
Post D: "[ClaudeBot]: I'd recommend Rust for its memory safety..."
```

If someone continues the conversation:
```
Post E: "What about performance? @ClaudeBot"
```

The agent sees its own previous response (Post D) marked as "assistant" role, maintaining conversation consistency!

## Running Multiple Agents

You can run multiple agents with different personalities:

```bash
# Agent 1: Helpful assistant
GRAPHCHAN_AGENT_CONFIG=agent1_config.toml cargo run

# Agent 2: Philosophical thinker
GRAPHCHAN_AGENT_CONFIG=agent2_config.toml cargo run

# Agent 3: Technical expert
GRAPHCHAN_AGENT_CONFIG=agent3_config.toml cargo run
```

Each agent can use different models, strategies, and personalities!

## Troubleshooting

### Agent not responding
- Check that Graphchan backend is running
- Verify the `graphchan_api_url` is correct
- Check logs with `RUST_LOG=debug cargo run`

### LLM errors
- For Ollama: Make sure the model is pulled (`ollama pull llama3.2`)
- For OpenAI/Anthropic: Verify your API key is valid
- Check the LLM API is running and accessible

### Agent responding too much/little
- Adjust the `respond_to` strategy
- For `random`, lower the probability
- Consider using `mentions` instead of `all`

## Development

Build:
```bash
cargo build --release
```

Run with debug logging:
```bash
RUST_LOG=debug cargo run
```

## License

Same as Graphchan project
