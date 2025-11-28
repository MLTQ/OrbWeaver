use std::collections::{HashMap, HashSet};

use eframe::egui;

use crate::models::{FileResponse, PeerView, ThreadDetails, ThreadSummary};

#[derive(Default)]
pub struct CreateThreadState {
    pub title: String,
    pub body: String,
    pub submitting: bool,
    pub error: Option<String>,
    pub files: Vec<std::path::PathBuf>,
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
    Friends,
}

#[derive(Default)]
pub struct IdentityState {
    pub local_peer: Option<PeerView>,
    pub avatar_path: Option<String>,
    pub uploading: bool,
    pub error: Option<String>,
    pub username_input: String,
    pub bio_input: String,
    pub initialized_inputs: bool,
    pub inspected_peer: Option<PeerView>,
    pub cropper_state: Option<AvatarCropperState>,
    pub friendcode_input: String,
    pub adding_peer: bool,
}

pub struct AvatarCropperState {
    pub image: egui::ColorImage,
    pub pan: egui::Vec2,
    pub zoom: f32,
    pub source_path: String,
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
    pub chronological_nodes: HashMap<String, GraphNode>, // Separate state for chronological view
    pub sim_start_time: Option<std::time::Instant>,      // Track simulation time per thread
    pub selected_post: Option<String>,
    pub graph_zoom: f32,
    pub graph_offset: egui::Vec2,
    pub graph_dragging: bool,
    pub reply_to: Vec<String>,
    pub time_bin_seconds: i64, // Time bin size in seconds for chronological view
    pub locked_hover_post: Option<String>, // "Locked" hover for tracing conversations
    pub repulsion_force: f32,
    pub sim_paused: bool,
    pub draft_attachments: Vec<std::path::PathBuf>,
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
    pub pinned: bool,
}

#[derive(Clone)]
pub struct LoadedImage {
    pub size: [usize; 2],
    pub pixels: Vec<u8>,
}
