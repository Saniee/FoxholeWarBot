# /usage-stats

Status: **shipped.**

## Summary

Tells whoever runs the bot how much it is being used: the one-shot commands, the schedule system,
and how many servers each reaches — as a daily table and a weekly one.

## The question this exists to answer

Not "who used what". The bot is a hobby project on hobby infrastructure, and the decisions it
feeds are about **load and worth**: is the scheduling machinery — the nightly rebuild, the
restore-on-boot, the approval queue, the dormancy notices — carrying its weight against the
number of reports it actually delivers, and does anyone reach for `/get-map` and `/full-map`
enough to justify what they cost.

That framing is what fixes every design decision below. A tally is enough to answer it; an
activity log is not needed to answer it; and nothing about a *person* is needed to answer it at
all.

## Command surface

- `/usage-stats` — global figures, across every server.
- Options:
  - `days` (integer, optional, 1–14, default 7) — how many daily buckets.
  - `weeks` (integer, optional, 1–8, default 4) — how many weekly buckets.
- `default_member_permissions = "ADMINISTRATOR"`, which only keeps it out of the picker. The real
  gate is `review::is_reviewer` — the same `REVIEWER_IDS` list that decides full-map requests.
- Always ephemeral, in the same sense `/full-map-requests` is: the reply aggregates every server
  the bot is in, and no one server's channel is the place to publish that.

## What is counted, and under what name

One table, `usage_daily`, with a row per `(day, command, guild_id)` and a `uses` counter.

| Source | Recorded name | Recorded by |
|---|---|---|
| Any slash command | its **qualified** name (`get-map`, `full-map-requests list`) | poise's `pre_command` hook (`main.rs` → `utils::usage::record_command`) |
| A delivered single-region report | `scheduled-report` | the tick, in `utils::cron::run_report` |
| A delivered full-map report | `scheduled-full-map` | the same |

The two synthetic names share the command column deliberately. It is what lets one table answer
"how much work did the schedule system do" beside "how often did anyone type `/get-map`", and
they cannot collide with a real command — nothing in `commands::all()` is named either.

`qualified_name` rather than `name`, so a subcommand is counted as `full-map-requests list`
rather than as a command called `list` that would merge with the `list` of anything added later.

### Commands count on invocation, deliveries count on arrival

Deliberately different, because the two words mean different things.

A **command** is counted in `pre_command`, before the body runs, so a `/full-map` that failed
because the Foxhole API was down still counts. That is the honest measure of demand: someone
wanted the full map. Counting only successes would make the numbers look healthiest on exactly
the days the bot worked worst.

A **delivery** is counted at the end of `run_report`, after the report reaches the channel. A
failed render returns before it, and a dormant full-map schedule never reaches it. Here the
question really is "did a report go out", and counting attempts would make a stalled schedule
indistinguishable from a busy one.

## Behavior

1. Defer ephemeral; refuse anyone not in `REVIEWER_IDS`.
2. Clamp `days` and `weeks` (Discord enforces `min`/`max`, but Discord is a client we don't
   control).
3. Read three things: the standing overview, the daily buckets, the weekly buckets.
4. Reply with one embed — the overview as prose, then two fixed-width tables in code fences.

### The overview is read live, not tallied

Servers configured, servers approved for full-map schedules, live schedules and how many are
full-map, and how many servers have any schedule at all — all counted from `guilds` and
`cronjobs` at read time.

These are facts about the present, and a tally of the past can only approximate them. A schedule
somebody created this morning that has not fired yet contributes nothing to `usage_daily` and is
exactly what a person asking "is the schedule system used" wants to know about.

### The tables

Columns: `get-map`, `full-map`, `reports` (both delivery kinds), `sch-cmd` (`schedule-report`,
`remove-report`, `schedule-help`, `request-full-map-schedule`), `other`, `servers`.

`reports` and `sch-cmd` are separate on purpose. One is people **setting schedules up**, the other
is what those schedules **produced**, and one server with an hourly report is a very different
picture from twenty servers that each set one up and never got posted to.

Buckets are UTC days, and weeks are the Monday-start weeks `date_trunc('week', …)` produces.
Everything the scheduler does is UTC (`specs/scheduling.md`), and a deployment's local midnight is
not a fact worth having in the data.

**A bucket with no usage is still a row, of zeroes.** The query returns nothing for a quiet
Sunday, so the labels are generated in Rust and the gaps filled — a table that silently skips a
day reads as lost data rather than as a quiet day.

