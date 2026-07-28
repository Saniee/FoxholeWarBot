# Architecture (cross-cutting)

Shared machinery every command depends on.

## Process lifecycle (`src/main.rs`)

1. Parse CLI args (`--local`, `--clear-commands`).
2. Read `TOKEN` and `DATABASE_URL` from env.
3. `Database::connect(url)` — opens the Postgres pool and applies the embedded migrations
   (`sqlx::migrate!`, so building needs no live database).
4. `CronHandler::new()` — creates and starts a `tokio_cron_scheduler::JobScheduler`.
5. Build the poise `Framework`:
   - `FrameworkOptions { commands: vec![...7 commands...], on_error, .. }`.
   - `setup` closure (runs **once**, after the gateway is ready):
     - Set presence to "Watching Foxhole Wars", status idle.
     - `save_maps_cache()` — refresh the per-shard map list cache.
     - Register commands: `register_globally` (or `register_in_guild(GUILD_ID)` when `--local`).
       A global registration then **clears the dev guild's command list**, because guild-scoped
       registrations from a `--local` run outlive the process that made them: the dev guild
       otherwise shows two of every command, and a leftover whose name or options no longer
       match a registered command produces an interaction poise has no handler for — never
       acknowledged, so it hangs until Discord reports "application did not respond".
       Best-effort: a failure here is logged, never fatal, since tidying the dev guild is not a
       reason to refuse to start in production. Not done in reverse — a `--local` run leaves
       global commands alone, or dev would deregister production.
     - Start the daily map-list refresh job and restart persisted report jobs **once**.
     - Return the shared `Data` (see below).
6. `serenity::ClientBuilder::new(TOKEN, GatewayIntents::GUILDS).framework(framework)` and start.

`--clear-commands` short-circuits all of the above: it builds an `Http` directly (no gateway
client) and sets both the global and dev-guild command lists to empty, then exits.

> **Why this structurally fixes QA C-6:** the `setup` closure runs a single time per process, so
> the daily map job and report-job restoration cannot re-run on a gateway reconnect. The old
> `ready`-based `cron_jobs_restarted` local guard (a dead write) is gone, and poise offers no
> place to reintroduce it.

### Logging (`src/utils/logging.rs`)

`fern`, initialised first thing in `main`, fanning one `log` facade out to **three sinks**: stderr
and two date-rotated files. Filters are `RUST_LOG` syntax throughout, parsed by `env_filter` — the
crate `env_logger` was built on, so the strings mean exactly what they used to.

| Sink | Default filter | Env override |
|---|---|---|
| stderr | `warn`, this crate at `info` | `RUST_LOG` |
| `logs/foxholewarbot.<date>.log` | the same | `LOG_FILE_FILTER` |
| `logs/foxholewarbot-verbose.<date>.log` | `debug`, this crate at `trace` | `LOG_VERBOSE_FILTER` |

**The console carries this crate and warnings, and nothing else.** Serenity and poise log every
HTTP request, every gateway event and every dispatch; sqlx logs every statement it runs, at
`info`. All of it is real information and none of it is what someone tailing `docker compose logs`
is looking for — it buries the bot's own dozen-a-day lines. The split keeps it, in the verbose
file, rather than filtering it away: it is the only record of what the bot asked Discord for and
what came back, and it is wanted precisely when something has already gone wrong.

The plain file is the console's **twin**, not a middle verbosity. Its job is "what did it say last
Tuesday" for someone who wasn't watching the terminal, and a stream that reads differently from
the one they know is a worse answer than the same one, kept.

#### The gateway is muted, and why level filtering couldn't do it
Serenity is instrumented with `tracing`, whose `log` bridge emits **two** kinds of record, and the
distinction is load-bearing:

- **Span enter/exit**, under the `tracing::span` target. Pure noise — `recv;`, `do_heartbeat;`,
  several a second, forever, carrying nothing beyond a name. `tracing::span=off` is in every
  filter, including the verbose one, and drops these.
- **Span creation, under the instrumented module's own target**, with the span's fields inline.
  `tracing::span=off` never touched these, which is what buried the console before the split: at a
  global `info`, every heartbeat and every gateway event was on it.

