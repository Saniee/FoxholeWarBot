//! Per-region map compositing: background hex + dynamic icons + optional labels.
//!
//! All placement and sizing constants live in [`RenderConfig`] and are expressed as
//! ratios of the region footprint, so a region looks the same rendered standalone
//! (`/get-map`) or as one tile of the stitched full map — only its pixel offset
//! differs. See `specs/active/rendering-placement.md`.

use std::sync::OnceLock;

use ab_glyph::{FontRef, PxScale};
use image::{imageops::overlay, imageops::FilterType, ImageBuffer, Rgba};
use imageproc::drawing::{draw_text_mut, text_size};
use thiserror::Error;

use crate::utils::api_definitions::foxhole::{
    DynamicMapData, MapMarkerType, StaticMapData,
};

/// Every `assets/Maps/Map*Hex.TGA` is 1024 x 888 (a regular flat-top hexagon:
/// 1024/888 = 1.153 ~= 2/sqrt(3)).
pub const REGION_WIDTH: u32 = 1024;
pub const REGION_HEIGHT: u32 = 888;

#[derive(Debug, Error)]
pub enum RenderError {
    #[error("could not load the background image at {path}: {source}")]
    Background {
        path: String,
        #[source]
        source: image::ImageError,
    },
    #[error("could not load the icon at {path}: {source}")]
    Icon {
        path: String,
        #[source]
        source: image::ImageError,
    },
    #[error("the bundled font could not be parsed")]
    Font,
    #[error("could not encode the render as PNG: {0}")]
    Encode(#[from] image::ImageError),
}

/// Where a drawn object sits relative to the API coordinate it belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Anchor {
    /// The object is centered on its coordinate. This is the correct reading of
    /// the API's normalized points and the default.
    Center,
    /// The coordinate is the object's top-left corner — reproduces the
    /// pre-rewrite output exactly, kept only as an A/B comparison escape hatch.
    TopLeft,
}

/// Named replacements for the placement literals that used to be inline.
/// [`RenderConfig::default`] reproduces the pre-rewrite *sizes* at 1024 x 888
/// (24 px icons, 25 px labels); the anchor is the one deliberate visual change.
#[derive(Debug, Clone)]
pub struct RenderConfig {
    /// Rendered icon edge as a fraction of the region width. 24/1024.
    ///
    /// Derived from the region, not from the icon: the old `icon.width() * 0.5`
    /// was a fixed 24 px target that only looked like a scale because every icon
    /// happens to be 48 x 48.
    pub icon_size_ratio: f32,
    /// Major label height as a fraction of the region height. 25/888.
    pub major_text_ratio: f32,
    /// Minor label height as a fraction of the region height. Smaller than major
    /// so the two marker types are finally distinguishable — the API has always
    /// told us which is which, the old renderer just ignored it.
    pub minor_text_ratio: f32,
    pub text_color: Rgba<u8>,
    /// 1 px outline for legibility over dark terrain. `None` preserves the
    /// pre-rewrite flat-black look.
    pub text_outline: Option<Rgba<u8>>,
    pub resample: FilterType,
    pub anchor: Anchor,
}

impl Default for RenderConfig {
    fn default() -> Self {
        RenderConfig {
            icon_size_ratio: 24.0 / REGION_WIDTH as f32,
            major_text_ratio: 25.0 / REGION_HEIGHT as f32,
            minor_text_ratio: 20.0 / REGION_HEIGHT as f32,
            text_color: Rgba([0, 0, 0, 255]),
            text_outline: None,
            resample: FilterType::Lanczos3,
            anchor: Anchor::Center,
        }
    }
}

impl RenderConfig {
    /// Icon edge length in pixels for a canvas of the given width.
    pub fn icon_size(&self, canvas_width: u32) -> u32 {
        ((canvas_width as f32 * self.icon_size_ratio).round() as u32).max(1)
    }

    pub fn text_size(&self, canvas_height: u32, marker: &MapMarkerType) -> f32 {
        let ratio = match marker {
            MapMarkerType::Major => self.major_text_ratio,
            MapMarkerType::Minor => self.minor_text_ratio,
        };
        (canvas_height as f32 * ratio).max(1.0)
    }
}

/// Converts a normalized API coordinate plus the drawn object's size into a
/// pixel top-left, honoring the anchor.
pub fn place(
    norm_x: f64,
    norm_y: f64,
    object_w: u32,
    object_h: u32,
    canvas_w: u32,
    canvas_h: u32,
    anchor: Anchor,
) -> (i64, i64) {
    let x = norm_x * canvas_w as f64;
    let y = norm_y * canvas_h as f64;

    match anchor {
        Anchor::TopLeft => (x as i64, y as i64),
        Anchor::Center => (
            (x - object_w as f64 / 2.0) as i64,
            (y - object_h as f64 / 2.0) as i64,
        ),
    }
}

