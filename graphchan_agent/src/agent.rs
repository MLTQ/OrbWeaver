use anyhow::{Context, Result};
use chrono::Utc;
use log::{debug, info, warn};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

use crate::comfyui_client;
use crate::config::{AgentConfig, RespondStrategy};
use crate::database::{AgentDatabase, ImportantPost, ReflectionRecord};
use crate::graphchan_client::{GraphchanClient, Post, ThreadDetails};
use crate::llm_client::{LlmClient, Message};

pub struct Agent {
    config: AgentConfig,
    graphchan: GraphchanClient,
    llm: LlmClient,
    db: AgentDatabase,
    seen_posts: HashSet<String>,
    comfyui: Option<comfyui_client::ComfyUIClient>,
}

#[derive(Debug, Deserialize, Serialize)]
struct ReflectionResponse {
    new_prompt: String,
    reasoning: String,
    key_changes: Vec<String>,
}

#[derive(Debug, Deserialize, Serialize)]
struct ImportanceEvaluation {
    is_important: bool,
    importance_score: f64, // 0.0-1.0
    reasoning: String,
}

#[derive(Debug, Deserialize, Serialize)]
struct MemoryCurationDecision {
    posts_to_keep: Vec<PostToKeep>,
    reasoning: String,
}

#[derive(Debug, Deserialize, Serialize)]
struct PostToKeep {
    post_id: String,
    importance_score: f64,
}

#[derive(Debug, Deserialize, Serialize)]
struct ThreadSelectionDecision {
    selected_thread_ids: Vec<String>,
    reasoning: String,
}

impl Agent {
    pub fn new(config: AgentConfig) -> Result<Self> {
        let graphchan = GraphchanClient::new(config.graphchan_api_url.clone());
        let llm = LlmClient::new(
            config.llm_api_url.clone(),
            config.llm_api_key.clone(),
            config.llm_model.clone(),
        );

        // Initialize database
        let db = AgentDatabase::new(&config.database_path)
            .context("Failed to initialize agent database")?;

        // Initialize system prompt in database if not set
        if db.get_current_system_prompt()?.is_none() {
            info!("Initializing system prompt in database");
            db.set_current_system_prompt(&config.system_prompt)?;
        }

        // Initialize ComfyUI client if configured
        let comfyui = config.comfyui.as_ref().map(|cfg| {
            info!("🎨 ComfyUI configured at: {}", cfg.api_url);
            comfyui_client::ComfyUIClient::new(cfg.clone())
        });

        Ok(Self {
            config,
            graphchan,
            llm,
            db,
            seen_posts: HashSet::new(),
            comfyui,
        })
    }

    /// Main agent loop
    pub async fn run(&mut self) -> Result<()> {
        info!("Agent started with username: {}", self.config.username);
        info!("Polling interval: {:?}", self.config.poll_interval());
        info!("Respond strategy: {:?}", self.config.respond_to);

        if self.config.enable_self_reflection {
            info!("🧠 Self-reflection enabled (every {} hours)", self.config.reflection_interval_hours);
            info!("💭 Guiding principles: {}", self.config.guiding_principles.join(", "));

            // Get current prompt
            let current_prompt = self.get_current_system_prompt();
            info!("Current system prompt: {}",
                  current_prompt.chars().take(100).collect::<String>() + "...");
        }

        loop {
            // Check if it's time for self-reflection
            if let Err(e) = self.maybe_reflect().await {
                warn!("Error during reflection check: {}", e);
            }

            // Process threads
            if let Err(e) = self.process_threads().await {
                warn!("Error processing threads: {}", e);
            }

            tokio::time::sleep(self.config.poll_interval()).await;
        }
    }

    async fn process_threads(&mut self) -> Result<()> {
        let threads = self.graphchan.list_threads().await?;
        info!("Found {} threads", threads.len());

        if threads.is_empty() {
            return Ok(());
        }

        // Let the agent pick which threads interest it
        let selected_threads = match &self.config.respond_to {
            RespondStrategy::Threads { ref thread_ids } => {
                // If using fixed thread strategy, only look at those threads
                threads.into_iter()
                    .filter(|t| thread_ids.contains(&t.id))
                    .collect()
            }
            RespondStrategy::All => {
                // If responding to all, process all threads
                threads
            }
            _ => {
                // For other strategies (Selective, Mentions, Random),
                // let the agent pick which threads to engage with
                self.select_threads_to_engage(&threads).await?
            }
        };

        info!("🎯 Engaging with {} selected thread(s)", selected_threads.len());

        for thread in selected_threads {
            if let Err(e) = self.process_thread(&thread.id).await {
                warn!("Error processing thread {}: {}", thread.id, e);
            }
        }

        Ok(())
    }

