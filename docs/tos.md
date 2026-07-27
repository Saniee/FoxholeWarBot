---
layout: default
---

# [](#header-1)Terms of Service

FoxholeWarBot is a free, unofficial, fan-made Discord bot. Adding it to a server means agreeing
to what's described here. There is no payment, subscription or premium tier at any point — if
anything ever asks you to pay to use this bot, it isn't this bot.

# [](#header-2)What the bot does

It reads the public Foxhole War API, renders hex maps and war statistics, and replies with them.
Optionally it posts a rendered map to a channel on a recurring schedule, through a webhook it
creates in that channel.

# [](#header-3)What it stores

Exactly this, and nothing else:

**Per server**

- The server (guild) ID.
- The chosen shard (Able / Baker / Charlie) and its display name.
- Whether command output is public or private.
- Whether the full map is shaded by faction control.
- Whether the server is approved to *schedule* full-map reports, and when that approval was
  granted.

**Per scheduled report**

- The schedule's name, and the schedule phrase that was typed for it (e.g. "every 6 hours").
- The webhook URL the report posts to.
- The region name — or a marker saying the report is the whole world map — and whether text
  labels are drawn on the map.
- Whether the server has already been told this schedule is paused, so it isn't told twice.

**Per full-map schedule request** (only if someone submits the form — see below)

- The Discord user ID of whoever submitted it, and the server it was submitted from.
- A one-off snapshot of the server's member count, recorded as context for whoever reviews the
  request. Nothing is decided from it, and it is never refreshed.
- The schedule that was asked for, and the channel the reports would post to.
- The free-text answers: what the server needs it for, its expected audience, and an optional
  contact handle.
- Whether the request is pending, approved or denied, who decided it, and when.

**Not stored:** message content, member lists, usernames, roles, personal profile data, or
payment data. The one personal identifier stored anywhere is the Discord user ID of someone who
submits a full-map request, and only because the answer has to reach them.

The bot also keeps an on-disk cache of Foxhole War API responses, so it doesn't re-request data
the game hasn't changed. That cache holds game data only — nothing about you or your server.

# [](#header-4)Scheduling a full map

Rendering the whole world map on demand with `/full-map` is **free for every server**, needs no
request, and no approval.

Putting one on a **recurring schedule** needs a short request first — `/request-full-map-schedule`
opens a form, and the bot's owner reviews it. This applies to every server regardless of size.

**No payment is involved at any point, and there is no paid tier.** The reason for the form is
plain arithmetic: an on-demand render costs one request, made deliberately by one person, while
a scheduled one fetches and stitches all 53 regions on a timer, forever, whether or not anyone
looks at it. The form is how that recurring load stays a number somebody has actually seen. It
is a queue, not a price.

Approval can be withdrawn later. If it is, the schedule is **paused, not deleted** — the server
is told once, and the schedule resumes by itself if approval is granted again. Single-region
schedules are never affected by any of this.

# [](#header-5)Retention

Settings and schedules are keyed to the server, not to any person. When the bot is removed from
a server its settings are deleted, and every schedule and full-map request belonging to that
server is deleted with them. Removing a schedule with `/remove-report` deletes both its stored
row and its webhook straight away.

Full-map requests that were denied or withdrawn are deleted after 90 days. Approved requests are
kept for as long as the approval stands, since they are the record of what was approved; once an
approval is withdrawn, its request falls under the same 90-day deletion.

A request that hasn't been answered yet can be taken back at any time by whoever filed it — run
`/request-full-map-schedule` again and use the **Withdraw request** button on the reply.

# [](#header-6)Availability

This is a hobby project on hobby infrastructure, offered as-is with no uptime guarantee. It may
be unavailable, out of date, or discontinued at any time.

# [](#header-7)Attribution

Foxhole is a registered trademark of Siege Camp. FoxholeWarBot is an unofficial, free, fan-made
tool and is not affiliated with or endorsed by Siege Camp. Map data is retrieved from the public
Foxhole War API; map and icon artwork are the property of Siege Camp.

# [](#header-8)Any Other Concerns:

Ask in <a href="https://discord.gg/9wzppSgXdQ">the Support Discord Server</a>, or message me
directly: @saniee (Discord), <a href="mailto:asamsku10@gmail.com">Email</a>.

[Back](https://saniee.github.io/FoxholeWarBot/)
