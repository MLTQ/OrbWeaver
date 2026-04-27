//! Background SSE consumer for the backend `/events` stream.
//!
//! The backend emits live `AppEvent`s (see graphchan_backend::events) every
//! time the gossip pipeline mutates state. Without this consumer the UI is
//! poll-only and won't know about an inbound DM until the user manually
//! reloads the conversations list. With it, the UI gets a wake-up signal
//! within milliseconds.
//!
//! Design:
//! - One long-lived OS thread, spawned once at app startup.
//! - Connects to GET /events, treats the body as a sequence of `event: <name>\n
//!   data: <json>\n\n` blocks (text/event-stream).
//! - Each `data:` line is parsed as `ServerEvent` and forwarded as
//!   `AppMessage::ServerEvent`. The dispatcher decides what to refresh.
//! - On any IO error or stream end, sleeps with exponential backoff (1s → 30s
//!   cap) and reconnects. The backend may have restarted, the network may
//!   have hiccuped — we don't care, we just keep trying.
//! - The keep-alive ping the backend sends every 15s arrives as
//!   `event: keep-alive\ndata: keep-alive` and is silently ignored.

use std::io::{BufRead, BufReader};
use std::sync::mpsc::Sender;
use std::thread;
use std::time::Duration;

use crate::api::ApiClient;
use crate::models::ServerEvent;

use super::messages::AppMessage;

/// Cap for exponential backoff between reconnect attempts.
const MAX_BACKOFF_SECS: u64 = 30;
/// Read timeout for the streaming response. Long enough to not interfere with
/// the 15-second backend keep-alive, short enough that a totally silent
/// connection doesn't park the thread forever.
const READ_TIMEOUT_SECS: u64 = 60;

/// Spawn the SSE consumer thread. Runs forever; never joined.
pub fn spawn_event_stream(client: ApiClient, tx: Sender<AppMessage>) {
    thread::Builder::new()
        .name("graphchan-sse".into())
        .spawn(move || run(client, tx))
        .expect("spawn event stream thread");
}

fn run(client: ApiClient, tx: Sender<AppMessage>) {
    let mut backoff_secs = 1u64;
    loop {
        match connect_and_pump(&client, &tx) {
            Ok(()) => {
                // Stream ended cleanly (server closed). Reset backoff so a
                // graceful restart reconnects immediately.
                backoff_secs = 1;
            }
            Err(err) => {
                log::warn!(
                    "SSE /events stream error: {} — reconnecting in {}s",
                    err,
                    backoff_secs
                );
                thread::sleep(Duration::from_secs(backoff_secs));
                backoff_secs = (backoff_secs * 2).min(MAX_BACKOFF_SECS);
            }
        }
        // If the channel is closed (UI exited), stop trying.
        if tx.send(AppMessage::EventStreamConnected).is_err() {
            return;
        }
    }
}

/// One pass: open the SSE stream, parse events until the stream ends or errors.
fn connect_and_pump(
    client: &ApiClient,
    tx: &Sender<AppMessage>,
) -> Result<(), Box<dyn std::error::Error>> {
    let url = format!("{}/events", client.base_url());
    let response = client
        .raw_client()
        .get(&url)
        .timeout(Duration::from_secs(READ_TIMEOUT_SECS))
        .send()?
        .error_for_status()?;

    let mut reader = BufReader::new(response);

    // Per the SSE spec, each event is a sequence of `field: value` lines
    // terminated by an empty line. We only care about `data:` (and ignore
    // `event:` because our payloads are self-tagged via the `type` field).
    let mut data_buf = String::new();
    let mut line = String::new();
    loop {
        line.clear();
        let n = reader.read_line(&mut line)?;
        if n == 0 {
            // End of stream
            return Ok(());
        }
        let trimmed = line.trim_end_matches(['\r', '\n']);

        if trimmed.is_empty() {
            // Empty line = event terminator. Dispatch what we accumulated.
            if !data_buf.is_empty() {
                dispatch_event(&data_buf, tx);
                data_buf.clear();
            }
        } else if let Some(rest) = trimmed.strip_prefix("data:") {
            // Per spec, multiple `data:` lines concatenate with newlines.
            // Our backend only emits single-line JSON so the trim is fine.
            let payload = rest.strip_prefix(' ').unwrap_or(rest);
            if !data_buf.is_empty() {
                data_buf.push('\n');
            }
            data_buf.push_str(payload);
        }
        // Other fields (event:, id:, retry:) are ignored — payloads are
        // self-describing via serde tag.
    }
}

fn dispatch_event(data: &str, tx: &Sender<AppMessage>) {
    // Ignore the keep-alive comment frame the backend emits. It comes through
    // as the literal string "keep-alive" in the data field rather than JSON.
    if data == "keep-alive" {
        return;
    }
    match serde_json::from_str::<ServerEvent>(data) {
        Ok(event) => {
            if let ServerEvent::Unknown = event {
                log::trace!("ignoring unknown SSE event: {data}");
                return;
            }
            let _ = tx.send(AppMessage::ServerEvent(event));
        }
        Err(err) => {
            log::warn!("failed to parse SSE event: {err} — payload: {data}");
        }
    }
}