fn font() -> Result<&'static FontRef<'static>, RenderError> {
    static FONT: OnceLock<Option<FontRef<'static>>> = OnceLock::new();

    FONT.get_or_init(|| {
        FontRef::try_from_slice(include_bytes!("../../assets/Inter-Bold.ttf") as &[u8]).ok()
    })
    .as_ref()
    .ok_or(RenderError::Font)
}

/// Composites one region: its background TGA, then dynamic map icons, then
/// (optionally) static text labels.
///
/// CPU-bound and synchronous — callers on the async runtime must wrap this in
/// `spawn_blocking` (QA L-8).
pub fn place_image_info<P>(
    dynamic_data: &DynamicMapData,
    static_data: &StaticMapData,
    draw_text: bool,
    background_img_path: &P,
    config: &RenderConfig,
) -> Result<ImageBuffer<Rgba<u8>, Vec<u8>>, RenderError>
where
    P: AsRef<std::path::Path>,
{
    let path_display = background_img_path.as_ref().display().to_string();
    let mut bg_img = image::open(background_img_path)
        .map_err(|source| RenderError::Background {
            path: path_display,
            source,
        })?
        .to_rgba8();

    let canvas_w = bg_img.width();
    let canvas_h = bg_img.height();

    draw_icons(&mut bg_img, dynamic_data, config)?;

    if draw_text {
        draw_labels(&mut bg_img, static_data, config, canvas_w, canvas_h)?;
    }

    Ok(bg_img)
}

fn draw_icons(
    canvas: &mut ImageBuffer<Rgba<u8>, Vec<u8>>,
    dynamic_data: &DynamicMapData,
    config: &RenderConfig,
) -> Result<(), RenderError> {
    let canvas_w = canvas.width();
    let canvas_h = canvas.height();
    let icon_size = config.icon_size(canvas_w);

    for item in &dynamic_data.map_items {
        let path = format!(
            "./assets/MapIcons/{}{:?}.png",
            item.icon_type, item.team_id
        );

        // A missing icon is expected whenever Foxhole ships a new structure type
        // before we ship its art; fall back rather than failing the whole render.
        let icon_path = if std::path::Path::new(&path).exists() {
            path
        } else {
            log::warn!("no icon for {path}, using the debug icon");
            "./assets/MapIcons/DebugIcon.png".to_string()
        };

        let icon = match load_img(&icon_path) {
            Ok(i) => i,
            Err(err) => {
                log::warn!("skipping icon {icon_path}: {err}");
                continue;
            }
        };

        let icon = image::imageops::resize(&icon, icon_size, icon_size, config.resample);
        let (x, y) = place(
            item.x,
            item.y,
            icon_size,
            icon_size,
            canvas_w,
            canvas_h,
            config.anchor,
        );

        overlay(canvas, &icon, x, y);
    }

    Ok(())
}

fn draw_labels(
    canvas: &mut ImageBuffer<Rgba<u8>, Vec<u8>>,
    static_data: &StaticMapData,
    config: &RenderConfig,
    canvas_w: u32,
    canvas_h: u32,
) -> Result<(), RenderError> {
    let font = font()?;

    for map_text in &static_data.map_text_items {
        let px = config.text_size(canvas_h, &map_text.map_marker_type);
        let scale = PxScale { x: px, y: px };

        let (text_w, text_h) = text_size(scale, font, &map_text.text);
        let (x, y) = place(
            map_text.x,
            map_text.y,
            text_w,
            text_h,
            canvas_w,
            canvas_h,
            config.anchor,
        );
        let (x, y) = (x as i32, y as i32);

        if let Some(outline) = config.text_outline {
            for (dx, dy) in [(-1, 0), (1, 0), (0, -1), (0, 1)] {
                draw_text_mut(canvas, outline, x + dx, y + dy, scale, font, &map_text.text);
            }
        }

        draw_text_mut(canvas, config.text_color, x, y, scale, font, &map_text.text);
    }

    Ok(())
}

pub fn load_img<P>(path: &P) -> Result<ImageBuffer<Rgba<u8>, Vec<u8>>, RenderError>
where
    P: AsRef<std::path::Path> + ?Sized,
{
    image::open(path)
        .map(|img| img.to_rgba8())
        .map_err(|source| RenderError::Icon {
            path: path.as_ref().display().to_string(),
            source,
        })
}
