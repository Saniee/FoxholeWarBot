# Docs & legal overhaul (ACTIVE)

Status: **proposed.** Covers `docs/` (the GitHub Pages site: ToS, Privacy, FAQ, index).

## Motivation
Two problems:
1. **The current text doesn't match the code.** It claims data collection that doesn't happen and
   permissions the bot doesn't use — an inaccurate privacy policy is worse than a sparse one.
2. **New surfaces need documenting:** the full-map request form (which collects free-text answers
   — the first genuinely *new* data the bot stores) and the move to Postgres.

Approach decided: **the form approach** — the full-map gate is a non-monetary approval form
(`specs/active/premium-full-map.md`), so the docs describe an application process, **not** a paid
tier. Nothing in `docs/` should imply payment, subscription, or premium purchase.

## Accuracy fixes (current text vs. actual code)

| Claim in docs | Reality | Action |
|---|---|---|
| ToS: collects "the id **and the owner**" of the guild | Only `guild_id`, `shard`, `shard_name`, `show_command_output` are stored (`db.rs`). No owner is ever read or stored. | Remove the owner claim. |
| ToS + Privacy: "will message the owner of a guild if there are any errors" | No such code path exists; the bot never DMs anyone. | Remove, or implement it — **remove** (simpler, and it's not planned). |
| FAQ: requires **Add Reactions** | Bot runs on `GUILDS` intent only and never reacts. | Remove from the permission list. |
| FAQ: "Send Messages in Threads" | Not used by any current command path (slash replies + webhooks only). | Verify against the rewrite; drop if unused. |
| Privacy: "destroys it after its done making the request" | Accurate for command args, but the on-disk API cache (`./cache/`) and the DB row persist. | Clarify what persists and for how long. |

**Genuinely required permissions** (verify against the rewrite before publishing): Send Messages,
Embed Links, Attach Files, Manage Webhooks (scheduled reports), Use Application Commands.

## New content to add

### Data collected (rewrite the ToS/Privacy around this)
State plainly, as a list:
- **Per guild:** guild ID, chosen shard, output-visibility preference, cached member count
  (for the full-map threshold), full-map approval flag.
- **Per scheduled report:** schedule name, cron/phrase, target channel's webhook URL, map name,
  draw-text flag.
- **Per full-map request (new):** requesting user's Discord ID, guild ID/name/member count,
  requested cadence and channel, and the free-text answers (use case, audience, optional contact).
- **Not collected:** message content, member lists, personal profile data, payment data (there is
  none — see below).
- **Retention:** guild rows and schedules are deleted when the bot leaves a guild
  (`ON DELETE CASCADE` per the Postgres migration); request rows retained for review history —
  **decide a retention window** (suggest: purge denied/withdrawn requests after 90 days).

### The full-map request form (new docs section)
Explain, in user-facing terms:
- The full map is **free to render on demand** for everyone.
- **Scheduled** full-map reports are free for guilds under the member threshold; larger guilds
  submit a short request because each scheduled render is a recurring cost on the bot's host.
- What the form asks, that answers are reviewed by the bot owner in the support server, and that
  approval may be revoked (with the schedule going dormant, not deleted).
- **No payment is involved at any point.**

### Attribution / third-party notice (new, important)
Add a clear notice — this is both good practice and useful cover given the ToS ambiguity found
earlier:
> Foxhole is a registered trademark of Siege Camp. This is an unofficial, free, fan-made tool,
> not affiliated with or endorsed by Siege Camp. Map data is retrieved from the public Foxhole
> War API; map and icon artwork are the property of Siege Camp.

(The reference site foxholestats.com carries an equivalent notice, which is the community norm.)

### Self-hosting / infra note
Brief mention that the bot now runs against Postgres (`specs/active/postgres-migration.md`) — only
relevant to self-hosters, so keep it in the README rather than the legal pages.

## Housekeeping
- **Contact details:** refresh the support-server invite, Discord handle, email, and the (possibly
  stale) Twitter link — the invite in `remove_report.rs` and `docs/` should match one source.
- **Dead link:** `/schedule-help` points at a `prnt.sc` screenshot for schedule phrases (link-rot
  risk, QA note in `specs/schedule-help.md`). Replace with inline text or a docs page listing the
  accepted phrases, and link that instead.
- `docs/index.md` — add the FAQ/ToS/Privacy links for the new sections; keep the midnight theme.
- Ensure the Pages workflow (`.github/workflows/pages.yml`) builds the updated site.

## Out of scope
- Any monetization, donation, or sponsorship language — the form approach means none is needed.
- Legal review. These are plain-language docs for a free community tool, not lawyer-drafted terms;
  if the bot ever handles money or personal data, that changes.

## Acceptance criteria
- No statement in `docs/` describes data the bot doesn't collect or permissions it doesn't use.
- The data-collected list matches the actual Postgres schema field-for-field.
- The full-map request form is documented, and nothing implies payment.
- A Siege Camp attribution/non-affiliation notice is present.
- Support contact details are current and consistent between the docs and in-bot messages.
