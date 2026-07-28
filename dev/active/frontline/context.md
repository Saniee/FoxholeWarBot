# Context: frontline overlay

Spec: `specs/active/frontline.md`. Tasks: `tasks.md`.

Branch `claude/specs-workflow-review-p0c7p3`, PR #2 (the logging work, now closed out, is on the
same branch).

## Current state

**§1–§5 done and pushed; §6 is under way against real feedback.** `/set-guild-settings` has a
`frontline` option, off by default, honoured by `/get-map`, `/full-map` and both branches of the
scheduled tick. Migration `0006` runs at startup. 20 tests; clippy clean but for the two
pre-existing warnings (`schedule_report.rs:24` too many arguments, `db.rs:202` large enum variant).

Commits: `25c2b19` neighbours · `1e6909b` field · `90e0ddc` drawing · `fa44c0a` neighbour fetch ·
`8c17de0` toggle · `fafaba7` state · **falloff (this one)**.

**Round 1 of live-war feedback is in, and the first triage entry below was wrong.** The user sent a
full map and a Callahans Passage hex: the line leaned onto the Colonial side, badly — measuring the
hex, the middle and right of the line sat 30–55 px from Colonial structures and 130–200 px from
Warden ones, so roughly a quarter of the way across instead of half.

The cause was not the point set, which is what the list below sent me to look at first. It was the
**exponent**. `F = Σ w/(d²+ε)` lets `n` clustered structures hold ground `√n` times as far out as a
lone base, so the contour was a *density* line, not a boundary — the denser side simply took the
middle. Raised to `w/(d²+ε)²` (`influence_falloff: 2`), which makes it `n^(1/4)`; measured on the
`a_crowd_does_not_buy_ground` fixture the crossing moved from 150 px off centre to 82 px.
`influence_radius_ratio` doubled to 100 px in the same change because `k` and `ε` are coupled —
see the spec's Configuration notes for the formula and for why it overstates the risk.

