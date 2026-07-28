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
F(p) = Σ  w / (d² + ε)   over Colonial structures
     − Σ  w / (d² + ε)   over Warden structures
```

The frontline is the contour `F(p) = 0`; `F` positive is Colonial ground, negative is Warden.

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

This deliberately departs from `controlling_team`, and the reason its rationale doesn't transfer is
worth stating: the tint **counts** structures and takes a majority, so a faction that built more
sheds inflates its total and miscolours a whole hex. A field **sums by distance**, so a structure
deep in friendly territory contributes nothing to where the boundary sits — only the ones near it
move the line. The failure mode the tint is guarding against does not exist here, and excluding
non-control structures would throw away most of the evidence of a footing.

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
   - Interpolate the sampled field up rather than sampling finely. Bilinear is enough; the
     contour in step 2 runs on the interpolated field.
2. **Contour.** Marching squares on `F = 0`, producing polylines in grid space, then one or two
   rounds of Chaikin smoothing. The reference changes slope by at most 1.0 px per px and 0.39 on
   average, so the target is a gentle curve with no staircase.
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

On `/full-map` the line is drawn on the full-resolution composite and then scaled down with
everything else, so it antialiases for free.

**Width must be sized backwards from the finished image**, exactly as `full_map_icon_px` already
does for icons. Applied as a ratio on the 63.6 MP composite and then scaled down, a hairline
disappears completely — the same bug the full-map renderer already hit once with icons, where
"the whole map came back looking like bare terrain".

## Configuration

Every constant is a ratio of the region footprint in `RenderConfig`, never a literal in the
compositing code. **The starting values are starting points, not findings** — width and colour in
particular are meant to be tuned by eye against a live render:

| Field | Meaning | Starting value |
|---|---|---|
| `frontline: bool` | draw it at all | `false` |
| `field_resolution_ratio: f32` | field sample spacing | ¼ of a region |
| `frontline_width_ratio: f32` | stroke width | `5.0 / REGION_WIDTH` |
| `full_map_frontline_px: f32` | stroke width wanted in the *finished* full map | ~3 |
| `frontline_color` | stroke | dark, high-contrast |
| `frontline_halo` | wider under-stroke, for dark terrain | light, semi-transparent |
| `influence_epsilon: f32` | the `ε` in `w / (d² + ε)`, keeping `F` finite on top of a structure | — |

The reference measures 2 px on a 1024-wide hex, but it is a rough vision rather than a
specification and the user has asked for **somewhat wider**; 5 px is that, and it is a knob, not a
conclusion. Colour is likewise open — black is what the mock happened to use, not a requirement,
and the halo exists so that whatever colour is chosen still reads on both light and dark terrain.

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
