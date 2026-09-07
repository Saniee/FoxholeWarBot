# Context: player-reported faction intel

Spec: none yet — pre-spec design brainstorm, not written up. Tasks: `tasks.md`.

## Why this exists

Competitor research proposed "intel/map-warning alerts" and "weather per hex." Both checked
against the live War API, `clapfoot/warapi`'s full commit history, and the official wiki (which
states outright the API's intent is to exclude anything not known to both factions) — neither
field has ever existed, anywhere, official or unofficial. Dead end, confirmed, not reopened. See
`specs/active/war-summaries.md`'s "Provenance" section for the same investigation applied to that
separate, finished feature.

Since there's no real intel feed, the idea became: crowdsource one from players instead.

## Current state

Pure brainstorm, now fairly deep. No spec, no migration, no code yet. Decisions so far:

**Shape of the shared state**
- Per-shard-per-faction, not per-guild — first cross-guild shared state this bot would have. The
  faction boundary is load-bearing, not incidental: a faction that reports diligently ends up with
  a genuinely better picture of the war than one that doesn't, and that edge *is* the incentive to
  participate. See the Weather entry in the category table for where this almost got quietly
  broken by sharing one category globally.
- Faction verification is organizational, not cryptographic (nothing exposes per-account faction):
  a guild declares its faction once, only a designated role within that guild can submit.
  Containment over prevention — source-guild tag on every report (visible), a per-guild kill
  switch for the bot owner, crowdsourced confirm/deny so a lone bad report gets buried.
- Retention: wipe a shard's whole report set on `War.warId` change (already polled, free), on top
  of per-category TTL decay. No default archival export — would quietly break the "ephemeral, gone
  at war end" privacy story.
- Moderation: curated slur/hate-speech wordlist (not general profanity), reject-with-message,
  repeat abuse is the guild's own role-gating to fix, not bot-side bans.

**Reporting UX**
- Ruled out parsing plain chat messages — bot only holds the `GUILDS` intent, no message content;
  adding that is a privileged-intent process and cuts against the slash-commands-only design.
- Settled: a slash command carrying just the hex (reuses `autocomplete_map` unchanged) → modal/
  component for the rest, **plus** a "🛡️ Report sighting here" button on existing map-render
  messages (`/get-map`/`/full-map`/scheduled reports) that already know their hex from context.
  The button needs a stable `custom_id` scheme since scheduled-report messages get edited in place
  post-creation (`specs/schedule-report.md`'s placeholder-then-edit pattern).

**Location precision**
- Confirmed Foxhole's real in-game grid system (`G9k3`: lettered/numbered "big square" + 3×3
  numeric "keypad") and that `Ctrl+LMB` on the map already copies it to the clipboard — existing
  player habit, nothing new to teach.
- Grid string via modal text field → parsed to the region's normalized `(x, y)` → fed through
  `request_processing.rs::place_image_info`'s existing `place()` path unchanged, same as a
  `MapItem`. One conversion formula for all 53 regions (`WorldExtent` is one fixed constant).
- **Still unverified, blocking the actual formula**: the grid's lettering/numbering range per hex,
  and the exact string `Ctrl+LMB` puts on the clipboard (bare token or more). **Deferred to local
  testing** — the user will check both in-game rather than this being researched further here.
- Nudging players toward the string: teach-at-point-of-failure on a bad parse (primary),
  label/placeholder text on the field, a tolerant parser that extracts the token from whatever got
  pasted (most important of the three). `/report-help` only if those aren't enough.

