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
                    ui.heading("Agent Identity");
                    ui.add_space(8.0);

                    ui.horizontal(|ui| {
                        ui.label("Username:");
                        ui.text_edit_singleline(&mut self.config.username);
                    });
                    ui.label("Name displayed in posts");
                    ui.add_space(16.0);

                    ui.separator();
                    ui.heading("Behavior");
                    ui.add_space(8.0);

                    ui.horizontal(|ui| {
                        ui.label("Poll interval (seconds):");
                        ui.add(egui::DragValue::new(&mut self.config.poll_interval_secs).range(10..=600));
                    });
                    ui.add_space(8.0);

                    ui.horizontal(|ui| {
                        ui.label("Max posts per hour:");
                        ui.add(egui::DragValue::new(&mut self.config.max_posts_per_hour).range(1..=100));
                    });
                    ui.add_space(8.0);

                    ui.horizontal(|ui| {
                        ui.label("Response strategy:");
                        egui::ComboBox::from_id_salt("response_type")
                            .selected_text(&self.config.respond_to.response_type)
                            .show_ui(ui, |ui| {
                                ui.selectable_value(&mut self.config.respond_to.response_type, "selective".to_string(), "Selective (LLM decides)");
                                ui.selectable_value(&mut self.config.respond_to.response_type, "all".to_string(), "All posts");
                                ui.selectable_value(&mut self.config.respond_to.response_type, "mentions".to_string(), "Only mentions");
                            });
                    });
                    ui.add_space(16.0);

                    ui.separator();
                    ui.heading("Self-Reflection & Evolution");
                    ui.add_space(8.0);

                    ui.checkbox(&mut self.config.enable_self_reflection, "Enable self-reflection");
                    ui.add_space(4.0);

                    ui.horizontal(|ui| {
                        ui.label("Reflection interval (hours):");
                        ui.add(egui::DragValue::new(&mut self.config.reflection_interval_hours).range(1..=168));
                    });
                    ui.add_space(8.0);

                    ui.label("Guiding principles (one per line):");
                    let mut principles_text = self.config.guiding_principles.join("\n");
                    if ui.text_edit_multiline(&mut principles_text).changed() {
                        self.config.guiding_principles = principles_text
                            .lines()
                            .filter(|l| !l.trim().is_empty())
                            .map(|l| l.trim().to_string())
                            .collect();
                    }
                    ui.add_space(16.0);

                    ui.separator();
                    ui.heading("Memory & Database");
                    ui.add_space(8.0);

                    ui.horizontal(|ui| {
                        ui.label("Database path:");
                        ui.text_edit_singleline(&mut self.config.database_path);
                    });
                    ui.add_space(8.0);

                    ui.horizontal(|ui| {
                        ui.label("Max important posts:");
                        ui.add(egui::DragValue::new(&mut self.config.max_important_posts).range(10..=1000));
                    });
                    ui.add_space(16.0);

                    ui.separator();
                    ui.heading("Image Generation (ComfyUI)");
                    ui.add_space(8.0);

                    ui.checkbox(&mut self.config.enable_image_generation, "Enable image generation");
                    ui.add_space(8.0);

                    if self.config.enable_image_generation {
                        ui.horizontal(|ui| {
                            ui.label("ComfyUI URL:");
                            ui.text_edit_singleline(&mut self.config.comfyui.api_url);
                        });
                        ui.add_space(4.0);

                        ui.horizontal(|ui| {
                            ui.label("Workflow type:");
                            egui::ComboBox::from_id_salt("workflow_type")
                                .selected_text(&self.config.comfyui.workflow_type)
                                .show_ui(ui, |ui| {
                                    ui.selectable_value(&mut self.config.comfyui.workflow_type, "sd".to_string(), "Stable Diffusion 1.5");
                                    ui.selectable_value(&mut self.config.comfyui.workflow_type, "sdxl".to_string(), "SDXL");
                                    ui.selectable_value(&mut self.config.comfyui.workflow_type, "flux".to_string(), "Flux");
                                });
                        });
                        ui.add_space(4.0);

                        ui.horizontal(|ui| {
                            ui.label("Model name:");
                            ui.text_edit_singleline(&mut self.config.comfyui.model_name);
                        });
                        ui.add_space(8.0);
                    }
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
                            ui.label("Config: agent_config.toml");
                        });
                    });
                });
            });

        new_config
    }
}
