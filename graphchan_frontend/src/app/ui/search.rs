use eframe::egui::{self, Color32, RichText};
use crate::models::SearchResultView;
use super::super::{GraphchanApp, state::{SearchState, ViewState, ThreadState}};

pub fn render_search_results(app: &mut GraphchanApp, ui: &mut egui::Ui, state: &mut SearchState) {
    ui.heading(format!("Search: \"{}\"", state.query));
    ui.add_space(10.0);

    if state.is_loading {
        ui.horizontal(|ui| {
            ui.add(egui::Spinner::new());
            ui.label("Searching...");
        });
        return;
    }

    if let Some(err) = &state.error {
        ui.colored_label(Color32::LIGHT_RED, format!("Error: {}", err));
        if ui.button("Retry").clicked() {
            app.spawn_search(state.query.clone());
            state.is_loading = true;
            state.error = None;
        }
        return;
    }

    if state.results.is_empty() {
        ui.label(RichText::new("No results found").italics());
        return;
    }

    ui.label(format!("{} result(s)", state.results.len()));
    ui.add_space(10.0);

    let mut thread_to_open: Option<(String, String)> = None;

    egui::ScrollArea::vertical().show(ui, |ui| {
        for result in &state.results {
            render_search_result(app, ui, result, &mut thread_to_open);
            ui.add_space(12.0);
        }
    });

    if let Some((thread_id, post_id)) = thread_to_open {
        app.open_thread_and_scroll_to_post(thread_id, post_id);
    }
}

fn render_search_result(
    app: &mut GraphchanApp,
    ui: &mut egui::Ui,
    result: &SearchResultView,
    thread_to_open: &mut Option<(String, String)>,
) {
    egui::Frame::group(ui.style())
        .fill(ui.visuals().extreme_bg_color)
        .inner_margin(egui::vec2(12.0, 10.0))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());

            // Header
            ui.horizontal(|ui| {
                let type_badge = if result.result_type == "file" { "📎 File" } else { "💬 Post" };
                ui.label(RichText::new(type_badge).small().weak());

                ui.label(RichText::new(&result.thread_title)
                    .strong()
                    .color(Color32::from_rgb(100, 149, 237)));

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("📍 View in context").clicked() {
                        *thread_to_open = Some((result.thread_id.clone(), result.post.id.clone()));
                    }

                    ui.label(RichText::new(
                        &super::super::format_timestamp(&result.post.created_at)
                    ).weak());
                });
            });

            ui.add_space(6.0);

            // Author
            if let Some(author_id) = &result.post.author_peer_id {
                let author_name = app.peers.get(author_id)
                    .and_then(|p| p.username.as_deref())
                    .unwrap_or("Unknown");
                ui.label(RichText::new(format!("by {}", author_name)).italics().weak());

                // Display agent badge if post has agent metadata
                if let Some(metadata) = &result.post.metadata {
                    if let Some(agent) = &metadata.agent {
                        let badge_text = if let Some(version) = &agent.version {
                            format!("[Agent: {} v{}]", agent.name, version)
                        } else {
                            format!("[Agent: {}]", agent.name)
                        };
                        ui.label(
                            RichText::new(badge_text)
                                .size(10.0)
                                .color(Color32::from_rgb(150, 200, 255))
                        );
                    }
                }

                ui.add_space(4.0);
            }

            // File info for file results
            if let Some(file) = &result.file {
                ui.label(RichText::new(&file.original_name.clone().unwrap_or_else(|| "Unnamed file".to_string())).strong());
                if let Some(size) = file.size_bytes {
                    ui.label(RichText::new(format!("{} bytes", size)).small().weak());
                }
                ui.add_space(4.0);
            }

            // Snippet with highlighting
            render_snippet(ui, &result.snippet);

            ui.add_space(6.0);

            // Footer
            ui.horizontal(|ui| {
                ui.label(RichText::new(format!("Score: {:.2}", result.bm25_score))
                    .small().weak());
                ui.label(RichText::new(&result.post.id)
                    .small().monospace().weak());
            });
        });
}

fn render_snippet(ui: &mut egui::Ui, snippet: &str) {
    let mut current_pos = 0;
    ui.horizontal_wrapped(|ui| {
        while current_pos < snippet.len() {
            if let Some(mark_start) = snippet[current_pos..].find("<mark>") {
                let absolute_start = current_pos + mark_start;

                if mark_start > 0 {
                    ui.label(&snippet[current_pos..absolute_start]);
                }

                let search_from = absolute_start + 6;
                if let Some(mark_end) = snippet[search_from..].find("</mark>") {
                    let absolute_end = search_from + mark_end;

                    ui.label(RichText::new(&snippet[search_from..absolute_end])
                        .background_color(Color32::from_rgb(255, 255, 100))
                        .strong());

                    current_pos = absolute_end + 7;
                } else {
                    ui.label(&snippet[current_pos..]);
                    break;
                }
            } else {
                ui.label(&snippet[current_pos..]);
                break;
            }
        }
    });
}
