---
layout: default
---

# [](#header-1)Privacy Policy:

Short version: the bot stores per-server settings and nothing about you personally.

# [](#header-2)What is collected

- **Server settings.** The server (guild) ID, the shard it reads from, and whether replies are
  public or private.
- **Scheduled reports.** For each one: its name, the schedule phrase, the webhook URL it posts
  to, the region, and whether labels are drawn.

That's the complete list. The full breakdown, field by field, is in the
[Terms of Service](tos).

# [](#header-3)What is not collected

Message content, member lists, usernames, the server owner's identity, roles, email addresses,
IP addresses, personal profile data, and payment data. None of it is read, and none of it is
stored.

The bot does not send direct messages to anyone. Older versions of this page said it would DM a
server owner about errors; it never did, and it doesn't now.

# [](#header-4)Command arguments

The options passed to a command (a region name, a schedule phrase) are used to build the request
and then discarded. The exception is `/schedule-report`, which by definition has to store what
you asked for so it can repeat it — those fields are listed above.

# [](#header-5)Cached game data

Responses from the Foxhole War API are cached on disk and re-validated against the API rather
than re-downloaded. This cache contains Foxhole game state only: region layouts, map icons,
casualty counts. Nothing in it identifies a person or a server.

# [](#header-6)Retention and removal

Removing the bot from a server deletes that server's settings and all of its schedules. Deleting
a single schedule with `/remove-report` removes its stored row and its webhook immediately.
To have anything else looked into, ask in
<a href="https://discord.gg/9wzppSgXdQ">the Support Discord Server</a> or email
<a href="mailto:asamsku10@gmail.com">asamsku10@gmail.com</a>.

[Back](https://saniee.github.io/FoxholeWarBot/)
