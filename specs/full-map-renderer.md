# Full-map renderer

Status: **shipped.** The layout was derived from a reference screenshot (foxholestats.com,
WC137), cross-checked 1:1 against `assets/Maps/`, then verified against the alpha channel and a
live render: all 53 hexes present, continuous coastlines. Builds on
`specs/rendering-placement.md` (the per-region placement primitive).

## Summary
Render the **entire world conquest map** as a single image: all 53 hex regions composited onto
their hex-grid positions, each drawn with the existing per-region pipeline (background TGA +
dynamic icons + optional labels).

**Scope: the hex grid only.** No header/war banner, no casualty tables, no event log, no
per-region stat readouts, no player counts. Those exist on the reference site but are explicitly
**out of scope** — the deliverable is the rendered hexes.

## Grid geometry

The region assets are **flat-top hexagons**: 1024 × 888, and 1024/888 = 1.153 ≈ 2/√3 = 1.1547,
the exact aspect of a regular flat-top hex. They tile as a standard flat-top hex grid:

| Quantity | Value | Derivation |
|---|---|---|
| Hex width `W` | **1024** | asset width |
| Hex height `H` | **888** | asset height |
| Column pitch | **768** | `3/4 × W` (flat-top horizontal spacing) |
| Row pitch (within a column) | **888** | `H` |
| Odd-column vertical offset | **444** | `H / 2` |

Placement for a region at grid `(col, row)`:
```
x = col * 768
y = row * 888 + (if col is odd { 444 } else { 0 })
```
Adjacent columns' bounding boxes overlap by `W/4 = 256 px`; the assets are 32-bit RGBA with
transparent hex corners, so alpha compositing produces clean seams.

**Canvas:** columns 0–12 ⇒ width `12 × 768 + 1024 = 10240`. Tallest even column is col 6
(rows 0–6) ⇒ `6 × 888 + 888 = 6216`. Full canvas **10240 × 6216** (~63.6 MP).

> ✅ **Measured, no longer derived.** The 3/4-width packing was originally inferred from the
> reference screenshot plus the hex aspect ratio. It has since been checked against the alpha
> channel of the actual TGAs:
>
> - The opaque footprint is a true flat-top hexagon with vertices at `(0, 444)`, `(256, 0)`,
>   `(768, 0)`, `(1023, 444)`, `(768, 887)`, `(256, 887)` — the flat top edge measures
>   `x = 254..769` against a theoretical `256..768`, the two-pixel spill being edge antialiasing.
>   There is **no** transparent padding to correct for.
> - Placing two neighbours at `(+768, +444)` covers their 256 × 444 shared band completely:
>   **0 uncovered pixels.** 1,690 pixels (1.5%) are painted by both, a ~3 px antialiased overlap
>   along the shared edge — an invisible double-draw, not a gap.
> - Compositing all 53 backgrounds at these offsets produces a continuous world map: coastlines
>   and rivers run unbroken across every hex boundary, and the canvas comes out at exactly
>   **10240 × 6216** when computed from the table rather than hardcoded.

## Region layout (53 regions)

Grid coordinates, `col` left→right, `row` top→bottom within a column (odd columns sit half a hex
lower). API/asset name is the `*Hex` identifier; the display name is what the game/UI shows.

