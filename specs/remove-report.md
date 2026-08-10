# /remove-report

## Summary
Removes a previously created scheduled report by name.

## Command surface
- Name: `remove-report`, guild-only.
- Options:
  - `schedule_name` (string, **required**, autocomplete) — the schedule to delete.
- Permissions: **`MANAGE_WEBHOOKS`**, the same gate as creating one.

### Autocomplete
- Lists this guild's job names from `cronjobs`; if none, a single placeholder choice
  ("This server has no scheduled reports.", value `"-"` — Discord rejects an empty value).
- Filter lowercases **both sides**, so typing "daily" matches a schedule named "Daily".

## Behavior
1. Look up guild; if not set up → ephemeral prompt.
2. Defer per `show_command_output`.
3. Look the job up by `(guild, name)`. No match → reply saying so; not an error.
4. Delete the `cronjobs` row first. If that fails, the scheduler entry and webhook remain usable
   and startup restoration still has a consistent row.
5. Remove the scheduler entry by its stored UUID. An id the scheduler no longer recognizes is
   fine — the job is gone either way.
6. Delete the webhook **only if no other schedule in this guild uses that URL**. Two schedules
   posting to the same channel share one webhook, so deleting on "is this the guild's only
   job?" — the old test — would break the survivor.
7. Confirm the removal.

A webhook someone already deleted by hand is logged and treated as success: that's the state
we wanted.

## External calls
- Discord: resolve and delete the channel webhook (conditionally).
- DB: `cronjobs` read + delete.

## Acceptance criteria
- `/remove-report schedule_name:<name>` stops future posts and deletes the DB row.
- Removing the last schedule using a webhook also cleans up that webhook; removing one of two
  schedules that share it does **not**.
- A manually-deleted webhook does not cause a failure.
- A non-existent name yields a clear message, not a crash.
- A member without Manage Webhooks cannot use the command.
