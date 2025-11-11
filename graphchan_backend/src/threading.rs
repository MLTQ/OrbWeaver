use crate::database::models::{PostRecord, ThreadRecord};
use crate::database::repositories::{PostRepository, ThreadRepository};
use crate::database::Database;
use crate::utils::now_utc_iso;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone)]
pub struct ThreadService {
    database: Database,
}

impl ThreadService {
    pub fn new(database: Database) -> Self {
        Self { database }
    }

    pub fn list_threads(&self, limit: usize) -> Result<Vec<ThreadSummary>> {
        self.database.with_repositories(|repos| {
            let threads = repos.threads().list_recent(limit)?;
            Ok(threads
                .into_iter()
                .map(ThreadSummary::from_record)
                .collect())
        })
    }

    pub fn get_thread(&self, thread_id: &str) -> Result<Option<ThreadDetails>> {
        self.database.with_repositories(|repos| {
            let thread = repos.threads().get(thread_id)?;
            let Some(thread) = thread else {
                return Ok(None);
            };
            let posts_repo = repos.posts();
            let posts = posts_repo.list_for_thread(thread_id)?;
            let mut views = Vec::with_capacity(posts.len());
            for post in posts {
                let parents = posts_repo.parents_of(&post.id)?;
                views.push(PostView::from_record(post, parents));
            }
            Ok(Some(ThreadDetails {
                thread: ThreadSummary::from_record(thread),
                posts: views,
            }))
        })
    }

    pub fn create_thread(&self, input: CreateThreadInput) -> Result<ThreadDetails> {
        if input.title.trim().is_empty() {
            anyhow::bail!("thread title may not be empty");
        }
        let thread_id = Uuid::new_v4().to_string();
        let created_at = input.created_at.unwrap_or_else(now_utc_iso);
        let thread_record = ThreadRecord {
            id: thread_id.clone(),
            title: input.title,
            creator_peer_id: input.creator_peer_id.clone(),
            created_at: created_at.clone(),
            pinned: input.pinned.unwrap_or(false),
        };

        let initial_post_body = input.body.clone();
        let author_peer_id = input.creator_peer_id.clone();

        self.database.with_repositories(|repos| {
            repos.threads().create(&thread_record)?;
            if let Some(body) = initial_post_body {
                if !body.trim().is_empty() {
                    let post_record = PostRecord {
                        id: Uuid::new_v4().to_string(),
                        thread_id: thread_id.clone(),
                        author_peer_id,
                        body,
                        created_at: created_at.clone(),
                        updated_at: None,
                    };
                    repos.posts().create(&post_record)?;
                }
            }
            Ok(())
        })?;

        self.get_thread(&thread_id)
            .and_then(|opt| opt.context("thread creation lost newly inserted record"))
    }

    pub fn create_post(&self, input: CreatePostInput) -> Result<PostView> {
        if input.body.trim().is_empty() {
            anyhow::bail!("post body may not be empty");
        }
        let post_record = PostRecord {
            id: Uuid::new_v4().to_string(),
            thread_id: input.thread_id.clone(),
            author_peer_id: input.author_peer_id.clone(),
            body: input.body,
            created_at: input.created_at.unwrap_or_else(now_utc_iso),
            updated_at: None,
        };

        let stored_post = self.database.with_repositories(|repos| {
            // ensure thread exists
            if repos.threads().get(&post_record.thread_id)?.is_none() {
                anyhow::bail!("thread not found");
            }
            let posts_repo = repos.posts();
            posts_repo.create(&post_record)?;
            posts_repo.add_relationships(&post_record.id, &input.parent_post_ids)?;
            Ok(post_record.clone())
        })?;

        Ok(PostView {
            id: stored_post.id,
            thread_id: stored_post.thread_id,
            author_peer_id: stored_post.author_peer_id,
            body: stored_post.body,
            created_at: stored_post.created_at,
            updated_at: stored_post.updated_at,
            parent_post_ids: input.parent_post_ids,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThreadSummary {
    pub id: String,
    pub title: String,
    pub creator_peer_id: Option<String>,
    pub created_at: String,
    pub pinned: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PostView {
    pub id: String,
    pub thread_id: String,
    pub author_peer_id: Option<String>,
    pub body: String,
    pub created_at: String,
    pub updated_at: Option<String>,
    pub parent_post_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThreadDetails {
    pub thread: ThreadSummary,
    pub posts: Vec<PostView>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateThreadInput {
    pub title: String,
    pub body: Option<String>,
    pub creator_peer_id: Option<String>,
    pub pinned: Option<bool>,
    /// Optional timestamp for imported threads. If None, uses current time.
    #[serde(default)]
    pub created_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreatePostInput {
    pub thread_id: String,
    pub author_peer_id: Option<String>,
    pub body: String,
    #[serde(default)]
    pub parent_post_ids: Vec<String>,
    /// Optional timestamp for imported posts. If None, uses current time.
    #[serde(default)]
    pub created_at: Option<String>,
}

impl ThreadSummary {
    fn from_record(record: ThreadRecord) -> Self {
        Self {
            id: record.id,
            title: record.title,
            creator_peer_id: record.creator_peer_id,
            created_at: record.created_at,
            pinned: record.pinned,
        }
    }
}

impl PostView {
    fn from_record(record: PostRecord, parent_post_ids: Vec<String>) -> Self {
        Self {
            id: record.id,
            thread_id: record.thread_id,
            author_peer_id: record.author_peer_id,
            body: record.body,
            created_at: record.created_at,
            updated_at: record.updated_at,
            parent_post_ids,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::database::Database;
    use rusqlite::Connection;

    fn setup_service() -> ThreadService {
        let conn = Connection::open_in_memory().expect("in-memory db");
        let db = Database::from_connection(conn, true);
        db.ensure_migrations().expect("migrations");
        ThreadService::new(db)
    }

    #[test]
    fn thread_creation_creates_initial_post() {
        let service = setup_service();
        let details = service
            .create_thread(CreateThreadInput {
                title: "Example".into(),
                body: Some("Hello".into()),
                creator_peer_id: None,
                pinned: None,
                created_at: None,
            })
            .expect("create thread");
        assert_eq!(details.thread.title, "Example");
        assert_eq!(details.posts.len(), 1);
        assert_eq!(details.posts[0].body, "Hello");
    }

    #[test]
    fn create_post_appends_to_thread() {
        let service = setup_service();
        let details = service
            .create_thread(CreateThreadInput {
                title: "Thread".into(),
                body: None,
                creator_peer_id: None,
                pinned: None,
                created_at: None,
            })
            .expect("create thread");

        let post = service
            .create_post(CreatePostInput {
                thread_id: details.thread.id.clone(),
                author_peer_id: None,
                body: "Reply".into(),
                parent_post_ids: vec![],
                created_at: None,
            })
            .expect("create post");
        assert_eq!(post.body, "Reply");

        let fetched = service
            .get_thread(&details.thread.id)
            .expect("fetch thread")
            .expect("thread exists");
        assert_eq!(fetched.posts.len(), 1);
        assert_eq!(fetched.posts[0].body, "Reply");
    }
}
