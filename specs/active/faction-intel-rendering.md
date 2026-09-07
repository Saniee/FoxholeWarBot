# Faction intel — location parsing and pin rendering

**Status: spec only, blocked on one open verification — see "Blocking" below.** Parent spec:
`specs/active/faction-intel.md` (the reporting system this renders for). Design history:
`dev/active/faction-intel/`.

## Summary
Turns a player-submitted location — Foxhole's own in-game grid reference, or a named landmark — into
a pin on the bot's rendered maps, reusing the existing icon-placement pipeline unchanged.

## Why the in-game grid, not an invented picker
Discord has no "tap a point on an image" component — buttons and selects are discrete, modals are
text-only, so any in-Discord picker necessarily quantizes to a coarse choice (a button grid, a
landmark list). Foxhole already has a precise one that players use constantly: a lettered/numbered
"big square" subdivided into a 3×3 numeric "keypad" for the small square (e.g. `G9k3`), and
`Ctrl+LMB` on the in-game map screen copies that exact string to the clipboard for pasting into
chat — existing muscle memory for arty calls and callouts, nothing new to teach. Taking that string
as a text field beats inventing a new coordinate system players would have to learn twice.

## Blocking
Two facts needed before the conversion formula below can be filled in, both deferred to the user's
own local/in-game testing rather than further desk research:
- **The grid's lettering/numbering range per hex.** Whether it's fixed across all 53 regions (a
  single formula, matching `WorldExtent` being one constant across every map per `warapi`'s
  README) or varies — expected to be fixed, not yet confirmed.
- **The exact string `Ctrl+LMB` puts on the clipboard** — a bare token (`G9k3`) or something with
  surrounding text. Decides how tolerant the parser in "Nudging" below needs to be.

Nothing past this point can be implemented until both are known. The rest of this spec describes
the shape the solution takes; only the formula's constants are missing.

## Conversion: grid reference → normalized `(x, y)`
Every region is the same in-game size (`WorldExtent` is one fixed constant across all maps,
confirmed in `clapfoot/warapi`'s README) — so one formula covers all 53 regions, not a per-region
lookup table. Once the grid's dimensions are known:
1. Parse the letter+number "big square" into a coarse `(col, row)` pair.
2. Parse the keypad digit (1–9) into a 3×3 offset within that square (`1`–`9` reading like a phone
   keypad: `1` top-left, `9` bottom-right, or whichever orientation testing confirms).
3. Combine into a normalized `(x, y)` in `[0, 1]` — the same space `MapItem`/`MapTextItem` already
   use (`specs/architecture.md` → Response types).

## Landmark fallback
When a player has no exact grid string (the modal's grid-reference field left blank per
`specs/active/faction-intel.md`'s submission flow), a select menu of that region's cached
`MapTextItems` (already fetched and cached for `draw_text` — no new data source) stands in,
capped at 25 entries per Discord's select-menu limit. The chosen landmark's own `(x, y)` — already
known, since it's a real map text item — becomes the report's coordinate directly. Coarser than a
grid reference, but zero new parsing logic: it's the same field every `MapTextItem` already
carries.

## Nudging players toward a good grid string
Three layers, cheapest first:
1. **Tolerant parsing** — the parser extracts the grid token from whatever got pasted rather than
   requiring an exact match. The most important of the three; a good parser means most players
   never need to read the other two.
2. **Teach at the point of failure** — an unparseable string gets a rejection with a worked example
   and the `Ctrl+LMB` instruction, same pattern as the invalid-`at_time` message on
   `/schedule-report`.
3. **Label/placeholder text on the modal field itself** (`specs/active/faction-intel.md`'s
   submission modal) — visible before anyone gets it wrong.

A dedicated `/report-help` command (mirroring `/schedule-help`'s always-ephemeral role) is
deliberately not in v1 — only worth adding if (1) and (2) turn out not to be enough once real
players try this.

## Pin rendering
Reuses `src/utils/request_processing.rs::place_image_info`'s existing
`place()`-at-normalized-ratio mechanism **unchanged** — a report pin is just another icon fed
through the same pipeline a `MapItem` already goes through:
- One new icon per category (or a shared pin icon with a category-coded color/badge — a rendering
  choice, not an architectural one).
- Placed at the report's `(x, y)` the same way a `MapItem` is placed at its `x, y` today.
- Subject to the same `icon_size_ratio × region width` sizing and the same debug-icon fallback for
  a missing asset.

No new rendering machinery is needed — this is additive to the existing composite, not a parallel
pipeline. Pins appear on `/get-map`, `/full-map`, and scheduled reports for any region that has
active (unexpired) reports, the same render call that already overlays `mapItems` just also
overlaying `faction_reports` rows for that region+faction+shard.

## Acceptance criteria
- A grid reference that parses lands its pin within the correct 3×3 keypad cell — exact pixel
  precision isn't the bar, "recognizably the right spot on the hex" is.
- A landmark-based report's pin sits exactly on that landmark's known coordinate.
- An unparseable grid string never silently produces a pin in the wrong place — it's rejected with
  the teaching message, not guessed at.
- A region with no active reports renders exactly as it does today — this is additive, never a
  behavior change for the common case.
- The same formula works for all 53 regions without a per-region exception (once the blocking
  verification lands).