| Col | Row | Display name | API / asset name (`Map…​.TGA`) |
|---|---|---|---|
| 0 | 2 | Olavis Wake | `OlavisWakeHex` |
| 1 | 1 | Pari Peak | `PariPeakHex` |
| 1 | 2 | Palantine Berm | `PalantineBermHex` |
| 1 | 3 | Oarbreaker Isles | `OarbreakerHex` |
| 2 | 1 | Kuura Strand | `KuuraStrandHex` |
| 2 | 2 | The Gutter | `GutterHex` |
| 2 | 3 | Fishermans Row | `FishermansRowHex` |
| 2 | 4 | Stema Landing | `StemaLandingHex` |
| 3 | 1 | Nevish Line | `NevishLineHex` |
| 3 | 2 | Farranac Coast | `FarranacCoastHex` |
| 3 | 3 | Westgate | `WestgateHex` |
| 3 | 4 | Origin | `OriginHex` |
| 4 | 1 | Callums Cape | `CallumsCapeHex` |
| 4 | 2 | Stonecradle | `StonecradleHex` |
| 4 | 3 | Kings Cage | `KingsCageHex` |
| 4 | 4 | Sableport | `SableportHex` |
| 4 | 5 | Ash Fields | `AshFieldsHex` |
| 5 | 0 | Speaking Woods | `SpeakingWoodsHex` |
| 5 | 1 | The Moors | `MooringCountyHex` |
| 5 | 2 | The Linn of Mercy | `LinnMercyHex` |
| 5 | 3 | Loch Mór | `LochMorHex` |
| 5 | 4 | The Heartlands | `HeartlandsHex` |
| 5 | 5 | Red River | `RedRiverHex` |
| 6 | 0 | Basin Sionnach | `BasinSionnachHex` |
| 6 | 1 | Reaching Trail | `ReachingTrailHex` |
| 6 | 2 | Callahans Passage | `CallahansPassageHex` |
| 6 | 3 | Deadlands | `DeadLandsHex` |
| 6 | 4 | Umbral Wildwood | `UmbralWildwoodHex` |
| 6 | 5 | Great March | `GreatMarchHex` |
| 6 | 6 | Kalokai | `KalokaiHex` |
| 7 | 0 | Howl County | `HowlCountyHex` |
| 7 | 1 | Viper Pit | `ViperPitHex` |
| 7 | 2 | Marban Hollow | `MarbanHollowHex` |
| 7 | 3 | The Drowned Vale | `DrownedValeHex` |
| 7 | 4 | Shackled Chasm | `ShackledChasmHex` |
| 7 | 5 | Acrithia | `AcrithiaHex` |
| 8 | 1 | Clanshead Valley | `ClansheadValleyHex` |
| 8 | 2 | Weathered Expanse | `WeatheredExpanseHex` |
| 8 | 3 | The Clahstra | `ClahstraHex` |
| 8 | 4 | Allods Bight | `AllodsBightHex` |
| 8 | 5 | Terminus | `TerminusHex` |
| 9 | 1 | Morgens Crossing | `MorgensCrossingHex` |
| 9 | 2 | Stlican Shelf | `StlicanShelfHex` |
| 9 | 3 | Endless Shore | `EndlessShoreHex` |
| 9 | 4 | Reavers Pass | `ReaversPassHex` |
| 10 | 2 | Godcrofts | `GodcroftsHex` |
| 10 | 3 | Tempest Island | `TempestIslandHex` |
| 10 | 4 | Wresta | `WrestaHex` |
| 10 | 5 | Onyx | `OnyxHex` |
| 11 | 2 | Lykos Isle | `LykosIsleHex` |
| 11 | 3 | The Fingers | `TheFingersHex` |
| 11 | 4 | Tyrant Foothills | `TyrantFoothillsHex` |
| 12 | 4 | Pipers Enclave | `PipersEnclaveHex` |

**Verified:** this table's 53 entries match the 53 `Map*Hex.TGA` assets exactly — no asset
unplaced, no entry without an asset.

### Storage
A static table in code, **not** config — the layout only changes when Siege Camp ships/removes a
region, which is a code change anyway (new art asset needed regardless).

Shipped as `utils::regions::REGIONS`, a `&[Region]` where `Region` carries `api_name`,
`display_name`, `col` and `row`. It lives in `regions.rs` rather than beside the renderer so a new
region is **one** edit: the display-name table and the grid table are the same table. Every
`api_name` in it has been confirmed to resolve to an existing `assets/Maps/Map*.TGA`.

### Identifier drift (gotcha, confirmed live)
The API's own spelling does **not** match the asset names, and matching them exactly loses
regions. Checked against a live `/worldconquest/maps` response (Able, 2026-07):

| API says | Assets and this table say | Consequence of an exact match |
|---|---|---|
| `MarbanHollow` | `MarbanHollowHex` | Region silently dropped — **a hole in the world map** |
| `DeadLandsHex` | `MapDeadLandsHex.TGA` / `MapDeadlandsHex.TGA` | Case drift on disk; missing background on Linux |

All 52 other names match exactly. Two rules follow, and the renderer holds both:

- **Region identifiers are compared leniently** — case-insensitively, with the `Hex` suffix
  optional (`regions::same_region`). No two regions collide under that rule.
- **The API's spelling addresses the API; the table's spelling addresses the assets.** They are
  not interchangeable. `MapMarbanHollow.TGA` exists as a stale leftover of an older art drop, so
  using the API's name for the asset path renders year-old terrain rather than failing loudly.

Beyond that, a missing background is never fatal to the layout: a region the API doesn't list, or
whose fetch fails, is drawn as bare terrain. Only the art going missing can leave a hole, and
`load_background` retries case-insensitively (with a warning) before giving up.

### Display-name mapping (gotcha)
Several API names differ from displayed names — the renderer must not assume they match:

