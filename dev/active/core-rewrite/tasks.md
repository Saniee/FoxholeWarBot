# Tasks: Core rewrite (poise + Postgres)

## 1. Dependencies & data layer
- [x] `Cargo.toml`: add poise 0.6, swap sqlx `sqlite` → `postgres` (+ `env_logger`)
- [x] `migrations/0001_init.sql`: guilds + cronjobs (BIGINT, BOOLEAN, UNIQUE guild_id,
      UNIQUE(guild,job_name), ON DELETE CASCADE)
- [x] `db.rs` → `PgPool`, `$N` placeholders, `bool` fields, upsert `create_guild`
      (honor `show` — fixes B-1), drop `migrate()`/legacy foxholewarbot path
- [x] Deployment: `Dockerfile`, `.dockerignore`, `compose.yaml` (db service + named volume
      + healthcheck)

## 2. Framework scaffold
- [x] `main.rs`: poise `Framework`, `Data`, `on_error`, run-once `setup` (fixes C-6)
- [x] Guild-only command contexts (fixes C-4); registration global vs `--local`
- [x] Rework `--clear-commands` under poise (REST-only, fixes B-4)

## 3. Command ports (1:1 behavior, per-command specs)
- [x] `war-state`
- [x] `war-report`
- [x] `get-map`
- [x] `set-guild-settings` (honor show-messages on create — B-1; shards are now choices)
- [x] `schedule-help` (ungated, phrases inline — L-5, L-1)
- [x] `schedule-report` (MANAGE_WEBHOOKS gate — S-4)
- [x] `remove-report` (MANAGE_WEBHOOKS gate; case-insensitive filter — L-3)
- [x] Shared autocomplete helper + explicit display-name mapping (`utils/regions.rs`)

## 4. Crash/QA fixes (cross-cutting)
- [x] C-1 unique render output (in-memory PNG, no shared `render.png`)
- [x] C-2 independent dynamic/static 304 handling, never unwrap absent cache
- [x] C-3 remove `.unwrap()` on network/JSON; surface via `on_error`
- [x] C-5 handle `from_timestamp_millis` None (`utils::format_timestamp`)
- [x] C-12 always reply after defer (no stuck spinner)
- [x] L-7 cache-file I/O errors handled (autocomplete before first map cache)
- [x] L-8 `spawn_blocking` for image work

## 5. Scheduling overhaul
- [x] C-7 resolve owning guild per job on restart (JOIN, not `jobs[0]`)
- [x] C-8 skip+log bad webhook instead of panicking the restart loop
- [x] C-9 treat already-deleted webhook as success on remove
- [x] B-3 schedule first, then persist (no orphan rows)

## 6. Rendering de-magic
- [x] `RenderConfig` (region-relative ratios), `Anchor` + `place()` helper
- [x] Icon size from region width; `MapMarkerType` sizes Major/Minor labels

## 7. Wrap-up
- [x] Update `CLAUDE.md` if layout/commands changed
- [x] Final self-review pass over the diff
- [x] Note for user: what to compile/verify at home, known gaps

## Deferred (NOT this pass)
Full-map renderer · premium/form gating · docs & legal overhaul
