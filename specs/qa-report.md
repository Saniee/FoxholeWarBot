# QA Report — crash sweep & general QA

Findings from a full read of the current `src/` (commit `a1c5a3f`). The code **compiles clean**
(`cargo check` OK; 5 clippy warnings). The issues below are runtime/logic problems, ranked by
severity. IDs are referenced from the feature specs.

Legend: **C** = crash/panic, **B** = behavior/correctness, **S** = security/permissions,
**L** = lower/minor. Every command handler in `main.rs` calls `.await.unwrap()` on the
command's `run`, so any panic below both fails that interaction (user sees "application did not
respond" or a stuck "thinking…" spinner) and can bring down the interaction task.

---

## Critical — crashes / panics

### C-1 · Shared `render.png` output file (concurrency)
`get_map.rs:84,97,101`, `cron.rs:171,182,193`. Every render writes and then attaches the single
fixed path `render.png` in the CWD. Two concurrent `/get-map` calls — or a scheduled tick firing
while a user runs the command — race on the same file; a user can receive another request's map,
or attach a half-written PNG. **Fix:** render to a unique path (temp file / UUID / in-memory
`CreateAttachment::bytes`) per invocation.

### C-2 · Mismatched 304 handling unwraps a `None` cache
`get_map.rs:69,86-88`. The "fresh" branch requires **both** dynamic and static to be non-304.
If exactly one endpoint returns `304` and there is no on-disk cache, control falls to the else
branch which does `cache_data.clone().unwrap()` → panic. **Fix:** handle each resource's
freshness independently, and never unwrap a possibly-absent cache.

### C-3 · Unwrapped network + JSON calls everywhere
`get_map.rs:54,56,60,62,70,73`; `war_report.rs:46,48,56`; `war_state.rs:27`;
`set_guild_settings.rs:30,50`; `cache.rs:39`; `cron.rs:143-162,195`. Every `reqwest` `send()`
and `json()` is `.unwrap()`ed. A transient network error, a 5xx with a non-JSON body, or an API
schema change panics the handler after it already deferred, leaving a permanent spinner.
**Fix:** propagate/handle errors and edit the deferred response with a user-facing failure.

### C-4 · `guild_id.unwrap()` in every command
`get_map.rs:12`, `war_report.rs:11`, `war_state.rs:9`, `set_guild_settings.rs:22`,
`schedule_report.rs:20`, `remove_report.rs:8`, `schedule_help.rs:8`. Global commands can be
invoked in DMs/user-install contexts unless explicitly restricted; there `guild_id` is `None`
and the command panics. **Fix:** set the commands' contexts to guild-only, and match on
`guild_id` instead of unwrapping.

### C-5 · `from_timestamp_millis(...).unwrap()`
`get_map.rs:100`, `war_state.rs:33`, `cron.rs:188,190`. An out-of-range/garbage timestamp from
the API panics formatting. **Fix:** `unwrap_or` a sentinel or handle `None`.

### C-6 · Report jobs (and the daily map job) duplicate on every reconnect
`main.rs:127,166-171`. `HandlerData::default()` is reconstructed inside `ready`, so
`cron_jobs_restarted` is always `false` when checked, and the write on line 170 is dead (clippy:
"value assigned to `data` is never read"). Discord fires `ready` on every resume/reconnect →
`start_map_update_job` and `restart_report_jobs` run again → schedules accumulate; each webhook
report fires N× after N reconnects. **Fix:** persist the "already started" flag in shared state
(e.g. an `AtomicBool`/`OnceCell` on `Handler`), guarding both the map job and restarts.

### C-7 · Cross-guild data corruption in `restart_report_jobs`
`cron.rs:65,74`. The function fetches the guild row for `jobs[0].guild` **once** and assigns that
same `guild.guild_id` to every `ReportJob` in the loop. After a restart, all scheduled reports —
regardless of which guild owns them — render and post using the **first** guild's shard/settings.
**Fix:** resolve the owning guild per job (join on `cronjobs.guild`).

### C-8 · One bad webhook aborts all restarts
`cron.rs:73`. `Webhook::from_url(...).await.unwrap()` inside the restart loop panics if a stored
webhook URL is invalid/deleted, aborting restoration of every remaining job. **Fix:** handle the
error per job and skip (optionally prune the dead row).

### C-9 · `remove-report` panics on an already-deleted webhook
`cron.rs:219-220`. `Webhook::from_url(...).unwrap()` then `webhook.delete(...).unwrap()` panic if
the webhook was removed in Discord manually. **Fix:** treat "webhook already gone" as success.

