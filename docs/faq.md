---
layout: default
---

# [](#header-1)Missing Needed Permissions?:

## The bot needs these permissions:

- View Channel
- Send Messages
- Embed Links
- Attach Files
- Use Application Commands
- Manage Webhooks — **only** for `/schedule-report`; everything else works without it

It does not need Add Reactions or Send Messages in Threads. It never reacts to messages, never
reads them, and runs on the `GUILDS` gateway intent alone.

# [](#header-2)Getting started:

Run `/set-guild-settings` first and pick your shard (Able, Baker or Charlie) and whether replies
should be public or private. Until that's done, the other commands will just point you back here
— a new server is never silently assigned a shard.

# [](#header-3)Commands:

- `/get-map` — render one region, with optional text labels.
- `/war-report` — casualties, enlistments and day of war for one region.
- `/war-state` — the global war state for the server's shard.
- `/set-guild-settings` — set the shard and reply visibility (needs Administrator).
- `/schedule-report` — post a region's map to a channel on a recurring schedule
  (needs Manage Webhooks).
- `/remove-report` — delete a schedule and its webhook (needs Manage Webhooks).
- `/schedule-help` — the schedule phrases, in the client.

# [](#header-4)Schedule phrases:

`/schedule-report` accepts plain-English phrases:

- `every 30 minutes`
- `every 2 hours`
- `at 6:30 pm`
- `every day at 09:00`
- `on Monday at 5:00 pm`
- `every Friday at 18:00`

A 6-field cron expression (`sec min hour day month weekday`) also works — `0 0 12 * * *` is
daily at noon. **Times are interpreted in UTC.** Schedule names must be unique within a server.

# [](#header-5)Downtime or bot is down?:

## Please head to the support discord and ping me "@saniee" so I know about it. I don't check the bot regularly.

<a href="https://discord.gg/9wzppSgXdQ">Support Discord Server</a>

[Back](https://saniee.github.io/FoxholeWarBot/)
