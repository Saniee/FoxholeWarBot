# Changelog — 2.0

Written for the `#changelog` channel, in the style of the previous entries. Paste as-is.
**Set the date to the day it actually ships**; everything below is otherwise final.

The version in `Cargo.toml` is `2.0.0` to match this heading — it isn't surfaced anywhere in the
bot, so it exists purely so the tag, the crate and this post agree.

---

# `Rewrite 2.0 - 27.7.2026`
- The bot has been rewritten again, on a new command framework. Commands reply faster, errors say what went wrong instead of nothing, and the bot no longer dies from a bad map name.
- Database moved from PocketBase to a self-hosted Postgres. Settings and scheduled reports live in one place now, and are migrated automatically.
- `/set-server` and `/set-visibility` are now one command: `/set-guild-settings`. Shard, reply visibility, timezone and the map tint are all set there.
- All map art and icons were regenerated from the official war API assets, so hexes stop going missing when Clapfoot renames something.

# `New: The Whole World Map`
- `/full-map` renders all 53 hexes into a single image. Free, on demand, as often as you like.
- Every hex is labelled with its region name, so you can find Marban Hollow without already knowing where it is.
- Region borders are drawn on the world map, so it's clear where one hex ends and the next begins.
- Optional faction colouring on the world map, off by default. Turn it on with `/set-guild-settings faction_tint:true`. The colour follows the **frontline**, not the hex outline — a contested region comes out part green and part blue, split exactly where the front runs through it.
- `/full-map` tells you it's working before it starts. Stitching 53 regions takes a moment, and the message says which shard and which overlays are on, then turns into the finished map.
- Putting the **world map** on a recurring schedule needs approval first, via `/request-full-map-schedule`. It's a short form and you get an answer in a channel you pick. **No payment involved, there is no paid tier** - a scheduled world map re-renders every region on a timer, so I'd like to know it's going somewhere that wants it. Single hex schedules and `/full-map` on demand are unaffected.

# `New: The Frontline`
- Maps can now draw the **contested frontline** — the line between what each faction holds. Off by default; turn it on with `/set-guild-settings frontline:true`.
- Works on both `/get-map` and `/full-map`, and on scheduled reports.
- The line is drawn from where each side has actually built, so it sits between the two front lines of bases rather than snapping to hex edges. A single hex's line accounts for its neighbours too, so it runs all the way to the edge of the image instead of stopping short.
- Each side of the line is coloured with the faction that holds that ground, so you can tell at a glance which way the front is facing. With the world map's faction colouring on, the line goes black instead — the ground either side is already carrying the colours.

# `Scheduled Reports, Reworked`
- **No more typing the schedule.** `/schedule-report` now asks for a frequency from a list. Every 30 minutes through weekly, plus a `Custom...` option for cron or a plain-English phrase if you liked the old way.
- **Timezones.** Set your server's timezone in `/set-guild-settings`, or override it per report. Pick an IANA name like `Europe/Berlin` from the autocomplete and daylight saving is handled for you. Reports no longer drift by an hour twice a year.
- `at_time` lines the schedule up with a clock time, and not just for daily reports. "Every 6 hours" at `03:30` is 03:30, 09:30, 15:30, 21:30.
- **The next three run times are shown before the schedule is saved**, on your own clock. If they aren't what you meant, nothing has been created yet.
- Reports post at most every 30 minutes, or every hour for the world map. Anything faster is just spam in a channel, and `/get-map` is still instant and unlimited.
- Creating a schedule does not post a report immediately. The first one arrives at the first time listed in the reply, and the reply now says so.
- Reports no longer appear out of nowhere. The bot posts a "fetching the war data" message first and then edits that same message into the finished report, so it's one message per run and you can see it working. The world map says it'll take a few seconds, because it does.
- If a report fails to render, that message says so instead of leaving you wondering. The schedule keeps running.
- The report embed shows when the next update is due, in your timezone.
- `/schedule-help` rewritten to explain what the options do, since there is no longer a syntax to get wrong.

# `Bug Fixes`
- Scheduled jobs no longer stack up and post duplicates when the bot reconnects to Discord.
- Every outgoing request now has a timeout. A silent connection to Discord or the war API used to hang a command forever with no reply.
- `/set-guild-settings` replies again.
- Two hexes that rendered as blank tiles are fixed, and a region the API forgets to list is now drawn as plain terrain instead of a hole in the map.
- Icons on the world map are legible instead of one pixel across.
- Missing map icons no longer error, and the log tells the two kinds of missing icon apart.
- Scheduling now fails cleanly. A rejected schedule leaves nothing behind in the database, and a report whose webhook is gone is skipped and logged instead of taking the scheduler down with it.
- Missing **Manage Webhooks** used to crash `/schedule-report`. It now tells you what to grant.
- A shard that is offline or between wars now says so, and names the shard, instead of failing with a generic error you can't act on.
- One more structure type has art and renders on the map. It's live in game but missing from the official asset list, so it was drawing as nothing.

# `Misc. Changes`
- Schedule names only have to be unique within your own server.
- `/schedule-report` and `/remove-report` need **Manage Webhooks**, `/set-guild-settings` needs **Administrator**.
- There is a website now, with a FAQ, terms and a privacy policy: what the bot stores, why, and how to get it deleted.

# `For Self-Hosters`
- The bot writes **log files** now, not just console output: a plain one and a verbose one that carries everything the libraries say. `LOG_DIR` sets where they go, `LOG_RETENTION_DAYS` how long they're kept (default 14). Both live on a mounted volume in the compose file, so they survive a rebuild.
- The console is quiet again — it shows the bot's own messages and warnings, and the gateway and per-statement database chatter goes to the verbose file instead.
- `RUST_LOG`, `LOG_FILE_FILTER` and `LOG_VERBOSE_FILTER` override the filtering for each of the three separately.
