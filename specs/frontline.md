# Frontline overlay

## Summary

Draw the contested boundary between Colonial- and Warden-held ground on rendered maps, as a line
following actual territory rather than hex borders. Opt-in per guild, on both `/get-map` and
`/full-map`.

## Why it isn't the hex borders

`full_map_faction_tint` already answers "who holds this hex" and washes the whole hex in one
colour. That is a region-resolution answer to a sub-region question: the front usually runs
*through* a hex, and the hex a player cares about most — the contested one — is exactly the one
whose single flat colour is least true. A frontline is the thing the tint cannot express.

## Territory model

**The line is approximate on purpose.** It shows roughly where each faction has a footing, not a
defensible claim about who owns a given square metre. Precision here would be false precision: the
API reports structures, not held ground, and no amount of contour resolution turns one into the
other. That target shapes every choice below — it is why a smooth, slightly vague line is the
*correct* output and a crisp one would be a worse answer more confidently stated.

So: an **influence field**, and its zero crossing.

```
F(p) = Σ  w / (d² + ε)^k   over Colonial structures
     − Σ  w / (d² + ε)^k   over Warden structures
```

The frontline is the contour `F(p) = 0`; `F` positive is Colonial ground, negative is Warden.

### One point per footing, not one per building

**This is the most important thing on the page, and it took two rounds of live-war feedback and one
wrong answer to find.**

The API reports one item per built object. Fed straight into the field, a town ringed by thirty
bunker icons casts thirty votes and a lone forward base casts one — so the contour tracks where each
side has **built more**, not where each side **is**. Against the first live war that put the line
about a quarter of the way across the contested ground instead of half, leaning toward whoever was
more lightly built, and it also made the line *angular*, because thirty points in a 60 px huddle
give the field a great deal of local structure to follow.

So the sources are clustered before the field sees them: same-side structures within
`footing_cluster_ratio` collapse to one point at their centroid, weighing 1. Opposing structures
never merge — two towns facing each other across a river are the entire subject of the picture.

### `k`, and the wrong answer that came first

`k` bounds how much a crowd is worth: one source at distance `a` balances `n` at distance `b` when
`b/a = n^(1/(2k))`. The first attempt at the leaning line was to raise `k` from 1 to 2, compressing
`√n` to `n^(1/4)`. **That was the wrong lever, and the measurement says so plainly.** On a fixture
carrying the same 3:1 density asymmetry as the real map:

| | off centre | total turning |
|---|---|---|
| every icon, `k = 1` | +138 px | 158° |
| every icon, `k = 2` | +68 px | 90° |
| **one point per footing, `k = 1`** | **+33 px** | **42°** |
| one point per footing, `k = 2` | +21 px | 60° |

Clustering beats the exponent at its own job and *improves* smoothness where raising `k` cost it.
The error was treating `n` as given: the exponent can only bound what a crowd is worth, where
clustering stops the crowd being counted as a crowd at all.

`k` stays at 1 and stays a knob. Once the point set is footings, a count difference means one side
genuinely holds more ground there, which is what the field should be reporting — `k > 1` would now
suppress signal rather than noise. The general property still holds and is worth recording: the
sign of `Σ w/(d²+ε)^k` is the sign of the difference between the two sides' `p`-norm soft-minimum
distances at `p = 2k`, so raising `k` walks the contour toward the medial axis and, in the limit,
*is* the Voronoi partition rejected below.

Clustering also made the field **16x faster** — it costs `cells × sources`, and 1995 sources became
128 footings. See Configuration.

Two alternatives were considered and rejected:

- **Union of discs** (each base claims a fixed radius, wilderness stays neutral). Killed by the
  reference render, where the line runs unbroken from one hex edge to the other across open water.
  A radius cutoff chops exactly the deep, quiet stretches of a front into disconnected segments.
- **Nearest-neighbour partition** (Voronoi between the two sides). Continuous, but brittle in the
  way that matters: it is decided by the single closest structure, so one forward base deep in
  enemy ground punches an island out of the partition and the overlay grows a closed loop nobody
  asked for. A field sums, so an isolated outpost makes a bump rather than a partition.

  **But the field alone does not fully deliver that criterion, and the implementation should not
  pretend it does.** Within about `√ε` of an isolated structure its own `w / ε` term outweighs
  everything around it, so `F` genuinely does cross zero there. What suppresses the island is the
  sampling grid: the crossing is a few tens of pixels across and the grid is sampled hundreds of
  pixels apart, so it falls entirely between samples. **The coarse grid is a low-pass filter on
  the field, not merely a cost saving** — which is the concrete reason not to "improve" this
  feature by sampling it finely, on top of the general one below.

