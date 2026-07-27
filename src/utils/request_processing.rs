//! Per-region map compositing: background hex + dynamic icons + optional labels.
//!
//! All placement and sizing constants live in [`RenderConfig`] and are expressed as
//! ratios of the region footprint, so a region looks the same rendered standalone
//! (`/get-map`) or as one tile of the stitched full map — only its pixel offset
//! differs. See `specs/rendering-placement.md`.

use std::sync::OnceLock;

use ab_glyph::{FontRef, PxScale};
use image::{imageops::overlay, imageops::FilterType, ImageBuffer, Rgba};
use imageproc::drawing::{draw_text_mut, text_size};
use thiserror::Error;

use crate::utils::api_definitions::foxhole::{
    DynamicMapData, MapMarkerType, StaticMapData, TeamId,
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
    ///
    /// Nothing constructs this: swapping it into `RenderConfig::default` by hand
    /// is the entire point. Keep it until the centered output has been reviewed
    /// against old renders, then it can go.
    #[allow(dead_code)]
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

    // --- full-map grid geometry -------------------------------------------
    // Ratios of the region footprint, not pixel literals, for the same reason
    // the sizes above are: the numbers only mean anything relative to a hex.
    // See `specs/full-map-renderer.md` for the derivation.
    /// Horizontal distance between adjacent columns, as a fraction of
    /// [`REGION_WIDTH`]. 3/4 is the flat-top hex packing: neighbouring columns
    /// overlap by a quarter of a hex, and the assets' transparent corners make
    /// the seam invisible.
    ///
    /// Measured, not guessed: the art's opaque footprint is a true flat-top
    /// hexagon with no padding, and two neighbours placed at this pitch cover
    /// their shared band with zero uncovered pixels.
    pub column_pitch_ratio: f32,
    /// Vertical distance between rows within a column, as a fraction of
    /// [`REGION_HEIGHT`]. Hexes in the same column stack edge to edge.
    pub row_pitch_ratio: f32,
    /// How far odd columns hang below even ones, as a fraction of
    /// [`REGION_HEIGHT`].
    pub odd_column_offset_ratio: f32,
    /// Long edge of the finished full-map PNG. The composite is ~63.6 MP, far
    /// past what Discord accepts as a PNG, so it is scaled uniformly — the
    /// per-region ratios above are never touched, or a hex would stop looking
    /// like its standalone render.
    pub full_map_long_edge: u32,
    /// Resampling for that one downscale. Deliberately cheaper than
    /// [`RenderConfig::resample`]: Lanczos3 over 63.6 MP costs seconds of CPU,
    /// and at 0.2x the difference from a triangle filter is not visible.
    pub full_map_resample: FilterType,
    /// Icon edge in the *finished* full-map PNG, in pixels.
    ///
    /// The one quantity here that is honestly absolute rather than a ratio: it
    /// is a legibility floor, and legibility is measured in screen pixels. The
    /// per-region [`RenderConfig::icon_size_ratio`] cannot serve the full map,
    /// because 24 px on a 1024 px tile survives the 0.2x downscale as under
    /// 5 px — which is why the first full render looked like bare terrain.
    /// [`RenderConfig::for_full_map`] sizes source icons backwards from this.
    pub full_map_icon_px: u32,
    /// Wash each hex in its controlling faction's colour. Off unless a guild
    /// opts in (`guilds.full_map_faction_tint`), and only ever set for the full
    /// map — see [`controlling_team`] for what "controlling" means.
    pub faction_tint: bool,
    pub colonial_tint: Rgba<u8>,
    pub warden_tint: Rgba<u8>,
    /// How far a tinted pixel moves toward the faction colour, 0.0 to 1.0.
    /// Low on purpose: the point is to read ownership at a glance without
    /// losing the terrain underneath it.
    pub faction_tint_strength: f32,
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
            column_pitch_ratio: 3.0 / 4.0,
            row_pitch_ratio: 1.0,
            odd_column_offset_ratio: 1.0 / 2.0,
            full_map_long_edge: 2048,
            full_map_resample: FilterType::Triangle,
            full_map_icon_px: 12,
            faction_tint: false,
            // Faction greens and blues, muted. Saturated versions of these read
            // as UI chrome laid over the map rather than as the map's own
            // colour, which is the opposite of what a control wash is for.
            colonial_tint: Rgba([74, 106, 62, 255]),
            warden_tint: Rgba([58, 92, 142, 255]),
            faction_tint_strength: 0.5,
        }
    }
}

