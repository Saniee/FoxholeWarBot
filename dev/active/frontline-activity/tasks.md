# Tasks: activity as tint intensity

Spec: `specs/active/frontline-activity.md`. Context: `context.md`.

## 0. Before any of this

- [x] Look at the shipped territory tint on a live map. It reads well — the wash follows the line
      with no hex edges, and the black halo keeps the seam legible. What it does **not** do is
      distinguish a contested hex from a backline one, so the Part 2 question was real

## 1. Option C — structure density as a free stand-in

- [x] Footing count per region, already in hand at render time, as the intensity input
- [x] Carried as a second value per cell on `Field`, interpolated by the same `Field::along` pass
- [x] Map it to a strength range with a floor well above zero
- [x] Render it and look

**Answered: no, and the code reverted.** Footing count is uncorrelated with the front — both
contested hexes rank below several deep-backline ones — and its 3x spread is the flat one this spec
already rejected enlistments for. Table and the full argument in the spec, along with what the
rendering half took, so it does not have to be rediscovered.

## 2. Option B — casualty rate

**Not recommended, and not started.** C did not refute B, but it did show the squeeze is in the
rendering rather than the input: variation only becomes visible at a floor low enough that quiet
regions stop reading as held. B would pay a migration, a retention policy, a `docs/` change and a
fresh-deployment fallback to find out whether 10.6x clears that bar where 3x did not.

If it is ever wanted, the rendering half is written up in the spec — only the number is a real
question.

- [ ] History table: `(shard, region, timestamp, colonial_casualties, warden_casualties)`
- [ ] A retention policy for it, and `docs/tos.md` + `docs/privacy.md` in the **same commit** as the
      migration (`specs/docs-site.md`)
- [ ] A defined fallback for a fresh deployment

## 3. Option A — cumulative casualties

Not started. Describes the whole war rather than the present, so by late war most of the front is
dark and the picture stops discriminating.

## Checks, whichever number it is

Both of the ones that matter were written, passed, and went out with the revert. They are described
in the spec because they are the checks any redo needs, not because the code still exists.

- [x] The line does not move — the traced contour asserted *equal* before and after, not close
- [x] No hex edges in the wash — a walk between two readings, largest single step bounded against
      the total
- [x] Cost measured in release on the real canvas: **below the noise floor.** Six alternating runs
      gave 4.1–6.1 s with it off and 4.5–5.2 s with it on, which overlap — whole-composite timings
      on this canvas are not trustworthy to better than a couple of seconds
