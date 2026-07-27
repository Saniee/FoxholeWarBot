//! Fetch-revalidate-render, for one region or for the whole world map.
//!
//! `/get-map` and the scheduled report tick used to carry their own near-identical
//! copy of this logic, which is how the same defects (C-1 shared output file,
//! C-2 mismatched 304 handling, C-3 unwrapped network calls) ended up duplicated
//! in each. One implementation now — and the full map is the same path fanned
//! out across the 53 regions, sharing the fetch, the cache and the ETags rather
//! than growing a second copy of them.

use std::io::Cursor;
use std::sync::Arc;

use image::imageops::{overlay, resize};
use image::{ImageBuffer, ImageFormat, Rgba};
use reqwest::StatusCode;
use thiserror::Error;
use tokio::sync::Semaphore;
use tokio::task::JoinSet;

use super::api_definitions::foxhole::{DynamicMapData, StaticMapData};
use super::cache::{load_dynamic_cache, load_maps, load_static_cache, save_map_cache};
use super::db::Shard;
use super::regions::{self, Region, REGIONS};
use super::request_processing::{
    load_background, place_image_info, RenderConfig, RenderError, REGION_HEIGHT, REGION_WIDTH,
};

/// Region fetches in flight at once during a full-map render.
///
/// The War API publishes no rate limit — its one stated rule is to respect the
/// cache headers, which the ETag path already does. 8 is polite: it finishes 53
/// regions in a handful of round trips without ever looking like a flood.
const FETCH_CONCURRENCY: usize = 8;

/// Discord's attachment ceiling for an unboosted upload. The full map is scaled
/// to fit well under this, but the encoded size depends on how busy the map is,
/// so it's checked rather than assumed.
const ATTACHMENT_LIMIT: usize = 10 * 1024 * 1024;

pub struct RenderedMap {
    /// The encoded PNG, handed straight to Discord as an in-memory attachment.
    /// Nothing is written to disk, so concurrent renders can't overwrite each
    /// other's output the way the shared `render.png` did (QA C-1).
    pub png: Vec<u8>,
    pub last_updated: i64,
}

