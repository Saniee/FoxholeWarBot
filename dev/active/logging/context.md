# Context: log files, and getting the console back

Spec: `specs/architecture.md` → Logging

## Why

Two complaints, one change:

1. **The console is unreadable.** Poise and serenity log every HTTP request, every gateway event
   and every dispatch; sqlx logs every statement it runs, at `info`. The bot's own dozen lines a
   day are somewhere underneath.
2. **Nothing is written to a file.** Capture was the caller's job (`docker compose logs`, or a
   redirect), so the record of what happened lived in a container that gets rebuilt on every
   deploy.

Filtering the dependencies away would fix the first and make the second worse: their traffic is
the only record of what the bot asked Discord for and what came back, and it is wanted precisely
when something has already gone wrong. So it moves rather than goes.

## Current state

**Written in full, compiled never** — same footing as the schedule-input work: the user builds and
runs locally, so nothing here has seen `cargo build`. Two new dependencies (`fern` with
`date-based`, `env_filter`), one removed (`env_logger`); `Cargo.lock` does **not** yet reflect
that and the first build will update it.

Three sinks from one facade:

| Sink | Default filter | Override |
|---|---|---|
| stderr | `warn`, this crate at `info` | `RUST_LOG` |
| `foxholewarbot.<date>.log` | the same | `LOG_FILE_FILTER` |
| `foxholewarbot-verbose.<date>.log` | `debug`, this crate at `trace` | `LOG_VERBOSE_FILTER` |

The plain file is the console's twin, not a middle verbosity — its job is "what did it say last
Tuesday", and a stream that reads differently from the one someone knows is a worse answer than
the same one, kept. The verbose file is the firehose, named for what it is (user's call — the
distinction is on the filename, not in a subdirectory or a level nobody can see).

## Key files

- `src/utils/logging.rs` — new, and the only place that knows any of this. `init()` returns
  `Option<LogFiles>` (`None` = console only), `prune`/`prune_and_report` enforce retention.
- `src/main.rs` — `logging::init()` first thing, a prune pass right after, `LogFiles` moved into
  the poise `setup` closure for the daily job.
- `src/utils/cron.rs` — `start_log_prune_job`, cron `0 50 3 * * *`.
- `compose.yaml` — `fwb_logs:/app/logs`, plus the five log env vars passed through as
  `${VAR:-}` (which is why empty strings have to read as "unset" everywhere in `logging.rs`).
- `Dockerfile` — `mkdir -p /app/cache /app/logs`.

## Decisions taken while building

- **Filters name this crate via `module_path!()`'s root**, not a string literal. The package is
  `FoxholeWarBot`, so the target is mixed-case and a literal is one rename from silently matching
  nothing.
- **`env_filter` rather than fern's own `level_for` chain**, so the filter strings keep exactly
  the `RUST_LOG` semantics `env_logger` gave them, overrides included.
- **The root dispatch's level is the max of all three filters.** It sets `log::max_level`; a
  record rejected there never reaches a per-sink filter, which would have made the verbose file
  quietly no more verbose than the console.
- **`LOG_DIR=""` means console-only.** One variable rather than a second flag that can disagree
  with the first.
- **Retention prunes at startup *and* nightly.** A bot restarted often would never reach the
  nightly job; one that never restarts would only ever prune from it.
- **Prune deletes only files matching its own two prefixes.** The directory may be a bind mount
  with other things in it, and deleting by age alone in a directory we don't own is how a log
  pruner eats something else.
- **A log directory that can't be created is a warning, not a failure** — the same rule the
  on-disk cache follows.
- **Docs count logs as data.** `docs/privacy.md` gained an operational-logs section and
  `docs/tos.md` a pointer: logs live on the host, hold server names/ids and error detail, and are
  deleted on a rolling window. No migration was involved, but the site's invariant is about what
  the bot keeps, and it now keeps this.

## Risks / gotchas

- `fern::DateBased` does not create its parent directory — `logging::log_dir` does, first.
- Nothing rotates *within* a day. A pathological `LOG_VERBOSE_FILTER=trace` could produce a very
  large single file; retention is by day, not by size.
- The verbose file records the bot's Discord HTTP traffic. It should not contain the token
  (serenity does not log auth headers), but it is not a file to paste publicly — say so if anyone
  is asked for one.
- `compose.yaml` passes every override as `${VAR:-}`, so an unset host variable arrives as an
  empty string, not as absent. Every reader in `logging.rs` treats empty as unset; a new one must
  too.
- User compiles and runs locally; **don't run `cargo build`/`check`/`run`.**

## Next steps

1. **Build it.** Expect the first failures in `logging.rs` around the `fern` 0.7 and `env_filter`
   0.1 API shapes (`DateBased::new`, `Filter::enabled`, `Filter::filter`).
2. Boot and check: `logs/` appears, both files are written, the console is quiet, and the verbose
   file has the serenity/sqlx traffic in it.
3. **Then the second pass, with a real log file in hand.** The filter split moves the
   dependencies' noise; what's left is this crate's own lines that repeat per render or per tick.
   Known suspects, not yet touched, deliberately — they get demoted from evidence, not from a
   guess:
   - `map_render::render_full_map` — "shard doesn't list X, drawing it as background only",
     `info`, once per unlisted region per full-map render.
   - `map_render::live_regions` — "lists X, which has no grid position", `warn`, re-warned on
     every render rather than once.
   - `cron::go_dormant` — "skipping this tick", `info`, every tick of a dormant schedule forever.
4. Retention is testable without waiting 14 days: `touch -d '30 days ago'` a file matching the
   prefix and restart.
