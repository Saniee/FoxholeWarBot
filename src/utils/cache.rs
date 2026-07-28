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

/// Whether a shard can actually serve requests, as its region list reports it.
///
/// The region list is the right thing to ask. It is what `/get-map`,
/// `/war-report` and `/full-map` all need before they can do anything, and a
/// shard that lists no regions cannot serve any of them whatever the reason —
/// down, between wars, or retired. Asking `/worldconquest/war` instead only
/// establishes that *something* answered on that host.
#[derive(Debug, Clone)]
pub enum ShardHealth {
    /// Serving a war, with this many regions in it.
    Ready(usize),
    /// Answered, but listed no regions. Nothing to render.
    NoRegions,
    /// Answered with something that isn't a region list.
    Unavailable(StatusCode),
    /// Never answered: DNS, refused connection, TLS, or a timeout.
    Unreachable(String),
}

impl ShardHealth {
    /// What to tell a user who picked this shard, or `None` if it is fine.
    ///
    /// Each case says which of the two it is — the shard is down, or the bot
    /// can't get to it — because the two have different answers. The first is
    /// waited out or worked around by picking another shard; the second may be
    /// the host's own network, and a user who is told "try again later" for it
    /// waits for something that will not happen on its own.
    pub fn explain(&self, shard: Shard) -> Option<String> {
        let name = shard.as_str();

        match self {
            ShardHealth::Ready(_) => None,
            ShardHealth::NoRegions => Some(format!(
                "Shard **{name}** isn't running a war right now, so there would be nothing to \
                 show. Pick another shard, or set this one up once its next war starts."
            )),
            ShardHealth::Unavailable(status) => Some(format!(
                "The Foxhole API returned `{status}` for shard **{name}**. It's likely down for \
                 the moment — try another shard, or try again shortly."
            )),
            ShardHealth::Unreachable(err) => Some(format!(
                "Couldn't reach shard **{name}** at all (`{err}`). Either the Foxhole API is \
                 unreachable from here, or that shard has gone away."
            )),
        }
    }
}

/// Fetches one shard's region list and caches it.
///
/// A list that comes back empty is **not** written over a list we already hold.
/// A shard between wars would otherwise erase a perfectly good list, and every
/// autocomplete for that shard goes blank until the war starts and the next
/// refresh runs. A stale list costs at most a few fetches that render as bare
/// terrain, which is a far cheaper mistake.
pub async fn refresh_maps(shard: Shard) -> ShardHealth {
    create_cache_dirs().await;

    let shard_name = shard.as_str();

    let resp = match http::client()
        .get(format!("{}/worldconquest/maps", shard.api_url()))
        .send()
        .await
    {
        Ok(resp) => resp,
        Err(err) => {
            log::warn!("could not reach shard {shard_name} for the map list: {err}");
            // The full chain (proxy, TLS, DNS) is noise in a Discord reply.
            return ShardHealth::Unreachable(short_reason(&err));
        }
    };

    if resp.status() != StatusCode::OK {
        log::warn!("shard {shard_name} returned {} for the map list", resp.status());
        return ShardHealth::Unavailable(resp.status());
    }

    let maps = match resp.json::<Maps>().await {
        Ok(maps) => maps,
        Err(err) => {
            log::warn!("could not decode the map list for shard {shard_name}: {err}");
            return ShardHealth::Unavailable(StatusCode::OK);
        }
    };

    if maps.is_empty() {
        log::info!("shard {shard_name} lists no regions; keeping whatever list we already have");
        return ShardHealth::NoRegions;
    }

    let count = maps.len();

    write_json(
        format!("./cache/map_choices/maps-{shard_name}.json"),
        &maps,
        "the map list",
    )
    .await;

    ShardHealth::Ready(count)
}

/// The short form of a transport failure, for a user-facing message.
///
/// `reqwest`'s `Display` is one line, but its `source` chain is where the
/// useful word lives — "dns error", "connection refused". The log gets the
/// whole thing; this is the last link, which is the one that names the fault.
fn short_reason(err: &reqwest::Error) -> String {
    if err.is_timeout() {
        return "timed out".to_string();
    }
    if err.is_connect() {
        return "could not connect".to_string();
    }

    let mut source: &dyn std::error::Error = err;

    while let Some(next) = source.source() {
        source = next;
    }

    source.to_string()
}

/// Refreshes the per-shard map list used by every autocomplete.
pub async fn save_maps_cache() {
    for shard in Shard::list_all() {
        match refresh_maps(shard).await {
            ShardHealth::Ready(regions) => {
                log::debug!("shard {} lists {regions} regions", shard.as_str())
            }
            health => log::warn!("shard {} is not usable right now: {health:?}", shard.as_str()),
        }
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
    save_dynamic_cache(dynamic_data, map_name, shard).await;
    save_static_cache(static_data, map_name, shard).await;
}

/// Writes one half on its own.
///
/// The frontline's neighbour fetch wants the dynamic half and nothing else —
/// it needs where the structures are, not what the towns are called — and the
/// two halves already live in separate files revalidated by separate ETags, so
/// writing one without the other is the existing model rather than a shortcut
/// around it.
pub async fn save_dynamic_cache(dynamic_data: &DynamicMapData, map_name: &str, shard: &str) {
    create_cache_dirs().await;

    write_json(
        format!("./cache/dynamic/Dynamic_{map_name}-{shard}.json"),
        dynamic_data,
        &format!("dynamic data for {map_name}"),
    )
    .await;
}

pub async fn save_static_cache(static_data: &StaticMapData, map_name: &str, shard: &str) {
    create_cache_dirs().await;

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