#[derive(Debug, Error)]
pub enum MapError {
    #[error("the Foxhole API is currently unavailable")]
    ApiUnavailable,
    #[error("could not reach the Foxhole API: {0}")]
    Request(reqwest::Error),
    #[error("the Foxhole API returned data we could not read: {0}")]
    Decode(reqwest::Error),
    #[error("the Foxhole API returned {0}")]
    Status(StatusCode),
    #[error("rendering failed: {0}")]
    Render(#[from] RenderError),
    #[error("the render task did not finish")]
    TaskPanicked,
    #[error("the render came out at {0} bytes, over Discord's upload limit")]
    TooLarge(usize),
}

async fn conditional_get(
    client: &reqwest::Client,
    url: String,
    version: Option<i64>,
) -> Result<reqwest::Response, MapError> {
    // The War API's `version` field doubles as its ETag; "0" means "we hold
    // nothing, send the body".
    let etag = format!("\"{}\"", version.unwrap_or(0));

    client
        .get(url)
        .header("If-None-Match", etag)
        .send()
        .await
        .map_err(MapError::Request)
}

/// Resolves one half of the map data: a fresh body, or the cached copy on 304.
///
/// Each half is revalidated against **its own** ETag and falls back to **its
/// own** cache. The old code required dynamic and static to agree before using
/// either, and unwrapped the cache in the mismatched case — a panic whenever one
/// endpoint had changed and the other had not (QA C-2).
async fn resolve<T: serde::de::DeserializeOwned>(
    response: reqwest::Response,
    cached: Option<T>,
) -> Result<(T, bool), MapError> {
    match response.status() {
        StatusCode::NOT_MODIFIED => match cached {
            Some(data) => Ok((data, false)),
            // 304 without a cached copy means our cache was deleted between the
            // read and the request. Report it instead of unwrapping a None.
            None => Err(MapError::Status(StatusCode::NOT_MODIFIED)),
        },
        StatusCode::OK => response
            .json::<T>()
            .await
            .map(|data| (data, true))
            .map_err(MapError::Decode),
        status => match cached {
            // A transient upstream error is not worth failing over when we hold
            // a usable copy.
            Some(data) => {
                log::warn!("the Foxhole API returned {status}, serving the cached copy");
                Ok((data, false))
            }
            None => Err(MapError::Status(status)),
        },
    }
}

/// Fetches both halves of one region, revalidating each against the on-disk
/// cache and writing back whatever came down fresh.
async fn fetch_region(
    client: &reqwest::Client,
    api_url: &str,
    shard_name: &str,
    map_name: &str,
) -> Result<(DynamicMapData, StaticMapData), MapError> {
    let cached_dynamic = load_dynamic_cache(map_name, shard_name).await;
    let cached_static = load_static_cache(map_name, shard_name).await;

    let dynamic_response = conditional_get(
        client,
        format!("{api_url}/worldconquest/maps/{map_name}/dynamic/public"),
        cached_dynamic.as_ref().map(|d| d.version),
    )
    .await?;
    let static_response = conditional_get(
        client,
        format!("{api_url}/worldconquest/maps/{map_name}/static"),
        cached_static.as_ref().map(|s| s.version),
    )
    .await?;

    if dynamic_response.status() == StatusCode::INTERNAL_SERVER_ERROR
        && static_response.status() == StatusCode::INTERNAL_SERVER_ERROR
    {
        return Err(MapError::ApiUnavailable);
    }

    let (dynamic_data, dynamic_is_fresh): (DynamicMapData, bool) =
        resolve(dynamic_response, cached_dynamic).await?;
    let (static_data, static_is_fresh): (StaticMapData, bool) =
        resolve(static_response, cached_static).await?;

    if dynamic_is_fresh || static_is_fresh {
        save_map_cache(&dynamic_data, &static_data, map_name, shard_name).await;
    }

    Ok((dynamic_data, static_data))
}

/// Fetches (revalidating against the on-disk cache) and renders one region.
pub async fn render_region(
    api_url: &str,
    shard_name: &str,
    map_name: &str,
    draw_text: bool,
    config: RenderConfig,
) -> Result<RenderedMap, MapError> {
    let client = reqwest::Client::new();

    let (dynamic_data, static_data) =
        fetch_region(&client, api_url, shard_name, map_name).await?;

    let last_updated = dynamic_data.last_updated;
    let background = background_path(map_name);

    // Compositing is CPU-bound and would otherwise stall the async runtime for
    // the duration of the render (QA L-8).
    let png = tokio::task::spawn_blocking(move || {
        let img = place_image_info(&dynamic_data, &static_data, draw_text, &background, &config)?;

        encode_png(&img)
    })
    .await
    .map_err(|_| MapError::TaskPanicked)??;

    Ok(RenderedMap { png, last_updated })
}

/// One hex of the full map: where it goes, and its data if the fetch succeeded.
type Tile = (&'static Region, Option<(DynamicMapData, StaticMapData)>);

/// Renders every region of a shard's world map onto one hex-grid canvas.
///
/// 53x the work of [`render_region`], which is the entire reason a *scheduled*
/// full map needs approval (`specs/active/premium-full-map.md`). The fetches run
/// concurrently but bounded; the compositing is one blocking task.
pub async fn render_full_map(
    api_url: &str,
    shard_name: &str,
    draw_text: bool,
    config: RenderConfig,
) -> Result<RenderedMap, MapError> {
    let client = reqwest::Client::new();
    let permits = Arc::new(Semaphore::new(FETCH_CONCURRENCY));
    let mut fetches: JoinSet<Tile> = JoinSet::new();

    for region in regions_on_shard(shard_name).await {
        let client = client.clone();
        let api_url = api_url.to_string();
        let shard_name = shard_name.to_string();
        let permits = Arc::clone(&permits);

        fetches.spawn(async move {
            // The semaphore is never closed, so the only error `acquire` can
            // report cannot happen here.
            let _permit = permits.acquire().await;

            match fetch_region(&client, &api_url, &shard_name, region.api_name).await {
                Ok(data) => (region, Some(data)),
                // One region failing is not the map failing: it is drawn as bare
                // terrain and the other 52 are unaffected.
                Err(err) => {
                    log::warn!(
                        "{}: {err} — drawing it as background only",
                        region.api_name
                    );
                    (region, None)
                }
            }
        });
    }

    let mut tiles: Vec<Tile> = Vec::with_capacity(REGIONS.len());
    let mut last_updated = 0;

    while let Some(joined) = fetches.join_next().await {
        let tile = joined.map_err(|_| MapError::TaskPanicked)?;

        if let Some((dynamic, _)) = &tile.1 {
            // The map is only as fresh as its stalest hex, but the user asked
            // "when was this data from", so report the newest we hold.
            last_updated = last_updated.max(dynamic.last_updated);
        }

        tiles.push(tile);
    }

    // A map where nothing resolved is 53 empty hexes — an outage dressed up as a
    // render. Say so instead.
    if tiles.iter().all(|(_, data)| data.is_none()) {
        return Err(MapError::ApiUnavailable);
    }

    let png = tokio::task::spawn_blocking(move || composite_full_map(tiles, draw_text, &config))
        .await
        .map_err(|_| MapError::TaskPanicked)??;

    if png.len() > ATTACHMENT_LIMIT {
        return Err(MapError::TooLarge(png.len()));
    }

    Ok(RenderedMap { png, last_updated })
}

/// Which grid regions to draw for this shard.
///
/// The cached `/worldconquest/maps` list wins when we have one: a region the API
/// doesn't list — a stub, or one Siege Camp pulled — shouldn't be drawn, and
/// that check is generic where a hardcoded name (the old `OriginHex` exclusion)
/// was not. On a cold start there is no cached list, and drawing nothing would
/// be a worse answer than drawing the table, so fall back to all 53.
async fn regions_on_shard(shard_name: &str) -> Vec<&'static Region> {
    let listed = load_maps(Shard::from_str(shard_name)).await;

    if listed.is_empty() {
        log::warn!("no cached map list for shard {shard_name}, rendering every known region");
        return REGIONS.iter().collect();
    }

    listed
        .iter()
        .filter_map(|api_name| match regions::find(api_name) {
            Some(region) => Some(region),
            None => {
                // A region with no grid coordinates has nowhere to go. It stays
                // out of the full map until the table in `regions.rs` learns it.
                log::warn!("shard {shard_name} lists {api_name}, which has no grid position");
                None
            }
        })
        .collect()
}

/// Draws every tile at its grid offset, then scales the composite down to
/// something Discord will accept.
///
/// CPU-bound and synchronous — callers on the async runtime must wrap this in
/// `spawn_blocking` (QA L-8), which matters far more at 63.6 MP than it did for
/// a single hex.
fn composite_full_map(
    tiles: Vec<Tile>,
    draw_text: bool,
    config: &RenderConfig,
) -> Result<Vec<u8>, RenderError> {
    let (canvas_w, canvas_h) = canvas_size(&tiles, config);
    let mut canvas = ImageBuffer::from_pixel(canvas_w, canvas_h, Rgba([0, 0, 0, 0]));

    for (region, data) in tiles {
        let background = background_path(region.api_name);

        let hex = match data {
            Some((dynamic, statics)) => {
                place_image_info(&dynamic, &statics, draw_text, &background, config)
            }
            None => load_background(&background),
        };

        match hex {
            Ok(hex) => {
                let (x, y) = config.grid_offset(region.col, region.row);
                overlay(&mut canvas, &hex, x as i64, y as i64);
            }
            // Missing art for one region shouldn't cost the other 52 either.
            Err(err) => log::warn!("skipping {}: {err}", region.api_name),
        }
    }

    let scale = config.full_map_scale(canvas_w, canvas_h);

    let scaled = if scale < 1.0 {
        let width = ((canvas_w as f32 * scale).round() as u32).max(1);
        let height = ((canvas_h as f32 * scale).round() as u32).max(1);

        resize(&canvas, width, height, config.full_map_resample)
    } else {
        canvas
    };

    encode_png(&scaled)
}

/// The canvas the placed tiles need, derived from the tiles themselves rather
/// than hardcoded — a region added to the grid table resizes the map by itself.
fn canvas_size(tiles: &[Tile], config: &RenderConfig) -> (u32, u32) {
    tiles
        .iter()
        .map(|(region, _)| config.grid_offset(region.col, region.row))
        .fold((0, 0), |(w, h), (x, y)| {
            (w.max(x + REGION_WIDTH), h.max(y + REGION_HEIGHT))
        })
}

fn background_path(map_name: &str) -> String {
    format!("./assets/Maps/Map{map_name}.TGA")
}

fn encode_png(img: &ImageBuffer<Rgba<u8>, Vec<u8>>) -> Result<Vec<u8>, RenderError> {
    let mut buf = Vec::new();

    img.write_to(&mut Cursor::new(&mut buf), ImageFormat::Png)
        .map_err(RenderError::Encode)?;

    Ok(buf)
}
