# Gating: scheduled full-map renders (ACTIVE)

Status: **proposed.** Primary mechanism (approval form) is ToS-clean and buildable now; any
**paid/donation** tier is deferred pending Siege Camp confirmation (see ToS findings).

## Gating model (locked)
The **full-map renderer** (all 53 hex regions stitched together — see
`specs/active/full-map-renderer.md`, built on `specs/rendering-placement.md`) is gated
**whenever it is scheduled** — for every guild, regardless of size:

| Path | Gated? |
|---|---|
| On-demand full-map render (one-off "dry run") | **Free for everyone** |
| Scheduled full-map report — **any** guild | **Requires approval** (form) |
| Single-hex commands & scheduled *hex* reports | **Free, unchanged** |

Rationale: the on-demand render costs the user one explicit request; a *scheduled* full render
costs the bot's infra repeatedly and forever, and that is true of a 5-member guild as much as a
500-member one — 53 regions fetched and composited on a timer is the same load either way. The
gate is a lightweight approval so the owner knows what recurring work has been signed up for —
**not** a paywall.

### No size exemption
There is **no free tier by member count.** An earlier draft let guilds under a
`FULL_MAP_FREE_MEMBER_THRESHOLD` (50) schedule full maps without asking; that is gone. Every
scheduled full-map report is approved individually, so total recurring load stays a number the
owner has actually seen rather than an emergent property of how many small guilds found the
feature.

Member count is still **recorded** — it's useful context when reviewing a request — but it is
**not** an entitlement input. Nothing decides anything from it. Because of that it's read from
the guild at request time and snapshotted onto the request row; it does **not** need to be
cached on `guilds` or kept fresh (which also means the gate doesn't care that no guild row
exists until `/set-guild-settings` runs).

