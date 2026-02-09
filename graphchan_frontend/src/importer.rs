use anyhow::Result;

use crate::api::ApiClient;

/// Import a 4chan thread via the backend's /import endpoint.
///
/// This delegates all the work to the backend, which handles:
/// - Fetching the thread JSON from 4chan
/// - Creating the thread and all posts in the database
/// - Downloading and storing all images
/// - Broadcasting a single complete ThreadSnapshot when done
///
/// This approach avoids flooding the network with individual PostUpdate
/// messages for each post, which was causing timeouts and message loss.
pub fn import_fourchan_thread(api: &ApiClient, url: &str, topics: Vec<String>) -> Result<String> {
    api.import_thread(url, topics)
}

pub fn import_reddit_thread(api: &ApiClient, url: &str, topics: Vec<String>) -> Result<String> {
    #[derive(serde::Serialize)]
    struct ImportRequest {
        url: String,
        platform: Option<String>,
        #[serde(skip_serializing_if = "Vec::is_empty")]
        topics: Vec<String>,
    }

    let req = ImportRequest {
        url: url.to_string(),
        platform: Some("reddit".to_string()),
        topics,
    };

    let response = api.post_json("/import", &req)?;

    #[derive(serde::Deserialize)]
    struct ImportResponse {
        id: String,
    }

    let wrapper: ImportResponse = response.json()?;
    Ok(wrapper.id)
}
