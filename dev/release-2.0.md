# Release 2.0 — what's done and what's left

Everything below is state at the time the release was prepped. Tick as you go.

## Done
- [x] `specs/active/` emptied — `full-map-renderer.md`, `premium-full-map.md` and
      `schedule-input.md` promoted and their status headers rewritten to **shipped**. Every
      `specs/active/...` reference in code comments, specs and `CLAUDE.md` repointed
- [x] `dev/active/` emptied — `full-map/` and `schedule-input/` moved to `dev/done/`
- [x] `Cargo.toml` (and the `Cargo.lock` package entry) at **2.0.0**
- [x] `scripts/update_assets.py --audit` exits 0 — 53 regions, one file each, names exact
- [x] Changelog written and final but for the date: `dev/changelog-2.0.md`
- [x] Docs site current: the stored-data lists in `docs/tos.md` and `docs/privacy.md` cover
      every column through migration `0005`, and `docs/faq.md` describes the schedule surface
      as built

## Left — do these before tagging
- [x] **Commit an updated `Cargo.lock`.** Done — `chrono-tz` 0.10.4 and `croner` 2.1.0, and as of
      the logging work `fern` 0.7.1 and `env_filter` 0.1.4 with `env_logger` gone
- [ ] `cargo clippy` clean. Currently **two warnings, both long-standing and both judgment calls
      rather than defects**: `schedule_report.rs:24` takes 10 arguments where clippy wants 7, and
      `db.rs:202` has a large size difference between enum variants. Decide whether to fix, `allow`
      with a reason, or accept — but decide, rather than leaving the box unticked because the
      output is noisy
- [ ] Tag `v2.0.0` and deploy: `docker compose up -d --build` (**never** `down -v` — it wipes
      the database)
- [ ] Post `dev/changelog-2.0.md` with the real date

## Known gaps, accepted for 2.0
- **90-day request purge** — never exercised. Backdate a denied/withdrawn row's `reviewed_at`
  past the window and watch the 03:30 job take it, or accept it on inspection.
  `docs/privacy.md` states the window, so something has to make it true; the query is
  `purge_stale_full_map_requests`
- **Peak memory during a full-map render** — never measured. 63.6 MP canvas, ~254 MB RGBA before
  downscale, against a container ceiling that isn't large. Worth watching on the first live
  full-map tick after deploy
- **The nightly DST rebuild across a real transition** — the code path is the one startup
  restoration uses on every boot, but it has only been seen doing that, not crossing a
  transition. First check is the next time the clocks move
- `REQUESTS_CHANNEL_ID` unset (should degrade to command-only review) and a two-reviewer race on
  one request (should answer "already decided by someone else") are both untested and low-stakes

## After 2.0
**Landed since this list was written, and therefore part of the 2.0 tag:**
- **Logging** (`dev/done/logging/`) — log files, a verbose second stream, and a mounted log
  volume. Touches no migration and no command surface
- **The frontline overlay** (`specs/frontline.md`) — migration `0006` adds `guilds.frontline`, and
  `docs/tos.md` + `docs/privacy.md` were updated in the same commit as the repo rule requires
- **The territory tint** (`specs/frontline-territory.md`) — the world map's faction colouring now
  follows the front rather than the hex. Config-only, no migration
- **The full-map status message** and **hex borders** (`specs/full-map-renderer.md`) — both
  config-only, no migration, no new command options

Not built, and deliberately: **activity as tint intensity**
(`specs/active/frontline-activity.md`) — tried as structure density, measured, rejected, reverted.
The reasoning is in the spec so it does not get re-proposed from scratch.

## Migrations in this release
`0001`–`0006`, applied at startup by `sqlx::migrate!`. `0005` adds `guilds.timezone`,
`cronjobs.timezone` and `cronjobs.schedule_label`; `0006` adds `guilds.frontline`. All default, so
existing guilds and schedules keep behaving exactly as they did — the frontline is off until a
guild asks for it. Next free number is **0007**.
