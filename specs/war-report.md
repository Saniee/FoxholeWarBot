# /war-report

## Summary
Returns per-region war statistics (enlistments, casualties per faction, day of war) in an embed.

## Command surface
- Name: `war-report`, guild-only.
- Options:
  - `map_name` (string, **required**, autocomplete) — API region id.
- Permissions: none.

### Autocomplete
Identical to `/get-map` — the same `commands/common.rs::autocomplete_map` helper. See that spec.

## Behavior
1. Look up guild; if not set up → ephemeral prompt to run `/set-guild-settings`.
2. Defer public/ephemeral per `show_command_output`.
3. Load the cached `WarReport` for `(map, shard)`.
4. `GET /worldconquest/warReport/{map}` with `If-None-Match` (cached version, or `"0"` when
   there is no cache).
5. Resolve the response:
   - `304` → serve the cached report. If the cache went away between the read and the request,
     refetch unconditionally rather than failing.
   - `200` → parse fresh and write the cache.
   - anything else → serve the cached report if there is one, otherwise reply with the status
     and return.
6. Embed (black) titled with the region's display name; fields Total Enlistments, Colonial
   Casualties, Warden Casualties, Day Of War; footer "Requested at" + timestamp.

## External calls
- Foxhole: `GET .../worldconquest/warReport/{map}`.
- Disk: reads/writes `cache/war_reports/Report_<map>-<Shard>.json`.

## Acceptance criteria
- `/war-report map_name:<region>` returns an embed with the four numeric fields populated.
- Repeated calls within an unchanged war state serve cached values (the 304 path) without error.
- A transient upstream error serves the cached report rather than failing, when one exists.
- Autocomplete behaves as in `/get-map`.
