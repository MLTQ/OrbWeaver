use std::collections::{HashMap, HashSet};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};


use eframe::egui;

use crate::models::{
    ConversationView, DirectMessageView, FileResponse, PeerView, ReactionsResponse, SearchResultView, ThreadDetails,
    ThreadSummary,
};

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
    Messages,  // List of all DM conversations (formerly "Private Threads" in catalog)
    Thread(ThreadState),
    Following,
    FollowingCatalog(PeerView),
    Import,
    Settings,
    Conversation(ConversationState),
    Blocking,
    SearchResults(SearchState),
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
    #[serde(skip)]
    pub scroll_to_post: Option<String>, // Post ID to scroll to in List view
    pub graph_zoom: f32,
    pub graph_offset: egui::Vec2,
    pub graph_dragging: bool,
    pub reply_to: Vec<String>,
    pub time_bin_seconds: i64, // Time bin size in seconds for chronological view
    pub locked_hover_post: Option<String>, // "Locked" hover for tracing conversations
    pub repulsion_force: f32,
    pub desired_edge_length: f32,
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

// Direct Messages State

#[derive(Default)]
pub struct ConversationState {
    pub peer_id: String,
    pub peer_info: Option<PeerView>,
    pub messages: Vec<DirectMessageView>,
    pub messages_loading: bool,
    pub messages_error: Option<String>,
    pub new_message_body: String,
    pub sending: bool,
    pub send_error: Option<String>,
}

#[derive(Default)]
pub struct DmState {
    pub conversations: Vec<ConversationView>,
    pub conversations_loading: bool,
    pub conversations_error: Option<String>,
    pub unread_count: usize,
}

// Blocking State

#[derive(Default)]
pub struct BlockingState {
    // Blocked peers
    pub blocked_peers: Vec<crate::models::BlockedPeerView>,
    pub blocked_peers_loading: bool,
    pub blocked_peers_error: Option<String>,

    // Block new peer form
    pub new_block_peer_id: String,
    pub new_block_reason: String,
    pub blocking_in_progress: bool,
    pub block_error: Option<String>,

    // Blocklists
    pub blocklists: Vec<crate::models::BlocklistSubscriptionView>,
    pub blocklists_loading: bool,
    pub blocklists_error: Option<String>,

    // Subscribe to blocklist form
    pub new_blocklist_id: String,
    pub new_blocklist_maintainer: String,
    pub new_blocklist_name: String,
    pub new_blocklist_description: String,
    pub new_blocklist_auto_apply: bool,
    pub subscribing_in_progress: bool,
    pub subscribe_error: Option<String>,

    // Viewing blocklist entries
    pub viewing_blocklist_id: Option<String>,
    pub blocklist_entries: Vec<crate::models::BlocklistEntryView>,
    pub blocklist_entries_loading: bool,
    pub blocklist_entries_error: Option<String>,

    // IP Blocking
    pub ip_blocks: Vec<crate::models::IpBlockView>,
    pub ip_blocks_loading: bool,
    pub ip_blocks_error: Option<String>,

    // Add new IP block form
    pub new_ip_block: String,
    pub new_ip_block_reason: String,
    pub adding_ip_block: bool,
    pub add_ip_block_error: Option<String>,

    // IP block stats
    pub ip_block_stats: Option<crate::models::IpBlockStatsResponse>,
    pub ip_block_stats_loading: bool,

    // Import/Export
    pub import_text: String,
    pub importing_ips: bool,
    pub import_error: Option<String>,

    // Clear all confirmation
    pub showing_clear_all_confirmation: bool,

    // Current tab (0 = peers, 1 = blocklists, 2 = IP blocks)
    pub current_tab: usize,
}

// Search State

#[derive(Default)]
pub struct SearchState {
    pub query: String,
    pub results: Vec<SearchResultView>,
    pub is_loading: bool,
    pub error: Option<String>,
}