Fixed-width text inside a code fence because an embed is proportional everywhere else, so a table
built out of spaces outside a fence is not a table. `MAX_DAYS`/`MAX_WEEKS` are sized so the
result always fits Discord's 1024-character field; the renderer drops whole rows if it ever
doesn't, rather than truncating the finished string, since a cut that lands inside the closing
fence turns the rest of the embed into code.

## Storage

See `migrations/0007_usage_stats.sql`.

```sql
CREATE TABLE usage_daily (
    day      DATE    NOT NULL,   -- UTC
    command  TEXT    NOT NULL,
    guild_id BIGINT  NOT NULL,   -- Discord snowflake, NOT a reference to guilds.id
    uses     INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (day, command, guild_id)
);
```

- **A tally, never an event log.** This is the privacy decision, not an optimisation. A row per
  invocation would be a record of what each server did and when — a different thing to store and a
  different sentence in `docs/privacy.md`. A counter can say "this server used `/full-map` 40
  times on Tuesday" and can never say when, in what order, or by whom. **No user id is stored
  anywhere in this feature**, which keeps `docs/privacy.md`'s claim — that the only personal
  identifier the bot holds is on a full-map request — true without amendment.
- **`command` is free text, not an enum.** A command added later must not need a migration before
  it can be counted, and a name that falls out of use just stops appearing.
- **`guild_id` is the snowflake and has no FK**, because a command can be invoked by a server that
  has never run `/set-guild-settings` — it gets the setup prompt — and those attempts are some of
  the most interesting there are. An FK would have nothing to point at, so they would be dropped
  or force a nullable key column. The cleanup an FK would have given is done explicitly in
  `Database::delete_guild`, in the same transaction as the settings.
- **No second index.** The primary key's btree already leads with `day`, which is what every read
  and the purge filter on.
- The date comes from `now() AT TIME ZONE 'utc'`, never `CURRENT_DATE` — that is read in the
  database session's timezone, so which day a use landed in would depend on how the Postgres
  container happens to be configured, and would move silently if that ever changed.
- The increment is an upsert (`ON CONFLICT … DO UPDATE SET uses = usage_daily.uses + 1`), so two
  commands finishing in the same millisecond in the same server contend on one row and both land.
  There is no read-then-write for them to race in.

### Reading it back

One query per period, using `GROUPING SETS ((bucket, command), (bucket))`. The extra
`command IS NULL` row per bucket carries that bucket's distinct-server count, which **cannot** be
derived from the per-command rows: a server that ran three commands on Tuesday is one server, and
adding the per-command distinct counts would report three. One query rather than two so both
halves come from the same scan — a use recorded between two queries would appear in one and not
the other.

## Retention

90 days, deleted by `start_usage_purge_job` at 03:40 UTC daily — its own job rather than a second
statement inside the request purge, because they enforce two separate promises and a failure in
one must not skip the other.

The window matches the one closed full-map requests already get, so `docs/privacy.md` and
`docs/tos.md` gain one consistent sentence rather than a second, differently-shaped rule.

Removing the bot from a server deletes its counts along with its settings and schedules, in one
transaction (`Database::delete_guild`).

## External calls

None. Postgres only — no Foxhole API, no Discord API beyond the reply itself.

## Notes

- Counting is never allowed to matter. `utils::usage::record` swallows a failed write as a
  `debug!`, not a `warn!`: whatever real work is failing at the same moment is already being
  reported, and a per-command line about the statistics would be the loudest thing in the log and
  the least useful — on exactly the sinks `specs/architecture.md` keeps quiet on purpose.
- `/usage-stats` counts itself, like every other command. It is reviewer-gated, so the number is
  small and honest; excluding it would be a special case in the recording path whose only effect
  is to make one row of a table the operator reads slightly tidier.
- The counts are **global and not per server**, and there is no per-guild view. A server admin
  asking "how much do we use this bot" is a different feature with a different privacy shape, and
  nothing in this one is built to be reused for it.

## Acceptance criteria

- `/usage-stats` refuses anyone not in `REVIEWER_IDS`, ephemerally, and shows nothing.
- Running any slash command increments exactly one row, for today's UTC date, that command's
  qualified name, and the invoking server.
- A command that errors is still counted; a scheduled report that fails to render is not.
- A single-region delivery counts as `scheduled-report`, a full-map delivery as
  `scheduled-full-map`, and a dormant full-map tick counts as neither.
- Every requested bucket appears in its table, including buckets with no usage.
- A bucket's `servers` figure counts each server once, however many commands it ran.
- Rows older than 90 days are gone after the nightly purge.
- Removing the bot from a server leaves no `usage_daily` rows for it.
- No user id is written by any part of this feature.
