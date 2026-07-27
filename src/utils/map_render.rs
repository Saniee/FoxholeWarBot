//! Fetch-revalidate-render for a single region, shared by `/get-map` and the
//! scheduled report tick.
//!
//! Both used to carry their own near-identical copy of this logic, which is how
//! the same defects (C-1 shared output file, C-2 mismatched 304 handling, C-3
//! unwrapped network calls) ended up duplicated in each. One implementation now.

use std::io::Cursor;

use image::ImageFormat;
use reqwest::StatusCode;
use thiserror::Error;

use super::api_definitions::foxhole::{DynamicMapData, StaticMapData};
use super::cache::{load_dynamic_cache, load_static_cache, save_map_cache};
use super::request_processing::{place_image_info, RenderConfig, RenderError};

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

/// Fetches (revalidating against the on-disk cache) and renders one region.
pub async fn render_region(
    api_url: &str,
    shard_name: &str,
    map_name: &str,
    draw_text: bool,
    config: RenderConfig,
) -> Result<RenderedMap, MapError> {
    let client = reqwest::Client::new();

    let cached_dynamic = load_dynamic_cache(map_name, shard_name).await;
    let cached_static = load_static_cache(map_name, shard_name).await;

    let dynamic_response = conditional_get(
        &client,
        format!("{api_url}/worldconquest/maps/{map_name}/dynamic/public"),
        cached_dynamic.as_ref().map(|d| d.version),
    )
    .await?;
    let static_response = conditional_get(
        &client,
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

    let last_updated = dynamic_data.last_updated;
    let background = format!("./assets/Maps/Map{map_name}.TGA");

    // Compositing is CPU-bound and would otherwise stall the async runtime for
    // the duration of the render (QA L-8).
    let png = tokio::task::spawn_blocking(move || {
        let img = place_image_info(&dynamic_data, &static_data, draw_text, &background, &config)?;

        let mut buf = Vec::new();
        img.write_to(&mut Cursor::new(&mut buf), ImageFormat::Png)
            .map_err(RenderError::Encode)?;

        Ok::<Vec<u8>, RenderError>(buf)
    })
    .await
    .map_err(|_| MapError::TaskPanicked)??;

    Ok(RenderedMap { png, last_updated })
}
