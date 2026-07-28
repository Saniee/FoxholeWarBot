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
3. `GET /worldconquest/war` on the guild's shard, parse `War`. Each of the three ways this can
   fail answers **here**, naming the shard, rather than falling through to `on_error`:
   - no answer at all → "couldn't reach shard **X**", with a pointer to `/set-guild-settings`.
   - a non-success status → the status, and that the shard may be down or between wars.
   - a body that won't parse → say so, and ask for a report if it persists.
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
- **This is the command a user reaches for when something looks wrong**, so it is the one that
  must not answer "Something went wrong running that command" — which is what `on_error` says,
  and what a guild whose shard had gone offline used to get for every one of the three failures
  above. Each now names the shard, which is the only part the user can act on.

## Acceptance criteria
- `/war-state` returns an embed with war number, winner, conquest start time, and required
  victory towns for the guild's configured shard.
- Response visibility matches the guild's `show_command_output` setting.
- An API outage produces an error reply, not a hanging spinner.
- A shard that is offline or unreachable produces a message naming that shard, not the generic
  command-failure reply.