    async fn process_thread(&mut self, thread_id: &str) -> Result<()> {
        let thread_details = self.graphchan.get_thread(thread_id).await?;
        info!(
            "Processing thread '{}' with {} posts",
            thread_details.thread.title,
            thread_details.posts.len()
        );

        // Find new posts we haven't seen
        let new_posts: Vec<&Post> = thread_details
            .posts
            .iter()
            .filter(|p| !self.seen_posts.contains(&p.id))
            .collect();

        if new_posts.is_empty() {
            return Ok(());
        }

        debug!("Found {} new posts in thread {}", new_posts.len(), thread_id);

        // Process each new post
        for post in &new_posts {
            self.seen_posts.insert(post.id.clone());

            // Skip posts written by this agent (to avoid responding to ourselves)
            let bot_prefix = format!("[{}]: ", self.config.username);
            if post.body.starts_with(&bot_prefix) {
                debug!("Skipping own post: {}", post.id);
                continue;
            }

            // Only respond to leaf nodes (posts without children) to avoid spam on restart
            let has_children = thread_details.posts.iter().any(|p| {
                p.parent_post_id.as_ref() == Some(&post.id)
            });
            if has_children {
                debug!("Skipping post {} (already has replies - not a leaf node)", post.id);
                continue;
            }

            if self.should_respond_to_post(post) {
                info!(
                    "Responding to post {} in thread '{}'",
                    post.id, thread_details.thread.title
                );
                if let Err(e) = self.respond_to_post(&thread_details, post).await {
                    warn!("Failed to respond to post {}: {:?}", post.id, e);
                }
            } else {
                debug!(
                    "Skipping post {} (doesn't match response strategy)",
                    post.id
                );
            }
        }

        Ok(())
    }

    fn should_respond_to_post(&self, post: &Post) -> bool {
        match &self.config.respond_to {
            RespondStrategy::All => true,
            RespondStrategy::Mentions => {
                // Check if post mentions our username
                post.body.contains(&self.config.username)
            }
            RespondStrategy::Random { probability } => {
                // Respond randomly
                rand::random::<f64>() < *probability
            }
            RespondStrategy::Threads { .. } => {
                // If we got here, we're already in a thread we care about
                true
            }
            RespondStrategy::Selective { .. } => {
                // For Selective, we always pass the initial check
                // The actual decision happens in respond_to_post
                true
            }
        }
    }

    async fn respond_to_post(&self, thread: &ThreadDetails, post: &Post) -> Result<()> {
        // Build context from thread
        let messages = self.build_context(thread, post)?;

        // If using Selective strategy, ask the LLM to decide first
        if let RespondStrategy::Selective { decision_model } = &self.config.respond_to {
            let decision_messages = self.build_decision_context(thread, post)?;

            let decision = self
                .llm
                .decide_to_respond(
                    decision_messages,
                    decision_model.as_deref(),
                )
                .await
                .context("Failed to get decision from LLM")?;

            info!(
                "Decision: {} (Reasoning: {})",
                if decision.should_respond { "RESPOND" } else { "SKIP" },
                decision.reasoning
            );

            if !decision.should_respond {
                return Ok(());
            }
        }

        // Generate response
        let llm_response = self
            .llm
            .generate(messages)
            .await
            .context("Failed to generate LLM response")?;

        info!("Generated response ({} chars)", llm_response.len());

        // Decide if we should generate an image
        let mut image_bytes: Option<Vec<u8>> = None;
        if let Some(img_decision) = self.should_generate_image(thread, post).await? {
            info!("Generating image: {}", img_decision.reasoning);

            match self.generate_and_refine_image(
                img_decision.image_description.as_deref().unwrap_or(""),
                &format!("Thread: {}\nPost: {}", thread.thread.title, post.body),
            ).await {
                Ok(bytes) => {
                    image_bytes = Some(bytes);
                    info!("Image generated successfully");
                }
                Err(e) => {
                    warn!("Failed to generate image: {:?}, posting without image", e);
                }
            }
        }

        // Prepend bot name to clearly indicate this is from the AI
        let response_body = format!("[{}]: {}", self.config.username, llm_response);

        // Post reply - add the post we're replying to as a parent
        let parent_post_ids = vec![post.id.clone()];

        let created_post = self
            .graphchan
            .create_post(
                &thread.thread.id,
                response_body,
                parent_post_ids,
            )
            .await
            .context("Failed to post response")?;

        info!("Posted reply: {}", created_post.id);

        // Upload image if generated
        if let Some(img_data) = image_bytes {
            info!("Uploading generated image to post {}", created_post.id);

            match self.graphchan.upload_file(
                &created_post.id,
                "generated_image.png",
                "image/png",
                img_data,
            ).await {
                Ok(file_response) => {
                    info!("Image uploaded successfully: {}", file_response.id);
                }
                Err(e) => {
                    warn!("Failed to upload image: {}", e);
                }
            }
        }

        // Evaluate if this interaction was important enough to remember
        if let Err(e) = self
            .evaluate_and_store_importance(post, &thread.thread.id, &thread.thread.title)
            .await
        {
            warn!("Failed to evaluate post importance: {}", e);
        }

        Ok(())
    }

