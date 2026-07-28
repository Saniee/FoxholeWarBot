# Tasks: activity as tint intensity

Spec: `specs/active/frontline-activity.md`. Context: `context.md`.

## 0. Before any of this

- [ ] Look at the shipped territory tint on a live map. It may be enough on its own, and everything
      below is speculative until that is checked

## 1. Option C — structure density as a free stand-in

The cheap experiment that answers "is varying intensity worth having at all", before paying for
data. Nothing here is a commitment to ship it.

- [ ] Footing count per region, already in hand at render time, as the intensity input
- [ ] Carried as a second value per cell on `Field`, interpolated by the same `Field::along` pass —
      **not** looked up per hex, or the wash grows hex edges again
- [ ] Map it to a strength range with a floor well above zero: a quiet region still has to read as
      *held*, or Part 1's answer disappears across most of the map
- [ ] Render it and look. If the effect is not worth having here, it is not worth a migration either

## 2. Option B — casualty rate, only if C says yes

- [ ] History table: `(shard, region, timestamp, colonial_casualties, warden_casualties)`
- [ ] A retention policy for it, and `docs/tos.md` + `docs/privacy.md` in the **same commit** as the
      migration (`specs/docs-site.md`)
- [ ] A defined fallback for a fresh deployment, which has no window yet and would otherwise draw a
      flat map for its first hours

## 3. Option A — cumulative casualties

Only if cheapness beats accuracy. Free and needs no storage, but describes the whole war rather than
the present, so by late war most of the front is dark and the picture stops discriminating.

## Checks, whichever number it is

- [ ] The line does not move. Byte-compare the frontline against a render with intensity off
- [ ] No hex edges in the wash — the failure this whole feature has to avoid
- [ ] Cost measured in release on the real canvas, and the number written into the spec
