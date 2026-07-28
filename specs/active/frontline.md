# Frontline overlay

**Status: planned.** This describes intended behavior. Promote to `specs/` when it ships.

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

### `k`, and why it is not 1

`k = 1` — plain inverse-square — is what shipped to the first live war, and it put the line
visibly onto the Colonial side of the ground it was meant to bisect. The cause is arithmetic.
Set one structure at distance `a` against `n` clustered ones at distance `b` and solve
`1/a^(2k) = n/b^(2k)`: the balance sits at `b/a = n^(1/(2k))`.

| structures in the cluster | ground it wins at `k = 1` | at `k = 2` |
|---|---|---|
| 9 | 3.0 x a lone base's reach | 1.7 x |
| 40 (a densely built hex) | 6.3 x | 2.5 x |

So at `k = 1` the contour is not a boundary between two territories, it is a **density** line, and
whichever side had built more per acre took the difference. A bunker line of thirty icons outvoted
a town across the river. Measured on the fixture in `a_crowd_does_not_buy_ground` — one base
against a cluster of nine, 600 px apart — the crossing sat 150 px off centre at `k = 1` and sits
82 px off at `k = 2`.

The exponent is a dial between the two rejected models below, not a free improvement. As `k` grows
the field approaches "whoever is nearest", which *is* the Voronoi partition, islands and all; and
the far field vanishes faster, which is what keeps the line steady across the long quiet stretches
where a hex holds almost nothing. `k = 2` is one notch, taken deliberately rather than two.

One property worth stating because it is not obvious: the *sign* of `Σ w/(d²+ε)^k` is the sign of
the difference between the two sides' `p`-norm soft-minimum distances at `p = 2k`. Raising `k`
therefore moves the zero contour toward the true medial axis between the two point sets — the
"centre between the footholds" — rather than just sharpening a line that was already in the wrong
place.

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

**Every faction-held structure**, not just `CONTROL_ICON_TYPES`.

This deliberately departs from `controlling_team`: the tint **counts** structures and takes a
majority, where a field weighs them by distance, so the ones deep in friendly ground matter far
less than the ones on the boundary. Excluding non-control structures would throw away most of the
evidence of a footing.

**The original wording of this section claimed the failure mode the tint guards against "does not
exist here" — that a structure deep in friendly territory contributes nothing. That was wrong, and
the first live war is what showed it.** Weighting by distance reduces a crowd's influence; it does
not remove it. Under `k = 1` a sufficiently large crowd two hundred metres back still outweighed a
lone base fifty metres forward, which is the same "whoever built more wins" defect the tint has,
arriving by a slower route. Raising `k` is what actually bounds it, and the bound is `n^(1/(2k))`
rather than nothing — so this is a matter of degree, and a live war remains the only place the
degree can be judged.

If `k = 2` still leaves the line leaning, the next lever is this section rather than the exponent:
thin the point set so a bunker line counts as one footing instead of thirty, either by restricting
to `CONTROL_ICON_TYPES` or by collapsing clusters. That attacks `n` at the source, and unlike
raising `k` it does not cost far-field stability.

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

     So 1/32 is the default and no further trick is needed — a quarter of a second sits inside
     the seconds the full map already spends resampling 63.6 MP. Tracing the contour is free at
     every resolution; the field is the whole cost, and it is linear in both cells and structures.
     If a war ever has enough structures to matter, drop to 1/16 before reaching for truncation
     and spatial binning.
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
3. **Draw.** Scale the polylines to canvas space and stroke at `frontline_width_ratio`, in
   `frontline_color`, with the wider `frontline_halo` laid down first so the line survives on the
   dark hexes (Deadlands, Umbral Wildwood) where a dark line on dark terrain would vanish.
4. **Run the line to the hex edge, then mask.** The field is sampled over an area **larger than
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
| `influence_falloff: u32` | the `k` of `w / (d² + ε)^k` | `2` |
| `frontline_color` | stroke | white |
| `frontline_halo` | wider under-stroke | opaque black |
| `frontline_halo_ratio: f32` | halo width as a multiple of the line | `2.0` |

