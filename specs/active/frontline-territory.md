# Territory tint, and activity as its intensity

**Status: planned.** This describes intended behavior. Promote to `specs/` when it ships.

Two pieces, at very different levels of confidence, kept in one spec because the second one is a
modifier on the first:

- **Part 1, the territory tint** — the line divides the wash, instead of hex borders doing it.
  Decided, buildable as written, needs no data we do not already have.
- **Part 2, activity as the wash's intensity** — busier regions wash darker. Worth costing, and the
  honest answer is that the data it wants does not exist in the form the reference implies.

**They compose in one direction only.** Part 1 decides which colour a pixel gets; Part 2 decides
how strong it is. Neither moves the frontline itself, which was signed off after three rounds of
live feedback and is not what either part is for.

# Part 1 — Territory tint

## Summary

Wash the full map in each faction's colour by **which side of the front the ground is on**, instead
of by which faction holds the hex it sits in. The boundary of the wash is the frontline the
overlay already draws, so a hex the front runs through comes out part green and part blue.

Opt-in per guild through the existing `full_map_faction_tint`. No new setting, no migration.

## Why the hex wash is the wrong resolution

`faction_tint` answers "who holds this hex" and paints the whole hex one colour. That is the same
region-resolution answer to a sub-region question that `specs/frontline.md` opens by rejecting — and
the two features now contradict each other on exactly the hexes that matter most. Rendering the
live cache showed it plainly: Callahans Passage is washed Colonial green while the drawn front runs
straight across it, so the picture says "Colonial hex" and "contested here" at the same time.

It also produced the one caveat the frontline shipped with. The line's halo carries a faction
colour on each flank, and under the hex wash that band sits on ground already half its own colour;
on the dark hexes the Colonial band all but disappears. Territory tint answers that question
better than the flanks do, which is why this is less a new feature than the other half of the one
that just shipped — see "What happens to the two-tone halo".

## What this is not

The reference the request came with is foxholestats.com, and it is **a guideline, not a target** —
the ask was explicitly not to copy it. Two reasons to keep our distance beyond that, and the first
one decides:

- The War API does not give us what it is drawing. There are no polygons for the named sub-regions
  a hex is divided into (Solas Gorge, The Latch, …) — only their label positions, in the static
  half. Reconstructing them means inventing a Voronoi of the text items and asserting control per
  cell, which is a second territory model sitting next to the one we have.
- Two models would disagree in public. The wash and the line would answer the same question by
  different arithmetic, and every hex where they diverged would look like a bug.

The field the frontline already computes *is* our territory model. Colouring by it is the version
of this feature that cannot contradict itself.

## Behavior

The field, the footings and the contour are all computed exactly as `specs/frontline.md` describes
— this adds no new fetching, no new sources, and no second pass over the structures.

1. **Decide whether a hex is washed at all**, unchanged: `controlling_team` returning `None` — a
   hex with no faction structures the API told us about — stays bare. Presence decides *whether*;
   the field decides *which*. Keeping the existing gate is what stops an empty corner of the map
   picking up a colour from the far field, where `F`'s sign is stable but meaningless.
2. **Colour each pixel by the sign of `F` there**, sampled from the same coarse grid the contour
   was traced on and **bilinearly interpolated** to the pixel. Positive is Colonial. Blend at
   `faction_tint_strength`, masked by the background's own alpha — the same wash `tint_region`
   already applies, with the colour chosen per pixel instead of per hex.
3. **Draw the line on top**, unchanged.

### Why the wash boundary lands on the line

Both are the zero set of the same interpolated field, so they coincide by construction rather than
by tuning. Two places they can still drift, both expected to be invisible under a stroke:

- Marching squares interpolates linearly **along cell edges** and joins those crossings with a
  straight segment, where the bilinear zero set inside a cell is a hyperbola. The two agree exactly
  wherever the contour crosses a cell boundary and differ inside a cell by well under its 32 px.
- Chaikin then moves the drawn line off the field's zero set by up to a fraction of a cell.

The stroke plus its halo is wider than either. **Measure it before assuming so** — a sliver of the
wrong colour along one side of the line is the failure mode, and it would look like the flank
colouring is inverted rather than like a rounding error.

### Islands, again

The coarse grid is what stops a lone outpost punching a closed loop of enemy colour into friendly
ground, and it goes on doing that here — the wash inherits the island suppression because it
inherits the sampling. **Interpolating to the pixel does not reintroduce them**: bilinear
interpolation cannot invent a sign change inside a cell whose four corners agree, which is the same
argument `specs/frontline.md` uses to reject the upsampled second grid.

