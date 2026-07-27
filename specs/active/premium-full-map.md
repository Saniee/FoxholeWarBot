# Gating: scheduled full-map renders (ACTIVE)

Status: **proposed.** Primary mechanism (approval form) is ToS-clean and buildable now; any
**paid/donation** tier is deferred pending Siege Camp confirmation (see ToS findings).

## Gating model (locked)
The **full-map renderer** (all 53 hex regions stitched together — see
`specs/active/full-map-renderer.md`, built on `specs/rendering-placement.md`) is gated
**only when scheduled**, and even then only for **larger guilds**:

| Path | Gated? |
|---|---|
| On-demand full-map render (one-off "dry run") | **Free for everyone** |
| Scheduled full-map report — small guild (< threshold) | **Free** |
| Scheduled full-map report — large guild (≥ threshold) | **Requires approval** (form) |
| Single-hex commands & scheduled *hex* reports | **Free, unchanged** |

Rationale: the on-demand render costs the user one explicit request; a *scheduled* full render
costs the bot's infra repeatedly and forever. Large guilds concentrate that recurring cost and
reach, so they go through a lightweight approval gate — **not** a paywall.

### "Larger guild" threshold
- A configurable member-count threshold, `FULL_MAP_FREE_MEMBER_THRESHOLD` = **50** (Foxhole
  guilds above ~200 are rare, so a higher value would gate almost nobody; 50 is where the gate
  actually bites — tune down further if needed). Below it: schedule full-map reports freely.
  At/above it: approval required.
- Member count comes from the gateway `guild_create` payload (`Guild.member_count`, available
  under the `GUILDS` intent); store it on the `guilds` row and refresh on `guild_create`/updates
  so the check works at command time without an extra fetch.

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
  request. `/schedule-report`, when a large guild targets the full map, does **not** hard-refuse:
  it points the user at this command (and can open the modal itself). Available to members who
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
- Button interactions are owner/admin-gated so only reviewers can approve/deny.

### Flow
1. Large guild → `/request-full-map-schedule` → modal → a `pending` row in `full_map_requests`.
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
- Guild member count (for the threshold record)
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

## Config
- `REQUESTS_CHANNEL_ID` — channel id on the support Discord where full-map requests are posted
  for review. Optional: unset ⇒ command-only review (`/full-map-requests`). Read via `dotenv::var`
  like the other ids.

## Data model (folds into `specs/postgres.md`)
```sql
-- guilds: approval state + cached size for the threshold
ALTER TABLE guilds ADD COLUMN member_count       INTEGER NOT NULL DEFAULT 0;
ALTER TABLE guilds ADD COLUMN full_map_approved   BOOLEAN NOT NULL DEFAULT FALSE;
ALTER TABLE guilds ADD COLUMN full_map_approved_at TIMESTAMPTZ;

-- pending/approved/denied application queue
CREATE TABLE full_map_requests (
    id            BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    guild         BIGINT NOT NULL REFERENCES guilds(id) ON DELETE CASCADE,
    requested_by  BIGINT NOT NULL,              -- Discord user id
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
// Default impl: small guild (member_count < threshold) => true;
//               large guild => guild.full_map_approved.
```
1. **At schedule creation** (`/schedule-report`, full-map target, large guild, not approved) →
   start the application flow instead of scheduling.
2. **At tick time** (ties into `specs/scheduling.md`, `is_full_map` job marker):
   re-check `is_allowed`. If a large guild's approval was revoked, the job goes **dormant**
   (skips rendering, posts a one-time heads-up) rather than silently burning infra. Re-approval
   reactivates it.

## Decisions (settled)
- **Threshold:** `FULL_MAP_FREE_MEMBER_THRESHOLD = 50` (configurable; tune from real guild sizes).
- **Form delivery: in-Discord modal.** Lowest friction, and answers land straight in
  `full_map_requests` with no external hosting or webhook round-trip. No web form.
- **Review surface: both.** The `REQUESTS_CHANNEL_ID` channel post with Approve/Deny buttons is
  primary; `/full-map-requests list|approve|deny` is the fallback/scriptable path. Both drive the
  same rows and flag.
- **Command names:** `/request-full-map-schedule` and `/full-map-requests` as specced.
- **On-demand render:** a dedicated `/full-map` command (see
  `specs/active/full-map-renderer.md` → Decisions), free for everyone and ungated.
- **Monetization: none.** The form gate is the only mechanism. No paid tier, no vote-wall, no
  donation prompt ships with this feature. If hosting donations are ever added, they stay separate
  from the gate and follow a `code-talk` post to Siege Camp first (**recommended regardless**, as
  a courtesy heads-up about the tool).

## Acceptance criteria
- On-demand full-map render works for everyone; small guilds can schedule full-map reports
  without any gate.
- A large guild scheduling a full-map report is routed into the application flow; a schedule is
  created only after approval.
- A tick for a large guild whose approval was revoked goes dormant (not deleted) and can resume
  on re-approval.
- No money changes hands anywhere in the shipped feature; single-hex commands and hex scheduling
  are unaffected.
