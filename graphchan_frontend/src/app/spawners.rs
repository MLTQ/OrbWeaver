use super::state;
use super::tasks;
use super::GraphchanApp;
use crate::models::CreatePostInput;

use super::state::{ConversationState, ThreadState};

impl GraphchanApp {
    pub(super) fn spawn_load_threads(&mut self) {
        if self.threads_loading {
            return;
        }
        self.threads_loading = true;
        self.threads_error = None;
        self.is_refreshing = true;
        self.last_refresh_time = Some(std::time::Instant::now());
        tasks::load_threads(self.api.clone(), self.tx.clone());
    }

    pub(super) fn spawn_load_thread(&mut self, thread_id: &str) {
        tasks::load_thread(self.api.clone(), self.tx.clone(), thread_id.to_string(), false);
    }

    pub(super) fn spawn_refresh_thread(&mut self, thread_id: &str) {
        tasks::load_thread(self.api.clone(), self.tx.clone(), thread_id.to_string(), true);
    }

    pub(super) fn spawn_create_thread(&mut self) {
        let title = self.create_thread.title.trim().to_string();
        if title.is_empty() {
            self.create_thread.error = Some("Title cannot be empty".into());
            return;
        }
        let body = self.create_thread.body.trim().to_string();
        let mut payload = crate::models::CreateThreadInput::default();
        payload.title = title;
        if !body.is_empty() {
            payload.body = Some(body);
        }
        // Set topics to announce on
        payload.topics = self.selected_topics.iter().cloned().collect();
        // Keep visibility for backward compatibility (deprecated)
        payload.visibility = Some("social".to_string());
        self.create_thread.submitting = true;
        self.create_thread.error = None;
        tasks::create_thread(self.api.clone(), self.tx.clone(), payload, self.create_thread.files.clone());
    }

    pub(super) fn spawn_create_post(&mut self, thread_state: &mut ThreadState) {
        let body = thread_state.new_post_body.trim().to_string();
        if body.is_empty() {
            thread_state.new_post_error = Some("Post body cannot be empty".into());
            return;
        }
        let thread_id = thread_state.summary.id.clone();
        let mut payload = CreatePostInput::default();
        payload.thread_id = thread_id.clone();
        payload.body = body;
        payload.parent_post_ids = thread_state.reply_to.clone();
        payload.rebroadcast = thread_state.is_hosting; // Use the Host/Leech toggle state
        let attachments = thread_state.draft_attachments.clone();
        thread_state.new_post_sending = true;
        thread_state.new_post_error = None;
        tasks::create_post(self.api.clone(), self.tx.clone(), thread_id, payload, attachments);
    }

    pub(super) fn spawn_import_fourchan(&mut self) {
        let url = self.importer.url.trim().to_string();
        if url.is_empty() {
            self.importer.error = Some("Paste a full 4chan thread URL".into());
            return;
        }
        self.importer.importing = true;
        self.importer.error = None;
        tasks::import_fourchan(self.api.clone(), self.tx.clone(), url);
    }

    pub(super) fn spawn_load_identity(&mut self) {
        tasks::load_identity(self.api.clone(), self.tx.clone());
    }

    pub(super) fn spawn_load_peers(&mut self) {
        tasks::load_peers(self.api.clone(), self.tx.clone());
    }

    pub(super) fn spawn_load_topics(&mut self) {
        if self.topics_loading {
            return;
        }
        self.topics_loading = true;
        self.topics_error = None;
        tasks::load_topics(self.api.clone(), self.tx.clone());
    }

    pub(super) fn spawn_load_theme_color(&mut self) {
        tasks::load_theme_color(self.api.clone(), self.tx.clone());
    }

    pub(super) fn spawn_subscribe_topic(&mut self, topic_id: String) {
        tasks::subscribe_topic(self.api.clone(), self.tx.clone(), topic_id);
    }

    pub(super) fn spawn_unsubscribe_topic(&mut self, topic_id: String) {
        tasks::unsubscribe_topic(self.api.clone(), self.tx.clone(), topic_id);
    }