## Why not a paywall / vote-wall (ToS findings)
Searched the actual terms (2026-07):
- **War API** ([clapfoot/warapi](https://github.com/clapfoot/warapi),
  [War API v1](https://clapfoot.github.io/warapi/)) states exactly one rule — *"respect the
  caching headers as returned by the API"* + use ETags. **No license, and no statement for or
  against commercial use.** Legally silent, not a green light.
- **Assets** (the map `*.TGA` and icon PNGs the full render composites) are **Siege Camp's
  copyrighted artwork.** The [Foxhole EULA](https://store.steampowered.com/eula/505460_eula_1)
  reserves all IP; community precedent ([foxholetools/assets](https://github.com/foxholetools/assets))
  distributes assets only under *"fair use for educational and research purposes,"* and resource
  packs state assets **cannot be used in files sold for money**. Embraced fan tools (e.g. the
  [Facility Planner](https://github.com/brandon-ray/foxhole-facility-planner)) are all **free**.

**Conclusion:** no concrete rule bans monetizing an API tool, but the weight of evidence leans
against putting their copyrighted map art behind a paid tier. Therefore:
- **Primary gate = a non-monetary approval form.** Rations infra cost without selling anything,
  so the commercial-use question never arises. ToS-clean.
- **Any money = optional hosting donations only**, framed as supporting the bot's server costs
  (not selling the map), and **only after** posting in the official Foxhole `code-talk` Discord
  to get Siege Camp's blessing on record. Until then, no paid/donation tier ships.

## The approval form (full-map schedule request)

### Command surface
The form is driven by commands on both sides (a Discord modal can only be opened from an
interaction, so a command has to trigger it regardless):

- **Applicant:** `/request-full-map-schedule` — opens the application modal and records a pending
  request. `/schedule-report`, when a guild targets the full map without approval, does **not**
  hard-refuse: it points the user at this command (and can open the modal itself). Available to
  members who
  can create schedules (same permission gate as `/schedule-report`, per the scheduling overhaul).
- **Reviewer (bot owner / designated admin):** `/full-map-requests` with subcommands:
  - `list` — show pending requests (id, guild, size, cadence, use case).
  - `approve <id>` — set the guild's `full_map_approved`, mark the request approved, notify the
    requester; the guild can then create the schedule.
  - `deny <id> [reason]` — mark denied, notify the requester with the reason.

### Dedicated requests channel (support Discord)
Every submitted request is also **posted to a dedicated channel on the bot's support Discord** —
its id comes from config, `REQUESTS_CHANNEL_ID` (see Config below). The post is an embed with the
request fields plus **Approve/Deny buttons**, so the owner reviews in one place without polling
the command. The channel is the **primary** review surface; `/full-map-requests` is the reliable
fallback/scriptable path over the same actions (both write the same `full_map_requests` rows and
`full_map_approved` flag, so they never diverge).

- Requires the bot to be in the support guild and able to post there; if `REQUESTS_CHANNEL_ID` is
  unset or unreachable, fall back to command-only review and log a warning (don't drop the
  request).
- Button interactions are reviewer-gated so only reviewers can approve/deny. **A reviewer is a
  user id in `REVIEWER_IDS` and nothing else** — not the application owner unless they listed
  themselves, and never "an administrator of the server this was used in". An admin check would
  have meant any administrator of any server that added the bot could `/full-map-requests list`
  and read every other server's form answers.

### Flow
1. Any guild wanting a scheduled full map → `/request-full-map-schedule` → modal → a `pending`
   row in `full_map_requests`.
2. The request is posted to `REQUESTS_CHANNEL_ID` (embed + Approve/Deny buttons) and is visible
   via `/full-map-requests list`.
3. **Approve** (button or command) → `full_map_approved = true`, requester notified, guild creates
   the schedule. **Deny** → requester notified with reason; no schedule.
4. The gate is re-checked at tick time (below), so approval can be revoked later.

Form delivery (pick at build time):
- **In-Discord modal** (poise/serenity modal, opened by the applicant command) — lowest friction,
  data flows straight into the requests table. Recommended.
- **External form** (e.g. a linked web form) — simpler to host, but needs a manual/webhook step to
  get the answers back to the bot. Fallback.

### Form fields
Auto-filled by the bot (not asked):
- Guild name + guild ID
- Guild member count — **review context only**, snapshotted at request time; it grants nothing
- Requesting user (ID + guild roles, to confirm they can speak for the guild)

Asked of the requester:
1. **Map target & cadence** — confirm "full map" and the requested schedule (e.g. "every 2h",
   "daily 18:00 UTC").
2. **Report channel** — where it will post.
3. **Use case** — one line on why a recurring full-map is needed (e.g. regiment ops planning).
4. **Expected audience** — rough size/who sees it.
5. **Contact** — optional Discord handle for follow-up.
6. **Acknowledgement** (required checkbox/confirm):
   - Understands this is a free community tool with no uptime guarantee.
   - Understands renders may be rate-limited or paused to protect infra.
   - Understands approval can be revoked.

### Taking a request back
Every state a request can leave is reachable from a button, not only from a typed command.
`/full-map-requests` stays as the fallback surface, but it is the fallback:

- **Withdraw** — the applicant's, on the ephemeral reply to `/request-full-map-schedule` (both
  when the request is filed and when the command finds one already open). A pending request only.
  Gated on being the person who filed it, in the guild that filed it, or a reviewer. Nothing is
  announced: they did it themselves.
- **Revoke approval** — the reviewer's, on the request post once it reads `approved`. Withdraws
  the guild's approval *and* closes the request in one transaction, so the flag and the record
  can never disagree. Not announced either — the dormancy notice already says it, in the channel
  the reports actually go to.

Both land on status `withdrawn`, which is right: neither is a refusal, and `denied` would record
a judgement nobody made. `withdrawn` also puts the row back under the 90-day purge.

Decided posts (denied, withdrawn) carry no buttons at all. They are a record, and a live button
on a settled decision is how one gets overturned by a misclick.

## Config
- `REQUESTS_CHANNEL_ID` — channel id on the support Discord where full-map requests are posted
  for review. Optional: unset ⇒ command-only review (`/full-map-requests`). Read via `dotenv::var`
  like the other ids.
- `REVIEWER_IDS` — Discord user ids allowed to decide requests. One id, or several separated by
  commas. **The complete list: the application's owner is _not_ added automatically and must
  appear in it like anyone else.** Empty means nobody can review, warned about at startup.

  **No implicit owner, deliberately.** This bot is open source and self-hosted, so "the owner can
  always approve" reads clearly to the person who wrote it and misleadingly to everyone deploying
  it — it invites a guess about whose account that is, and on a team application the Discord
  owner may be nobody who runs the deployment. One variable listing every reviewer by id cannot be
  misread. The cost is that forgetting to set it means nobody can review; the startup warning is
  what pays that back.

  **In `.env` rather than in the database, deliberately.** Reviewing means reading other servers'
  free-text answers and granting recurring load on the host, so the list of people who may do it
  belongs to whoever runs the host — not to a table editable from inside Discord, where a
  compromised account with the right command could add itself. It also means the bot stores no
  record of who its reviewers are, which keeps the `docs/` stored-data list accurate without a
  word of change. The cost is a restart to change the list, which for a list that changes about
  once a year is the right trade.

## Data model (folds into `specs/postgres.md`)
```sql
-- guilds: approval state only. No cached member_count column: nothing decides
-- anything from member count any more, so there is nothing to keep fresh.
ALTER TABLE guilds ADD COLUMN full_map_approved    BOOLEAN NOT NULL DEFAULT FALSE;
ALTER TABLE guilds ADD COLUMN full_map_approved_at TIMESTAMPTZ;

-- pending/approved/denied application queue
CREATE TABLE full_map_requests (
    id            BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    guild         BIGINT NOT NULL REFERENCES guilds(id) ON DELETE CASCADE,
    requested_by  BIGINT NOT NULL,              -- Discord user id
    member_count  INTEGER,                      -- snapshot, review context only
    map_target    TEXT   NOT NULL,              -- "full-map"
    cadence       TEXT   NOT NULL,              -- requested schedule phrase
    channel_id    BIGINT NOT NULL,
    use_case      TEXT,
    audience      TEXT,
    contact       TEXT,
    status        TEXT   NOT NULL DEFAULT 'pending',  -- pending | approved | denied
    reviewed_by   BIGINT,
    reviewed_at   TIMESTAMPTZ,
    created_at    TIMESTAMPTZ NOT NULL DEFAULT now()
);
```
(Donation/expiry columns like `premium_until` are intentionally omitted until a paid tier is
cleared with Siege Camp.)

## Entitlement check (swappable, both points)
Keep the source behind a trait so a future donation/sponsor mechanism can replace the flag
without touching command logic:
```rust
#[async_trait]
trait FullMapScheduling {
    /// May this guild run a *scheduled* full-map report right now?
    async fn is_allowed(&self, guild: &GuildData) -> bool;
}
// Default impl: guild.full_map_approved. That's the whole rule — no size branch,
// no threshold constant, nothing to configure.
```
1. **At schedule creation** (`/schedule-report`, full-map target, not approved) → start the
   application flow instead of scheduling.
2. **At tick time** (ties into `specs/scheduling.md`, `is_full_map` job marker):
   re-check `is_allowed`. If approval was revoked, the job goes **dormant** (skips rendering,
   posts a one-time heads-up) rather than silently burning infra. Re-approval reactivates it.

The trait stays even though its default impl is now a single field read: it's the seam that keeps
a future entitlement source (sponsor role, donation tier) out of the command bodies.

## Decisions (settled)
- **No size exemption.** Every scheduled full-map report needs approval, whatever the guild's
  size. Supersedes the earlier `FULL_MAP_FREE_MEMBER_THRESHOLD = 50` free tier, which is gone;
  member count is recorded for the reviewer and grants nothing.
- **Form delivery: in-Discord modal.** Lowest friction, and answers land straight in
  `full_map_requests` with no external hosting or webhook round-trip. No web form.
- **Review surface: both.** The `REQUESTS_CHANNEL_ID` channel post with Approve/Deny buttons is
  primary; `/full-map-requests list|approve|deny` is the fallback/scriptable path. Both drive the
  same rows and flag.
- **Command names:** `/request-full-map-schedule` and `/full-map-requests` as specced.
- **Reviewers are exactly the `REVIEWER_IDS` allow-list.** No implicit owner, and guild
  administrator grants nothing. See Config for why.
- **On-demand render:** a dedicated `/full-map` command (see
  `specs/active/full-map-renderer.md` → Decisions), free for everyone and ungated.
- **Monetization: none.** The form gate is the only mechanism. No paid tier, no vote-wall, no
  donation prompt ships with this feature. If hosting donations are ever added, they stay separate
  from the gate and follow a `code-talk` post to Siege Camp first (**recommended regardless**, as
  a courtesy heads-up about the tool).

## Docs impact (carried over from the docs overhaul)
The docs site was brought in line with the shipped schema ahead of this feature
(`specs/docs-site.md`), which deliberately left out everything below — documenting data the bot
doesn't yet store would have broken that spec's central invariant. Ship these **in the same
commit** as the schema change:

- **`tos.md` / `privacy.md` — extend the stored-data list** with the new `guilds` columns
  (approval flag and its timestamp) and the `full_map_requests` row: requesting user's Discord ID,
  guild ID / name, a member-count snapshot, requested cadence and channel, and the free-text
  answers.
  The free-text answers are the first genuinely *new* data the bot stores, and the first data
  tied to an individual user rather than a server — say so plainly.
- **Retention.** Guild rows and schedules cascade on leave; request rows are kept for review
  history. Pick and state a window — suggested: purge denied/withdrawn requests after 90 days.
- **A new docs section for the form**, in user-facing terms: the full map is free to render on
  demand for everyone; *scheduling* one requires a short request, from every server regardless of
  size, because each scheduled render is a recurring cost on the host; answers are reviewed by the
  bot owner in the support server; approval may be revoked, which makes the schedule dormant
  rather than deleted. **No payment is involved at any point** — state it plainly and early, since
  "apply for access" is exactly the shape users expect a paywall to take, and a gate with no free
  tier at all invites that reading more than the old one did.

## Acceptance criteria
- On-demand full-map render works for everyone, with no request and no approval.
- **No guild, of any size, can create a scheduled full-map report without an approved request.**
- A guild scheduling a full-map report is routed into the application flow; the schedule is
  created only after approval.
- A tick for a guild whose approval was revoked goes dormant (not deleted) and resumes on
  re-approval.
- Member count appears in the request embed and nowhere in any decision path.
- No money changes hands anywhere in the shipped feature; single-hex commands and hex scheduling
  are unaffected.
