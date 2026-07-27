# Architecture (cross-cutting)

Shared machinery every command depends on. A rewrite should reproduce these contracts.

> **Command framework — rewrite decision:** the rewrite moves from raw serenity slash-command
> plumbing to **[poise](https://crates.io/crates/poise) `0.6`** (built on serenity `0.12`, same
> maintainers). Poise replaces the hand-rolled `interaction_create` dispatch `match`, the
> per-command `register()` builders, the manual autocomplete routing, and the `Handler`-struct
> `.clone()` threading. The data contracts below (shards, API, DB, cache, rendering, scheduler)
> are framework-agnostic and unchanged. See **Command framework (poise)** below.

## Process lifecycle (`src/main.rs`) — rewrite target

1. Parse CLI args (`--local`, `--clear-commands`).
2. `CronHandler::new()` — creates and starts a `tokio_cron_scheduler::JobScheduler`.
3. Read `TOKEN` from env; `Database::connect()` opens `database.db` (created if missing).
4. Create tables `guilds` and `cronjobs` if they don't exist (schema below).
5. `db.migrate()` — one-time migration from a legacy `foxholewarbot` table (see below).
6. Build the poise `Framework`:
   - `FrameworkOptions { commands: vec![...7 commands...], on_error, .. }`.
   - `setup` closure (runs **once**, after the gateway is ready):
     - Set presence to "Watching Foxhole Wars", status idle.
     - `save_maps_cache()` — refresh the per-shard map list cache.
     - Register commands: `register_globally` (or `register_in_guild(GUILD_ID)` when `--local`).
     - Start the daily map-list refresh job and restart persisted report jobs **once**.
     - Return the shared `Data` (see below).
7. `serenity::Client::builder(TOKEN, GatewayIntents::GUILDS).framework(framework)` and start.
8. `--clear-commands`: build the framework with an empty command list and register it (globally
   and to `GUILD_ID`), which clears all registered commands, then exit. (Alternatively call the
   REST delete endpoints as today — either is acceptable.)

> **Why this fixes QA C-6:** the `setup` closure runs a single time for the process, so the
> daily map job and report-job restoration no longer re-run on every gateway reconnect. The old
> `ready`-based `cron_jobs_restarted` local guard (a dead write) is retired entirely.

### Gateway intents
Only `GUILDS`. The bot does **not** request message content or member intents; it operates
purely through slash-command interactions and webhooks.

## Command framework (poise)

Poise is the "better slash-command workflow" the rewrite standardizes on.

### Shared state — `Data`
A single struct handed to every command via `ctx.data()`, replacing the fields threaded through
the old `Handler` and its `.clone()`s:

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

Key mappings from the current raw-serenity code:

| Concern | Current (raw serenity) | Rewrite (poise) |
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
returns choices. It reads `ctx.data().db` for the guild's shard and the cached map list — same
logic as today (substring filter, exclude `OriginHex`, strip `Hex`, cap 25), minus the manual
`CreateAutocompleteResponse` plumbing.

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
Add `poise = "0.6"` to `Cargo.toml`; it re-exports the compatible `serenity`, so the direct
`serenity` dependency can be dropped or kept in sync via poise's re-export
(`poise::serenity_prelude`).

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
"serve the cached copy"; any other 2xx means "store the fresh copy". `500` responses are
treated as "give up silently".

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

## Database schema (SQLite, `database.db`)

