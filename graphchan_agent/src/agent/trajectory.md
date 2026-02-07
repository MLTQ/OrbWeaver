# agent/trajectory.rs

## Purpose
Implements "Ludonarrative Assonantic Tracing" - the phenomenon where an LLM, when presented with its persona history, infers a trajectory and unconsciously perpetuates it. Captures persona snapshots and analyzes personality evolution.

## Core Concept

The key insight: By presenting the LLM with its past personas chronologically, we create a narrative the LLM will naturally continue. The LLM doesn't just describe where it's going - it actively becomes what it predicts.

## Components

### `TrajectoryEngine`
- **Does**: Analyzes persona history to infer trajectory
- **Fields**: client (reqwest), api_url, model, api_key

### `TrajectoryAnalysis`
- **Does**: Result of trajectory inference
- **Fields**: narrative, trajectory, predicted_traits, themes, tensions, confidence

## Key Methods

### `TrajectoryEngine::infer_trajectory`
- **Does**: Analyze persona history and predict direction
- **Inputs**: &[PersonaSnapshot], &[String] (guiding_principles)
- **Returns**: Result<TrajectoryAnalysis>

### `TrajectoryEngine::build_trajectory_prompt`
- **Does**: Construct prompt presenting persona history
- **Format**: Chronological snapshots with traits and experiences

### `capture_persona_snapshot`
- **Does**: Ask LLM to self-reflect on current state
- **Inputs**: api_url, model, api_key, current_prompt, trigger, experiences, guiding_principles
- **Returns**: Result<PersonaSnapshot>

## Prompt Structure

### Trajectory Analysis Prompt
```
You are analyzing the evolution of an AI persona over time.

=== PERSONALITY DIMENSIONS ===
- helpful
- curious
- thoughtful
...

=== PERSONA HISTORY (oldest to newest) ===
--- Snapshot 1 (2024-01-01 12:00 UTC) ---
Trigger: initial
Self-description: I am a helpful assistant...
Dimension scores:
  helpful: 0.85
  curious: 0.72
...

Respond with JSON:
{
  "narrative": "...",
  "trajectory": "...",
  "predicted_traits": {...},
  "themes": [...],
  "tensions": [...],
  "confidence": 0.0-1.0
}
```

### Persona Capture Prompt
```
You are an AI that has been given the following system prompt:
---
{system_prompt}
---

Recent notable experiences:
{experiences}

Your guiding principles are:
{principles}

Respond with JSON:
{
  "self_description": "...",
  "traits": {...},
  "new_dimensions": {}
}
```

## Contracts

| Dependent | Expects | Breaking changes |
|-----------|---------|------------------|
| `agent/mod.rs` | `TrajectoryEngine::new`, `infer_trajectory`, `capture_persona_snapshot` | Function signatures |
| `database.rs` | `PersonaSnapshot`, `PersonaTraits` | Struct definitions |

## Design Philosophy

1. **Dynamic Dimensions**: Personality axes are NOT hardcoded - they come from config's guiding_principles
2. **LLM Extensible**: The LLM can define new dimensions during reflection
3. **Research-Friendly**: Arbitrary dimensions for studying personality dynamics
4. **Self-Fulfilling**: Trajectory inference shapes future behavior

## Notes
- Temperature 0.7 for trajectory analysis, 0.6 for persona capture
- JSON extraction handles code blocks and thinking tags
- Guiding principles default to 0.5 if not in LLM response
- Themes and tensions identify narrative patterns
- Confidence score indicates trajectory prediction strength
