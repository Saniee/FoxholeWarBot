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
     - Start the daily map-list refresh job and restart persisted report jobs **once**.
     - Return the shared `Data` (see below).
6. `serenity::ClientBuilder::new(TOKEN, GatewayIntents::GUILDS).framework(framework)` and start.

`--clear-commands` short-circuits all of the above: it builds an `Http` directly (no gateway
client) and sets both the global and dev-guild command lists to empty, then exits.

> **Why this structurally fixes QA C-6:** the `setup` closure runs a single time per process, so
> the daily map job and report-job restoration cannot re-run on a gateway reconnect. The old
> `ready`-based `cron_jobs_restarted` local guard (a dead write) is gone, and poise offers no
> place to reintroduce it.

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
   (e.g. `12Colonials.png`, `40None.png`). Missing icon → fall back to `DebugIcon.png`.
   Each icon is resized to `icon_size_ratio × region width` and overlaid at the position given
   by `place()`, which honors the config's `Anchor`.
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
