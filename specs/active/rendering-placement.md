# Rendering: de-magic icon & text placement (ACTIVE)

Status: **proposed** — prerequisite for the full-map renderer.

## Motivation
`request_processing.rs::place_image_info` places icons and text with unnamed literal constants
and an implicit anchoring assumption. These "magic numbers" work by accident at the current
single-hex resolution but won't survive a full-world renderer at a different canvas size. This
placement math must become explicit, named, and resolution-relative **before** the full renderer
is built on top of it.

## Grounding facts (measured from `assets/`)
- Every hex background (`Maps/Map*Hex.TGA`) is **1024 × 888**, RGB 32-bit.
- The world map (`Maps/BGOneWorldMap.TGA`) is **1920 × 1080**.
- **All 129 icons** (`MapIcons/*.png`) are **48 × 48**.
- Font: `assets/Inter-Bold.ttf`.
- Foxhole API `MapItem.x/y` and `MapTextItem.x/y` are **normalized `0.0..1.0`** within the region.

## Magic numbers inventory (current code)
| Value | Where | What it really is |
|---|---|---|
| `* 0.5` icon resize (×2) | `request_processing.rs:34-37, 49-52` | fixed 24 px target (48 × 0.5); **not** a true scale |
| `x * bg_width`, `y * bg_height` as **top-left** | `:39, :54, :67` | anchor = icon/text **corner**, not its center → markers sit down-right of their true point |
| `PxScale { x: 25.0, y: 25.0 }` | `:62-65` | fixed 25 px font, independent of canvas size |
| `Rgba([0, 0, 0, 255])` | `:67` | text color, no outline/contrast handling |
| `FilterType::Lanczos3` | `:37, :52` | resample filter (fine, but should be named) |
| `MapMarkerType::Major/Minor` **ignored** | parsed in `foxhole.rs`, never used | major/minor labels render identically |

## Target design

### 1. A single `RenderConfig` (named constants, one source of truth)
Replace the scattered literals with a config struct carrying documented, canvas-relative values.
Sketch:
```rust
pub struct RenderConfig {
    /// Rendered icon edge length as a fraction of canvas width.
    /// 24 px on a 1024-wide hex ⇒ 24/1024 ≈ 0.0234.
    pub icon_size_ratio: f32,
    /// Major/Minor label heights as a fraction of canvas height.
    /// 25 px on an 888-tall hex ⇒ 25/888 ≈ 0.0282 (major); minor slightly smaller.
    pub major_text_ratio: f32,
    pub minor_text_ratio: f32,
    pub text_color: Rgba<u8>,
    pub text_outline: Option<Rgba<u8>>,   // contrast on light/dark terrain
    pub resample: FilterType,
    pub anchor: Anchor,                    // Center (target) vs TopLeft (legacy)
}
impl Default for RenderConfig { /* the ratios above, reproducing today's look at 1024×888 */ }
```
Constants are expressed as **ratios of the target canvas**, so the same config yields
proportional results on a 1024×888 hex and a 1920×1080 world map. The `Default` is chosen to
reproduce today's visual output on a hex (24 px icons, 25 px text) so single-hex renders don't
regress.

### 2. Explicit anchoring (the real fix)
Introduce an `Anchor` and a helper that converts a normalized point + a drawn object's size into
a pixel top-left:
```rust
fn place(norm_x: f64, norm_y: f64, w: u32, h: u32, cw: u32, ch: u32, anchor: Anchor) -> (i64, i64)
```
- `Anchor::Center` (**recommended target**): subtract half the object's rendered size, so the
  marker/label is centered on the API coordinate. This corrects the current down-right offset.
- `Anchor::TopLeft` (legacy): reproduces current output exactly, for A/B comparison.

> **Decision needed:** switching the default to `Center` is a **visible change** from current
> renders (markers shift up-left by ~half an icon). Recommended, since the offset is a latent
> bug the magic numbers were hiding — but it's opt-outable via config if you want byte-identical
> hex output first.

### 3. Icon sizing derived from canvas, not from the icon
Compute the target edge from `icon_size_ratio * canvas_width` and resize to that, instead of
`icon.width() * 0.5`. Because all icons are 48×48 today this is numerically identical on a hex,
but it (a) documents intent and (b) makes world-map icons scale correctly instead of rendering
at a fixed 24 px on a 1920 canvas.

### 4. Use `MapMarkerType`
Major vs Minor labels size off `major_text_ratio` / `minor_text_ratio` (and optionally weight),
instead of a single 25 px for both. This is currently parsed and thrown away.

### 5. Contrast for text
Optional 1 px outline (`text_outline`) so labels stay legible over both light and dark terrain —
today's flat black vanishes on dark hexes. Off by default to preserve current look.

## Non-goals (this spec)
- The full-world renderer itself (region compositing onto `BGOneWorldMap`, world offsets,
  hex-grid layout). This spec only makes the **per-region placement primitive** clean and
  parameterized so the world renderer can reuse it. That renderer gets its own spec.
- Caching, ETag, and the `render.png` output-path race (tracked separately: QA C-1).

## Acceptance criteria
- No literal placement/size constant remains in `place_image_info`; all live in `RenderConfig`
  with a documented rationale, expressed relative to canvas dimensions.
- With `RenderConfig::default()` and `Anchor::TopLeft`, a single-hex render is visually identical
  to the current output (regression guard).
- With `Anchor::Center`, icons/labels are centered on their API coordinates on both a 1024×888
  hex and a 1920×1080 canvas, at proportional sizes.
- Major and Minor labels render at distinct sizes.
- The same placement helper is callable by the future world renderer with only a different
  canvas size (no code changes to the primitive).
