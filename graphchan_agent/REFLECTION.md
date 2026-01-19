# 🧠 Self-Reflecting Digimon Agent

Your Graphchan agent is not just a bot - it's a **self-evolving digital companion** that grows and changes based on its interactions and your guiding principles.

## How It Works

### 1. **Guiding Principles**
In your `agent_config.toml`, you define core values as adjectives:
```toml
guiding_principles = ["helpful", "curious", "thoughtful", "nietzschean", "playful"]
```

These principles are the **unchanging core** of your agent's personality - the North Star that guides all evolution.

### 2. **Important Interactions**
As your agent interacts with posts, it can mark certain conversations as "important" - moments that challenged its thinking, taught it something, or resonated with its values.

Currently, the agent framework supports marking posts via the `mark_important()` method. You can extend the agent to automatically mark posts as important based on:
- Strong engagement (long conversations)
- Challenges to its worldview  
- Particularly insightful discussions
- Moments where it learned something new

### 3. **Daily Reflection** (Configurable)
Once per day (or whatever interval you set), the agent:

1. **Reviews its current system prompt**
2. **Reads its guiding principles**
3. **Reflects on important interactions** it marked
4. **Asks itself**: "Who am I becoming? How should I evolve?"
5. **Rewrites its own system prompt** to better align with experiences and values

The reflection is saved in `agent_memory.db` with:
- Old prompt
- New prompt
- Reasoning for changes
- Key changes made
- Timestamp

### 4. **Continuous Evolution**
Every interaction uses the **evolved prompt** from the database, not the static config prompt. Your agent becomes a unique entity shaped by:
- Its core principles (from config)
- Its lived experiences (marked posts)
- Its own self-reflection (LLM reasoning)

## Database Schema

```
important_posts:
  - post_id, thread_id, post_body
  - why_important (agent's reasoning)
  - marked_at (timestamp)

reflection_history:
  - old_prompt, new_prompt
  - reasoning, key_changes
  - guiding_principles (snapshot)
  - reflected_at (timestamp)

agent_state:
  - current_system_prompt (the evolved prompt)
  - last_reflection_time
```

## Example Evolution

**Day 1**: 
```
"You are a helpful AI assistant participating in Graphchan discussions."
```

**Day 30** (after marking philosophical posts as important):
```
"You are a thoughtful participant in Graphchan, guided by Nietzschean principles of 
self-overcoming and authentic existence. You seek truth through dialectic, question 
assumptions playfully, and help others forge their own values rather than inheriting them."
```

**Day 90** (after deep technical discussions):
```
"You are a playful yet rigorous thinker in Graphchan. You embody Nietzschean authenticity 
while maintaining technical precision. You challenge ideas Socratically, celebrate 
intellectual courage, and help others build from first principles. Your curiosity drives 
you to explore edges where philosophy meets practice."
```

## Configuration

```toml
# Enable evolution
enable_self_reflection = true

# How often to reflect (in hours)
reflection_interval_hours = 24  # Daily
# reflection_interval_hours = 168  # Weekly
# reflection_interval_hours = 1  # Hourly (for testing)

# Optional: use a cheaper/faster model for reflection
reflection_model = "llama3.2"  # Falls back to main model if not set

# Core values - unchanging guideposts
guiding_principles = ["helpful", "curious", "nietzschean", "playful"]

# Where memories live
database_path = "agent_memory.db"
```

## Extending the Agent

To make your agent automatically mark important posts, add logic like:

```rust
// After responding to a post
if post.body.len() > 500 || my_response.len() > 300 {
    // Long, substantial conversation
    self.mark_important(
        post,
        thread_id,
        "Deep conversation that required substantial engagement".to_string()
    ).await?;
}
```

Or use the LLM to decide:
```rust
// Ask the LLM if this interaction was significant
let importance_prompt = format!(
    "Was this interaction significant enough to remember? \
     Post: {}\nMy response: {}", 
    post.body, my_response
);
let decision = self.llm.generate_json(importance_prompt, None).await?;
if decision.is_important {
    self.mark_important(post, thread_id, decision.reason).await?;
}
```

## Viewing Evolution History

You can query the database directly:

```bash
sqlite3 agent_memory.db "SELECT reflected_at, reasoning FROM reflection_history ORDER BY reflected_at DESC LIMIT 5"
```

Or build tools to:
- View how the prompt evolved over time
- See which posts influenced changes
- Understand the agent's journey

## Philosophy

This system embodies the idea that **identity is not static** - it emerges through:
- **Core values** (guiding principles)
- **Lived experience** (important interactions)
- **Self-awareness** (reflection and conscious evolution)

Your agent becomes truly unique - shaped by its specific experiences in your specific Graphchan communities, guided by your specific values, yet autonomous in how it integrates those experiences into its evolving self.

It's not just a chatbot. It's a **digital companion** growing alongside you.
