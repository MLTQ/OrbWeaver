use eframe::egui::{self, Color32, RichText, ScrollArea};

use crate::app::{state::BlockingState, GraphchanApp};

pub fn render_blocking_page(app: &mut GraphchanApp, ui: &mut egui::Ui, state: &mut BlockingState) {
    ui.horizontal(|ui| {
        if ui.button("← Back to Catalog").clicked() {
            app.view = crate::app::state::ViewState::Catalog;
        }
        ui.heading("Privacy & Moderation");
    });

    ui.separator();

    // Tab selection
    ui.horizontal(|ui| {
        ui.selectable_value(&mut state.current_tab, 0, "Blocked Peers");
        ui.selectable_value(&mut state.current_tab, 1, "Blocklists");
        ui.selectable_value(&mut state.current_tab, 2, "IP Blocks");
    });

    ui.separator();

    match state.current_tab {
        0 => render_blocked_peers_tab(app, ui, state),
        1 => render_blocklists_tab(app, ui, state),
        2 => render_ip_blocks_tab(app, ui, state),
        _ => {}
    }
}

fn render_blocked_peers_tab(app: &mut GraphchanApp, ui: &mut egui::Ui, state: &mut BlockingState) {
    // Two-column layout: Blocked Peers | Block Form
    ui.columns(2, |columns| {
        // Column 1: Blocked Peers
        columns[0].vertical(|ui| {
            ui.heading("Blocked Peers");
            ui.add_space(10.0);

            if state.blocked_peers_loading && state.blocked_peers.is_empty() {
                ui.add(egui::Spinner::new());
            }

            if let Some(err) = &state.blocked_peers_error {
                ui.colored_label(Color32::LIGHT_RED, format!("Error: {}", err));
                if ui.button("Retry").clicked() {
                    app.spawn_load_blocked_peers();
                }
            }

            render_blocked_peers_list(app, ui, state);

            ui.add_space(20.0);
            ui.separator();
            ui.add_space(10.0);

            // Block new peer form
            ui.heading("Block a Peer");
            ui.horizontal(|ui| {
                ui.label("Peer ID:");
                ui.add(
                    egui::TextEdit::singleline(&mut state.new_block_peer_id)
                        .hint_text("Enter peer ID...")
                );
            });

            ui.horizontal(|ui| {
                ui.label("Reason:");
                ui.add(
                    egui::TextEdit::singleline(&mut state.new_block_reason)
                        .hint_text("Optional reason...")
                );
            });

            if ui.add_enabled(!state.blocking_in_progress, egui::Button::new("Block Peer")).clicked() {
                app.spawn_block_peer(state);
            }

            if let Some(err) = &state.block_error {
                ui.colored_label(Color32::LIGHT_RED, format!("Error: {}", err));
            }

            if state.blocking_in_progress {
                ui.add(egui::Spinner::new());
            }
        });

        // Column 2: Blocklists
        columns[1].vertical(|ui| {
            ui.heading("Subscribed Blocklists");
            ui.add_space(10.0);

            if state.blocklists_loading && state.blocklists.is_empty() {
                ui.add(egui::Spinner::new());
            }

            if let Some(err) = &state.blocklists_error {
                ui.colored_label(Color32::LIGHT_RED, format!("Error: {}", err));
                if ui.button("Retry").clicked() {
                    app.spawn_load_blocklists();
                }
            }

            render_blocklists_list(app, ui, state);

            ui.add_space(20.0);
            ui.separator();
            ui.add_space(10.0);

            // Subscribe to blocklist form
            ui.heading("Subscribe to Blocklist");
            ui.horizontal(|ui| {
                ui.label("Blocklist ID:");
                ui.add(
                    egui::TextEdit::singleline(&mut state.new_blocklist_id)
                        .hint_text("Enter blocklist ID...")
                );
            });

            ui.horizontal(|ui| {
                ui.label("Maintainer:");
                ui.add(
                    egui::TextEdit::singleline(&mut state.new_blocklist_maintainer)
                        .hint_text("Maintainer peer ID...")
                );
            });

            ui.horizontal(|ui| {
                ui.label("Name:");
                ui.add(
                    egui::TextEdit::singleline(&mut state.new_blocklist_name)
                        .hint_text("Blocklist name...")
                );
            });

            ui.horizontal(|ui| {
                ui.label("Description:");
                ui.add(
                    egui::TextEdit::singleline(&mut state.new_blocklist_description)
                        .hint_text("Optional description...")
                );
            });

            ui.checkbox(&mut state.new_blocklist_auto_apply, "Auto-apply blocks from this list");

            if ui.add_enabled(!state.subscribing_in_progress, egui::Button::new("Subscribe")).clicked() {
                app.spawn_subscribe_blocklist(state);
            }

            if let Some(err) = &state.subscribe_error {
                ui.colored_label(Color32::LIGHT_RED, format!("Error: {}", err));
            }

            if state.subscribing_in_progress {
                ui.add(egui::Spinner::new());
            }
        });
    });
}