    pub(super) fn spawn_load_recent_posts(&mut self) {
        if self.recent_posts_loading {
            return;
        }
        self.recent_posts_loading = true;
        self.recent_posts_error = None;
        self.last_recent_posts_refresh = Some(std::time::Instant::now());
        tasks::load_recent_posts(self.api.clone(), self.tx.clone());
    }

    pub(super) fn spawn_load_conversations(&mut self) {
        if self.dm_state.conversations_loading {
            return;
        }
        self.dm_state.conversations_loading = true;
        self.dm_state.conversations_error = None;
        tasks::load_conversations(self.api.clone(), self.tx.clone());
    }

    pub(super) fn spawn_load_messages(&mut self, state: &mut ConversationState) {
        if state.messages_loading {
            return;
        }
        state.messages_loading = true;
        state.messages_error = None;
        tasks::load_messages(self.api.clone(), self.tx.clone(), state.peer_id.clone());
    }

    pub(super) fn spawn_send_dm(&mut self, state: &mut ConversationState) {
        let body = state.new_message_body.trim().to_string();
        if body.is_empty() {
            state.send_error = Some("Message cannot be empty".into());
            return;
        }
        state.sending = true;
        state.send_error = None;
        tasks::send_dm(
            self.api.clone(),
            self.tx.clone(),
            state.peer_id.clone(),
            body,
        );
    }

    pub(super) fn spawn_load_blocked_peers(&mut self) {
        if self.blocking_state.blocked_peers_loading {
            return;
        }
        self.blocking_state.blocked_peers_loading = true;
        self.blocking_state.blocked_peers_error = None;
        tasks::load_blocked_peers(self.api.clone(), self.tx.clone());
    }

    pub(super) fn spawn_block_peer(&mut self, state: &mut state::BlockingState) {
        let peer_id = state.new_block_peer_id.trim().to_string();
        if peer_id.is_empty() {
            state.block_error = Some("Peer ID cannot be empty".into());
            return;
        }
        let reason = if state.new_block_reason.trim().is_empty() {
            None
        } else {
            Some(state.new_block_reason.trim().to_string())
        };

        state.blocking_in_progress = true;
        state.block_error = None;
        tasks::block_peer(self.api.clone(), self.tx.clone(), peer_id, reason);
    }

    pub(super) fn spawn_unblock_peer(&mut self, peer_id: String) {
        tasks::unblock_peer(self.api.clone(), self.tx.clone(), peer_id);
    }

    pub(super) fn spawn_load_blocklists(&mut self) {
        if self.blocking_state.blocklists_loading {
            return;
        }
        self.blocking_state.blocklists_loading = true;
        self.blocking_state.blocklists_error = None;
        tasks::load_blocklists(self.api.clone(), self.tx.clone());
    }

    pub(super) fn spawn_subscribe_blocklist(&mut self, state: &mut state::BlockingState) {
        let blocklist_id = state.new_blocklist_id.trim().to_string();
        let maintainer = state.new_blocklist_maintainer.trim().to_string();
        let name = state.new_blocklist_name.trim().to_string();

        if blocklist_id.is_empty() || maintainer.is_empty() || name.is_empty() {
            state.subscribe_error = Some("Blocklist ID, maintainer, and name are required".into());
            return;
        }

        let description = if state.new_blocklist_description.trim().is_empty() {
            None
        } else {
            Some(state.new_blocklist_description.trim().to_string())
        };

        state.subscribing_in_progress = true;
        state.subscribe_error = None;
        tasks::subscribe_blocklist(
            self.api.clone(),
            self.tx.clone(),
            blocklist_id,
            maintainer,
            name,
            description,
            state.new_blocklist_auto_apply,
        );
    }

    pub(super) fn spawn_unsubscribe_blocklist(&mut self, blocklist_id: String) {
        tasks::unsubscribe_blocklist(self.api.clone(), self.tx.clone(), blocklist_id);
    }

