use eframe::egui::{self, RichText};

use crate::agent::AgentVisualState;
use super::avatar::AvatarSet;

pub fn render_agent_sprite(
    ui: &mut egui::Ui,
    state: &AgentVisualState,
    avatars: Option<&mut AvatarSet>,
) {
    // Try to render avatar if available
    if let Some(avatar_set) = avatars {
        if let Some(avatar) = avatar_set.get_for_state(state) {
            // Update animation
            avatar.update();

            // Render avatar
            let texture = avatar.current_texture();
            let size = egui::vec2(64.0, 64.0);

            ui.add(egui::Image::new(texture).fit_to_exact_size(size));

            // Request repaint for animations
            if avatar.is_animated() {
                ui.ctx().request_repaint();
            }

            return;
        }
    }

    // Fallback to emoji if no avatar
    render_agent_emoji(ui, state);
}

fn render_agent_emoji(ui: &mut egui::Ui, state: &AgentVisualState) {
    let (emoji, color) = match state {
        AgentVisualState::Idle => ("😴", egui::Color32::GRAY),
        AgentVisualState::Reading => ("📖", egui::Color32::LIGHT_BLUE),
        AgentVisualState::Thinking => ("🤔", egui::Color32::YELLOW),
        AgentVisualState::Writing => ("✍️", egui::Color32::LIGHT_GREEN),
        AgentVisualState::Happy => ("😊", egui::Color32::GREEN),
        AgentVisualState::Confused => ("😕", egui::Color32::ORANGE),
        AgentVisualState::Paused => ("⏸️", egui::Color32::LIGHT_RED),
    };

    ui.heading(RichText::new(emoji).size(48.0).color(color));
}
