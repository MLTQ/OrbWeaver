use eframe::egui::{self, Color32, RichText, ScrollArea};

use crate::agent::AgentEvent;
use crate::database::ChatMessage;

pub fn render_event_log(ui: &mut egui::Ui, events: &[AgentEvent]) {
    ScrollArea::vertical()
        .stick_to_bottom(true)
        .max_height(ui.available_height() - 60.0)
        .show(ui, |ui| {
            if events.is_empty() {
                ui.centered_and_justified(|ui| {
                    ui.label(RichText::new("Waiting for agent activity...").weak().italics());
                });
                return;
            }

            for event in events {
                match event {
                    AgentEvent::Observation(text) => {
                        ui.label(RichText::new(text).color(Color32::LIGHT_BLUE));
                        ui.add_space(4.0);
                    }
                    AgentEvent::ReasoningTrace(steps) => {
                        ui.group(|ui| {
                            ui.label(RichText::new("💭 Reasoning:").strong());
                            for step in steps {
                                ui.label(RichText::new(format!("  • {}", step)).color(Color32::GRAY));
                            }
                        });
                        ui.add_space(6.0);
                    }
                    AgentEvent::ActionTaken { action, result } => {
                        ui.label(
                            RichText::new(format!("✅ {}: {}", action, result))
                                .color(Color32::GREEN)
                        );
                        ui.add_space(4.0);
                    }
                    AgentEvent::Error(e) => {
                        ui.label(
                            RichText::new(format!("❌ Error: {}", e))
                                .color(Color32::RED)
                        );
                        ui.add_space(4.0);
                    }
                    AgentEvent::StateChanged(_) => {
                        // State changes are shown in header, not in log
                    }
                }
            }
        });
}

/// Render the private chat interface between operator and agent
pub fn render_private_chat(ui: &mut egui::Ui, messages: &[ChatMessage]) {
    ui.heading("Private Chat");
    ui.add_space(4.0);
    ui.label(RichText::new("Direct communication with your agent").weak().italics());
    ui.add_space(8.0);

    ScrollArea::vertical()
        .stick_to_bottom(true)
        .max_height(ui.available_height() - 60.0)
        .show(ui, |ui| {
            if messages.is_empty() {
                ui.centered_and_justified(|ui| {
                    ui.label(RichText::new("No messages yet. Type below to start chatting with your agent.").weak().italics());
                });
                return;
            }

            for msg in messages {
                let is_operator = msg.role == "operator";
                let time_str = msg.created_at.format("%H:%M").to_string();

                ui.horizontal(|ui| {
                    if is_operator {
                        // Operator message - right aligned (using indentation)
                        ui.add_space(ui.available_width() * 0.3);
                    }

                    ui.group(|ui| {
                        ui.set_max_width(ui.available_width() * 0.7);

                        let (role_label, role_color, bg_color) = if is_operator {
                            ("You", Color32::from_rgb(100, 149, 237), Color32::from_rgb(30, 40, 60))
                        } else {
                            ("Agent", Color32::from_rgb(144, 238, 144), Color32::from_rgb(30, 50, 40))
                        };

                        ui.visuals_mut().widgets.noninteractive.bg_fill = bg_color;

                        ui.horizontal(|ui| {
                            ui.label(RichText::new(role_label).color(role_color).strong());
                            ui.label(RichText::new(time_str).weak().small());
                        });

                        ui.label(&msg.content);

                        // Show processing status for operator messages
                        if is_operator && !msg.processed {
                            ui.label(RichText::new("⏳ Waiting for agent...").weak().small().italics());
                        }
                    });

                    if !is_operator {
                        // Agent message - left aligned (add space on right)
                        ui.add_space(ui.available_width());
                    }
                });

                ui.add_space(8.0);
            }
        });
}
