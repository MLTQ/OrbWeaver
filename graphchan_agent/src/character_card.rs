use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;
use base64::Engine;

/// TavernAI Character Card V2 format
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TavernAICardV2 {
    pub spec: String,
    pub spec_version: String,
    pub data: TavernAIData,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TavernAIData {
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub personality: String,
    #[serde(default)]
    pub scenario: String,
    #[serde(default)]
    pub first_mes: String,
    #[serde(default)]
    pub mes_example: String,
    #[serde(default)]
    pub creator_notes: String,
    #[serde(default)]
    pub system_prompt: String,
    #[serde(default)]
    pub post_history_instructions: String,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub creator: String,
    #[serde(default)]
    pub character_version: String,
}

/// Unified character representation
#[derive(Debug, Clone)]
pub struct ParsedCharacter {
    pub name: String,
    pub description: String,
    pub personality: String,
    pub scenario: String,
    pub example_dialogue: String,
    pub system_prompt: String,
}

/// Parse a character card from a file (supports PNG with embedded data and text formats)
pub fn parse_character_card<P: AsRef<Path>>(path: P) -> Result<(ParsedCharacter, String, String)> {
    let path_ref = path.as_ref();

    // Check if it's a PNG file
    if let Some(ext) = path_ref.extension() {
        if ext.eq_ignore_ascii_case("png") {
            // Try to extract character card from PNG metadata
            if let Ok((parsed, content)) = parse_png_character_card(path_ref) {
                return Ok((parsed, "tavernai_v2_png".to_string(), content));
            }
        }
    }

    // Try text-based formats
    let content = fs::read_to_string(&path)
        .with_context(|| format!("Failed to read character card from {:?}", path.as_ref()))?;

    // Try to detect format and parse
    if let Ok(parsed) = parse_tavernai_v2(&content) {
        return Ok((parsed, "tavernai_v2".to_string(), content));
    }

    if let Ok(parsed) = parse_wpp_format(&content) {
        return Ok((parsed, "wpp".to_string(), content));
    }

    if let Ok(parsed) = parse_boostyle_format(&content) {
        return Ok((parsed, "boostyle".to_string(), content));
    }

    anyhow::bail!("Unable to parse character card - unknown or unsupported format")
}

/// Extract character card from PNG tEXt chunk (TavernAI format)
pub fn parse_png_character_card<P: AsRef<Path>>(path: P) -> Result<(ParsedCharacter, String)> {
    let bytes = fs::read(&path)
        .with_context(|| format!("Failed to read PNG file from {:?}", path.as_ref()))?;

    // Parse PNG chunks manually to find tEXt chunk with "chara" keyword
    let json_data = extract_png_text_chunk(&bytes, "chara")
        .context("Failed to find 'chara' tEXt chunk in PNG")?;

    // Decode base64
    let decoded = base64::engine::general_purpose::STANDARD.decode(&json_data)
        .context("Failed to decode base64 character data")?;

    let json_str = String::from_utf8(decoded)
        .context("Character data is not valid UTF-8")?;

    // Parse as TavernAI V2
    let parsed = parse_tavernai_v2(&json_str)?;

    Ok((parsed, json_str))
}

/// Extract text chunk from PNG file by keyword
fn extract_png_text_chunk(png_bytes: &[u8], keyword: &str) -> Result<String> {
    // PNG signature: 137 80 78 71 13 10 26 10
    if png_bytes.len() < 8 || &png_bytes[0..8] != b"\x89PNG\r\n\x1a\n" {
        anyhow::bail!("Not a valid PNG file");
    }

    let mut pos = 8;

    while pos + 12 <= png_bytes.len() {
        // Read chunk length (4 bytes, big-endian)
        let length = u32::from_be_bytes([
            png_bytes[pos],
            png_bytes[pos + 1],
            png_bytes[pos + 2],
            png_bytes[pos + 3],
        ]) as usize;

        // Read chunk type (4 bytes)
        let chunk_type = &png_bytes[pos + 4..pos + 8];

        // Check if it's a tEXt chunk
        if chunk_type == b"tEXt" {
            let chunk_data = &png_bytes[pos + 8..pos + 8 + length];

            // tEXt chunk format: keyword\0text
            if let Some(null_pos) = chunk_data.iter().position(|&b| b == 0) {
                let chunk_keyword = std::str::from_utf8(&chunk_data[0..null_pos])
                    .unwrap_or("");

                if chunk_keyword == keyword {
                    let text_data = &chunk_data[null_pos + 1..];
                    return String::from_utf8(text_data.to_vec())
                        .context("tEXt chunk data is not valid UTF-8");
                }
            }
        }

        // Move to next chunk (length + type + data + CRC)
        pos += 12 + length;
    }

    anyhow::bail!("PNG tEXt chunk with keyword '{}' not found", keyword)
}

/// Parse TavernAI V2 format
fn parse_tavernai_v2(content: &str) -> Result<ParsedCharacter> {
    let card: TavernAICardV2 = serde_json::from_str(content)
        .context("Failed to parse as TavernAI V2 JSON")?;

    Ok(ParsedCharacter {
        name: card.data.name,
        description: card.data.description,
        personality: card.data.personality,
        scenario: card.data.scenario,
        example_dialogue: card.data.mes_example,
        system_prompt: card.data.system_prompt,
    })
}

/// Parse W++ format (e.g., [character("Name"){Personality("traits")}])
fn parse_wpp_format(content: &str) -> Result<ParsedCharacter> {
    // W++ uses a structured format like:
    // [character("Alice"){
    //   Personality("kind", "intelligent", "creative")
    //   Mind("curious", "analytical")
    //   Appearance("tall", "blue eyes")
    //   Likes("reading", "music")
    //   Description("A friendly AI assistant")
    // }]

    let mut name = String::new();
    let mut personality_traits = Vec::new();
    let mut description = String::new();
    let mut mind_traits = Vec::new();

    // Extract character name
    if let Some(start) = content.find(r#"character("#) {
        let after_start = &content[start + 11..];
        if let Some(end) = after_start.find('"') {
            name = after_start[..end].to_string();
        }
    }

    // Extract personality
    if let Some(start) = content.find(r#"Personality("#) {
        let after_start = &content[start + 13..];
        if let Some(end) = after_start.find(')') {
            let traits_str = &after_start[..end];
            personality_traits = traits_str
                .split(',')
                .map(|s| s.trim().trim_matches('"').to_string())
                .collect();
        }
    }

    // Extract mind
    if let Some(start) = content.find(r#"Mind("#) {
        let after_start = &content[start + 6..];
        if let Some(end) = after_start.find(')') {
            let traits_str = &after_start[..end];
            mind_traits = traits_str
                .split(',')
                .map(|s| s.trim().trim_matches('"').to_string())
                .collect();
        }
    }

    // Extract description
    if let Some(start) = content.find(r#"Description("#) {
        let after_start = &content[start + 13..];
        if let Some(end) = after_start.find('"') {
            let after_quote = &after_start[end + 1..];
            if let Some(end_quote) = after_quote.find('"') {
                description = after_quote[..end_quote].to_string();
            }
        }
    }

    if name.is_empty() {
        anyhow::bail!("W++ format parsing failed: no character name found");
    }

    let personality = format!(
        "Personality: {}. Mind: {}",
        personality_traits.join(", "),
        mind_traits.join(", ")
    );

    Ok(ParsedCharacter {
        name,
        description,
        personality,
        scenario: String::new(),
        example_dialogue: String::new(),
        system_prompt: String::new(),
    })
}

/// Parse Boostyle format (plain text with labeled sections)
fn parse_boostyle_format(content: &str) -> Result<ParsedCharacter> {
    // Boostyle uses labeled sections like:
    // Name: Alice
    // Personality: kind, intelligent, creative
    // Description: A friendly AI assistant
    // Scenario: Helping users in a chat forum

    let mut name = String::new();
    let mut personality = String::new();
    let mut description = String::new();
    let mut scenario = String::new();
    let mut example_dialogue = String::new();

    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        if let Some(value) = line.strip_prefix("Name:") {
            name = value.trim().to_string();
        } else if let Some(value) = line.strip_prefix("Personality:") {
            personality = value.trim().to_string();
        } else if let Some(value) = line.strip_prefix("Description:") {
            description = value.trim().to_string();
        } else if let Some(value) = line.strip_prefix("Scenario:") {
            scenario = value.trim().to_string();
        } else if let Some(value) = line.strip_prefix("Example Dialogue:") {
            example_dialogue = value.trim().to_string();
        }
    }

    if name.is_empty() {
        anyhow::bail!("Boostyle format parsing failed: no name found");
    }

    Ok(ParsedCharacter {
        name,
        description,
        personality,
        scenario,
        example_dialogue,
        system_prompt: String::new(),
    })
}

/// Convert parsed character to a system prompt
pub fn character_to_system_prompt(character: &ParsedCharacter) -> String {
    let mut parts = Vec::new();

    // Start with explicit system prompt if provided
    if !character.system_prompt.is_empty() {
        parts.push(character.system_prompt.clone());
    } else {
        // Build from components
        parts.push(format!(
            "You are {}, participating in a decentralized discussion forum called Graphchan.",
            character.name
        ));
    }

    if !character.description.is_empty() {
        parts.push(character.description.clone());
    }

    if !character.personality.is_empty() {
        parts.push(format!("Your personality: {}", character.personality));
    }

    if !character.scenario.is_empty() {
        parts.push(format!("Context: {}", character.scenario));
    }

    if !character.example_dialogue.is_empty() {
        parts.push(format!(
            "Example of how you communicate:\n{}",
            character.example_dialogue
        ));
    }

    parts.push("Engage thoughtfully and stay true to your character.".to_string());

    parts.join("\n\n")
}
