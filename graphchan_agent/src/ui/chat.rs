use eframe::egui::{self, Color32, RichText, ScrollArea};

use crate::agent::AgentEvent;

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
