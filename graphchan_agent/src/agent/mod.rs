pub mod reasoning;
pub mod actions;
pub mod image_gen;

use anyhow::Result;
use flume::Sender;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::time::{sleep, Duration};

use crate::api_client::GraphchanClient;
use crate::config::AgentConfig;

#[derive(Debug, Clone)]
pub enum AgentVisualState {
    Idle,
    Reading,
    Thinking,
    Writing,
    Happy,
    Confused,
    Paused,
}

#[derive(Debug, Clone)]
pub enum AgentEvent {
    StateChanged(AgentVisualState),
    Observation(String),
    ReasoningTrace(Vec<String>),
    ActionTaken { action: String, result: String },
    Error(String),
}

pub struct AgentState {
    pub visual_state: AgentVisualState,
    pub paused: bool,
    pub posts_this_hour: u32,
    pub last_post_time: Option<chrono::DateTime<chrono::Utc>>,
}

impl Default for AgentState {
    fn default() -> Self {
        Self {
            visual_state: AgentVisualState::Idle,
            paused: false,
            posts_this_hour: 0,
            last_post_time: None,
        }
    }
}

pub struct Agent {
    client: GraphchanClient,
    config: Arc<RwLock<AgentConfig>>,
    state: Arc<RwLock<AgentState>>,
    event_tx: Sender<AgentEvent>,
    reasoning: Arc<RwLock<reasoning::ReasoningEngine>>,
    image_gen: Arc<RwLock<Option<image_gen::ImageGenerator>>>,
}

impl Agent {
    pub fn new(
        client: GraphchanClient,
        config: AgentConfig,
        event_tx: Sender<AgentEvent>,
    ) -> Self {
        let reasoning = reasoning::ReasoningEngine::new(
            config.llm_api_url.clone(),
            config.llm_model.clone(),
            config.llm_api_key.clone(),
            config.system_prompt.clone(),
        );

        // Initialize image generator if workflow is configured
        let image_gen = if config.enable_image_generation {
            if let Some(ref workflow_json) = config.workflow_settings {
                match serde_json::from_str::<crate::comfy_workflow::ComfyWorkflow>(workflow_json) {
                    Ok(workflow) => {
                        tracing::info!("Image generation enabled with workflow: {}", workflow.name);
                        Some(image_gen::ImageGenerator::new(
                            config.comfyui.api_url.clone(),
                            Some(workflow),
                        ))
                    }
                    Err(e) => {
                        tracing::error!("Failed to load workflow: {}", e);
                        None
                    }
                }
            } else {
                tracing::warn!("Image generation enabled but no workflow configured");
                None
            }
        } else {
            None
        };

        Self {
            client,
            config: Arc::new(RwLock::new(config)),
            state: Arc::new(RwLock::new(AgentState::default())),
            event_tx,
            reasoning: Arc::new(RwLock::new(reasoning)),
            image_gen: Arc::new(RwLock::new(image_gen)),
        }
    }

    /// Reload config and recreate reasoning engine and image generator
    pub async fn reload_config(&self, new_config: AgentConfig) {
        tracing::info!("Reloading agent configuration...");

        // Create new reasoning engine with updated config
        let new_reasoning = reasoning::ReasoningEngine::new(
            new_config.llm_api_url.clone(),
            new_config.llm_model.clone(),
            new_config.llm_api_key.clone(),
            new_config.system_prompt.clone(),
        );

        // Recreate image generator if needed
        let new_image_gen = if new_config.enable_image_generation {
            if let Some(ref workflow_json) = new_config.workflow_settings {
                match serde_json::from_str::<crate::comfy_workflow::ComfyWorkflow>(workflow_json) {
                    Ok(workflow) => {
                        tracing::info!("Image generation enabled with workflow: {}", workflow.name);
                        Some(image_gen::ImageGenerator::new(
                            new_config.comfyui.api_url.clone(),
                            Some(workflow),
                        ))
                    }
                    Err(e) => {
                        tracing::error!("Failed to load workflow: {}", e);
                        None
                    }
                }
            } else {
                tracing::warn!("Image generation enabled but no workflow configured");
                None
            }
        } else {
            None
        };

        // Update all components atomically
        *self.config.write().await = new_config;
        *self.reasoning.write().await = new_reasoning;
        *self.image_gen.write().await = new_image_gen;

        self.emit(AgentEvent::Observation("✓ Configuration reloaded".to_string())).await;
        tracing::info!("Configuration reloaded successfully");
    }

    pub async fn toggle_pause(&self) {
        let mut state = self.state.write().await;
        state.paused = !state.paused;
        let new_state = if state.paused {
            AgentVisualState::Paused
        } else {
            AgentVisualState::Idle
        };
        state.visual_state = new_state.clone();
        drop(state);
        
        let _ = self.event_tx.send(AgentEvent::StateChanged(new_state));
    }

