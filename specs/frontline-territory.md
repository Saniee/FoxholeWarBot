# Territory tint

The full map's faction wash is coloured by **which side of the front the ground is on**, not by
which faction holds the hex it sits in. The boundary of the wash is the frontline the overlay
draws, so a hex the front runs through comes out part green and part blue.

Opt-in per guild through the existing `full_map_faction_tint`. No new setting, no migration.

Follows on from `specs/frontline.md`, which owns the field, the footings and the contour. This adds
no fetching, no new sources, and no second pass over the structures — it reads the field that was
already computed.

## Why not one colour per hex

`faction_tint` used to answer "who holds this hex" and paint the whole hex one colour. That is the
same region-resolution answer to a sub-region question that `specs/frontline.md` opens by rejecting,
and the two features contradicted each other on exactly the hexes that matter most: Callahans
Passage was washed Colonial green while the drawn front ran straight across it, so the picture said
"Colonial hex" and "contested here" at the same time.

It also produced the caveat the frontline shipped with. The line's halo carried a faction colour on
each flank, and under the hex wash that band sat on ground already half its own colour, so on dark
hexes the Colonial band all but disappeared. The territory tint answers that question better than
the flanks do — see "The halo" below.

## What this is not

The reference the request came with is foxholestats.com, and it is **a guideline, not a target** —
the ask was explicitly not to copy it. Two reasons to keep the distance beyond that, and the first
one decides:

- The War API does not give us what it is drawing. There are no polygons for the named sub-regions a
  hex is divided into (Solas Gorge, The Latch, …) — only their label positions, in the static half.
  Reconstructing them means inventing a Voronoi of the text items and asserting control per cell,
  which is a second territory model sitting next to the one we have.
- Two models would disagree in public. The wash and the line would answer the same question by
  different arithmetic, and every hex where they diverged would look like a bug.

The field the frontline computes *is* our territory model. Colouring by it is the version of this
feature that cannot contradict itself.

## Behavior

1. **Whether a hex is washed at all** is unchanged: `controlling_team` returning `None` — a hex with
   no faction structures the API told us about — stays bare terrain. Presence decides *whether*; the
   field decides *which*. Keeping the existing gate is what stops an empty corner of the map picking
   up a colour from the far field, where `F`'s sign is stable but meaningless.
2. **Each pixel takes the colour of the sign of `F` there**, read from the same coarse grid the
   contour was traced on and bilinearly interpolated to the pixel. Positive is Colonial. Blended at
   `faction_tint_strength` and masked by the background's own alpha — the same wash as before, with
   the colour chosen per pixel instead of per hex.
3. **The line is drawn on top**, unchanged.

When `Field::sample` returns `None` — one faction holds everything the API told us about — the wash
falls back to the per-hex `controlling_team` colour, which gives the same answer everywhere on such
a map anyway.

### Why the wash boundary lands on the line

Both are the zero set of the same interpolated field, so they coincide by construction rather than by
tuning. Two places they can still drift:

- Marching squares interpolates linearly **along cell edges** and joins those crossings with a
  straight segment, where the bilinear zero set inside a cell is a hyperbola. The two agree exactly
  wherever the contour crosses a cell boundary and differ inside a cell by well under its 32 px.
- Chaikin then moves the drawn line off the field's zero set by up to a fraction of a cell.

The stroke plus its halo is wider than either, so the seam sits under the line rather than beside it.
**This is measured, not assumed** — `the_seam_between_the_two_colours_hides_under_the_stroke` walks
every row of a rendered front, finds the column where the wash turns over, and checks the stroke
covers it. A sliver of the wrong colour along one side of the line is the failure mode, and it would
read as inverted flanks rather than as a rounding error.

### Islands

