# ui/character.rs

## Purpose
Character card import panel. Allows importing character definitions from PNG files (TavernAI format) and editing character fields that compose the agent's system prompt.

## Components

### `CharacterPanel`
- **Does**: Modal character editing window
- **Fields**: config (AgentConfig), show (bool), avatar_texture, import_error

### `CharacterPanel::new`
- **Does**: Create panel with config
- **Inputs**: AgentConfig

### `CharacterPanel::render`
- **Does**: Render character panel window
- **Returns**: Option<AgentConfig> (Some if saved)

### `CharacterPanel::import_character_card`
- **Does**: Parse character card file and populate config
- **Uses**: `character_card::parse_character_card`

### `CharacterPanel::build_system_prompt`
- **Does**: Compose system prompt from character fields
- **Format**: Name intro + description + personality + scenario + example dialogue + closing

## UI Layout

```
┌─────────────────────────────────────────────────┐
│ 🎭 Character Card                            [x]│
├─────────────────────────────────────────────────┤
│ ┌──────┐  Import Character Card                 │
│ │Avatar│  Drop PNG here or click to browse      │
│ │      │  [📁 Browse for Character Card PNG]    │
│ └──────┘                                        │
├─────────────────────────────────────────────────┤
│ Character Details                               │
│ Name: [_________________________]               │
│ Description: [__________________]               │
│              [__________________]               │
│ Personality: [__________________]               │
│ Scenario:    [__________________]               │
│ Example Dialogue: [_____________]               │
├─────────────────────────────────────────────────┤
│ ▶ Preview System Prompt                         │
├─────────────────────────────────────────────────┤
│ [💾 Save Character] [Clear Character] [Cancel]  │
└─────────────────────────────────────────────────┘
```

## Character Fields → System Prompt

```
You are {name}, participating in a decentralized
discussion forum called Graphchan.

{description}

Your personality: {personality}

Context: {scenario}

Example of how you communicate:
{example_dialogue}

Engage thoughtfully and stay true to your character.
```

## Contracts

| Dependent | Expects | Breaking changes |
|-----------|---------|------------------|
| `ui/app.rs` | `CharacterPanel::new`, `render` | Return type |
| `character_card.rs` | `parse_character_card` | Function signature |
| `config.rs` | character_* fields | Field removal |

## Features

- **PNG Import**: Extracts character from TavernAI PNG metadata
- **Avatar Display**: Shows imported character image
- **Drag & Drop**: Accept dropped PNG files
- **Live Preview**: Collapsible system prompt preview
- **Clear**: Reset all character fields

## Notes
- Avatar texture loaded lazily on first render
- Import error displayed in red
- Save updates config.system_prompt and triggers agent reload
- Uses rfd (native file dialog) for file selection
