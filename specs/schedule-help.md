# /schedule-help

## Summary
Explains the accepted phrases for the `schedule` argument of `/schedule-report`.

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

Covered:
- Schedule names must be unique **within the server**.
- Phrase examples: `every 30 minutes`, `every 2 hours`, `at 6:30 pm`, `every day at 09:00`,
  `on Monday at 5:00 pm`, `every Friday at 18:00`.
- A raw 6-field cron expression (`sec min hour day month weekday`) also works.
- **Times are UTC** — the scheduler runs in UTC, so the help says so.

## Acceptance criteria
- `/schedule-help` returns the phrase reference without requiring guild setup.
- The text names UTC explicitly.
- No external link is required to understand the syntax.
