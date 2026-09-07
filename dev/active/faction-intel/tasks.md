# Tasks: player-reported faction intel

Spec: none yet. Context: `context.md`.

## 0. Premise

- [x] Confirm no real intel/weather/warning feed exists anywhere (official API, full `warapi`
      history, official wiki, every community wrapper) — dead end confirmed, see
      `specs/active/war-summaries.md`'s Provenance section.
- [x] Decide to crowdsource instead of dropping the idea.

## 1. Shape of the shared state

- [x] Scope: per-shard-per-faction, not per-guild.
- [x] Faction verification: organizational (guild declares once, role-gated submission).
- [x] Anti-poison: source-guild tag, per-guild kill switch, confirm/deny voting.
- [x] Retention: wipe per shard on `War.warId` change; no default export/archive.
- [x] Moderation: slur/hate-speech wordlist, reject-with-message, not a profanity filter.

## 2. Reporting UX

- [x] Ruled out plain-message parsing (no `MESSAGE_CONTENT` intent).
- [x] Settled: hex-autocomplete command → modal/component, plus a report button on existing
      map-render messages.
- [ ] `custom_id` scheme for the report button that survives a scheduled-report message being
      edited in place post-creation.

## 3. Location precision

- [x] Confirmed the in-game grid system (`G9k3`) and `Ctrl+LMB`-to-clipboard.
- [x] Decision: grid string via modal text field → normalized `(x, y)` → existing
      `place_image_info` pipeline.
- [ ] **Blocking, deferred to the user's own local testing:** verify the grid's
      lettering/numbering range per hex.
- [ ] **Blocking, deferred to the user's own local testing:** verify the exact string `Ctrl+LMB`
      copies to clipboard.
- [ ] Write the grid→normalized-coordinate conversion formula once both are known.
- [ ] Landmark-based fallback (autocomplete over cached `MapTextItems`) — proposed, not designed.

## 4. Nudging players to the grid string

- [x] Settled on three layers: teach-at-point-of-failure (primary), label/placeholder text,
      tolerant parsing. `/report-help` deferred unless needed.
- [ ] Nothing built yet — message-copy decisions, not implemented.

## 5. Database schema

- [x] `guilds` columns: `faction`, `intel_reporter_role_id`, `intel_webhook_url`,
      `intel_enabled`, `intel_prompt_state`, `intel_reporting_banned`.
- [x] `faction_reports` table — drafted, see `context.md` for the full DDL.
- [x] `faction_report_votes` table, cascades off `faction_reports` so voter identity never outlives
      the report.
- [x] `shard_war_state` table for rollover detection (`War` isn't cached on disk today, so this is
      new state regardless).
- [x] Dropped redundant `confirms`/`denies` counters in favor of `COUNT()` off the votes table at
      read time — one source of truth.
- [x] Category table (Rust-side): TTL + cooldown + cap per category, see `context.md`'s table.
      Frontline and Weather are the only categories with a cooldown (3h each) and an active cap
      (3 per hex+faction for Frontline, 1 for Weather).
- [x] At-cap behavior: evict oldest active marker, don't reject — same rule for both categories;
      Weather's cap of 1 makes this "new report replaces the existing one."
- [x] Weather added: 36h TTL, 3h cooldown, 1 active per hex+faction. Noted as the one category
      that isn't really faction-specific (a storm is the same fact for both sides) but is siloed
      per-faction anyway, for one uniform rule instead of a carve-out. No sourced real-world storm
      duration exists (checked raw wikitext directly) — numbers here mirror Frontline's shape, not
      an observed minimum.
- [x] Frontline TTL confirmed at 36h.
- [x] Free-text note length cap confirmed at 200 characters.

## 6. Delivery and voting

- [x] Fan-out via per-guild webhook (same pattern as `/schedule-report`) to every opted-in guild on
      the shard+faction, submitter's own guild included.
- [x] Voting open to anyone in an opted-in guild's channel, not gated to the reporter role;
      submitter can't vote on their own report.
- [x] Decided against live-editing the tally across every guild's mirror on every vote — votes
      affect future reads, not historical posts.
- [x] Decided the posted embed never shows the reporter's identity — internal-only, for anti-abuse
      checks. Expired/wiped rows' old Discord messages are left alone in scrollback.

## 7. Opt-in flow

- [x] One-time automatic nudge appended to `/set-guild-settings`'s confirmation (two buttons:
      Enable / No thanks), never repeated once answered.
- [x] Manual re-entry: new command `/set-faction-intel` (Administrator-gated), always opens the
      same 3-step flow (faction → reporter role → channel) regardless of current state. Nudge's
      "Enable" button and this command converge on one handler.
- [x] `intel_prompt_state` tracks nudge history only; `intel_enabled` tracks current state
      separately, so disabling doesn't erase the remembered config.
- [x] **Confirmed:** a "Reconfigure / Disable" first screen shown only when `intel_enabled` is
      already true, before the 3-step flow — "Reconfigure" proceeds into the normal 3 steps,
      "Disable" flips `intel_enabled` off immediately with no further clicks. Skipped entirely for
      a guild that's never been enabled. Going in as designed; revisit based on user response.

## 8. Spec-writing

- [x] Decided: two specs, not one — `specs/active/faction-intel.md` (reporting, schema, delivery,
      voting, moderation) and `specs/active/faction-intel-rendering.md` (grid parsing, pins),
      split because only the rendering half is blocked on the user's own local verification.
- [x] Both written.
- [ ] Per-category/per-region filtering on the delivery channel — left out of both specs,
      consistent with "probably skip for v1." Revisit only if asked for later.

## 9. Next: implementation (not started)

- [ ] Grid dimensions + clipboard format (the user's local testing) — unblocks
      `faction-intel-rendering.md`'s conversion formula.
- [ ] Migration for the schema in `faction-intel.md`'s Data model section.
- [ ] `docs/tos.md` + `docs/privacy.md` update, same commit as the migration.
- [ ] Everything else in both specs' Acceptance Criteria sections.