| Display | API name |
|---|---|
| The Moors | `MooringCountyHex` ⚠️ wholly different |
| Deadlands | `DeadLandsHex` ⚠️ capital **L** |
| Loch Mór | `LochMorHex` (no accent) |
| Oarbreaker Isles | `OarbreakerHex` (no "Isles") |
| The Linn of Mercy | `LinnMercyHex` |
| The Gutter / The Heartlands / The Drowned Vale / The Clahstra | `GutterHex` / `HeartlandsHex` / `DrownedValeHex` / `ClahstraHex` (no "The") |
| The Fingers | `TheFingersHex` ⚠️ **keeps** "The" — inconsistent with the above |

The existing autocomplete's "strip `Hex`" display transform is therefore **wrong** for these; the
rewrite should use an explicit display-name table (this one) rather than string surgery.

## Non-conquest assets (not part of the grid)
- `BGOneWorldMap.TGA` (1920 × 1080, 24-bit, no alpha) — background/decorative art, **not** the
  render target and not a grid tile (confirms the earlier correction).
- `MapHomeRegionC.TGA` / `MapHomeRegionW.TGA` — Colonial/Warden home regions; not part of the
  world-conquest grid, excluded.
- ~~`MapMarbanHollow.TGA`, `MapClahstraHexMap.TGA`~~ — **deleted.** They were *stale* near-
  duplicates of their `*Hex` counterparts, not identical to them: the art drops updated
  `MapMarbanHollowHex.TGA` and `MapClahstraHex.TGA` only, and `/get-map` was resolving the API's
  `MarbanHollow` straight onto the year-old file. Both are gone, but asset paths still go through
  `regions::asset_name` — deletion fixed this instance, the routing is what stops the next one.
  `assets/Maps/` now holds exactly one file per table region plus the three non-conquest assets
  above.

## `OriginHex` — un-exclude it
Current code skips `OriginHex` in every autocomplete (`get_map.rs:128`, `war_report.rs:105`,
`schedule_report.rs:109`). The reference screenshot shows **Origin as a live region** (col 3,
row 4) with real activity. The exclusion appears to be legacy (Origin was once a dev/placeholder
region). **Decided: include it** in both the full map and autocomplete. Guard at runtime rather
than by hardcoded name — if the API omits a region from `/worldconquest/maps`, it simply isn't
rendered, which handles stubs generically without a special case.

## Rendering pipeline
1. Fetch dynamic (+ static if labels are on) data for **all 53 regions** — see Performance.
2. Allocate the 10240 × 6216 RGBA canvas.
3. For each region: run the existing per-region pipeline
   (`specs/rendering-placement.md` — background + icons at region-relative sizes), then
   `overlay` the result at its `(x, y)` grid offset. Region-relative sizing means a hex looks
   identical standalone or tiled; only the offset differs.
4. Downscale the finished composite (see below).
5. **Trace the hex borders on the downscaled image** (see below).
6. **Write the region names on it**, then encode PNG.

### Hex borders (drawn after the downscale)
The map is 53 hexes of continuous terrain, and nothing on it said where one ended. A name gives a
hex an identity but not an extent, so which of two regions a base near a seam belonged to was a
question the picture could not answer. Each region's hexagon is traced in black:
`full_map_hex_borders` (default **on**, full map only — a `/get-map` render *is* one hexagon,
already bounded by the edge of the image).

**After the downscale**, for the same reason the names are: the width is stated in finished pixels
and a hairline traced on the 63.6 MP composite is a fifth of a pixel by the time anyone sees it.
**Before the names**, because a border is the frame the map is divided by — it belongs over the
terrain and the icons — but a black line through a label costs more legibility than it buys.

`full_map_hex_border_px` (default **1.5**) is a graticule rather than a feature of the map: enough
to find a seam when looking for one, thin enough that 53 of them do not become the picture. 1.0 was
compared against it on a live shard and rejected — legible over the pale northern terrain, too
faint over the dark Colonial ground in the south, and a border that fades out over half the map is
worse than none.

`hex_border_color` is **opaque** and has to be. Every interior edge is traced twice, once by each
of the two hexes sharing it, and each of a hexagon's six corners is covered by two of its own
segments — so a semi-transparent ink blends twice in all of those places, making shared edges
darker than the outer silhouette and beading every corner. This is the same defect
`specs/frontline.md` documents for the halo, at 318 segments instead of one contour. Weight is
controlled with the width instead.

The stroke is masked by the canvas's own alpha, which settles two cases without special handling:
the outer silhouette is traced only as far as the terrain reaches, so nothing is painted into the
void around the map, and a region whose art failed to load stays an empty gap rather than gaining
an outline of a hex that is not there.

