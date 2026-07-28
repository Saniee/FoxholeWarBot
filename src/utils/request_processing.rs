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
use crate::utils::frontline::{Edge, Field, Point};

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
    /// Wash the ground in each faction's colour. Off unless a guild opts in
    /// (`guilds.full_map_faction_tint`), and only ever set for the full map.
    ///
    /// **Which side of the front** a pixel is on, not which hex it is in — see
    /// `specs/frontline-territory.md` and [`tint_by_field`]. [`controlling_team`]
    /// still decides *whether* a hex is washed at all, and is the fallback
    /// colour on a map with no front to speak of.
    pub faction_tint: bool,
    pub colonial_tint: Rgba<u8>,
    pub warden_tint: Rgba<u8>,

    // --- frontline overlay ------------------------------------------------
    // `specs/frontline.md`. The model itself lives in `utils::frontline`
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
    /// structures: **21 ms** for the field, a quarter millisecond for the
    /// contour. It was 334 ms before [`crate::utils::frontline::footings`]
    /// clustered the point set — the field costs `cells x sources`, and that cut
    /// 1995 sources to 128.
    ///
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
    /// A cluster of `n` structures holds ground `n^(1/(2k))` times as far out as
    /// a lone one, so this bounds how much a crowd is worth. See
    /// [`crate::utils::frontline::influence`].
    ///
    /// **Back to 1, and it should stay there unless the point set changes.** It
    /// was briefly 2, as the first answer to a line that leaned toward whoever
    /// had built more — but the crowd was the problem, not the exponent, and
    /// [`crate::utils::frontline::footings`] removes it at source. With one
    /// point per footing a count difference means one side genuinely holds more
    /// ground there, which is exactly what the field should be reporting; `k`
    /// above 1 would now be suppressing signal rather than noise. It also costs
    /// smoothness — measured, raising it made the contour's total turning worse,
    /// which is the angular line the second round of feedback picked up on.
    pub influence_falloff: u32,
    /// How close two same-side structures have to be to count as one footing,
    /// as a fraction of [`REGION_WIDTH`]. See
    /// [`crate::utils::frontline::footings`] — this is the knob that decides
    /// whether the line follows territory or building density.
    pub footing_cluster_ratio: f32,
    pub frontline_color: Rgba<u8>,
    /// Halo width as a multiple of the line width.
    ///
    /// The halo is the wider under-stroke the line sits on, and it is what
    /// carries the faction colours: the core covers the middle of it, leaving a
    /// band of [`RenderConfig::colonial_tint`] on the Colonial flank and
    /// [`RenderConfig::warden_tint`] on the Warden one. So this ratio is also
    /// how much colour is visible — at 3.0 each band is about as thick as the
    /// line itself, which is what makes it read as a side rather than as a
    /// fringe.
    ///
    /// The faction colours are **opaque**, and have to be. Segments are stroked
    /// one at a time and overlap at every vertex, so a semi-transparent halo
    /// blends twice there and beads visibly along the line; opaque blending is
    /// idempotent.
    pub frontline_halo_ratio: f32,
    /// What the halo becomes when the territory wash is under it
    /// ([`RenderConfig::faction_tint`]).
    ///
    /// Black, because with the ground either side already carrying the faction
    /// colours the flanks would be saying a second time, in a thin band, what the
    /// wash says across the whole hex — and saying it at the low contrast
    /// `dev/done/frontline/` measured, since a coloured band over a wash of the
    /// same colour barely separates from it. Black separates the line from both
    /// tints and from the terrain.
    pub frontline_halo: Rgba<u8>,
    /// How far a tinted pixel moves toward the faction colour, 0.0 to 1.0.
    /// Low on purpose: the point is to read ownership at a glance without
    /// losing the terrain underneath it.
    pub faction_tint_strength: f32,

    // --- full-map hex borders ---------------------------------------------
    /// Trace each region's hexagon on the finished full map.
    ///
    /// The map is 53 hexes of continuous terrain, and until this there was
    /// nothing on it that said where one ended. Region names give a hex an
    /// identity but not an extent — which of two names a base near a seam
    /// belongs to was a question the picture could not answer.
    ///
    /// Full map only, and unconditionally: a standalone `/get-map` render *is*
    /// one hexagon, already bounded by the edge of the image.
    pub full_map_hex_borders: bool,
    /// Border width in the *finished* full-map PNG, in pixels.
    ///
    /// Absolute for the same reason [`RenderConfig::full_map_icon_px`] and
    /// [`RenderConfig::full_map_label_px`] are, and it is drawn after the
    /// downscale for the same reason the labels are: a hairline traced on the
    /// 63.6 MP composite is a fifth of a pixel by the time anyone sees it.
    pub full_map_hex_border_px: f32,
    /// Ink for those borders.
    ///
    /// **Opaque, and it has to be.** Every interior edge is traced twice, once
    /// by each of the two hexes that share it, and the six corners of a hexagon
    /// are each covered by two of its own segments. A semi-transparent ink
    /// blends twice in all of those places, so shared edges would come out
    /// darker than the map's outer silhouette and every corner would bead —
    /// the same defect `frontline_halo_ratio` documents, at 318 segments
    /// instead of one contour. Opaque blending is idempotent, so weight is
    /// controlled with [`RenderConfig::full_map_hex_border_px`] instead.
    pub hex_border_color: Rgba<u8>,
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
            full_map_hex_borders: true,
            // A hex finishes about 205 px across on the 2048-wide render, so
            // this is a graticule rather than a feature of the map: enough to
            // find a seam when looking for one, thin enough that 53 of them do
            // not become the picture.
            full_map_hex_border_px: 1.5,
            hex_border_color: Rgba([0, 0, 0, 255]),
            frontline: false,
            field_resolution_ratio: 1.0 / 32.0,
            frontline_width_ratio: 5.0 / REGION_WIDTH as f32,
            full_map_frontline_px: 3.0,
            // Comfortably wider than the halo, so the contour is always traced
            // past wherever the stroke can reach.
            frontline_margin_ratio: 48.0 / REGION_WIDTH as f32,
            // 100 px, doubled from the 50 the synthetic renders were tuned at,
            // and it stays doubled even though the falloff it was raised
            // alongside has gone back to 1. Clustering makes it *more*
            // necessary, not less: the radius at which a lone structure flips
            // the field against `m` friendly neighbours `s` away is
            // `r² = (s² + ε)/m^(1/k) - ε`, and collapsing towns to footings
            // drops `m` from tens to a handful while pushing `s` out. At m=5,
            // s=250 the old 50 gives a 102 px island — resolvable by the 32 px
            // grid, which is the loop the whole model exists to avoid. 100 px
            // takes it to 67.
            influence_radius_ratio: 100.0 / REGION_WIDTH as f32,
            influence_falloff: 1,
            // One footing per town, near enough. Foxhole towns huddle inside
            // about 80 px of a 1024 px hex and sit a couple of hundred apart,
            // so this merges a town without reaching the next one. It is the
            // same 100 px as the influence radius by coincidence of scale, not
            // by construction — they answer different questions and should be
            // free to move apart.
            footing_cluster_ratio: 100.0 / REGION_WIDTH as f32,
            // White core over a dark halo, which is the label style and the one
            // combination already shown to survive both Acrithia's pale desert
            // and Deadlands' near-black — the same problem a line crossing 53
            // regions has. Separate from `text_color` all the same: the spec
            // calls colour a knob, and a line is not a label.
            //
            // The halo used to be black and is now the two faction colours,
            // which costs less contrast than it sounds like. Both tints are
            // dark enough (luminance ~95 and ~90 against white's 255) to hold
            // the core off pale terrain, and where they cannot — Deadlands,
            // where they are close to the ground they sit on — what is left is
            // a white line on near-black, which needed no halo to begin with.
            frontline_color: Rgba([255, 255, 255, 255]),
            // 3.0, not the 2.0 a plain halo wanted. Two thirds of the halo is
            // now the only thing saying which side is which, and at 2.0 the
            // visible band is half a line width — under a pixel once the full
            // map is downscaled, which is a colour nobody can name.
            frontline_halo_ratio: 3.0,
            frontline_halo: Rgba([0, 0, 0, 255]),
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

    /// Radius within which same-side structures collapse to one footing, in
    /// world pixels.
    pub fn footing_radius(&self) -> f32 {
        (REGION_WIDTH as f32 * self.footing_cluster_ratio).max(1.0)
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
    frontline: &[Edge],
    ground: Option<Ground>,
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
    //
    // `controlling_team` is the *presence* gate and nothing more — a hex with
    // nothing built in it stays bare terrain. Which colour the ground takes is
    // the field's answer, not this one, wherever there is a field to ask.
    if config.faction_tint {
        if let Some(team) = controlling_team(dynamic_data) {
            match ground {
                Some(ground) => tint_by_field(&mut bg_img, ground, config),
                // No boundary anywhere near this hex, so the field would have
                // said the same thing at every pixel of it anyway.
                None => {
                    if let Some(color) = config.tint_color(team) {
                        tint_region(&mut bg_img, color, config.faction_tint_strength);
                    }
                }
            }
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

/// A tile, and the field that says who holds each pixel of it.
///
/// Two spaces meet here, which is the only fiddly part: the field is sampled in
/// world (full-map canvas) pixels, because a region's front depends on the bases
/// in the regions beside it, while the canvas being painted is one hex's own.
/// `origin` is what converts between them, and it is the tile's grid offset —
/// the same number [`RenderConfig::grid_offset`] gives the compositor.
///
/// Borrowed rather than owned so the full map can hand all 53 tiles the same
/// field. Copying it per tile would be a quarter of a megabyte each time, for a
/// value none of them modify.
#[derive(Clone, Copy)]
pub struct Ground<'a> {
    pub field: &'a Field,
    pub origin: (f32, f32),
}

/// Washes the ground in each faction's colour, split where the field changes
/// sign.
///
/// The same blend as [`tint_region`] — same alpha mask, same strength, same
/// "before the icons" placement — differing only in choosing the colour per
/// pixel instead of once for the hex. That is deliberate: everything the flat
/// wash got right about staying inside the hexagon is behaviour to keep, and the
/// only question being changed is *which* colour, not how it is applied.
///
/// The boundary of the wash lands on the drawn line without any effort to make
/// it, because the two are the same curve: the line is traced along `F = 0` and
/// this switches colour at `F = 0`. Smoothing moves the stroke off the contour
/// by up to a cell, but the stroke is several pixels wide and covers its own
/// drift — so the seam between the two colours is under the line rather than
/// beside it.
fn tint_by_field(
    canvas: &mut ImageBuffer<Rgba<u8>, Vec<u8>>,
    ground: Ground,
    config: &RenderConfig,
) {
    let strength = config.faction_tint_strength.clamp(0.0, 1.0);
    let width = canvas.width() as usize;

    for (y, row) in canvas.chunks_exact_mut(width * 4).enumerate() {
        // Pixel centres, so the sample matches the geometry the stroke uses.
        let sample = ground.field.along(ground.origin.1 + y as f32 + 0.5);

        for (x, pixel) in row.chunks_exact_mut(4).enumerate() {
            let weight = strength * (pixel[3] as f32 / 255.0);

            // Transparent corners are the gaps the hexes interlock through, and
            // a wash in them makes every seam visible.
            if weight <= 0.0 {
                continue;
            }

            // Positive is Colonial, which is the one place a faction is encoded
            // in the field — see `frontline::Source::weight`.
            let color = if sample.at(ground.origin.0 + x as f32 + 0.5) > 0.0 {
                config.colonial_tint
            } else {
                config.warden_tint
            };

            for (channel, value) in pixel.iter_mut().take(3).enumerate() {
                let base = *value as f32;
                let target = color.0[channel] as f32;
                *value = (base + (target - base) * weight).round() as u8;
            }
        }
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
    lines: &[Edge],
    config: &RenderConfig,
) {
    if !config.frontline || lines.is_empty() {
        return;
    }

    let width = config.frontline_width(canvas.width());

    // The flanks name the two sides — but so does the territory wash, better and
    // over the whole hex rather than in a band. Where the wash is underneath,
    // the halo goes back to doing the one job the colours displaced: holding the
    // white core off pale terrain.
    let halo = if config.faction_tint {
        Paint::Flat(config.frontline_halo)
    } else {
        Paint::Flanked {
            colonial: config.colonial_tint,
            warden: config.warden_tint,
        }
    };

    stroke(canvas, lines, width * config.frontline_halo_ratio, halo);

    stroke(canvas, lines, width, Paint::Flat(config.frontline_color));
}

/// How one stroke pass colours itself.
#[derive(Debug, Clone, Copy)]
enum Paint {
    /// One colour for the whole stroke.
    Flat(Rgba<u8>),
    /// Each faction's colour on its own side of the line, split down the
    /// segment. See [`Edge::colonial_side`] — the side comes from the field, and
    /// the sign test below is the geometric half of that same convention.
    Flanked { colonial: Rgba<u8>, warden: Rgba<u8> },
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
    lines: &[Edge],
    width: f32,
    paint: Paint,
) {
    let radius = (width / 2.0).max(0.5);
    let canvas_w = canvas.width() as i64;
    let canvas_h = canvas.height() as i64;

    for line in lines {
        for (index, pair) in line.points.windows(2).enumerate() {
            let (from, to) = (pair[0], pair[1]);
            let (dx, dy) = (to.0 - from.0, to.1 - from.1);

            // A zero-length segment has no sides to split, and painting it flat
            // would put a disc of one faction's colour across both flanks. The
            // segments either side of it cover the same pixels anyway.
            if matches!(paint, Paint::Flanked { .. }) && dx * dx + dy * dy <= f32::EPSILON {
                continue;
            }

            let colonial_side = line.colonial_side.get(index).copied().unwrap_or(true);

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

                    let color = match paint {
                        Paint::Flat(color) => color,
                        Paint::Flanked { colonial, warden } => {
                            // Positive is the `(-dy, dx)` side, which is the
                            // side `Edge::colonial_side` is stated about.
                            let side = dx * (y as f32 + 0.5 - from.1)
                                - dy * (x as f32 + 0.5 - from.0);

                            if (side > 0.0) == colonial_side {
                                colonial
                            } else {
                                warden
                            }
                        }
                    };

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

/// The six corners of one region's hexagon, closed back to the first.
///
/// A flat-top hexagon inscribed in the `width` x `height` footprint: leftmost
/// and rightmost points at half height, the four others a quarter of the width
/// in from each side. That quarter is not a choice — it is the same number as
/// [`RenderConfig::column_pitch_ratio`] seen from the other side. Columns are
/// pitched `3/4` of a width apart *because* the slanted corners occupy the
/// outer quarters, so a hexagon drawn with any other inset would not lie on the
/// seam it is meant to mark. `a_border_lands_on_the_seam_it_shares` is that
/// statement as a test.
fn hex_outline(origin: (f32, f32), width: f32, height: f32) -> Vec<Point> {
    let (x, y) = origin;
    let (quarter, middle) = (width / 4.0, height / 2.0);

    vec![
        (x, y + middle),
        (x + quarter, y),
        (x + width - quarter, y),
        (x + width, y + middle),
        (x + width - quarter, y + height),
        (x + quarter, y + height),
        (x, y + middle),
    ]
}

/// Traces every region's hexagon on the finished full map.
///
/// **After the downscale, and before the labels.** After, because the width is
/// stated in finished pixels and a line resampled to a fifth of itself is a
/// smudge — the same argument [`draw_region_labels`] makes. Before, because a
/// name is the layer the map is read by and nothing is drawn over those.
///
/// `origins` are the tiles' top-left corners in those same finished pixels.
///
/// Two things fall out of [`stroke`] rather than needing code here. It masks by
/// the canvas's own alpha, so the outer silhouette is traced only as far as the
/// terrain actually reaches and nothing is painted into the void around the map
/// — including around a region whose art failed to load, which stays an empty
/// hex rather than gaining an outline of one. And it clips per segment, so the
/// cost is the length of 318 short lines rather than anything to do with the
/// size of the canvas.
pub fn draw_hex_borders(
    canvas: &mut ImageBuffer<Rgba<u8>, Vec<u8>>,
    origins: &[(f32, f32)],
    scale: f32,
    config: &RenderConfig,
) {
    let (width, height) = (
        REGION_WIDTH as f32 * scale,
        REGION_HEIGHT as f32 * scale,
    );

    // `colonial_side` is left empty on purpose: it says which faction holds the
    // ground either side of a *frontline*, and a hex boundary is not one. Only
    // `Paint::Flanked` reads it, and this paints flat.
    let hexes: Vec<Edge> = origins
        .iter()
        .map(|&origin| Edge {
            points: hex_outline(origin, width, height),
            colonial_side: Vec::new(),
        })
        .collect();

    stroke(
        canvas,
        &hexes,
        config.full_map_hex_border_px,
        Paint::Flat(config.hex_border_color),
    );
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
    ///
    /// Walking downward with Colonial ground on the `(-dy, dx)` side puts them
    /// on the left of the canvas: `d = (0, +104)`, so that side is `(-104, 0)`,
    /// which points at decreasing x.
    fn line() -> Vec<Edge> {
        vec![Edge {
            points: vec![(32.0, -20.0), (32.0, 84.0)],
            colonial_side: vec![true],
        }]
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

    /// **The geometry check that matters**, because it is the one that can be
    /// wrong while everything still renders: an inset that is not a quarter of
    /// the width still draws a plausible hexagon, just one that misses the seam
    /// by a few pixels on every hex of the map.
    ///
    /// Two columns are pitched `column_pitch_ratio` apart with the odd one
    /// dropped by `odd_column_offset_ratio`, and the claim is that the right
    /// slanted edge of one hex is the left slanted edge of its neighbour — the
    /// same two points, not merely nearby ones.
    #[test]
    fn a_border_lands_on_the_seam_it_shares() {
        let config = RenderConfig::default();
        let (width, height) = (REGION_WIDTH as f32, REGION_HEIGHT as f32);

        let corner = |col: u32, row: u32| {
            let (x, y) = config.grid_offset(col, row);
            hex_outline((x as f32, y as f32), width, height)
        };

        let left = corner(0, 0);
        let right = corner(1, 0);

        // Right hex's leftmost vertex against the left hex's lower-right corner,
        // and the right hex's upper-left corner against the left hex's rightmost
        // vertex. Those two pairs are the shared edge, read from either side.
        assert_eq!(right[0], left[4], "the shared edge's lower end");
        assert_eq!(right[1], left[3], "and its upper end");
    }

    #[test]
    fn a_hexagon_closes() {
        let outline = hex_outline((10.0, 20.0), 1024.0, 888.0);

        assert_eq!(outline.len(), 7, "six corners and back to the first");
        assert_eq!(outline[0], outline[6]);
    }

    #[test]
    fn borders_stay_out_of_the_transparent_corners() {
        let mut transparent = canvas(0);

        draw_hex_borders(&mut transparent, &[(0.0, 0.0)], 64.0 / 1024.0, &config());

        assert!(
            transparent.pixels().all(|pixel| pixel.0 == [128, 128, 128, 0]),
            "a border painted into the gap the hexes interlock through would \
             show up on every seam of the full map"
        );
    }

    #[test]
    fn borders_off_draws_nothing() {
        let config = RenderConfig {
            full_map_hex_borders: false,
            ..config()
        };
        let mut opaque = canvas(255);
        let before = opaque.clone();

        // The flag is read by the caller, so what this pins is that the caller
        // is the only reader — a border drawn regardless would show up here.
        if config.full_map_hex_borders {
            draw_hex_borders(&mut opaque, &[(0.0, 0.0)], 64.0 / 1024.0, &config);
        }

        assert_eq!(opaque.pixels().collect::<Vec<_>>(), before.pixels().collect::<Vec<_>>());
    }

    /// Drawn on opaque ground, the outline has to actually darken the edge it
    /// traces and leave the middle of the hex alone.
    #[test]
    fn a_border_marks_the_edge_and_not_the_middle() {
        let mut opaque = canvas(255);
        let scale = 64.0 / REGION_WIDTH as f32;

        draw_hex_borders(&mut opaque, &[(0.0, 0.0)], scale, &config());

        let height = (REGION_HEIGHT as f32 * scale).round() as u32;

        // The leftmost vertex sits at half height on x = 0.
        assert!(
            opaque.get_pixel(0, height / 2).0[0] < 128,
            "the hexagon's left point should be inked"
        );
        assert_eq!(
            channels(opaque.get_pixel(32, height / 2)),
            [128, 128, 128],
            "and the middle of the hex left alone"
        );
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
    fn each_faction_gets_the_flank_the_field_gave_it() {
        let mut opaque = canvas(255);

        draw_frontline(&mut opaque, &line(), &config());

        // Six pixels of line on a 3.0 halo leaves nine either side of centre, so
        // x = 26 and x = 38 are both inside the coloured band and clear of the
        // white core.
        let left = opaque.get_pixel(26, 32).0;
        let right = opaque.get_pixel(38, 32).0;
        let config = config();

        assert_eq!(
            [left[0], left[1], left[2]],
            [
                config.colonial_tint.0[0],
                config.colonial_tint.0[1],
                config.colonial_tint.0[2]
            ],
            "the Colonial side of the walk should carry the Colonial colour"
        );
        assert_eq!(
            [right[0], right[1], right[2]],
            [
                config.warden_tint.0[0],
                config.warden_tint.0[1],
                config.warden_tint.0[2]
            ],
            "the other side should carry the Warden colour"
        );
    }

    #[test]
    fn reversing_the_walk_does_not_swap_the_factions() {
        // The whole reason the side is carried rather than derived: `chain`
        // hands back polylines pointing whichever way it happened to walk them.
        // The same front described backwards has to paint the same picture.
        let mut forward = canvas(255);
        let mut backward = canvas(255);

        draw_frontline(&mut forward, &line(), &config());
        draw_frontline(
            &mut backward,
            &[Edge {
                points: vec![(32.0, 84.0), (32.0, -20.0)],
                colonial_side: vec![false],
            }],
            &config(),
        );

        assert_eq!(forward, backward);
    }

    fn source(x: f32, weight: f32) -> crate::utils::frontline::Source {
        crate::utils::frontline::Source { x, y: 32.0, weight }
    }

    /// The canvas's own footprint. These tests draw a single tile sitting at the
    /// world origin, so canvas pixels and world pixels coincide and `Ground`'s
    /// offset is zero — the space conversion is exercised by the full-map path,
    /// not by the colouring rule.
    fn over_canvas(sources: &[crate::utils::frontline::Source]) -> Field {
        Field::sample(
            sources,
            crate::utils::frontline::Bounds {
                min_x: 0.0,
                min_y: 0.0,
                max_x: 64.0,
                max_y: 64.0,
            },
            8.0,
            config().influence_model(),
        )
        .expect("both factions are present")
    }

    fn ground(field: &Field) -> Ground<'_> {
        Ground {
            field,
            origin: (0.0, 0.0),
        }
    }

    fn washed() -> RenderConfig {
        RenderConfig {
            faction_tint: true,
            ..config()
        }
    }

    fn channels(pixel: &Rgba<u8>) -> [u8; 3] {
        [pixel.0[0], pixel.0[1], pixel.0[2]]
    }

    fn tint(color: Rgba<u8>, strength: f32) -> [u8; 3] {
        let mut washed = Rgba([128, 128, 128, 255]);

        for channel in 0..3 {
            let target = color.0[channel] as f32;
            washed.0[channel] = (128.0 + (target - 128.0) * strength).round() as u8;
        }

        channels(&washed)
    }

    #[test]
    fn the_wash_stays_out_of_the_transparent_corners() {
        let mut transparent = canvas(0);
        let before = transparent.clone();
        let field = over_canvas(&[source(-100.0, 1.0), source(164.0, -1.0)]);

        tint_by_field(&mut transparent, ground(&field), &washed());

        // Same rule as the flat wash and the stroke: the corners are the gaps
        // the hexes interlock through, and colour there shows up as seams.
        assert_eq!(transparent, before);
    }

    #[test]
    fn a_hex_the_front_crosses_comes_out_in_both_colours() {
        let mut opaque = canvas(255);
        let config = washed();
        let field = over_canvas(&[source(-100.0, 1.0), source(164.0, -1.0)]);

        tint_by_field(&mut opaque, ground(&field), &config);

        let strength = config.faction_tint_strength;

        assert_eq!(
            channels(opaque.get_pixel(4, 32)),
            tint(config.colonial_tint, strength),
            "the Colonial end of the hex"
        );
        assert_eq!(
            channels(opaque.get_pixel(60, 32)),
            tint(config.warden_tint, strength),
            "and the Warden end, in the same hex"
        );
    }

    #[test]
    fn ground_behind_the_front_is_all_one_colour() {
        let mut opaque = canvas(255);
        let config = washed();
        // Both sources off to the west, the Colonial one nearer: the boundary
        // between them never reaches this hex, so every pixel of it is theirs.
        let field = over_canvas(&[source(-500.0, 1.0), source(-1000.0, -1.0)]);

        tint_by_field(&mut opaque, ground(&field), &config);

        let expected = tint(config.colonial_tint, config.faction_tint_strength);

        assert!(
            opaque.pixels().all(|pixel| channels(pixel) == expected),
            "a hex the front does not cross has no business being two colours"
        );
    }

    /// The failure the spec names: a sliver of one faction's colour on the wrong
    /// side of its own frontline, which would read as inverted flanks rather than
    /// as a rounding error.
    ///
    /// The wash switches colour on the field's zero set; the stroke is drawn
    /// along a contour that marching squares located to about a cell and Chaikin
    /// then moved again. They are not the same curve to the pixel — the claim is
    /// only that the stroke is wide enough to cover the difference, and that is
    /// measured here rather than assumed.
    #[test]
    fn the_seam_between_the_two_colours_hides_under_the_stroke() {
        let config = washed();
        // Off-centre on purpose, so a boundary that quietly defaulted to the
        // middle of the canvas would not pass.
        let sources = [source(-40.0, 1.0), source(120.0, -1.0)];
        let field = over_canvas(&sources);

        let edges = crate::utils::frontline::flanks(
            crate::utils::frontline::smooth(
                crate::utils::frontline::contour(&field),
                crate::utils::frontline::SMOOTHING_ROUNDS,
            ),
            &sources,
            config.influence_model(),
            8.0,
        );
        assert!(!edges.is_empty(), "the fixture has to produce a front");

        let mut wash_only = canvas(255);
        tint_by_field(&mut wash_only, ground(&field), &config);

        let mut with_line = wash_only.clone();
        draw_frontline(&mut with_line, &edges, &config);

        let colonial = tint(config.colonial_tint, config.faction_tint_strength);
        let mut seams = 0;

        for y in 0..64 {
            let Some(seam) = (1..64).find(|x| {
                channels(wash_only.get_pixel(*x, y)) != channels(wash_only.get_pixel(x - 1, y))
            }) else {
                continue;
            };

            assert_eq!(
                channels(wash_only.get_pixel(seam - 1, y)),
                colonial,
                "row {y}: the Colonials are west of this front, so the wash \
                 should change from their colour, not to it"
            );
            assert_ne!(
                channels(with_line.get_pixel(seam, y)),
                channels(wash_only.get_pixel(seam, y)),
                "row {y}: the wash changes colour at x={seam}, which the stroke \
                 does not cover — that seam is a visible sliver"
            );
            assert_ne!(
                channels(with_line.get_pixel(seam - 1, y)),
                channels(wash_only.get_pixel(seam - 1, y)),
                "row {y}: the stroke covers the seam on one side only"
            );

            seams += 1;
        }

        assert!(seams > 50, "only {seams} rows had a seam at all");
    }

    #[test]
    fn the_halo_goes_black_where_the_wash_names_the_sides_instead() {
        let mut opaque = canvas(255);

        draw_frontline(&mut opaque, &line(), &washed());

        // The same two pixels `each_faction_gets_the_flank_the_field_gave_it`
        // checks, which carry the faction colours when there is no wash.
        for x in [26, 38] {
            assert_eq!(
                channels(opaque.get_pixel(x, 32)),
                [0, 0, 0],
                "x={x} should be halo, not a faction band, once the ground \
                 either side is already coloured"
            );
        }
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

