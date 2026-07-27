# /set-guild-settings

## Summary
Admin command that sets the shard the guild pulls data from and whether command output is public
or ephemeral. Creates the guild row on first use, updates it thereafter.

## Command surface
- Name: `set-guild-settings`, guild-only.
- Options:
  - `shard` (**required**, choice) — `Able` | `Baker` | `Charlie`.
  - `show_messages` (boolean, **required**) — `true` = public, `false` = ephemeral.
- Permissions: `ADMINISTRATOR` (`default_member_permissions`).

The shard is a Discord **choice list**, not free text with autocomplete. The pre-rewrite
version accepted arbitrary strings and resolved anything unrecognized to Able, so a typo
silently pointed a guild at the wrong shard.

## Behavior
1. Always defer ephemeral.
2. Validate the shard by `GET {shard_api}/worldconquest/war`:
   - `200 OK` → continue.
   - `503 Service Unavailable` → reply that the shard is unavailable; **nothing is written**.
   - anything else → reply with the status; nothing is written.
3. Upsert the guild row (`INSERT … ON CONFLICT (guild_id) DO UPDATE`). One statement serves both
   create and update, and `show_messages` is honored on **both** paths.
4. Reply "Created"/"Updated" with the chosen shard and the resulting visibility, in words.

## External calls
- Foxhole: `GET /worldconquest/war` on the selected shard (validation probe).
- DB: upsert the `guilds` row, keyed on the `guild_id UNIQUE` constraint.

## Notes
- A guild with no row is "not set up": every other command prompts the user here. No row is
  created when the bot joins a server — an admin picks the shard deliberately rather than
  silently inheriting Able.
- Because `guild_id` is `UNIQUE`, a guild can never end up with two rows, which is what used to
  make lookups fail permanently once duplicates existed.

## Acceptance criteria
- An admin runs `/set-guild-settings shard:Baker show_messages:true`; the guild row is created
  **with visibility public**, and subsequent data commands read from Baker.
- Running it twice for the same guild updates one row — no duplicates.
- Selecting an unavailable shard yields the "unavailable" message and does not change settings.
- Non-admins cannot see or use the command.
