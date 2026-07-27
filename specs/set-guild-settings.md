# /set-guild-settings ⚠️

## Summary
Admin command that sets the shard the guild pulls data from and whether command output is
public or ephemeral. Creates the guild row on first use, updates it thereafter.

## Command surface
- Name: `set-guild-settings`
- Options:
  - `shard` (string, **required**, autocomplete) — server to read from.
  - `show-messages` (boolean, **required**) — `true` = public, `false` = ephemeral.
- Permissions: `ADMINISTRATOR` (`default_member_permissions`).

### Autocomplete
Static list: `Able`, `Baker`, `Charlie` (does not filter by typed input).

## Behavior
1. Always defer ephemeral.
2. Read `shard` and `show-messages`.
3. Validate the shard by `GET {shard_api}/worldconquest/war`:
   - `200 OK` → create or update the guild row; reply "Created!" / "Updated!".
   - `503 Service Unavailable` → reply "Cannot set as the server selected is Unavailable!".
   - anything else → reply "An Error Occured!".
4. Create path uses `create_guild`; update path uses `update_guild`.

## External calls
- Foxhole: `GET /worldconquest/war` on the selected shard (validation probe).
- DB: insert or update the `guilds` row.

## Quirks & known bugs
- **`show-messages` is ignored on first-time setup** — the create path calls `create_guild`,
  which hard-codes `show_command_output = 0`. A brand-new guild that sets `show-messages:true`
  silently gets ephemeral output until they run the command a second time (update path).
  (qa-report: B-1)
- **`send().await.unwrap()`** on the validation probe panics on a network error (the
  interaction was already deferred → stuck spinner). (qa-report: C-3)
- Any unrecognized shard string silently resolves to `Able` (`Shard::from_str` default),
  so a typo doesn't error — it just picks Able.
- `guild_id.unwrap()` panics outside a guild. (qa-report: C-4)

## Acceptance criteria
- An admin runs `/set-guild-settings shard:Baker show-messages:true`; the guild row is created
  and subsequent data commands read from Baker.
- Selecting an unavailable shard yields the "Unavailable" message and does not change settings.
- Non-admins cannot see/use the command.
- **Target behavior (fix):** `show-messages` is honored on the first invocation too.