So the wash will be smoother and less blobby than the reference, by the same deliberate choice that
made the line smooth. If a fortified pocket genuinely should show as an island, that is a
`footing_cluster_ratio` or `influence_radius_ratio` conversation about the model, not a reason to
sample the wash more finely than the line.

## What happens to the two-tone halo

With the ground either side already carrying the faction colours, the flanks say a second time what
the wash says better, and they say it at the low contrast measured in `dev/done/frontline/`.
**Put the halo back to opaque black wherever the territory wash is underneath it**, and keep it
two-tone on `/get-map`, where there is no wash and the flanks are the only thing naming the sides.

This is a `RenderConfig` decision keyed off `faction_tint`, not a new guild setting — a guild that
asked for a tinted map has not asked to choose a halo style.

## Command surface

- **No new setting and no migration.** `guilds.full_map_faction_tint` keeps its name and meaning:
  wash the full map by faction. This changes what "by faction" resolves to, which is a rendering
  decision rather than a stored one — so `docs/tos.md` and `docs/privacy.md` do not change either.
- The wash needs the field, so the field is computed whenever the tint is on, whether or not
  `guilds.frontline` is. At 21 ms on the real composite that is affordable; the alternative is a
  guild that turned the tint on getting the old per-hex answer until it also turns the line on,
  which is a coupling nobody asked for.
- **`/get-map` is out of scope for now, and not for a rendering reason.** The setting is
  `full_map_faction_tint` and a hex render has never been washed, so extending it is a schema
  change — a new column or a rename, `migrations/`, `db.rs`, `/set-guild-settings`, and the
  stored-data list in `docs/` in the same commit. Worth doing, worth doing separately, and worth
  seeing the full map land first.

## External calls

None. Same fetches, same cache, same ETags as the frontline overlay.

## Notes

- **Cost is a full-canvas pass that already exists.** `tint_region` touches every pixel of every
  tile today (~48 MP across the 53); this adds a bilinear lookup and a sign test per pixel and
  removes nothing. Expect tens of milliseconds on the composite, against the 21 ms field and the
  seconds the downscale costs. Measure in release, on the real canvas, and put the number in this
  spec — the frontline's numbers are all measured and this one should be too.
- **The field is world-space, the wash is per tile.** `composite_full_map` computes the contour
  once for the whole world and hands every tile its own translated copy; the field has to travel
  the same way. It is one `Field` for the whole canvas, so tiles read from it rather than each
  sampling their own — per-tile sampling would break the wash at hex seams exactly as it would
  break the line.
- **`Field::sample` returns `None` when one side holds everything.** The wash then falls back to
  today's per-hex `controlling_team`, which on a map one faction holds outright gives the same
  answer everywhere anyway.
- `Field`'s internals are private and its interpolation is not exposed yet. This needs a
  `pub fn at(&self, point: Point) -> f32` or a `sign_at`, bilinear, clamped at the field's edge —
  the one new piece of public surface in `utils::frontline`.

## Rejected

- **Voronoi of the static text items**, one cell per named sub-region, coloured by the nearest
  structure's team. This is the closest reconstruction of the reference and it is what would
  produce its blobby look. Rejected under "What this is not": it is a second territory model, it
  disagrees with the line, and the sub-region shapes would be invented rather than read.
- **Deriving the line from a sub-region wash instead**, so the two agree the other way round. That
  reopens the centring the line took three rounds of live feedback to settle, to buy a look nobody
  has asked for.
- **Sampling the wash on a finer grid than the line.** Reintroduces the islands both features exist
  to avoid, and makes the wash boundary stop being the line.

## Acceptance criteria

- Tint off ⇒ the render is byte-identical to today's.
- A hex the front runs through comes out in both colours, split along the drawn line.
- A hex well behind the front is washed uniformly, as it is today.
- A hex with no faction structures is not washed at all.
- No wrong-coloured sliver alongside the line: sample both sides of the stroke along its length and
  check the wash agrees with `Edge::colonial_side` at every segment.
- The wash boundary is continuous across hex seams — the defect that per-tile sampling would cause,
  and the reason the field is computed once for the world.
- Nothing is washed in the transparent corners, for the same seam reason `tint_region` already
  documents.

# Part 2 — Activity as tint intensity

## The idea

Part 1 decides *which* colour a pixel gets. This decides *how strong* it is: a region where the war
is actually being fought washes darker, a quiet backline washes faint. The line still divides the
tint exactly as Part 1 describes — **activity never moves the boundary, only the intensity either
side of it.**

That separation is what makes this safe to want. Feeding activity into the field instead would
change where the line sits, and the centring took three rounds of live feedback to settle; feeding
it into `faction_tint_strength` cannot move the line by a pixel. Build it that way round even if a
weighted field looks tempting later.

## The problem: there are no player counts