The second kind is also enormous, because the field *is* the payload:
`handle_event; event=Ok(Dispatch(N, GuildCreate(GuildCreateEvent { guild: Guild { .. } })))` is an
entire guild — every channel, role and emoji — `Debug`-formatted onto one line. Measured on a live
run: **1,837 lines over 2h05m, 65 MB, of which 1,695 lines came from `serenity::gateway::shard`
and `serenity::gateway::ws`.** An average line of 36 KB, and ~750 MB a day — 10 GB across the
retention window.

So those two targets are muted at `warn` in the verbose filter. **Muting is blunt on purpose: no
level separates the noise from the signal here**, because the noise is `INFO` while the little
worth keeping under the same targets (`Received a Hello`, `Sending presence update`) is `DEBUG`.
What actually matters about the gateway — reconnects, resumes, failures — is `WARN` from these
targets and from `shard_runner`, `shard_manager` and `shard_queuer`, none of which are touched.

**`serenity::http` is muted for the same reason and by the same measurement.** With the gateway
silenced the next sample was still **55.7 MB from 711 lines** — 80 KB a line — because
`build; self=Request { body: Some([N, N, N, ..` is the request body as a decimal list, one element
per byte. A map PNG posted to a webhook is megabytes, so one scheduled tick writes one log line of
megabytes. Method, route and status are not worth that, and a 429 or a failed request is `WARN`,
which survives.

Everything else stays at `debug` on the evidence rather than on suspicion: `h2`, `rustls`,
`hyper_util` and `tungstenite` were 16, 8, 7 and 2 lines in the first sample. They were the prime
suspects before the log was read, and they are not the problem — `hyper_util`'s connection pooling
is the largest of them at 227 small lines, which is what a full-map render's 53 fetches looks like
and is worth having.

Nothing of **this crate's** logging was demoted for volume. Across both samples it contributed a
handful of lines, and the one place it repeated was a fact rather than an event: see the
per-process de-duplication in Map rendering below.

Filters name this crate by `module_path!()`'s root rather than by a string literal, so a rename
can't leave them pointing at nothing.

#### Files, rotation, retention
- `LOG_DIR` (default `./logs`, `/app/logs` in the container) is created at startup. **Empty means
  console-only** — one variable to say it, rather than a second flag that can disagree with the
  first.
- Rotation is daily, by `fern::DateBased`: the date is in the filename, so nothing renames or
  reopens anything.
- `LOG_RETENTION_DAYS` (default 14, `0` keeps everything) is enforced by `logging::prune`, run
  **at startup and by a daily job at 03:50 UTC**. Both, because a bot restarted often would never
  reach the nightly job and one that never restarts would only ever prune from it. It deletes only
  files matching its own two prefixes — the directory may be a bind mount with other things in it.
- Nothing here is fatal. A log directory that can't be created is a warning on a console that is
  already working, the same rule the on-disk cache follows.

The directory is a **mounted volume** (`fwb_logs`) for the same reason the cache is: a container
rebuilt on every deploy otherwise takes the record of what happened with it. A named volume is
not a host directory, so the files are *not* in `./logs` when the bot runs in Docker —
`compose.override.example.yaml` swaps in a bind mount for anyone who wants them there, and
`compose.override.yaml` is loaded automatically without flags.

Two things exist because that distinction is easy to trip over, both of which turn "it says it's
logging and the folder is empty" into an answer at startup:
- The directory is **canonicalized before it is announced**, so the startup line names an absolute
  path rather than a `./logs` whose meaning depends on the working directory it was read in.
- The directory is **probed for writability** during `init`. `fern::DateBased` opens its file
  lazily on the first record and has nowhere to report a failure to, so an unwritable mount would
  otherwise be perfectly silent: a working console, and a directory that never fills.

### Gateway intents
Only `GUILDS`. The bot does **not** request message content or member intents; it operates
purely through slash-command interactions and webhooks.

## Command framework (poise)

### Shared state — `Data`
A single struct handed to every command via `ctx.data()`, replacing the fields that used to be
threaded through the `Handler` struct and its `.clone()`s:

