# /schedule-help ✅

## Summary
Explains the rules for the `schedule` argument of `/schedule-report` and links a screenshot of
accepted phrases.

## Command surface
- Name: `schedule-help`
- Options: none.
- Permissions: none.

## Behavior
1. Look up guild; if not set up → ephemeral prompt to run `/set-guild-settings`.
2. Defer per `show_command_output`.
3. Reply with static text: schedules need unique names, and a link to a screenshot of accepted
   phrases (`https://prnt.sc/JOzbuRouDNmq`).

## Quirks & known bugs
- Requires guild setup even though it only returns static help text — a user who hasn't set up
  the bot can't read the scheduling help.
- Help content is an external screenshot link rather than inline text (link rot risk).

## Acceptance criteria
- `/schedule-help` returns the scheduling rules text and the phrase reference link.