The field is also smooth by construction, which is most of the "roughly" the user asked for — it
comes from the model rather than from post-hoc smoothing of a jagged boundary.

### What counts as a footing

**Every faction-held structure is evidence; every cluster of them is one footing.** The two halves
matter equally, and it took two rounds of feedback to get both.

Structures are collected without filtering — not narrowed to `CONTROL_ICON_TYPES` — and then
clustered. Narrowing at collection time would answer the same question worse: it throws away every
footing that is not a town, and a hex held only by field bunkers would disappear from the field
entirely. Clustering keeps that evidence while counting the *place* once instead of counting the
buildings.

**Two claims this section used to make were wrong, and both were found by looking at a live war
rather than by thinking harder.**

- It said the tint's failure mode "does not exist here" — that a structure deep in friendly
  territory contributes nothing to where the boundary sits. False. Weighting by distance reduces a
  crowd's influence; it does not remove it, and the bound is `n^(1/(2k))` rather than nothing.
- Having admitted that, it then proposed the exponent as the fix and clustering as a fallback.
  Backwards. See the table above: clustering wins on centring *and* on smoothness, where the
  exponent trades one against the other.

Neutral-owned structures contribute to neither side.

> **This is the one real risk in the feature.** "Every structure" is a judgement, and the first
> thing to check against a live war. If the line sits wrong, try `CONTROL_ICON_TYPES` only, or a
> per-type weight `w`, before touching resolution or smoothing — those will not fix a bad input.

## Coordinate space

One world space for both commands, so there is one implementation and not two.

`RenderConfig::grid_offset(col, row)` already gives a hex's top-left pixel on the full-map canvas.
A map item's world position is therefore:

```
world = grid_offset(region.col, region.row) + (item.x × REGION_WIDTH, item.y × REGION_HEIGHT)
```

`/get-map` renders a single hex, so it computes the field in world space and then draws only the
window belonging to its own region. Computing in world space rather than region-local space is
what makes the line continuous: a per-region computation breaks the line at every hex seam,
because a region's front depends on bases in the regions next to it.

## Neighbour data

**A single-hex frontline is wrong at its own edges without its neighbours' bases.** `/get-map`
must therefore fetch the dynamic data of the (up to six) adjacent regions as well.

Adjacency is **derived, not tabulated.** `regions.rs` stores `(col, row)` in a flat-top odd-q
offset grid — odd columns sit half a hex lower — so the six neighbours are arithmetic:

```
odd  col: (0,-1) (0,+1) (-1, 0) (-1,+1) (+1, 0) (+1,+1)
even col: (0,-1) (0,+1) (-1,-1) (-1, 0) (+1,-1) (+1, 0)
```

Checked against the shipped table: the relation is symmetric for all 53 regions, degrees run 2–6,
and it reproduces real geography (Deadlands ↔ Callahans Passage, Umbral Wildwood, The Linn of
Mercy, Loch Mór, Marban Hollow, The Drowned Vale). A neighbour column would be a second thing to
edit when Siege Camp ships a region, which is the mistake `regions.rs` already exists to avoid.

Cost: up to 7 dynamic fetches instead of 1. They are the same ETag-revalidated, disk-cached
requests everything else makes, so a warm cache pays six conditional requests answered `304`.
A neighbour that fails to fetch is **skipped with a warning, not fatal** — the line degrades near
that edge exactly as it would have without the feature, which is strictly better than no render.

> Note the existing rule: the dynamic and static halves revalidate independently and must never be
> assumed to agree. Only the dynamic half is needed here.

## Rendering

1. **Sample the field.** Evaluate `F` on a grid at `field_resolution_ratio` of the footprint.
   Coarse on purpose: an approximate line does not need a fine grid, and the field is already
   smooth, so sampling it densely buys nothing but time.
   - `/get-map`: a ~256 × 222 grid (~57k cells) against the region's structures plus its
     neighbours' — order 100 points, a few million float ops, sub-millisecond.
   - `/full-map`: `F` is a sum over *all* points, so it is O(cells × points) and does not reduce
     to a distance transform the way a nearest-neighbour partition would. **Measured** on the real
     10240 × 6216 composite with 2000 structures, release build:

     | sampling | cells | field | contour + smoothing |
     |---|---|---|---|
     | 1/16 of a region (64 px) | 15.9k | 61 ms | 0.16 ms |
     | 1/32 (32 px) | 62.9k | **255 ms** | 0.26 ms |
     | 1/64 (16 px) | 250k | 965 ms | 0.89 ms |

     **Those are pre-clustering numbers, kept because they are what the resolution choice was made
     against.** Since the field is `O(cells × points)` and clustering cut 1995 structures to 128
     footings, 1/32 now measures **21 ms**, and clustering itself costs 0.2 ms. The accuracy fix
     was a 16x speedup. Tracing the contour is free at every resolution; the field is the whole
     cost, and it is linear in both cells and points.

     The headroom this opens is deliberately not spent. Finer sampling would undo the island
     suppression above, which is the one thing the coarse grid is load-bearing for.
   - The same spacing serves both commands, so the line looks identical whichever one drew it
     rather than being smoother in one of them.
