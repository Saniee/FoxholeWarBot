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

## Current state
Branch `feat/rewrite-overhaul` → PR #1. **The core rewrite is written** (commits `dbab23a`
data layer, `5638ac4` poise + QA fixes). All 7 sections of `tasks.md` are done.

**Compiles clean** (`cargo build` + `cargo clippy`, no warnings) as of `c3288e5`. Verified by
the user at home, not in this environment. Docker build also fixed (font needed at compile
time; libssl3 needed at runtime). Still unverified: actually running against a live Postgres
and Discord.

## Ground rules for this run
- **Do NOT run `cargo build` / `check` / `run`** — user explicitly abstained for this pass.
- **Ask, don't assume** on unspecced decisions (user is responsive).
- Commit + push incrementally to `feat/rewrite-overhaul` (updates PR #1). Never push elsewhere.
- Attribution footer required on any GitHub comment.

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

## Known gaps / what to verify at home
1. ~~`cargo check`~~ — done, clean.
2. `docker compose up` — confirm migrations apply and the bot connects.
3. Visual check of a `/get-map` render: `Anchor::Center` intentionally shifts markers up-left
   by half an icon vs. the old output. Flip to `Anchor::TopLeft` in `RenderConfig::default()`
   to A/B it.
4. Scheduling paths (C-7/C-8/C-9/B-3) need two schedules in different guilds plus a process
   restart to actually exercise — the least-covered part of the rewrite.
5. sqlx has no TLS feature enabled — fine for local compose, needs `tls-rustls` if the DB ever
   moves to a remote host requiring SSL.

## Next steps
Compile and iterate. After that, the deferred specs in priority order: docs & legal overhaul,
then full-map renderer, then the premium/form gating.