**Database schema** (drafted, not migrated)
```sql
-- guilds: faction declaration, reporting gate, opt-in state, delivery target
ALTER TABLE guilds
    ADD COLUMN faction TEXT CHECK (faction IN ('COLONIALS', 'WARDENS')),
    ADD COLUMN intel_reporter_role_id BIGINT,
    ADD COLUMN intel_webhook_url TEXT,
    ADD COLUMN intel_enabled BOOLEAN NOT NULL DEFAULT FALSE,
    ADD COLUMN intel_prompt_state TEXT NOT NULL DEFAULT 'unprompted'
        CHECK (intel_prompt_state IN ('unprompted', 'declined', 'acted')),
    ADD COLUMN intel_reporting_banned BOOLEAN NOT NULL DEFAULT FALSE;  -- owner kill switch

-- the shared table itself
CREATE TABLE faction_reports (
    id           BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    shard_name   TEXT NOT NULL,
    faction      TEXT NOT NULL CHECK (faction IN ('COLONIALS', 'WARDENS')),
    war_id       TEXT NOT NULL,              -- rollover-wipe key
    region       TEXT NOT NULL,              -- hex id string, same form as cronjobs.map_name
    category     TEXT NOT NULL,              -- free text; TTL/cooldown/cap table lives in Rust
    grid_ref     TEXT,
    x            REAL,
    y            REAL,
    note         TEXT,
    source_guild BIGINT NOT NULL REFERENCES guilds(id) ON DELETE CASCADE,
    reported_by  BIGINT NOT NULL,            -- never rendered in the posted embed, see below
    created_at   TIMESTAMPTZ NOT NULL DEFAULT now(),
    expires_at   TIMESTAMPTZ NOT NULL
);
CREATE INDEX ON faction_reports (shard_name, faction, region, category, expires_at);

-- one row per voter per report; cascades with the report so a voter's identity
-- never outlives the report it's attached to
CREATE TABLE faction_report_votes (
    report_id BIGINT NOT NULL REFERENCES faction_reports(id) ON DELETE CASCADE,
    voter     BIGINT NOT NULL,
    vote      TEXT NOT NULL CHECK (vote IN ('confirm', 'deny')),
    voted_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (report_id, voter)
);

-- rollover detector; War isn't cached on disk today (architecture.md's cache list
-- is map_choices/dynamic/static/war_reports, not War), so this is new state either way
CREATE TABLE shard_war_state (
    shard_name TEXT NOT NULL PRIMARY KEY,
    war_id     TEXT NOT NULL
);
```
No `confirms`/`denies` counter columns — deliberately dropped after drafting them once. They'd be
a second source of truth that can drift from `faction_report_votes`; `COUNT(*) ... WHERE vote = …`
at read time is cheap at this volume and has only one source of truth.

**Category table** (Rust-side, not DB — same "config in code" precedent as `schedule::Frequency`):

| Category | TTL | Cooldown (per hex+faction) | Active cap (per hex+faction) |
|---|---|---|---|
| Enemy armor sighted | ~15 min | none | none |
| Enemy infantry push | ~20 min | none | none |
| Structure under attack | ~30 min | none | none |
| Supply stockpile low | ~2 h | none | none |
| Logi / escort requested | ~1 h | none | none |
| **Frontline** | **36 h** — confirmed | **3 h** | **3** |
| **Weather** | **36 h** — confirmed | **3 h** | **1** |
| Other | ~30 min | none | none |