```rust
pub struct Data {
    pub db: Database,
    pub cron: CronHandler,   // holds the JobScheduler (already Clone/Arc-backed)
    pub local: bool,
}
type Error = Box<dyn std::error::Error + Send + Sync>;
type Context<'a> = poise::Context<'a, Data, Error>;
```

### Command definition
Each command is a plain async fn annotated with the macro; registration metadata lives on the
attribute instead of a separate `register()` builder. One module per command under
`src/commands/`, each exporting its command fn (collected into the `commands: vec![...]`).

```rust
/// Responds with an image of that Hex/Map chunk.
#[poise::command(slash_command, guild_only)]
pub async fn get_map(
    ctx: Context<'_>,
    #[description = "Name of the Hex you want displayed."]
    #[autocomplete = "autocomplete_map"]
    map_name: String,
    #[description = "Render text labels for things on the map."]
    draw_text: Option<bool>,
) -> Result<(), Error> { /* ... */ }
```

Key mappings from the pre-rewrite raw-serenity code, kept as a reading aid for old commits:

| Concern | Pre-rewrite (raw serenity) | Now (poise) |
|---|---|---|
| Dispatch | `match command.data.name` in `interaction_create` | framework routes to the fn |
| Registration | per-command `register() -> CreateCommand` | attribute on the fn + `register_globally` |
| Options | positional `interaction.data.options[i]` indexing | typed fn parameters (`Option<T>` = optional) |
| Autocomplete | separate `autocomplete()` + manual dispatch | `#[autocomplete = "fn"]` on the parameter |
| Guild-only | `interaction.guild_id.unwrap()` | `guild_only` on the macro (**fixes QA C-4**) |
| Admin gate | `default_member_permissions(ADMINISTRATOR)` | `default_member_permissions = "ADMINISTRATOR"` |
| Errors | `.unwrap()` in the `main.rs` match | central `on_error` hook (**helps QA C-3/C-12**) |
| Shared state | `Handler { db, cron, local }.clone()` | `ctx.data()` |

### Autocomplete
An autocomplete handler is a fn referenced by name; it takes `Context` + the partial input and
returns `Vec<AutocompleteChoice>`. The map-name handler lives once in
`commands/common.rs::autocomplete_map` and is shared by `/get-map`, `/war-report` and
`/schedule-report`: it reads `ctx.data().db` for the guild's shard, loads the cached region
list, matches case-insensitively against both the display name and the API id, and caps at 25.
Labels come from `utils::regions::display_name`; `OriginHex` is not excluded.

### Error handling
`FrameworkOptions.on_error` is the single place transport/JSON/render failures surface. Commands
return `Result<(), Error>`; a returned `Err` (or one bubbled with `?`) is reported to the user by
the hook instead of panicking a spawned task. This is the mechanism the rewrite uses to retire
the pervasive `.unwrap()` on network/JSON calls (QA C-3) and to guarantee a deferred interaction
always gets a final reply (QA C-12).

### Output visibility (ephemeral vs public)
Unchanged contract, expressed in poise: read `guilds.show_command_output`, then call
`ctx.defer_ephemeral()` (`0`) or `ctx.defer()` (`1`) at the top of the command, and send replies
with `CreateReply`. `/set-guild-settings` stays always-ephemeral.

### Dependency
`poise = "0.6"` re-exports the compatible `serenity` as `poise::serenity_prelude`, which is what
the command modules import. The direct `serenity` dependency is retained so version drift
between the two is explicit rather than implicit.

## Shards (`Shard` enum, `src/utils/db.rs`)

Three live shards map to Foxhole War API base URLs:

| Name    | Base URL                                              |
|---------|-------------------------------------------------------|
| Able    | `https://war-service-live.foxholeservices.com/api`    |
| Baker   | `https://war-service-live-2.foxholeservices.com/api`  |
| Charlie | `https://war-service-live-3.foxholeservices.com/api`  |

- `Shard::from_str` is case-insensitive and **defaults to `Able`** for any unrecognized string.
- A guild stores both `shard` (the resolved base URL) and `shard_name` (the display/cache key).

## Foxhole War API endpoints used

