# Context: frontline overlay

Spec: `specs/active/frontline.md`. Tasks: `tasks.md`.

Branch `claude/specs-workflow-review-p0c7p3`, PR #2 (the logging work, now closed out, is on the
same branch).

## Current state

**§1–§4 done.** `regions::neighbours` (`25c2b19`), `src/utils/frontline.rs` (the field and
contour, no rendering in it), the stroke, and both commands wired — `/full-map` composites the
world's contour per tile, `/get-map` fetches its up-to-six neighbours' dynamic data first.
19 tests, clippy clean but for the two pre-existing warnings.
**Next is `tasks.md` §5, the toggle — one commit, migration + `db.rs` + `/set-guild-settings` +
`docs/tos.md` + `docs/privacy.md`.** Until it lands, `frontline` defaults to `false` and nothing
sets it, so the feature is unreachable in production; that is deliberate, not an oversight.

**The full-map draw-order question is settled: draw before the downscale**, as the spec always
said. The region-name precedent does not transfer — what the downscale destroys is *internal*
detail (glyph counters, stroke gaps) and a stroke has none. Two constraints also rule the
alternative out outright: icons are drawn per tile, so drawing afterwards puts the line over every
one of them; and the alpha mask stops existing once the tiles are composited and their transparent
corners are filled in by neighbours. Written into the spec. Do not reopen.

The `frontline` module's surface, all of it world-space pixels in and out:
`sources_in(region, dynamic, config) -> Vec<Source>` · `Bounds::around_region(region, config,
margin)` · `Field::sample(sources, bounds, spacing, epsilon) -> Option<Field>` ·
`contour(&Field) -> Vec<Polyline>` · `smooth(lines, rounds)`.
`spacing` and `epsilon` are parameters rather than config fields on purpose — the config wires
`field_spacing()` and `influence_epsilon()` to them, and the module stays testable without a
`RenderConfig` full of drawing knobs.

The drawing side is `draw_frontline(canvas, lines, config)` in `request_processing.rs`, taking
polylines already translated into **that canvas's** pixels; `map_render::translate` does the
shift, because the grid is the caller's business. Coverage comes from distance-to-segment rather
than a scanline, so it antialiases for free and costs work proportional to the line's length
rather than the canvas's area.

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

All three of these are now resolved; kept because the reasoning is what stops them being reopened.

- **The precedent for drawing after the downscale did not carry over**, and §3 draws before it as
  originally specced. See the Current state section for why — in short, the downscale destroys
  internal detail, and a stroke has none.
- **The halo follows `draw_haloed_text`'s spirit, not its implementation.** Text needs a square
  ring because glyphs have thin strokes in every direction; a polyline just needs a wider stroke
  of the same shape underneath, which is what `stroke` does at `frontline_halo_ratio`. The one
  rule that did transfer is the colour: light over dark, the style the user asked for everywhere.
- **Order on the full map is: tiles (background → tint → frontline → icons) → downscale → region
  names.** The line goes under the icons and under the names, which is right on both counts.

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
  the exact bug that once made the whole map "look like bare terrain". Done in `for_full_map`,
  with a test that asserts the round trip.
- Draw after the tint, before the icons — same rule the tint already follows. Done.
- **A tile drawn as bare terrain still needs the line.** `composite_full_map` has two paths, and
  the `None` one calls `load_background` directly — it needs its own `draw_frontline` call or the
  front breaks wherever a region failed to fetch.
- **The dynamic and static halves being independently revalidated is what permits the neighbour
  fetch.** They already had separate ETags and separate cache files, so `fetch_dynamic` /
  `save_dynamic_cache` are a split of the existing model rather than a shortcut around it, and
  `/get-map` costs 7 dynamic requests instead of 14 of everything.
- **Don't measure the overlay by timing whole renders.** A 63.6 MP composite varies by ~150 ms
  run to run, which swamps it. §2's isolated field benchmark is the number to quote.

## Next steps

**Start `tasks.md` §4: the `/get-map` neighbour fetch.** `render_region` currently passes an
empty slice of polylines with a comment saying why — a single hex's line is wrong at its own edges
without its neighbours' structures, so wiring it from the region's own data alone would put a
confidently wrong line on the map. `regions::neighbours` and `Bounds::around_region` are both
written and waiting; the dead-code warnings on them clear when §4 lands.

Then §5 (migration + `docs/`) whole — it is one commit by repo rule.

**Still unvalidated, and it needs a live war, not another synthetic render:**

- `influence_radius_ratio` = 50 px. It decides how close to a lone structure the field flips, so it
  and `field_resolution_ratio` are the pair that governs the island behaviour. 50 is a guess that
  made the fixtures behave.
- Width (5 px on a hex, 3 px on the finished full map) and colour. The synthetic render shows the
  mechanism works, not that the numbers are right.
- **"Every structure" versus `CONTROL_ICON_TYPES`** — still the single most likely thing to be
  wrong, and §6 says to try that before touching resolution or smoothing.
