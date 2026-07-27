# Context: Core rewrite (poise + Postgres)

Specs (the plan — do not duplicate here):
- `specs/architecture.md` — cross-cutting contracts + poise framework design
- `specs/qa-report.md` — ranked crash/QA findings to fix (C-*, B-*, S-*, L-* ids)
- `specs/active/postgres-migration.md` — SQLite → Postgres
- `specs/active/scheduling-overhaul.md` — cron lifecycle fixes
- `specs/active/rendering-placement.md` — de-magic RenderConfig
- `specs/*.md` — per-command 1:1 behavior specs (get-map, war-report, war-state,
  set-guild-settings, schedule-report, remove-report, schedule-help)

Deferred, NOT in this pass: `specs/active/full-map-renderer.md`,
`specs/active/premium-full-map.md`, `specs/active/docs-legal-overhaul.md`.

## Current state
Branch `feat/rewrite-overhaul` → PR #1. 9 commits, all specs/scaffolding; `src/` still
untouched and is the original raw-serenity + SQLite code. Starting the Core rewrite now.

## Ground rules for this run
- **Do NOT run `cargo build` / `check` / `run`** — user explicitly abstained for this first
  pass; they compile and verify at home. Write code, don't verify by compiling.
- **Ask, don't assume** on unspecced decisions (user is responsive, answering quickly).
- Commit + push incrementally to `feat/rewrite-overhaul` (updates PR #1). Never push elsewhere.
- Attribution footer required on any GitHub comment.

## Key files (current, pre-rewrite)
- `src/main.rs` — Handler/EventHandler, interaction_create match, inline CREATE TABLE
- `src/commands/*.rs` — 7 modules, each `NAME` + `run` + `register` (+ `autocomplete`)
- `src/utils/db.rs` — `Database`, `GuildData`, `JobData`, `Shard` enum
- `src/utils/cron.rs` — `CronHandler`, report job ticks
- `src/utils/cache.rs` — on-disk JSON cache + ETag versions
- `src/utils/request_processing.rs` — `place_image_info` (magic numbers live here)
- `src/utils/api_definitions/foxhole.rs` — API types (unchanged by the rewrite)

## Decisions made (all settled in specs — see "Decisions" sections there)
- poise 0.6 over raw serenity; shared `Data { db, cron, local }` via `ctx.data()`
- Postgres, fresh start (no SQLite data migration); `sqlx::migrate!` + `migrations/`
- BIGINT for snowflakes, BOOLEAN for the 0/1 flags, `$1` placeholders
- `Anchor::Center` default; TopLeft kept as A/B escape hatch
- `MANAGE_WEBHOOKS` gates the scheduling commands
- Fix order: C-1/C-3/C-4/C-12 first, then C-6/C-7, then C-10/B-1/B-2, then S-4

## Next steps
Start with Cargo.toml (poise + sqlx postgres) and the data layer (`db.rs` → Postgres,
`migrations/0001_init.sql`), then the poise framework scaffold in `main.rs`, then port
commands one at a time.