    async fn emit(&self, event: AgentEvent) {
        let _ = self.event_tx.send(event);
    }

    async fn set_state(&self, visual_state: AgentVisualState) {
        let mut state = self.state.write().await;
        state.visual_state = visual_state.clone();
        drop(state);
        
        self.emit(AgentEvent::StateChanged(visual_state)).await;
    }

    pub async fn run_loop(self: Arc<Self>) -> Result<()> {
        tracing::info!("Agent loop starting...");

        // Check API connection
        self.emit(AgentEvent::Observation("Connecting to Graphchan API...".to_string())).await;
        if let Err(e) = self.client.health_check().await {
            self.emit(AgentEvent::Error(format!("Cannot connect to Graphchan API: {}", e))).await;
            return Err(e);
        }
        self.emit(AgentEvent::Observation("✓ Connected to Graphchan".to_string())).await;

        loop {
            // Check if paused
            {
                let state = self.state.read().await;
                if state.paused {
                    sleep(Duration::from_secs(5)).await;
                    continue;
                }
            }

            self.set_state(AgentVisualState::Idle).await;

            // Get poll interval from config
            let poll_interval = {
                let config = self.config.read().await;
                config.poll_interval_secs
            };
            sleep(Duration::from_secs(poll_interval)).await;

            // Check for rate limiting
            {
                let state = self.state.read().await;
                let config = self.config.read().await;
                if state.posts_this_hour >= config.max_posts_per_hour {
                    self.emit(AgentEvent::Observation(
                        format!("⏸ Rate limit reached ({}/{}), waiting...",
                                state.posts_this_hour,
                                config.max_posts_per_hour)
                    )).await;
                    continue;
                }
            }

            // Main agent logic
            if let Err(e) = self.run_cycle().await {
                tracing::error!("Agent cycle error: {}", e);
                self.emit(AgentEvent::Error(e.to_string())).await;
                self.set_state(AgentVisualState::Confused).await;
                sleep(Duration::from_secs(10)).await;
            }
        }
    }

    async fn run_cycle(&self) -> Result<()> {
        // Read recent posts
        self.set_state(AgentVisualState::Reading).await;
        self.emit(AgentEvent::Observation("📖 Checking for recent posts...".to_string())).await;

        let recent_posts = self.client.get_recent_posts(20).await?;

        if recent_posts.posts.is_empty() {
            self.emit(AgentEvent::Observation("No recent posts found.".to_string())).await;
            return Ok(());
        }

        self.emit(AgentEvent::Observation(
            format!("Found {} recent posts", recent_posts.posts.len())
        )).await;

        // Reason about posts using LLM
        self.set_state(AgentVisualState::Thinking).await;
        self.emit(AgentEvent::Observation("💭 Asking LLM to analyze posts...".to_string())).await;

        let decision = {
            let reasoning = self.reasoning.read().await;
            reasoning.analyze_posts(&recent_posts.posts).await?
        };

        match decision {
            reasoning::Decision::Reply { post_id, content, reasoning } => {
                // Show reasoning trace
                self.emit(AgentEvent::ReasoningTrace(reasoning)).await;

                // Post the reply
                self.set_state(AgentVisualState::Writing).await;
                self.emit(AgentEvent::Observation(
                    format!("✍️ Writing reply to post {}...", &post_id[..8])
                )).await;

                // Find the thread_id for this post
                let thread_id = recent_posts.posts.iter()
                    .find(|p| p.post.id == post_id)
                    .map(|p| p.post.thread_id.clone())
                    .ok_or_else(|| anyhow::anyhow!("Could not find thread for post"))?;

                // Create the post
                let input = crate::models::CreatePostInput {
                    thread_id: thread_id.clone(),
                    author_peer_id: None, // Use default from backend
                    body: content.clone(),
                    parent_post_ids: vec![post_id.clone()],
                };

                match self.client.create_post(input).await {
                    Ok(posted) => {
                        self.emit(AgentEvent::ActionTaken {
                            action: "Posted reply".to_string(),
                            result: format!("Post ID: {}", posted.id),
                        }).await;

                        // Update post count
                        let mut state = self.state.write().await;
                        state.posts_this_hour += 1;
                        state.last_post_time = Some(chrono::Utc::now());
                        drop(state);

                        self.set_state(AgentVisualState::Happy).await;
                        sleep(Duration::from_secs(2)).await; // Enjoy the moment!
                    }
                    Err(e) => {
                        self.emit(AgentEvent::Error(format!("Failed to post: {}", e))).await;
                        self.set_state(AgentVisualState::Confused).await;
                    }
                }
            }
            reasoning::Decision::NoAction { reasoning } => {
                self.emit(AgentEvent::ReasoningTrace(reasoning)).await;
                self.emit(AgentEvent::Observation("No action needed at this time.".to_string())).await;
            }
        }

        self.set_state(AgentVisualState::Idle).await;

        Ok(())
    }
}
