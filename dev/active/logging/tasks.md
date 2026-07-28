# Tasks: log files, and getting the console back

Spec: `specs/architecture.md` → Logging. Context: `dev/active/logging/context.md`.

Shipped in two passes on purpose (user's call): **files first**, then the per-call cleanup once
there is a real log to point at.

## Pass 1 — file logging (written, not yet built)
- [x] `src/utils/logging.rs` — three sinks from one `log` facade: quiet stderr, a plain daily
      file, and a `-verbose` daily file holding what the dependencies say
- [x] `env_logger` → `fern` (`date-based`) + `env_filter`; `RUST_LOG` keeps its meaning, and
      `LOG_FILE_FILTER` / `LOG_VERBOSE_FILTER` join it
- [x] Console default `warn` + this crate at `info` — poise, serenity, reqwest and sqlx move off
      it and into the verbose file rather than being filtered away
- [x] Crate target taken from `module_path!()`, not a literal
- [x] Daily rotation by filename date; `LOG_DIR` created at startup, empty = console only
- [x] `LOG_RETENTION_DAYS` (default 14, `0` = forever), pruned at startup **and** by a daily job
      (`cron::start_log_prune_job`, 03:50 UTC); only files matching this module's own prefixes
- [x] `/app/logs` created in the `Dockerfile`, mounted as the `fwb_logs` volume in
      `compose.yaml`, with the log env vars passed through and a bind-mount recipe documented
- [x] `specs/architecture.md` → Logging rewritten; `CLAUDE.md` layout, runtime config and a
      convention for where a new `debug!` aims; `README.md` self-hosting section; `.env.example`
- [x] `docs/privacy.md` + `docs/tos.md` — operational logs, what they hold, rolling deletion
- [x] Stale `specs/active/...` paths in `compose.yaml` and `.env.example` repointed (missed in
      the 2.0 sweep)

### To verify on the first build
- [ ] Compiles — `fern` 0.7 / `env_filter` 0.1 API shapes are the likely first failure
- [ ] `logs/` appears; both files written; console quiet; verbose file has the serenity/sqlx
      traffic
- [ ] `RUST_LOG=info` still puts everything back on the console
- [ ] `LOG_DIR=""` degrades to console-only, and an unwritable `LOG_DIR` warns and continues
- [ ] Retention: `touch -d '30 days ago' logs/foxholewarbot.2000-01-01.log`, restart, it's gone
- [ ] `Cargo.lock` regenerated and committed (`fern`, `env_filter` in; `env_logger` out)

## Pass 2 — the crate's own noise (blocked on a log sample)
Not guessed at. The user is uploading a real log file; each line goes to `debug` (verbose only)
or stays on evidence, not on a hunch. Suspects already noted:
- [ ] `map_render::render_full_map` — "shard doesn't list X", `info`, per unlisted region per
      full-map render
- [ ] `map_render::live_regions` — "no grid position", `warn`, re-warned every render instead of
      once per process
- [ ] `cron::go_dormant` — "skipping this tick", `info`, every tick of a dormant schedule
- [ ] Whatever the sample shows that this list doesn't