Five of these carry a reason beyond taste:

- **`field_resolution_ratio` is not a free knob.** An earlier draft suggested ¼ of a region; 1/32
  is both affordable (255 ms on the real composite) and the value the island behaviour above
  depends on. It, `influence_radius_ratio` and `influence_falloff` all have to move together.
- **`influence_radius_ratio` replaces the `influence_epsilon` this table used to name.** `ε` is in
  px², and 2500 px² is not a number anyone can look at a map and have an opinion about; 50 px is.
- **It doubled to 100 px when `influence_falloff` went to 2, and the two are coupled.** The radius
  at which a lone structure flips the field against `m` friendly ones `s` away is
  `r² = (s² + ε)/m^(1/k) − ε`: raising `k` widens that hole, raising `ε` closes it. Left at 50,
  `k = 2` opens a 105 px island on plausible numbers — comfortably resolved by the 32 px grid,
  which is the loop the whole model exists to avoid.

  Treat that formula as an upper bound rather than a prediction. It says `k = 1` at `ε = 2500`
  should already have produced islands up to 113 px wide, and the live war produced none — real
  outposts sit closer to friendly support than the worst case assumes. It is a guide to which
  direction to move a knob, not a number to tune against.
- **`frontline_halo` is opaque, not semi-transparent as first drafted.** Segments are stroked one
  at a time and overlap at every vertex, so a translucent halo blends twice there and beads
  visibly along the line. Opaque blending is idempotent.
- **The colours are the label style**, light stroke over a dark halo. That combination is already
  shown to survive both Acrithia's pale desert and Deadlands' near-black, which is the same
  problem a line crossing 53 regions has. They stay separate fields from `text_color` all the
  same: a line is not a label, and colour is explicitly a knob here.

The reference measures 2 px on a 1024-wide hex, but it is a rough vision rather than a
specification and the user has asked for **somewhat wider**; 5 px is that, and it is a knob, not a
conclusion.

## Saying which side is which — **planned, not built**

A bare line says *where* the boundary is and nothing about *whose* ground lies either side of it.
On a hex where a player already knows the war that reads fine; on `/full-map`, or on a hex someone
opened cold, it is a stripe across a picture. This section is gated on the centring work above
landing — a legend on a line that sits in the wrong place is worse than no legend.

**Decided: A, the two-tone edge.** Both were asked for and both are written up below, because the
argument for A is the reason to keep B available if it turns out not to be enough. B is not queued
work — do not build it speculatively.

**A. A two-tone edge.** The stroke already lays a wider halo down before the line. Give that halo a
faction colour per side — `colonial_tint` on the Colonial flank, `warden_tint` on the Warden — and
the answer is carried along the entire length of the boundary at no cost in space. This is the
recommended default:

- It reuses the existing per-segment rasteriser. A band is the same distance-to-segment coverage
  test with the sign of the offset taken into account, so it costs one extra pass, not a new
  renderer.
- **Which flank is which is a question the field already answers.** The polyline's own direction is
  arbitrary — it falls out of the order `chain` happened to walk the segments — so orientation must
  not be inferred from it. Step off the segment midpoint along its normal in both directions and
  evaluate `influence` there: positive is Colonial. Two evaluations per segment against a few
  hundred segments is free next to the field itself.
- It survives the full map's downscale for the same reason the stroke does: a band has no internal
  detail to lose. It does need sizing backwards through `for_full_map` like everything else.
- It agrees with `faction_tint` by construction if it reuses the same two colours, and the two
  features then answer the same question at different resolutions rather than contradicting each
  other on a contested hex.

**B. Text along the line.** More explicit, and the only option that survives being screenshotted
without a caption. Costs more:

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

A is continuous, cheap, scale-free and needs no space; B is a per-location annotation that has to
compete with icons and region names for room. If B ever does ship it is not a second setting — the
guild has already opted into a frontline and should not have to opt into being told what it means.

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
