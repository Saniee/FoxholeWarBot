# War summaries — `/war-summary` and `/schedule-summary`

**Status: spec only, nothing in the code yet.**

## Summary
A compact "what's happening in the war" digest — day of war, total casualties, conquest
progress, and the most-fought-over regions — as a one-shot command and as a recurring scheduled
post, distinct from the map render. `/get-map` and `/full-map` answer "where things are";
this answers "what's happened since the war started."

## Provenance, and what this deliberately excludes
Sourced from a round of competitor research that also proposed "intel/map-warning alerts" and
"weather per hex." Both were checked against the live API and the full commit history of
`clapfoot/warapi`'s README — neither a `mapWarnings` field nor a weather-data field has ever
existed. "Weather Station (83)" is a buildable player structure, not a data feed; `mapWarnings`
never appears in the schema at all, under any name, in any revision. Those two are dropped
entirely, not reworked — there's no real data source behind them. This spec only uses fields
the bot already fetches and models: `War` and `WarReport` (`specs/architecture.md` → Foxhole War
API endpoints).

Player counts (Steam's `GetNumberOfCurrentPlayers`) were floated in the same research as part of
the digest. Left out of this spec on request — cosmetic for now, a separate feature if picked up
later, and not required for anything below.

## What's actually available, and what it costs
- **`GET /worldconquest/war`** — one fetch per shard per summary. `winner`, `conquestStartTime`,
  `resistanceStartTime`, `requiredVictoryTowns`, `shortRequiredVictoryTowns`. No per-region cost.
- **`GET /worldconquest/warReport/{map}`** — one fetch per *region*. `totalEnlistments`,
  `colonialCasualties`, `wardenCasualties`, `dayOfWar`, `version`. Already cached on disk with
  ETag revalidation (`cache/war_reports/Report_<map>-<Shard>.json`) and already fetched in bulk
  for other features — this is the same fan-out shape `/full-map` already does across the shard's
  region list, just against a much smaller response than a map render.
- **`dayOfWar` is shard-global, not per-region** — confirmed in `specs/active/frontline-activity.md`
  (two regions of the same war, `dayOfWar` identical, everything else differing). One region's
  report is enough to read it; no need to agree across all of them.
- **Casualties and enlistments are cumulative for the war, not a daily figure.** The same caveat
  `specs/active/frontline-activity.md` already worked through for the tint-intensity idea applies
  here unchanged: there is no "today's casualties" without storing history and differencing two
  polls, which is a migration, a retention policy, and a `docs/tos.md` + `docs/privacy.md` change
  in the same commit. **Out of scope for this spec.** The summary reports cumulative totals
  plainly labelled "since the war began," not "today" — the wrong label here is exactly the kind
  of misleading number the tint spec was written to avoid.
- **There is no API field for which faction holds which victory town.** `requiredVictoryTowns`
  and `shortRequiredVictoryTowns` are target counts, not a list of towns with an owner. A faction's
  current *held* VT count would need per-region dynamic map data, filtered for map items carrying
  the `IsVictoryBase` flag, read for `teamId`, across every region — a second full fan-out per
  summary (dynamic data, not just war reports). **Left out of v1** for the same reason the quick-wins
  research filed this whole feature under "low lift, no new data layer": v1 uses only what
  `WarReport` and `War` already hand back. Revisit as a P1 addition once v1 is live and the extra
  fan-out's cost is worth measuring against actual use.

## "Hottest regions" — what it means here
Ranked by cumulative casualties (`colonialCasualties + wardenCasualties`) per region, highest
first, same reasoning `specs/active/frontline-activity.md` already worked out when it compared
casualties against enlistments as a signal: casualties carry roughly 5x the spread between a
contested hex and a quiet one, enlistments barely any. **This is the same cumulative-totals
caveat as above** — a region fought hard for a week and quiet since stays near the top of this
list forever, because there's no rate, only a running total. The embed says "most casualties
this war," never "most active right now," so the number means what it says.

## Command surface

### `/war-summary` — one-shot
- Guild-only, no extra permission gate (matches `/war-report`, `/war-state`: public information,
  no side effects).
- No options. Reads the caller's guild shard, same as every other one-shot command.
- Output visibility: `guilds.show_command_output`, same as every other data command
  (`specs/architecture.md` → Output visibility).

### `/schedule-summary` — recurring
- Guild-only, `default_member_permissions = "MANAGE_WEBHOOKS"` — same gate as
  `/schedule-report`, for the same reason: it creates and deletes webhooks, so that's the
  permission that matches what it does.
- Options, mirroring `/schedule-report` exactly except for the map target it drops:
  - `schedule_name` (string) — unique **within this server**. Shares the `cronjobs` table and its
    `UNIQUE (guild, job_name)` constraint with map-report schedules — a guild can't name a summary
    schedule the same as a map-report schedule, which is consistent with the existing rule that
    names are per-guild, not per-feature.
  - `report_channel` (channel).
  - `frequency` (choice) — the same `Frequency` enum and cadence generator `/schedule-report`
    uses (`specs/schedule-input.md`). A digest has no per-tick rendering cost like a 53-region
    composite, so there's no case for a *looser* floor than the general one — keep the existing
    30-minute minimum, not a new, separate number to maintain.
  - `at_time`, `timezone`, `day`, `custom` — identical to `/schedule-report`.
  - No `map_name`, no `draw_text` — nothing to render.
- Everything about *how* the schedule is created — next-three-fires preview before saving,
  schedule-first-persist-second ordering, duplicate-name rejection, webhook reuse-or-create — is
  unchanged from `specs/schedule-report.md` steps 5–11. Only the thing being scheduled differs.

### `/remove-report` — reused, unchanged
Already looks up a job by `(guild, job_name)` and deletes by a type-agnostic row
(`get_job_entry`/`remove_job_entry`/`unschedule`/webhook-cleanup-if-unused), none of which inspects
`map_name` or anything specific to a map report. A summary schedule's row satisfies the same shape,
so it is removable through the existing command and autocomplete with no code change — confirmed
by reading `src/commands/remove_report.rs`, which never branches on what kind of job it's deleting.

## Behavior

### Building a summary (shared by both commands)
1. Resolve the guild's shard.
2. Fetch `War` for the shard (one call).
3. Fetch `WarReport` for every region in the shard's cached map list (`cache/map_choices/`), same
   fan-out `/full-map` already performs — ETag-revalidated, so an unchanged region costs a `304`
   and a cache read, not a full response.
4. Sum `colonialCasualties` and `wardenCasualties` across all regions for shard-wide totals. Take
   `dayOfWar` from any one successful response (they agree). Rank regions by
   `colonialCasualties + wardenCasualties`, descending, for the "top contested regions" list
   (cap at 5 — a digest, not a leaderboard for its own sake).
5. A region that fails to fetch is skipped and logged, same as the frontline overlay's neighbour
   fetches (`specs/frontline.md`) — one bad region degrades the ranking and the totals by its own
   absence, never fails the whole summary.
6. Compose one embed:
   - War number, day of war, current winner (or "undecided").
   - Victory towns required to end the war (`requiredVictoryTowns`, reduced by
     `scorchedVictoryTowns` the same way `specs/architecture.md`'s `War` notes describe — *not*
     "towns held," which isn't available; see above).
   - Shard-wide casualties, Colonial vs Warden, labelled "since the war began."
   - Top contested regions by cumulative casualties, labelled "most casualties this war," region
     display names via `utils::regions::display_name` (never raw API ids — existing convention).

### `/war-summary`
1. Guild lookup; not set up → ephemeral prompt (standard).
2. Defer per `show_command_output`.
3. Build and send the embed as above.

### `/schedule-summary` tick
1. Re-read the owning guild's settings (shard may have changed since creation).
2. Resolve the webhook.
3. **Post a placeholder**, same reasoning as the map-report tick: building the embed means a
   shard-wide fan-out of region reports, which is not instant, and the channel should see
   something was promised rather than nothing until it lands.
4. Build the embed as above; edit the placeholder into it. Same failure handling as
   `specs/schedule-report.md` step 6: edit fails → post fresh and delete the placeholder; render
   fails → the placeholder becomes a failure notice, never left reading "fetching."

### Startup restoration
Unchanged from `specs/schedule-report.md` — a `cronjobs` row restores against its `job_type`
(see Data model) the same way it already restores against its guild's shard.

## Data model

Extends `cronjobs` rather than adding a table — a summary schedule is the same shape (name,
cadence, timezone, webhook, guild) minus the two map-specific columns, which is a narrower row,
not a different one.

```sql
ALTER TABLE cronjobs
    ADD COLUMN job_type TEXT NOT NULL DEFAULT 'map_report'
        CHECK (job_type IN ('map_report', 'war_summary'));
```

- Existing rows default to `'map_report'` — no backfill logic needed, no behavior change for any
  schedule that exists today.
- `map_name` and `draw_text` stay as they are (nullable / defaulted) and are simply not read for a
  `war_summary` row, rather than adding a second nullable pair of columns for the new type. Two
  unread columns on a fraction of rows is a smaller cost than a schema that grows a new pair of
  optional columns every time a new job type is added.
- The tick executor branches once, at the top, on `job_type`: render-and-post (existing path) or
  build-summary-and-post (new path). Nothing before that branch — guild lookup, webhook
  resolution, placeholder posting, edit-or-repost on failure — needs to know which kind of job it
  is.

**`docs/tos.md` and `docs/privacy.md` need the matching update, in the same commit as the
migration** (per `AGENTS.md`'s stored-data convention): the "Scheduled reports" bullet currently
describes every stored schedule as having a region-or-world-map marker; it needs to say a schedule
is either that **or** a war summary, with nothing else new stored (no additional personal or
per-member data — same webhook URL, name, cadence, timezone shape as today).

## External calls
- Foxhole: `War` (one per summary), `WarReport` × region count (ETag-revalidated, cached).
- Discord: list/create channel webhook (schedule creation, shared with `/schedule-report`);
  execute webhook per tick; edit placeholder.
- DB: `cronjobs` insert/read (shared table, `job_type` discriminates); `guilds` read per tick.

## Acceptance criteria
- `/war-summary` produces one embed with day of war, shard-wide casualties since the war began,
  current winner, VT progress, and up to 5 top-casualty regions by display name.
- `/schedule-summary` creates a schedule through the same creation flow as `/schedule-report`
  (preview-before-save, 30-minute floor, per-guild unique name, webhook reuse), minus the map
  target and draw-text options.
- A guild can have a map-report schedule and a summary schedule with different names; reusing a
  name already taken by either kind in the same guild is rejected.
- `/remove-report` deletes a summary schedule exactly as it deletes a map-report schedule, with no
  code change beyond what already exists.
- A region that fails to fetch during a summary build is absent from the ranking and excluded from
  the totals, and does not fail the summary.
- Every number in the embed is one an engaged player could derive by hand from the public API —
  nothing about "live intel," "warnings," or "weather" appears anywhere in this feature, because
  none of that data exists.
- Existing map-report schedules are unaffected by the migration: every pre-existing `cronjobs` row
  reads as `job_type = 'map_report'` with no explicit backfill.
