use std::collections::{HashMap, HashSet};

use eframe::egui;

use crate::models::{FileResponse, ThreadDetails, ThreadSummary};

#[derive(Default)]
pub struct CreateThreadState {
    pub title: String,
    pub body: String,
    pub submitting: bool,
    pub error: Option<String>,
}

#[derive(Default)]
pub struct ImporterState {
    pub open: bool,
    pub url: String,
    pub importing: bool,
    pub error: Option<String>,
}

pub enum ViewState {
    Catalog,
    Thread(ThreadState),
}

pub struct ThreadState {
    pub summary: ThreadSummary,
    pub details: Option<ThreadDetails>,
    pub is_loading: bool,
    pub error: Option<String>,
    pub new_post_body: String,
    pub new_post_error: Option<String>,
    pub new_post_sending: bool,
    pub attachments: HashMap<String, Vec<FileResponse>>,
    pub attachments_loading: HashSet<String>,
    pub attachments_errors: HashMap<String, String>,
    pub display_mode: ThreadDisplayMode,
    pub last_layout_mode: Option<ThreadDisplayMode>,
    pub graph_nodes: HashMap<String, GraphNode>,
    pub selected_post: Option<String>,
    pub graph_zoom: f32,
    pub graph_offset: egui::Vec2,
    pub graph_dragging: bool,
    pub reply_to: Vec<String>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ThreadDisplayMode {
    List,
    Graph,
    Chronological,
}

pub struct GraphNode {
    pub pos: egui::Pos2,
    pub vel: egui::Vec2,
    pub size: egui::Vec2,
    pub dragging: bool,
}

#[derive(Clone)]
pub struct LoadedImage {
    pub size: [usize; 2],
    pub pixels: Vec<u8>,
}
