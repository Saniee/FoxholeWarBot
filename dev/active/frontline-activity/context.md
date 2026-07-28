# Context: activity as tint intensity

Spec: `specs/active/frontline-activity.md`. Tasks: `tasks.md`.

Branch `claude/specs-workflow-review-p0c7p3`, PR #2.

## Current state

**Option C built, rendered, rejected. B and A not started and not recommended.** Part 2 of the tint
request; Part 1 shipped the same day (`specs/frontline-territory.md`,
`dev/done/frontline-territory/`).

Part 1 decides *which* colour a pixel gets. This decides *how strong* it is: a region where the war
is being fought washes darker, a quiet backline washes faint.

In the code, off by default and reachable from no command: `Field::with_activity` /
`Row::activity` in `frontline.rs`, `activity_readings` in `map_render.rs`,
`faction_tint_activity` + `faction_tint_activity_floor` in `RenderConfig`. Kept rather than
reverted because B would rebuild exactly it; the only thing that would change is the number going
in. With no readings, `Row::activity` returns 1.0 and the wash is arithmetic identical to shipped.

**The finding, in one line:** footing count does not track the front (both contested hexes rank
below deep-backline ones) and its 3x spread is invisible at a safe floor. Turning the floor down far
enough to see it bleaches the quiet regions — so the tradeoff lives in the rendering, not the input,
and even a good signal has to clear that bar. Full table in the spec.

## The three things that decide this

1. **Activity feeds intensity, never the field.** Weighting footings by activity would move the
   line, and the centring took three rounds of live feedback. This was the user's own framing.
2. **The War API has no player counts.** The reference's `470/hr` figures are casualty *rates*,
   derived by polling and differencing. Cumulative per-region casualties are free (already modelled
   and cached); rates need history we do not store — a table, a retention policy, and a
   `docs/tos.md` + `docs/privacy.md` change in the same commit.
3. **Casualties, not enlistments.** Both are per-region (settled against a plausible argument that
   enlistments would be war-global). Casualties carry ~5x the contrast between a hot hex and a quiet
   one, and dynamic range is the whole point of a signal driving visible wash strength. The table is
   in the spec.

## The trap

A per-region intensity **puts hex edges straight back** into a wash whose entire point is not having
them — as steps in strength rather than colour flips, which on a large flat wash are just as
visible. Whatever the number is, it has to be interpolated across the map, not looked up per hex.
`Field` already spans the world and is already sampled per pixel; a second `Vec<f32>` on it is the
obvious carrier.

## Next steps

**None. Stop unless asked.** The territory tint answers the question that was asked and the drawn
line already marks where the fighting is. Reopen only if a live map is looked at and a flat wash is
what is missing from it — in which case the work is option B's number, not its rendering.

## Rendering locally

The live API is 403 through the agent proxy. The user's `cache.zip` of a live Able shard unzips into
`./cache` (gitignored by the root `/*` rule); neither render path needs the network after that.
`dev/done/frontline/context.md` has the details, `dev/done/frontline-territory/context.md` the
Marban Hollow filename trap that silently leaves one hex bare.
