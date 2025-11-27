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
pub fn import_fourchan_thread(api: &ApiClient, url: &str) -> Result<String> {
    api.import_thread(url)
}
