use eframe::egui;
use crate::config::AgentConfig;

pub struct SettingsPanel {
    pub config: AgentConfig,
    pub show: bool,
}

impl SettingsPanel {
    pub fn new(config: AgentConfig) -> Self {
        Self {
            config,
            show: false,
        }
    }

    pub fn render(&mut self, ctx: &egui::Context) -> Option<AgentConfig> {
        if !self.show {
            return None;
        }

        let mut new_config = None;

        egui::Window::new("⚙ Settings")
            .open(&mut self.show)
            .default_width(500.0)
            .show(ctx, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    ui.heading("Graphchan Connection");
                    ui.add_space(8.0);

                    ui.horizontal(|ui| {
                        ui.label("API URL:");
                        ui.text_edit_singleline(&mut self.config.graphchan_api_url);
                    });
                    ui.label("Example: http://localhost:8080");
                    ui.add_space(16.0);

                    ui.separator();
                    ui.heading("LLM Configuration");
                    ui.add_space(8.0);

                    ui.horizontal(|ui| {
                        ui.label("API URL:");
                        ui.text_edit_singleline(&mut self.config.llm_api_url);
                    });
                    ui.label("Example: http://localhost:11434 (Ollama)");
                    ui.add_space(8.0);

                    ui.horizontal(|ui| {
                        ui.label("Model:   ");
                        ui.text_edit_singleline(&mut self.config.llm_model);
                    });
                    ui.label("Example: llama3.2, qwen2.5, mistral");
                    ui.add_space(8.0);

                    ui.horizontal(|ui| {
                        ui.label("API Key: ");
                        let mut key_str = self.config.llm_api_key.clone().unwrap_or_default();
                        if ui.text_edit_singleline(&mut key_str).changed() {
                            self.config.llm_api_key = if key_str.is_empty() {
                                None
                            } else {
                                Some(key_str)
                            };
                        }
                    });
                    ui.label("Optional - only needed for OpenAI/Claude");
                    ui.add_space(16.0);

                    ui.separator();
                    ui.heading("Behavior");
                    ui.add_space(8.0);

                    ui.horizontal(|ui| {
                        ui.label("Check interval (seconds):");
                        ui.add(egui::DragValue::new(&mut self.config.check_interval_seconds).range(10..=600));
                    });
                    ui.add_space(8.0);

                    ui.horizontal(|ui| {
                        ui.label("Max posts per hour:");
                        ui.add(egui::DragValue::new(&mut self.config.max_posts_per_hour).range(1..=100));
                    });
                    ui.add_space(8.0);

                    ui.horizontal(|ui| {
                        ui.label("Agent name:");
                        ui.text_edit_singleline(&mut self.config.agent_name);
                    });
                    ui.add_space(16.0);

                    ui.separator();
                    ui.heading("System Prompt");
                    ui.add_space(8.0);

                    ui.label("Customize how the agent behaves:");
                    ui.text_edit_multiline(&mut self.config.system_prompt);
                    ui.add_space(16.0);

                    ui.separator();
                    ui.add_space(8.0);

                    ui.horizontal(|ui| {
                        if ui.button("💾 Save & Apply").clicked() {
                            new_config = Some(self.config.clone());
                        }

                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.label(format!("Config: {:?}", AgentConfig::config_path()));
                        });
                    });
                });
            });

        new_config
    }
}
