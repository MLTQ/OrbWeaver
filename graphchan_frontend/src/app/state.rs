use std::collections::{HashMap, HashSet};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};


use eframe::egui;

use crate::models::{FileResponse, PeerView, ReactionsResponse, ThreadDetails, ThreadSummary};

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
    Following,
    FollowingCatalog(PeerView),
    Import,
    Settings,
}

#[derive(Default)]
pub struct RedditImporterState {
    pub url: String,
    pub importing: bool,
    pub error: Option<String>,
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

#[derive(Default, Clone, Copy, PartialEq, Eq)]
pub enum NavigationMode {
    #[default]
    None,      // No navigation used yet - default to reply
    Parent,    // User navigated parents (u, o, p, i)
    Reply,     // User navigated replies (j, l, ;, k)
}

#[derive(Default, Serialize, Deserialize)]
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
    pub reactions: HashMap<String, ReactionsResponse>,
    pub reactions_loading: HashSet<String>,
    pub emoji_picker_open: Option<String>, // Post ID for which emoji picker is open
    pub display_mode: ThreadDisplayMode,
    pub last_layout_mode: Option<ThreadDisplayMode>,
    pub graph_nodes: HashMap<String, GraphNode>,
    pub chronological_nodes: HashMap<String, GraphNode>,
    pub sugiyama_nodes: HashMap<String, GraphNode>,
    #[serde(skip)]
    pub sim_start_time: Option<std::time::Instant>,      // Track simulation time per thread
    #[serde(skip)]
    pub secondary_selected_post: Option<String>,
    #[serde(skip)]
    pub parent_cursor_index: usize, // Index into parent_post_ids of selected post
    #[serde(skip)]
    pub reply_cursor_index: usize,  // Index into replies of selected post
    #[serde(skip)]
    pub last_navigation_mode: NavigationMode, // Track which navigation was last used
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
    pub is_hosting: bool, // True = Host (rebroadcast), False = Leech (don't rebroadcast)

    // Audio State
    #[serde(skip)]
    pub audio_stream: Option<rodio::OutputStream>,
    #[serde(skip)]
    pub audio_handle: Option<rodio::OutputStreamHandle>,
    #[serde(skip)]
    pub audio_sink: Option<rodio::Sink>,
    #[serde(skip)]
    pub currently_playing: Option<String>, // File ID or Path
    #[serde(skip)]
    pub audio_promise: Option<poll_promise::Promise<Result<(String, Vec<u8>), String>>>,
}

#[derive(Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum ThreadDisplayMode {
    #[default]
    List,
    Graph,
    Chronological,
    Sugiyama,
}

#[derive(Default, Serialize, Deserialize)]
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
