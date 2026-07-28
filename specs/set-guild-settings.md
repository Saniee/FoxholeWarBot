# /set-guild-settings

## Summary
Admin command that sets the shard the guild pulls data from, whether command output is public or
ephemeral, the full-map faction tint, the frontline overlay, and the server's default timezone
for scheduled reports.
Creates the guild row on first use, updates it thereafter.

## Command surface
- Name: `set-guild-settings`, guild-only.
- Options:
  - `shard` (**required**, choice) — `Able` | `Baker` | `Charlie`.
  - `show_messages` (boolean, **required**) — `true` = public, `false` = ephemeral.
  - `faction_tint` (boolean, optional) — shade each hex on `/full-map` by who holds it.
  - `frontline` (boolean, optional) — draw the contested boundary between the factions. **Both**
    `/get-map` and `/full-map`, unlike the tint: the tint answers "who holds this hex" at hex
    resolution, and the frontline answers a sub-region question that the contested hex — the one
    a player cares about most — is served worst by. See `specs/active/frontline.md`.
  - `timezone` (string, optional, autocomplete) — IANA name, the default new schedules inherit.
- Permissions: `ADMINISTRATOR` (`default_member_permissions`).

**The three optional settings mean "leave it alone" when omitted, not "reset it".** `shard` and
`show_messages` are required every time, so a server changing shards would otherwise silently
lose its tint or its frontline — and, worse, have its clock reset to UTC underneath every
schedule that inherits from it. Only an insert falls back to a default (both overlays off,
`UTC`).

The shard is a Discord **choice list**, not free text with autocomplete. The pre-rewrite
version accepted arbitrary strings and resolved anything unrecognized to Able, so a typo
silently pointed a guild at the wrong shard.

## Behavior
1. Always defer ephemeral.
2. Validate the shard by `GET {shard_api}/worldconquest/maps` — the **region list**, not
   `/worldconquest/war`. Every outcome below leaves the guild row untouched and says so:
   - a non-empty list → continue, and the list is written to the cache on the way past, so
     `/get-map`'s autocomplete works immediately rather than at the next hourly refresh.
   - an **empty** list → the shard isn't running a war; there would be nothing to render.
   - a non-`200` status → reply with the status.
   - **no answer at all** (DNS, refused, TLS, timeout) → reply that the shard is unreachable.

   Two things here are deliberate. The probe asks for the region list because that is what
   `/get-map`, `/war-report` and `/full-map` all need before they can do anything — a host
   answering on `/worldconquest/war` proves only that something is listening. And the transport
   failure is a case in its own right: it used to propagate to `on_error` as "Something went
   wrong running that command", which named neither the shard nor the problem, for the one
   input the check exists to reject.
3. Validate `timezone` against the IANA database **before** storing it, so a typo can't sit in
   the row until the next schedule quietly falls back to UTC. The canonical name is what's
   stored, whatever case was typed.
4. Upsert the guild row (`INSERT … ON CONFLICT (guild_id) DO UPDATE`). One statement serves both
   create and update, and `show_messages` is honored on **both** paths.
5. Reply "Created"/"Updated" with the chosen shard, the resulting visibility, tint and timezone —
   read back from the stored row, so an omitted option reports what the guild actually has rather
   than "unchanged".

## External calls
- Foxhole: `GET /worldconquest/maps` on the selected shard (validation probe; its result is
  cached as the shard's region list).
- DB: upsert the `guilds` row, keyed on the `guild_id UNIQUE` constraint.

## Notes
- A guild with no row is "not set up": every other command prompts the user here. No row is
  created when the bot joins a server — an admin picks the shard deliberately rather than
  silently inheriting Able.
- Because `guild_id` is `UNIQUE`, a guild can never end up with two rows, which is what used to
  make lookups fail permanently once duplicates existed.
- **All three shards are always offered, including a shard that is down.** A choice list is baked
  into the command at registration and served from Discord's own copy, so an option cannot be
  withdrawn for the hour a shard is offline. Validating at the moment of choosing is the whole
  defence here — and it is not sufficient by itself, since a shard can go down after it is set,
  which is why every data command also degrades gracefully rather than assuming its shard is up.

## Acceptance criteria
- An admin runs `/set-guild-settings shard:Baker show_messages:true`; the guild row is created
  **with visibility public**, and subsequent data commands read from Baker.
- Running it twice for the same guild updates one row — no duplicates.
- Selecting a shard that returns an error, lists no regions, or cannot be reached at all yields a
  message naming that shard and which of the three it was, and changes no settings.
- A successful run leaves `/get-map`'s region autocomplete populated without waiting for a
  refresh.
- Non-admins cannot see or use the command.