2. **Contour.** Marching squares on `F = 0` **directly on the sampled grid**, producing polylines
   in world space, then two rounds of Chaikin smoothing on the *polyline*.

   An earlier draft of this spec interpolated the field up bilinearly and contoured the finer
   grid. That was dropped: bilinear interpolation cannot invent a crossing inside a cell whose
   four corners agree in sign, so it changes nothing about the field or the island behaviour, and
   smoothing the traced line reaches the same curve with one grid instead of two. Measured over
   two rounds on a bending front: worst slope change 0.183 → 0.105 rad while the *total* turning
   is unchanged, which is corner cutting rather than straightening — the line stays where the
   field put it.
3. **Ask the field which side is whose**, once the line is smoothed and before it leaves world
   space — `flanks`, one flag per segment. See "Saying which side is which" for why this cannot
   come from the polyline's own direction.
4. **Draw.** Scale the edges to canvas space and stroke at `frontline_width_ratio`, in
   `frontline_color`, over a wider halo laid down first — split down the line into each faction's
   colour — so the line survives on the dark hexes (Deadlands, Umbral Wildwood) where a dark line
   on dark terrain would vanish.
5. **Run the line to the hex edge, then mask.** The field is sampled over an area **larger than
   the hex** and the contour is traced across all of it; only then is the stroke masked by the
   background's own alpha.

   Both halves matter and for different reasons. Masking is what keeps the stroke out of the
   transparent corners, where region art is deliberately see-through so the tiles interlock — a
   line painted there shows on the seams between hexes on the full map, the same trap
   `tint_region` already documents for the wash. Oversampling is what makes the line reach the
   silhouette *exactly*: terminate the polyline at the hex bounds instead and it stops a stroke
   width short, leaving a gap at both ends. The reference has that gap — its line stops ~2 px
   inside the silhouette — and it is an artifact of the mock, not a target.

   On `/get-map` this is precisely what the neighbour fetch buys. Without the adjacent regions'
   structures there is no field to sample outside the hex, so the line could not be drawn to the
   edge even in principle.

**Order: after the background and the tint, before the icons.** The tint is a wash and would
swallow the line; the icons are the map's actual content and a line drawn over them costs
legibility for decoration. This mirrors the existing rule that the tint goes on before the icons.

On `/full-map` the line is drawn **per tile, on the full-resolution composite**, and scaled down
with everything else, so it antialiases for free.

This was reconsidered once the region-name work established a precedent for drawing at final size
*after* the downscale, and it survived. Three reasons, worth recording so it is not reopened a
third time:

- **What the downscale destroys is internal detail, not thinness.** Text broke at 1024 → 205
  because its counters, stroke gaps and inter-letter spacing are resampled into a smudge. A
  stroke has no internal structure; scaled up 5x, drawn, and scaled back, it survives intact and
  gains free antialiasing.
- **Icons are drawn per tile.** Drawing the line after the downscale would put it over every icon
  on the map, which is precisely the ordering this spec rejects.
- **The alpha mask only exists per tile.** Once the tiles are composited, the transparent corners
  have been filled in by their neighbours and there is nothing left to mask against.

**Width must be sized backwards from the finished image**, exactly as `full_map_icon_px` already
does for icons. Applied as a ratio on the 63.6 MP composite and then scaled down, a hairline
disappears completely — the same bug the full-map renderer already hit once with icons, where
"the whole map came back looking like bare terrain".

## Configuration

Every constant is a ratio of the region footprint in `RenderConfig`, never a literal in the
compositing code. **The starting values are starting points, not findings** — width and colour in
particular are meant to be tuned by eye against a live render:

