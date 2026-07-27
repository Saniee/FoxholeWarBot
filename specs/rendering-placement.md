# Rendering: icon & text placement

Status: **shipped.** Documents the current per-region placement primitive, which the full-map
renderer (`specs/full-map-renderer.md`) builds on unchanged.

## Motivation
`request_processing.rs::place_image_info` places icons and text with unnamed literal constants
and an implicit anchoring assumption. These "magic numbers" work by accident at the current
single-hex resolution but won't survive a full-world renderer at a different canvas size. This
placement math must become explicit, named, and resolution-relative **before** the full renderer
is built on top of it.

## Grounding facts (measured from `assets/`)
- Every hex background (`Maps/Map*Hex.TGA`) is **1024 × 888**, RGB 32-bit.
- **All 129 icons** (`MapIcons/*.png`) are **48 × 48**.
- Font: `assets/Inter-Bold.ttf`.
- Foxhole API `MapItem.x/y` and `MapTextItem.x/y` are **normalized `0.0..1.0`** within the region.

### What "the full map" is (corrected)
The full Foxhole map is **all hex regions stitched together on their hex-grid positions** — each
region rendered at its own **1024 × 888** footprint and offset into a larger canvas by its grid
coordinate. It is **not** a composite onto `BGOneWorldMap.TGA` (1920 × 1080); that image is a
decorative/background asset, **not** the render target. (Its exact role — and the
`MapHomeRegion*.TGA` assets — is out of scope here; a home-view screenshot is forthcoming and
will inform the full-map layout spec.)

Consequence for this spec: the placement primitive is **region-relative** (anchored to the
1024 × 888 region footprint). It is identical whether a region is rendered standalone (`/get-map`)
or as one tile of the stitched full map — the full renderer only adds a per-region pixel offset.
The full canvas size is therefore derived from the hex-grid packing of regions, not a fixed
1920 × 1080.

## Magic numbers inventory (the pre-rewrite code this replaced)
| Value | Where | What it really is |
|---|---|---|
| `* 0.5` icon resize (×2) | `request_processing.rs:34-37, 49-52` | fixed 24 px target (48 × 0.5); **not** a true scale |
| `x * bg_width`, `y * bg_height` as **top-left** | `:39, :54, :67` | anchor = icon/text **corner**, not its center → markers sit down-right of their true point |
| `PxScale { x: 25.0, y: 25.0 }` | `:62-65` | fixed 25 px font, independent of canvas size |
| `Rgba([0, 0, 0, 255])` | `:67` | text color, no outline/contrast handling |
| `FilterType::Lanczos3` | `:37, :52` | resample filter (fine, but should be named) |
| `MapMarkerType::Major/Minor` **ignored** | parsed in `foxhole.rs`, never used | major/minor labels render identically |

## Design

### 1. A single `RenderConfig` (named constants, one source of truth)
Replace the scattered literals with a config struct carrying documented, region-relative values.
Sketch:
```rust
pub struct RenderConfig {
    /// Rendered icon edge length as a fraction of the **region** width (1024).
    /// 24 px ⇒ 24/1024 ≈ 0.0234.
    pub icon_size_ratio: f32,
    /// Major/Minor label heights as a fraction of the **region** height (888).
    /// 25 px ⇒ 25/888 ≈ 0.0282 (major); minor slightly smaller.
    pub major_text_ratio: f32,
    pub minor_text_ratio: f32,
    pub text_color: Rgba<u8>,
    pub text_outline: Option<Rgba<u8>>,   // contrast on light/dark terrain
    pub resample: FilterType,
    pub anchor: Anchor,                    // Center (target) vs TopLeft (legacy)
}
impl Default for RenderConfig { /* the ratios above, reproducing today's look at 1024×888 */ }
```
Constants are expressed as **ratios of the 1024 × 888 region footprint**, so a region looks the
same standalone or as a tile of the stitched full map (the full renderer only offsets each region
into place). The `Default` reproduces today's visual output on a hex (24 px icons, 25 px text) so
single-hex renders don't regress. If the finished full map is downscaled to fit Discord's image
limits, scale the **composite** uniformly rather than changing these per-region ratios.

### 2. Explicit anchoring (the real fix)
Introduce an `Anchor` and a helper that converts a normalized point + a drawn object's size into
a pixel top-left:
```rust
fn place(norm_x: f64, norm_y: f64, w: u32, h: u32, cw: u32, ch: u32, anchor: Anchor) -> (i64, i64)
```
- `Anchor::Center` (**recommended target**): subtract half the object's rendered size, so the
  marker/label is centered on the API coordinate. This corrects the current down-right offset.
- `Anchor::TopLeft` (legacy): reproduces current output exactly, for A/B comparison.

> **Decided: `Anchor::Center` is the default.** This is a **visible change** from current renders
> (markers shift up-left by ~half an icon), accepted because the offset is a latent bug the magic
> numbers were hiding — icons should sit *on* their coordinate, not down-right of it.
> `Anchor::TopLeft` is retained in the config purely as an A/B escape hatch for comparing against
> old renders.

### 3. Icon sizing derived from the region footprint, not from the icon
Compute the target edge from `icon_size_ratio * region_width` (1024) and resize to that, instead
of `icon.width() * 0.5`. Because all icons are 48×48 today this is numerically identical on a hex,
but it (a) documents intent and (b) keeps icons correctly sized when the region is a tile of the
stitched full map, regardless of the full canvas size.

### 4. Use `MapMarkerType`
Major vs Minor labels size off `major_text_ratio` / `minor_text_ratio` (and optionally weight),
instead of a single 25 px for both. This is currently parsed and thrown away.

### 5. Contrast for text
Optional 1 px outline (`text_outline`) so labels stay legible over both light and dark terrain —
today's flat black vanishes on dark hexes. Off by default to preserve current look.

## Non-goals (this spec)
- The full-map renderer itself (stitching hex regions onto their hex-grid positions, the
  region→grid-position layout, and the total canvas size). This spec only makes the **per-region
  placement primitive** clean and parameterized so the full renderer can reuse it unchanged.
  That renderer is specced in **`specs/full-map-renderer.md`** (layout table derived and
  verified against the assets), and it depends on this spec landing first.
- Caching and ETag revalidation — that's the map pipeline's job
  (`architecture.md` → Map pipeline). The shared-output-file race this spec used to defer to
  was QA C-1, now fixed by encoding renders in memory.

## Acceptance criteria
- No literal placement/size constant remains in `place_image_info`; all live in `RenderConfig`
  with a documented rationale, expressed relative to the 1024 × 888 region footprint.
- With `RenderConfig::default()` and `Anchor::TopLeft`, a single-hex render is visually identical
  to the current output (regression guard).
- With `Anchor::Center`, icons/labels are centered on their API coordinates within the region.
- A region renders identically standalone and as a tile of the stitched full map (only its pixel
  offset differs) — the placement primitive needs no changes to serve the full renderer.
- Major and Minor labels render at distinct sizes.