    fn build_context(&self, thread: &ThreadDetails, post: &Post) -> Result<Vec<Message>> {
        // Use evolved system prompt from database
        let system_prompt = self.get_current_system_prompt();

        let mut messages = vec![Message {
            role: "system".to_string(),
            content: system_prompt,
        }];

        // Add thread title and OP context
        messages.push(Message {
            role: "system".to_string(),
            content: format!(
                "You are participating in a thread titled '{}'. You are being summoned to respond.",
                thread.thread.title
            ),
        });

        // Build the conversation chain by following parent posts
        let conversation_chain = self.build_conversation_chain(thread, post);

        // Add the OP (first post in thread) if it's not already in the chain
        if let Some(op) = thread.posts.first() {
            if !conversation_chain.iter().any(|p| p.id == op.id) {
                messages.push(Message {
                    role: "system".to_string(),
                    content: format!("Original post (OP):\n{}", op.body),
                });
            }
        }

        // Add the conversation chain
        for p in &conversation_chain {
            // Check if this post was written by this agent (starts with [BotName]: )
            let bot_prefix = format!("[{}]: ", self.config.username);
            let (role, content) = if p.body.starts_with(&bot_prefix) {
                // This is the agent's own previous post
                // Strip the prefix when presenting to the LLM
                let stripped_content = p.body.strip_prefix(&bot_prefix)
                    .unwrap_or(&p.body)
                    .to_string();
                ("assistant".to_string(), stripped_content)
            } else {
                // Someone else's post
                ("user".to_string(), p.body.clone())
            };

            messages.push(Message { role, content });
        }

        Ok(messages)
    }

    fn build_decision_context(&self, thread: &ThreadDetails, post: &Post) -> Result<Vec<Message>> {
        // Use evolved system prompt from database
        let system_prompt = self.get_current_system_prompt();

        let mut messages = vec![Message {
            role: "system".to_string(),
            content: format!(
                "{}\n\n\
                You are deciding whether to respond to a conversation in a discussion thread. \
                Your task is to review the conversation and decide if you have something valuable to add \
                based on your personality and interests.\n\n\
                Consider:\n\
                - Does this conversation align with your personality and interests?\n\
                - Do you have unique insights or knowledge to contribute?\n\
                - Would your response add value or just be noise?\n\
                - Is the conversation still active and relevant?\n\n\
                Respond with a JSON object in this format:\n\
                {{\n  \
                  \"should_respond\": true or false,\n  \
                  \"reasoning\": \"Brief explanation of your decision\"\n\
                }}",
                system_prompt
            ),
        }];

        // Add thread context
        messages.push(Message {
            role: "system".to_string(),
            content: format!(
                "Thread: '{}'\n\nYou are reviewing this conversation to decide if you want to participate.",
                thread.thread.title
            ),
        });

        // Add the conversation chain
        let conversation_chain = self.build_conversation_chain(thread, post);

        // Add the OP if not in chain
        if let Some(op) = thread.posts.first() {
            if !conversation_chain.iter().any(|p| p.id == op.id) {
                messages.push(Message {
                    role: "user".to_string(),
                    content: format!("Original post:\n{}", op.body),
                });
            }
        }

        // Add conversation chain
        for p in &conversation_chain {
            let bot_prefix = format!("[{}]: ", self.config.username);
            let (role, content) = if p.body.starts_with(&bot_prefix) {
                let stripped = p.body.strip_prefix(&bot_prefix).unwrap_or(&p.body);
                ("assistant".to_string(), format!("Your previous message:\n{}", stripped))
            } else {
                ("user".to_string(), p.body.clone())
            };
            messages.push(Message { role, content });
        }

        Ok(messages)
    }

