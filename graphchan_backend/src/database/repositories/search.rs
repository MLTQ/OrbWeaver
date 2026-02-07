use crate::database::models::{FileRecord, PostRecord, SearchResultRecord, SearchResultType};
use anyhow::Result;
use rusqlite::{params, Connection};

pub(super) struct SqliteSearchRepository<'conn> {
    pub(super) conn: &'conn Connection,
}

impl<'conn> super::SearchRepository for SqliteSearchRepository<'conn> {
    fn search(&self, query: &str, limit: usize) -> Result<Vec<SearchResultRecord>> {
        if query.trim().is_empty() {
            return Ok(Vec::new());
        }

        let mut results = Vec::new();

        // Search posts
        let mut stmt = self.conn.prepare(
            r#"SELECT
                p.id, p.thread_id, p.author_peer_id, p.author_friendcode, p.body, p.created_at, p.updated_at, p.metadata,
                bm25(posts_fts) as score,
                t.title,
                snippet(posts_fts, -1, '<mark>', '</mark>', '...', 30) as snippet
            FROM posts_fts
            JOIN posts p ON posts_fts.id = p.id
            JOIN threads t ON p.thread_id = t.id
            WHERE posts_fts MATCH ?1
            ORDER BY score ASC, datetime(p.created_at) DESC
            LIMIT ?2"#,
        )?;

        let post_results = stmt.query_map(params![query, limit as i64], |row| {
            Ok(SearchResultRecord {
                result_type: SearchResultType::Post,
                post: PostRecord {
                    id: row.get(0)?,
                    thread_id: row.get(1)?,
                    author_peer_id: row.get(2)?,
                    author_friendcode: row.get(3)?,
                    body: row.get(4)?,
                    created_at: row.get(5)?,
                    updated_at: row.get(6)?,
                    metadata: row.get(7)?,
                },
                file: None,
                bm25_score: row.get(8)?,
                thread_title: row.get(9)?,
                snippet: row.get(10)?,
            })
        })?;

        for result in post_results {
            results.push(result?);
        }

        // Search files
        let mut stmt = self.conn.prepare(
            r#"SELECT
                p.id, p.thread_id, p.author_peer_id, p.author_friendcode, p.body, p.created_at, p.updated_at, p.metadata,
                f.id, f.post_id, f.path, f.original_name, f.mime, f.size_bytes, f.blob_id, f.checksum, f.ticket, f.download_status,
                bm25(files_fts) as score,
                t.title,
                snippet(files_fts, -1, '<mark>', '</mark>', '...', 30) as snippet
            FROM files_fts
            JOIN files f ON files_fts.id = f.id
            JOIN posts p ON f.post_id = p.id
            JOIN threads t ON p.thread_id = t.id
            WHERE files_fts MATCH ?1
            ORDER BY score ASC, datetime(p.created_at) DESC
            LIMIT ?2"#,
        )?;

        let file_results = stmt.query_map(params![query, limit as i64], |row| {
            Ok(SearchResultRecord {
                result_type: SearchResultType::File,
                post: PostRecord {
                    id: row.get(0)?,
                    thread_id: row.get(1)?,
                    author_peer_id: row.get(2)?,
                    author_friendcode: row.get(3)?,
                    body: row.get(4)?,
                    created_at: row.get(5)?,
                    updated_at: row.get(6)?,
                    metadata: row.get(7)?,
                },
                file: Some(FileRecord {
                    id: row.get(8)?,
                    post_id: row.get(9)?,
                    path: row.get(10)?,
                    original_name: row.get(11)?,
                    mime: row.get(12)?,
                    size_bytes: row.get(13)?,
                    blob_id: row.get(14)?,
                    checksum: row.get(15)?,
                    ticket: row.get(16)?,
                    download_status: row.get(17)?,
                }),
                bm25_score: row.get(18)?,
                thread_title: row.get(19)?,
                snippet: row.get(20)?,
            })
        })?;

        for result in file_results {
            results.push(result?);
        }

        // Re-sort combined results by score
        results.sort_by(|a, b| {
            a.bm25_score.partial_cmp(&b.bm25_score)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| b.post.created_at.cmp(&a.post.created_at))
        });

        results.truncate(limit);
        Ok(results)
    }
}
