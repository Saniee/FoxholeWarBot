# Context: Core rewrite (poise + Postgres)

Specs (the plan — do not duplicate here):
- `specs/architecture.md` — cross-cutting contracts + poise framework design
- `specs/qa-report.md` — ranked crash/QA findings to fix (C-*, B-*, S-*, L-* ids)
- `specs/active/postgres-migration.md` — SQLite → Postgres
- `specs/active/scheduling-overhaul.md` — cron lifecycle fixes
- `specs/active/rendering-placement.md` — de-magic RenderConfig
- `specs/*.md` — per-command 1:1 behavior specs

Deferred, NOT in this pass: `specs/active/full-map-renderer.md`,
`specs/active/premium-full-map.md`, `specs/active/docs-legal-overhaul.md`.

## Status: DONE
Branch `feat/rewrite-overhaul` → PR #1. All 7 sections of `tasks.md` complete.

Verified by the user at home: `cargo build` and `cargo clippy` clean, `docker compose up`
brings up Postgres + bot with migrations applied, and map rendering is correct. The
`Anchor::Center` change was reviewed and accepted.

Key commits: `dbab23a` data layer, `5638ac4` poise + QA fixes, `c3288e5` Docker build.

## Decisions made
- poise 0.6, shared `Data { db, cron, local }` via `ctx.data()`; run-once `setup` closure
- Postgres, fresh start; `sqlx::migrate!` + `migrations/`; BIGINT snowflakes, native BOOLEAN
- `Anchor::Center` default, `Anchor::TopLeft` kept as A/B escape hatch
- `MANAGE_WEBHOOKS` gates both scheduling commands
- **New (this run):** no guild row is created on join — a new server is prompted to run
  `/set-guild-settings` instead of silently defaulting to shard Able (user chose this)
- **New:** `set-guild-settings` shard is a Discord choice list, not free-text autocomplete
- **New:** `env_logger` added — the `log::` calls previously had no consumer
- **New:** `OriginHex` is no longer excluded from autocomplete (per full-map spec)
- **New:** `schedule-help` is ungated and lists phrases inline (the prnt.sc link is gone)
- **New:** `Cargo.lock` is committed (whitelisted in `.gitignore`), so container and local
  builds resolve identical versions

## Carried forward (not blockers, but worth knowing)
- **Scheduling paths are the least-exercised code in the rewrite.** C-7 (per-guild shard
  resolution), C-8 (skip a bad webhook), C-9 and B-3 only show themselves with schedules in
  two guilds plus a process restart. Everything else has been run.
- **sqlx has no TLS feature enabled.** Fine for the local compose setup; needs `tls-rustls`
  if the database ever moves to a remote host requiring SSL.
- `Anchor::TopLeft` is now unused-but-kept (`#[allow(dead_code)]`). Once the centered render
  has lived a while, it can go.

## What's next
Deferred specs, in priority order:
1. `specs/active/docs-legal-overhaul.md`
2. `specs/active/full-map-renderer.md` (depends on `rendering-placement.md`, which landed)
3. `specs/active/premium-full-map.md`
