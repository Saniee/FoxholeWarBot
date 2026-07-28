# Context: territory tint

Spec: `specs/frontline-territory.md`. Tasks: `tasks.md`.

Branch `claude/specs-workflow-review-p0c7p3`, PR #2.

## Current state

**Shipped.** Requested 2026-07-28 off a foxholestats.com screenshot and built the same day, directly
on top of the frontline overlay (`specs/frontline.md`, `dev/done/frontline/`).

The full map's faction wash is coloured per pixel by the sign of the influence field instead of per
hex by `controlling_team`, so the wash boundary *is* the drawn line. Verified against the user's
live cache: every hex the front crosses comes out in both colours, no hex-edge steps anywhere, and
the colour seam sits under the stroke.

Part 2 — activity as the wash's intensity — was **split out unbuilt** into
`specs/active/frontline-activity.md`. Nothing about it is started, deliberately: the spec says look
at Part 1 live first.

## What it came down to

- **The wash and the line are the same curve**, because both are the zero set of one field. Nothing
  is tuned to make them agree, which is the reason to colour by the field rather than reconstruct
  the reference's sub-region polygons — a second territory model would disagree with the line in
  public.
- **`controlling_team` survived, narrowed to presence.** A hex with nothing built in it is still
  bare terrain. Presence decides whether the ground is coloured, the field decides which colour.
  Dropping the gate would let an empty corner of the map pick up a colour from the far field, where
  `F`'s sign is stable but meaningless.
- **The halo went back to black under the wash**, two-tone still on `/get-map`. Keyed off
  `faction_tint` in `RenderConfig`, no new guild setting. This closes the one caveat the frontline
  shipped with — a coloured band over a wash of the same colour barely separates from it.
- **No migration, no `docs/` change.** `full_map_faction_tint` keeps its name; only what "by
  faction" resolves to changed.

## The correctness condition, and that it held

`Field::along(y).at(x)` and `Edge::colonial_side` have to agree everywhere, or a sliver of one
faction's colour lands on the wrong side of its own frontline — which reads as inverted flanks, not
as a rounding error. Two tests pin it, and both passed first try:

- `the_wash_and_the_line_agree_about_which_side_is_whose` (frontline.rs) — numbers, walked segment
  by segment along a traced contour.
- `the_seam_between_the_two_colours_hides_under_the_stroke` (request_processing.rs) — pixels, per
  row of a rendered front, checking the stroke actually covers the column where the wash turns over.

The second is the one worth being honest about: the spec predicted the drift would hide under the
stroke and said to *measure* it rather than assume. It does hide, with room to spare.

## Cost

Measured in release on the real 10240 x 6216 composite, 53 hexes of wash: flat 0.57 s, field-coloured
**1.79 s**. About +23 ms a hex.

Two things about getting there:

- The obvious suspect — the two divisions per pixel — was **not** the cost. Caching `1/spacing`
  moved 2.13 s to 2.10 s. Hoisting the row out of the sample (`Field::along`, so `y` is resolved
  once per thousand pixels rather than per pixel) took it to 1.79 s.
- **Do not time whole composites to measure this.** Back-to-back runs of the identical unwashed
  render came out at 2.15 s and 4.08 s on this machine. `dev/done/frontline/` says the same thing
  about the overlay; the isolated pass is the only number worth writing down.

## Key files

- `src/utils/frontline.rs` — `Field::along(y) -> Row`, `Row::at(x)`. Bilinear, edge-clamped. The
  only new public surface, and split by row rather than offered as `at(x, y)` purely for the
  hoisting above.
- `src/utils/request_processing.rs` — `tint_by_field` (the per-pixel wash), `Ground` (a field plus
  the tile's world origin), `controlling_team` (unchanged, now the presence gate), `frontline_halo`.
- `src/utils/map_render.rs` — `Front { edges, field, origin }` carries the field out of both
  frontline passes; `world_frontline` now runs when `frontline || faction_tint`.

## Rendering locally

The live API is 403 through the agent proxy. The user's `cache.zip` of a live Able shard unzips into
`./cache` (gitignored by the root `/*` rule) and neither render path needs the network after that.
`dev/done/frontline/context.md` has the details.

One trap worth repeating, because it cost a render: a scratch harness reading
`cache/dynamic/Dynamic_{region.api_name}-Able.json` **misses Marban Hollow**, because the table says
`MarbanHollowHex` and the API says `MarbanHollow`. It fails silently as one bare, unwashed hex in
the middle of the map, which looks exactly like a rendering bug.
