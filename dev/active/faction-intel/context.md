# Context: player-reported faction intel

Spec: none yet — pre-spec design brainstorm, not written up. Tasks: `tasks.md`.

## Why this exists

Competitor research proposed "intel/map-warning alerts" and "weather per hex" as features. Both
checked against the live War API, `clapfoot/warapi`'s full commit history, and the official wiki
(which states outright the API's intent is to exclude anything not known to both factions) —
neither field has ever existed, anywhere, official or unofficial. Dead end, confirmed, not
reopened. See `specs/active/war-summaries.md`'s "Provenance" section for the same investigation
applied to that (separate, finished) feature.

Since there's no real intel feed, the idea became: crowdsource one from players instead.

## Current state

Pure brainstorm. No spec, no migration, no code. Seven decisions reached, in order, each closing
one open question before the next was asked:

1. **Scope: per-shard-per-faction, not per-guild.** First cross-guild shared state this bot would
   have — everything today (`specs/architecture.md`) is strictly per-guild.
2. **Faction verification is organizational, not cryptographic** — nothing (game, Steam, War API)
   exposes per-account faction. Two layers: a guild declares its faction once on
   `/set-guild-settings` (new field, Administrator-gated like the rest of that command); only a
   designated role within that guild can submit (same shape as `REVIEWER_IDS`). Containment over
   prevention: every report is tagged with its source guild (visible), the bot owner gets a kill
   switch per guild, and a crowdsourced confirm/deny buries a lone bad report rather than trusting
   it.
3. **Retention solves itself**: wipe a shard's report set on `War.warId` change (already polled,
   free). Explicit wipe, not reliance on natural decay. **Decided against** a default archival
   export — it would quietly break the "ephemeral, gone at war end" privacy story and reopen the
   retention question right after closing it. P2 if ever actually asked for, never default.
4. **Moderation**: curated slur/hate-speech wordlist (reuse an existing open list), *not* a general
   profanity filter — the game's culture is fine with casual swearing. Reject-with-message on a
   hit, same refusal style as the rest of the bot. Repeat abuse handled by the guild's own
   role-gating (point 2), not bot-side bans. Accepted v1 gap: no leetspeak/non-English coverage.
5. **Reporting UX.** Ruled out parsing plain chat messages outright — the bot only holds the
   `GUILDS` intent, no message content, and adding that is a privileged-intent process that cuts
   against the whole slash-commands-and-webhooks design. Landed on two combined approaches out of
   three proposed: (a) a slash command carrying just the hex (reuses
   `commands/common.rs::autocomplete_map` unchanged) → a modal/component flow for the rest; (b) a
   "🛡️ Report sighting here" button attached to existing map-render messages
   (`/get-map`/`/full-map`/scheduled reports), which already know their hex from context. (b)
   needs a stable `custom_id` scheme since scheduled-report messages get edited in place after
   posting (`specs/schedule-report.md`'s placeholder-then-edit pattern) — the button has to keep
   resolving on an old, edited message.
6. **Location precision.** Confirmed Foxhole has a real in-game grid overlay players already use
   for arty calls: lettered/numbered "big square" + 3×3 numeric "keypad" (`G9k3`), and `Ctrl+LMB`
   on the map screen copies that string to the clipboard already — existing muscle memory, nothing
   to teach from scratch. Decision: take the grid string as a modal text field; the bot parses it
   into the region's normalized `(x, y)` — the same space `MapItem`/`MapTextItem` already use.
   Every region is the same in-game size (`WorldExtent` is one fixed constant across all maps,
   confirmed in `warapi`'s README), so one conversion formula covers all 53 regions. Pin rendering
   then reuses `request_processing.rs::place_image_info`'s existing `place()`-at-normalized-ratio
   path unchanged — a report pin is just another icon through the pipeline a `MapItem` already
   goes through. **Unverified, blocking the actual formula:** the grid's lettering/numbering range
   per hex, and the exact string `Ctrl+LMB` puts on the clipboard (may carry more than the bare
   token). Landmark-based fallback (autocomplete over that region's cached `MapTextItems`) proposed
   for when a player has no exact grid string.
7. **Nudging players toward the grid string.** (a) teach-at-point-of-failure — an unparseable grid
   string gets a rejection with an example and the `Ctrl+LMB` instruction, same pattern as the
   invalid-`at_time` message on `/schedule-report`; (b) the instruction lives in the modal
   `TextInput`'s label/placeholder; (c) — the one that matters most — the parser tolerantly pulls
   the grid token out of whatever got pasted rather than requiring an exact match; (d) a possible
   `/report-help` mirroring `/schedule-help`'s always-ephemeral role, only worth it if (a)+(b)
   aren't enough once real players try it.

## Open threads — not reached yet

- The report schema itself: category list, free-text cap, per-category decay/expiry timers.
- The grid→normalized-coordinate math (point 6's blocker).
- New schema/migration shape: the shared per-shard-per-faction report table, the new guild-faction
  field.
- Whether this ships as one spec or several (command surface / data model / moderation / rendering
  could each stand alone — undecided).

## Next steps

Resume the brainstorm: report schema next (category list, free-text cap, decay timers per
category), **or** verify point 6's two unknowns (grid range, exact clipboard string) before
finalizing the coordinate conversion. Either is a reasonable next question — neither is started.
Nothing here blocks on the other.