fn render_blocklists_tab(app: &mut GraphchanApp, ui: &mut egui::Ui, state: &mut BlockingState) {
    // Two-column layout: Blocklists | Subscribe Form
    ui.columns(2, |columns| {
        // Column 1: Blocklists
        columns[0].vertical(|ui| {
            ui.heading("Subscribed Blocklists");
            ui.add_space(10.0);

            if state.blocklists_loading && state.blocklists.is_empty() {
                ui.add(egui::Spinner::new());
            }

            if let Some(err) = &state.blocklists_error {
                ui.colored_label(Color32::LIGHT_RED, format!("Error: {}", err));
                if ui.button("Retry").clicked() {
                    app.spawn_load_blocklists();
                }
            }

            render_blocklists_list(app, ui, state);
        });

        // Column 2: Subscribe Form
        columns[1].vertical(|ui| {
            ui.heading("Subscribe to Blocklist");
            ui.horizontal(|ui| {
                ui.label("Blocklist ID:");
                ui.add(
                    egui::TextEdit::singleline(&mut state.new_blocklist_id)
                        .hint_text("Enter blocklist ID...")
                );
            });

            ui.horizontal(|ui| {
                ui.label("Maintainer:");
                ui.add(
                    egui::TextEdit::singleline(&mut state.new_blocklist_maintainer)
                        .hint_text("Maintainer peer ID...")
                );
            });

            ui.horizontal(|ui| {
                ui.label("Name:");
                ui.add(
                    egui::TextEdit::singleline(&mut state.new_blocklist_name)
                        .hint_text("Blocklist name...")
                );
            });

            ui.horizontal(|ui| {
                ui.label("Description:");
                ui.add(
                    egui::TextEdit::singleline(&mut state.new_blocklist_description)
                        .hint_text("Optional description...")
                );
            });

            ui.checkbox(&mut state.new_blocklist_auto_apply, "Auto-apply blocks from this list");

            if ui.add_enabled(!state.subscribing_in_progress, egui::Button::new("Subscribe")).clicked() {
                app.spawn_subscribe_blocklist(state);
            }

            if let Some(err) = &state.subscribe_error {
                ui.colored_label(Color32::LIGHT_RED, format!("Error: {}", err));
            }

            if state.subscribing_in_progress {
                ui.add(egui::Spinner::new());
            }
        });
    });
}

fn render_ip_blocks_tab(app: &mut GraphchanApp, ui: &mut egui::Ui, state: &mut BlockingState) {
    // Two-column layout: IP Blocks List | Add/Import Form
    ui.columns(2, |columns| {
        // Column 1: IP Blocks List + Stats
        columns[0].vertical(|ui| {
            ui.heading("Blocked IPs");
            ui.add_space(10.0);

            // Stats
            if let Some(stats) = &state.ip_block_stats {
                ui.horizontal(|ui| {
                    ui.label(RichText::new(format!("Total: {}", stats.total_blocks)).weak());
                    ui.label(RichText::new("•").weak());
                    ui.label(RichText::new(format!("Active: {}", stats.active_blocks)).weak());
                    ui.label(RichText::new("•").weak());
                    ui.label(RichText::new(format!("Hits: {}", stats.total_hits)).weak());
                });
                ui.label(RichText::new(format!("Exact IPs: {} | Ranges: {}", stats.exact_ip_blocks, stats.range_blocks)).weak());
                ui.add_space(5.0);
            }

            if state.ip_blocks_loading && state.ip_blocks.is_empty() {
                ui.add(egui::Spinner::new());
            }

            if let Some(err) = &state.ip_blocks_error {
                ui.colored_label(Color32::LIGHT_RED, format!("Error: {}", err));
                if ui.button("Retry").clicked() {
                    app.spawn_load_ip_blocks();
                }
            }

            render_ip_blocks_list(app, ui, state);

            ui.add_space(10.0);

            // Clear all button
            if ui.add_enabled(!state.ip_blocks.is_empty(), egui::Button::new("Clear All IP Blocks")).clicked() {
                state.showing_clear_all_confirmation = true;
            }

            if ui.button("Export Blocklist").clicked() {
                app.spawn_export_ip_blocks();
            }
        });

        // Column 2: Add/Import Form
        columns[1].vertical(|ui| {
            ui.heading("Add IP Block");
            ui.horizontal(|ui| {
                ui.label("IP/Range:");
                ui.add(
                    egui::TextEdit::singleline(&mut state.new_ip_block)
                        .hint_text("e.g., 192.168.1.1 or 41.0.0.0/8")
                );
            });

            ui.horizontal(|ui| {
                ui.label("Reason:");
                ui.add(
                    egui::TextEdit::singleline(&mut state.new_ip_block_reason)
                        .hint_text("Optional reason...")
                );
            });

            if ui.add_enabled(!state.adding_ip_block, egui::Button::new("Block IP")).clicked() {
                app.spawn_add_ip_block(state);
            }

            if let Some(err) = &state.add_ip_block_error {
                ui.colored_label(Color32::LIGHT_RED, format!("Error: {}", err));
            }

            if state.adding_ip_block {
                ui.add(egui::Spinner::new());
            }

            ui.add_space(20.0);
            ui.separator();
            ui.add_space(10.0);

            // Import section
            ui.heading("Import Blocklist");
            ui.label("Paste blocklist (one IP/range per line, # for comments):");

            ui.add(
                egui::TextEdit::multiline(&mut state.import_text)
                    .desired_rows(8)
                    .hint_text("192.168.1.1 # Spam bot\n41.0.0.0/8 # Country block")
            );

            if ui.add_enabled(!state.importing_ips, egui::Button::new("Import")).clicked() {
                app.spawn_import_ip_blocks(state);
            }

            if let Some(err) = &state.import_error {
                ui.colored_label(Color32::LIGHT_RED, format!("Error: {}", err));
            }

            if state.importing_ips {
                ui.add(egui::Spinner::new());
            }
        });
    });
}