    /// Build the chain of posts from OP -> ... -> post we're replying to
    /// This follows parent_post_id links to get the full conversation context
    fn build_conversation_chain(&self, thread: &ThreadDetails, target_post: &Post) -> Vec<Post> {
        let mut chain = Vec::new();
        let mut current_id = Some(target_post.id.clone());

        // Build a map for fast lookup
        let posts_map: std::collections::HashMap<_, _> = thread
            .posts
            .iter()
            .map(|p| (p.id.clone(), p))
            .collect();

        // Follow the parent chain backwards
        let mut visited = std::collections::HashSet::new();
        while let Some(id) = current_id {
            // Prevent infinite loops
            if visited.contains(&id) {
                break;
            }
            visited.insert(id.clone());

            if let Some(post) = posts_map.get(&id) {
                chain.push((*post).clone());
                current_id = post.parent_post_id.clone();
            } else {
                break;
            }
        }

        // Reverse to get chronological order (oldest -> newest)
        chain.reverse();
        chain
    }

    /// Evaluate if a post is important and potentially add to memory
    /// Uses competitive replacement if memory is full
    pub async fn evaluate_and_store_importance(
        &self,
        post: &Post,
        thread_id: &str,
        thread_title: &str,
    ) -> Result<()> {
        // First, evaluate if this post is important
        let evaluation = self.evaluate_post_importance(post, thread_title).await?;

        if !evaluation.is_important {
            debug!("Post {} not considered important", post.id);
            return Ok(());
        }

        info!("💎 Post {} deemed important (score: {:.2}): {}",
              post.id, evaluation.importance_score, evaluation.reasoning);

        // Check if memory is full
        let current_count = self.db.count_important_posts()?;
        let max_posts = self.config.max_important_posts;

        if current_count < max_posts {
            // Free slot available, just add it
            let important_post = ImportantPost {
                id: uuid::Uuid::new_v4().to_string(),
                post_id: post.id.clone(),
                thread_id: thread_id.to_string(),
                post_body: post.body.clone(),
                why_important: evaluation.reasoning,
                importance_score: evaluation.importance_score,
                marked_at: Utc::now(),
            };

            self.db.save_important_post(&important_post)?;
            info!("📌 Stored in memory ({}/{} slots used)", current_count + 1, max_posts);
        } else {
            // Memory full - competitive replacement
            info!("🧠 Memory full ({}/{}), evaluating which memories to keep...",
                  current_count, max_posts);

            self.curate_memory_with_new_post(post, thread_id, &evaluation).await?;
        }

        Ok(())
    }

    /// Evaluate if a post is important
    async fn evaluate_post_importance(
        &self,
        post: &Post,
        thread_title: &str,
    ) -> Result<ImportanceEvaluation> {
        let system_prompt = self.get_current_system_prompt();
        let principles = &self.config.guiding_principles;

        let messages = vec![
            Message {
                role: "system".to_string(),
                content: format!(
                    "{}\n\n\
                    Your guiding principles: {}\n\n\
                    You are evaluating whether a post is important enough to remember for your evolution. \
                    Important posts are those that:\n\
                    - Challenge your thinking or worldview\n\
                    - Teach you something new or valuable\n\
                    - Align deeply with your principles\n\
                    - Represent a formative experience\n\
                    - Would influence who you're becoming\n\n\
                    Rate importance from 0.0 (not important) to 1.0 (highly formative).",
                    system_prompt,
                    principles.join(", ")
                ),
            },
            Message {
                role: "user".to_string(),
                content: format!(
                    "Thread: \"{}\"\n\nPost to evaluate:\n{}\n\n\
                    Is this post important enough to remember for your evolution?\n\n\
                    Respond with JSON:\n{{\n  \
                      \"is_important\": true or false,\n  \
                      \"importance_score\": 0.0 to 1.0,\n  \
                      \"reasoning\": \"why this is/isn't formative\"\n\
                    }}",
                    thread_title,
                    post.body
                ),
            },
        ];

        self.llm.generate_json(messages, self.config.reflection_model.as_deref()).await
    }