impl RenderConfig {
    /// Icon edge length in pixels for a canvas of the given width.
    pub fn icon_size(&self, canvas_width: u32) -> u32 {
        ((canvas_width as f32 * self.icon_size_ratio).round() as u32).max(1)
    }

    /// Top-left pixel of the hex at grid `(col, row)` on the full-map canvas.
    ///
    /// Odd columns are pushed down half a hex — that half-step is what makes a
    /// rectangular `(col, row)` table describe a hex grid.
    pub fn grid_offset(&self, col: u32, row: u32) -> (u32, u32) {
        let x = col as f32 * REGION_WIDTH as f32 * self.column_pitch_ratio;
        let mut y = row as f32 * REGION_HEIGHT as f32 * self.row_pitch_ratio;

        if col % 2 == 1 {
            y += REGION_HEIGHT as f32 * self.odd_column_offset_ratio;
        }

        (x.round() as u32, y.round() as u32)
    }

    /// Scale factor that fits a canvas of `width` x `height` inside
    /// [`RenderConfig::full_map_long_edge`]. Never upscales.
    pub fn full_map_scale(&self, width: u32, height: u32) -> f32 {
        let long_edge = width.max(height) as f32;

        (self.full_map_long_edge as f32 / long_edge).min(1.0)
    }

    /// This config with its icons resized for tiles that are about to be
    /// scaled by `scale`.
    ///
    /// Icons are drawn on full-size 1024 px tiles and only shrink at the very
    /// end, so sizing them for the tile is sizing them for an image nobody
    /// sees. Dividing the target by the downscale is what makes
    /// [`RenderConfig::full_map_icon_px`] mean what it says, at whatever
    /// [`RenderConfig::full_map_long_edge`] happens to be.
    ///
    /// Only the icons change. The downscale itself is left alone deliberately —
    /// it is what keeps a hex's terrain looking like its standalone render.
    pub fn for_full_map(&self, scale: f32) -> RenderConfig {
        let source_px = (self.full_map_icon_px as f32 / scale.max(f32::EPSILON)).round();

        RenderConfig {
            icon_size_ratio: source_px.max(1.0) / REGION_WIDTH as f32,
            ..self.clone()
        }
    }

