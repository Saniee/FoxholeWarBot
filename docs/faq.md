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
- `/full-map` — render the whole world map, all 53 regions on one image. Free for everyone; it
  takes a few seconds, so the reply arrives after a short wait.
- `/war-report` — casualties, enlistments and day of war for one region.
- `/war-state` — the global war state for the server's shard.
- `/set-guild-settings` — set the shard, reply visibility and the server's timezone (needs
  Administrator).
- `/schedule-report` — post a region's map, or the whole world map, to a channel on a recurring
  schedule (needs Manage Webhooks). Scheduling the *world map* needs approval first — see below.
- `/request-full-map-schedule` — apply to schedule world-map reports (needs Manage Webhooks).
- `/remove-report` — delete a schedule and its webhook (needs Manage Webhooks).
- `/schedule-help` — how schedules are timed, in the client.

# [](#header-4)When reports post:

`/schedule-report` asks for a **frequency** from a list — every 15 minutes through weekly — so
there is no phrase to get right. `at_time` (24-hour `HH:MM`) is what the cadence lines up with,
and it isn't only for daily reports:

- `every 6 hours` at `03:30` → 03:30, 09:30, 15:30, 21:30
- `every 15 minutes` at `00:07` → :07, :22, :37, :52
- `daily` at `18:00` → 18:00, once a day

Left blank it means the top of the hour. These are **clock times, not "from now"**: `every 6
hours` created at 09:20 next fires at 12:00, not 15:20.

**Timezones.** `/set-guild-settings` sets your server's default and `/schedule-report` can
override it for one report — both from an autocomplete of IANA names like `Europe/Berlin`, so
daylight saving is handled for you. A schedule keeps the timezone it was created with, even if
you change the server default later.

If the list doesn't cover it, `Custom…` still takes a plain-English phrase or a 6-field cron
expression (`sec min hour day month weekday`). Either way, the reply shows **the next three
times it will fire** before anything is saved — if those aren't what you meant, nothing has been
created yet. Schedule names must be unique within a server.

World-map schedules can't run more often than once an hour; every region is re-rendered each
time. `/full-map` on demand has no such limit.

# [](#header-5)Scheduling the world map:

`/full-map` is free for everyone, needs no approval, and you can run it as often as you like.

Putting the world map on a **recurring schedule** is the one thing that needs asking first. Run
`/request-full-map-schedule`, fill in the short form, and you'll get an answer in the channel you
nominated.

**No payment is involved and there is no paid tier.** The reason for the form is arithmetic: an
on-demand render costs one request that somebody deliberately made, while a scheduled one stitches
all 53 regions on a timer, forever, whether or not anyone looks at it. The form keeps that
recurring load to a number somebody has actually seen. It's a queue, not a price.

If approval is later withdrawn, the schedule is **paused, not deleted** — the channel is told once,
and it resumes on its own if approval comes back. Single-region schedules are never affected.

# [](#header-6)Downtime or bot is down?:

## Please head to the support discord and ping me "@saniee" so I know about it. I don't check the bot regularly.

<a href="https://discord.gg/9wzppSgXdQ">Support Discord Server</a>

[Back](https://saniee.github.io/FoxholeWarBot/)
