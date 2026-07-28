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
use super::cache::{
    load_dynamic_cache, load_maps, load_static_cache, save_dynamic_cache, save_map_cache,
};
use super::db::Shard;
use super::http;
use super::regions::{self, Region, REGIONS};
use super::frontline::{self, Edge};
use super::request_processing::{
    draw_frontline, draw_region_labels, load_background, place_image_info, RegionLabel,
    RenderConfig, RenderError, REGION_HEIGHT, REGION_WIDTH,
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

/// Fetches just the dynamic half of a region.
///
/// The frontline needs where the structures are, not what the towns are called,
/// and the two halves have always been revalidated against separate ETags and
/// separate cache files — so asking for one is the existing model, not a
/// shortcut around it. Halves the request count of the neighbour fan-out.
async fn fetch_dynamic(
    client: &reqwest::Client,
    api_url: &str,
    shard_name: &str,
    map_name: &str,
) -> Result<DynamicMapData, MapError> {
    let cached = load_dynamic_cache(map_name, shard_name).await;

    let response = conditional_get(
        client,
        format!("{api_url}/worldconquest/maps/{map_name}/dynamic/public"),
        cached.as_ref().map(|d| d.version),
    )
    .await?;

    let (data, is_fresh): (DynamicMapData, bool) = resolve(response, cached).await?;

    if is_fresh {
        save_dynamic_cache(&data, map_name, shard_name).await;
    }

    Ok(data)
}

/// The frontline through one region, in that region's own pixels.
///
/// **Fetches the up-to-six adjacent regions**, because a single hex's line is
/// wrong at its own edges without them: the field outside the hex is what the
/// contour needs in order to reach the silhouette at all, and there is none
/// without the neighbours' structures. A neighbour that fails is skipped with a
/// warning — the line then degrades near that one edge exactly as it would have
/// without the feature, which is strictly better than refusing to render.
///
/// Empty whenever there is nothing to draw, which includes the ordinary case of
/// a hex where only one faction holds anything.
async fn region_frontline(
    client: &reqwest::Client,
    api_url: &str,
    shard_name: &str,
    map_name: &str,
    own_data: &DynamicMapData,
    config: &RenderConfig,
) -> Vec<Edge> {
    if !config.frontline {
        return Vec::new();
    }

    let Some(region) = regions::find(map_name) else {
        log::warn!("{map_name} has no grid position, so it gets no frontline");
        return Vec::new();
    };

    let mut sources = frontline::sources_in(region, own_data, config);

    // The API's spelling for the API's URLs, the table's for everything else —
    // conflating the two is what lost Marban Hollow. An empty list means the
    // cache is cold rather than that the war has no regions.
    let listed = load_maps(Shard::from_str(shard_name)).await;
    let live = (!listed.is_empty()).then_some(listed);

    let mut fetches = JoinSet::new();

    for neighbour in regions::neighbours(region) {
        let Some(fetch_name) = live_name(&live, neighbour) else {
            continue;
        };

        let fetch_name = fetch_name.to_string();
        let client = client.clone();
        let api_url = api_url.to_string();
        let shard_name = shard_name.to_string();

        fetches.spawn(async move {
            (
                neighbour,
                fetch_dynamic(&client, &api_url, &shard_name, &fetch_name).await,
            )
        });
    }

    while let Some(joined) = fetches.join_next().await {
        match joined {
            Ok((neighbour, Ok(dynamic))) => {
                sources.extend(frontline::sources_in(neighbour, &dynamic, config));
            }
            Ok((neighbour, Err(err))) => log::warn!(
                "frontline: no data for {}, so the line may drift at that edge: {err}",
                neighbour.api_name
            ),
            Err(_) => log::warn!("frontline: a neighbour fetch panicked, skipping it"),
        }
    }

    let bounds = frontline::Bounds::around_region(region, config, config.frontline_margin());

    // After the neighbours are in, never per region: a town sitting on a hex
    // boundary has its structures split across two API responses, and
    // clustering each response on its own would leave it as two footings on
    // exactly the ground where the line is most sensitive.
    let sources = frontline::footings(&sources, config.footing_radius());

    let Some(field) = frontline::Field::sample(
        &sources,
        bounds,
        config.field_spacing(),
        config.influence_model(),
    ) else {
        log::debug!("no frontline in {map_name}: only one faction holds anything nearby");
        return Vec::new();
    };

    let lines = frontline::smooth(frontline::contour(&field), frontline::SMOOTHING_ROUNDS);
    let edges = frontline::flanks(
        lines,
        &sources,
        config.influence_model(),
        config.field_spacing(),
    );
    let (origin_x, origin_y) = config.grid_offset(region.col, region.row);

    translate(&edges, origin_x, origin_y)
}

/// Fetches (revalidating against the on-disk cache) and renders one region.
pub async fn render_region(
    api_url: &str,
    shard_name: &str,
    map_name: &str,
    draw_text: bool,
    config: RenderConfig,
) -> Result<RenderedMap, MapError> {
    let client = http::client().clone();

    let (dynamic_data, static_data) =
        fetch_region(&client, api_url, shard_name, map_name).await?;

    let last_updated = dynamic_data.last_updated;
    let background = background_path(map_name);

    let frontline = region_frontline(
        &client,
        api_url,
        shard_name,
        map_name,
        &dynamic_data,
        &config,
    )
    .await;

    // Compositing is CPU-bound and would otherwise stall the async runtime for
    // the duration of the render (QA L-8).
    let png = tokio::task::spawn_blocking(move || {
        let img = place_image_info(
            &dynamic_data,
            &static_data,
            draw_text,
            &background,
            &frontline,
            &config,
        )?;

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
/// full map needs approval (`specs/premium-full-map.md`). The fetches run
/// concurrently but bounded; the compositing is one blocking task.
pub async fn render_full_map(
    api_url: &str,
    shard_name: &str,
    draw_text: bool,
    config: RenderConfig,
) -> Result<RenderedMap, MapError> {
    let client = http::client().clone();
    let permits = Arc::new(Semaphore::new(FETCH_CONCURRENCY));
    let mut fetches: JoinSet<Tile> = JoinSet::new();
    let mut tiles: Vec<Tile> = Vec::with_capacity(REGIONS.len());

    let live = live_regions(shard_name).await;

    for region in REGIONS {
        // Every region in the table is drawn, always. Whether the API lists it
        // only decides whether we go looking for icons to put on it — a region
        // we can't fetch is bare terrain, never a hole in the world.
        let Some(fetch_name) = live_name(&live, region) else {
            log::info!(
                "shard {shard_name} doesn't list {}, drawing it as background only",
                region.api_name
            );
            tiles.push((region, None));
            continue;
        };

        // The API's spelling for the API's URLs and cache keys; the table's for
        // assets and placement. Conflating the two is what lost Marban Hollow.
        let fetch_name = fetch_name.to_string();
        let client = client.clone();
        let api_url = api_url.to_string();
        let shard_name = shard_name.to_string();
        let permits = Arc::clone(&permits);

        fetches.spawn(async move {
            // The semaphore is never closed, so the only error `acquire` can
            // report cannot happen here.
            let _permit = permits.acquire().await;

            match fetch_region(&client, &api_url, &shard_name, &fetch_name).await {
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

/// The regions the API says are in this shard's war, if we have a cached list.
///
/// `None` on a cold start, before the first map refresh has run — which is a
/// different thing from "the war has no regions" and must not be read as one.
async fn live_regions(shard_name: &str) -> Option<Vec<String>> {
    let listed = load_maps(Shard::from_str(shard_name)).await;

    if listed.is_empty() {
        log::warn!("no cached map list for shard {shard_name}, fetching every known region");
        return None;
    }

    for api_name in &listed {
        if regions::find(api_name).is_none() {
            // Nothing to be done at render time — a region with no grid
            // coordinates has nowhere to go — but it means Siege Camp shipped a
            // region and `regions.rs` hasn't learned it yet.
            log::warn!("shard {shard_name} lists {api_name}, which has no grid position");
        }
    }

    Some(listed)
}

/// What the API calls this region, or `None` if it isn't in play.
///
/// Matched leniently: the API spells Marban Hollow `MarbanHollow` while its
/// assets and this table say `MarbanHollowHex`, and an exact comparison quietly
/// drops the region on that alone. The API's own spelling comes back out,
/// because that is what its URLs answer to.
fn live_name<'a>(live: &'a Option<Vec<String>>, region: &'static Region) -> Option<&'a str> {
    match live {
        Some(live) => live
            .iter()
            .find(|api_name| regions::same_region(api_name, region.api_name))
            .map(String::as_str),
        // No cached list to disagree with; the table's spelling is the best
        // guess we have.
        None => Some(region.api_name),
    }
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

    // Known before the first tile is drawn, because the icons have to be sized
    // for the image that comes out the far side of it.
    let scale = config.full_map_scale(canvas_w, canvas_h);
    let tile_config = config.for_full_map(scale);

    // Taken before the loop consumes the tiles. Every region gets a name,
    // including one drawn as bare terrain — a hex whose data didn't arrive is
    // the one a user most needs identified.
    let labels = region_labels(&tiles, scale, config);

    // One field over the whole world, traced once. Per-region would break the
    // line at every seam, because a region's front depends on the bases in the
    // regions beside it.
    let frontline = world_frontline(&tiles, canvas_w, canvas_h, config);

    for (region, data) in tiles {
        let background = background_path(region.api_name);
        let (x, y) = config.grid_offset(region.col, region.row);

        // Every tile is handed the whole contour, shifted into its own pixels.
        // `stroke` clips per segment, so the 52 regions' worth that miss this
        // hex cost nothing — and the ones that overhang it are exactly what
        // carries the line to the silhouette instead of stopping short.
        let local = translate(&frontline, x, y);

        let hex = match data {
            Some((dynamic, statics)) => place_image_info(
                &dynamic,
                &statics,
                draw_text,
                &background,
                &local,
                &tile_config,
            ),
            // No icons to sit under here, so the order the overlay cares about
            // is moot; the alpha mask still keeps it inside the hexagon.
            None => load_background(&background).map(|mut bare| {
                draw_frontline(&mut bare, &local, &tile_config);
                bare
            }),
        };

        match hex {
            Ok(hex) => {
                overlay(&mut canvas, &hex, x as i64, y as i64);
            }
            // Missing art for one region shouldn't cost the other 52 either.
            Err(err) => log::warn!("skipping {}: {err}", region.api_name),
        }
    }

    let mut scaled = if scale < 1.0 {
        let width = ((canvas_w as f32 * scale).round() as u32).max(1);
        let height = ((canvas_h as f32 * scale).round() as u32).max(1);

        resize(&canvas, width, height, config.full_map_resample)
    } else {
        canvas
    };

    // The last thing to touch the image, deliberately. Names are the layer the
    // user reads the map *by*, so nothing is drawn over them and nothing
    // resamples them.
    if config.full_map_region_labels {
        draw_region_labels(&mut scaled, &labels, config)?;
    }

    encode_png(&scaled)
}

/// The frontline across the whole world, in world pixels.
///
/// Empty whenever there is nothing to draw: the feature is off, or the field
/// has no zero crossing because one side holds everything the API told us
/// about. Both are ordinary states, not failures.
fn world_frontline(
    tiles: &[Tile],
    canvas_w: u32,
    canvas_h: u32,
    config: &RenderConfig,
) -> Vec<Edge> {
    if !config.frontline {
        return Vec::new();
    }

    let sources: Vec<_> = tiles
        .iter()
        .filter_map(|(region, data)| data.as_ref().map(|(dynamic, _)| (region, dynamic)))
        .flat_map(|(region, dynamic)| frontline::sources_in(region, dynamic, config))
        .collect();

    // Across the whole world, not per tile, for the reason `region_frontline`
    // gives: a town on a hex boundary is split between two responses.
    let sources = frontline::footings(&sources, config.footing_radius());

    // Past the canvas on every side, for the same reason a single hex is
    // oversampled: a contour that stops at the edge stops a stroke short of it.
    let margin = config.frontline_margin();
    let bounds = frontline::Bounds {
        min_x: -margin,
        min_y: -margin,
        max_x: canvas_w as f32 + margin,
        max_y: canvas_h as f32 + margin,
    };

    let Some(field) = frontline::Field::sample(
        &sources,
        bounds,
        config.field_spacing(),
        config.influence_model(),
    ) else {
        log::debug!("no frontline to draw: only one faction holds anything on this map");
        return Vec::new();
    };

    let lines = frontline::smooth(frontline::contour(&field), frontline::SMOOTHING_ROUNDS);

    frontline::flanks(
        lines,
        &sources,
        config.influence_model(),
        config.field_spacing(),
    )
}

/// The same edges, moved from world space into a tile's own pixels.
///
/// A translation and nothing else, which is what lets the sides survive it: the
/// flank each segment carries is a handedness, and a shift preserves it where a
/// mirror would silently swap the two factions over.
fn translate(edges: &[Edge], origin_x: u32, origin_y: u32) -> Vec<Edge> {
    edges
        .iter()
        .map(|edge| Edge {
            points: edge
                .points
                .iter()
                .map(|(x, y)| (x - origin_x as f32, y - origin_y as f32))
                .collect(),
            colonial_side: edge.colonial_side.clone(),
        })
        .collect()
}

/// Where each region's name goes on the finished image, and how much room it
/// has.
///
/// A hex is at its widest across its own vertical centre, and that is also the
/// one height at which the neighbouring columns' art does not reach it: their
/// centres sit half a hex above and below, so at this line they are at their
/// own flat top or bottom edge, which spans only the middle half of their
/// width. The two footprints meet exactly and never overlap — which is what
/// makes a centred name safe at most of the hex's width.
fn region_labels(tiles: &[Tile], scale: f32, config: &RenderConfig) -> Vec<RegionLabel> {
    tiles
        .iter()
        .map(|(region, _)| {
            let (x, y) = config.grid_offset(region.col, region.row);

            RegionLabel {
                text: region.display_name.to_string(),
                center: (
                    (x as f32 + REGION_WIDTH as f32 / 2.0) * scale,
                    (y as f32 + REGION_HEIGHT as f32 / 2.0) * scale,
                ),
                max_width: REGION_WIDTH as f32 * scale * config.full_map_label_width_ratio,
            }
        })
        .collect()
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

/// The background asset for a region, named the way the *assets* spell it.
///
/// Never the way the caller spelled it: the API asks for `MarbanHollow`, and
/// `MapMarbanHollow.TGA` used to exist alongside `MapMarbanHollowHex.TGA` as a
/// stale leftover of an older art drop — `/get-map` was quietly rendering
/// year-old terrain off it. That file is gone, but the routing is what keeps
/// the next one from mattering: only names in the table can address art.
fn background_path(map_name: &str) -> String {
    format!("./assets/Maps/Map{}.TGA", regions::asset_name(map_name))
}

fn encode_png(img: &ImageBuffer<Rgba<u8>, Vec<u8>>) -> Result<Vec<u8>, RenderError> {
    let mut buf = Vec::new();

    img.write_to(&mut Cursor::new(&mut buf), ImageFormat::Png)
        .map_err(RenderError::Encode)?;

    Ok(buf)
}
