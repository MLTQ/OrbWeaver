//! Shared helpers and constants will live here.

use chrono::Utc;

pub const APP_NAME: &str = "graphchan_backend";

pub fn now_utc_iso() -> String {
    Utc::now().to_rfc3339()
}
