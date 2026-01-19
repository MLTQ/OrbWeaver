use eframe::egui;
use crate::comfy_workflow::{ComfyWorkflow, ControllableInput, InputType};
use crate::config::AgentConfig;
use std::path::PathBuf;

pub struct ComfySettingsPanel {
    pub show: bool,
    pub workflow: Option<ComfyWorkflow>,
    workflow_texture: Option<egui::TextureHandle>,
    import_error: Option<String>,
    test_status: Option<String>,
}

impl ComfySettingsPanel {
    pub fn new() -> Self {
        Self {
            show: false,
            workflow: None,
            workflow_texture: None,
            import_error: None,
            test_status: None,
        }
    }

    pub fn render(&mut self, ctx: &egui::Context, config: &mut AgentConfig) -> bool {
        if !self.show {
            return false;
        }

        let mut should_save = false;
        let mut import_png_path: Option<PathBuf> = None;
        let mut import_json_path: Option<PathBuf> = None;
        let mut should_close = false;
        let mut should_test = false;

        let mut is_open = self.show;

        egui::Window::new("🎨 ComfyUI Workflow")
            .open(&mut is_open)
            .default_width(700.0)
            .show(ctx, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    // Import section
                    ui.heading("Import Workflow");
                    ui.add_space(8.0);

                    ui.horizontal(|ui| {
                        if ui.button("📁 Import from PNG").clicked() {
                            if let Some(path) = rfd::FileDialog::new()
                                .add_filter("PNG Image", &["png"])
                                .pick_file()
                            {
                                import_png_path = Some(path);
                            }
                        }

                        if ui.button("📄 Import from JSON").clicked() {
                            if let Some(path) = rfd::FileDialog::new()
                                .add_filter("JSON File", &["json"])
                                .pick_file()
                            {
                                import_json_path = Some(path);
                            }
                        }
                    });

                    if let Some(ref error) = self.import_error {
                        ui.colored_label(egui::Color32::RED, format!("Error: {}", error));
                    }

                    ui.add_space(16.0);
                    ui.separator();

                    // Workflow preview and settings
                    if let Some(ref mut workflow) = self.workflow {
                        ui.heading("Current Workflow");
                        ui.add_space(8.0);

                        ui.horizontal(|ui| {
                            // Preview image
                            if let Some(ref texture) = self.workflow_texture {
                                ui.image((texture.id(), egui::vec2(128.0, 128.0)));
                            } else if let Some(ref path) = workflow.preview_image_path {
                                // Try to load preview
                                if let Ok(img) = image::open(path) {
                                    let rgba = img.to_rgba8();
                                    let size = [rgba.width() as usize, rgba.height() as usize];
                                    let pixels = rgba.into_raw();
                                    let color_image = egui::ColorImage::from_rgba_unmultiplied(size, &pixels);
                                    let texture = ctx.load_texture("workflow_preview", color_image, Default::default());
                                    ui.image((texture.id(), egui::vec2(128.0, 128.0)));
                                    self.workflow_texture = Some(texture);
                                }
                            }

                            ui.vertical(|ui| {
                                ui.label(format!("Name: {}", workflow.name));
                                ui.label(format!("Output Node: {}", workflow.output_node_id));
                                ui.label(format!("Controllable Nodes: {}", workflow.controllable_nodes.len()));
                            });
                        });

                        ui.add_space(16.0);

                        // Controllable inputs
                        ui.heading("Controllable Inputs");
                        ui.label("Configure which inputs the agent can modify");
                        ui.add_space(8.0);

                        for (node_id, node) in &mut workflow.controllable_nodes {
                            ui.group(|ui| {
                                ui.label(format!("Node {} ({})", node_id, node.class_type));
                                ui.add_space(4.0);

                                for input in &mut node.inputs {
                                    ui.horizontal(|ui| {
                                        ui.checkbox(&mut input.agent_modifiable, "");

                                        ui.label(&input.name);
                                        ui.label(format!("({:?})", input.input_type));

                                        // Show current value
                                        match &input.input_type {
                                            InputType::Text => {
                                                if let Some(text) = input.default_value.as_str() {
                                                    let short = if text.len() > 30 {
                                                        format!("{}...", &text[..30])
                                                    } else {
                                                        text.to_string()
                                                    };
                                                    ui.label(format!("\"{}\"", short)).on_hover_text(text);
                                                }
                                            }
                                            InputType::Int | InputType::Seed => {
                                                if let Some(num) = input.default_value.as_i64() {
                                                    ui.label(format!("[{}]", num));
                                                }
                                            }
                                            InputType::Float => {
                                                if let Some(num) = input.default_value.as_f64() {
                                                    ui.label(format!("[{:.2}]", num));
                                                }
                                            }
                                            InputType::Bool => {
                                                if let Some(b) = input.default_value.as_bool() {
                                                    ui.label(format!("[{}]", b));
                                                }
                                            }
                                        }

                                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                            if input.agent_modifiable {
                                                ui.label("✓ Agent can modify").on_hover_text(&input.description);
                                            } else {
                                                ui.label("🔒 Fixed").on_hover_text(&input.description);
                                            }
                                        });
                                    });
                                }
                            });
                            ui.add_space(8.0);
                        }

                        ui.add_space(16.0);
                        ui.separator();
                    } else {
                        ui.label("No workflow loaded");
                        ui.label("Import a ComfyUI workflow PNG or JSON to get started");
                    }

