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

Not a plain Voronoi over bases. In Foxhole ground is held by proximity to a claimed town base, so
the model is a **union of discs**:

- Every control structure (`CONTROL_ICON_TYPES`: 45, 46, 47, 56, 57, 58 — town and relic bases)
  held by a faction projects a disc of `claim_radius_ratio × REGION_WIDTH` around itself.
- A point is Colonial or Warden if its nearest control structure *within that radius* belongs to
  that faction, and neutral if there is none.
- The frontline is the contour where a Colonial region meets a Warden one. Where either meets
  neutral wilderness there is **no line** — that is a border with nobody, not a front.

Reusing `CONTROL_ICON_TYPES` is deliberate and its existing rationale carries over intact:
refineries, storage and garrisons record where a faction has *been*, not what it holds, and a
frontline drawn from them bulges around whoever built more sheds. The one difference from the tint
is that the tint counts non-control structures as a **tiebreak**; the frontline must not, because
a tiebreak has no position — it is a fact about a whole hex, and this overlay is asking a question
about a point.

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

1. **Classify.** Rasterize a label grid (`Colonial` / `Warden` / `Neutral`) at
   `field_resolution_ratio` of the target footprint — coarse on purpose, because the contour is
   smoothed by step 2 and not by the grid.
   - `/get-map`: ~57k cells for one hex. Naive nearest-neighbour over the region's own control
     structures plus its neighbours' is a few million float ops — sub-millisecond.
   - `/full-map`: ~1M cells at ⅛ of the 63.6 MP composite, against ~800 world-wide control
     structures. Naive would be ~800M ops; a **two-pass chamfer distance transform** is O(cells),
     so ~5 ms and ~5 MB. The naive path must not be used here.
2. **Contour.** Marching squares over the label grid, Colonial-vs-Warden edges only, producing
   polylines in grid space. Smoothness comes from the polyline, not from grid resolution.
3. **Draw.** Scale the polylines to canvas space and stroke them at
   `frontline_width_ratio × REGION_WIDTH`, in `frontline_color`, with a darker `frontline_outline`
   underneath so the line reads over both factions' terrain and over the tint.

**Order: after the background and the tint, before the icons.** The tint is a wash and would
swallow the line; the icons are the map's actual content and a line drawn over them costs
legibility for decoration. This mirrors the existing rule that the tint goes on before the icons.

On `/full-map` the line is drawn on the full-resolution composite and then scaled down with
everything else, so it antialiases for free — the same reason the icons are sized backwards from
the finished image rather than drawn small.

## Configuration

Every constant is a ratio of the region footprint in `RenderConfig`, never a literal in the
compositing code:

| Field | Meaning |
|---|---|
| `frontline: bool` | draw it at all |
| `claim_radius_ratio: f32` | how far a control structure holds ground |
| `field_resolution_ratio: f32` | label-grid cell size |
| `frontline_width_ratio: f32` | stroke width |
| `frontline_color`, `frontline_outline` | stroke and its backing |

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

- **Neutral is a real answer.** A hex neither side has reached gets no line at all, and the
  contour must not be closed around wilderness to make a tidier shape.
- **The overlay is only as good as the control-point set.** Validate against a live war before
  tuning anything: if the line looks wrong, the first suspect is `claim_radius_ratio`, and the
  second is whether the API reported a base at all.
- Discs are drawn in **canvas pixels**, and `REGION_WIDTH`/`REGION_HEIGHT` differ (1024 × 888).
  A "radius" is therefore an ellipse in normalized item coordinates; do the distance maths in
  canvas space, not in the API's normalized space, or the front will be squashed vertically.

## Acceptance criteria

- With `frontline` off, output is byte-identical to today's.
- On a contested hex, the line runs between the two sides' town bases, not along the hex border.
- On `/full-map`, the line crosses hex seams without breaking or kinking.
- A hex held entirely by one faction, or held by nobody, draws no line.
- `/get-map` on a border region draws the line correctly up to the hex edge.
- A neighbour region that fails to fetch produces a warning and a still-successful render.
- `/full-map` render time grows by single-digit milliseconds, not seconds.
