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
  asked for. A field sums, so an isolated outpost makes a bump that doesn't cross zero.

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
   - `/full-map`: this is the one that needs care. `F` is a sum over *all* points, so it is
     O(cells × points) and does not reduce to a distance transform the way a nearest-neighbour
     partition would. At ~2000 world structures, a 1M-cell grid is 2×10⁹ operations — too slow.
     Sample coarser (~1/32 of the composite, ~62k cells → ~10⁸ ops, a few hundred ms) and, if
     that is still too slow, truncate each point's influence at a radius and bin the points into
     a spatial grid so each cell only sums its neighbourhood.
   - Interpolate the sampled field up rather than sampling finely. Bilinear is enough; the
     contour in step 2 runs on the interpolated field.
2. **Contour.** Marching squares on `F = 0`, producing polylines in grid space, then one or two
   rounds of Chaikin smoothing. Measured on the reference: its line changes slope by at most
   1.0 px per px and 0.39 on average, so the target is a gentle curve with no staircase.
3. **Draw.** Scale the polylines to canvas space and stroke at `frontline_width_ratio`, in
   `frontline_color` — plain black, matching the reference. `frontline_halo` is a lighter,
   wider stroke laid down first, so the line survives on the dark hexes (Deadlands, Umbral
   Wildwood) where black-on-black would vanish; the reference never had to solve this because it
   is a single light-terrain hex.
4. **Clip to the hex.** Mask the stroke by the background's own alpha. Region art is transparent
   in the hex corners so the tiles interlock, and a line drawn into that gap paints over the seam
   between hexes on the full map — the same trap `tint_region` already documents for the wash.
   Measured on the reference: its line stops within 2 px of the silhouette at both ends.

**Order: after the background and the tint, before the icons.** The tint is a wash and would
swallow the line; the icons are the map's actual content and a line drawn over them costs
legibility for decoration. This mirrors the existing rule that the tint goes on before the icons.

On `/full-map` the line is drawn on the full-resolution composite and then scaled down with
everything else, so it antialiases for free.

**Width must be sized backwards from the finished image**, exactly as `full_map_icon_px` already
does for icons. The reference line is **2 px on a 1024-wide hex** (median of 664 sampled columns;
ratio ≈ 0.002). Applied as a ratio on the 63.6 MP composite and then scaled down, that line
disappears completely — the same bug the full-map renderer already hit once with icons, where
"the whole map came back looking like bare terrain".

## Configuration

Every constant is a ratio of the region footprint in `RenderConfig`, never a literal in the
compositing code:

| Field | Meaning | Starting value |
|---|---|---|
| `frontline: bool` | draw it at all | `false` |
| `field_resolution_ratio: f32` | field sample spacing | ¼ of a region |
| `frontline_width_ratio: f32` | stroke width | `2.0 / REGION_WIDTH`, from the reference |
| `full_map_frontline_px: f32` | stroke width wanted in the *finished* full map | ~2 |
| `frontline_color` | stroke | black |
| `frontline_halo` | wider under-stroke for dark hexes | light, semi-transparent |
| `influence_epsilon: f32` | the `ε` in `w / (d² + ε)`, keeping `F` finite on top of a structure | — |

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
- The reference render the design was measured against is one hex of Endless Shore at exactly
  1024 × 888 — the same footprint the renderer uses — which is what made "2 px" a directly usable
  number rather than something to eyeball.

## Acceptance criteria

- With `frontline` off, output is byte-identical to today's.
- On a contested hex, the line runs between the two sides' structures, not along the hex border,
  and reads as a single smooth curve rather than a staircase.
- The line runs edge to edge across a contested hex, including over water and empty terrain.
- The line is clipped to the hex silhouette and never appears in the transparent corners.
- A lone structure deep inside enemy territory does **not** produce a closed loop around itself.
- A hex held entirely by one faction, or holding nothing at all, draws no line.
- On `/full-map`, the line crosses hex seams without breaking or kinking, and is still visible at
  the finished resolution.
- `/get-map` on a border region draws the line correctly up to the hex edge.
- A neighbour region that fails to fetch produces a warning and a still-successful render.