                    ui.add_space(8.0);

                    // Test and save buttons
                    ui.horizontal(|ui| {
                        if ui.button("🧪 Test Workflow").clicked() {
                            should_test = true;
                        }

                        if ui.button("💾 Save").clicked() {
                            should_save = true;
                        }

                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.button("Cancel").clicked() {
                                should_close = true;
                            }
                        });
                    });

                    if let Some(ref status) = self.test_status {
                        ui.add_space(8.0);
                        if status.starts_with("✅") {
                            ui.colored_label(egui::Color32::GREEN, status);
                        } else {
                            ui.colored_label(egui::Color32::RED, status);
                        }
                    }
                });
            });

        // Update window state
        self.show = is_open && !should_close;

        // Handle import
        if let Some(path) = import_png_path {
            self.import_workflow_png(path);
        }
        if let Some(path) = import_json_path {
            self.import_workflow_json(path);
        }

        // Handle test
        if should_test {
            self.test_workflow(config);
        }

        // Handle save
        if should_save && self.workflow.is_some() {
            self.save_workflow_to_config(config);
            return true;
        }

        false
    }

    fn import_workflow_png(&mut self, path: PathBuf) {
        self.import_error = None;
        self.test_status = None;

        match crate::comfy_workflow::ComfyWorkflow::from_png(&path) {
            Ok(mut workflow) => {
                workflow.name = path.file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("Imported Workflow")
                    .to_string();

                tracing::info!("Imported workflow from PNG: {}", workflow.name);
                self.workflow = Some(workflow);
                self.workflow_texture = None; // Force reload
            }
            Err(e) => {
                self.import_error = Some(format!("Failed to import PNG: {}", e));
                tracing::error!("Workflow import error: {}", e);
            }
        }
    }

    fn import_workflow_json(&mut self, path: PathBuf) {
        self.import_error = None;
        self.test_status = None;

        match crate::comfy_workflow::ComfyWorkflow::from_json_file(&path) {
            Ok(mut workflow) => {
                workflow.name = path.file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("Imported Workflow")
                    .to_string();

                tracing::info!("Imported workflow from JSON: {}", workflow.name);
                self.workflow = Some(workflow);
            }
            Err(e) => {
                self.import_error = Some(format!("Failed to import JSON: {}", e));
                tracing::error!("Workflow import error: {}", e);
            }
        }
    }

    fn test_workflow(&mut self, config: &AgentConfig) {
        self.test_status = None;

        if !config.enable_image_generation {
            self.test_status = Some("❌ Image generation is disabled in settings".to_string());
            return;
        }

        // Test connection to ComfyUI
        let client = crate::comfy_client::ComfyUIClient::new(config.comfyui.api_url.clone());

        let runtime = tokio::runtime::Runtime::new().unwrap();
        match runtime.block_on(client.test_connection()) {
            Ok(_) => {
                self.test_status = Some(format!("✅ Connected to ComfyUI at {}", config.comfyui.api_url));
            }
            Err(e) => {
                self.test_status = Some(format!("❌ Failed to connect: {}", e));
            }
        }
    }

    fn save_workflow_to_config(&mut self, config: &mut AgentConfig) {
        if let Some(ref workflow) = self.workflow {
            // Serialize workflow to JSON string
            match serde_json::to_string(workflow) {
                Ok(json) => {
                    config.workflow_settings = Some(json);
                    if let Some(ref path) = workflow.preview_image_path {
                        config.workflow_path = Some(path.clone());
                    }
                    tracing::info!("Saved workflow settings to config");
                }
                Err(e) => {
                    tracing::error!("Failed to serialize workflow: {}", e);
                }
            }
        }
    }

    pub fn load_workflow_from_config(&mut self, config: &AgentConfig) {
        if let Some(ref json) = config.workflow_settings {
            match serde_json::from_str::<ComfyWorkflow>(json) {
                Ok(workflow) => {
                    tracing::info!("Loaded workflow from config: {}", workflow.name);
                    self.workflow = Some(workflow);
                }
                Err(e) => {
                    tracing::error!("Failed to deserialize workflow from config: {}", e);
                }
            }
        }
    }
}
