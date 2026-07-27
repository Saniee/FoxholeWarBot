# /war-state

## Summary
Reports the global state of the current war on the guild's shard.

## Command surface
- Name: `war-state`, guild-only.
- Options: none.
- Permissions: none.

## Behavior
1. Look up guild; if not set up → ephemeral prompt to run `/set-guild-settings`.
2. Defer public/ephemeral per `show_command_output`.
3. `GET /worldconquest/war` on the guild's shard, parse `War`. A non-success status is an
   error, handled by the framework's `on_error` hook, which always replies.
4. Embed (red) fields: Shard/Server (`shard_name`), War Number, Winner, Conquest Start Time,
   Required Victory Towns; footer "Requested at" + timestamp.

## External calls
- Foxhole: `GET /worldconquest/war`. No caching — always live.

## Notes
- **No ETag/caching.** This endpoint has no `version` field to revalidate against, so every
  call hits the API. It's a single small JSON response, so this is acceptable.
- `winner` is shown raw (e.g. `NONE` while the war is ongoing). Deliberate — it mirrors what
  the API reports.
- Timestamps go through `utils::format_timestamp`, which renders an unrepresentable value as
  `unknown` rather than failing.
- The command description no longer claims it "defaults to not showing"; it honors the guild's
  `show_command_output` like every other command, and always did.

## Acceptance criteria
- `/war-state` returns an embed with war number, winner, conquest start time, and required
  victory towns for the guild's configured shard.
- Response visibility matches the guild's `show_command_output` setting.
- An API outage produces an error reply, not a hanging spinner.
