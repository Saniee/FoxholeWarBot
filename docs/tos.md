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

**Per scheduled report**

- The schedule's name, and the schedule phrase that was typed for it (e.g. "every 6 hours").
- The webhook URL the report posts to.
- The region name, and whether text labels are drawn on the map.

**Not stored:** message content, member lists, usernames, server owner identity, roles, personal
profile data, or payment data.

The bot also keeps an on-disk cache of Foxhole War API responses, so it doesn't re-request data
the game hasn't changed. That cache holds game data only — nothing about you or your server.

# [](#header-4)Retention

Settings and schedules are keyed to the server, not to any person. When the bot is removed from
a server its settings are deleted, and every schedule belonging to that server is deleted with
them. Removing a schedule with `/remove-report` deletes both its stored row and its webhook
straight away.

# [](#header-5)Availability

This is a hobby project on hobby infrastructure, offered as-is with no uptime guarantee. It may
be unavailable, out of date, or discontinued at any time.

# [](#header-6)Attribution

Foxhole is a registered trademark of Siege Camp. FoxholeWarBot is an unofficial, free, fan-made
tool and is not affiliated with or endorsed by Siege Camp. Map data is retrieved from the public
Foxhole War API; map and icon artwork are the property of Siege Camp.

# [](#header-7)Any Other Concerns:

Ask in <a href="https://discord.gg/9wzppSgXdQ">the Support Discord Server</a>, or message me
directly: @saniee (Discord), <a href="mailto:asamsku10@gmail.com">Email</a>.

[Back](https://saniee.github.io/FoxholeWarBot/)
