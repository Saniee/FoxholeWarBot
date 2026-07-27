# Context: full-map renderer + scheduling gate

Specs: `specs/active/full-map-renderer.md` (what to draw)
       `specs/active/premium-full-map.md` (who may schedule it, and the form)

Taken together because they're one deliverable: the gate exists only to ration the renderer, and
the renderer's cost is the gate's entire justification. Build in that order — the renderer first,
ungated, then the gate around it.

## Current state
**Phase 1 is written, not yet compiled.** `/full-map` renders all 53 hexes: grid table in
`regions.rs`, geometry in `RenderConfig`, `render_full_map` in `map_render.rs`, command in
`commands/full_map.rs`. Phase 2 (the gate) is untouched.

The user pushed new map art mid-work (`b6bcef1`, `307050c`) — same 1024 x 888 32-bit
uncompressed TGAs, now all under `assets/Maps/`. That move also swept `Inter-Bold.ttf` into
`assets/Maps/`, which broke both `include_bytes!` and `Dockerfile:18`; it has been moved back to
`assets/`. Icons are the user's next art pass.

## Key files
- `src/utils/request_processing.rs` — `RenderConfig` + per-region compositing. The full map is
  53 calls to `place_image_info` plus offsets; region-relative ratios already make a hex look the
  same standalone or tiled.
- `src/utils/map_render.rs` — the fetch → ETag revalidate → render path for **one** region.
  The full map needs the same shape fanned out 53×, sharing `conditional_get`/`resolve`.
- `src/utils/regions.rs` — the 53 `(api_name, display_name)` pairs. The grid table is a third
  column (`col`, `row`); keep it in this file so a new region is one edit, not two.
- `src/utils/cron.rs` — `ReportJob` gains the full-map marker; `run_report` gains the dormancy
  branch.
- `src/utils/db.rs` — new columns and the `full_map_requests` table.
- `src/commands/` — new `full_map.rs`, `request_full_map_schedule.rs`, `full_map_requests.rs`.
- `migrations/0002_*.sql` — **new file.** `0001_init.sql` has run against real databases and
  must not be edited.

## Decisions already settled (in the specs — don't re-litigate)
- Dedicated `/full-map` command, not a flag on `/get-map`.
- On-demand `/full-map` always free; **every** scheduled full map needs an approved request,
  whatever the guild's size. The old 50-member free tier is gone (user's call, this session).
- In-Discord modal for the form; review via both `REQUESTS_CHANNEL_ID` buttons and
  `/full-map-requests`.
- No monetization ships. Faction tint off by default; per-hex name labels are v2.
- `OriginHex` is already un-excluded (landed in the core rewrite).

## Risks / gotchas
- ~~Column pitch 768 is derived, not measured.~~ **Resolved.** Measured off the assets' alpha
  channel: the art is a true flat-top hexagon with no padding, two neighbours at `(+768, +444)`
  leave **0 uncovered pixels**, and a 53-hex composite has continuous coastlines across every
  boundary. The canvas computes to exactly 10240 × 6216 from the table. (Method:
  `scratchpad/interlock.py` + `composite.py` — a pure-Python TGA compositor, since the Rust side
  can't be run here. Rewritable in ten minutes if it's ever needed again.)
- **63.6 MP canvas.** RGBA at full size is ~254 MB before downscale. Watch the container's memory
  ceiling; consider compositing per-column or downscaling regions before overlay if it bites.
- ~~`member_count` has nowhere to land at join time.~~ **Dissolved** by dropping the size
  exemption. Nothing decides anything from member count now, so there's no cached column to keep
  fresh, no refresh-on-`guild_create` path, and no conflict with "no guild row is created on
  join". The count is read live when a request is filed and snapshotted onto the request row as
  review context. Worth remembering if a size rule ever comes back: the reason it was awkward is
  that the entitlement check ran at command time, when a guild might have no row at all.
- **Docs ship with the migration, not after.** `specs/docs-site.md`'s invariant is that the
  stored-data list matches the schema; `premium-full-map.md` → "Docs impact" lists exactly what
  to add. Same commit as `0002`.
- Scheduling paths remain the least-exercised code in the rewrite (see
  `dev/done/core-rewrite/context.md`), and this task adds a second job kind to them.
- User compiles and runs locally; **don't run `cargo build`/`check`/`run`** unless told otherwise.

## Next steps
Build and run `/full-map` once. Two things to look at in the output: whether the icons are still
legible at the 0.2x downscale (the knob is `icon_size_ratio`, not the downscale — see the spec),
and peak memory during the composite. Then Phase 2, starting with `migrations/0002_*.sql`.
