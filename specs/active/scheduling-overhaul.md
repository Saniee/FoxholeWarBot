# Scheduling system overhaul (ACTIVE)

Status: **proposed** — promoted from `specs/schedule-report.md` / `specs/remove-report.md`.
The scheduling subsystem carries the most severe defects in the QA sweep (C-6, C-7, C-8, C-9,
B-2, B-3, S-4). This spec describes the **intended** behavior for the rewrite, not the current
one.

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

## Intended behavior

### Startup restoration (fixes C-6, C-7, C-8)
- Restoration runs **once per process**, guarded by shared state on `Handler`
  (`AtomicBool`/`OnceCell`), not a value-typed local in `ready`.
- For each `cronjobs` row, resolve the owning guild via its `guild` foreign key
  (`JOIN guilds ON guilds.id = cronjobs.guild`) — never reuse `jobs[0]`'s guild.
- A row whose webhook URL fails to resolve is **skipped and logged** (optionally pruned), and
  restoration continues.

### Creating a schedule (`/schedule-report`) (fixes B-2, B-3, S-4)
- Gate behind `MANAGE_WEBHOOKS` (or `MANAGE_GUILD`) via `default_member_permissions`.
- Uniqueness is `(guild, job_name)`; a name already used **in this guild** is rejected, but the
  same name in another guild is allowed.
- Schedule the job first; only persist the row after the scheduler accepts it, then store the
  UUID — no orphan rows on failure.
- Handle missing "Manage Webhooks" permission with a clear reply (fixes C-11).

### Per-tick execution (fixes C-1, C-3, C-5)
- Render to a unique/in-memory attachment, not the shared `render.png`.
- No `.unwrap()` on `send`/`json`/`webhook.execute`/`from_timestamp_millis`; a failed tick logs
  and returns without panicking.
- Continue to revalidate via ETag against the owning guild's shard.

### Removing a schedule (`/remove-report`) (fixes C-9, L-3)
- Same permission gate as creation.
- Case-insensitive autocomplete filtering.
- Deleting an already-removed webhook is treated as success, not a panic.

## Data model changes
```sql
-- cronjobs: replace global UNIQUE(job_name) with a composite uniqueness
UNIQUE (guild, job_name)
-- consider NOT NULL + ON DELETE CASCADE for the guild FK so removing a guild
-- cleans up its jobs.
```

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
- **Premium gating** — scheduling a *full-map* report is entitlement-gated (on-demand full-map
  renders stay free). The full-map job type carries an `is_full_map` marker so the tick applies
  the gate. See `specs/active/premium-full-map.md`.

## Out of scope (future)
- Listing schedules (`/list-reports`) — currently only removable via autocomplete.
- Editing an existing schedule in place (today: remove + recreate).
