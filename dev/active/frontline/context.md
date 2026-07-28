# Context: frontline overlay

Spec: `specs/active/frontline.md`. Tasks: `tasks.md`.

Branch `claude/specs-workflow-review-p0c7p3`, PR #2 (the logging work, now closed out, is on the
same branch).

## Current state

**Spec written, no code yet.** Awaiting review of the spec before implementation starts.

## Decisions already made (user's calls, don't re-ask)

- **Both `/get-map` and `/full-map`**, not just single hexes — the full map costs ~5 ms with a
  distance transform, so cheapness removed the reason to skip it.
- **`/get-map` fetches its up-to-6 neighbours' dynamic data**, so the line is correct at the hex
  edge rather than drifting where it matters most.
- **Guild setting**, like `full_map_faction_tint` — which means a migration, and therefore
  `docs/tos.md` + `docs/privacy.md` in the same commit.

## Established by measurement, not assumed

- **Neighbours are derivable, no table needed.** `regions.rs` is a flat-top **odd-q** offset grid
  (odd columns half a hex lower). The six-neighbour arithmetic in the spec was checked against the
  shipped table: symmetric for all 53 regions, degrees 2–6, and it reproduces real geography
  (Deadlands ↔ Callahans Passage, Umbral Wildwood, Linn of Mercy, Loch Mór, Marban Hollow,
  Drowned Vale). This belongs in a unit test.
- **Measured off the user's reference render** (one hex of Endless Shore, exactly 1024 × 888 —
  the same footprint the renderer uses): 2 px wide, plain black, unbroken edge to edge, smooth
  (max slope change 1.0 px/px, mean 0.39), stopping ~2 px inside the hex silhouette.
  **Only the shape is binding.** The user has since said it is a rough vision: width should be
  somewhat larger (start ~5 px), colour is open, and the gap at the silhouette is a defect —
  the line must run all the way to the edge.
- **Cost.** One hex: ~57k cells, order 100 points, sub-millisecond. Full map: the influence field
  is O(cells × points) and does **not** reduce to a distance transform — 1M cells × ~2000
  structures is 2×10⁹ ops. Sample at ~1/32 (~62k cells, ~10⁸ ops, a few hundred ms) and truncate
  influence with spatial binning if that isn't enough.

## Key files (expected)

- `src/utils/request_processing.rs` — `CONTROL_ICON_TYPES` and `controlling_team` already exist
  and are the model to reuse; `RenderConfig` gains the frontline ratios; `grid_offset` is the
  world-space anchor.
- `src/utils/regions.rs` — neighbour arithmetic from `(col, row)`.
- `src/utils/map_render.rs` — `composite_full_map` draws at full res *before* the downscale, which
  is where the line goes; `render_region` gains the neighbour fetches.
- `src/utils/db.rs`, `migrations/`, `src/commands/set_guild_settings.rs`, `docs/` — the toggle.

## The model, and two rejected ones

**Approximate is the target** — the user's words: "roughly split where each faction has a footing".
The reference itself "isn't that accurate", and precision would be false precision, since the API
reports structures rather than held ground.

So: an **influence field** `F(p) = Σ w/(d²+ε)` over Colonial minus the same over Warden, contoured
at zero. Smooth by construction, and robust to outliers.

- **Union of discs** — rejected. A radius cutoff breaks the line into segments across the quiet
  stretches of a front; the reference runs unbroken over open water.
- **Nearest-neighbour / Voronoi** — rejected. Decided by the single closest structure, so one
  forward base deep in enemy ground punches out an island and the overlay grows a closed loop.

**Footing = every faction-held structure**, not `CONTROL_ICON_TYPES`. The tint's objection to
sheds doesn't transfer: the tint *counts* and takes a majority, where a field *sums by distance*,
so a structure deep in friendly ground contributes nothing to where the boundary sits. This is the
judgement call most likely to be wrong — check it against a live war first.

## Traps worth not rediscovering

- `REGION_WIDTH` 1024 ≠ `REGION_HEIGHT` 888, so a radius in the API's normalized coordinates is an
  ellipse. Do the distance maths in canvas pixels.
- **Oversample the field past the hex, then mask by the background's alpha.** Both halves: the
  mask keeps the stroke out of the transparent corners (hex art is see-through there so tiles
  interlock, and painting it shows on the full map's seams — the trap `tint_region` documents),
  and the oversampling is what makes the line reach the silhouette *exactly*. Terminating the
  polyline at the hex bounds instead leaves a stroke-width gap at both ends. On `/get-map` this
  is what the neighbour fetch is for — without it there is no field outside the hex to trace.
- **Size the width backwards from the finished full map** (`full_map_frontline_px`), like
  `full_map_icon_px`. A 2 px ratio applied to the 63.6 MP composite vanishes in the downscale —
  the exact bug that once made the whole map "look like bare terrain".
- Draw after the tint, before the icons — same rule the tint already follows.

## Next steps

Get the spec reviewed. Then implement in the order in `tasks.md`, starting with the neighbour
arithmetic and its test, which is the one piece that can be verified with no rendering at all.
