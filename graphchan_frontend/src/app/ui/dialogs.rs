use eframe::egui::{self, Align2, Color32, Context};

use super::super::state::CreateThreadState;
use super::super::GraphchanApp;

impl GraphchanApp {
    pub(crate) fn render_create_thread_dialog(&mut self, ctx: &Context) {
        if !self.show_create_thread {
            return;
        }

        let mut should_close = false;
        let mut should_submit = false;

        egui::Window::new("Create Thread")
            .open(&mut self.show_create_thread)
            .default_width(400.0)
            .anchor(Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
            .show(ctx, |ui| {
                if let Some(err) = &self.create_thread.error {
                    ui.colored_label(Color32::LIGHT_RED, err);
                }
                ui.label("Title");
                ui.text_edit_singleline(&mut self.create_thread.title);
                ui.add_space(6.0);
                ui.label("Body (optional)");
                ui.add(
                    egui::TextEdit::multiline(&mut self.create_thread.body)
                        .desired_rows(6)
                        .hint_text("Write the opening post..."),
                );
                ui.add_space(12.0);
                ui.horizontal(|ui| {
                    if self.create_thread.submitting {
                        ui.add(egui::Spinner::new());
                    } else if ui.button("Create").clicked() {
                        should_submit = true;
                    }
                    if ui.button("Cancel").clicked() {
                        should_close = true;
                    }
                });
            });

        if should_submit {
            self.spawn_create_thread();
        }
        if should_close {
            self.show_create_thread = false;
            self.create_thread = CreateThreadState::default();
        }
    }

    pub(crate) fn render_import_dialog(&mut self, ctx: &Context) {
        if !self.importer.open {
            return;
        }

        let mut should_import = false;
        let mut should_close = false;

        egui::Window::new("Import from 4chan")
            .open(&mut self.importer.open)
            .resizable(false)
            .default_width(420.0)
            .anchor(Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
            .show(ctx, |ui| {
                ui.label("Paste a thread URL (e.g. https://boards.4chan.org/g/thread/123456789)");
                ui.text_edit_singleline(&mut self.importer.url);
                if let Some(err) = &self.importer.error {
                    ui.colored_label(Color32::LIGHT_RED, err);
                }
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    if self.importer.importing {
                        ui.add(egui::Spinner::new());
                    } else if ui.button("Import").clicked() {
                        should_import = true;
                    }
                    if ui.button("Close").clicked() {
                        should_close = true;
                    }
                });
            });

        if should_import {
            self.spawn_import_fourchan();
        }
        if should_close {
            self.importer.open = false;
        }
    }
}
