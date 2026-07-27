# Context: full-map renderer + scheduling gate

Specs: `specs/active/full-map-renderer.md` (what to draw)
       `specs/active/premium-full-map.md` (who may schedule it, and the form)

Taken together because they're one deliverable: the gate exists only to ration the renderer, and
the renderer's cost is the gate's entire justification. Build in that order — the renderer first,
ungated, then the gate around it.

## Current state
**Phase 1 is functionally complete and rendering correctly.** All 53 hexes, icons legible,
faction tint in. Phase 2 (the gate) is untouched — not started, no code written.

Both earlier open loops are closed and confirmed by live renders: the two missing hexes are back,
and the invisible icons are visible (`full_map_icon_px`, sized backwards from the finished PNG).

Commits on `feat/rewrite-overhaul` (PR #1), most recent last: `5bff8b3` renderer, `288182f`
identifier fixes, `82cf4e0` log filter, `cf27f29` icon sizing + stale-asset deletion, `081ffea`
the asset script, `a685033` **the user's own 66-file icon update** (ran the script for real),
`a9ca90d` faction tint, `7991e6c` hand-sourced icon split, `e4f4d19` description-length fix,
`d1a09f8` dev-guild command clearing, `5c3898b` HTTP timeouts.

**Unverified, needs a build:** the faction tint render. `faction_tint_strength` went 0.3 → **0.5**,
the user's pick off a single-hex preview grid, not yet seen at full-map scale. Everything else in
Phase 1 has been seen working.

Art is now maintained by `scripts/update_assets.py` against a local `clapfoot/warapi` clone. The
user ran it and it updated 66 icons, including 70/71/72 and 88–92 which had no art at all and
were rendering as `DebugIcon.png`.

## Key files
- `src/utils/request_processing.rs` — `RenderConfig` + per-region compositing. The full map is
  53 calls to `place_image_info` plus offsets; region-relative ratios already make a hex look the
  same standalone or tiled.
- `src/utils/map_render.rs` — `fetch_region` (shared), `render_region` (one hex),
  `render_full_map` (all 53), `composite_full_map` (the blocking half).
- `src/utils/regions.rs` — the `Region` table: `api_name`, `display_name`, `col`, `row`, plus
  `same_region`/`asset_name`, which are what keep the API's spelling and the assets' spelling
  from being confused for each other. One edit adds a region.
- `src/utils/cron.rs` — `ReportJob` gains the full-map marker; `run_report` gains the dormancy
  branch.
- `src/utils/db.rs` — new columns and the `full_map_requests` table.
- `src/commands/` — `full_map.rs` exists; `request_full_map_schedule.rs` and
  `full_map_requests.rs` are Phase 2 and do not exist yet.
- `src/utils/http.rs` — **the** HTTP client. One shared `reqwest::Client` with a 15s request and
  5s connect timeout. Every outbound call goes through it; `reqwest::Client::new()` must not come
  back, it has no timeout at all.
- `scripts/update_assets.py` — art in from a warapi clone, renamed to the table's spelling.
  `--audit` alone cross-checks `assets/Maps/` and exits non-zero. `HAND_SOURCED` lists the 13
  icon types warapi doesn't ship, so only a real upstream rename is reported as UNEXPECTED.
- `migrations/0003_*.sql` — **the Phase 2 file, not yet written.** `0002_faction_tint.sql` is
  taken. `0001_init.sql` has run against real databases and must not be edited.

## Decisions already settled (in the specs — don't re-litigate)
- Dedicated `/full-map` command, not a flag on `/get-map`.
- On-demand `/full-map` always free; **every** scheduled full map needs an approved request,
  whatever the guild's size. The old 50-member free tier is gone (user's call, this session).
- In-Discord modal for the form; review via both `REQUESTS_CHANNEL_ID` buttons and
  `/full-map-requests`.
- No monetization ships. Faction tint off by default; per-hex name labels are v2.
- `OriginHex` is already un-excluded (landed in the core rewrite).
- Logs go to **stderr only**, no file; the default filter drops `tracing::span` (serenity's
  per-heartbeat gateway spam). See `specs/architecture.md` → Logging. If the user asks for file
  logging, it needs a rotation policy — the container disk is not large.

## Risks / gotchas
- **Identifier drift is real, and it was silent.** The first live render came back with two holes:
  the API calls Marban Hollow `MarbanHollow` (no `Hex`), and `MapDeadLandsHex.TGA` has a
  capitalisation history — there is a commit literally named "Rename MapDeadlandsHex.TGA to
  MapDeadLandsHex.TGA". Both now handled (lenient `same_region`, case-insensitive background
  retry, assets addressed through the table). If a hex ever goes missing again, the log names it.
- ~~Two stale assets are still in the tree.~~ **Deleted** (`MapMarbanHollow.TGA`,
  `MapClahstraHexMap.TGA` — older art, not copies). `assets/Maps/` now holds exactly one file per
  table region, name-for-name, plus `BGOneWorldMap.TGA` and the two `MapHomeRegion{C,W}.TGA`.
  A quick cross-check script against `regions.rs` is worth re-running after any art drop.
- **Anything drawn on a tile is drawn at 5x too small.** The tiles are composited at full size and
  the whole 63.6 MP canvas is scaled once at the end, so a per-region ratio tuned for `/get-map`
  is five times too small on the full map. Icons hit this; labels would too, which is part of why
  `draw_text` is `false` there. `for_full_map` is where anything else with a legibility floor
  belongs.
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
  to add. Same commit as `0003`. (Done correctly for `0002` — copy that pattern.)
- **Discord caps a slash command description at 100 characters**, and poise checks it at compile
  time, so an over-long doc comment fails the build outright — and reports it as three bogus
  "unused import" warnings, because the macro bails before generating the body. Applies to
  parameter descriptions too. Phase 2 adds three commands and wants long "no payment involved"
  wording: that text goes in the modal or the reply, never the description.
- Scheduling paths remain the least-exercised code in the rewrite (see
  `dev/done/core-rewrite/context.md`), and this task adds a second job kind to them.
- User compiles and runs locally; **don't run `cargo build`/`check`/`run`** unless told otherwise.

## Open question, unresolved
`/set_guild_settings shard:Able show_messages:false faction_tint:true` **did not reply** — the
user reported it hangs, then clarified the effect appeared to work. Not diagnosed. Two candidates,
and the logs decide between them:
- The shard health check had **no HTTP timeout** (now fixed — `utils::http`). If the API accepted
  the connection and went quiet, the command hung before ever writing to the DB. This fits "no
  reply" but *not* "the setting applied".
- Something after `upsert_guild` failed, so the write landed and `ctx.say` didn't. That fits both
  halves of the report and is **not** fixed by the timeout work.

Ask for `docker compose logs bot` around the invocation. `on_error` logs `command /… failed: …`
for any command error, so its presence or absence splits the two cleanly.

## Next steps
Build, then `/full-map` with the tint on to see `faction_tint_strength: 0.5` at real scale — the
only Phase 1 thing never seen working. If the answer to the hang question above arrives with it,
take that first; it is the one open defect. Peak memory is still unmeasured.

Then Phase 2, starting with `migrations/0003_*.sql` and its `docs/` change in the same commit.
