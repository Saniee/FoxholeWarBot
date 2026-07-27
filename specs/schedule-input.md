# Schedule input: structured options and timezones

Status: **shipped.** Superseded the free-text `schedule` option on `/schedule-report`; the
surface as built is described in `specs/schedule-report.md`, and the timezone machinery in
`specs/scheduling.md`.

Two reported problems, one root cause:
1. **Users can't get a phrase accepted.** `/schedule-report schedule:` is free text handed to
   `Job::schedule_to_cron`. When it doesn't parse, the command says so and the user guesses again.
2. **Reports arrive at the wrong time.** Nothing anywhere captures a timezone. Every schedule is
   UTC, and the embed says so in small print, so a guild that asked for "18:00" gets 18:00 UTC.

Both are the same mistake: the bot asks the user to express a time in a language it never told
them, then interprets it in a timezone they never chose.

## What the current input actually accepts

`tokio-cron-scheduler` 0.13's `english` feature. `Job::schedule_to_cron` tries an English phrase
first and falls through to treating the input as a raw 6-field cron expression, so both already
work today — the free-text box is a cron field wearing a costume, and users type into it as if it
were a chat message.

Worth knowing before designing the replacement:

- **"every 6 hours" is not "6 hours from now."** It becomes `0 0 */6 * * *` — 00:00, 06:00, 12:00,
  18:00, absolute clock times. A user who creates it at 09:20 expecting 15:20 is surprised at
  12:00, and their complaint reads as "it fires at the wrong time" even with no timezone involved.
  Some share of the timezone reports are probably this.
- **The seconds field is required** (`with_seconds_required`), so a user who knows five-field cron
  from crontab writes a valid-looking expression that silently means something else.

The fix for both is the same: **the bot builds the cron string; the user never types one.**

## Proposed command surface

Replace the `schedule` option on `/schedule-report` with:

| Option | Type | Notes |
|---|---|---|
| `frequency` | choice, required | Fixed list, below, plus `Custom…`. No parsing in the common path. |
| `at_time` | string, optional | `HH:MM`. The anchor — what the cadence lines up with. |
| `timezone` | string, optional, autocompleted | IANA name. Defaults to the guild's. |
| `custom` | string, optional | Only read when `frequency` is `Custom…`. See below. |

**Frequency choices:** every 30 minutes, hourly, every 2 / 3 / 4 / 6 / 8 / 12 hours, daily,
weekly (with a `day` option, or drop weekly from v1), and `Custom…`.

**Built with a floor of 30 minutes on every schedule** (60 for the full map), which is why the
15-minute option this spec originally listed isn't there. Enforced from the previewed fire times,
so `Custom…` can't undercut it.

`at_time` anchors the cadence rather than only applying to `daily`. "Every 6 hours at 03:30"
means 03:30, 09:30, 15:30, 21:30 — which is what people mean, and it's the piece the current
system can't express at all. Omitted, it means the top of the hour. For sub-hourly frequencies
only the minutes are used.

Generating the cron from `(frequency, at_time)` is a small total function with no failure mode:
every input is one of nine choices plus an `HH:MM` that either parses or is rejected with an
example. The only user-facing error left is a malformed `at_time`.

### The `Custom…` escape hatch (decided: keep power users, don't cost anyone else)
The list is what a non-technical user picks from; `Custom…` is the door for someone who knows
exactly what they want and finds nine choices confining. Two rules keep it from re-creating the
problem this spec exists to solve:

1. **It is never the default and never in the way.** A user who doesn't choose `Custom…` never
   sees the field, never reads about cron, and cannot fail at it.
2. **It must prove itself before anything is saved.** A custom expression is parsed at creation
   and the reply shows **the next three times it will actually fire**, in the chosen timezone,
   before the schedule is written. Nothing is stored until that is on screen.

That preview is the difference between a free-text box and a safe one. Today's failure isn't only
that the phrase is rejected — it's that an *accepted* phrase can mean something the user didn't
intend (`*/6` hours meaning 00:00/06:00/12:00/18:00, or a five-field expression that silently
shifts because the seconds field is required). Showing three real timestamps catches both, and
it's worth showing for the choice-list path as well.

Accept both a cron expression and the English phrases the current box takes — `schedule_to_cron`
already handles both, existing users have working phrases, and there's no reason to take that
away from the people it works for.

### Alternative considered: subcommands
`/schedule-report interval` and `/schedule-report daily`, so no option is ever inert. Cleaner
semantics, but it duplicates the five shared options across both and adds a step to the common
case. **Recommendation: single command with a choice list.** `at_time` isn't inert under the
proposal above — it applies to every frequency.

### `/schedule-help`
Mostly obsolete once the phrase box is gone; it exists to teach a syntax nobody will type.
Either delete it or repoint it at "what the frequencies mean and how timezones are handled."

## Timezones

**Store IANA names** (`Europe/Bratislava`), never fixed offsets. An offset stored in summer is
wrong in winter.

- **Guild default:** `/set-guild-settings` gains a `timezone` option; `guilds.timezone TEXT NOT
  NULL DEFAULT 'UTC'`. Most servers have one.
