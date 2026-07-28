# Tasks: territory tint

Spec: `specs/frontline-territory.md`. Context: `context.md`.

All done. Part 2 was split into `specs/active/frontline-activity.md` unbuilt — see §5.

## 1. Sampling the field at a point

- [x] Bilinear, edge-clamped sampling on `Field`. Shipped as `Field::along(y) -> Row` + `Row::at(x)`
      rather than the `at(point)` first planned: the wash walks a row at a time, and hoisting `y`
      out of the sample is worth ~6 ms a hex
- [x] Test: it agrees in sign with `Edge::colonial_side` at every segment of a traced contour.
      **The feature's correctness condition**, and it passed first try
- [x] Test: it reproduces the grid values exactly at cell corners
- [x] Test: sampling outside the field reads the nearest cell instead of panicking

## 2. The wash follows the field

- [x] `tint_by_field` — the whole-canvas alpha-masked blend kept intact, colour chosen per pixel
- [x] `controlling_team` kept as the presence gate: presence decides whether, the field decides which
- [x] Falls back to the per-hex wash when `Field::sample` returns `None`
- [x] One field for the world, carried to each tile by `Front` + `Ground` with the tile's own origin

## 3. Checks that would catch it looking wrong

- [x] Tint off ⇒ unchanged (the `faction_tint` guard, same shape as the frontline's
      `off_draws_nothing`)
- [x] A hex the front crosses comes out in both colours; a hex behind the front is uniform; a hex
      with nothing in it is bare
- [x] **No wrong-coloured sliver beside the line** — measured per row against the drawn stroke, not
      assumed. It hides with room to spare
- [x] Nothing washed in the transparent corners
- [x] Cost measured in release on the real canvas and written into the spec: 0.57 s → 1.79 s over
      53 hexes

## 4. Rendering it and looking at it

- [x] Full map rendered from the cached war; Callahans Passage, Marban Hollow and Deadlands all
      split correctly, no seams at hex edges
- [x] Halo black under the wash, still two-tone on `/get-map`
- [x] No migration crept in — `docs/` untouched, as claimed

## 5. Part 2 — split out, not built

Moved to `specs/active/frontline-activity.md`. The order it records, unchanged: structure density as
a free stand-in first, casualty **rate** if it proves worth having, cumulative casualties only if
cheapness beats accuracy — and whichever number it is, interpolated across the map rather than
looked up per hex, or it puts hex edges straight back into a wash whose whole point is not having
them.
