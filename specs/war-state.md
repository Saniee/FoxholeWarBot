# /war-state ⚠️

## Summary
Reports the global state of the current war on the guild's shard.

## Command surface
- Name: `war-state`
- Options: none.
- Permissions: none.
- Self-description claims "defaults to not showing", but it actually honors the guild's
  `show_command_output` like every other command.

## Behavior
1. Look up guild; if not set up → ephemeral prompt to run `/set-guild-settings`.
2. Defer public/ephemeral per `show_command_output`.
3. `GET /worldconquest/war` on the guild's shard, parse `War`.
4. Embed (red color) fields: Shard/Server (`shard_name`), War Number, Winner,
   Conquest Start Time (formatted from `conquest_start_time` millis),
   Required Victory Towns; footer "Requested at" + timestamp.

## External calls
- Foxhole: `GET /worldconquest/war`. No caching (always live).

## Quirks & known bugs
- **No ETag/caching** — always hits the API.
- **Chained unwraps on one line** — `send().await.unwrap()` → `json::<War>().await.unwrap()` →
  `from_timestamp_millis(...).unwrap()`. Any of a network error, malformed JSON, or an
  out-of-range timestamp panics after the interaction was already deferred, leaving a
  permanent "thinking…" spinner. (qa-report: C-3, C-5)
- `guild_id.unwrap()` panics outside a guild. (qa-report: C-4)
- `winner` is shown raw (e.g. `"NONE"` while the war is ongoing).

## Acceptance criteria
- `/war-state` returns an embed with war number, winner, conquest start time, and required
  victory towns for the guild's configured shard.
- Response visibility matches the guild's `show_command_output` setting.
