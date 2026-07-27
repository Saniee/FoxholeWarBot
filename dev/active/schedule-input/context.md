# Context: structured schedule input + timezones

Spec: `specs/active/schedule-input.md`

## Current state
**Written in full, compiled never.** Everything in the spec is implemented — migration `0005`,
`utils::schedule`, the new `/schedule-report` surface, the timezone option on
`/set-guild-settings`, the nightly DST rebuild, `<t:epoch:F>` in the embed, and rewritten
`/schedule-help`. Docs and the four affected specs are updated in the same commit as the
migration.

Nothing has been built or run. `Cargo.lock` is tracked and does **not** yet contain the two new
dependencies (`chrono-tz`, `croner`) — the first `cargo build` will update it, and that build
needs network.

## Key files
- `src/utils/schedule.rs` — new. `Frequency`/`Day` choice enums, `cadence()` (the pick → cron),
  `timezone()`/`timezone_or_utc()`, `next_fires()`, `shortest_gap_minutes()`, `preview_lines()`.
  Everything about turning a user's pick into a schedule is here and only here.
- `src/utils/cron.rs` — `ReportJob` carries `timezone` + `schedule_label`; `schedule()` uses
  `Job::new_async_tz`; `load_jobs(replace)` serves both startup restore and the nightly rebuild;
  `start_job_rebuild_job` (04:20 UTC).
- `src/commands/schedule_report.rs` — the new option set, the preview, the full-map hourly floor.
- `src/commands/common.rs` — `autocomplete_timezone` (guild's own zone first when the box is
  empty, then a common list; `_` and space interchangeable when filtering).
- `migrations/0005_schedule_input.sql` — `guilds.timezone`, `cronjobs.timezone`,
  `cronjobs.schedule_label`. Next free number is **0006**.

## Decisions taken while building (beyond the spec)
- **Weekly ships**, against the spec's own suggestion to defer it. The old free-text box accepted
  "every Friday at 18:00", so deferring would have *removed* something that worked. `day` defaults
  to Monday, so it's never a required-but-not-really option.
- **The full-map hourly floor is measured from the previewed fire times, not read off the
  expression.** `*/5` and a hand-written list of twelve minutes are the same problem and only one
  of them looks like it, so `Custom…` is held to the same rule for free.
- **Minutes and hours are written out in full** (`0 30 3,9,15,21 * * *`), never `*/6`. Nothing then
  depends on how a parser reads a step, and the label can state the times exactly.
- **`croner` is a direct dependency** so the preview is computed by the same parser the scheduler
  runs, with the same `with_seconds_required().with_dom_and_dow()` chain. A preview from a
  different implementation is a guess.
- **The preview is shown on the choice-list path too**, not just `Custom…` — "every 6 hours"
  meaning absolute clock times is exactly the surprise it exists to catch.
- **The guild timezone is "leave it alone" when omitted**, like `faction_tint`. `shard` and
  `show_messages` are required every time, so anything else defaulting on update would reset a
  server's clock underneath its schedules.
- **Timezone validated before storing**, in both commands, so a typo can't sit in a row until the
  next schedule quietly falls back to UTC.

## Risks / gotchas
- **The nightly rebuild is what makes stored timezones true**, not housekeeping.
  `Job::new_async_tz` snapshots a fixed offset at construction and never re-reads the zone.
- The rebuild unschedules before re-adding, so a `schedule()` failure mid-rebuild leaves that one
  report stopped until the next rebuild or restart. Chosen deliberately over the duplicate-post
  risk of the other order.
- The preview uses the real zone (correct across DST); the scheduler uses a fixed offset. They can
  disagree for at most a day after a transition, which is what the rebuild closes.
- `chrono-tz` is pinned `0.10` and was never resolved here — if the version is wrong the build
  says so immediately.
- Discord requires **required options before optional ones**; `/schedule-report`'s option order is
  registration order, not importance. Reordering it casually will fail registration.
- User compiles and runs locally; **don't run `cargo build`/`check`/`run`**.

## Next steps
**Build it.** Expect the first failures in `utils::schedule` (croner/chrono-tz API shapes) and in
poise's derive for `Frequency`/`Day`. Then walk:
1. Boot: migration `0005` applies, existing schedules restore, "started the nightly schedule
   rebuild" appears.
2. `/set-guild-settings` with and without `timezone` — the second must not reset it.
3. A region schedule on the choice list; check the three previewed times against the label.
4. `Custom…` with a phrase and with cron; check the preview catches a wrong-looking one.
5. A full-map schedule under an hour — must be refused; at an hour — must be accepted.
6. The report embed: cadence in words, and the next run on the reader's own clock.

Testing the DST rebuild properly means either waiting for a transition or temporarily pointing the
rebuild job at a minute away. The second is the only practical check.
