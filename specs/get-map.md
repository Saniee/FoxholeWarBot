# /get-map ⚠️

## Summary
Renders a Foxhole hex map as a PNG with colored control/structure icons (and optional text
labels) and returns it in an embed.

## Command surface
- Name: `get-map`
- Options:
  - `map-name` (string, **required**, autocomplete) — hex name (e.g. `DeadLandsHex`).
  - `draw-text` (boolean, optional, default `false`) — overlay static text labels.
- Permissions: none (any member).

### Autocomplete
- Requires the guild to be set up; otherwise returns a single choice telling the user to run
  `/set-guild-settings`.
- Loads the shard's cached map list, filters by case-insensitive substring, excludes
  `OriginHex`, caps at 25 choices, and strips the `Hex` suffix for the display label while
  keeping the raw name as the value.

## Behavior
1. Look up the guild. If not set up → ephemeral "No Shard set… run `/set-guild-settings`".
2. Defer public or ephemeral per `show_command_output`.
3. Read `map-name` and `draw-text` options.
4. Load the on-disk dynamic+static cache for `(map, shard)`.
5. Request `dynamic/public` and `static` with `If-None-Match` set to the cached versions
   (or `"0"` if no cache).
6. If **both** responses are `500` → return silently (leaves the interaction deferred).
7. If **both** responses are *not* `304` → parse fresh JSON, render, save `render.png`,
   update the cache.
   Else → render from cache.
8. Reply with an embed: green color, description `Last API Update: <formatted last_updated>`,
   the rendered image attached, footer "Requested at" + timestamp.

## External calls
- Foxhole: `GET .../maps/{map}/dynamic/public`, `GET .../maps/{map}/static`.
- Disk: reads/writes cache JSON; writes `render.png`.

## Rendering
See `architecture.md` → Map rendering. Background is `assets/Maps/Map<map>.TGA`; a missing
background yields the reply "Couldn't find the map… ||Or an error occured…||".

## Quirks & known bugs
- **Fixed output filename `render.png`** shared by every invocation → concurrent `/get-map`
  calls (or a scheduled report firing simultaneously) can attach the wrong image.
  (qa-report: C-1)
- **Mismatched 304 handling** — the branch condition requires *both* dynamic and static to be
  non-304. If exactly one is `304` and there is no cache, the else-branch unwraps a `None`
  cache and panics. (qa-report: C-2)
- **Unwrapped network/JSON calls** — `send().await.unwrap()` and `json().await.unwrap()` panic
  on transport errors or malformed payloads, leaving the interaction stuck on "thinking…".
  (qa-report: C-3)
- `interaction.guild_id.unwrap()` panics if the command is ever invoked outside a guild.
  (qa-report: C-4)
- `from_timestamp_millis(last_updated).unwrap()` panics on an out-of-range timestamp.
  (qa-report: C-5)
- Both-500 path leaves the user with a permanent "thinking…" spinner (no error reply).

## Acceptance criteria
- Setting up a guild then `/get-map map-name:DeadLandsHex` returns an embed with a rendered
  map image and a "Last API Update" line.
- `draw-text:true` overlays region text labels.
- Autocomplete suggests hex names filtered by the typed substring, never lists `OriginHex`,
  and shows names without the `Hex` suffix.
- An unknown/absent background hex responds with the "Couldn't find the map…" message rather
  than hanging.