The coarse grid is what stops a lone outpost punching a closed loop of enemy colour into friendly
ground, and it goes on doing that here — the wash inherits the island suppression because it
inherits the sampling. **Interpolating to the pixel does not reintroduce them**: bilinear
interpolation cannot invent a sign change inside a cell whose four corners agree, which is the same
argument `specs/frontline.md` uses to reject an upsampled second grid.

So the wash is smoother and less blobby than the reference, by the same deliberate choice that made
the line smooth. If a fortified pocket genuinely should show as an island, that is a
`footing_cluster_ratio` or `influence_radius_ratio` conversation about the model, not a reason to
sample the wash more finely than the line.

## The halo

With the ground either side already carrying the faction colours, the flanks said a second time what
the wash says better. **The halo is
opaque black wherever the territory wash is underneath it** (`frontline_halo`), and stays two-tone
on `/get-map`, where there is no wash and the flanks are the only thing naming the sides.

Keyed off `faction_tint` in `RenderConfig`, not a new guild setting — a guild that asked for a tinted
map has not asked to choose a halo style.

## Command surface

- **No new setting and no migration.** `guilds.full_map_faction_tint` keeps its name and meaning:
  wash the full map by faction. What changed is what "by faction" resolves to, which is a rendering
  decision rather than a stored one — so `docs/tos.md` and `docs/privacy.md` do not change either.
- The wash needs the field, so **the field is computed whenever the tint is on**, whether or not
  `guilds.frontline` is. At 21 ms on the real composite that is affordable; the alternative is a
  guild that turned the tint on getting the old per-hex answer until it also turns the line on,
  which is a coupling nobody asked for.
- **`/get-map` is out of scope, and not for a rendering reason.** The setting is
  `full_map_faction_tint` and a hex render has never been washed, so extending it is a schema change
  — a new column or a rename, `migrations/`, `db.rs`, `/set-guild-settings`, and the stored-data list
  in `docs/` in the same commit. Worth doing, worth doing separately.

## External calls

None. Same fetches, same cache, same ETags as the frontline overlay.

## Notes

- **The field is world-space, the wash is per tile.** `composite_full_map` computes the contour once
  for the whole world and hands every tile its own translated copy; the field travels the same way,
  as one `Field` for the whole canvas plus each tile's grid offset (`Ground`). Per-tile sampling
  would step the wash at hex seams exactly as it would break the line.
- **Cost, measured in release on the real 10240 x 6216 composite**, 53 hexes' worth of wash:

  | | |
  |---|---|
  | flat per-hex wash (what this replaces) | 0.57 s |
  | wash coloured by the field | **1.79 s** |

  So about 23 ms a hex more than the wash already cost, on a composite whose downscale costs
  seconds. The grid is sampled once and read back — `influence` is never re-evaluated per pixel,
  which would be 48 million points against a few hundred footings. Reading it back row by row
  (`Field::along`) rather than point by point is worth about 6 ms a hex on its own, because `y` is
  constant for a thousand consecutive samples.

## Rejected

- **Voronoi of the static text items**, one cell per named sub-region, coloured by the nearest
  structure's team. This is the closest reconstruction of the reference and it is what would produce
  its blobby look. Rejected under "What this is not": it is a second territory model, it disagrees
  with the line, and the sub-region shapes would be invented rather than read.
- **Deriving the line from a sub-region wash instead**, so the two agree the other way round. That
  reopens the centring the line took three rounds of live feedback to settle, to buy a look nobody
  has asked for.
- **Sampling the wash on a finer grid than the line.** Reintroduces the islands both features exist
  to avoid, and makes the wash boundary stop being the line.

## Acceptance criteria

- Tint off ⇒ the render is byte-identical to before.
- A hex the front runs through comes out in both colours, split along the drawn line.
- A hex well behind the front is washed uniformly.
- A hex with no faction structures is not washed at all.
- No wrong-coloured sliver alongside the line — measured, per row, against the drawn stroke.
- The wash is continuous across hex seams.
- Nothing is washed in the transparent corners, for the same seam reason `tint_region` documents.