Base = the guild's stored shard URL.
- `GET /worldconquest/war` — global war state (`War`).
- `GET /worldconquest/maps` — list of active hex names (`Vec<String>`).
- `GET /worldconquest/warReport/{map}` — casualties/enlistments (`WarReport`).
- `GET /worldconquest/maps/{map}/dynamic/public` — dynamic map items/icons (`DynamicMapData`).
- `GET /worldconquest/maps/{map}/static` — static map text labels (`StaticMapData`).

### ETag revalidation
`warReport`, `dynamic`, and `static` responses carry a `version` integer. The bot sends
`If-None-Match: "<version>"` (or `"0"` when it has no cache). A `304 Not Modified` means
"serve the cached copy"; `200` means "store the fresh copy". Any other status serves the cached
copy when one exists, and otherwise surfaces as a user-facing error — never a silent return.

The dynamic and static halves are revalidated **independently**, each against its own ETag and
its own cache. They routinely disagree, and requiring them to agree is what made the old code
unwrap an absent cache (QA C-2).

## Response types (`src/utils/api_definitions/foxhole.rs`)

- `War { war_id, war_number, winner, conquest_start_time, conquest_end_time?,
  resistance_start_time?, required_victory_towns }`
- `WarReport { total_enlistments, colonial_casualties, warden_casualties, day_of_war, version }`
- `DynamicMapData { region_id, scorched_victory_towns, map_items[], last_updated, version }`
- `MapItem { team_id: TeamId, icon_type, x, y, flags, view_direction }`
- `TeamId` = `COLONIALS | NONE | WARDENS`
- `StaticMapData { region_id, scorched_victory_towns, map_text_items[], last_updated, version }`
- `MapTextItem { text, x, y, map_marker_type: Major|Minor }`
- All use `camelCase` serde renaming to match the API.

## Database schema (Postgres)

Managed by `sqlx::migrate!` from `migrations/`, embedded at compile time and applied at startup.
Full detail, rationale and the compose setup: `specs/postgres.md`.

```sql
CREATE TABLE guilds (
    id                  BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    guild_id            BIGINT  NOT NULL UNIQUE,
    shard               TEXT    NOT NULL,   -- resolved API base URL
    shard_name          TEXT    NOT NULL,   -- "Able" | "Baker" | "Charlie"
    show_command_output BOOLEAN NOT NULL DEFAULT FALSE
);

CREATE TABLE cronjobs (
    id          BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    guild       BIGINT  NOT NULL REFERENCES guilds(id) ON DELETE CASCADE,
    job_name    TEXT    NOT NULL,
    schedule    TEXT    NOT NULL,   -- the user's english phrase, converted on use
    webhook_url TEXT    NOT NULL,
    map_name    TEXT    NOT NULL,
    draw_text   BOOLEAN NOT NULL DEFAULT FALSE,
    job_id      TEXT,               -- scheduler UUID, reissued each process
    UNIQUE (guild, job_name)
);
```

Constraints doing real work:
- `guilds.guild_id … UNIQUE` — a guild can never have two rows, which is what used to make its
  lookups fail permanently (C-10). It's also what the `create_guild` upsert conflicts on.
- `UNIQUE (guild, job_name)` — schedule names are unique per guild, not globally (B-2).
- `ON DELETE CASCADE` — removing a guild cleans up its schedules, so leaving a server needs one
  statement.
- `BIGINT` throughout for snowflakes: Postgres `INTEGER` is 32-bit and would truncate a Discord
  id. SQLite's is 64-bit, which is why this was invisible before.

`cronjobs.schedule` stores the **original phrase**, not the derived cron expression, so it stays
readable; `Job::schedule_to_cron` converts it at schedule time.

## On-disk cache (`src/utils/cache.rs`)

Created lazily under `./cache/`:
- `cache/map_choices/maps-<Shard>.json` — map list per shard (autocomplete source).
- `cache/dynamic/Dynamic_<map>-<Shard>.json` — last `DynamicMapData`.
- `cache/static/Static_<map>-<Shard>.json` — last `StaticMapData`.
- `cache/war_reports/Report_<map>-<Shard>.json` — last `WarReport`.

`save_maps_cache()` refreshes all three shards' map lists (skipping shards that return 503).

