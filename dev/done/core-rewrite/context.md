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

## Follow-on: specs reconciled (done, `92f04c9`)
`specs/` now describes the shipped bot, not the thing it replaced. Per-command specs rewritten;
`postgres.md` / `rendering-placement.md` / `scheduling.md` promoted out of `specs/active/`;
`qa-report.md` kept and marked resolved with an index (its IDs are cited from commits and code
comments, so they must stay resolvable). `specs/active/` now holds only pending work.

## Follow-on: docs & legal overhaul (done)
`docs/` now matches the code. Removed three false claims (owner ID stored, bot DMs owners about
errors, Add Reactions / Send Messages in Threads required); the stored-data list is now the
`guilds` + `cronjobs` schema in prose; Siege Camp attribution added to `index`, `tos` and the
README; `faq.md` finally has `layout: default` (it had been rendering unthemed); the support
invite is the `discord.gg` form in docs, README and `SUPPORT_INVITE` alike; README rewritten
(it still advertised `/set-shard` and `/set-visibility`, which the rewrite replaced).

`specs/active/docs-legal-overhaul.md` is gone: what shipped is described by `specs/docs-site.md`,
and the parts that couldn't ship — the request-form docs and the data it collects — moved into
`specs/active/premium-full-map.md` → "Docs impact", to be written **in the same commit** as the
schema change. Documenting data the bot doesn't yet store is exactly the failure the overhaul
was fixing.

## What's next
Both remaining specs — `full-map-renderer.md` and `premium-full-map.md` — are now being taken on
together as one task: **`dev/active/full-map/`**. Read that instead; this file is history.

**Gotcha for whoever starts premium-full-map:** its spec describes adding columns to `guilds`
(cached member count, approval flag) and a `full_map_requests` table. The schema is now live, so
that is a **new** `migrations/0002_*.sql` — not an edit to `0001_init.sql`, which has already run
against real databases.
