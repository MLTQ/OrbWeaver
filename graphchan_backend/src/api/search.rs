use super::{AppState, ApiError, ApiResult, FileResponse};
use axum::extract::{Query, State};
use axum::Json;
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
pub(crate) struct SearchParams {
    q: String,
    #[serde(default = "default_search_limit")]
    limit: Option<usize>,
}

pub(crate) fn default_search_limit() -> Option<usize> {
    Some(50)
}

#[derive(Debug, Serialize)]
pub(crate) struct SearchResultView {
    pub result_type: String,
    pub post: crate::threading::PostView,
    pub file: Option<FileResponse>,
    pub thread_id: String,
    pub thread_title: String,
    pub bm25_score: f64,
    pub snippet: String,
}

#[derive(Debug, Serialize)]
pub(crate) struct SearchResponse {
    pub results: Vec<SearchResultView>,
    pub query: String,
}

pub(crate) async fn search_handler(
    State(state): State<AppState>,
    Query(params): Query<SearchParams>,
) -> ApiResult<SearchResponse> {
    use crate::database::repositories::SearchRepository;

    let query = params.q.trim();
    let limit = params.limit.unwrap_or(50).min(200);

    if query.is_empty() {
        return Ok(Json(SearchResponse {
            results: Vec::new(),
            query: query.to_string(),
        }));
    }

    let search_results = state.database.with_repositories(|repos| {
        repos.search().search(query, limit)
    }).map_err(ApiError::Internal)?;

    let results = search_results.into_iter().map(|r| {
        // Parse metadata JSON if present
        let metadata = r.post.metadata.as_ref().and_then(|json_str| {
            serde_json::from_str::<crate::threading::PostMetadata>(json_str).ok()
        });

        SearchResultView {
            result_type: match r.result_type {
                crate::database::models::SearchResultType::Post => "post".to_string(),
                crate::database::models::SearchResultType::File => "file".to_string(),
            },
            post: crate::threading::PostView {
                id: r.post.id,
                thread_id: r.post.thread_id.clone(),
                author_peer_id: r.post.author_peer_id,
                author_friendcode: r.post.author_friendcode,
                body: r.post.body,
                created_at: r.post.created_at,
                updated_at: r.post.updated_at,
                parent_post_ids: Vec::new(),
                files: Vec::new(),
                thread_hash: None,
                metadata,
            },
            file: r.file.map(|f| {
                let download_url = format!("/files/{}", f.id);
                FileResponse {
                    id: f.id,
                    post_id: f.post_id,
                    original_name: f.original_name,
                    mime: f.mime,
                    size_bytes: f.size_bytes,
                    checksum: f.checksum,
                    blob_id: f.blob_id,
                    ticket: f.ticket.clone(),
                    path: f.path,
                    download_url,
                    present: true,
                    download_status: f.download_status,
                }
            }),
            thread_id: r.post.thread_id,
            thread_title: r.thread_title,
            bm25_score: r.bm25_score,
            snippet: r.snippet,
        }
    }).collect();

    Ok(Json(SearchResponse {
        results,
        query: query.to_string(),
    }))
}