> **Rewrite decision:** the store moves from SQLite to a self-hosted **Postgres** (docker-compose,
> persistent volume) — SQLite's single local file has been wiped by accident more than once. The
> rewrite starts with a **fresh** schema (no data carried over), retires the legacy
> `foxholewarbot` migration, and manages schema via `sqlx::migrate!`. Placeholders become `$1`,
> id columns become `BIGINT` (Discord snowflakes don't fit Postgres `INTEGER`), and the `0/1`
> flags become `BOOLEAN`. Full detail, constraints, and the compose sketch:
> `specs/active/postgres-migration.md`. The **current** SQLite schema is documented below.

```sql
CREATE TABLE guilds (
  id                  INTEGER PRIMARY KEY AUTOINCREMENT,
  guild_id            INTEGER,
  shard               TEXT,        -- resolved API base URL
  shard_name          TEXT,        -- "Able" | "Baker" | "Charlie"
  show_command_output INTEGER CHECK (show_command_output IN (0,1))
);

CREATE TABLE cronjobs (
  guild       INTEGER REFERENCES guilds(id),
  job_name    TEXT UNIQUE,         -- schedule name (globally unique, not per-guild)
  schedule    TEXT,                -- cron expression (from english phrase)
  webhook_url TEXT,
  map_name    TEXT,
  draw_text   INTEGER CHECK (draw_text IN (0,1)),
  job_id      TEXT UNIQUE          -- scheduler UUID, filled in after scheduling
);
```

Notes / quirks (see `qa-report.md`):
- `guilds.guild_id` has **no UNIQUE constraint** → duplicate guild rows are possible, and
  `get_guild` uses `fetch_optional` which errors if more than one row matches.
- `cronjobs.job_name` is globally unique, so two different guilds cannot use the same
  schedule name.

### Legacy migration (`Database::migrate`)
On startup, if a table named `foxholewarbot` exists, every row is copied into `guilds` and the
old table is dropped. If the table is absent the query errors and migration is skipped.

## On-disk cache (`src/utils/cache.rs`)

Created lazily under `./cache/`:
- `cache/map_choices/maps-<Shard>.json` — map list per shard (autocomplete source).
- `cache/dynamic/Dynamic_<map>-<Shard>.json` — last `DynamicMapData`.
- `cache/static/Static_<map>-<Shard>.json` — last `StaticMapData`.
- `cache/war_reports/Report_<map>-<Shard>.json` — last `WarReport`.

`save_maps_cache()` refreshes all three shards' map lists (skipping shards that return 503).

## Map rendering (`src/utils/request_processing.rs::place_image_info`)

> **Rewrite decision:** the placement math is being de-magicked before the full-world renderer
> lands. The literal `0.5` icon scale (really a fixed 24 px), the `25.0` font size, and the
> implicit top-left anchoring move into a documented, canvas-relative `RenderConfig`, and
> `MapMarkerType` (currently parsed but unused) starts sizing Major vs Minor labels. Detail and
> the anchoring decision: `specs/active/rendering-placement.md`. Current behavior below.

Inputs: `DynamicMapData`, `StaticMapData`, `draw_text: bool`, background image path.
1. Open `assets/Maps/Map<name>.TGA` as RGBA; return `None` if it can't be opened.
2. For each dynamic `map_item`: load `assets/MapIcons/<icon_type><TeamId>.png`
   (e.g. `12Colonials.png`, `40None.png`). Missing icon → fall back to `DebugIcon.png`.
   Each icon is scaled to 50% and overlaid at `(x * width, y * height)`.
3. If `draw_text`: render each `map_text_item.text` in Inter-Bold 25px black at its
   normalized coordinates.
4. Return the composited `ImageBuffer`.

The composite is written to a **fixed path `render.png`** in the working directory and attached
to the Discord response. (This shared filename is a concurrency bug — see `qa-report.md`.)

## Scheduler (`src/utils/cron.rs`)

- `start_map_update_job` — cron `0 0 0 * * *` (daily at 00:00) refreshes the map lists.
- Report jobs are stored in `cronjobs` and re-created on startup by `restart_report_jobs`.
- `add_report_job(from_db)` — when `from_db == false`, first persists the job row, then
  schedules it and writes back the scheduler UUID via `update_job_uuid`.
- `remove_report_job` — looks up the job, deletes the channel webhook **only if it's the
  guild's last job**, removes the scheduler entry by UUID, deletes the DB row.
- English-phrase schedules are parsed by `Job::schedule_to_cron` ("every 30 seconds",
  "at 5:00 pm", "On Monday at 8:00", …).

## Output visibility

Every data command reads `guilds.show_command_output`:
- `1` → `interaction.defer()` (public response, visible to the channel).
- `0` → `interaction.defer_ephemeral()` (only the caller sees it).

`/set-guild-settings` is always ephemeral. `/war-state` documents itself as "defaults to not
showing" but actually honors the guild setting like every other command.
