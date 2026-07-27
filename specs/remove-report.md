# /remove-report 🔧

> Part of the scheduling system slated for overhaul (`specs/active/scheduling-overhaul.md`).

## Summary
Removes a previously created scheduled report by name.

## Command surface
- Name: `remove-report`
- Options:
  - `schedule-name` (string, **required**, autocomplete) — name of the schedule to delete.
- Permissions: none declared.

### Autocomplete
- Requires guild setup.
- Lists the guild's job names from `cronjobs`; if none, a single placeholder choice
  "There are no scheduled reports for this guild." with value `-`.
- Filter is **case-sensitive** (`job_name.trim().contains(filter)` where `filter` is
  lowercased but `job_name` is not) — a known filtering quirk. (qa-report: L-3)

## Behavior
1. Look up guild; if not set up → ephemeral prompt.
2. Defer per `show_command_output`.
3. `remove_report_job(name, guild)`:
   - No such job → `CronError::NoJobFound` → "not in the guild database…".
   - Guild has no jobs → `CronError::JobListEmpty` → "currently has no scheduled reports".
   - If this is the guild's **only** job, delete the channel webhook.
   - Remove the scheduler entry by UUID and delete the `cronjobs` row.
4. Success → "was removed!". Other errors → generic message pointing to the support server.

## Quirks & known bugs
- **Case-sensitive autocomplete filter** vs lowercased input → filtering misses matches.
  (qa-report: L-3)
- `Webhook::from_url(...).unwrap()` / `webhook.delete(...).unwrap()` panic if the webhook was
  already removed manually. (qa-report: C-9)
- Deletes the webhook only when it's the guild's last job; if multiple guilds/jobs share a
  channel webhook this is fine, but a manually-deleted webhook still panics.

## Acceptance criteria
- `/remove-report schedule-name:<name>` stops future posts and deletes the DB row.
- Removing the last schedule in a guild also cleans up the webhook.
- A non-existent name yields the "not in the guild database" message, not a crash.
