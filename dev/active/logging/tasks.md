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
- [x] Compiles — `cargo check` and `cargo clippy` clean (the two clippy warnings that remain,
      `schedule_report`'s argument count and `Revoked`'s variant sizes, both predate this and are
      untouched)
- [x] Directory canonicalized before it's announced, and probed for writability at `init` — the
      first live run had an empty `./logs` on the host because the container was writing to the
      `fwb_logs` volume, which is the expected behaviour and now says so
- [x] `compose.override.example.yaml` — copy to `compose.override.yaml` for logs in `./logs`;
      README, `compose.yaml` and the spec all point at it
- [ ] `logs/` appears; both files written; console quiet; verbose file has the serenity/sqlx
      traffic
- [ ] `RUST_LOG=info` still puts everything back on the console
- [ ] `LOG_DIR=""` degrades to console-only, and an unwritable `LOG_DIR` warns and continues
- [ ] Retention: `touch -d '30 days ago' logs/foxholewarbot.2000-01-01.log`, restart, it's gone
- [x] `Cargo.lock` regenerated and committed — `fern` 0.7.1, `env_filter` 0.1.4 in, `env_logger`
      and its colour/humantime tree out, nothing else moved. Done with `cargo metadata`, which
      resolves and writes the lock **without compiling**: it needs the network, not a build, and
      it keeps every existing pin rather than re-resolving from scratch the way
      `cargo generate-lockfile` would

## Pass 2 — done, and it was not what anyone expected

A verbose log from a live run settled it: **1,837 lines over 2h05m, 65.0 MB.** 36 KB a line.

- [x] **The problem was line size, not line count, and it was serenity's gateway.**
      `serenity::gateway::shard` (1,507) and `serenity::gateway::ws` (188) were 1,695 of the 1,837
      lines and effectively all of the bytes: `tracing` span-creation records whose fields are the
      event payload, so one `GuildCreate` line is an entire guild. Muted at `warn` in the verbose
      filter — the only two targets that are
- [x] **`tracing::span=off` was never going to catch them.** It drops span *enter/exit*; span
      *creation* records carry the instrumented module's own target. The comment inherited from
      the rewrite said otherwise and is corrected in `specs/architecture.md`
- [x] **The suspects in this list were wrong, and so was the h2/rustls/hyper guess** — 16, 8 and 7
      lines respectively. Left at `debug`, on the evidence
- [x] **This crate's own logging was never the problem**: `FoxholeWarBot::utils::cron` contributed
      **5 lines** in two hours and nothing else of ours appeared at all. No demotions made — the
      three suspects below stay exactly as they are until something shows they matter:
      `map_render::render_full_map`'s "shard doesn't list X", `map_render::live_regions`' "no grid
      position", `cron::go_dormant`'s "skipping this tick". All three are per-full-map-render or
      per-dormant-tick, and neither happened in this sample
- [x] Volume after the mute: ~140 lines per two hours from everything else, against ~750 MB/day
      and 10 GB across the retention window before it. No shorter verbose window and no gzip on
      prune needed — both were on the table and neither is now

### Still worth a look on the next live run
- [ ] Confirm the verbose file is a sane size after the mute, and that a scheduled tick and a
      `/full-map` still leave a useful trail in it
