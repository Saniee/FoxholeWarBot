# Context: log files, and getting the console back

Spec: `specs/architecture.md` → Logging. Tasks: `tasks.md` (three passes, all landed).

Branch `claude/specs-workflow-review-p0c7p3`, PR #2. Everything below is committed and pushed.

## Current state

**Shipped and building.** `cargo check` and `cargo clippy` are clean (the two remaining clippy
warnings — `schedule_report`'s argument count, `Revoked`'s variant sizes — predate this work and
are untouched). `Cargo.lock` is committed: `fern` 0.7.1 and `env_filter` 0.1.4 in, `env_logger`
out. The user has run it in Docker; the console is quiet and both files are written.

Three sinks from one `log` facade (`utils::logging`): stderr and a plain daily file, both `warn` +
this crate at `info`; a `-verbose` daily file at `debug`. `LOG_DIR` (`/app/logs`, `fwb_logs`
volume), `LOG_RETENTION_DAYS` (14, pruned at startup and 03:50 daily), `RUST_LOG` /
`LOG_FILE_FILTER` / `LOG_VERBOSE_FILTER` override the three filters.

## What two real log samples changed

Both guesses made before reading a log were wrong; the fixes below are measured, not reasoned.

1. **65 MB / 1,837 lines / 2h05m.** Not volume — 36 KB a line. `serenity::gateway::shard` and
   `::ws` were 1,695 of the lines: `tracing` **span-creation** records whose fields are the
   payload, so one `GuildCreate` is an entire guild. Muted at `warn`.
2. **Still 55.7 MB / 711 lines.** `serenity::http` logs the request **body as a decimal list, one
   element per byte** — a tick posting a map PNG is one multi-megabyte line. Muted at `warn`.
3. **`tracing::span=off` only drops span enter/exit.** Span creation carries the instrumented
   module's own target, which is why it never caught either of the above. The spec's inherited
   claim is corrected.
4. **`h2`/`rustls`/`hyper` were never the problem** (16/8/7 lines) and stay at `debug`.
5. **This crate's own logging was never the problem** either — a handful of lines per sample. The
   one exception was a *fact* repeated as an event: see below.

## Key files
- `src/utils/logging.rs` — the only place that knows any of this. `init() -> Option<LogFiles>`,
  `prune`/`prune_and_report`, the three filter defaults, the crate target from `module_path!()`.
- `src/utils/cron.rs` — `start_log_prune_job` (03:50 UTC).
- `src/utils/request_processing.rs` — `warn_missing_icon`, once per icon type per process.
- `compose.yaml` + `compose.override.example.yaml` — `fwb_logs` volume; the override swaps it for
  a `./logs` bind mount (copy to `compose.override.yaml`, loaded automatically).

## Decisions worth not re-litigating
- **Relocate noise, don't filter it away.** Dependency traffic is worthless on a console and
  valuable in a postmortem — hence a verbose file rather than a stricter global level.
- **The plain file is the console's twin, not a middle verbosity.**
- **Muting a target is blunt on purpose.** For both serenity families the noise is `INFO` and the
  little worth keeping is `DEBUG`, so no level separates them; what matters (reconnects, 429s,
  failures) is `WARN` and survives.
- **Root dispatch level = max of all three filters**, or `log::max_level` silently caps the
  verbose sink.
- **Empty env var reads as unset** everywhere — compose passes `${VAR:-}`.
- **Log setup is never fatal**; the directory is canonicalized before it's announced and probed
  for writability, because `fern::DateBased` opens lazily and can't report a failure.

## The icon gap the logs exposed — closed

The 113 `no icon for …` warnings were never missing *structures*. Foxhole ships **one neutral
icon per structure** and tints it in game, so `MapIconStorageFacilityColonial.TGA` does not exist
upstream at all, while the renderer asks for `33Colonials.png` by name. 25 icon types had a
neutral file and no faction pair.

`scripts/update_assets.py --derive-icons` now generates them: `decode_png`, `tint_linear_burn`,
`derive_team_icons`. Linear burn (`out = neutral + faction − 255`, clamped, alpha untouched) is
not a taste call — the faction icons drawn by hand years ago are *exactly* it, matching pixel for
pixel on 38 of the 66 pairs on disk, and the colours (Colonial `101,135,94`, Warden
`72,125,169`) were recovered from that fit as what pure white maps to. Black clamps to itself, so
the outline survives. The other 28 pairs are upstream's own faction art (5-9, 19, 28-30) or drawn
from different sources; existing files are never overwritten. 50 icons written, run is idempotent.

Every neutral type gets a pair, including resource fields that are never captured — 2 KB of dead
weight against a curated list that goes stale into a DebugIcon on a live map.

## Next steps

- **Logging is done.** The user confirms the verbose file is now a few KB, down from 65 MB. The
  two serenity mutes were the whole problem; nothing of ours needed demoting.
- **Unverified in production:** a `/full-map` should now emit no `no icon for …` at all. Check
  the next verbose log, and that a tick and a `/full-map` still leave a useful trail in it.
- **A separate finding, not acted on.** `ICON_SOURCES` names look stale: upstream now ships
  `MapIconMedical`/`Vehicle`/`Supplies`/`Manufacturing` where the table says
  `Hospital`/`VehicleFactory`/`SupplyStation`/`ManufacturingPlant`, so 11, 12, 14 and 16 are
  listed in `HAND_SOURCED` when upstream does have art for them. Left alone deliberately: the
  upstream art is **not** byte-identical to what is on disk, so adopting it is a visual change
  the user should look at first.
