# Context: full-map renderer + scheduling gate

Specs: `specs/full-map-renderer.md` (what to draw)
       `specs/premium-full-map.md` (who may schedule it, and the form)

Taken together because they're one deliverable: the gate exists only to ration the renderer, and
the renderer's cost is the gate's entire justification. Build in that order — the renderer first,
ungated, then the gate around it.

## Current state
**Phase 1 is functionally complete and rendering correctly.** All 53 hexes, icons legible,
faction tint in and seen working.

**Phase 2 is done and walked end to end by the user**: boot + migration `0004`, applying,
withdrawing, deciding, scheduling, and the dormancy pause all behave as designed. The tint
tiebreak reads correctly at full-map scale.

Only two things in this task are unverified, and neither blocks the work that comes next: the
90-day purge (can't be exercised without backdating a row) and peak render memory (never
measured). See `tasks.md` → Release checklist.

Both earlier open loops are closed and confirmed by live renders: the two missing hexes are back,
and the invisible icons are visible (`full_map_icon_px`, sized backwards from the finished PNG).

Commits on `feat/rewrite-overhaul` (PR #1), most recent last: `5bff8b3` renderer, `288182f`
identifier fixes, `82cf4e0` log filter, `cf27f29` icon sizing + stale-asset deletion, `081ffea`
the asset script, `a685033` **the user's own 66-file icon update** (ran the script for real),
`a9ca90d` faction tint, `7991e6c` hand-sourced icon split, `e4f4d19` description-length fix,
`d1a09f8` dev-guild command clearing, `5c3898b` HTTP timeouts.

The tint has now been seen at full-map scale at strength **0.5** and reads correctly. One hex came
back untinted — an even split of bases, which `controlling_team` scored as a tie. Fixed by falling
through to all other faction-held structures; **that tiebreak is unverified, needs a build.**
Everything else in Phase 1 has been seen working.

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
- `src/commands/` — `full_map.rs`, plus Phase 2's `request_full_map_schedule.rs` (the modal) and
  `full_map_requests.rs` (`list`/`approve`/`deny`/`revoke`).
- `src/utils/review.rs` — the review embed, the Approve/Deny buttons, and `is_reviewer`. Both
  review surfaces call the same `review_full_map_request`, so they can't disagree.
- `src/utils/entitlement.rs` — the one place the scheduling rule is written.
- `src/utils/http.rs` — **the** HTTP client. One shared `reqwest::Client` with a 15s request and
  5s connect timeout. Every outbound call goes through it; `reqwest::Client::new()` must not come
  back, it has no timeout at all.
- `scripts/update_assets.py` — art in from a warapi clone, renamed to the table's spelling.
  `--audit` alone cross-checks `assets/Maps/` and exits non-zero. `HAND_SOURCED` lists the 13
  icon types warapi doesn't ship, so only a real upstream rename is reported as UNEXPECTED.
- `migrations/0003_full_map_gate.sql` — the gate. `0004_request_message.sql` — the review post's
  message id. Next free number is **0005**, which `specs/schedule-input.md` now claims.
  `0001_init.sql` has run against real databases and must not be edited.

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
- **`async fn` in a trait produces a future with no `Send` bound**, and both this codebase's
  async call sites require one: poise boxes every command body as `Pin<Box<dyn Future + Send>>`
  and the scheduler does the same to every tick. `FullMapScheduling::is_allowed` is therefore
  spelled `-> impl Future<Output = bool> + Send`, not `async fn`. Written the short way it
  compiles at the definition and fails at both call sites, with an error that points at them.
  (Cost a build. The `#[allow(async_fn_in_trait)]` I had put on it was wrong — the lint was
  telling the truth.)
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

## Closed: the `/set_guild_settings` non-reply
It replies again, and has since the HTTP timeout work landed (`utils::http`). Never diagnosed
directly — no logs were captured — so this is a fix by circumstance, not by proof. The candidate
it fits is the shard health check hanging on a connection the API accepted and then went quiet on,
with no request timeout to cut it. If it ever returns, the other candidate was "something after
`upsert_guild` failed", and `on_error`'s `command /… failed: …` line splits the two cleanly.

## Adjacent work raised this session
`specs/schedule-input.md` — picked up as its own task; see `dev/active/schedule-input/`.
It claimed migration `0005`, so the next free number is **0006**.

## Phase 2 decisions taken while building (not in the spec — worth keeping)
- **Reviewers are exactly `REVIEWER_IDS`, with no implicit owner.** The spec said "owner/admin";
  an admin check would have meant any server admin could run `/full-map-requests list` and read
  other servers' free-text answers. The owner shortcut then went too, at the user's call: this is
  open source and self-hosted, so "the owner can always approve" invites a guess about whose
  account that is. You must list yourself. Empty ⇒ nobody can review, warned at startup.
  Env var, not a table — reviewing grants recurring load on the host, so the list belongs to
  whoever runs the host, and keeping it out of the database also keeps `docs/` unchanged.
- **`cronjobs.map_name` nullable, NULL = full map.** No `is_full_map` flag beside it. The `Option`
  is what made the change find its own call sites.
- **The full-map target is a sentinel in the existing `map-name` option** (`full-map`, offered
  first by `autocomplete_schedule_target`), not a second `full:true` option — one field says what
  a schedule renders.
- **Notifications go to the request's own channel, never a DM.** `docs/privacy.md` promises the
  bot never DMs anyone, and that promise is worth more than the convenience.
- **The spec's `map_target` column was dropped** — with full-map the only target, it stores a
  constant.
- **Every exit from a request is a button, not only a command.** Withdraw (applicant, on the
  ephemeral reply to `/request-full-map-schedule`) and Revoke approval (reviewer, on an approved
  post). `/full-map-requests revoke` stays as the fallback — it wants a server id typed by hand,
  which is fine as a backstop and poor as the primary way to undo something. Both land on
  `withdrawn`: neither is a refusal, and `denied` would record a judgement nobody made.
- **Revoking moves two rows in one transaction** (`revoke_full_map_approval`) — the guild flag and
  the request that granted it. `set_full_map_approved(guild, bool)` was deleted rather than
  reused: granting and revoking each have to move a *different* second row, so a shared setter
  could only do the flag, which is how the flag and the record start disagreeing.
- **Neither undo announces anything.** A withdrawal is the applicant's own doing; a revocation is
  already the dormancy notice's job, in the channel the reports actually go to. Two sources for
  one fact is how they end up disagreeing. The dormancy notice therefore has to *say* "withdrawn"
  — it used to say the approval "isn't active", which left a reader guessing between a revocation
  and never having been approved. It is the only message that ever reports a revocation.
- **`/request-full-map-schedule` replies in public, not ephemerally** (user's call). Applying for
  a recurring server-wide report isn't a private act, and an ephemeral confirmation disappears on
  dismiss, leaving the next admin unable to tell whether anything was filed. The Withdraw button
  consequently sits on a public message; `withdrawable` checks the presser against the filer, so
  a bystander clicking it is told it isn't theirs.
- **`revoke_full_map_approval` returns `Revoked`, not `Option`/`bool`.** "Nothing to correct" and
  "nothing happened" are different answers — a guild whose request was purged still gets revoked,
  and an `Option` would have had the command report "nothing to withdraw" right after withdrawing
  it.

## Next steps
**This task is finished bar the two unverified items in `tasks.md`** (the 90-day purge and peak
render memory), both of which are release-gate checks rather than development. Active work has
moved to `dev/active/schedule-input/`.

When those two are closed and both `specs/` specs are promoted, move
`dev/active/full-map/` to `dev/done/`.
