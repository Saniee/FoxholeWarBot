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
- [ ] `cargo clippy` clean
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
- **Logging** (`dev/active/logging/`) — log files, a verbose second stream, and a mounted log
  volume. Written, not yet built. Not part of the 2.0 tag unless it lands first; it touches no
  migration and no command surface

## Migrations in this release
`0001`–`0005`, applied at startup by `sqlx::migrate!`. `0005` adds `guilds.timezone`,
`cronjobs.timezone` and `cronjobs.schedule_label`, all defaulting so existing schedules keep
firing exactly as they did. Next free number is **0006**.
