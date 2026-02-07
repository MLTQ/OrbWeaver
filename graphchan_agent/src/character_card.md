# character_card.rs

## Purpose
Character card parsing for multiple formats. Imports character definitions from TavernAI V2 (PNG with embedded JSON), W++ format, and Boostyle plain text. Converts parsed characters to system prompts.

## Components

### `TavernAICardV2`
- **Does**: TavernAI V2 JSON structure
- **Fields**: spec, spec_version, data (TavernAIData)

### `TavernAIData`
- **Does**: Character data within TavernAI format
- **Fields**: name, description, personality, scenario, first_mes, mes_example, creator_notes, system_prompt, post_history_instructions, tags, creator, character_version

### `ParsedCharacter`
- **Does**: Unified character representation
- **Fields**: name, description, personality, scenario, example_dialogue, system_prompt

## Key Functions

### `parse_character_card`
- **Does**: Auto-detect format and parse character
- **Inputs**: file path
- **Returns**: Result<(ParsedCharacter, format_string, raw_content)>
- **Tries**: PNG metadata → TavernAI V2 JSON → W++ → Boostyle

### `parse_png_character_card`
- **Does**: Extract character from PNG tEXt chunk
- **Keyword**: "chara"
- **Decodes**: Base64 → JSON → TavernAI V2

### `extract_png_text_chunk`
- **Does**: Low-level PNG chunk parser
- **Format**: PNG signature → chunks → tEXt with keyword

### `parse_tavernai_v2`
- **Does**: Parse TavernAI V2 JSON format
- **Returns**: Result<ParsedCharacter>

### `parse_wpp_format`
- **Does**: Parse W++ structured format
- **Pattern**: `[character("Name"){Personality("traits")}]`

### `parse_boostyle_format`
- **Does**: Parse plain text with labeled sections
- **Pattern**: `Name: ...\nPersonality: ...\nDescription: ...`

### `character_to_system_prompt`
- **Does**: Convert ParsedCharacter to system prompt
- **Returns**: Formatted string for LLM

## Contracts

| Dependent | Expects | Breaking changes |
|-----------|---------|------------------|
| `ui/character.rs` | `parse_character_card` | Return type |
| `database.rs` | CharacterCard storage (separate struct) | N/A |

## Format Support

### TavernAI V2 (PNG)
```
PNG tEXt chunk: keyword="chara"
                data=base64(json)
```

### W++
```
[character("Alice"){
  Personality("kind", "intelligent")
  Mind("curious")
  Description("A helpful assistant")
}]
```

### Boostyle
```
Name: Alice
Personality: kind, intelligent
Description: A helpful assistant
Scenario: Chat forum
```

## Notes
- PNG parsing handles raw binary chunk extraction
- TavernAI V2 is most common format (SillyTavern, TavernAI)
- System prompt generation adds Graphchan context
- Avatar stored separately as file path in config
