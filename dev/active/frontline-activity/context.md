# Context: activity as tint intensity

Spec: `specs/active/frontline-activity.md`. Tasks: `tasks.md`.

Branch `claude/specs-workflow-review-p0c7p3`, PR #2.

## Current state

**Nothing built.** Part 2 of the tint request; Part 1 shipped the same day
(`specs/frontline-territory.md`, `dev/done/frontline-territory/`).

Part 1 decides *which* colour a pixel gets. This decides *how strong* it is: a region where the war
is being fought washes darker, a quiet backline washes faint.

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

## Before starting

**Look at the shipped territory tint live first.** The spec says so, and it may be enough on its
own. Then option C (structure density, free) to find out whether varying intensity is worth having
at all, before paying for the polling history option B needs.

## Rendering locally

The live API is 403 through the agent proxy. The user's `cache.zip` of a live Able shard unzips into
`./cache` (gitignored by the root `/*` rule); neither render path needs the network after that.
`dev/done/frontline/context.md` has the details, `dev/done/frontline-territory/context.md` the
Marban Hollow filename trap that silently leaves one hex bare.
