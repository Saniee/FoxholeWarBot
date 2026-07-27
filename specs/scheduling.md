# Scheduling system

Status: **shipped.** Cross-cutting spec for the scheduling subsystem — the `CronHandler`, the
`cronjobs` table, and startup restoration. Per-command surfaces live in
`specs/schedule-report.md` and `specs/remove-report.md`.

This subsystem carried the most severe defects in the QA sweep (C-6, C-7, C-8, C-9, B-2, B-3,
S-4); all are fixed. The "fixes X" annotations below are kept so the reasoning stays attached
to the design.

## Goals
1. A given schedule fires exactly on its cadence — never duplicated by gateway reconnects.
2. Each scheduled report always uses **its own guild's** shard and settings.
3. A single broken webhook or guild never breaks restoration of other jobs.
4. Job names are unique **per guild**, not globally.
5. Only authorized members can create/remove schedules.

## Scope
- `/schedule-report`, `/remove-report`, the `CronHandler`, and the `cronjobs` table.
- The map rendering path is shared with `/get-map`; see the C-1 fix (unique output) which both
  must adopt.

## Behavior

### Startup restoration (fixes C-6, C-7, C-8)
- Restoration runs **once per process**, guarded by shared state on `Handler`
  (`AtomicBool`/`OnceCell`), not a value-typed local in `ready`.
- For each `cronjobs` row, resolve the owning guild via its `guild` foreign key
  (`JOIN guilds ON guilds.id = cronjobs.guild`) — never reuse `jobs[0]`'s guild.
- A row whose webhook URL fails to resolve is **skipped and logged** (optionally pruned), and
  restoration continues.

### Creating a schedule (`/schedule-report`) (fixes B-2, B-3, S-4)
- Gate behind **`MANAGE_WEBHOOKS`** via `default_member_permissions` (decided — the command
  creates and deletes webhooks, so it's the permission that actually matches the action; it also
  guarantees the bot's own webhook calls won't be attempted by someone who lacks the right).
- Uniqueness is `(guild, job_name)`; a name already used **in this guild** is rejected, but the
  same name in another guild is allowed.
- Schedule the job first; only persist the row after the scheduler accepts it, then store the
  UUID — no orphan rows on failure.
- Handle missing "Manage Webhooks" permission with a clear reply (fixes C-11).

### Timezones, and the nightly rebuild
Each `cronjobs` row stores an IANA name (`Europe/Bratislava`), never a fixed offset — an offset
stored in summer is wrong in winter. It is resolved when the schedule is created and kept on the
row, so changing the guild's default never moves a report that already exists.

**Storing the name is necessary but not sufficient.** `Job::new_async_tz` does not track a
timezone: it calls `offset_from_utc_datetime` once at construction and keeps the resulting fixed
offset for the life of the job. A schedule created in January fires an hour out all summer, and
only a process restart corrects it.

So a **nightly job rebuild** (04:20 UTC) re-registers every stored schedule from its stored name,
unscheduling the current registration first. Any transition is corrected within a day of itself,
without anyone noticing there was something to correct. It shares one code path with startup
restoration — the only difference is that the rebuild removes the existing registration before
adding the replacement, and at startup there is nothing to remove because the stored UUID belongs
to a previous process.

Rejected alternatives: rebuilding only on each zone's two transition dates (more code, per zone,
to save seconds of work a day), and documenting that DST applies at restart (cheapest, and the
wrong answer for a bot whose complaint of record is "it fires at the wrong time").

### Per-tick execution (fixes C-1, C-3, C-5)
- Renders to an in-memory attachment; no file is written, so ticks and interactive
  commands cannot collide.
- No `.unwrap()` on `send`/`json`/`webhook.execute`/`from_timestamp_millis`; a failed tick logs
  and returns without panicking.
- Continue to revalidate via ETag against the owning guild's shard.

### Removing a schedule (`/remove-report`) (fixes C-9, L-3)
- Same permission gate as creation.
- Case-insensitive autocomplete filtering.
- Deleting an already-removed webhook is treated as success, not a panic.

## Data model
```sql
-- cronjobs: composite uniqueness, replacing the old global UNIQUE(job_name)
UNIQUE (guild, job_name)
-- the guild FK is NOT NULL + ON DELETE CASCADE, so removing a guild
-- cleans up its jobs.
```
Full schema: `specs/postgres.md`.

## Acceptance criteria
- Reconnecting the gateway (or forcing multiple `ready` events) does **not** increase the number
  of scheduled ticks or duplicate posts.
- With schedules in two guilds on different shards, after a process restart each guild's report
  renders its own shard's data.
- A deleted webhook (manual) neither aborts restoration nor panics `/remove-report`.
- Two guilds can each have a schedule named "daily".
- A non-privileged member cannot create or remove schedules.
- No scheduling code path can leave a deferred interaction without a final reply.
- A schedule keeps firing at its stated wall-clock time across a DST transition, with no restart
  and no user action.
- Every schedule created before timezones existed keeps firing exactly when it did: both columns
  default to `'UTC'`, and an English phrase still parses.

## Related
- **Full-map gating** — scheduling a *full-map* report requires a (non-monetary) approval, for
  every guild regardless of size; on-demand full-map renders stay free and ungated for all. The
  job carries a **NULL `map_name`** rather than an `is_full_map` flag, so there is one fact and
  not two that can disagree, and the tick re-reads the approval so it can be withdrawn. The
  approval decides *whether*, not how often: the once-an-hour floor on full-map schedules lives
  in `/schedule-report`. See `specs/active/premium-full-map.md`.

## Out of scope (future)
- Listing schedules (`/list-reports`) — currently only removable via autocomplete.
- Editing an existing schedule in place (today: remove + recreate).
