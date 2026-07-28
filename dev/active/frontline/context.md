# Context: frontline overlay

Spec: `specs/active/frontline.md`. Tasks: `tasks.md`.

Branch `claude/specs-workflow-review-p0c7p3`, PR #2 (the logging work, now closed out, is on the
same branch).

## Current state

**Spec written, no code yet.** Awaiting review of the spec before implementation starts.

## Decisions already made (user's calls, don't re-ask)

- **Both `/get-map` and `/full-map`**, not just single hexes — the full map costs ~5 ms with a
  distance transform, so cheapness removed the reason to skip it.
- **`/get-map` fetches its up-to-6 neighbours' dynamic data**, so the line is correct at the hex
  edge rather than drifting where it matters most.
- **Guild setting**, like `full_map_faction_tint` — which means a migration, and therefore
  `docs/tos.md` + `docs/privacy.md` in the same commit.

## Established by measurement, not assumed

- **Neighbours are derivable, no table needed.** `regions.rs` is a flat-top **odd-q** offset grid
  (odd columns half a hex lower). The six-neighbour arithmetic in the spec was checked against the
  shipped table: symmetric for all 53 regions, degrees 2–6, and it reproduces real geography
  (Deadlands ↔ Callahans Passage, Umbral Wildwood, Linn of Mercy, Loch Mór, Marban Hollow,
  Drowned Vale). This belongs in a unit test.
- **Cost.** One hex: ~57k cells at ¼ res, naive nearest-neighbour is sub-millisecond. Full map:
  ~1M cells at ⅛ of 63.6 MP against ~800 control structures — naive is ~800M ops and too slow, a
  two-pass chamfer distance transform is O(cells), ~5 ms, ~5 MB.

## Key files (expected)

- `src/utils/request_processing.rs` — `CONTROL_ICON_TYPES` and `controlling_team` already exist
  and are the model to reuse; `RenderConfig` gains the frontline ratios; `grid_offset` is the
  world-space anchor.
- `src/utils/regions.rs` — neighbour arithmetic from `(col, row)`.
- `src/utils/map_render.rs` — `composite_full_map` draws at full res *before* the downscale, which
  is where the line goes; `render_region` gains the neighbour fetches.
- `src/utils/db.rs`, `migrations/`, `src/commands/set_guild_settings.rs`, `docs/` — the toggle.

## Traps worth not rediscovering

- `REGION_WIDTH` 1024 ≠ `REGION_HEIGHT` 888, so a radius in the API's normalized coordinates is an
  ellipse. Do the distance maths in canvas pixels.
- `controlling_team` uses non-control structures as a **tiebreak**; the frontline must not, since
  a tiebreak is a fact about a whole hex and has no position.
- Draw after the tint, before the icons — same rule the tint already follows.

## Next steps

Get the spec reviewed. Then implement in the order in `tasks.md`, starting with the neighbour
arithmetic and its test, which is the one piece that can be verified with no rendering at all.
