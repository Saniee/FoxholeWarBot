# Tasks: Core rewrite (poise + Postgres)

## 1. Dependencies & data layer
- [x] `Cargo.toml`: add poise 0.6, swap sqlx `sqlite` → `postgres`
- [x] `migrations/0001_init.sql`: guilds + cronjobs (BIGINT, BOOLEAN, UNIQUE guild_id,
      UNIQUE(guild,job_name), ON DELETE CASCADE)
- [x] `db.rs` → `PgPool`, `$N` placeholders, `bool` fields, upsert `create_guild`
      (honor `show` — fixes B-1), drop `migrate()`/legacy foxholewarbot path
- [x] Deployment: `Dockerfile`, `.dockerignore`, `compose.yaml` (db service + named volume
      + healthcheck)

## 2. Framework scaffold
- [ ] `main.rs`: poise `Framework`, `Data`, `on_error`, run-once `setup` (fixes C-6)
- [ ] Guild-only command contexts (fixes C-4); registration global vs `--local`
- [ ] Rework `--clear-commands` under poise

## 3. Command ports (1:1 behavior, per-command specs)
- [ ] `war-state`
- [ ] `war-report`
- [ ] `get-map`
- [ ] `set-guild-settings` (honor show-messages on create — B-1)
- [ ] `schedule-help`
- [ ] `schedule-report` (MANAGE_WEBHOOKS gate — S-4)
- [ ] `remove-report` (MANAGE_WEBHOOKS gate; case-insensitive filter — L-3)
- [ ] Shared autocomplete helper + explicit display-name mapping (not "strip Hex")

## 4. Crash/QA fixes (cross-cutting)
- [ ] C-1 unique render output (no shared `render.png`)
- [ ] C-2 independent dynamic/static 304 handling, never unwrap absent cache
- [ ] C-3 remove `.unwrap()` on network/JSON; surface via `on_error`
- [ ] C-5 handle `from_timestamp_millis` None
- [ ] C-12 always reply after defer (no stuck spinner)
- [ ] L-7 cache-file I/O errors handled (autocomplete before first map cache)
- [ ] L-8 `spawn_blocking` for image work

## 5. Scheduling overhaul
- [ ] C-7 resolve owning guild per job on restart (JOIN, not `jobs[0]`)
- [ ] C-8 skip+log bad webhook instead of panicking the restart loop
- [ ] C-9 treat already-deleted webhook as success on remove
- [ ] B-3 schedule first, then persist (no orphan rows)

## 6. Rendering de-magic
- [ ] `RenderConfig` (region-relative ratios), `Anchor` + `place()` helper
- [ ] Icon size from region width; `MapMarkerType` sizes Major/Minor labels

## 7. Wrap-up
- [ ] Update `CLAUDE.md` if layout/commands changed
- [ ] Final self-review pass over the diff
- [ ] Note for user: what to compile/verify at home, known gaps

## Deferred (NOT this pass)
Full-map renderer · premium/form gating · docs & legal overhaul
