# Changelog draft — the rewrite

Written for the `#changelog` channel, in the style of the previous entries. Paste as-is.
Date is a placeholder until it actually ships.

---

# `Rewrite 2.0 - 27.7.2026`
- The bot has been rewritten again, on a new command framework. Commands reply faster, errors say what went wrong instead of nothing, and the bot no longer dies from a bad map name.
- Database moved from PocketBase to a self-hosted Postgres. Settings and scheduled reports live in one place now, and are migrated automatically.
- `/set-server` and `/set-visibility` are now one command: `/set-guild-settings`. Shard, reply visibility, timezone and the map tint are all set there.
- All map art and icons were regenerated from the official war API assets, so hexes stop going missing when Clapfoot renames something.

# `New: The Whole World Map`
- `/full-map` renders all 53 hexes into a single image. Free, on demand, as often as you like.
- Optional faction colouring on the world map, off by default. Turn it on with `/set-guild-settings faction_tint:true` and each hex is tinted by who holds it.
- Putting the **world map** on a recurring schedule needs approval first, via `/request-full-map-schedule`. It's a short form and you get an answer in a channel you pick. **No payment involved, there is no paid tier** - a scheduled world map re-renders every region on a timer, so I'd like to know it's going somewhere that wants it. Single hex schedules and `/full-map` on demand are unaffected.

# `Scheduled Reports, Reworked`
- **No more typing the schedule.** `/schedule-report` now asks for a frequency from a list. Every 30 minutes through weekly, plus a `Custom...` option for cron or a plain-English phrase if you liked the old way.
- **Timezones.** Set your server's timezone in `/set-guild-settings`, or override it per report. Pick an IANA name like `Europe/Berlin` from the autocomplete and daylight saving is handled for you. Reports no longer drift by an hour twice a year.
- `at_time` lines the schedule up with a clock time, and not just for daily reports. "Every 6 hours" at `03:30` is 03:30, 09:30, 15:30, 21:30.
- **The next three run times are shown before the schedule is saved**, on your own clock. If they aren't what you meant, nothing has been created yet.
- Reports post at most every 30 minutes, or every hour for the world map. Anything faster is just spam in a channel, and `/get-map` is still instant and unlimited.
- Creating a schedule does not post a report immediately. The first one arrives at the first time listed in the reply, and the reply now says so.
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

# `Misc. Changes`
- Schedule names only have to be unique within your own server.
- `/schedule-report` and `/remove-report` need **Manage Webhooks**, `/set-guild-settings` needs **Administrator**.
- There is a website now, with a FAQ, terms and a privacy policy: what the bot stores, why, and how to get it deleted.