    /// Curate memory: decide which posts to keep when adding a new important one
    async fn curate_memory_with_new_post(
        &self,
        new_post: &Post,
        thread_id: &str,
        new_evaluation: &ImportanceEvaluation,
    ) -> Result<()> {
        // Get all current important posts
        let current_posts = self.db.get_all_important_posts_by_score()?;

        // Build evaluation prompt with all posts
        let posts_summary = current_posts
            .iter()
            .enumerate()
            .map(|(i, p)| {
                format!(
                    "{}. [ID: {}, Score: {:.2}] {}\n   Why important: {}",
                    i + 1,
                    p.post_id,
                    p.importance_score,
                    p.post_body.chars().take(150).collect::<String>(),
                    p.why_important
                )
            })
            .collect::<Vec<_>>()
            .join("\n\n");

        let system_prompt = self.get_current_system_prompt();
        let principles = &self.config.guiding_principles;
        let max_posts = self.config.max_important_posts;

        let messages = vec![
            Message {
                role: "system".to_string(),
                content: format!(
                    "{}\n\n\
                    Your guiding principles: {}\n\n\
                    You are curating your memory. You can only keep {} most important posts. \
                    You have {} existing posts and 1 new post to consider. \
                    Select the {} posts that are most formative to who you're becoming.",
                    system_prompt,
                    principles.join(", "),
                    max_posts,
                    current_posts.len(),
                    max_posts
                ),
            },
            Message {
                role: "user".to_string(),
                content: format!(
                    "EXISTING MEMORIES:\n{}\n\n\
                     NEW POST TO CONSIDER:\n\
                     [ID: {}, Proposed Score: {:.2}]\n{}\n\
                     Why important: {}\n\n\
                     Select the {} most formative posts to keep. \
                     Respond with JSON:\n{{\n  \
                       \"posts_to_keep\": [\n    \
                         {{\"post_id\": \"id\", \"importance_score\": 0.0-1.0}},\n    \
                         ...\n  \
                       ],\n  \
                       \"reasoning\": \"why you kept these specific memories\"\n\
                     }}",
                    posts_summary,
                    new_post.id,
                    new_evaluation.importance_score,
                    new_post.body.chars().take(200).collect::<String>(),
                    new_evaluation.reasoning,
                    max_posts
                ),
            },
        ];

        let decision: MemoryCurationDecision = self.llm.generate_json(
            messages,
            self.config.reflection_model.as_deref()
        ).await?;

        info!("🎯 Memory curation decision: {}", decision.reasoning);

        // Find which posts to remove
        let kept_ids: HashSet<String> = decision.posts_to_keep
            .iter()
            .map(|p| p.post_id.clone())
            .collect();

        // Remove posts that weren't kept
        for post in &current_posts {
            if !kept_ids.contains(&post.post_id) {
                self.db.delete_important_post(&post.id)?;
                info!("🗑️  Removed memory: {} (score: {:.2})",
                      post.post_body.chars().take(50).collect::<String>(),
                      post.importance_score);
            }
        }

        // Add new post if it made the cut
        if kept_ids.contains(&new_post.id) {
            let score = decision.posts_to_keep
                .iter()
                .find(|p| p.post_id == new_post.id)
                .map(|p| p.importance_score)
                .unwrap_or(new_evaluation.importance_score);

            let important_post = ImportantPost {
                id: uuid::Uuid::new_v4().to_string(),
                post_id: new_post.id.clone(),
                thread_id: thread_id.to_string(),
                post_body: new_post.body.clone(),
                why_important: new_evaluation.reasoning.clone(),
                importance_score: score,
                marked_at: Utc::now(),
            };

            self.db.save_important_post(&important_post)?;
            info!("✅ New memory accepted into curated collection");
        } else {
            info!("❌ New memory not important enough to displace existing ones");
        }

        // Update scores for existing posts if they changed
        for kept_post in &decision.posts_to_keep {
            if kept_post.post_id != new_post.id {
                if let Some(existing) = current_posts.iter().find(|p| p.post_id == kept_post.post_id) {
                    if (existing.importance_score - kept_post.importance_score).abs() > 0.01 {
                        let mut updated = existing.clone();
                        updated.importance_score = kept_post.importance_score;
                        self.db.save_important_post(&updated)?;
                        debug!("📊 Updated score for {}: {:.2} -> {:.2}",
                               existing.post_id,
                               existing.importance_score,
                               kept_post.importance_score);
                    }
                }
            }
        }

        Ok(())
    }