    pub(super) fn spawn_load_blocklist_entries(&mut self, blocklist_id: String) {
        self.blocking_state.viewing_blocklist_id = Some(blocklist_id.clone());
        self.blocking_state.blocklist_entries_loading = true;
        self.blocking_state.blocklist_entries_error = None;
        tasks::load_blocklist_entries(self.api.clone(), self.tx.clone(), blocklist_id);
    }

    // IP Blocking spawn methods

    pub(super) fn spawn_load_ip_blocks(&mut self) {
        if self.blocking_state.ip_blocks_loading {
            return;
        }
        self.blocking_state.ip_blocks_loading = true;
        self.blocking_state.ip_blocks_error = None;
        tasks::load_ip_blocks(self.api.clone(), self.tx.clone());
    }

    pub(super) fn spawn_load_ip_block_stats(&mut self) {
        if self.blocking_state.ip_block_stats_loading {
            return;
        }
        self.blocking_state.ip_block_stats_loading = true;
        tasks::load_ip_block_stats(self.api.clone(), self.tx.clone());
    }

    pub(super) fn spawn_add_ip_block(&mut self, state: &mut state::BlockingState) {
        let ip_or_range = state.new_ip_block.trim().to_string();
        if ip_or_range.is_empty() {
            state.add_ip_block_error = Some("IP/Range cannot be empty".into());
            return;
        }
        let reason = if state.new_ip_block_reason.trim().is_empty() {
            None
        } else {
            Some(state.new_ip_block_reason.trim().to_string())
        };

        state.adding_ip_block = true;
        state.add_ip_block_error = None;
        tasks::add_ip_block(self.api.clone(), self.tx.clone(), ip_or_range, reason);
    }

    pub(super) fn spawn_remove_ip_block(&mut self, block_id: i64) {
        tasks::remove_ip_block(self.api.clone(), self.tx.clone(), block_id);
    }

    pub(super) fn spawn_import_ip_blocks(&mut self, state: &mut state::BlockingState) {
        if state.import_text.trim().is_empty() {
            state.import_error = Some("Import text cannot be empty".into());
            return;
        }

        state.importing_ips = true;
        state.import_error = None;
        tasks::import_ip_blocks(self.api.clone(), self.tx.clone(), state.import_text.clone());
    }

    pub(super) fn spawn_export_ip_blocks(&mut self) {
        tasks::export_ip_blocks(self.api.clone(), self.tx.clone());
    }

    pub(super) fn spawn_clear_all_ip_blocks(&mut self) {
        tasks::clear_all_ip_blocks(self.api.clone(), self.tx.clone());
    }

    pub(super) fn spawn_delete_thread(&mut self, thread_id: String) {
        tasks::delete_thread(self.api.clone(), self.tx.clone(), thread_id);
    }

    pub(super) fn spawn_ignore_thread(&mut self, thread_id: String) {
        tasks::ignore_thread(self.api.clone(), self.tx.clone(), thread_id, true);
    }

    pub(super) fn spawn_load_reactions(&mut self, post_id: &str) {
        tasks::load_reactions(self.api.clone(), self.tx.clone(), post_id.to_string());
    }

    pub(super) fn spawn_add_reaction(&mut self, post_id: &str, emoji: &str) {
        tasks::add_reaction(self.api.clone(), self.tx.clone(), post_id.to_string(), emoji.to_string());
    }

    pub(super) fn spawn_remove_reaction(&mut self, post_id: &str, emoji: &str) {
        tasks::remove_reaction(self.api.clone(), self.tx.clone(), post_id.to_string(), emoji.to_string());
    }

    pub(super) fn spawn_search(&self, query: String) {
        let tx = self.tx.clone();
        let api = self.api.clone();

        std::thread::spawn(move || {
            let result = api.search(&query, Some(50));
            let _ = tx.send(super::messages::AppMessage::SearchCompleted { query, result });
        });
    }
}
