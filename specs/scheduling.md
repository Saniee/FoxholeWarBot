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

## Related
- **Full-map gating** — scheduling a *full-map* report is free for small guilds but requires a
  (non-monetary) approval form for large guilds; on-demand full-map renders stay free for all.
  The full-map job type carries an `is_full_map` marker so the tick applies the gate. See
  `specs/active/premium-full-map.md`.

## Out of scope (future)
- Listing schedules (`/list-reports`) — currently only removable via autocomplete.
- Editing an existing schedule in place (today: remove + recreate).