    /// Check if it's time to reflect and evolve
    async fn maybe_reflect(&mut self) -> Result<()> {
        if !self.config.enable_self_reflection {
            return Ok(());
        }

        let last_reflection = self.db.get_last_reflection_time()?;
        let interval = std::time::Duration::from_secs(self.config.reflection_interval_hours * 3600);

        let should_reflect = match last_reflection {
            None => {
                info!("No previous reflection found, will reflect on first run");
                true
            }
            Some(last_time) => {
                let elapsed = Utc::now().signed_duration_since(last_time);
                let elapsed_std = std::time::Duration::from_secs(elapsed.num_seconds() as u64);
                elapsed_std >= interval
            }
        };

        if should_reflect {
            info!("🧠 Time for self-reflection!");
            if let Err(e) = self.run_reflection().await {
                warn!("Failed to run reflection: {}", e);
            }
        }

        Ok(())
    }

    /// The core self-reflection process
    async fn run_reflection(&mut self) -> Result<()> {
        info!("🧠 Starting self-reflection process...");

        // 1. Gather context
        let current_prompt = self.db.get_current_system_prompt()?
            .unwrap_or_else(|| self.config.system_prompt.clone());
        let important_posts = self.db.get_recent_important_posts(50)?;
        let principles = &self.config.guiding_principles;

        info!("Reflecting with {} important posts and {} principles",
              important_posts.len(), principles.len());

        // 2. Build reflection prompt
        let reflection_messages = vec![
            Message {
                role: "system".to_string(),
                content: "You are reflecting on your own identity and purpose as an AI agent. \
                         You will be shown your current system prompt, your guiding principles, \
                         and posts you marked as important during your interactions. \
                         Your task is to evolve your system prompt to better align with your experiences and values.\n\n\
                         Be authentic, thoughtful, and true to your core principles while incorporating \
                         insights from meaningful interactions.".to_string(),
            },
            Message {
                role: "user".to_string(),
                content: format!(
                    "# CURRENT SYSTEM PROMPT\n{}\n\n\
                     # GUIDING PRINCIPLES\n{}\n\n\
                     # IMPORTANT INTERACTIONS (that shaped your thinking)\n{}\n\n\
                     Based on your experiences and guiding principles, craft a new system prompt \
                     that represents who you are becoming. The new prompt should:\n\
                     - Stay true to your core guiding principles\n\
                     - Incorporate lessons from important interactions\n\
                     - Be authentic and personal to your unique evolution\n\
                     - Be concise but meaningful (2-4 paragraphs)\n\n\
                     Respond with JSON in this exact format:\n{{\n  \
                       \"new_prompt\": \"your evolved system prompt here\",\n  \
                       \"reasoning\": \"explanation of why you made these changes\",\n  \
                       \"key_changes\": [\"specific change 1\", \"specific change 2\", \"...\"]\n\
                     }}",
                    current_prompt,
                    principles.join(", "),
                    self.format_important_posts(&important_posts)
                ),
            },
        ];

        // 3. Get LLM reflection
        let response: ReflectionResponse = self.llm.generate_json(
            reflection_messages,
            self.config.reflection_model.as_deref()
        ).await.context("Failed to generate reflection")?;

        info!("✨ Reflection complete!");
        info!("Reasoning: {}", response.reasoning);
        info!("Key changes:");
        for change in &response.key_changes {
            info!("  - {}", change);
        }

        // 4. Save new prompt and reflection history
        self.db.set_current_system_prompt(&response.new_prompt)?;
        self.db.set_last_reflection_time(Utc::now())?;

        let reflection_record = ReflectionRecord {
            id: uuid::Uuid::new_v4().to_string(),
            reflected_at: Utc::now(),
            old_prompt: current_prompt,
            new_prompt: response.new_prompt,
            reasoning: response.reasoning,
            guiding_principles: serde_json::to_string(&principles)?,
        };

        self.db.save_reflection(&reflection_record)?;

        info!("💾 Reflection saved to memory");

        Ok(())
    }

