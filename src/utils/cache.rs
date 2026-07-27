//! On-disk JSON cache for Foxhole War API responses, under `./cache/`.
//!
//! Nothing here is durable state — it exists to serve `304 Not Modified`
//! revalidation (the War API's one explicit rule is to respect its cache
//! headers). Every failure is therefore recoverable: a cache miss just means a
//! full fetch, so I/O errors are logged and folded into `None` rather than
//! propagated (QA L-7 — the old code panicked on a cold cache).

use reqwest::StatusCode;

use super::api_definitions::foxhole::{DynamicMapData, Maps, StaticMapData, WarReport};
use super::db::Shard;
use super::http;

const CACHE_DIRS: [&str; 5] = [
    "./cache",
    "./cache/map_choices",
    "./cache/static",
    "./cache/dynamic",
    "./cache/war_reports",
];

async fn create_cache_dirs() {
    for dir in CACHE_DIRS {
        if let Err(err) = tokio::fs::create_dir_all(dir).await {
            log::warn!("could not create the cache directory {dir}: {err}");
        }
    }
}

async fn write_json<T: serde::Serialize>(path: String, value: &T, what: &str) {
    let encoded = match serde_json::to_string(value) {
        Ok(s) => s,
        Err(err) => return log::warn!("could not encode {what} for the cache: {err}"),
    };

    if let Err(err) = tokio::fs::write(&path, encoded).await {
        log::warn!("could not write {what} to {path}: {err}");
    }
}

async fn read_json<T: serde::de::DeserializeOwned>(path: String) -> Option<T> {
    let raw = tokio::fs::read_to_string(&path).await.ok()?;

    match serde_json::from_str(&raw) {
        Ok(value) => Some(value),
        Err(err) => {
            // A corrupt cache file should not be fatal; drop it and refetch.
            log::warn!("cache file {path} is unreadable ({err}), ignoring it");
            None
        }
    }
}

/// Refreshes the per-shard map list used by every autocomplete.
pub async fn save_maps_cache() {
    create_cache_dirs().await;

    let client = http::client().clone();

    for shard in Shard::list_all() {
        let api_url = shard.api_url();
        let shard_name = shard.as_str();

        let resp = match client
            .get(format!("{api_url}/worldconquest/maps"))
            .send()
            .await
        {
            Ok(resp) => resp,
            Err(err) => {
                log::warn!("could not reach shard {shard_name} for the map list: {err}");
                continue;
            }
        };

        if resp.status() != StatusCode::OK {
            log::warn!("shard {shard_name} returned {} for the map list", resp.status());
            continue;
        }

        let maps = match resp.json::<Maps>().await {
            Ok(maps) => maps,
            Err(err) => {
                log::warn!("could not decode the map list for shard {shard_name}: {err}");
                continue;
            }
        };

        write_json(
            format!("./cache/map_choices/maps-{shard_name}.json"),
            &maps,
            "the map list",
        )
        .await;
    }
}

/// Returns an empty list rather than panicking when the cache has not been
/// written yet — autocomplete fires before the first map refresh on a cold start.
pub async fn load_maps(shard: Shard) -> Maps {
    let shard_name = shard.as_str();

    read_json(format!("./cache/map_choices/maps-{shard_name}.json"))
        .await
        .unwrap_or_default()
}

pub async fn save_map_cache(
    dynamic_data: &DynamicMapData,
    static_data: &StaticMapData,
    map_name: &str,
    shard: &str,
) {
    create_cache_dirs().await;

    write_json(
        format!("./cache/dynamic/Dynamic_{map_name}-{shard}.json"),
        dynamic_data,
        &format!("dynamic data for {map_name}"),
    )
    .await;
    write_json(
        format!("./cache/static/Static_{map_name}-{shard}.json"),
        static_data,
        &format!("static data for {map_name}"),
    )
    .await;
}

pub async fn save_war_report(war_report: &WarReport, map_name: &str, shard: &str) {
    create_cache_dirs().await;

    write_json(
        format!("./cache/war_reports/Report_{map_name}-{shard}.json"),
        war_report,
        &format!("war report for {map_name}"),
    )
    .await;
}

/// The dynamic and static halves are returned independently so each can be
/// revalidated with its own ETag. The old combined `Option<(dynamic, static)>`
/// is what made the mismatched-304 path unwrap an absent cache (QA C-2).
pub async fn load_dynamic_cache(map_name: &str, shard: &str) -> Option<DynamicMapData> {
    read_json(format!("./cache/dynamic/Dynamic_{map_name}-{shard}.json")).await
}

pub async fn load_static_cache(map_name: &str, shard: &str) -> Option<StaticMapData> {
    read_json(format!("./cache/static/Static_{map_name}-{shard}.json")).await
}

pub async fn load_war_report(map_name: &str, shard: &str) -> Option<WarReport> {
    read_json(format!("./cache/war_reports/Report_{map_name}-{shard}.json")).await
}
