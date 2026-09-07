# Faction intel — player-reported sightings

**Status: spec only, nothing in the code yet.** Companion spec: `specs/active/faction-intel-rendering.md`
(pin placement on rendered maps — split out because it depends on an open question this spec
doesn't need). Design history: `dev/active/faction-intel/` (move to `dev/done/` once this ships).

## Summary
A shared, per-shard-per-faction feed of player-submitted sightings — enemy armor, supply-low
stockpiles, the current front line, weather — crowdsourced because the Foxhole War API has none of
this data and never has (see Provenance). Opted into per guild, posted to one channel via webhook,
corrected by confirm/deny voting, and wiped at war's end.

## Provenance
Grew out of competitor research proposing "intel/map-warning alerts" and "weather per hex" as
features. Both were checked against the live War API, the full commit history of
`clapfoot/warapi`, and the official wiki — none of that data has ever existed, officially or
unofficially, in any community wrapper. See `specs/active/war-summaries.md`'s own Provenance
section for the same investigation applied to a different, already-finished feature. Since no real
feed exists, this crowdsources one instead.

## Why per-faction, not global
This is the bot's first cross-guild shared state — everything else (`specs/architecture.md`) is
strictly per-guild. The faction boundary is the point, not an implementation detail: **a faction
that reports diligently ends up with a genuinely better picture of the war than one that doesn't**,
and that edge is the actual reason to participate. A category shared across both factions would
quietly remove that edge for itself. The one category this almost happened to is Weather — a storm
is the same physical fact for both sides — and it stays per-faction anyway, for one uniform rule
rather than a carve-out (see the category table below).

## Data model

```sql
-- guilds: faction declaration, the reporting gate, opt-in state, delivery target
ALTER TABLE guilds
    ADD COLUMN faction TEXT CHECK (faction IN ('COLONIALS', 'WARDENS')),
    ADD COLUMN intel_reporter_role_id BIGINT,
    ADD COLUMN intel_webhook_url TEXT,
    ADD COLUMN intel_enabled BOOLEAN NOT NULL DEFAULT FALSE,
    ADD COLUMN intel_prompt_state TEXT NOT NULL DEFAULT 'unprompted'
        CHECK (intel_prompt_state IN ('unprompted', 'declined', 'acted')),
    ADD COLUMN intel_reporting_banned BOOLEAN NOT NULL DEFAULT FALSE;

-- the shared table: one row per sighting, visible to every opted-in guild on
-- the same (shard_name, faction)
CREATE TABLE faction_reports (
    id           BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    shard_name   TEXT        NOT NULL,
    faction      TEXT        NOT NULL CHECK (faction IN ('COLONIALS', 'WARDENS')),
    war_id       TEXT        NOT NULL,   -- War.warId at insert time; the rollover-wipe key
    region       TEXT        NOT NULL,   -- hex id string, same form as cronjobs.map_name
    category     TEXT        NOT NULL,   -- free text; the category table (below) is Rust-side
    grid_ref     TEXT,                   -- raw "G9k3" as submitted; null if landmark-based
    x            REAL,                   -- normalized coords, see faction-intel-rendering.md
    y            REAL,
    note         TEXT,                   -- optional, <= 200 chars, passed through moderation
    source_guild BIGINT      NOT NULL REFERENCES guilds(id) ON DELETE CASCADE,
    reported_by  BIGINT      NOT NULL,   -- Discord user id; never rendered, see Privacy
    created_at   TIMESTAMPTZ NOT NULL DEFAULT now(),
    expires_at   TIMESTAMPTZ NOT NULL    -- created_at + this category's TTL
);
CREATE INDEX ON faction_reports (shard_name, faction, region, category, expires_at);

-- one row per voter per report; cascades with the report so a voter's identity
-- never outlives the report it's attached to
CREATE TABLE faction_report_votes (
    report_id BIGINT      NOT NULL REFERENCES faction_reports(id) ON DELETE CASCADE,
    voter     BIGINT      NOT NULL,
    vote      TEXT        NOT NULL CHECK (vote IN ('confirm', 'deny')),
    voted_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (report_id, voter)
);

-- war-rollover detector. War isn't cached on disk today (architecture.md's cache
-- list is map_choices/dynamic/static/war_reports, not War itself), so this is new
-- state regardless of anything else here.
CREATE TABLE shard_war_state (
    shard_name TEXT NOT NULL PRIMARY KEY,
    war_id     TEXT NOT NULL
);
```

No `confirms`/`denies` counter columns on `faction_reports` — considered and dropped. They'd be a
second source of truth that can drift from `faction_report_votes`; `COUNT(*) ... WHERE vote = …` at
read time is cheap at this volume and has exactly one source of truth.

**`docs/tos.md` and `docs/privacy.md` need the matching update, in the same commit as the
migration** (`AGENTS.md`'s stored-data convention). This is a new, second class of stored personal
identifier — `reported_by`/`voter`, beyond the existing `full_map_requests.requested_by` case — but
bounded much tighter: gone with the report at its category's TTL or immediately at war end, never
the open-ended "until removed" lifetime a setting has, and never shown in the delivered embed (see
Privacy below).

## Category table (Rust-side, not the database)

Same "config in code" precedent as `schedule::Frequency` — a category added later shouldn't need a
migration, which is also why `faction_reports.category` is free `TEXT` rather than a DB `CHECK`.

| Category | TTL | Cooldown (per hex+faction) | Active cap (per hex+faction) |
|---|---|---|---|
| Enemy armor sighted | ~15 min | none | none |
| Enemy infantry push | ~20 min | none | none |
| Structure under attack | ~30 min | none | none |
| Supply stockpile low | ~2 h | none | none |
| Logi / escort requested | ~1 h | none | none |
| Frontline | 36 h | 3 h | 3 |
| Weather | 36 h | 3 h | 1 |
| Other | ~30 min | none | none |

The cooldown/cap filter is `(shard_name, faction, region, category)`, deliberately *not*
per-guild or per-reporter — a shared signal, not one-per-submitter, so several different guilds
can't bypass the cooldown by taking turns. At the cap, a new submission past cooldown **evicts the
oldest active report** in that bucket rather than being rejected (Weather's cap of 1 makes this
"a new report replaces the existing one"). No published real-world figure exists for either
Frontline's or Weather's numbers — the arty-grid system (Frontline's closest real-world anchor) and
storm duration (Weather's) were both checked directly against primary sources; storm duration has
no disclosed minimum or typical length anywhere, official or fan wiki. Both categories share one
placeholder shape rather than being independently guessed.

## Retention — two independent layers
1. **Per-category TTL**, computed once at insert from the table above, enforced by a purge job
   every 5–10 minutes (reports decay on the order of minutes to hours, not days — tighter than the
   existing daily 03:xx jobs).
2. **War-rollover wipe**, independent of TTL: a job polls `War` per shard, compares `warId` against
   `shard_war_state`; on a mismatch, one transaction updates the row and deletes every
   `faction_reports` row for that `shard_name` (cascading its votes). Covers the case a long-TTL
   category (Frontline, Weather) would otherwise survive past the war that produced it.

Deleting the row never reaches back to delete the Discord messages that already mirrored it across
N guilds' channels — see Privacy below for why that's an accepted gap, not a bug to fix.

## Command surface

### The opt-in nudge — appended to `/set-guild-settings`
The first time a guild with `intel_prompt_state = 'unprompted'` runs `/set-guild-settings`
successfully, its confirmation carries one extra follow-up: two buttons, **"Enable faction intel
sharing"** / **"No thanks"**.
- **"No thanks"** → `intel_prompt_state = 'declined'`. Never nudged again — "soft opt-out" means
  one nudge, honored permanently, not repeated pressure.
- **"Enable"** → enters the same onboarding flow `/set-faction-intel` opens manually (below).
- Either choice sets `intel_prompt_state = 'acted'` once the flow underneath it completes.

### `/set-faction-intel` — Administrator-gated
Same permission tier as `/set-guild-settings` (declaring a faction and handing out a reporting
role is a bigger call than the Manage-Webhooks tier `/schedule-report` sits at). Can be run at any
time, regardless of `intel_prompt_state` — declining the automatic nudge only stops the nudge, it
never locks this door.

1. If `intel_enabled` is already true: a first screen, two buttons — **"Reconfigure"** (continue to
   step 2) / **"Disable"** (set `intel_enabled = false` immediately, stop — faction/role/channel
   stay remembered for next time, not cleared). A guild that's never been enabled skips straight to
   step 2.
2. Faction — two buttons, Colonial / Warden.
3. Reporter role — a role-select component.
4. Destination channel — a channel-select component, same resolve-or-create webhook pattern
   `/schedule-report` already uses (`resolve_webhook` in `schedule_report.rs`), stored as
   `intel_webhook_url`.
5. Sets `intel_enabled = true`, confirms what was chosen.

### `/report-intel <hex>` — the submission path
Guild-only. Takes the hex via the existing `autocomplete_map` helper, unchanged
(`commands/common.rs`). No `default_member_permissions` gate — visibility is the same as
`/war-report`, but the handler checks, in order, before anything is shown:
1. `intel_enabled` — if false, say so and point at `/set-faction-intel`.
2. `intel_reporting_banned` — if true, a neutral refusal (no elaboration — this is the bot owner's
   kill switch, not something to explain in public).
3. The invoker holds `intel_reporter_role_id` — if not, refuse with which role is needed.

Having passed those, the flow is a **category select, then a modal** — not a single modal with
everything in it, because Discord modals support only text-input fields, not select menus; a
category picker has to happen as a separate component step before the modal that carries the free
text. So:
1. Reply with a select menu of categories (the table above, by name).
2. The select's response opens a modal: a grid-reference text field (label/placeholder teaching the
   `Ctrl+LMB` shortcut — `specs/active/faction-intel-rendering.md` has the detail) and an optional
   note field, capped at 200 characters.
3. On submit: moderate the note (slur/hate-speech wordlist — see Moderation), check the category's
   cooldown/cap (evict-oldest if at cap and past cooldown; refuse with the remaining cooldown time
   if not), insert the row, fan out.

An empty grid-reference field falls back to a landmark select (that region's cached
`MapTextItems`, capped at 25 per Discord's select limit) as one more step — only when needed, never
on the primary path.

### The report button — the shortcut path
A "🛡️ Report sighting here" button on every `/get-map`, `/full-map`, and scheduled-report message.
Clicking it already knows the hex from the message it's attached to, so it skips straight to the
category select — same handler as `/report-intel` from that point on. Needs a stable `custom_id`
scheme (`intel_report:<region>`) that keeps resolving correctly after a scheduled-report message is
edited in place post-creation (`specs/schedule-report.md`'s placeholder-then-edit pattern) — the
button is part of what gets carried into the edit, not dropped by it.

### Confirm/Deny — voting
Two buttons on every posted report embed, `custom_id` carrying the report's id
(`intel_vote:confirm:<id>` / `intel_vote:deny:<id>`) — the row is shared, the message is one of N
mirrors of it. **Open to anyone who can see it in an opted-in guild's channel, not gated to the
reporter role** — submission is narrow and trusted, correction should be broad. The submitter
cannot vote on their own report. First vote per `(report_id, voter)` sticks (the table's primary
key enforces it); a second click says "you've already voted."

## Behavior: delivery and voting
- A report that passes moderation and the cooldown/cap check fans out via **per-guild webhook**
  (same `resolve_webhook`-or-create pattern `/schedule-report` uses) to **every opted-in guild on
  that shard+faction**, including the submitting guild's own channel — one uniform feed, no
  special-casing.
- **Votes never live-edit the posted messages.** With N guilds each holding their own mirror, that
  would be an edit fan-out per vote, and Discord messages aren't meant to be continuously-refreshed
  dashboards (that's what the single-message placeholder-edit pattern in `/schedule-report` is
  for — one message, one tick). Instead, votes affect what a *future* read acts on (e.g. a
  heavily-denied report dropping out of whatever "current intel" view exists) — never the
  historical post.

## Moderation
A curated slur/hate-speech wordlist (reuse an existing open list — do not hand-roll one), *not* a
general profanity filter; this is an 18+ wargame and casual swearing is part of its culture. A
match rejects the submission with a clear message naming that the note couldn't be posted, same
refusal style as the full-map gate and every other clear-refusal path in this bot — never a silent
drop. Repeat abuse is the guild's own role-gating to fix (revoke `intel_reporter_role_id` from the
offender), not something the bot polices with user-level bans. Accepted v1 gap: a wordlist catches
neither leetspeak nor non-English slurs.

## Privacy — the embed never identifies the reporter
The posted embed shows category, location, and note — never a username, never a mention.
`reported_by` and `voter` stay internal, used only for the can't-vote-on-your-own-report check and
for tracing abuse back to a source guild. This is what makes the retention gap harmless: a DB
row's deletion (TTL or war-wipe) can't reach back and delete N already-posted Discord messages
across N guilds' channels, so anything visible in an embed would otherwise outlive the data's own
retention promise. Keeping identity out of the embed entirely means that gap costs nothing.
Expired/wiped reports' old messages are left alone in scrollback — chasing deletes across every
guild's channel on every expiry is a lot of API calls for information that's already harmless once
stale.

## Anti-abuse, beyond moderation
- Every report is tagged with its source guild — visible in the embed's footer or similar, not
  hidden, so a pattern of bad reports points at one server.
- The bot owner has a per-guild kill switch (`intel_reporting_banned`) to cut a hostile guild's
  reporting rights without touching anything else it does with the bot.
- Confirm/deny voting (above) is the main crowdsourced defense — a lone bad report gets buried by
  correction rather than trusted outright.

## External calls
- Foxhole: `War` per shard (rollover check, reused by `specs/active/war-summaries.md`'s own needs
  if that ships too — no new fetch shape).
- Discord: role-select/channel-select components; webhook create/execute per opted-in guild per
  report; button/select interactions for the reporting and voting flows.
- DB: `faction_reports`/`faction_report_votes`/`shard_war_state` read/write; `guilds` read/write
  for the new columns.

## Acceptance criteria
- A guild sees the opt-in nudge exactly once, appended to `/set-guild-settings`, and never again
  regardless of its answer.
- `/set-faction-intel` works identically whether reached from the nudge or run directly, at any
  time, regardless of prior state.
- A guild that declines the nudge can still enable later via `/set-faction-intel` — declining never
  permanently blocks it.
- Only a holder of the guild's configured reporter role can submit; anyone in an opted-in guild's
  channel can vote, except on their own report.
- A report reaches every opted-in guild on its shard+faction, including the submitter's, and none
  on the other faction.
- Frontline and Weather respect their cooldown and active cap; at cap, the oldest active report in
  that hex+faction+category is evicted, never a hard rejection.
- A report's row is gone by its category's TTL or immediately on `War.warId` changing for its
  shard — whichever comes first.
- No posted report embed ever displays who submitted it.
- A note containing flagged language is rejected with a clear message, never silently dropped and
  never silently posted.
- A voter can vote on a given report exactly once; a resubmitted vote is refused, not double
  counted.
