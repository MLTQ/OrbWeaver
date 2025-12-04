use eframe::egui::Color32;
use std::collections::HashMap;

/// Calculate a color for a post based on its reactions using additive RGB model
/// Starts from black (0,0,0) and each reaction adds RGB values
pub fn calculate_post_color(
    post_reactions: &HashMap<String, usize>,
    _max_score_in_thread: f32, // Kept for API compatibility but not used in additive model
) -> Color32 {
    let mut r: f32 = 0.0;
    let mut g: f32 = 0.0;
    let mut b: f32 = 0.0;

    // Each emoji adds its RGB delta multiplied by count
    for (emoji, count) in post_reactions {
        let (delta_r, delta_g, delta_b) = emoji_to_rgb_delta(emoji);
        let count_f = *count as f32;

        r += delta_r * count_f;
        g += delta_g * count_f;
        b += delta_b * count_f;
    }

    // Clamp to valid RGB range
    let r = r.clamp(0.0, 255.0) as u8;
    let g = g.clamp(0.0, 255.0) as u8;
    let b = b.clamp(0.0, 255.0) as u8;

    // If no reactions, use subtle default color instead of pure black
    if r == 0 && g == 0 && b == 0 {
        Color32::from_rgb(60, 65, 90)
    } else {
        Color32::from_rgb(r, g, b)
    }
}

/// Map each emoji to RGB delta values
/// Each reaction adds these values to the color
fn emoji_to_rgb_delta(emoji: &str) -> (f32, f32, f32) {
    match emoji {
        // Thumbs up -> Yellow/Green (positive energy)
        // Goal: ~10 reactions = (150, 150, 0) bright yellow
        "👍" => (15.0, 15.0, 0.0),

        // Heart -> Red/Pink (love, passion)
        // Goal: ~10 reactions = (200, 80, 100) vibrant pink
        "❤️" => (20.0, 8.0, 10.0),

        // Laugh -> Yellow/Orange (joy, humor)
        // Goal: ~10 reactions = (180, 140, 0) golden
        "😂" => (18.0, 14.0, 0.0),

        // Party -> Magenta/Purple (celebration)
        // Goal: ~10 reactions = (160, 80, 160) party purple
        "🎉" => (16.0, 8.0, 16.0),

        // Fire -> Orange/Red (hot, controversial)
        // Goal: ~10 reactions = (200, 100, 0) bright orange
        "🔥" => (20.0, 10.0, 0.0),

        // Sparkles -> White (brilliant, quality)
        // Goal: ~10 reactions = (180, 180, 180) bright white
        "✨" => (18.0, 18.0, 18.0),

        // Eyes -> Cyan (attention, interest)
        // Goal: ~10 reactions = (0, 150, 180) cyan
        "👀" => (0.0, 15.0, 18.0),

        // Thinking -> Purple (contemplative)
        // Goal: ~10 reactions = (120, 60, 150) thoughtful purple
        "🤔" => (12.0, 6.0, 15.0),

        // Thumbs down -> Dark Red (negative, downvote)
        // Goal: ~10 reactions = (100, 0, 0) deep red
        "👎" => (10.0, 0.0, 0.0),

        // Sad -> Blue (sadness, empathy)
        // Goal: ~10 reactions = (0, 60, 120) melancholy blue
        "😢" => (0.0, 6.0, 12.0),

        // Angry -> Red (anger, disagreement)
        // Goal: ~10 reactions = (150, 0, 0) angry red
        "😡" => (15.0, 0.0, 0.0),

        // Default for unknown emojis -> Subtle gray
        _ => (8.0, 8.0, 8.0),
    }
}

/// Calculate the maximum reaction score across all posts in a thread
/// Not used in additive model but kept for API compatibility
pub fn calculate_max_score_from_responses(_all_reactions: &HashMap<String, crate::models::ReactionsResponse>) -> f32 {
    1.0 // Return dummy value since we don't normalize in additive model
}
