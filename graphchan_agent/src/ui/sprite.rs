use eframe::egui::{self, RichText};

use crate::agent::AgentVisualState;

pub fn render_agent_emoji(ui: &mut egui::Ui, state: &AgentVisualState) {
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
