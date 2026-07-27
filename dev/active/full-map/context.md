# Context: full-map renderer + scheduling gate

Specs: `specs/active/full-map-renderer.md` (what to draw)
       `specs/active/premium-full-map.md` (who may schedule it, and the form)

Taken together because they're one deliverable: the gate exists only to ration the renderer, and
the renderer's cost is the gate's entire justification. Build in that order — the renderer first,
ungated, then the gate around it.

## Current state
Nothing started. Core rewrite, specs reconciliation and the docs overhaul have all landed on
`feat/rewrite-overhaul` (PR #1); working tree was clean at `d494223`.

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
- **Column pitch 768 is derived, not measured.** The spec flags it: it comes from the hex aspect
  ratio and a reference screenshot, not from the TGAs' transparent margins. First real render,
  check the seams before trusting the 10240 × 6216 canvas.
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
Start with the grid table + canvas geometry in `regions.rs`, then `render_full_map` in
`map_render.rs`. Get one real render out and check the seams before building anything on top of
the geometry.