| Field | Meaning | Value |
|---|---|---|
| `frontline: bool` | draw it at all | `false` |
| `field_resolution_ratio: f32` | field sample spacing | `1/32` of a region (32 px) |
| `frontline_width_ratio: f32` | stroke width | `5.0 / REGION_WIDTH` |
| `full_map_frontline_px: f32` | stroke width wanted in the *finished* full map | `3.0` |
| `frontline_margin_ratio: f32` | how far past a hex the field is sampled | `48.0 / REGION_WIDTH` |
| `influence_radius_ratio: f32` | saturation distance; `ε` is its square | `100.0 / REGION_WIDTH` |
| `influence_falloff: u32` | the `k` of `w / (d² + ε)^k` | `1` |
| `footing_cluster_ratio: f32` | how close same-side structures merge into one footing | `100.0 / REGION_WIDTH` |
| `frontline_color` | stroke | white |
| `frontline_halo_ratio: f32` | halo width as a multiple of the line | `3.0` |

The halo has no colour of its own: it is painted in `colonial_tint` and `warden_tint`, a side each,
which is how the line says whose ground is whose. See "Saying which side is which".

Six of these carry a reason beyond taste:

- **`field_resolution_ratio` is not a free knob.** An earlier draft suggested ¼ of a region; 1/32
  is both affordable (21 ms on the real composite) and the value the island behaviour above
  depends on. It, `influence_radius_ratio` and `influence_falloff` all have to move together.
- **`footing_cluster_ratio` is the knob that decides whether the line follows territory or
  building density**, and on the evidence so far it is the one that matters most. 100 px is about
  one Foxhole town on a 1024 px hex, with towns a couple of hundred apart, so it merges a town
  without reaching the next one. Too small and the crowd comes back; too large and two genuinely
  separate holdings become one footing halfway between them, which invents ground nobody holds.
- **`influence_radius_ratio` replaces the `influence_epsilon` this table used to name.** `ε` is in
  px², and 2500 px² is not a number anyone can look at a map and have an opinion about; 100 px is.
- **It doubled to 100 px, and stayed doubled after `influence_falloff` went back to 1.** The radius
  at which a lone structure flips the field against `m` friendly ones `s` away is
  `r² = (s² + ε)/m^(1/k) − ε`. Clustering makes that *worse*, not better — it drops `m` from tens
  to a handful and pushes `s` out — so the wider saturation earns its place independently of the
  exponent it was first raised alongside.

  Treat that formula as an upper bound rather than a prediction. It says `k = 1` at `ε = 2500`
  should already have produced islands up to 113 px wide, and the live war produced none — real
  outposts sit closer to friendly support than the worst case assumes. It is a guide to which
  direction to move a knob, not a number to tune against.
- **The halo colours are opaque, not semi-transparent as first drafted.** Segments are stroked one
  at a time and overlap at every vertex, so a translucent halo blends twice there and beads
  visibly along the line. Opaque blending is idempotent.
- **The colours are the label style**, light stroke over a dark halo. That combination is already
  shown to survive both Acrithia's pale desert and Deadlands' near-black, which is the same
  problem a line crossing 53 regions has. They stay separate fields from `text_color` all the
  same: a line is not a label, and colour is explicitly a knob here.

The reference measures 2 px on a 1024-wide hex, but it is a rough vision rather than a
specification and the user has asked for **somewhat wider**; 5 px is that, and it is a knob, not a
conclusion.

## Saying which side is which

A bare line says *where* the boundary is and nothing about *whose* ground lies either side of it.
On a hex where a player already knows the war that reads fine; on `/full-map`, or on a hex someone
opened cold, it is a stripe across a picture.

**Built: the two-tone edge.** The halo the line already sat on is now the two faction colours —
`colonial_tint` on the Colonial flank, `warden_tint` on the Warden — so the answer is carried along
the whole length of the boundary at no cost in space. It replaced the black halo rather than being
added outside it, which keeps the line the same weight it was.

- It reuses the per-segment rasteriser. Splitting a segment down the middle is the existing
  distance-to-segment coverage test plus the sign of `dx·(py−y0) − dy·(px−x0)`, so it costs a
  comparison per painted pixel, not a new renderer.
- **Which flank is which is a question the field answers, never the polyline.** A polyline's
  direction falls out of the order `chain` happened to walk the segments, and `chain` reverses
  segments freely to attach them — so the winding is an accident of iteration, not a fact about the
  war. Inferring the flank from it would be right about half the time, per polyline: correct in one
  screenshot and inverted in the next. `flanks` steps off each segment midpoint along its normal in
  both directions, evaluates `influence` at both, and records which side is the larger. The answer
  travels with the geometry in `Edge::colonial_side`, one flag per segment, and `translate` is a
  pure shift precisely so it cannot mirror them.