## Map rendering (`src/utils/request_processing.rs::place_image_info`)

Every placement and sizing constant lives in `RenderConfig`, expressed as a ratio of the region
footprint. Detail and the anchoring decision: `specs/rendering-placement.md`.

Inputs: `DynamicMapData`, `StaticMapData`, `draw_text: bool`, background path, `&RenderConfig`.
1. Open `assets/Maps/Map<name>.TGA` as RGBA; a failure is a typed `RenderError`.
2. For each dynamic `map_item`: load `assets/MapIcons/<icon_type><TeamId>.png`
   (e.g. `12Colonials.png`, `40None.png`). Missing icon → fall back to `DebugIcon.png`, warned
   **once per icon type per process**. The fact is about the asset set, not about this render: a
   type Foxhole ships before we do is missing for every structure of that type, in every region,
   on every render — one measured full-map render produced 113 identical warnings from two types,
   which buries a real signal (art needs updating) under its own repetition. Per process rather
   than per render, since the answer only changes when someone deploys new art.
   Each icon is resized to `icon_size_ratio × region width` and overlaid at the position given
   by `place()`, which honors the config's `Anchor`.

   The faction art mostly does not exist upstream to be copied — Foxhole ships one *neutral*
   icon per structure and tints it in game — so `scripts/update_assets.py --derive-icons`
   generates `<icon_type>{Colonials,Wardens}.png` from `<icon_type>None.png` by linear burn
   (`out = neutral + faction − 255`, clamped, alpha untouched). That operation is not a taste
   call: the faction icons drawn by hand years ago are exactly it, matching pixel for pixel on
   38 of the 66 pairs on disk, and the two colours (Colonial `101,135,94`, Warden `72,125,169`)
   were recovered from that fit as what pure white maps to. The black outline survives because
   black clamps to itself; a blend would wash it grey and cost the icon its edge on a dark hex.
   Existing art always wins — nothing hand-made is overwritten by a generated approximation.
3. If `draw_text`: render each `map_text_item.text` in Inter-Bold at
   `major_text_ratio`/`minor_text_ratio` × region height, per its `MapMarkerType`.
4. Return the composited `ImageBuffer`.

## Map pipeline (`src/utils/map_render.rs`)

`render_region` is the single fetch-revalidate-render path, shared by `/get-map` and the
scheduled report tick. It:
1. loads the dynamic and static caches independently,
2. issues both conditional requests,
3. resolves each half against its own ETag and cache,
4. writes the cache back if either half was fresh,
5. composites and PNG-encodes inside `spawn_blocking` (the work is CPU-bound; L-8).

It returns the encoded PNG **in memory**. Nothing is written to disk for a render, which is why
concurrent renders can no longer collide (C-1).

## Scheduler (`src/utils/cron.rs`)

See `specs/scheduling.md` for the full subsystem.

- `start_map_update_job` — cron `0 0 0 * * *` (daily at 00:00) refreshes the region lists.
- `start_log_prune_job` — cron `0 50 3 * * *` deletes log files past `LOG_RETENTION_DAYS`
  (see Logging). Twenty minutes after the request purge, so the two aren't doing filesystem work
  in the same minute.
- `restore_jobs` — runs once from `setup`; joins every `cronjobs` row to its owning guild so each
  restores against its own shard (C-7), and skips-and-logs any row it can't restore (C-8).
- `schedule` — registers a job with the scheduler and returns its UUID. It does **not** touch the
  database: callers persist only after it succeeds (B-3).
- `unschedule` — removes by UUID; an unrecognized id is not an error.
- Ticks run in UTC. Embeds label the next-tick time as UTC rather than formatting in the host's
  local time (L-2).
- English-phrase schedules are parsed by `Job::schedule_to_cron` ("every 30 seconds",
  "at 5:00 pm", "On Monday at 8:00", …).

## Output visibility

Every data command reads `guilds.show_command_output` (a native `BOOLEAN`) through
`commands/common.rs::defer_for`:
- `true` → `ctx.defer()` (public response, visible to the channel).
- `false` → `ctx.defer_ephemeral()` (only the caller sees it).

`/set-guild-settings` and `/schedule-help` are always ephemeral.
