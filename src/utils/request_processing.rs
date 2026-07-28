//! Per-region map compositing: background hex + dynamic icons + optional labels.
//!
//! All placement and sizing constants live in [`RenderConfig`] and are expressed as
//! ratios of the region footprint, so a region looks the same rendered standalone
//! (`/get-map`) or as one tile of the stitched full map — only its pixel offset
//! differs. See `specs/rendering-placement.md`.

use std::collections::HashSet;
use std::sync::{Mutex, OnceLock};

use ab_glyph::{FontRef, PxScale};
use image::{imageops::overlay, imageops::FilterType, ImageBuffer, Rgba};
use imageproc::drawing::{draw_text_mut, text_size};
use thiserror::Error;

use crate::utils::api_definitions::foxhole::{
    DynamicMapData, MapMarkerType, StaticMapData, TeamId,
};
use crate::utils::frontline::{Point, Polyline};

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
    /// Fill for every label the bot draws — region names on the full map, and
    /// the API's own town and field labels on a hex.
    ///
    /// Light fill over a dark halo, and one style for both, because the problem
    /// is the same in both places: the terrain runs from Acrithia's pale desert
    /// to Deadlands' near-black, and a label has to survive being drawn over an
    /// icon as well as over open ground.
    pub text_color: Rgba<u8>,
    /// Halo laid down under the fill. `None` gives the pre-rewrite flat-black
    /// text, which is unreadable the moment it crosses a dark icon — the reason
    /// hex labels looked like they were *behind* the icons rather than on top
    /// of them, which is where they have always actually been drawn.
    pub text_outline: Option<Rgba<u8>>,
    /// Gap between an icon and the label sitting under it, as a fraction of the
    /// region height.
    ///
    /// A town's name and its base icon arrive from the API on the *same*
    /// coordinate, and both were drawn centred on it — so the label landed
    /// squarely on top of the icon and hid it. Labels are drawn after icons, so
    /// what was lost was the icon: the map's actual data, covered by its own
    /// caption. The name now sits clear of the icon, which is the ordinary map
    /// convention and costs nothing, since the label was never legible on top
    /// of an icon anyway.
    pub label_icon_clearance_ratio: f32,
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
    /// Write each region's name across its hex on the full map.
    ///
    /// Only the full map, and nothing to do with [`place_image_info`]'s
    /// `draw_text`: that draws the API's in-region labels (town names, fields)
    /// onto a 1024 px tile, which the downscale reduces to a smudge. This is
    /// one name per hex, drawn on the finished image at a size chosen for it.
    pub full_map_region_labels: bool,
    /// Cap height for those names in the *finished* full-map PNG, in pixels.
    ///
    /// Absolute for the same reason [`RenderConfig::full_map_icon_px`] is:
    /// legibility is measured in screen pixels, and a ratio of the 63.6 MP
    /// composite means nothing once it has been scaled to a fifth of its size.
    pub full_map_label_px: f32,
    /// Widest a name may be, as a fraction of its hex's finished width. Longer
    /// names shrink to fit rather than run into the next hex.
    ///
    /// Colour is not repeated here: region names use
    /// [`RenderConfig::text_color`] and [`RenderConfig::text_outline`] like
    /// every other label, so there is one style to change rather than two that
    /// can drift apart.
    pub full_map_label_width_ratio: f32,
    /// Wash each hex in its controlling faction's colour. Off unless a guild
    /// opts in (`guilds.full_map_faction_tint`), and only ever set for the full
    /// map — see [`controlling_team`] for what "controlling" means.
    pub faction_tint: bool,
    pub colonial_tint: Rgba<u8>,
    pub warden_tint: Rgba<u8>,

    // --- frontline overlay ------------------------------------------------
    // `specs/active/frontline.md`. The model itself lives in `utils::frontline`
    // and knows nothing about canvases; everything here is about drawing what
    // it produces.
    /// Trace the contested boundary between the two factions. Off unless a
    /// guild opts in (`guilds.frontline`).
    pub frontline: bool,
    /// Field sample spacing, as a fraction of [`REGION_WIDTH`]. 1/32 is 32 px
    /// of world space.
    ///
    /// **Coarse is load-bearing, not just cheap.** Within about the saturation
    /// radius below, an isolated enemy structure really does flip the field, and
    /// what stops that becoming a closed loop around every lone outpost is that
    /// the flip is narrower than the gap between samples. Sampling finely would
    /// create the islands the model was chosen to avoid — see
    /// [`crate::utils::frontline::contour`].
    ///
    /// Measured at this value on the real 10240 x 6216 composite with 2000
    /// structures: 255 ms for the field, a quarter millisecond for the contour.
    /// The same spacing serves a single hex, so the line looks identical in
    /// both commands rather than being smoother in one of them.
    pub field_resolution_ratio: f32,
    /// Stroke width as a fraction of [`REGION_WIDTH`].
    pub frontline_width_ratio: f32,
    /// Stroke width wanted in the *finished* full map, in pixels.
    ///
    /// Sized backwards by [`RenderConfig::for_full_map`], exactly as
    /// [`RenderConfig::full_map_icon_px`] is, because a hairline applied to the
    /// 63.6 MP composite disappears in the downscale.
    pub full_map_frontline_px: f32,
    /// How far past a hex's own footprint the field is sampled, as a fraction
    /// of [`REGION_WIDTH`].
    ///
    /// The contour is traced across all of it and only then masked back to the
    /// silhouette. Terminating the polyline at the hex bounds instead leaves a
    /// stroke-width gap at both ends — the defect the reference render has.
    pub frontline_margin_ratio: f32,
    /// Distance at which a single structure's influence saturates, as a
    /// fraction of [`REGION_WIDTH`]; the `ε` of `w / (d² + ε)` is its square.
    ///
    /// Expressed as a length rather than as `ε` directly because a length is a
    /// thing you can look at a map and have an opinion about, where 2500 px² is
    /// not. It is also the knob that decides how close to a lone structure the
    /// field flips, so it and [`RenderConfig::field_resolution_ratio`] have to
    /// be moved together.
    pub influence_radius_ratio: f32,
    /// The `k` of `w / (d² + ε)^k` — how sharply a structure's pull falls off.
    ///
    /// This is the knob that decides whether the line bisects the ground between
    /// the two sides or merely separates them, and the first live war showed the
    /// original value of 1 doing the latter. See
    /// [`crate::utils::frontline::influence`] for the arithmetic; the short
    /// version is that at `k = 1` a cluster of `n` holds ground `sqrt(n)` times
    /// as far out as a lone base, so the denser side quietly took the middle.
    ///
    /// Raising it pushes toward the Voronoi model the spec rejects, so it moves
    /// together with [`RenderConfig::influence_radius_ratio`], which is what
    /// keeps a lone base from opening an island.
    pub influence_falloff: u32,
    pub frontline_color: Rgba<u8>,
    /// Wider under-stroke, laid down first.
    ///
    /// **Opaque on purpose.** Segments are stroked one at a time and overlap at
    /// every vertex, so a semi-transparent halo would blend twice there and
    /// bead visibly along the line. Opaque blending is idempotent.
    pub frontline_halo: Option<Rgba<u8>>,
    /// Halo width as a multiple of the line width.
    pub frontline_halo_ratio: f32,
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
            text_color: Rgba([255, 255, 255, 255]),
            text_outline: Some(Rgba([0, 0, 0, 220])),
            label_icon_clearance_ratio: 3.0 / REGION_HEIGHT as f32,
            resample: FilterType::Lanczos3,
            anchor: Anchor::Center,
            column_pitch_ratio: 3.0 / 4.0,
            row_pitch_ratio: 1.0,
            odd_column_offset_ratio: 1.0 / 2.0,
            full_map_long_edge: 2048,
            full_map_resample: FilterType::Triangle,
            full_map_icon_px: 12,
            full_map_region_labels: true,
            // 15 px on the 2048-wide render. A hex finishes about 205 px
            // across there, so this is a name spanning roughly two thirds of
            // its own hex — findable at a glance without becoming the map.
            full_map_label_px: 19.0,
            full_map_label_width_ratio: 0.82,
            faction_tint: false,
            // Faction greens and blues, muted. Saturated versions of these read
            // as UI chrome laid over the map rather than as the map's own
            // colour, which is the opposite of what a control wash is for.
            colonial_tint: Rgba([74, 106, 62, 255]),
            warden_tint: Rgba([58, 92, 142, 255]),
            faction_tint_strength: 0.5,
            frontline: false,
            field_resolution_ratio: 1.0 / 32.0,
            frontline_width_ratio: 5.0 / REGION_WIDTH as f32,
            full_map_frontline_px: 3.0,
            // Comfortably wider than the halo, so the contour is always traced
            // past wherever the stroke can reach.
            frontline_margin_ratio: 48.0 / REGION_WIDTH as f32,
            // 100 px, doubled from the 50 the synthetic renders were tuned at.
            // The two have to move together: the radius at which a lone
            // structure flips the field against `m` friendly neighbours `s`
            // away is `r² = (s² + ε)/m^(1/k) - ε`, so raising `k` widens the
            // island and raising `ε` closes it again. At k=2, m=10, s=200 the
            // old 50 gives r = 105 px — wide enough for the 32 px grid to
            // resolve, which is precisely the loop the model exists to avoid.
            // 100 px drives it back under the sampling.
            influence_radius_ratio: 100.0 / REGION_WIDTH as f32,
            influence_falloff: 2,
            // The label style, deliberately: light stroke over a dark halo. It
            // is the one combination already shown to survive both Acrithia's
            // pale desert and Deadlands' near-black, which is the same problem
            // a line crossing 53 regions has. Separate fields from
            // `text_color` all the same — the spec calls colour a knob, and a
            // line is not a label.
            frontline_color: Rgba([255, 255, 255, 255]),
            frontline_halo: Some(Rgba([0, 0, 0, 255])),
            frontline_halo_ratio: 2.0,
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

        let stroke_px = (self.full_map_frontline_px / scale.max(f32::EPSILON)).max(1.0);

        RenderConfig {
            icon_size_ratio: source_px.max(1.0) / REGION_WIDTH as f32,
            // Same trick, same reason. Unlike the region names, the line is
            // genuinely better off drawn here and scaled down with everything
            // else: what the downscale destroys is *internal* detail — the
            // counters and stroke gaps in a glyph — and a stroke has none, so
            // it survives the resample and picks up free antialiasing.
            frontline_width_ratio: stroke_px / REGION_WIDTH as f32,
            ..self.clone()
        }
    }

    /// Stroke width in pixels for a canvas of the given width.
    pub fn frontline_width(&self, canvas_width: u32) -> f32 {
        (canvas_width as f32 * self.frontline_width_ratio).max(1.0)
    }

    /// Field sample spacing, in world pixels.
    pub fn field_spacing(&self) -> f32 {
        (REGION_WIDTH as f32 * self.field_resolution_ratio).max(1.0)
    }

    /// The shape of the influence field: `ε` in world pixels squared, and `k`.
    pub fn influence_model(&self) -> crate::utils::frontline::Model {
        let radius = REGION_WIDTH as f32 * self.influence_radius_ratio;

        crate::utils::frontline::Model {
            epsilon: (radius * radius).max(1.0),
            falloff: self.influence_falloff,
        }
    }

    /// How far past a hex the field is sampled, in world pixels.
    pub fn frontline_margin(&self) -> f32 {
        REGION_WIDTH as f32 * self.frontline_margin_ratio
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
    frontline: &[Polyline],
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

    // After the tint, before the icons. The tint is a wash and would swallow
    // the line; the icons are the map's actual content, and covering them with
    // a line costs legibility for decoration.
    draw_frontline(&mut bg_img, frontline, config);

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

/// Strokes the frontline onto one region's canvas, halo first.
///
/// `lines` are in **this canvas's** pixels — the caller translates them out of
/// world space, because it is the only place that knows the grid. They are
/// expected to run past the canvas on both sides; that is the point, and the
/// clipping below is what turns it into a line that meets the silhouette
/// exactly instead of stopping a stroke-width short of it.
pub fn draw_frontline(
    canvas: &mut ImageBuffer<Rgba<u8>, Vec<u8>>,
    lines: &[Polyline],
    config: &RenderConfig,
) {
    if !config.frontline || lines.is_empty() {
        return;
    }

    let width = config.frontline_width(canvas.width());

    if let Some(halo) = config.frontline_halo {
        stroke(canvas, lines, width * config.frontline_halo_ratio, halo);
    }

    stroke(canvas, lines, width, config.frontline_color);
}

/// Paints polylines at a given width, masked by the canvas's own alpha.
///
/// Masking by alpha is the same rule [`tint_region`] follows and for the same
/// reason: region art is transparent in the corners so the tiles interlock, and
/// anything painted there shows up on the seams between hexes on the full map.
/// Alpha itself is never written.
///
/// Coverage comes from the distance to the segment rather than from a scanline,
/// which antialiases the edges for free and costs work proportional to the
/// line's length rather than to the canvas.
fn stroke(
    canvas: &mut ImageBuffer<Rgba<u8>, Vec<u8>>,
    lines: &[Polyline],
    width: f32,
    color: Rgba<u8>,
) {
    let radius = (width / 2.0).max(0.5);
    let canvas_w = canvas.width() as i64;
    let canvas_h = canvas.height() as i64;

    for line in lines {
        for pair in line.windows(2) {
            let (from, to) = (pair[0], pair[1]);

            // Clamped to the canvas rather than tested inside the loop, so a
            // segment belonging to a different hex costs nothing at all. On the
            // full map every tile is handed the whole world's contour, so most
            // segments are exactly that.
            let min_x = ((from.0.min(to.0) - radius - 1.0).floor() as i64).max(0);
            let max_x = ((from.0.max(to.0) + radius + 1.0).ceil() as i64).min(canvas_w - 1);
            let min_y = ((from.1.min(to.1) - radius - 1.0).floor() as i64).max(0);
            let max_y = ((from.1.max(to.1) + radius + 1.0).ceil() as i64).min(canvas_h - 1);

            if min_x > max_x || min_y > max_y {
                continue;
            }

            for y in min_y..=max_y {
                for x in min_x..=max_x {
                    let distance =
                        distance_to_segment((x as f32 + 0.5, y as f32 + 0.5), from, to);
                    let coverage = (radius + 0.5 - distance).clamp(0.0, 1.0);

                    if coverage <= 0.0 {
                        continue;
                    }

                    let pixel = canvas.get_pixel_mut(x as u32, y as u32);
                    let weight = coverage * (pixel.0[3] as f32 / 255.0)
                        * (color.0[3] as f32 / 255.0);

                    for channel in 0..3 {
                        let base = pixel.0[channel] as f32;
                        let target = color.0[channel] as f32;
                        pixel.0[channel] = (base + (target - base) * weight).round() as u8;
                    }
                }
            }
        }
    }
}

fn distance_to_segment(point: Point, from: Point, to: Point) -> f32 {
    let (dx, dy) = (to.0 - from.0, to.1 - from.1);
    let length_squared = dx * dx + dy * dy;

    // A zero-length segment is a point. Marching squares can emit one where a
    // crossing lands exactly on a grid corner.
    let t = if length_squared <= f32::EPSILON {
        0.0
    } else {
        (((point.0 - from.0) * dx + (point.1 - from.1) * dy) / length_squared).clamp(0.0, 1.0)
    };

    let nearest = (from.0 + t * dx, from.1 + t * dy);

    ((point.0 - nearest.0).powi(2) + (point.1 - nearest.1).powi(2)).sqrt()
}

/// Warns about a missing icon **once per icon type per process**.
///
/// The fact is about the asset set, not about this render: an icon type Foxhole
/// ships before we do is missing for every structure of that type, in every
/// region, on every render. Measured on one full-map render: 113 identical
/// warnings from two icon types, which is a real signal — art needs updating —
/// buried under its own repetition, and repeated on the console where warnings
/// are meant to be worth reading.
///
/// Per process rather than per render, deliberately. The answer doesn't change
/// until someone deploys new art, and that means a restart.
fn warn_missing_icon(path: &str) {
    static SEEN: OnceLock<Mutex<HashSet<String>>> = OnceLock::new();

    let seen = SEEN.get_or_init(|| Mutex::new(HashSet::new()));

    // A poisoned lock here means another thread panicked mid-insert. The set is
    // a de-duplication cache and nothing reads it for correctness, so recovering
    // the guard and carrying on is right — the alternative is taking a render
    // down over a logging detail.
    let mut seen = seen.lock().unwrap_or_else(|err| err.into_inner());

    if seen.insert(path.to_string()) {
        log::warn!(
            "no icon for {path}, using the debug icon — said once per icon type, \
             and a sign the art needs updating (scripts/update_assets.py)"
        );
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
            warn_missing_icon(&path);
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
    let icon_size = config.icon_size(canvas_w);

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

        // Down by half an icon plus half the text, so the label clears an icon
        // centred on the same point instead of covering it. Only meaningful
        // for a centred anchor: `Anchor::TopLeft` exists to reproduce the
        // pre-rewrite output, and moving its labels would defeat the one thing
        // it is for.
        let drop = match config.anchor {
            Anchor::Center => {
                (icon_size + text_h) as f32 / 2.0
                    + canvas_h as f32 * config.label_icon_clearance_ratio
            }
            Anchor::TopLeft => 0.0,
        };

        let (x, y) = (x as i32, y as i32 + drop.round() as i32);

        draw_haloed_text(canvas, &map_text.text, x, y, scale, font, config);
    }

    Ok(())
}

/// One region name to write on the finished full map.
///
/// Geometry is settled by the caller, which is the only place that knows the
/// grid and the downscale; this module knows fonts.
pub struct RegionLabel {
    pub text: String,
    /// Where the name is centered, in finished-image pixels.
    pub center: (f32, f32),
    /// Widest it may be, in the same pixels, before it has to shrink.
    pub max_width: f32,
}

/// Writes region names across the finished full map.
///
/// **After the downscale, never before.** Text drawn on a 1024 px tile and then
/// scaled to a fifth is unreadable — the reason `/full-map` has always passed
/// `draw_text: false` — and no amount of choosing a bigger size on the tile
/// fixes it, because the glyphs are resampled along with the terrain. Drawn
/// here the text is rendered once, at its final size, on its final pixels.
pub fn draw_region_labels(
    canvas: &mut ImageBuffer<Rgba<u8>, Vec<u8>>,
    labels: &[RegionLabel],
    config: &RenderConfig,
) -> Result<(), RenderError> {
    let font = font()?;

    for label in labels {
        // Measure at the wanted size, then shrink to fit. "Onyx" and "The Linn
        // of Mercy" are the same hex's worth of room, so a single size either
        // wastes most of it or runs the long names into the next region.
        let mut px = config.full_map_label_px;
        let (text_w, _) = text_size(PxScale { x: px, y: px }, font, &label.text);

        if text_w as f32 > label.max_width && text_w > 0 {
            px *= label.max_width / text_w as f32;
        }

        // Under this the glyphs are a texture rather than a word, and a
        // smaller one is not a smaller improvement — it is clutter.
        if px < 6.0 {
            continue;
        }

        let scale = PxScale { x: px, y: px };
        let (text_w, text_h) = text_size(scale, font, &label.text);

        let x = (label.center.0 - text_w as f32 / 2.0).round() as i32;
        let y = (label.center.1 - text_h as f32 / 2.0).round() as i32;

        draw_haloed_text(canvas, &label.text, x, y, scale, font, config);
    }

    Ok(())
}

/// Draws one string with its halo underneath it.
///
/// The halo is a full square ring, not the four cardinal offsets the labels
/// originally used: a four-neighbour outline leaves the diagonals bare, and a
/// glyph's thinnest strokes are exactly where it then breaks up — which is the
/// case that matters, because thin strokes over a busy icon are what goes
/// unreadable first.
///
/// Its thickness scales with the text. A fixed 1 px ring vanishes under 25 px
/// glyphs and swallows 8 px ones.
fn draw_haloed_text(
    canvas: &mut ImageBuffer<Rgba<u8>, Vec<u8>>,
    text: &str,
    x: i32,
    y: i32,
    scale: PxScale,
    font: &FontRef<'static>,
    config: &RenderConfig,
) {
    if let Some(outline) = config.text_outline {
        let weight = (scale.y / 12.0).round().max(1.0) as i32;

        for dy in -weight..=weight {
            for dx in -weight..=weight {
                if (dx, dy) != (0, 0) {
                    draw_text_mut(canvas, outline, x + dx, y + dy, scale, font, text);
                }
            }
        }
    }

    draw_text_mut(canvas, config.text_color, x, y, scale, font, text);
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

#[cfg(test)]
mod tests {
    use super::*;

    fn canvas(alpha: u8) -> ImageBuffer<Rgba<u8>, Vec<u8>> {
        ImageBuffer::from_pixel(64, 64, Rgba([128, 128, 128, alpha]))
    }

    /// Straight down the middle, running past the canvas at both ends the way
    /// a real contour is meant to.
    fn line() -> Vec<Polyline> {
        vec![vec![(32.0, -20.0), (32.0, 84.0)]]
    }

    /// Width is pinned rather than taken from the default, so these tests keep
    /// testing the drawing rules when the default is tuned by eye. It also
    /// keeps the centre of the stroke fully covered: at a 1 px width the halo
    /// blackens a pixel and the half-covered white line returns it to exactly
    /// its original grey, which is a real result and a useless probe.
    fn config() -> RenderConfig {
        RenderConfig {
            frontline: true,
            frontline_width_ratio: 6.0 / 64.0,
            ..RenderConfig::default()
        }
    }

    #[test]
    fn the_stroke_stays_out_of_the_transparent_corners() {
        let mut transparent = canvas(0);
        let before = transparent.clone();

        draw_frontline(&mut transparent, &line(), &config());

        // Region art is see-through in the corners so the tiles interlock;
        // anything painted there shows up on the seams of the full map.
        assert_eq!(transparent, before);
    }

    #[test]
    fn the_stroke_reaches_both_edges() {
        let mut opaque = canvas(255);

        draw_frontline(&mut opaque, &line(), &config());

        // A contour terminated at the canvas bounds stops a stroke-width short
        // of them. This one is traced past both, so the very first and last
        // rows are painted.
        for y in [0, 63] {
            assert_ne!(
                opaque.get_pixel(32, y).0[0],
                128,
                "nothing drawn on row {y} — the line stops short of the edge"
            );
        }
    }

    #[test]
    fn alpha_is_never_written() {
        let mut half = canvas(120);

        draw_frontline(&mut half, &line(), &config());

        assert!(half.pixels().all(|pixel| pixel.0[3] == 120));
    }

    #[test]
    fn off_draws_nothing() {
        let mut opaque = canvas(255);
        let before = opaque.clone();

        draw_frontline(&mut opaque, &line(), &RenderConfig::default());

        assert_eq!(opaque, before);
    }

    #[test]
    fn the_full_map_sizes_its_stroke_backwards() {
        let config = RenderConfig::default();
        let scale = 0.2;

        // What a tile is stroked at, once the composite has been scaled down,
        // is what was asked for in the finished image — the same rule the icons
        // follow, and the bug that once made the whole map look like bare
        // terrain when it was not applied.
        let finished = config.for_full_map(scale).frontline_width(REGION_WIDTH) * scale;

        assert!((finished - config.full_map_frontline_px).abs() < 0.5);
    }
}