- Sampled on **both** sides and compared, not tested for sign on one. The line sits where `F = 0`
  and Chaikin then moves it by up to a cell, so a single probe can land back across the contour and
  read the wrong side; the difference between two only flips if the probe overshoots the far side of
  the front entirely. One field cell is the right step for the same reason — shorter is inside the
  noise, much longer starts sampling where the front has curved away.
- `frontline_halo_ratio` went 2.0 → 3.0 with it. Two thirds of the halo is now the only thing
  saying which side is which, and at 2.0 the visible band is half a line width — under a pixel once
  the full map is downscaled, which is a colour nobody can name. At 3.0 each band is about as thick
  as the line itself.
- **Losing the black halo costs less contrast than it sounds like.** Both tints are dark (luminance
  ~95 and ~90 against white's 255), so they hold the white core off pale terrain the way black did.
  Where they cannot — Deadlands, whose ground is about as dark as they are — what is left is a white
  line on near-black, which never needed a halo.
- It agrees with `faction_tint` by construction, reusing its two colours: the wash and the line now
  answer the same question at different resolutions instead of contradicting each other on a
  contested hex.
- It survives the full map's downscale for the same reason the stroke does — a band has no internal
  detail to lose — and is sized backwards through `for_full_map` along with the stroke it is a
  multiple of.

**Rejected: text along the line.** Asked for as the alternative and written up here so the fallback
stays costed, not because it is queued. Build it only if the two-tone edge turns out not to be
enough:

- Glyphs following a curve need per-glyph placement and rotation along the tangent. `ab_glyph` can
  transform an outline, but nothing in this crate does it today — every label the renderer draws is
  axis-aligned.
- It needs room. At `full_map_long_edge` 2048 a hex is about 205 px across, so a word set along the
  line inside one hex is a handful of pixels tall. Region labels are already at 19 px there and
  that is the floor of legibility.
- A cheaper variant drops the curve: place a horizontal `COLONIAL` / `WARDEN` pair, offset either
  side of the line's normal, at a few points spaced along it. No glyph rotation, no new text
  machinery, and it reads at hex scale. It does not read at full-map scale, which argues for making
  it hex-only rather than for making it curved.

Either way it is not a second setting — the guild has already opted into a frontline and should not
have to opt into being told what it means.

## Command surface

- A `guilds.frontline` boolean, set through `/set-guild-settings` alongside the faction tint, so
  scheduled reports honour it too. It reports what the guild actually has, not "unchanged" —
  the existing behaviour of that command.
- **This is a migration, so `docs/tos.md` and `docs/privacy.md` change in the same commit.**
  The stored-data list on the docs site is the schema in prose (`specs/docs-site.md`).

## External calls

- `GET /worldconquest/maps/{map}/dynamic/public` — for the rendered region **and each neighbour**
  on `/get-map`; unchanged for `/full-map`, which already fetches every region.
- No new endpoints, no new assets.

## Notes

- **Approximate is the specification, not a shortfall.** Do not add resolution, exact geometry or
  a tighter fit in the belief that it improves the feature. The input is a list of structures; a
  crisper line would only state a guess more confidently.
- Distances go in **canvas pixels**. `REGION_WIDTH` and `REGION_HEIGHT` differ (1024 × 888), so a
  radius expressed in the API's normalized coordinates is an ellipse, and the front comes out
  squashed vertically.
- **A hex with no structures at all draws nothing** — `F` is identically zero, which is not a
  contour. Guard the degenerate case explicitly rather than letting marching squares decide.
- **The reference is a rough vision, not a target.** It is one hex of Endless Shore at exactly
  1024 × 888 — the same footprint the renderer uses — which made it worth measuring, but only its
  *shape* is load-bearing: continuous, smooth, edge to edge. Its 2 px width, its black, and its
  2 px gap at the silhouette are all incidental, and the last of those is a defect to avoid.

## Acceptance criteria

- With `frontline` off, output is byte-identical to today's.
- On a contested hex, the line runs between the two sides' structures, not along the hex border,
  and reads as a single smooth curve rather than a staircase.
- The line runs edge to edge across a contested hex, including over water and empty terrain.
- **The line reaches the hex silhouette with no gap at either end**, and never appears in the
  transparent corners outside it.
- The line is legible on both the lightest and the darkest region art.
- A lone structure deep inside enemy territory does **not** produce a closed loop around itself.
- A hex held entirely by one faction, or holding nothing at all, draws no line.
- On `/full-map`, the line crosses hex seams without breaking or kinking, and is still visible at
  the finished resolution.
- `/get-map` on a border region draws the line correctly up to the hex edge.
- A neighbour region that fails to fetch produces a warning and a still-successful render.
