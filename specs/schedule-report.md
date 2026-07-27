# /schedule-report

## Summary
Creates a recurring scheduled map report that posts a rendered region to a channel via a
webhook, on an english-phrase or cron schedule.

## Command surface
- Name: `schedule-report`, guild-only.
- Options (all **required**):
  - `map_name` (string, autocomplete) — region to render.
  - `schedule_name` (string) — unique **within this server**.
  - `report_channel` (channel) — where reports post.
  - `schedule` (string) — english phrase or 6-field cron; see `/schedule-help`.
  - `draw_text` (boolean) — overlay text labels.
- Permissions: **`MANAGE_WEBHOOKS`** (`default_member_permissions`). The command creates and
  deletes webhooks, so that's the permission that matches what it actually does.

### Autocomplete
`map_name` only, the shared helper — see `get-map.md`.

## Behavior
1. Look up guild; if not set up → ephemeral prompt.
2. Defer public/ephemeral per `show_command_output`.
3. Validate `schedule` via `Job::schedule_to_cron`; on failure, point at `/schedule-help`.
4. Reject a `schedule_name` already used **in this guild**. The `UNIQUE (guild, job_name)`
   constraint is the real guard; this check exists to produce a friendlier message.
5. Find or create a webhook named "Scheduled Map Report Webhook" in `report_channel`. Missing
   permission replies with what to grant.
6. **Schedule first, persist second.** Register the job with the scheduler, then insert the
   `cronjobs` row carrying the scheduler UUID. If the insert fails, the job is unscheduled
   again — no orphan rows, and no reports posting that nothing knows about.
7. Reply confirming, pointing at the channel.

## Scheduled job execution (per tick)
1. Re-read the owning guild's settings, so a shard or visibility change takes effect without a
   restart. A job whose guild is gone logs and skips.
2. Resolve the webhook from its stored URL.
3. `render_region` — the same pipeline `/get-map` uses, ETag-revalidated.
4. `webhook.execute` an embed titled "Scheduled Report: `<name>`" with the region display name,
   "Last API Update", and "Next Scheduled Update … UTC" when a next tick is known.

A failing tick logs and returns. It never panics, and never takes the scheduler with it.

## Startup restoration
Runs **once per process**, from the framework's `setup` hook. Each `cronjobs` row is joined to
its owning guild, so every job restores against its own shard. A row that can't be restored —
unresolvable webhook, unparseable schedule — is skipped and logged, and restoration continues.
The scheduler issues fresh UUIDs each process, so the stored id is rewritten on restore.

## External calls
- Discord: list/create channel webhooks; execute the webhook per tick.
- Foxhole + disk: as `/get-map`.
- DB: `cronjobs` insert; `guilds` read per tick.

## Acceptance criteria
- A valid `/schedule-report` creates a webhook (if absent) and begins posting on schedule.
- An invalid schedule phrase is rejected with a pointer to `/schedule-help`.
- A duplicate name **in the same guild** is rejected; two different guilds can each have a
  schedule named "daily".
- Reconnecting the gateway does not increase the number of scheduled ticks or duplicate posts.
- With schedules in two guilds on different shards, after a restart each renders its own shard.
- A member without Manage Webhooks cannot use the command.
- A scheduler rejection leaves no row behind.