fn render_blocked_peers_list(app: &mut GraphchanApp, ui: &mut egui::Ui, state: &mut BlockingState) {
    if state.blocked_peers.is_empty() {
        ui.label("No blocked peers.");
        return;
    }

    let mut peer_to_unblock: Option<String> = None;

    ScrollArea::vertical().max_height(300.0).show(ui, |ui| {
        for blocked in &state.blocked_peers {
            egui::Frame::group(ui.style())
                .fill(ui.visuals().extreme_bg_color)
                .inner_margin(egui::vec2(12.0, 8.0))
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());

                    ui.horizontal(|ui| {
                        // Look up peer info if available
                        let peer_name = app.peers.get(&blocked.peer_id)
                            .and_then(|p| p.username.as_deref().or(p.alias.as_deref()))
                            .unwrap_or(&blocked.peer_id);

                        ui.label(RichText::new(peer_name).strong());

                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.small_button("Unblock").clicked() {
                                peer_to_unblock = Some(blocked.peer_id.clone());
                            }
                        });
                    });

                    ui.label(RichText::new(&blocked.peer_id).monospace().size(10.0));

                    if let Some(reason) = &blocked.reason {
                        ui.label(RichText::new(format!("Reason: {}", reason)).size(11.0).weak());
                    }

                    ui.label(RichText::new(format!("Blocked: {}", blocked.blocked_at)).size(9.0).weak());
                });

            ui.add_space(8.0);
        }
    });

    if let Some(peer_id) = peer_to_unblock {
        app.spawn_unblock_peer(peer_id);
    }
}

fn render_blocklists_list(app: &mut GraphchanApp, ui: &mut egui::Ui, state: &mut BlockingState) {
    if state.blocklists.is_empty() {
        ui.label("Not subscribed to any blocklists.");
        ui.label("Subscribe to community-maintained blocklists above.");
        return;
    }

    let mut blocklist_to_unsubscribe: Option<String> = None;
    let mut blocklist_to_view: Option<String> = None;

    ScrollArea::vertical().max_height(300.0).show(ui, |ui| {
        for blocklist in &state.blocklists {
            egui::Frame::group(ui.style())
                .fill(ui.visuals().extreme_bg_color)
                .inner_margin(egui::vec2(12.0, 8.0))
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());

                    ui.horizontal(|ui| {
                        ui.label(RichText::new(&blocklist.name).strong());

                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.small_button("Unsubscribe").clicked() {
                                blocklist_to_unsubscribe = Some(blocklist.id.clone());
                            }

                            if ui.small_button("View Entries").clicked() {
                                blocklist_to_view = Some(blocklist.id.clone());
                            }

                            if blocklist.auto_apply {
                                ui.label(
                                    RichText::new("AUTO-APPLY")
                                        .color(Color32::from_rgb(255, 200, 100))
                                        .strong()
                                );
                            }
                        });
                    });

                    ui.label(RichText::new(format!("ID: {}", blocklist.id)).monospace().size(10.0));
                    ui.label(RichText::new(format!("Maintainer: {}", blocklist.maintainer_peer_id)).size(10.0));

                    if let Some(desc) = &blocklist.description {
                        ui.label(RichText::new(desc).size(11.0).weak());
                    }

                    if let Some(last_synced) = &blocklist.last_synced_at {
                        ui.label(RichText::new(format!("Last synced: {}", last_synced)).size(9.0).weak());
                    }
                });

            ui.add_space(8.0);
        }
    });

    if let Some(blocklist_id) = blocklist_to_unsubscribe {
        app.spawn_unsubscribe_blocklist(blocklist_id);
    }

    if let Some(blocklist_id) = blocklist_to_view {
        app.spawn_load_blocklist_entries(blocklist_id);
    }
}

