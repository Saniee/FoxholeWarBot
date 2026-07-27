# Tasks: structured schedule input + timezones

Spec: `specs/schedule-input.md` — **shipped**, promoted out of `specs/active/` for 2.0.

## Built
- [x] `migrations/0005_schedule_input.sql` — `guilds.timezone`, `cronjobs.timezone`,
      `cronjobs.schedule_label`; both timezones default `'UTC'` so nothing existing moves
- [x] `docs/tos.md` + `docs/privacy.md` stored-data lists, **in the same commit as the migration**
- [x] `src/utils/schedule.rs` — `Frequency` (8 intervals + daily + weekly + `Custom…`), `Day`,
      cron generation, IANA timezone resolution, fire-time preview, gap measurement
- [x] `/schedule-report`: `frequency` + `at_time` + `timezone` + `day` + `custom`, replacing the
      free-text `schedule` option
- [x] Next-three-fire-times preview before anything is stored — on the choice list as well as
      `Custom…`
- [x] One-hour floor for full-map schedules, measured from the preview so `Custom…` is covered
- [x] **30-minute floor for every schedule** (user call: anything faster reads as spam).
      `Every 15 minutes` dropped from the list rather than offered and then refused
- [x] "Nothing posts right now" in the creation reply, next to the first fire time — the
      "I made a schedule and nothing happened" report
- [x] Per-tick placeholder, **edited into** the finished report (one message per run), with a
      failure notice if the render fails and a post-fresh fallback if the edit fails
- [x] `/set-guild-settings timezone`, validated before storing, "leave it alone" when omitted
- [x] Nightly job rebuild (04:20 UTC) — the fix for `new_async_tz`'s snapshotted fixed offset
- [x] `<t:epoch:F>` + `<t:epoch:R>` in the report embed, and the cadence in words
- [x] `/schedule-help` repointed from "phrases you may type" to "what the frequencies mean and
      which clock they're read in"
- [x] Specs reconciled: `schedule-report.md`, `scheduling.md`, `schedule-help.md`,
      `set-guild-settings.md`, `postgres.md`; both spec questions answered in `schedule-input.md`

## Verified
- [x] Builds; `chrono-tz` and `croner` resolved
- [x] Walked by the user — boot, both schedule paths, timezones and the floors all behave
      ("everything works")
- [x] Spec promoted out of `specs/active/`

Not walked, and knowingly so: the nightly rebuild across a real DST transition. Seeing it
properly means moving its cron to a minute away, and the code path is the same one startup
restoration uses on every boot.

## Left for the release
Tracked in `dev/done/full-map/tasks.md` → release checklist: the 90-day request purge (needs a
backdated `reviewed_at`) and peak memory during a full-map render. Neither blocks the tag.