At the Frontline cap, a new submission (past cooldown) **evicts the oldest active** marker for
that hex+faction rather than being rejected — confirmed. Weather uses the identical mechanism with
`max_active = 1`, so a new report always replaces the sole existing one ("overwritten on a new one
added" is just the cap=1 case of the same eviction rule, nothing new to build). The cooldown/cap
filter is `(shard_name, faction, region, category)`, deliberately *not* per-guild — a shared
signal, not one-per-submitter, so five different guilds can't bypass the cooldown by taking turns.

**Weather staying per-faction is deliberate, not a gap** — corrected after initially being framed
as a quirk to tolerate. Weather is the same physical fact for both sides, so sharing it globally
would be the one category that leaks across the faction line for free. The whole point of the
system is that a faction that reports diligently ends up with a genuinely better picture of the
war than one that doesn't — that's the actual incentive to participate, not a side effect — and a
globally-shared category would quietly remove that edge for itself while leaving it intact
everywhere else. Per-faction siloing isn't the cost of one uniform rule; it's the uniform rule
doing exactly what it's for. No sourced duration number exists for real storm length — the wiki
states storms vary in duration with nothing published (checked the raw wikitext directly, not just
a search summary, after an AI search summary claimed a number that does not actually appear on the
page — same conflation failure mode as the original `mapWarnings` research). 3h/36h here is the
same shape as Frontline, not a number grounded in observed storm duration the way Frontline was
grounded in the real arty-grid system.

**Delivery and voting**
- Fan-out via per-guild webhook, same `resolve_webhook`-or-create pattern `/schedule-report`
  already uses, to every opted-in guild on that shard+faction (including the submitter's own
  guild — one uniform feed, no special-casing).
- Two buttons on the posted embed, Confirm/Deny, `custom_id` carrying the report id (the row is
  shared; the Discord message is one of N mirrors of it). **Voting is open to anyone who can see
  it in an opted-in guild's channel — not gated to the reporter role**; submission is narrow and
  trusted, correction should be broad. Submitter can't vote on their own report.
- **Deliberately not live-updating the tally across every guild's copy on every vote** — that's an
  edit fan-out per vote across N guilds, and Discord messages aren't meant to be continuously
  refreshed dashboards. Votes affect what a *future* read acts on (a heavily-denied report drops
  out of whatever "current intel" view exists later), never the historical post.
- **The posted embed never shows who reported it** — no username, no mention. `reported_by` and
  `voter` stay internal (anti-self-vote check, abuse tracing), never rendered. This matters because
  the DB row's retention can delete the *row* but can't reach back and delete N already-posted
  Discord messages across N guilds' channels — keeping identity out of the embed means that gap
  costs nothing. Expired/wiped reports' old messages are left alone in channel scrollback,
  deliberately — chasing deletes across every guild's channel on every expiry is a lot of API calls
  for information that's already harmless once stale.

**Opt-in flow**
- "Soft opt-out" in spirit: the bot nudges proactively, but declining costs one click and is
  honored permanently (no re-nudging). Trigger: appended once to a guild's `/set-guild-settings`
  confirmation, the first time a guild with `intel_prompt_state = 'unprompted'` runs it — no
  unsolicited channel post, no DM, goes to the admin already taking an action.
  Two buttons: "Enable faction intel sharing" / "No thanks".
- **"No thanks" only stops the automatic nudge** — it does not lock the door. A guild can always
  come back via **`/set-faction-intel`**, a new Administrator-gated command (same tier as
  `/set-guild-settings` — this is a bigger call than the Manage-Webhooks tier `/schedule-report`
  sits at) that opens the *same* 3-step flow on demand: faction (buttons) → reporter role
  (role-select) → destination channel (channel-select, same resolve-or-create webhook pattern).
  The nudge's "Enable" button and the manual command converge on one handler, not two.
- `intel_prompt_state` flips to `'acted'` the moment a guild decides through *either* door — the
  automatic nudge never fires again regardless of outcome. `intel_enabled` is a separate field so
  disabling later doesn't erase the remembered faction/role/channel.
- **Confirmed**: a "Reconfigure" / "Disable" first screen shown only when `intel_enabled` is
  already true, before the 3-step flow — "Reconfigure" proceeds into the normal 3 steps,
  "Disable" flips `intel_enabled` off immediately. Skipped entirely for a guild that's never been
  enabled. Going in as designed; revisit based on how real users respond to it.

## Open threads — not reached yet

- Free-text note cap: **confirmed at 200 characters.**
- The grid→normalized-coordinate math — blocked on the user's own local/in-game testing (grid
  dimensions, exact clipboard string), not further research here. Come back to this once that
  testing happens.
- Per-category/per-region filtering on the delivery channel (a logi-only server not wanting
  "armor sighted") — floated, not decided, probably fine to skip for v1.
- Whether this ships as one spec or several (command surface / data model / moderation / rendering
  could each stand alone — still undecided).

## Next steps

Interaction design and schema are now fully settled, including the Reconfigure/Disable first
screen (going in as designed, to be judged on how real users respond rather than argued further
now). What's left: the filtering question (probably skip for v1), the user's own local
verification of the grid system (dimensions + clipboard format), and then this is ready to write
up as an actual spec (or several — still undecided which) instead of staying a brainstorm.
