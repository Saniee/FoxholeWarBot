# /schedule-help

## Summary
Explains how scheduled reports are timed: what the frequency choices mean, what `at_time`
anchors, and how timezones are handled.

## Command surface
- Name: `schedule-help`, guild-only.
- Options: none.
- Permissions: none.

## Behavior
Replies ephemerally with static help text. **No guild-setup check** — it's documentation, and
gating it meant a server that hadn't configured the bot yet couldn't read the instructions for
configuring it.

The content is inline, not a link. The pre-rewrite version pointed at a `prnt.sc` screenshot,
which made an external image host the sole documentation for the syntax.

This page used to teach a syntax — the English phrases a free-text `schedule` box accepted. That
box is gone (`specs/schedule-input.md`), so what remains worth explaining is what the
frequencies *mean* and which clock they are read in, which is where the confusion actually was.

Covered:
- Schedule names must be unique **within the server**.
- `at_time` anchors the cadence, not only a daily one: `every 6 hours` at `03:30` is 03:30,
  09:30, 15:30, 21:30. Blank means the top of the hour.
- These are **clock times, not "from now"** — `every 6 hours` created at 09:20 next fires at
  12:00, not 15:20. This alone accounts for a share of the old "it fires at the wrong time".
- Timezones: the guild default from `/set-guild-settings`, the per-report override on
  `/schedule-report`, both IANA names from an autocomplete, and the fact that a schedule keeps
  the zone it was created with.
- `Custom…` takes cron or an English phrase, for whatever the list doesn't cover.
- Every schedule shows its next three fire times before it is saved.

## Acceptance criteria
- `/schedule-help` returns the reference without requiring guild setup.
- The text explains that frequencies are absolute clock times.
- No external link is required to understand any of it.
