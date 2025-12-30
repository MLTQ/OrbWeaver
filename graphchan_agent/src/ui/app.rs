use eframe::egui;
use flume::Receiver;
use std::sync::Arc;

use crate::agent::{Agent, AgentEvent, AgentVisualState};
use crate::config::AgentConfig;
use super::settings::SettingsPanel;

pub struct AgentApp {
    events: Vec<AgentEvent>,
    event_rx: Receiver<AgentEvent>,
    agent: Arc<Agent>,
    current_state: AgentVisualState,
    user_input: String,
    runtime: tokio::runtime::Runtime,
    settings_panel: SettingsPanel,
}

impl AgentApp {
    pub fn new(
        event_rx: Receiver<AgentEvent>,
        agent: Arc<Agent>,
        config: AgentConfig,
    ) -> Self {
        Self {
            events: Vec::new(),
            event_rx,
            agent,
            current_state: AgentVisualState::Idle,
            user_input: String::new(),
            runtime: tokio::runtime::Runtime::new().unwrap(),
            settings_panel: SettingsPanel::new(config),
        }
    }
}

impl eframe::App for AgentApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Poll for new events from agent (non-blocking)
        while let Ok(event) = self.event_rx.try_recv() {
            match &event {
                AgentEvent::StateChanged(state) => {
                    self.current_state = state.clone();
                }
                _ => {}
            }
            self.events.push(event);
        }
        
        egui::CentralPanel::default().show(ctx, |ui| {
            // Header with agent sprite
            ui.horizontal(|ui| {
                super::sprite::render_agent_emoji(ui, &self.current_state);
                ui.vertical(|ui| {
                    ui.heading("Graphchan Agent");
                    ui.label(format!("Status: {:?}", self.current_state));
                });

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let pause_text = "⏸ Pause";
                    if ui.button(pause_text).clicked() {
                        let agent = self.agent.clone();
                        self.runtime.spawn(async move {
                            agent.toggle_pause().await;
                        });
                    }

                    if ui.button("⚙ Settings").clicked() {
                        self.settings_panel.show = true;
                    }
                });
            });
            
            ui.separator();
            
            // Event log
            super::chat::render_event_log(ui, &self.events);
            
            ui.separator();
            
            // User input (for future directives)
            ui.horizontal(|ui| {
                ui.label("💬");
                let response = ui.text_edit_singleline(&mut self.user_input);
                
                if response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                    // TODO: Send directive to agent
                    tracing::info!("User directive: {}", self.user_input);
                    self.user_input.clear();
                }
            });
        });

        // Render settings panel
        if let Some(new_config) = self.settings_panel.render(ctx) {
            // User saved new config - persist it to disk
            if let Err(e) = new_config.save() {
                tracing::error!("Failed to save config: {}", e);
            } else {
                tracing::info!("Config saved successfully");
                // TODO: Reload agent with new config
                // For now, user needs to restart the application
            }
        }

        // Request repaint for smooth updates
        ctx.request_repaint_after(std::time::Duration::from_millis(100));
    }
}
