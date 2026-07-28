---
layout: default
---

# [](#header-1)Privacy Policy:

Short version: the bot stores per-server settings and nothing about you personally.

# [](#header-2)What is collected

- **Server settings.** The server (guild) ID, the shard it reads from, whether replies are
  public or private, whether the full map is shaded by faction control, whether maps are drawn
  with the contested frontline, the server's default timezone for scheduled reports, and whether
  the server is approved to *schedule* full-map reports (plus when that approval was granted).
- **Scheduled reports.** For each one: its name, when it runs (both the timing expression and the
  same cadence in words), the timezone it is read in, the webhook URL it posts to, the region —
  or a marker saying it's the whole world map — and whether labels are drawn.
- **Full-map schedule requests.** Only if someone in your server fills in
  `/request-full-map-schedule`: the Discord user ID of whoever submitted it, the server's ID, a
  one-off snapshot of the server's member count, the channel the reports would post to, and the
  free-text answers given in the form (requested schedule, use case, expected audience, and an
  optional contact handle), plus where the request stands and the ID of the bot's own review post
  for it. See below.

That's the complete list. The full breakdown, field by field, is in the
[Terms of Service](tos).

# [](#header-3)About the full-map request form

This is the one place where the bot stores something tied to a **person** rather than to a
server, and the one place it stores free text somebody typed. It is worth being blunt about:

- It only happens if someone deliberately fills in the form. Nothing is recorded unless they do.
- What's stored is what they wrote, plus their Discord user ID so the decision can be sent back
  to them.
- The answers are read by the bot's owner, in a private channel of the support server, to decide
  the request. They are not published, shared, or used for anything else.
- **No payment is involved at any point.** The form exists because a full map put on a timer is a
  recurring cost on the machine hosting the bot — not because the feature is being sold. There is
  no paid tier, and rendering the full map on demand with `/full-map` is free for everyone and
  needs no request at all.
- Don't put anything sensitive in the free-text boxes. They're for a sentence about what the
  server needs the map for.

# [](#header-4)What is not collected

Message content, member lists, usernames, roles, email addresses, IP addresses, personal profile
data, and payment data. None of it is read, and none of it is stored.

The only personal identifier stored anywhere is the Discord user ID of someone who submits a
full-map schedule request, described above. Servers that never use that form have nothing
personal stored at all.

The bot does not send direct messages to anyone. Older versions of this page said it would DM a
server owner about errors; it never did, and it doesn't now.

# [](#header-5)Command arguments

The options passed to a command (a region name, a frequency) are used to build the request
and then discarded. The exceptions are `/schedule-report` and `/request-full-map-schedule`, which
by definition have to store what you asked for — those fields are listed above.

# [](#header-6)Cached game data

Responses from the Foxhole War API are cached on disk and re-validated against the API rather
than re-downloaded. This cache contains Foxhole game state only: region layouts, map icons,
casualty counts. Nothing in it identifies a person or a server.

# [](#header-7)Operational logs

Like any server software, the bot writes logs about what it is doing. They stay on the machine
running it, are never sent anywhere, and exist so a failure can be diagnosed. They hold what you
would expect from that: server names and IDs, the names of schedules and regions involved in a
failure, and the error itself. A second, more detailed log also records the requests the bot
makes to Discord and to the Foxhole API.

Logs are **not** where the data above is kept — they are a running account of activity, deleted
on a rolling window (14 days by default on the official instance). Nothing in them is used for
anything but keeping the bot working, and no message content is in them: the bot cannot read
messages at all.

# [](#header-8)Retention and removal

Removing the bot from a server deletes that server's settings, all of its schedules, and any
full-map requests it filed. Deleting a single schedule with `/remove-report` removes its stored
row and its webhook immediately.

Full-map requests that were **denied or withdrawn** are deleted after 90 days. Approved requests
are kept while the approval stands, because they are the record of what was approved; if the
approval is later withdrawn, the request counts as withdrawn from that point and is deleted on
the same 90-day clock.
To have anything else looked into, ask in
<a href="https://discord.gg/9wzppSgXdQ">the Support Discord Server</a> or email
<a href="mailto:asamsku10@gmail.com">asamsku10@gmail.com</a>.

[Back](https://saniee.github.io/FoxholeWarBot/)