    /// The wash colour for a faction, or `None` for the neutral team — which
    /// has no colour by design, so an uncontested hex keeps its plain art.
    pub fn tint_color(&self, team: TeamId) -> Option<Rgba<u8>> {
        match team {
            TeamId::Colonials => Some(self.colonial_tint),
            TeamId::Wardens => Some(self.warden_tint),
            TeamId::None => None,
        }
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
    let mut bg_img = load_background(background_img_path)?;

    let canvas_w = bg_img.width();
    let canvas_h = bg_img.height();

    // Before the icons, never after: the icons are already faction-coloured,
    // and washing them in the same colour is how you make a map you can't read.
    if config.faction_tint {
        if let Some(color) = controlling_team(dynamic_data).and_then(|team| config.tint_color(team))
        {
            tint_region(&mut bg_img, color, config.faction_tint_strength);
        }
    }

    draw_icons(&mut bg_img, dynamic_data, config)?;

    if draw_text {
        draw_labels(&mut bg_img, static_data, config, canvas_w, canvas_h)?;
    }

    Ok(bg_img)
}

/// The API's `iconType`s that actually decide who holds a region: town bases
/// and relic bases.
///
/// Everything else a faction builds — refineries, storage, garrisons — says
/// where they have *been*, not what they hold, and counting it tints a hex for
/// whoever built more sheds in it. Relic bases 46 and 47 were retired in
/// Update 52 and simply never appear now; listing them costs nothing and means
/// the tint keeps working if they come back.
///
/// ids from <https://github.com/clapfoot/warapi> (MapIconType).
const CONTROL_ICON_TYPES: &[i64] = &[45, 46, 47, 56, 57, 58];

/// A per-faction count of what a region holds: bases first, everything else as
/// the tiebreak.
#[derive(Default)]
struct Holdings {
    bases: usize,
    structures: usize,
}

/// Which faction holds a region, by majority of its control structures.
///
/// An even split of bases is common — a hex with one base each is the normal
/// shape of a contested front — and leaving those untinted put a plain hole in
/// the middle of an otherwise tinted map, which reads as a rendering fault
/// rather than as "contested". So a tie on bases falls through to every other
/// structure the two sides hold there, which is the same question asked with
/// finer resolution: whoever has more built in a hex is the one actually
/// sitting in it.
///
/// `None` only when the two are equal on *both* counts — in practice a hex with
/// nothing in it at all, or one the API didn't report, where there is genuinely
/// nothing to colour it by.
fn controlling_team(dynamic_data: &DynamicMapData) -> Option<TeamId> {
    let mut colonial = Holdings::default();
    let mut warden = Holdings::default();

    for item in &dynamic_data.map_items {
        let side = match item.team_id {
            TeamId::Colonials => &mut colonial,
            TeamId::Wardens => &mut warden,
            TeamId::None => continue,
        };

        if CONTROL_ICON_TYPES.contains(&item.icon_type) {
            side.bases += 1;
        } else {
            side.structures += 1;
        }
    }

    match colonial
        .bases
        .cmp(&warden.bases)
        .then(colonial.structures.cmp(&warden.structures))
    {
        std::cmp::Ordering::Greater => Some(TeamId::Colonials),
        std::cmp::Ordering::Less => Some(TeamId::Wardens),
        std::cmp::Ordering::Equal => None,
    }
}

/// Blends every pixel toward `color`, scaled by its own alpha.
///
/// Scaling by alpha is what keeps the wash inside the hexagon: the art's corners
/// are transparent so the tiles can interlock, and tinting them flat would paint
/// the faction colour into the gaps between hexes and turn the seams visible.
/// Alpha itself is never touched.
fn tint_region(canvas: &mut ImageBuffer<Rgba<u8>, Vec<u8>>, color: Rgba<u8>, strength: f32) {
    let strength = strength.clamp(0.0, 1.0);

    for pixel in canvas.pixels_mut() {
        let weight = strength * (pixel.0[3] as f32 / 255.0);

        for channel in 0..3 {
            let base = pixel.0[channel] as f32;
            let target = color.0[channel] as f32;
            pixel.0[channel] = (base + (target - base) * weight).round() as u8;
        }
    }
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

/// Loads a region's background TGA on its own.
///
/// The full map calls this directly for a region whose data didn't arrive: the
/// hex is drawn as bare terrain rather than left as a hole in the grid.
pub fn load_background<P>(path: &P) -> Result<ImageBuffer<Rgba<u8>, Vec<u8>>, RenderError>
where
    P: AsRef<std::path::Path> + ?Sized,
{
    let path = path.as_ref();

    let source = match image::open(path) {
        Ok(img) => return Ok(img.to_rgba8()),
        Err(source) => source,
    };

    // The map assets' capitalisation has drifted before — this repo carries a
    // commit called "Rename MapDeadlandsHex.TGA to MapDeadLandsHex.TGA", and a
    // checkout on a case-insensitive filesystem can keep the older spelling on
    // disk while git reports the tree clean. On Linux that reads as a missing
    // file, and a missing background is a hole punched in the world map.
    //
    // So: on the failure path only (the common case costs no extra syscall),
    // look again ignoring case, and say so loudly enough to get it renamed.
    if let Some(actual) = find_ignoring_case(path) {
        log::warn!(
            "{} does not exist, but {} does — the asset is misnamed, opening it anyway",
            path.display(),
            actual.display()
        );

        return image::open(&actual)
            .map(|img| img.to_rgba8())
            .map_err(|source| RenderError::Background {
                path: actual.display().to_string(),
                source,
            });
    }

    Err(RenderError::Background {
        path: path.display().to_string(),
        source,
    })
}

/// The real name of a file that differs from `path` only in capitalisation.
fn find_ignoring_case(path: &std::path::Path) -> Option<std::path::PathBuf> {
    let wanted = path.file_name()?.to_str()?;

    std::fs::read_dir(path.parent()?)
        .ok()?
        .flatten()
        .find_map(|entry| {
            entry
                .file_name()
                .to_str()?
                .eq_ignore_ascii_case(wanted)
                .then(|| entry.path())
        })
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
