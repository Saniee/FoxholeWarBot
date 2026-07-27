# /set-guild-settings

## Summary
Admin command that sets the shard the guild pulls data from, whether command output is public or
ephemeral, the full-map faction tint, and the server's default timezone for scheduled reports.
Creates the guild row on first use, updates it thereafter.

## Command surface
- Name: `set-guild-settings`, guild-only.
- Options:
  - `shard` (**required**, choice) — `Able` | `Baker` | `Charlie`.
  - `show_messages` (boolean, **required**) — `true` = public, `false` = ephemeral.
  - `faction_tint` (boolean, optional) — shade each hex on `/full-map` by who holds it.
  - `timezone` (string, optional, autocomplete) — IANA name, the default new schedules inherit.
- Permissions: `ADMINISTRATOR` (`default_member_permissions`).

**The two optional settings mean "leave it alone" when omitted, not "reset it".** `shard` and
`show_messages` are required every time, so a server changing shards would otherwise silently
lose its tint — and, worse, have its clock reset to UTC underneath every schedule that inherits
from it. Only an insert falls back to a default (tint off, `UTC`).

The shard is a Discord **choice list**, not free text with autocomplete. The pre-rewrite
version accepted arbitrary strings and resolved anything unrecognized to Able, so a typo
silently pointed a guild at the wrong shard.

## Behavior
1. Always defer ephemeral.
2. Validate the shard by `GET {shard_api}/worldconquest/war`:
   - `200 OK` → continue.
   - `503 Service Unavailable` → reply that the shard is unavailable; **nothing is written**.
   - anything else → reply with the status; nothing is written.
3. Validate `timezone` against the IANA database **before** storing it, so a typo can't sit in
   the row until the next schedule quietly falls back to UTC. The canonical name is what's
   stored, whatever case was typed.
4. Upsert the guild row (`INSERT … ON CONFLICT (guild_id) DO UPDATE`). One statement serves both
   create and update, and `show_messages` is honored on **both** paths.
5. Reply "Created"/"Updated" with the chosen shard, the resulting visibility, tint and timezone —
   read back from the stored row, so an omitted option reports what the guild actually has rather
   than "unchanged".

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
