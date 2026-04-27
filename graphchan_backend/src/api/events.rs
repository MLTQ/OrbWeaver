use super::AppState;
use axum::extract::State;
use axum::response::sse::{Event, KeepAlive, Sse};
use futures_util::stream::Stream;
use std::convert::Infallible;
use std::time::Duration;
use tokio_stream::wrappers::BroadcastStream;
use tokio_stream::StreamExt;

/// Server-Sent Events stream of live `AppEvent`s emitted by the gossip
/// ingest pipeline. Agents and UIs subscribe instead of polling.
///
/// Each emitted event is JSON-encoded `AppEvent` (see `crate::events`). The
/// SSE event `name` field carries the event variant name (`post_added`,
/// `thread_announced`, etc.) so simple consumers can filter without parsing.
/// A `lagged` event is sent when the per-client ring buffer overflows; the
/// client should refetch state from REST endpoints to recover.
pub(crate) async fn stream_events(
    State(state): State<AppState>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let rx = state.network.events.subscribe();
    let stream = BroadcastStream::new(rx).map(|item| {
        let ev = match item {
            Ok(event) => {
                let name = event_name(&event);
                let data = serde_json::to_string(&event).unwrap_or_else(|_| "{}".into());
                Event::default().event(name).data(data)
            }
            Err(_lag) => Event::default()
                .event("lagged")
                .data("{\"reason\":\"slow_consumer\"}"),
        };
        Ok::<Event, Infallible>(ev)
    });

    Sse::new(stream).keep_alive(
        KeepAlive::new()
            .interval(Duration::from_secs(15))
            .text("keep-alive"),
    )
}

fn event_name(event: &crate::events::AppEvent) -> &'static str {
    use crate::events::AppEvent::*;
    match event {
        PostAdded { .. } => "post_added",
        ThreadAnnounced { .. } => "thread_announced",
        FileAnnounced { .. } => "file_announced",
        FileDownloaded { .. } => "file_downloaded",
        ProfileUpdated { .. } => "profile_updated",
        ReactionUpdated { .. } => "reaction_updated",
        DmReceived { .. } => "dm_received",
    }
}
