# Context: frontline overlay

Spec: `specs/active/frontline.md`. Tasks: `tasks.md`.

Branch `claude/specs-workflow-review-p0c7p3`, PR #2 (the logging work, now closed out, is on the
same branch).

## Current state

**§1 and §2 done.** `regions::neighbours` (`25c2b19`), and `src/utils/frontline.rs` — the whole
field-and-contour model with 11 unit tests, no rendering in it. **Next is `tasks.md` §3, drawing**,
whose first act is the draw-before/draw-after decision below.

The §2 surface, all of it world-space pixels in and out:
`sources_in(region, dynamic, config) -> Vec<Source>` · `Bounds::around_region(region, config,
margin)` · `Field::sample(sources, bounds, spacing, epsilon) -> Option<Field>` ·
`Field::value_at(x, y)` · `contour(&Field) -> Vec<Polyline>` · `smooth(lines, rounds)`.
`spacing` and `epsilon` are parameters rather than config fields on purpose — §3 wires
`field_resolution_ratio` and `influence_epsilon` to them, and the module stays testable without a
`RenderConfig` full of drawing knobs.

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
- **Cost — benchmarked, no longer an estimate.** Real canvas is 10240 × 6216. With 2000
  structures, release build: 1/16 sampling 61 ms, **1/32 255 ms**, 1/64 965 ms. Contour and
  smoothing are under a millisecond at every one of those, so **the field is the entire cost** and
  it is linear in cells and in structures. 1/32 is the default and the spec's truncation +
  spatial-binning fallback is **not needed** — don't build it. Debug builds are far slower; the
  Dockerfile ships `--release`, so the numbers above are the ones that matter.
- **The "lone outpost makes no island" criterion is delivered by the coarse grid, not the field.**
  Within ~√ε of an isolated structure `F` really does cross zero — its own `w/ε` term wins locally.
  The crossing is a few tens of pixels wide and the samples are hundreds of pixels apart, so it
  falls between them. Sampling finely would *create* the islands the model was chosen to avoid.
  Written into `contour`'s doc comment and the spec; do not "fix" it by raising resolution.
- **Chaikin measured**: on a bending fixture, worst turn 0.183 → 0.105 rad over two rounds while
  total turning is unchanged (0.6897 → 0.6898). That pair is the test — it distinguishes smoothing
  from straightening, which a single "is it smoother" assertion does not.

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
- `src/utils/frontline.rs` — **done.** The field and the contour, and nothing else: it imports
  `RenderConfig` only for `grid_offset`, and has no idea a canvas exists. Keep drawing out of it.
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

**Start `tasks.md` §3: drawing**, and settle the full-map draw order before writing any of it —
that choice decides whether `RenderConfig` needs `full_map_frontline_px` at all.

The field is done and trustworthy; §3 is the part that can only be judged by eye. So render early
and render locally (see "Verifying without the API" above) rather than reasoning about stroke
widths — the spec's 5 px and its colour are explicitly starting points, and the whole tuning loop
is meant to happen against a picture.

One `epsilon` question §3 has to answer that §2 deliberately left open: the tests use 2500 px²
(≈50 px), which is a guess that made the fixtures behave, **not** a finding. It sets how close to
a lone structure the field will flip, so it interacts directly with the island behaviour above.
Pick it against a real render.

Leave §5 (migration + `docs/`) whole — it is one commit by repo rule.