- **Per-schedule override:** the `timezone` option on `/schedule-report`, resolved at creation
  and stored on the row (`cronjobs.timezone TEXT NOT NULL DEFAULT 'UTC'`) so a later change to the
  guild default can't silently move an existing schedule.
- **Autocomplete** over the IANA list, so the name is always valid — this is exactly the class of
  input that must not be free text, which is the whole point of this spec.

Needs a new dependency: **`chrono-tz`** (`chrono` alone has no IANA database). Its `Tz` implements
`chrono::TimeZone`, which is what `Job::new_async_tz` wants.

### The DST trap, which is real and easy to miss
`Job::new_async_tz` does **not** track a timezone. It calls
`offset_from_utc_datetime(&Utc::now())` once at construction and keeps the resulting **fixed
offset** for the life of the job. A schedule created in January fires an hour late all summer,
and only a process restart fixes it.

So storing the IANA name is necessary but not sufficient. Options:
1. **Nightly rebuild** — a daily job re-registers every schedule from its stored IANA name.
   Reuses `restore_jobs`, which already does exactly this at startup, and self-heals within a day
   of any transition. **Recommended.**
2. Rebuild only on the two transition dates per zone — less work, more code, no real gain.
3. Ignore it and document that DST is applied at restart. Cheapest, and the wrong answer for a
   bot whose complaint of record is "it fires at the wrong time".

## Display

The embed prints `Next Scheduled Update: … UTC`. Replace with Discord's `<t:{epoch}:F>`, which
every reader sees **in their own timezone** with no work from us. Add `<t:{epoch}:R>` ("in 4
hours") beside it. This costs one line and may resolve a share of the "wrong time" reports on its
own, since some of them are a correct schedule described in a timezone the reader doesn't think in.

Show the schedule as a human phrase too ("every 6 hours at 03:30 Europe/Bratislava"), not the
generated cron.

## Data model

```sql
-- migrations/0005_schedule_input.sql  (0003 is the full-map gate, 0004 the review-post id)
ALTER TABLE guilds   ADD COLUMN timezone TEXT NOT NULL DEFAULT 'UTC';
ALTER TABLE cronjobs ADD COLUMN timezone TEXT NOT NULL DEFAULT 'UTC';
-- Human-readable cadence for embeds and listings; `schedule` holds the generated cron.
ALTER TABLE cronjobs ADD COLUMN schedule_label TEXT;
```

`cronjobs.schedule` keeps its meaning — a string `schedule_to_cron` accepts — so **existing rows
need no migration**: an English phrase already there still parses, and new rows store a generated
cron expression. `schedule_label` is NULL for old rows, which display falls back from.

Defaulting both timezone columns to `'UTC'` preserves today's behaviour exactly for every existing
schedule. Nobody's report moves because this shipped.

**Docs, in the same commit as the migration** (`specs/docs-site.md`): the stored-data lists in
`docs/tos.md` and `docs/privacy.md` gain the guild timezone, the per-schedule timezone, and the
cadence label.

## Decisions (settled — user's call, this session)
- **Timezone: guild default with a per-schedule override.** `/set-guild-settings` sets the
  server's; `/schedule-report` can override it for one report. The override is resolved at
  creation and stored on the row, so changing the guild default never silently moves a schedule
  that already exists.
- **Frequency: choice list *and* a `Custom…` escape hatch**, per the rules above. The stated goal
  is that someone non-technical can create a schedule without reading anything, and someone who
  knows cron isn't forced into nine boxes. The list serves the first, `Custom…` the second, and
  the fire-time preview is what stops the second from becoming the current problem again.
- **Existing schedules are left alone.** They keep firing exactly as they do now, in UTC, with
  their English phrase. Nothing moves because this shipped — which is also why both new timezone
  columns default to `'UTC'`.

## The two open questions, answered
1. **Weekly ships in v1**, against the earlier suggestion to defer it. The objection was the
   inert `day` option, but `custom` is inert too and this command already carries it, so weekly
   costs one more optional field rather than a new shape. The alternative was worse than it
   looked: the old free-text box *did* accept "every Friday at 18:00", so deferring weekly would
   have removed something that worked and pointed the people who wanted it at cron. `day`
   defaults to Monday rather than erroring, so it is never a required-but-not-really option.
2. **A one-hour floor for full-map schedules**, as suggested. Enforced by measuring the gaps
   between the previewed fire times rather than by reading the expression, so `Custom…` is held
   to the same rule — `*/5` and a hand-written list of twelve minutes are the same problem, and
   only one of them looks like it. Region schedules are unchanged; the floor exists because a
   full map re-renders all 53 regions each time.

## Acceptance criteria
- No command takes a free-text schedule phrase or a cron expression.
- A schedule created with a timezone fires at the stated wall-clock time in that zone, **including
  after a DST transition, without a restart.**
- Every existing schedule keeps firing exactly when it did before the change.
- The embed shows the next run in each reader's own timezone, and the cadence in words.
- An invalid `at_time` is rejected with an example, at creation.
- A user who never picks `Custom…` never encounters a free-text time field at all.
- Creating any schedule shows the next three fire times, in the chosen timezone, before it is
  saved — so an expression that parses but means the wrong thing is caught by the person who
  wrote it.
