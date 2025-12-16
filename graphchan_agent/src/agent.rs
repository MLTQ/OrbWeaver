use anyhow::{Context, Result};
use log::{debug, info, warn};
use std::collections::HashSet;

use crate::config::{AgentConfig, RespondStrategy};
use crate::graphchan_client::{GraphchanClient, Post, ThreadDetails};
use crate::llm_client::{LlmClient, Message};

pub struct Agent {
    config: AgentConfig,
    graphchan: GraphchanClient,
    llm: LlmClient,
    seen_posts: HashSet<String>,
}

impl Agent {
    pub fn new(config: AgentConfig) -> Self {
        let graphchan = GraphchanClient::new(config.graphchan_api_url.clone());
        let llm = LlmClient::new(
            config.llm_api_url.clone(),
            config.llm_api_key.clone(),
            config.llm_model.clone(),
        );

        Self {
            config,
            graphchan,
            llm,
            seen_posts: HashSet::new(),
        }
    }

    /// Main agent loop
    pub async fn run(&mut self) -> Result<()> {
        info!("Agent started with username: {}", self.config.username);
        info!("Polling interval: {:?}", self.config.poll_interval());
        info!("Respond strategy: {:?}", self.config.respond_to);

        loop {
            if let Err(e) = self.process_threads().await {
                warn!("Error processing threads: {}", e);
            }

            tokio::time::sleep(self.config.poll_interval()).await;
        }
    }

    async fn process_threads(&mut self) -> Result<()> {
        let threads = self.graphchan.list_threads().await?;
        info!("Found {} threads", threads.len());

        for thread in threads {
            // Check if we should process this thread
            if let RespondStrategy::Threads { ref thread_ids } = self.config.respond_to {
                if !thread_ids.contains(&thread.id) {
                    continue;
                }
            }

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

        Ok(())
    }

    fn build_context(&self, thread: &ThreadDetails, post: &Post) -> Result<Vec<Message>> {
        let mut messages = vec![Message {
            role: "system".to_string(),
            content: self.config.system_prompt.clone(),
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
                self.config.system_prompt
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
}