**The quarter-width inset is not a free parameter.** The hexagon is flat-top with its slanted
corners a quarter of the width in from each side, which is the same number as `column_pitch_ratio`
(3/4) seen from the other direction — columns are pitched three quarters of a width apart
*because* the corners occupy the outer quarters. Any other inset draws a plausible hexagon that
misses the seam on every hex of the map, which is why the test asserts the shared edge's two
endpoints are *the same points* read from either neighbour rather than merely close.

### Region names (drawn after the downscale)
Each hex carries its `display_name`, centred, so a reader can orient themselves — the whole point
of a world map being one image.

**After the downscale, never before**, and this is the crux: text drawn on a 1024 px tile is
resampled along with the terrain and arrives as a smudge, which is why `/full-map` has always
passed `draw_text: false`. Choosing a bigger size on the tile does not fix it — the glyphs still
go through the filter. Drawn afterwards they are rendered once, at final size, on final pixels.
It is also the last thing to touch the image: names are the layer the map is read *by*, so
nothing is drawn over them.

`full_map_label_px` (default **19**) states the cap height wanted in the finished PNG, for the
same reason `full_map_icon_px` does. A name too long for its hex shrinks to
`full_map_label_width_ratio` (0.82) of the hex's finished width rather than running into its
neighbour; at 19 px nothing currently needs to, so the fit rule is a safety net and not a routine
step. Colour and halo come from `text_color` / `text_outline` — the same style as every other
label (`specs/rendering-placement.md` → Contrast for text).

**Centring is safe at most of the hex's width**, and not by luck: a flat-top hex is widest across
its own vertical centre, and that is exactly the height at which the neighbouring columns' art
does not reach it. Their centres sit half a hex above and below, so at this line they are at
their own flat top or bottom edge, which spans only the middle half of their width. The two
footprints meet and never overlap.

Every region is named, including one drawn as bare terrain because its fetch failed — the hex a
user most needs identified is the one with no data on it.

### Downscaling (required)
10240 × 6216 is far past Discord's attachment limit as a PNG. Scale the **composite** uniformly —
never the per-region ratios — with a target long-edge of **~2048 px** (0.2× ⇒ 2048 × 1243), tuned
against the real encoded size. Make the factor a named constant in `RenderConfig`, not a literal.
If icons become illegible at that scale, prefer bumping `icon_size_ratio` for full-map renders
over changing the downscale.

**They did — confirmed on the first live render.** The per-region 24 px icon is 4.8 px after the
0.2× downscale, and about **one pixel** once Discord scales the embed to ~400 px. The first full
map came back looking like bare terrain for exactly this reason; the icons were drawn, then
resampled out of existence.

Fixed by sizing icons backwards from the finished image rather than by picking a bigger ratio:
`full_map_icon_px` (default **12**) states the icon edge wanted in the output PNG, and
`RenderConfig::for_full_map(scale)` returns a config whose `icon_size_ratio` is whatever the
1024 px tile needs to land there — 60 px at 0.2×. A raw ratio bump would have been a magic number
tied to today's `full_map_long_edge`; this stays correct if that ever changes. **The downscale
itself is untouched**, so terrain still matches a standalone `/get-map` render.

### Performance
This is 53× the work of a single hex — the reason scheduled full-map renders are gated
(`specs/premium-full-map.md`):
- Fetch the 53 regions **concurrently but bounded at 8**, and **respect the API's cache
  headers/ETags** — the one explicit rule in the War API terms. The existing on-disk cache makes
  most ticks 304s. Implemented with `JoinSet` + a `Semaphore(8)` rather than
  `StreamExt::buffer_unordered`: identical bound, and it avoids pulling the `futures` crate in as
  a direct dependency for one combinator.
- Run the CPU-bound compositing in `spawn_blocking` so the async runtime isn't stalled
  (QA **L-8**, which matters far more at this scale).
- A partial failure (one region 500s or its asset is missing) should render that hex as
  background-only or skip it, **never** abort the whole map.
- Consider caching the finished composite keyed by shard + newest `last_updated`, so concurrent
  requests share one render.

### Saying what it is doing

A full map is 53 fetches and a 63.6 MP composite. That is long enough that a deferred interaction
showing nothing but a spinner reads as a command that failed silently, so `/full-map` posts a status
message before the work starts and **edits that same message into the finished map** when it is
done. Errors edit it too — otherwise the "Rendering…" line sits above the failure forever.