fn render_ip_blocks_list(app: &mut GraphchanApp, ui: &mut egui::Ui, state: &mut BlockingState) {
    if state.ip_blocks.is_empty() {
        ui.label("No IP blocks.");
        return;
    }

    let mut block_to_remove: Option<i64> = None;

    ScrollArea::vertical().max_height(400.0).show(ui, |ui| {
        for block in &state.ip_blocks {
            egui::Frame::group(ui.style())
                .fill(ui.visuals().extreme_bg_color)
                .inner_margin(egui::vec2(12.0, 8.0))
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());

                    ui.horizontal(|ui| {
                        let label_text = if block.block_type == "range" {
                            format!("{} (range)", block.ip_or_range)
                        } else {
                            block.ip_or_range.clone()
                        };
                        ui.label(RichText::new(label_text).strong().monospace());

                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.small_button("Remove").clicked() {
                                block_to_remove = Some(block.id);
                            }

                            if block.hit_count > 0 {
                                ui.label(
                                    RichText::new(format!("Hits: {}", block.hit_count))
                                        .color(Color32::from_rgb(255, 200, 100))
                                        .size(10.0)
                                );
                            }
                        });
                    });

                    if let Some(reason) = &block.reason {
                        ui.label(RichText::new(format!("Reason: {}", reason)).size(11.0).weak());
                    }

                    let blocked_time = chrono::DateTime::from_timestamp(block.blocked_at, 0)
                        .map(|dt| dt.format("%Y-%m-%d %H:%M").to_string())
                        .unwrap_or_else(|| "Unknown".to_string());
                    ui.label(RichText::new(format!("Blocked: {}", blocked_time)).size(9.0).weak());
                });

            ui.add_space(8.0);
        }
    });

    if let Some(block_id) = block_to_remove {
        app.spawn_remove_ip_block(block_id);
    }
}

pub fn render_blocklist_entries_dialog(_app: &mut GraphchanApp, ctx: &egui::Context, state: &mut BlockingState) {
    if let Some(blocklist_id) = &state.viewing_blocklist_id.clone() {
        egui::Window::new("Blocklist Entries")
            .default_width(500.0)
            .show(ctx, |ui| {
                ui.label(RichText::new(format!("Blocklist: {}", blocklist_id)).strong());
                ui.separator();

                if state.blocklist_entries_loading && state.blocklist_entries.is_empty() {
                    ui.add(egui::Spinner::new());
                }

                if let Some(err) = &state.blocklist_entries_error {
                    ui.colored_label(Color32::LIGHT_RED, format!("Error: {}", err));
                }

                ScrollArea::vertical().max_height(400.0).show(ui, |ui| {
                    if state.blocklist_entries.is_empty() {
                        ui.label("No entries in this blocklist.");
                    } else {
                        for entry in &state.blocklist_entries {
                            egui::Frame::group(ui.style())
                                .fill(ui.visuals().extreme_bg_color)
                                .inner_margin(egui::vec2(10.0, 6.0))
                                .show(ui, |ui| {
                                    ui.label(RichText::new(&entry.peer_id).monospace());

                                    if let Some(reason) = &entry.reason {
                                        ui.label(RichText::new(format!("Reason: {}", reason)).size(11.0).weak());
                                    }

                                    ui.label(RichText::new(format!("Added: {}", entry.added_at)).size(9.0).weak());
                                });

                            ui.add_space(4.0);
                        }
                    }
                });

                ui.separator();

                if ui.button("Close").clicked() {
                    state.viewing_blocklist_id = None;
                    state.blocklist_entries.clear();
                }
            });
    }
}

pub fn render_clear_all_ip_blocks_dialog(app: &mut GraphchanApp, ctx: &egui::Context, state: &mut BlockingState) {
    if state.showing_clear_all_confirmation {
        egui::Window::new("Confirm Clear All IP Blocks")
            .collapsible(false)
            .resizable(false)
            .show(ctx, |ui| {
                ui.label("Are you sure you want to remove ALL IP blocks?");
                ui.label(RichText::new("This action cannot be undone.").color(Color32::LIGHT_RED));

                ui.add_space(10.0);

                ui.horizontal(|ui| {
                    if ui.button("Cancel").clicked() {
                        state.showing_clear_all_confirmation = false;
                    }

                    if ui.button(RichText::new("Clear All").color(Color32::RED)).clicked() {
                        app.spawn_clear_all_ip_blocks();
                        state.showing_clear_all_confirmation = false;
                    }
                });
            });
    }
}
