# Tasks: structured schedule input + timezones

Spec: `specs/active/schedule-input.md`

## Written (none of it compiled yet)
- [x] `migrations/0005_schedule_input.sql` — `guilds.timezone`, `cronjobs.timezone`,
      `cronjobs.schedule_label`; both timezones default `'UTC'` so nothing existing moves
- [x] `docs/tos.md` + `docs/privacy.md` stored-data lists, **in the same commit as the migration**
- [x] `src/utils/schedule.rs` — `Frequency` (9 intervals + daily + weekly + `Custom…`), `Day`,
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

## Next
- [ ] **`cargo build`** — nothing here has been compiled. New deps: `chrono-tz`, `croner`
- [ ] Boot: `0005` applies, existing schedules restore, the rebuild job registers
- [ ] `/set-guild-settings` with and without `timezone`; omitted must not reset it
- [ ] A choice-list schedule end to end — preview matches the label, report arrives on time
- [ ] `Custom…`, with a phrase and with cron
- [ ] Floors: a region schedule refused under 30 minutes, a full-map one under an hour
- [ ] Changelog draft (`dev/changelog-rewrite.md`) — read it, fix the date, post it
- [ ] The rebuild, by temporarily moving its cron to a minute away — the only practical way to see
      it without waiting for a DST transition
- [ ] `cargo clippy` clean
- [ ] Promote the spec out of `specs/active/`

## Carried over from the full-map task (`dev/active/full-map/`)
Still open, still not blocking: the 90-day request purge (needs a backdated `reviewed_at`) and
peak memory during a full-map render. Both are release-gate items, not development ones.
