# Context: territory tint

Spec: `specs/active/frontline-territory.md`. Tasks: `tasks.md`.

Branch `claude/specs-workflow-review-p0c7p3`, PR #2.

## Current state

**Spec written, nothing built.** Requested 2026-07-28 off a foxholestats.com screenshot, with the
scope clarified twice in the same conversation — first that it is a guideline rather than something
to copy, then that activity should drive the tint's *intensity* rather than anything about the
line.

Follows on directly from the frontline overlay (`specs/frontline.md`, `dev/done/frontline/`), which
shipped the same day.

## The shape, in three lines

- **Part 1:** the wash is coloured by the sign of the influence field per pixel, not by which
  faction holds the hex. The boundary of the wash is the drawn line, because both are the zero set
  of the same field.
- **Part 2:** per-region activity scales `faction_tint_strength` up and down. Colour from Part 1,
  intensity from Part 2.
- **Neither touches where the line sits.**

## Decisions already made

- **Not a reconstruction of foxholestats.** Its sub-hex colouring needs sub-region polygons the API
  does not publish; inventing them is a second territory model that would visibly disagree with the
  line. The influence field is the territory model we already have.
- **Activity feeds intensity, never the field.** Weighting footings by activity would move the
  line, and the centring took three rounds to settle. This was the user's own framing and it is the
  safer one — keep it even if a weighted field looks tempting later.
- **No new guild setting and no migration for Part 1.** `full_map_faction_tint` keeps its name; what
  changes is what "by faction" resolves to. So no `docs/tos.md` / `docs/privacy.md` change either.
- **The two-tone halo goes back to black where the wash is underneath it**, and stays two-tone on
  `/get-map` where there is no wash. Keyed off `faction_tint` in `RenderConfig`, not a new setting.

## The finding that decides Part 2

**The War API publishes no player counts.** The reference's `470/hr` figures are *casualties*, and
the rates are derived by polling `/worldconquest/warReport/{map}` over time and differencing. We
already model and cache `WarReport { total_enlistments, colonial_casualties, warden_casualties,
day_of_war, version }` per region — so cumulative totals are free, and rates need history we do not
store (a table, a retention policy, and a docs change).

`totalEnlistments` is **not** a live player count — a contested hex reads 22,656 against 470,272
casualties, ~21 deaths per enlistment, so it is cumulative like the rest of that response.

**It is per-region, settled by two samples**, against a reasonable-sounding argument that it would
be war-global. Callahans-shaped hex 22,656, Deadlands 10,354, `version` 135 against 101, while
`dayOfWar` is 500 in both — so the payload mixes scopes, with enlistments and casualties per-region
and `dayOfWar` global. The endpoint being per-hex proves nothing on its own; that was the trap.

**Use casualties as the intensity anyway.** The same two samples:

| | enlistments | casualties |
|---|---|---|
| contested hex | 22,656 | 470,272 |
| Deadlands, behind the line | 10,354 | 44,468 |
| ratio | 2.2x | 10.6x |

Casualties separate a hot hex from a quiet one ~5x more sharply, and dynamic range is the whole
point of a signal that drives visible wash strength. Caveats: cumulative, not rates; two regions,
chosen as extremes.

Cross-check worth keeping: Deadlands sits behind the Colonial line in our own render of this war,
the reference shows it at `0/hr`, and its casualties are a tenth of the contested hex's. The signal
tracks the front, which is what Part 2 assumes.

Spec's recommendation: ship Part 1, look at it, and only then try structure density as a free
stand-in before paying for the polling history.

## Key files

- `src/utils/frontline.rs` — `Field` is private inside; Part 1 needs a `pub fn at(&self, Point)
  -> f32`, bilinear, edge-clamped. The one new piece of public surface.
- `src/utils/request_processing.rs` — `tint_region` (whole-canvas wash, becomes per-pixel),
  `controlling_team` (stays, as the presence gate), `Paint`/`draw_frontline` (halo colour choice).
- `src/utils/map_render.rs` — `composite_full_map` computes the contour once for the world and
  translates per tile; the `Field` has to travel the same way or the wash breaks at hex seams.

## Next step

Part 1, step 1: expose bilinear sampling on `Field` and pin it with a test that the sign it reports
agrees with `Edge::colonial_side` along a traced contour. That check is the whole feature's
correctness condition, and it is cheaper to write before the rendering than after.

## Rendering locally

The live API is 403 through the agent proxy. The user has a `cache.zip` of a live Able shard; unzip
it into `./cache` (gitignored by the root `/*` rule) and neither render path needs the network.
`dev/done/frontline/context.md` has the details — including that `faction_tint` must be **off** for
a hex render, since it is full-map only and leaving it on produces a washed hex that looks nothing
like `/get-map`.