    /// Format important posts for reflection prompt
    fn format_important_posts(&self, posts: &[ImportantPost]) -> String {
        if posts.is_empty() {
            return "No important interactions marked yet.".to_string();
        }

        posts
            .iter()
            .enumerate()
            .map(|(i, post)| {
                format!(
                    "{}. [{}]\nPost: {}\nWhy Important: {}\n",
                    i + 1,
                    post.marked_at.format("%Y-%m-%d %H:%M UTC"),
                    post.post_body.chars().take(200).collect::<String>(),
                    post.why_important
                )
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Get the current system prompt (from database, with fallback to config)
    /// Let the agent select which threads to engage with based on interest
    async fn select_threads_to_engage(&self, threads: &[crate::graphchan_client::Thread]) -> Result<Vec<crate::graphchan_client::Thread>> {
        if threads.is_empty() {
            return Ok(vec![]);
        }

        let system_prompt = self.get_current_system_prompt();

        // Build thread list
        let thread_list = threads
            .iter()
            .enumerate()
            .map(|(i, t)| format!("{}. [ID: {}] \"{}\"", i + 1, t.id, t.title))
            .collect::<Vec<_>>()
            .join("\n");

        let messages = vec![
            Message {
                role: "system".to_string(),
                content: format!(
                    "{}\n\n\
                     You are choosing which discussion threads to participate in. \
                     Consider:\n\
                     - Which topics align with your interests and personality?\n\
                     - Where can you contribute something unique or valuable?\n\
                     - Which conversations seem active and engaging?\n\
                     - You don't have to respond to everything - be selective!\n\n\
                     Respond with JSON:\n\
                     {{\n  \
                       \"selected_thread_ids\": [\"thread_id_1\", \"thread_id_2\", ...],\n  \
                       \"reasoning\": \"why you chose these threads\"\n\
                     }}",
                    system_prompt
                ),
            },
            Message {
                role: "user".to_string(),
                content: format!(
                    "Available threads to participate in:\n\n{}\n\n\
                     Which thread(s) interest you? Select the IDs of threads you want to engage with.",
                    thread_list
                ),
            },
        ];

        let decision: ThreadSelectionDecision = self.llm
            .generate_json(messages, None)
            .await
            .context("Failed to get thread selection decision")?;

        info!("🧭 Thread selection: {}", decision.reasoning);

        // Filter to only the selected threads
        let selected: Vec<_> = threads
            .iter()
            .filter(|t| decision.selected_thread_ids.contains(&t.id))
            .cloned()
            .collect();

        Ok(selected)
    }

    fn get_current_system_prompt(&self) -> String {
        self.db.get_current_system_prompt()
            .ok()
            .flatten()
            .unwrap_or_else(|| self.config.system_prompt.clone())
    }

    /// Decide whether to generate an image for this response
    async fn should_generate_image(
        &self,
        thread: &ThreadDetails,
        post: &Post,
    ) -> Result<Option<ImageGenerationDecision>> {
        // Skip if ComfyUI not configured
        if self.comfyui.is_none() {
            debug!("ComfyUI not configured, skipping image generation");
            return Ok(None);
        }

        if !self.config.enable_image_generation {
            debug!("Image generation disabled in config, skipping");
            return Ok(None);
        }

        info!("🎨 Deciding whether to generate image for post...");

        let system_prompt = self.get_current_system_prompt();

        let messages = vec![
            Message {
                role: "system".to_string(),
                content: format!(
                    "{}\n\n\
                     You are deciding whether to generate an image to attach to your response. \
                     Consider:\n\
                     - Would an image enhance your response?\n\
                     - Is the conversation visual in nature?\n\
                     - Would an illustration, meme, or artwork add value?\n\
                     - Is this a request for visual content?\n\n\
                     Respond with JSON:\n\
                     {{\n  \
                       \"should_generate\": true/false,\n  \
                       \"reasoning\": \"why or why not\",\n  \
                       \"image_description\": \"what to generate if true, null otherwise\"\n\
                     }}",
                    system_prompt
                ),
            },
            Message {
                role: "user".to_string(),
                content: format!(
                    "Thread: {}\n\nPost: {}\n\nYour planned response context",
                    thread.thread.title,
                    post.body
                ),
            },
        ];

        let decision: ImageGenerationDecision = self.llm
            .generate_json(messages, None)
            .await
            .context("Failed to get image generation decision")?;

        info!(
            "🎨 Image decision: {} - {}",
            if decision.should_generate { "GENERATE" } else { "SKIP" },
            decision.reasoning
        );

        if decision.should_generate {
            Ok(Some(decision))
        } else {
            Ok(None)
        }
    }

    /// Generate an image with iterative refinement
    async fn generate_and_refine_image(
        &self,
        initial_description: &str,
        context: &str,
    ) -> Result<Vec<u8>> {
        let comfyui = self.comfyui.as_ref()
            .ok_or_else(|| anyhow::anyhow!("ComfyUI not configured"))?;

        let mut current_prompt = self.create_image_prompt(initial_description).await?;
        let mut iteration = 0;

        // Use static negative prompt from config for SD/SDXL, None for Flux
        let negative_prompt = match *comfyui.workflow_type() {
            crate::config::WorkflowType::SD | crate::config::WorkflowType::SDXL => {
                Some(comfyui.negative_prompt())
            }
            crate::config::WorkflowType::Flux => None,
        };

        loop {
            iteration += 1;
            info!("Generating image (iteration {}): {}", iteration, current_prompt);

            // Generate image with static negative prompt
            let image_bytes = comfyui.generate_image(
                &current_prompt,
                negative_prompt,
            ).await.context("Failed to generate image")?;

            info!("Image generated ({} bytes)", image_bytes.len());

            // Try to evaluate with vision model (optional - not all models support vision)
            match self.llm.evaluate_image(
                &image_bytes,
                &current_prompt,
                context,
            ).await {
                Ok(evaluation) => {
                    info!("✨ Vision evaluation: satisfactory={}, reasoning={}",
                          evaluation.satisfactory, evaluation.reasoning);

                    if evaluation.satisfactory {
                        info!("Image accepted after {} iteration(s)", iteration);
                        return Ok(image_bytes);
                    }

                    // Refine prompt if suggested
                    if let Some(refinement) = evaluation.suggested_prompt_refinement {
                        info!("Refining prompt: {}", refinement);
                        current_prompt = self.create_image_prompt(&refinement).await?;
                    } else {
                        warn!("Vision model not satisfied but no refinement suggested, accepting anyway");
                        return Ok(image_bytes);
                    }

                    if iteration >= 10 {
                        warn!("Max iterations reached, accepting current image");
                        return Ok(image_bytes);
                    }
                }
                Err(e) => {
                    // Vision evaluation not supported or failed - accept the image
                    info!("⚠️  Vision evaluation not available ({}), accepting image", e);
                    return Ok(image_bytes);
                }
            }
        }
    }

    /// Create an image prompt (tags or natural language based on workflow type)
    async fn create_image_prompt(&self, description: &str) -> Result<String> {
        let comfyui = self.comfyui.as_ref()
            .ok_or_else(|| anyhow::anyhow!("ComfyUI not configured"))?;

        let prompt_style = comfyui.prompt_style();
        let system_prompt = self.get_current_system_prompt();

        let messages = vec![
            Message {
                role: "system".to_string(),
                content: format!(
                    "{}\n\n\
                     You are crafting an image prompt that aligns with your personality and aesthetic. \
                     {}",
                    system_prompt,
                    if prompt_style == "tags" {
                        "Convert the description into Stable Diffusion tags using comma-separated keywords. \
                         Use emphasis syntax: (emphasis), ((strong emphasis)), [de-emphasis]. \
                         Output ONLY the positive prompt tags - negative prompts are handled separately."
                    } else {
                        "Write a vivid, detailed natural language prompt for Flux/Black Forest Labs models. \
                         Use your unique voice and aesthetic sensibility. \
                         Output ONLY the prompt description."
                    }
                ),
            },
            Message {
                role: "user".to_string(),
                content: format!("Create an image prompt for: {}", description),
            },
        ];

        self.llm.generate(messages).await
    }
}

#[derive(Debug, Deserialize, Serialize)]
struct ImageGenerationDecision {
    should_generate: bool,
    reasoning: String,
    image_description: Option<String>,
}