**The War API publishes no player counts, per region or anywhere else.** The numbers on the
reference that look like activity — `470/hr`, `492/hr`, `220k 250k` — are casualties, and the
per-hour figures are rates that site derives by **polling over time and differencing**. The
underlying endpoint is one this bot already calls:

`GET /worldconquest/warReport/{map}` → `WarReport { total_enlistments, colonial_casualties,
warden_casualties, day_of_war, version }`, per region.

`totalEnlistments` is **not** a live player count, which is the first thing anyone checks it for.
It is a cumulative counter: a hex holds players in the low hundreds, and a real sample reads 22,656
against 470,272 casualties in the same region — about 21 deaths per enlistment, which is a ratio
accumulated over a war rather than a snapshot of one. The casualty fields in that same sample match
the reference screenshot's per-hex `220k 250k` pair exactly, which is what confirms those tiles are
war-report casualties and not something we cannot get.

**Whether `totalEnlistments` is per-region or war-global is unsettled**, and one request answers it:
fetch a second region's report and compare. Identical means global — plausible, since `dayOfWar`
and `version` in the same response certainly are. Different means per-region.

If it is per-region, **differenced over time it is the better activity signal of the two**, and the
one to reach for first. Enlistments per hour measures people arriving; casualties per hour measures
people dying, which over-reads a stalemate where two sides feed a meat grinder for days without
moving, and under-reads a front collapsing so fast nobody is dying to hold it. Same polling cost,
same history table — so the choice between them is free once either is being stored.

So what is actually available is:

- **Per-region cumulative casualties per faction, right now** — one fetch per region, already
  modelled in `api_definitions::foxhole`, already cached by `/war-report`. Free. Cumulative
  enlistments come in the same response, at no extra cost, subject to the scope question above.
- **Rates** — only by keeping history. Two polls of every region and the difference between them,
  which is state this bot does not store today.

That distinction is the whole cost of this part.

## What each option would actually cost

**A. Cumulative casualties as the intensity.** No new storage. Fetch 53 war reports on a full-map
render — the fan-out already exists for the dynamic half and this is the same shape.

The objection is that cumulative totals describe *the whole war*, not the present. A region fought
over for a week and quiet since keeps its number forever, so the map would darken over wherever the
war *has been* rather than where it is. By late war most of the front is dark and the picture stops
discriminating. **Usable as a first cut precisely because it is cheap and cannot mislead about
territory** — it is only ever intensity — but it answers a different question than the one asked.

**B. A rate, by differencing polls — enlistments if they are per-region, casualties otherwise.**
The kind of number the reference actually shows, and the one that means "where the war is right
now". It needs history: a table of `(shard, region, timestamp,
colonial_casualties, warden_casualties)`, written by a background poll or on each render, read back
as a delta over a window.

That is a migration, a retention policy for rows nobody has asked to keep, and a `docs/tos.md` +
`docs/privacy.md` change in the same commit (`specs/docs-site.md`). It also makes the render depend
on the bot having been running for the length of the window: a fresh deployment draws a flat map
for its first hours, and that needs a defined fallback rather than being discovered in a screenshot.

**C. Structure density as a stand-in.** Free, already in hand, needs nothing new — the footings
count per region is a fair proxy for "how much is going on here", and it is data the render already
holds. Not the same quantity as activity, and it would make heavily-built quiet regions dark. Worth
one experiment before paying for B, because the experiment costs an afternoon and the answer might
be "close enough".

## The thing to get right: this reintroduces hex edges

Part 1's whole point is that the wash stops being hex-shaped. **A per-region intensity puts hex
boundaries straight back into it** — not as colour flips this time, but as steps in strength, which
on a large flat wash are just as visible.

So the intensity has to be a *field*, not a lookup:

- Sample the per-region figure at each hex centre, then interpolate across the map the way the
  influence field is interpolated — so a region twice as busy as its neighbour reads as a gradient
  between them rather than a visible hexagon.
- The influence field's own grid is the obvious carrier: it already spans the world in world space,
  it is already interpolated per pixel for Part 1, and a second value per cell costs one more
  `Vec<f32>` and no extra sampling passes.
- Clamp the mapped strength to a range with a floor well above zero. A quiet region must still read
  as *held*, or Part 1's answer disappears wherever nobody is fighting — which is most of the map,
  most of the time.

## Recommendation

**Ship Part 1 first and look at it.** It is the piece that was actually asked for, it needs no new
data, and it may well be enough on its own — a wash that follows the front is a much bigger change
to how the map reads than a wash that varies in strength.

Then, if intensity is still wanted: **C to find out whether the effect is worth having, B to do it
properly, A only if cheapness beats accuracy.** All three are the same rendering change downstream
of a different number, so the experiment is not wasted whichever way it goes.
