# /schedule-report

## Summary
Creates a recurring scheduled map report that posts a rendered region — or the whole world map —
to a channel via a webhook, on a cadence picked from a list and read in a chosen timezone.

## Command surface
- Name: `schedule-report`, guild-only.
- Options, required first (Discord's ordering rule):
  - `map_name` (string, autocomplete) — region to render, or the `full-map` sentinel.
  - `schedule_name` (string) — unique **within this server**.
  - `report_channel` (channel) — where reports post.
  - `frequency` (choice) — every 15/30 minutes, hourly, every 2/3/4/6/8/12 hours, daily, weekly,
    or `Custom…`.
  - `draw_text` (boolean) — overlay text labels.
  - `at_time` (string, optional) — 24-hour `HH:MM`; the anchor the cadence lines up with.
    Defaults to the top of the hour.
  - `timezone` (string, optional, autocomplete) — IANA name. Defaults to the guild's.
  - `day` (choice, optional) — for `Weekly`; defaults to Monday, inert otherwise.
  - `custom` (string, optional) — read only when `frequency` is `Custom…`: a 6-field cron
    expression or a plain-English phrase.
- Permissions: **`MANAGE_WEBHOOKS`** (`default_member_permissions`). The command creates and
  deletes webhooks, so that's the permission that matches what it actually does.

**The bot writes the cron string; the user never does** (`specs/active/schedule-input.md`).
`(frequency, at_time, day)` generates it as a total function — the only error an ordinary path
can produce is a malformed `at_time`, and that message carries an example. Minutes and hours are
written out in full (`0 30 3,9,15,21 * * *`) rather than as `*/6`, so nothing depends on how a
parser reads a step.

### Autocomplete
`map_name` (the shared helper plus the full-map sentinel — see `get-map.md`) and `timezone`
(the IANA list; an empty box offers the guild's own zone first, then a short common list).

## Behavior
1. Look up guild; if not set up → ephemeral prompt.
2. Defer public/ephemeral per `show_command_output`.
3. Full-map target with no approval → the request-form reply, never a bare refusal
   (`specs/active/premium-full-map.md`).
4. Resolve the timezone — the `timezone` option, else the guild's default — and store the
   resolved name on the row, so a later change to the guild default can't move this schedule.
5. Generate the cron expression and the human label from the cadence.
6. **Compute the next three fire times and show them in the reply.** Nothing is stored until they
   are on screen: an expression that parses can still mean something the user didn't intend
   (`every 6 hours` is absolute clock times, not six hours from now), and three real timestamps
   are what catch that. Rendered as `<t:epoch:F>`, so each reader sees their own timezone.
7. A **full-map** schedule may not fire more often than once an hour. Measured from the gaps
   between those same fire times, so `Custom…` is held to it too — the approval queue decides
   *whether* a guild may schedule the world map, not how often.
8. Reject a `schedule_name` already used **in this guild**. The `UNIQUE (guild, job_name)`
   constraint is the real guard; this check exists to produce a friendlier message.
9. Find or create a webhook named "Scheduled Map Report Webhook" in `report_channel`. Missing
   permission replies with what to grant.
10. **Schedule first, persist second.** Register the job with the scheduler, then insert the
    `cronjobs` row carrying the scheduler UUID. If the insert fails, the job is unscheduled
    again — no orphan rows, and no reports posting that nothing knows about.
11. Reply confirming: the cadence in words, its timezone, the channel, and the next three runs.

## Scheduled job execution (per tick)
1. Re-read the owning guild's settings, so a shard or visibility change takes effect without a
   restart. A job whose guild is gone logs and skips.
2. Resolve the webhook from its stored URL.
3. `render_region` — the same pipeline `/get-map` uses, ETag-revalidated.
4. `webhook.execute` an embed titled "Scheduled Report: `<name>`" with the region display name,
   the cadence in words and its timezone, "Last API Update", and — when a next tick is known —
   "Next Scheduled Update" as `<t:epoch:F> (<t:epoch:R>)`, so every reader sees it on their own
   clock rather than in UTC.

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
- No option takes a free-text schedule phrase unless the user explicitly picked `Custom…`.
- Creating any schedule shows the next three fire times before it is saved.
- A schedule created with a timezone fires at that wall-clock time in that zone, including after
  a DST transition, without a restart (see `specs/scheduling.md` → nightly rebuild).
- An invalid `at_time` is rejected at creation, with an example.
- A full-map schedule cannot be created with gaps shorter than an hour, by any path.
- A duplicate name **in the same guild** is rejected; two different guilds can each have a
  schedule named "daily".
- Reconnecting the gateway does not increase the number of scheduled ticks or duplicate posts.
- With schedules in two guilds on different shards, after a restart each renders its own shard.
- A member without Manage Webhooks cannot use the command.
- A scheduler rejection leaves no row behind.