**This corrected a claim the spec had asserted outright** ("a structure deep in friendly territory
contributes nothing to where the boundary sits"). It is false; distance weighting reduces a crowd's
pull, it does not remove it. That section of the spec now says so.

**Next: the user is sending an example of how the line should sit.** Read it against "What a live
war has to answer", which is now ordered correctly. If `k = 2` is still not centred enough, the two
remaining levers, in order: thin the point set (attacks `n` at source, costs no far-field
stability), then `k = 3` (measured at 52 px off centre on the same fixture, but the far field
vanishes faster and long quiet stretches may start to wobble — that is why this round took one
notch and not two).

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

## Key files

- `src/utils/request_processing.rs` — **done.** The frontline ratios are on `RenderConfig` with
  accessors (`frontline_width`, `field_spacing`, `influence_epsilon`, `frontline_margin`);
  `draw_frontline` / `stroke` do the painting. `grid_offset` is the world-space anchor;
  `tint_region` is the precedent for alpha-aware compositing.
  `controlling_team` / `CONTROL_ICON_TYPES` are **neighbours of this work, not its basis** — see
  the model section for why their rule doesn't carry over.
- `src/utils/regions.rs` — **done.** `neighbours()` is there and tested.
- `src/utils/frontline.rs` — **done.** The field and the contour, and nothing else: it imports
  `RenderConfig` only for `grid_offset`, and has no idea a canvas exists. Keep drawing out of it.
- `src/utils/map_render.rs` — **done.** `world_frontline` (whole-map contour) and
  `region_frontline` (one hex + its neighbours) both end by calling `translate` into tile pixels.
  `fetch_dynamic` is the dynamic-half-only fetch.
- `src/utils/db.rs`, `migrations/0006_frontline.sql`, `src/commands/set_guild_settings.rs`,
  `docs/` — **done.** Four render sites read `guild.frontline`: `/get-map`, `/full-map`, and both
  branches of the cron tick.

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
so a structure deep in friendly ground contributes nothing to where the boundary sits. **This is
the judgement call most likely to be wrong, and it is what shipped** — it is item 1 on the live-war
list below, and the first thing to change if the line sits wrong.

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

## What a live war has to answer

Read incoming feedback against this list before changing anything — **the order matters**, because
three of these cannot be fixed by the knob a symptom first suggests.

1. **"The line isn't centred — it leans toward one side."** → **`influence_falloff` first.** This
   is the entry round 1 got wrong: it sent me to the point set, and the answer was the exponent.
   A crowd of `n` holds ground `n^(1/(2k))` times as far out as a lone base, so at `k = 1` the
   contour tracked *density* rather than territory and the better-built side took the middle.
   Now 2. If it still leans, thin the point set (`CONTROL_ICON_TYPES` only, or collapse clusters —
   `Source.weight` already carries a magnitude and only its sign is used today) **before** going to
   `k = 3`: thinning attacks `n` at source and costs no far-field stability, where raising `k`
   again trades against cases 3 and 5.

   **Do not touch `field_resolution_ratio` or the smoothing rounds for this** — they cannot fix a
   field that is leaning, and raising resolution actively makes case 3 worse.
2. **"Too thick / too thin / wrong colour."** → `frontline_width_ratio` (hex),
   `full_map_frontline_px` (finished full map), `frontline_color` / `frontline_halo`. Pure taste,
   change freely. Note the two widths are independent: the full map's is sized backwards in
   `for_full_map`, so changing the hex one does nothing to the world map.
3. **"There are little loops / blobs around isolated bases."** → **raise
   `influence_radius_ratio`** (now 100 px), or lower resolution, or lower `influence_falloff`.
   Never raise resolution: the islands are real zero crossings that the coarse grid is filtering
   out. This is the one counter-intuitive knob in the feature. It is also the price of case 1 —
   `k` and `ε` move together, which is why the radius doubled when the falloff did.
4. **"The line is jagged / staircased."** → `frontline::SMOOTHING_ROUNDS` (2). Cheap.
5. **"It breaks at hex seams."** → a real bug, not tuning. The field is computed once in world
   space, so a seam break means either the per-tile `translate` or the alpha mask, not the model.
6. **"It's missing near one edge of a hex on `/get-map`."** → look for the neighbour-fetch warning
   in the log naming that region. Degrading at one edge when a neighbour fails is by design.

## Next steps

**§6.** Take the user's live-war results, triage with the list above, then finish:

- `cargo clippy` clean (the two pre-existing warnings excepted) — currently true, keep it.
- Re-check the acceptance criteria in the spec. Three are already measured and recorded:
  off ⇒ byte-identical, the line reaching the silhouette with no gap (13 edge pixels painted at
  each end), and nothing landing in the transparent corners.
- Promote `specs/active/frontline.md` → `specs/`, update `specs/README.md` (it says `specs/active/`
  is empty as of 2.0 — that changes), and move `dev/active/frontline/` → `dev/done/`.

Whatever the feedback changes, **update the spec in the same commit** — it is meant to be 1:1 with
the code, and **four** of its original claims have now needed correcting (cost estimate, the island
mechanism, the bilinear upsample, and "a structure deep in friendly territory contributes nothing").

**Still unvalidated, and it needs a live war, not another synthetic render:**

- **Whether `influence_falloff: 2` is enough.** Round 1's fixture says it halves the error, not that
  it eliminates it — 82 px off centre on a 600 px gap. The user's incoming example is what settles
  this. `k = 3` measures 52 px on the same fixture and is a one-line change.
- `influence_radius_ratio` = 100 px, doubled with the falloff and still a calculated guess rather
  than a finding. It and `field_resolution_ratio` govern the island behaviour.
- Width (5 px on a hex, 3 px on the finished full map) and colour. The synthetic render shows the
  mechanism works, not that the numbers are right. **The user has not complained about these**, on
  either image, which is weak evidence they are close enough.
- **"Every structure" versus `CONTROL_ICON_TYPES`.** Demoted from "most likely thing to be wrong"
  to the second lever for case 1 — the exponent turned out to matter more — but a bunker line still
  counts thirty times, and that is a real distortion whatever `k` is.
