pub mod reasoning;
pub mod actions;
pub mod image_gen;
pub mod trajectory;

use anyhow::Result;
use chrono::{Duration as ChronoDuration, Utc};
use flume::Sender;
use std::collections::HashSet;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::time::{sleep, Duration};

use crate::api_client::GraphchanClient;
use crate::config::AgentConfig;
use crate::database::AgentDatabase;

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
    pub processed_posts: HashSet<String>, // Post IDs we've already analyzed
}

impl Default for AgentState {
    fn default() -> Self {
        Self {
            visual_state: AgentVisualState::Idle,
            paused: false,
            posts_this_hour: 0,
            last_post_time: None,
            processed_posts: HashSet::new(),
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
    database: Arc<RwLock<Option<AgentDatabase>>>,
    trajectory_engine: Arc<RwLock<Option<trajectory::TrajectoryEngine>>>,
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

        // Initialize database for memory and persona tracking
        let database = match AgentDatabase::new(&config.database_path) {
            Ok(db) => {
                tracing::info!("📚 Agent memory database initialized: {}", config.database_path);
                Some(db)
            }
            Err(e) => {
                tracing::error!("Failed to initialize agent database: {}", e);
                None
            }
        };

        // Initialize trajectory engine for Ludonarrative Assonantic Tracing
        let trajectory_engine = if config.enable_self_reflection {
            let model = config.reflection_model.clone()
                .unwrap_or_else(|| config.llm_model.clone());
            tracing::info!("🔮 Trajectory engine enabled (model: {})", model);
            Some(trajectory::TrajectoryEngine::new(
                config.llm_api_url.clone(),
                model,
                config.llm_api_key.clone(),
            ))
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
            database: Arc::new(RwLock::new(database)),
            trajectory_engine: Arc::new(RwLock::new(trajectory_engine)),
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

        // Recreate trajectory engine if self-reflection settings changed
        let new_trajectory = if new_config.enable_self_reflection {
            let model = new_config.reflection_model.clone()
                .unwrap_or_else(|| new_config.llm_model.clone());
            Some(trajectory::TrajectoryEngine::new(
                new_config.llm_api_url.clone(),
                model,
                new_config.llm_api_key.clone(),
            ))
        } else {
            None
        };

        // Update all components atomically
        *self.config.write().await = new_config;
        *self.reasoning.write().await = new_reasoning;
        *self.image_gen.write().await = new_image_gen;
        *self.trajectory_engine.write().await = new_trajectory;

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

        // Capture initial persona snapshot if this is the first run
        self.maybe_capture_initial_persona().await;

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

            // Check if it's time for persona evolution (Ludonarrative Assonantic Tracing)
            self.maybe_evolve_persona().await;

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

    /// Capture initial persona snapshot if database is empty
    async fn maybe_capture_initial_persona(&self) {
        let db_lock = self.database.read().await;
        if let Some(ref db) = *db_lock {
            match db.count_persona_snapshots() {
                Ok(0) => {
                    drop(db_lock);
                    self.emit(AgentEvent::Observation("🔮 Capturing initial persona snapshot...".to_string())).await;
                    if let Err(e) = self.capture_persona_snapshot("initial").await {
                        tracing::warn!("Failed to capture initial persona: {}", e);
                    }
                }
                Ok(n) => {
                    tracing::info!("📚 Found {} existing persona snapshots", n);
                }
                Err(e) => {
                    tracing::warn!("Failed to count persona snapshots: {}", e);
                }
            }
        }
    }

    /// Check if it's time to evolve persona and run trajectory inference
    async fn maybe_evolve_persona(&self) {
        let config = self.config.read().await;
        if !config.enable_self_reflection {
            return;
        }

        let reflection_interval_hours = config.reflection_interval_hours;
        drop(config);

        // Check last reflection time
        let db_lock = self.database.read().await;
        let should_reflect = if let Some(ref db) = *db_lock {
            match db.get_last_reflection_time() {
                Ok(Some(last_time)) => {
                    let elapsed = Utc::now() - last_time;
                    elapsed > ChronoDuration::hours(reflection_interval_hours as i64)
                }
                Ok(None) => true, // Never reflected before
                Err(e) => {
                    tracing::warn!("Failed to get last reflection time: {}", e);
                    false
                }
            }
        } else {
            false
        };
        drop(db_lock);

        if should_reflect {
            self.emit(AgentEvent::Observation("🔮 Beginning persona evolution cycle...".to_string())).await;
            self.set_state(AgentVisualState::Thinking).await;

            if let Err(e) = self.run_persona_evolution().await {
                tracing::error!("Persona evolution failed: {}", e);
                self.emit(AgentEvent::Error(format!("Persona evolution error: {}", e))).await;
            }
        }
    }

    /// Run the full persona evolution cycle (Ludonarrative Assonantic Tracing)
    async fn run_persona_evolution(&self) -> Result<()> {
        // 1. Capture current persona snapshot
        self.emit(AgentEvent::Observation("📸 Capturing persona snapshot...".to_string())).await;
        let snapshot = self.capture_persona_snapshot("scheduled_reflection").await?;

        // 2. Get persona history and guiding principles for trajectory inference
        let (history, guiding_principles) = {
            let db_lock = self.database.read().await;
            let config = self.config.read().await;
            let principles = config.guiding_principles.clone();
            drop(config);

            if let Some(ref db) = *db_lock {
                (db.get_persona_history(10)?, principles)
            } else {
                return Err(anyhow::anyhow!("Database not available"));
            }
        };

        // 3. Run trajectory inference
        self.emit(AgentEvent::Observation("🔮 Inferring personality trajectory...".to_string())).await;
        let trajectory_analysis = {
            let engine_lock = self.trajectory_engine.read().await;
            if let Some(ref engine) = *engine_lock {
                engine.infer_trajectory(&history, &guiding_principles).await?
            } else {
                return Err(anyhow::anyhow!("Trajectory engine not available"));
            }
        };

        // 4. Log the trajectory analysis
        tracing::info!("🔮 Trajectory Analysis:");
        tracing::info!("  Narrative: {}", trajectory_analysis.narrative);
        tracing::info!("  Trajectory: {}", trajectory_analysis.trajectory);
        tracing::info!("  Themes: {:?}", trajectory_analysis.themes);
        tracing::info!("  Confidence: {:.2}", trajectory_analysis.confidence);

        self.emit(AgentEvent::Observation(format!(
            "✨ Trajectory inferred: {} (confidence: {:.0}%)",
            &trajectory_analysis.trajectory[..trajectory_analysis.trajectory.len().min(80)],
            trajectory_analysis.confidence * 100.0
        ))).await;

        // 5. Update the snapshot with trajectory and save
        let mut updated_snapshot = snapshot;
        updated_snapshot.inferred_trajectory = Some(trajectory_analysis.trajectory.clone());

        {
            let db_lock = self.database.read().await;
            if let Some(ref db) = *db_lock {
                db.save_persona_snapshot(&updated_snapshot)?;
                db.set_last_reflection_time(Utc::now())?;
            }
        }

        // 6. Emit reasoning trace with trajectory insights
        self.emit(AgentEvent::ReasoningTrace(vec![
            format!("📊 Persona Evolution Complete"),
            format!("Narrative: {}", trajectory_analysis.narrative),
            format!("Direction: {}", trajectory_analysis.trajectory),
            format!("Themes: {}", trajectory_analysis.themes.join(", ")),
            format!("Tensions: {}", if trajectory_analysis.tensions.is_empty() {
                "None identified".to_string()
            } else {
                trajectory_analysis.tensions.join(", ")
            }),
        ])).await;

        self.set_state(AgentVisualState::Happy).await;
        sleep(Duration::from_secs(2)).await;

        Ok(())
    }

    /// Capture a persona snapshot
    async fn capture_persona_snapshot(&self, trigger: &str) -> Result<crate::database::PersonaSnapshot> {
        let config = self.config.read().await;
        let api_url = config.llm_api_url.clone();
        let model = config.reflection_model.clone()
            .unwrap_or_else(|| config.llm_model.clone());
        let api_key = config.llm_api_key.clone();
        let system_prompt = config.system_prompt.clone();
        let guiding_principles = config.guiding_principles.clone();
        drop(config);

        // Get recent important posts as formative experiences
        let experiences: Vec<String> = {
            let db_lock = self.database.read().await;
            if let Some(ref db) = *db_lock {
                db.get_recent_important_posts(5)
                    .unwrap_or_default()
                    .into_iter()
                    .map(|p| p.why_important)
                    .collect()
            } else {
                vec![]
            }
        };

        let snapshot = trajectory::capture_persona_snapshot(
            &api_url,
            &model,
            api_key.as_deref(),
            &system_prompt,
            trigger,
            &experiences,
            &guiding_principles,
        ).await?;

        // Save the snapshot
        {
            let db_lock = self.database.read().await;
            if let Some(ref db) = *db_lock {
                db.save_persona_snapshot(&snapshot)?;
            }
        }

        tracing::info!("📸 Captured persona snapshot: {}", snapshot.self_description);
        Ok(snapshot)
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

        // Get processed posts set and username
        let (processed_posts, username) = {
            let state = self.state.read().await;
            let config = self.config.read().await;
            (state.processed_posts.clone(), config.username.clone())
        };

        // Filter out:
        // 1. Agent's own posts (check metadata.agent.name)
        // 2. Posts we've already processed
        let filtered_posts: Vec<_> = recent_posts.posts.iter()
            .filter(|post_view| {
                // Check if post has agent metadata matching our username
                let is_agent_post = post_view.post.metadata.as_ref()
                    .and_then(|m| m.agent.as_ref())
                    .map(|a| a.name == username)
                    .unwrap_or(false);
                let already_processed = processed_posts.contains(&post_view.post.id);
                !is_agent_post && !already_processed
            })
            .cloned()
            .collect();

        let num_already_processed = recent_posts.posts.iter()
            .filter(|p| processed_posts.contains(&p.post.id))
            .count();

        if filtered_posts.is_empty() {
            if num_already_processed > 0 {
                self.emit(AgentEvent::Observation(
                    format!("No new posts (already processed {} posts)", num_already_processed)
                )).await;
            } else {
                self.emit(AgentEvent::Observation("No new posts to analyze (all are from agent).".to_string())).await;
            }
            return Ok(());
        }

        self.emit(AgentEvent::Observation(
            format!("Found {} new posts to analyze (filtered {} agent posts, {} already processed)",
                    filtered_posts.len(),
                    recent_posts.posts.len() - filtered_posts.len() - num_already_processed,
                    num_already_processed)
        )).await;

        // Reason about posts using LLM
        self.set_state(AgentVisualState::Thinking).await;
        self.emit(AgentEvent::Observation("💭 Asking LLM to analyze posts...".to_string())).await;

        let decision = {
            let reasoning = self.reasoning.read().await;
            reasoning.analyze_posts(&filtered_posts).await?
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
                let thread_id = filtered_posts.iter()
                    .find(|p| p.post.id == post_id)
                    .map(|p| p.post.thread_id.clone())
                    .ok_or_else(|| anyhow::anyhow!("Could not find thread for post"))?;

                // Get username from config and add to metadata
                let username = {
                    let config = self.config.read().await;
                    config.username.clone()
                };

                // Create metadata with agent info
                let metadata = Some(crate::models::PostMetadata {
                    agent: Some(crate::models::AgentInfo {
                        name: username,
                        version: None,
                    }),
                    client: Some("graphchan_agent".to_string()),
                });

                // Create the post
                let input = crate::models::CreatePostInput {
                    thread_id: thread_id.clone(),
                    author_peer_id: None, // Use default from backend
                    body: content,
                    parent_post_ids: vec![post_id.clone()],
                    metadata,
                };

                match self.client.create_post(input).await {
                    Ok(posted) => {
                        self.emit(AgentEvent::ActionTaken {
                            action: "Posted reply".to_string(),
                            result: format!("Post ID: {}", posted.id),
                        }).await;

                        // Mark this post as processed and update stats
                        let mut state = self.state.write().await;
                        state.posts_this_hour += 1;
                        state.last_post_time = Some(chrono::Utc::now());
                        state.processed_posts.insert(post_id.clone());
                        drop(state);

                        self.set_state(AgentVisualState::Happy).await;
                        sleep(Duration::from_secs(2)).await; // Enjoy the moment!
                    }
                    Err(e) => {
                        self.emit(AgentEvent::Error(format!("Failed to post: {}", e))).await;
                        self.set_state(AgentVisualState::Confused).await;
                        // Still mark as processed to avoid retrying repeatedly
                        let mut state = self.state.write().await;
                        state.processed_posts.insert(post_id.clone());
                    }
                }
            }
            reasoning::Decision::NoAction { reasoning } => {
                self.emit(AgentEvent::ReasoningTrace(reasoning)).await;
                self.emit(AgentEvent::Observation("No action needed at this time.".to_string())).await;

                // Mark all analyzed posts as processed so we don't re-analyze them
                let mut state = self.state.write().await;
                for post_view in &filtered_posts {
                    state.processed_posts.insert(post_view.post.id.clone());
                }
                let num_marked = filtered_posts.len();
                drop(state);

                tracing::debug!("Marked {} posts as processed (no action needed)", num_marked);
            }
        }

        self.set_state(AgentVisualState::Idle).await;

        Ok(())
    }
}