**One message, posted once, never updated in place.** A moving progress bar is the obvious design
and is the wrong one: editing an interaction response counts against Discord's rate limits, and the
render passes through something worth showing about a hundred times (53 fetches, 53 hexes, the
downscale, the encode). A live bar therefore means either throttling down to a handful of edits that
barely move, or spending a guild's rate limit on decoration. A single message says the same thing in
one call.

What it says is deliberately specific rather than "Working…":

- **The shard**, so a guild pointed at the wrong one finds out before waiting.
- **The region count**, which is what makes the wait read as proportionate rather than stuck.
- **Which overlays are on**, when either is. A user who forgot they enabled the frontline or the
  territory tint has no way to tell from the finished image whether the setting took.

Scheduled reports get none of this. They post through a webhook with no message to edit and nobody
watching a spinner.

## Decisions (settled)
- **Faction control tint — shipped, opt-in per guild.** The reference tints each hex blue/green by
  controlling faction; our renderer composites the real map art, so a tint is a visual departure
  from the per-hex renders. **The wash is no longer per hex** — `specs/frontline-territory.md`
  replaced "which faction holds this hex" with "which side of the front is this pixel on", so what
  follows describes the gate and the blend, which both survived, rather than the colour choice,
  which did not. It lives behind `RenderConfig::faction_tint` with the colours and
  strength as named constants, and a guild turns it on with
  `/set-guild-settings faction-tint:true` (`guilds.full_map_faction_tint`, default off). The
  "darker = more recent change" recency shading is **not** in scope for v1.
  - **Full map only.** `/get-map` renders one hex at a scale where the terrain is what you are
    reading, and a wash over it costs more than it says. The flag is set in `full_map.rs` and
    nowhere else, so `/get-map` cannot pick it up by accident.
  - **Control = town bases + relic bases, by majority** (`CONTROL_ICON_TYPES`: 45, 46, 47, 56, 57,
    58). Those are the structures that actually flip a region. Counting every faction-owned
    structure as equal instead would tint a hex for whoever built more sheds in it.
    - **A tie on bases falls through to every other structure the two sides hold there.** Ties are
      not rare — one base each is the ordinary shape of a contested hex — and the first live render
      with the tint on came back with exactly one untinted hex in the middle of the front, which
      reads as a rendering fault, not as "contested". The tiebreak asks the same question at finer
      resolution: whoever has more built in a hex is the one sitting in it. Untinted now means
      equal on *both* counts, which in practice is an empty hex or one the API didn't report.
  - **The wash is applied to the background before the icons**, and scaled by each pixel's alpha.
    Both matter: tinting after would wash the already-faction-coloured icons in the same colour,
    and tinting flat would paint the hex's transparent corners — which carry real RGB under
    `alpha = 0` — making every interlock seam visible.
- **Per-hex region name labels — deferred to v2.** In-region POI labels (`draw_text`) would be
  unreadable at the 0.2× composite scale, so a full-map render draws no text in v1. A dedicated
  "region name per hex" pass (drawn *after* downscale, at readable size) is the v2 approach.
- **Command surface — a dedicated `/full-map` command.** Not an option on `/get-map`: the gating
  (`specs/premium-full-map.md`) applies to the full map only, and `/get-map`'s required
  `map-name` + autocomplete would have to be made meaningless when a `full:true` flag was set.
  A separate command keeps both surfaces clean.

## Acceptance criteria
- All 53 regions render at their correct grid positions with clean seams (no gaps/overlap
  artifacts).
- A single region's appearance is identical to its standalone `/get-map` render, modulo the
  composite downscale.
- The output attaches successfully to a Discord message (within upload limits).
- One failing region degrades that hex only; the rest of the map still renders.
- Region fetches are concurrent and ETag-revalidated; compositing does not block the runtime.
- Every hex carries its region name, centred, legible at the finished resolution and over both
  the palest and the darkest terrain; a name never strays onto a neighbouring hex.
- Every region's boundary is visible, and each border lies *on* the seam it marks rather than
  beside it — two neighbours' shared edge is one line, not two.
- Borders are legible over the darkest terrain on the map as well as the palest, and no border is
  painted outside the terrain — neither into the void around the silhouette nor around a hex whose
  art is missing.
- No border crosses a region name.
- A region whose data failed to fetch is still named.
- No stats/banner/log furniture is drawn — hexes and their names only.
- `/full-map` says what it is doing before the render starts, and the finished map replaces that
  message rather than arriving beside it. Every failure path replaces it too.
- Exactly one message is posted and one edit made per invocation, whatever the render does.
