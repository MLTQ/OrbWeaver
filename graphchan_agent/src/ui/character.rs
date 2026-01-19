use eframe::egui;
use crate::config::AgentConfig;
use std::path::PathBuf;

pub struct CharacterPanel {
    pub config: AgentConfig,
    pub show: bool,
    avatar_texture: Option<egui::TextureHandle>,
    import_error: Option<String>,
}

impl CharacterPanel {
    pub fn new(config: AgentConfig) -> Self {
        Self {
            config,
            show: false,
            avatar_texture: None,
            import_error: None,
        }
    }

    pub fn render(&mut self, ctx: &egui::Context) -> Option<AgentConfig> {
        if !self.show {
            return None;
        }

        let mut new_config = None;
        let mut import_path: Option<PathBuf> = None;
        let mut should_clear = false;
        let mut should_save = false;
        let mut should_close = false;

        // Build system prompt preview outside the closure to avoid borrowing issues
        let system_prompt_preview = self.build_system_prompt_preview();

        let mut is_open = self.show;

        egui::Window::new("🎭 Character Card")
            .open(&mut is_open)
            .default_width(600.0)
            .show(ctx, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    // Avatar and Import Section
                    ui.horizontal(|ui| {
                        // Show avatar thumbnail if available
                        if let Some(ref texture) = self.avatar_texture {
                            ui.image((texture.id(), egui::vec2(128.0, 128.0)));
                        } else if self.config.character_avatar_path.is_some() {
                            // Try to load the avatar
                            if let Some(ref path) = self.config.character_avatar_path {
                                if let Ok(img) = image::open(path) {
                                    let rgba = img.to_rgba8();
                                    let size = [rgba.width() as usize, rgba.height() as usize];
                                    let pixels = rgba.into_raw();
                                    let color_image = egui::ColorImage::from_rgba_unmultiplied(size, &pixels);
                                    let texture = ctx.load_texture("character_avatar", color_image, Default::default());
                                    ui.image((texture.id(), egui::vec2(128.0, 128.0)));
                                    self.avatar_texture = Some(texture);
                                }
                            }
                        } else {
                            // Placeholder
                            ui.vertical(|ui| {
                                ui.set_width(128.0);
                                ui.set_height(128.0);
                                ui.centered_and_justified(|ui| {
                                    ui.label("No Avatar");
                                });
                            });
                        }

                        ui.vertical(|ui| {
                            ui.heading("Import Character Card");
                            ui.label("Drop a PNG character card here or click to browse");

                            if ui.button("📁 Browse for Character Card PNG").clicked() {
                                if let Some(path) = rfd::FileDialog::new()
                                    .add_filter("PNG Image", &["png"])
                                    .pick_file()
                                {
                                    import_path = Some(path);
                                }
                            }

                            if let Some(ref error) = self.import_error {
                                ui.colored_label(egui::Color32::RED, format!("Error: {}", error));
                            }
                        });
                    });

                    ui.add_space(16.0);
                    ui.separator();

                    // Character Details
                    ui.heading("Character Details");
                    ui.add_space(8.0);

                    ui.horizontal(|ui| {
                        ui.label("Name:");
                        ui.text_edit_singleline(&mut self.config.character_name);
                    });
                    ui.add_space(4.0);

                    ui.label("Description:");
                    ui.text_edit_multiline(&mut self.config.character_description);
                    ui.add_space(4.0);

                    ui.label("Personality:");
                    ui.text_edit_multiline(&mut self.config.character_personality);
                    ui.add_space(4.0);

                    ui.label("Scenario:");
                    ui.text_edit_multiline(&mut self.config.character_scenario);
                    ui.add_space(4.0);

                    ui.label("Example Dialogue:");
                    ui.text_edit_multiline(&mut self.config.character_example_dialogue);
                    ui.add_space(16.0);

                    ui.separator();
                    ui.add_space(8.0);

                    // Preview system prompt
                    ui.collapsing("Preview System Prompt", |ui| {
                        ui.label(&system_prompt_preview);
                    });

                    ui.add_space(8.0);

                    // Save/Cancel buttons
                    ui.horizontal(|ui| {
                        if ui.button("💾 Save Character").clicked() {
                            should_save = true;
                        }

                        if ui.button("Clear Character").clicked() {
                            should_clear = true;
                        }

                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.button("Cancel").clicked() {
                                should_close = true;
                            }
                        });
                    });
                });
            });

        // Update window state
        self.show = is_open && !should_close;

        // Handle save after the window is closed to avoid borrowing issues
        if should_save {
            // Update system prompt from character data
            self.config.system_prompt = self.build_system_prompt();
            new_config = Some(self.config.clone());
        }

        // Handle import after the window is closed to avoid borrowing issues
        if let Some(path) = import_path {
            self.import_character_card(path);
        }

        // Handle clear after the window is closed
        if should_clear {
            self.config.character_name.clear();
            self.config.character_description.clear();
            self.config.character_personality.clear();
            self.config.character_scenario.clear();
            self.config.character_example_dialogue.clear();
            self.config.character_avatar_path = None;
            self.avatar_texture = None;
        }

        // Handle drag-and-drop
        ctx.input(|i| {
            if !i.raw.dropped_files.is_empty() {
                if let Some(file) = i.raw.dropped_files.first() {
                    if let Some(ref path) = file.path {
                        self.import_character_card(path.clone());
                    }
                }
            }
        });

        new_config
    }

    fn import_character_card(&mut self, path: PathBuf) {
        self.import_error = None;

        // Try to parse the character card
        match crate::character_card::parse_character_card(&path) {
            Ok((parsed, _format, _raw)) => {
                // Update config with parsed data
                self.config.character_name = parsed.name;
                self.config.character_description = parsed.description;
                self.config.character_personality = parsed.personality;
                self.config.character_scenario = parsed.scenario;
                self.config.character_example_dialogue = parsed.example_dialogue;

                // Store avatar path
                self.config.character_avatar_path = Some(path.to_string_lossy().to_string());

                // Clear texture to force reload
                self.avatar_texture = None;

                tracing::info!("Successfully imported character card: {}", self.config.character_name);
            }
            Err(e) => {
                self.import_error = Some(format!("Failed to parse character card: {}", e));
                tracing::error!("Character card import error: {}", e);
            }
        }
    }

    fn build_system_prompt(&self) -> String {
        let mut parts = Vec::new();

        if !self.config.character_name.is_empty() {
            parts.push(format!(
                "You are {}, participating in a decentralized discussion forum called Graphchan.",
                self.config.character_name
            ));
        } else {
            parts.push("You are a helpful AI agent participating in forum discussions.".to_string());
        }

        if !self.config.character_description.is_empty() {
            parts.push(self.config.character_description.clone());
        }

        if !self.config.character_personality.is_empty() {
            parts.push(format!("Your personality: {}", self.config.character_personality));
        }

        if !self.config.character_scenario.is_empty() {
            parts.push(format!("Context: {}", self.config.character_scenario));
        }

        if !self.config.character_example_dialogue.is_empty() {
            parts.push(format!(
                "Example of how you communicate:\n{}",
                self.config.character_example_dialogue
            ));
        }

        parts.push("Engage thoughtfully and stay true to your character.".to_string());

        parts.join("\n\n")
    }

    fn build_system_prompt_preview(&self) -> String {
        self.build_system_prompt()
    }
}
