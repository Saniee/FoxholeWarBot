# /war-report ⚠️

## Summary
Returns per-hex war statistics (enlistments, casualties per faction, day of war) in an embed.

## Command surface
- Name: `war-report`
- Options:
  - `map-name` (string, **required**, autocomplete) — hex name.
- Permissions: none.

### Autocomplete
Identical to `/get-map`: cached shard map list, case-insensitive substring filter, excludes
`OriginHex`, 25-choice cap, `Hex` suffix stripped from labels. Requires guild setup.

## Behavior
1. Look up guild; if not set up → ephemeral prompt (message says `/set-shard`, a stale command
   name — see quirks).
2. Defer public/ephemeral per `show_command_output`.
3. Load cached `WarReport` for `(map, shard)`.
4. `GET /worldconquest/warReport/{map}` with `If-None-Match` (cached version or `"0"`).
5. `500` → return silently.
6. Non-`304` → parse fresh, build embed, save cache. `304` → build embed from cache.
7. Embed (black color) fields: Total Enlistments, Colonial Casualties, Warden Casualties,
   Day Of War; footer "Requested at" + timestamp.

## External calls
- Foxhole: `GET .../worldconquest/warReport/{map}`.
- Disk: reads/writes `cache/war_reports/Report_<map>-<Shard>.json`.

## Quirks & known bugs
- **Stale command name in copy** — the not-set-up message references `/set-shard`, which no
  longer exists (it's `/set-guild-settings`). (qa-report: L-1)
- Unwrapped `send()`/`json()` → panic on transport/parse error. (qa-report: C-3)
- `guild_id.unwrap()` panics outside a guild. (qa-report: C-4)
- `500` path leaves the interaction deferred with no reply.

## Acceptance criteria
- `/war-report map-name:<hex>` returns an embed with the four numeric fields populated.
- Repeated calls within an unchanged war state serve cached values (304 path) without error.
- Autocomplete behaves as in `/get-map`.
