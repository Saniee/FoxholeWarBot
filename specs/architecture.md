# Architecture (cross-cutting)

Shared machinery every command depends on. A rewrite should reproduce these contracts.

## Process lifecycle (`src/main.rs`)

1. Parse CLI args (`--local`, `--clear-commands`).
2. `CronHandler::new()` — creates and starts a `tokio_cron_scheduler::JobScheduler`.
3. If `--clear-commands`: build a minimal client, delete all global commands and all
   dev-guild (`GUILD_ID`) commands, then exit.
4. Read `TOKEN` from env; `Database::connect()` opens `database.db` (created if missing).
5. Create tables `guilds` and `cronjobs` if they don't exist (schema below).
6. `db.migrate()` — one-time migration from a legacy `foxholewarbot` table (see below).
7. Build the serenity `Client` with only `GatewayIntents::GUILDS` and the `Handler`.
8. On `ready`:
   - Set presence to "Watching Foxhole Wars", status idle.
   - `save_maps_cache()` — refresh the per-shard map list cache.
   - Register the 7 slash commands globally (or to `GUILD_ID` when `--local`).
   - Start the daily map-list refresh job and restart persisted report jobs.

### Gateway intents
Only `GUILDS`. The bot does **not** request message content or member intents; it operates
purely through slash-command interactions and webhooks.

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
