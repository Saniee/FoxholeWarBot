# /schedule-report 🔧

> Needs an overhaul. Current behavior is documented here for 1:1 reference; the intended
> reworked behavior lives in `specs/active/scheduling-overhaul.md`.

## Summary
Creates a recurring scheduled map report that posts a rendered hex map to a channel via a
webhook on a cron/english-phrase schedule.

## Command surface
- Name: `schedule-report`
- Options (all **required**, in order):
  - `map-name` (string, autocomplete) — hex to render.
  - `schedule-name` (string) — **must be globally unique** (see quirks).
  - `report-channel` (channel) — where reports post.
  - `schedule` (string) — english phrase, e.g. "every 30 seconds", "at 5:00 pm",
    "On Monday at 8:00".
  - `draw-text` (boolean) — overlay text labels.
- Permissions: none declared (any member can create one — see qa-report S-4).

### Autocomplete
`map-name` only, same behavior as `/get-map`.

## Behavior
1. Look up guild; if not set up → ephemeral prompt.
2. Defer public/ephemeral per `show_command_output`.
3. Validate `schedule` via `Job::schedule_to_cron`; on failure → "not formated acordingly".
4. Find or create a webhook named "Scheduled Map Report Webhook" in `report-channel`.
5. `add_report_job(from_db=false)`:
   - Insert the `cronjobs` row (guild id, name, cron, webhook URL, map, draw_text).
   - Schedule the job; on success write back the scheduler UUID.
   - If the job name is already taken → reply "already has a name that's being used".
6. Reply "created!" pointing at the channel.

## Scheduled job execution (per tick)
See `architecture.md` and `cron.rs`. Each tick: reload guild, revalidate dynamic+static via
ETag, render, and `webhook.execute` an embed titled "Scheduled Report: <name>" with a
"Last API Update" / "Next Scheduled Update" description.

## Quirks & known bugs (why it needs an overhaul)
- **Duplicate jobs after every reconnect** — `restart_report_jobs` runs on every `ready`
  event because the `cron_jobs_restarted` guard is a local variable that's reset each time
  (`main.rs:170`, dead-write confirmed by clippy). Discord fires `ready` on every
  resume/reconnect, so scheduled reports multiply and the daily map-update job stacks.
  (qa-report: C-6)
- **Cross-guild data corruption on restart** — `restart_report_jobs` fetches the guild for
  `jobs[0]` only and reuses that single `guild_id` for *every* restarted job. After a restart,
  all scheduled reports across all guilds render/post using the first guild's shard.
  (qa-report: C-7)
- **Panic aborts all restarts** — a deleted webhook makes `Webhook::from_url(...).unwrap()`
  panic, killing the restart loop for every remaining job. (qa-report: C-8)
- **Globally-unique job names** — `cronjobs.job_name` is UNIQUE across all guilds, so two
  guilds can't both name a schedule "daily". (qa-report: B-2)
- **Shared `render.png`** — scheduled ticks and interactive commands write the same file.
  (qa-report: C-1)
- **No permission gate** — any member can create a webhook-backed schedule. (qa-report: S-4)
- Many unwrapped calls in the tick closure (`send`, `json`, `webhook.execute`,
  `from_timestamp_millis`, `webhook.url`). (qa-report: C-3)
- **Timezone mismatch** — schedules run in the scheduler's clock (UTC) while embeds format
  timestamps in `chrono::Local`. (qa-report: L-2)

## Acceptance criteria (current behavior to preserve)
- A valid `/schedule-report` creates a webhook (if absent) and begins posting on schedule.
- An invalid schedule phrase is rejected with a helpful message.
- A duplicate schedule name is rejected.
- (Deferred to overhaul: correct multi-guild restart, single restart per process,
  per-guild-unique names, permission gating.)
