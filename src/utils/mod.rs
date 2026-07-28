pub mod api_definitions;
pub mod cache;
pub mod cron;
pub mod db;
pub mod entitlement;
pub mod frontline;
pub mod http;
pub mod logging;
pub mod map_render;
pub mod regions;
pub mod request_processing;
pub mod review;
pub mod schedule;

/// Formats a Foxhole API millisecond timestamp.
///
/// The API can report a timestamp outside chrono's representable range; the old
/// code unwrapped it and took the whole command down with it (QA C-5).
pub fn format_timestamp(millis: i64) -> String {
    match chrono::DateTime::from_timestamp_millis(millis) {
        Some(ts) => ts.format("%Y %m %d %H:%M:%S").to_string(),
        None => "unknown".to_string(),
    }
}
