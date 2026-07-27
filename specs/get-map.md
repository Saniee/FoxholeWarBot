# /get-map

## Summary
Renders one Foxhole region as a PNG — background hex art, colored control/structure icons, and
optional text labels — and returns it in an embed.

## Command surface
- Name: `get-map`, guild-only.
- Options:
  - `map_name` (string, **required**, autocomplete) — API region id (e.g. `DeadLandsHex`).
  - `draw_text` (boolean, optional, default `false`) — overlay static text labels.
- Permissions: none (any member).

### Autocomplete
Shared with `/war-report` and `/schedule-report` — one implementation,
`commands/common.rs::autocomplete_map`:
- Requires the guild to be set up; otherwise returns a single placeholder choice telling the
  user to run `/set-guild-settings`. Its value is `"-"`, not empty — Discord rejects an empty
  choice value.
- Loads the shard's cached region list, matches the typed substring case-insensitively against
  **both** the display name and the API id (so "mooring" and "the moors" both find The Moors),
  caps at 25 choices.
- Labels come from `utils::regions::display_name`, an explicit table. Stripping the `Hex`
  suffix — what the pre-rewrite code did — is wrong for roughly a third of the map.
- `OriginHex` is **not** excluded. The only guard is "the API didn't list it", which is
  inherent: the list comes from `/worldconquest/maps` for the guild's own shard.

## Behavior
1. Look up the guild. If not set up → ephemeral "No shard is set for this server…".
2. Defer public or ephemeral per `show_command_output`.
3. Delegate to `utils::map_render::render_region` — shared with the scheduled report tick, see
   `architecture.md` → Map pipeline.
4. Reply with an embed: green, title = the region's display name, description
   `Last API Update: <formatted last_updated>`, the rendered PNG attached as `<map_name>.png`,
   footer "Requested at" + timestamp.

Every failure path replies, so a deferred interaction never hangs on its spinner:

| Condition | Reply |
|---|---|
| Both endpoints 500 / API unreachable | "The Foxhole API is not responding right now." |
| Render failure (e.g. missing background art) | "Couldn't render that region." |
| Fetch or decode failure | "Couldn't fetch that region's data." |

## External calls
- Foxhole: `GET .../maps/{map}/dynamic/public`, `GET .../maps/{map}/static`.
- Disk: reads/writes cache JSON. **Nothing is written for the render itself** — the PNG is
  encoded in memory and attached directly, so concurrent renders cannot collide.

## Rendering
See `architecture.md` → Map rendering and `rendering-placement.md`. Background art is
`assets/Maps/Map<map>.TGA`.

## Acceptance criteria
- Setting up a guild then `/get-map map_name:DeadLandsHex` returns an embed with a rendered
  map image and a "Last API Update" line.
- `draw_text:true` overlays region text labels, sized differently for Major and Minor markers.
- Autocomplete filters on the typed substring, shows real display names ("Deadlands", not
  "DeadLands"), and includes Origin.
- Concurrent `/get-map` calls for different regions never return each other's image.
- A region whose background art is missing replies with an error rather than hanging.
