# Context: frontline overlay

Spec: `specs/active/frontline.md`. Tasks: `tasks.md`.

Branch `claude/specs-workflow-review-p0c7p3`, PR #2 (the logging work, now closed out, is on the
same branch).

## Current state

**§1 done and pushed** (`25c2b19`): `regions::neighbours` + four unit tests. Nothing else started.
**Next is `tasks.md` §2, the field.**

Two unrelated pieces of work landed on this branch in between and both touch code this feature
will edit — read the notes below before assuming the files look like the spec describes:
`8161a36` (offline/unreachable shards) and `cc9a0cf` (full-map region names, one label style).

## Decisions already made (user's calls, don't re-ask)

- **Both `/get-map` and `/full-map`**, not just single hexes. Decided when the full map looked
  ~5 ms; the cost has since been revised up (see below) but the decision stands — a few hundred
  ms is still nothing beside the seconds Lanczos3 already spends there.
- **`/get-map` fetches its up-to-6 neighbours' dynamic data.** Approved to stop the line drifting
  at the hex edge; it turned out to be load-bearing, since without it there is no field outside
  the hex and the line cannot reach the silhouette at all.
- **Guild setting**, like `full_map_faction_tint` — which means a migration, and therefore
  `docs/tos.md` + `docs/privacy.md` in the same commit.

## Established by measurement, not assumed

- **Neighbours are derivable, no table needed** — now shipped and tested, not just believed.
  `regions.rs` is a flat-top **odd-q** offset grid (odd columns half a hex lower). Symmetric for
  all 53, degrees 2–6, reproduces real geography.
  **`REGIONS` is a `const`, so `ptr::eq` is not a valid identity test on regions** — each use
  site can be handed its own promoted copy. The symmetry test failed on its first run against
  arithmetic that was already right; compare `api_name`. Anything in §2 that wants a set or a
  lookup of regions hits the same trap.
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

## What the region-name work changed for this feature

Three things, all of them useful, one of them a decision that has to be re-made.

- **There is now a precedent for drawing at final size, after the downscale.** Region names are
  drawn on the *scaled* image because text on a 1024 px tile is resampled with the terrain and
  arrives as a smudge. A thin line has the same problem, so `/full-map`'s frontline is now an
  open choice rather than the settled one in the spec: draw on the full-res composite and size
  the width backwards (`full_map_frontline_px`, what the spec says), or scale the contour and
  stroke it at final width afterwards (simpler, and `for_full_map` needs no new field). **Decide
  this before writing §3**, and update the spec either way.
- **`draw_haloed_text` in `request_processing.rs` is the halo the spec asks for**, already
  written: a full square ring whose thickness scales with the feature (`px / 12`, min 1), not
  four cardinal offsets — those leave the diagonals bare where a stroke is thinnest. The
  frontline's halo should follow it rather than invent a second one.
- **Order on the full map is now: tiles → downscale → region names.** The line goes *under* the
  names, which is right — but it means "draw the frontline last" is no longer available on the
  full map, and `composite_full_map` has a second post-downscale step to slot into.

Also worth knowing: `text_color`/`text_outline` defaults changed (white on a dark halo), and hex
labels now drop clear of the icon they'd otherwise cover. Neither affects the frontline, but the
"off ⇒ byte-identical output" acceptance criterion is against **today's** renders, not the ones
from before those commits.

## Verifying without the API

The API is blocked from this environment (`403` through the agent proxy) and was for the whole
of the label work — which turned out not to matter, and the same trick serves the contour:

- `composite_full_map(tiles, false, &config)` with every tile `(region, None)` renders the whole
  world from local art alone.
- `place_image_info` takes hand-built `DynamicMapData` / `StaticMapData`, so structures can be
  placed exactly where a test wants them — two clusters either side of a hex is precisely the
  input the field needs to be checked against.
- Write the PNG into the scratchpad from a `#[cfg(test)]` module and look at it. **Delete the
  module before committing**; two were written and removed this way already.

This is faster than a live war for everything except the last question, "does the line sit where
a player would draw it", which genuinely needs real data (§6).

## Key files (expected)

- `src/utils/request_processing.rs` — `RenderConfig` gains the frontline ratios; `grid_offset` is
  the world-space anchor; `tint_region` is the precedent for alpha-aware compositing.
  `controlling_team` / `CONTROL_ICON_TYPES` are **neighbours of this work, not its basis** — see
  the model section for why their rule doesn't carry over.
- `src/utils/regions.rs` — **done.** `neighbours()` is there and tested.
- `src/utils/map_render.rs` — `composite_full_map` now draws tiles, downscales, *then* labels via
  `region_labels()`; `render_region` gains the neighbour fetches.
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

**Start `tasks.md` §2: the field**, and finish it before touching §3. A contour can be checked as
numbers long before it is checked as pixels, and no amount of visual tuning in §3 rescues a field
that is wrong underneath it.

Concretely: world-space points from every faction-held structure, `F = Σ w/(d²+ε)` Colonial minus
Warden, sampled coarsely, then marching squares at zero. Assert on the numbers — a hex held by one
side has no zero crossing; two clusters either side produce one crossing between them, not several
— before rendering anything. The degenerate case (no structures at all) is a guard, not a thing
to let marching squares decide.

Then §3, whose first act is the draw-before/draw-after decision above. Leave §5 (migration +
`docs/`) whole — it is one commit by repo rule.
