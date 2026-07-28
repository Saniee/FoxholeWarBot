# Tasks: territory tint

Spec: `specs/active/frontline-territory.md`. Context: `context.md`.

Ordered so each step is checkable before the next depends on it. Part 2 is deliberately not
scheduled — see §5.

## 1. Sampling the field at a point

- [ ] `Field::at(&self, point: Point) -> f32` — bilinear, clamped at the field's edge so a pixel
      just outside the sampled bounds reads the nearest cell rather than panicking
- [ ] Test: on a fixture with a known front, `at` agrees in sign with `Edge::colonial_side` at every
      segment of the traced contour. **This is the feature's correctness condition** — if the wash
      and the line disagree anywhere, this is where it shows, and it is far cheaper to catch here
      than in a render
- [ ] Test: `at` reproduces the grid values exactly at cell corners

## 2. The wash follows the field

- [ ] `tint_region` gains a per-pixel colour: keep the whole-canvas alpha-masked blend, choose the
      colour by `field.at(world_pixel)` instead of taking one for the hex
- [ ] Keep `controlling_team` as the gate — a hex with no faction structures stays bare. Presence
      decides whether, the field decides which
- [ ] Fall back to today's per-hex wash when `Field::sample` returns `None` (one faction holds
      everything). Same answer everywhere in that case anyway, but it must not panic
- [ ] The `Field` travels from `composite_full_map` to each tile the way the contour already does —
      **one field for the world**, or the wash steps at every hex seam

## 3. Checks that would catch it looking wrong

- [ ] Tint off ⇒ byte-identical to today (the frontline has the same criterion; same method)
- [ ] A hex the front crosses comes out in both colours; a hex behind the front is uniform; a hex
      with nothing in it is bare
- [ ] **No wrong-coloured sliver beside the line.** Sample both sides of the stroke along its whole
      length and check the wash agrees with the flank. The spec expects the drift to hide under the
      stroke — measure it rather than assuming
- [ ] Nothing washed in the transparent corners
- [ ] Cost measured in release on the real 10240 x 6216 canvas, and the number written into the
      spec — every other figure in the frontline specs is measured, this one should be too

## 4. Rendering it and looking at it

- [ ] Render the full map from the cached war and look at Callahans Passage, Marban Hollow and
      Westgate — the hexes the front actually crosses
- [ ] Halo back to black under the wash, still two-tone on `/get-map`. Check both in one sitting;
      it is one config decision and two renders
- [ ] Spec + `docs/` unchanged is a *claim* — confirm no migration crept in

## 5. Part 2 — not scheduled

Do not start without a live look at Part 1 first. The order if it is wanted, from the spec:

- [ ] Structure density as a free stand-in, to find out whether varying intensity is worth having
      at all before paying for data
- [ ] Casualty **rate** if it is — history table, retention policy, and `docs/tos.md` +
      `docs/privacy.md` in the same commit
- [ ] Cumulative casualties only if cheapness beats accuracy; they describe the whole war rather
      than the present, and by late war most of the front is dark
- [ ] Whichever number it is, it has to be interpolated across the map, not looked up per hex —
      a per-region intensity puts hex edges straight back into a wash whose entire point is not
      having them