### C-10 · `get_guild` errors permanently if duplicate guild rows exist
`db.rs:99` uses `fetch_optional`, which **errors** when more than one row matches. Because
`guilds.guild_id` has no UNIQUE constraint (`main.rs:213`) and `create_guild` is called from the
`guild_create` handler (`main.rs:110`) plus the legacy migration, duplicate rows are possible;
once present, every command for that guild panics on the `.unwrap()` at `db.rs:99`. **Fix:** add a
UNIQUE constraint / upsert, and use `fetch_optional` on a guaranteed-unique key.

### C-11 · `schedule-report` panics without "Manage Webhooks"
`schedule_report.rs:61`. `channel.webhooks(ctx).await.unwrap()` panics if the bot lacks the
permission (the FAQ lists it as required, but users won't always grant it). **Fix:** handle the
error and reply with a permissions hint.

### C-12 · Both-endpoints-500 leaves a stuck spinner
`get_map.rs:65-67`, `war_report.rs:51-53`, `cron.rs:154-156`. On 500 the handler `return Ok(())`
**after** deferring, so the interaction is never edited → permanent "thinking…". **Fix:** edit
the deferred response with an error message before returning.

---

## Correctness / behavior

### B-1 · `show-messages` ignored on first-time setup
`set_guild_settings.rs:34` → `db.rs:102-105`. The create path calls `create_guild`, which
hard-codes `show_command_output = 0` and never reads the `show-messages` option. A new guild that
sets `show-messages:true` stays ephemeral until the command is run a second time. **Fix:** pass
`show` through `create_guild`.

### B-2 · Schedule names are globally unique, not per-guild
`main.rs:225` (`job_name TEXT UNIQUE`). Two different guilds cannot use the same schedule name.
**Fix:** make uniqueness `(guild, job_name)`.

### B-3 · `add_report_job` persists before scheduling succeeds
`cron.rs:92-99` vs `101-207`. The DB row is inserted before the scheduler call; if scheduling
fails, an orphan row with a NULL `job_id` remains. **Fix:** schedule first (or roll back on
failure).

### B-4 · `--clear-commands` builds a client but never connects
`main.rs:186-205`. It uses `client.http` without starting the gateway; works because the REST
token is enough, but the extra `Client::builder(...).await` is misleading and could be a bare
`Http::new(token)`. Minor.

---

## Security / permissions

### S-4 · `/schedule-report` and `/remove-report` have no permission gate
`schedule_report.rs:122`, `remove_report.rs:84`. Only `/set-guild-settings` sets
`default_member_permissions(ADMINISTRATOR)`. Any member can create/delete webhook-backed
schedules (and thereby cause the bot to create webhooks). **Fix:** gate both behind an
appropriate permission (e.g. `MANAGE_WEBHOOKS` or `MANAGE_GUILD`).

---

## Lower / minor

- **L-1** · `war_report.rs:16` — not-set-up message references the removed `/set-shard`
  command name.
- **L-2** · Timezone mismatch — schedules evaluate in the scheduler clock (UTC) while embeds
  format with `chrono::Local` (`cron.rs:188-190`, `get_map.rs:100`).
- **L-3** · `remove_report.rs:66,72` — autocomplete lowercases the filter but matches against the
  raw (case-sensitive) `job_name`, so mixed-case names don't filter correctly.
- **L-4** · `war-state` self-describes as "defaults to not showing" but honors the guild setting
  (`war_state.rs:20-24,43`). Doc/behavior mismatch.
- **L-5** · `schedule-help` requires guild setup to return static help text
  (`schedule_help.rs:11-17`).
- **L-6** · Clippy: 4 `unwrap`-after-`is_some/is_none` sites (`cache.rs:104,105`,
  `cron.rs:188`, `schedule_report.rs:75`) — safe today but fragile; prefer `if let`/`match`.
- **L-7** · `save_maps_cache` / `load_maps` unwrap file I/O (`cache.rs:56,65-67`); autocomplete
  panics if the cache file is missing before the first `save_maps_cache` completes.
- **L-8** · Blocking image work (`image::open`, resize, `img.save`) runs on the async runtime
  thread; heavy renders can stall other tasks. Consider `spawn_blocking`.

---

## Suggested fix order for the rewrite
1. C-1, C-3, C-4, C-12 — eliminate the "stuck spinner"/panic class (unwraps, unique output file,
   guild-only contexts, error replies).
2. C-6, C-7 — fix the scheduler lifecycle (single restart, per-guild resolution) — see
   `specs/active/scheduling-overhaul.md`.
3. C-10, B-1, B-2 — database integrity (unique guild, per-guild job names, honor `show`).
4. S-4 — permission gates.
5. Remaining L-items during the port.
