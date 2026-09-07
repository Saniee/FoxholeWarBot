# Tasks: player-reported faction intel

Spec: none yet. Context: `context.md`.

## 0. Premise

- [x] Confirm no real intel/weather/warning feed exists anywhere (official API, full `warapi`
      history, official wiki, every community wrapper) — done, dead end confirmed, see
      `specs/active/war-summaries.md`'s Provenance section for the investigation.
- [x] Decide to crowdsource instead of dropping the idea.

## 1. Shape of the shared state

- [x] Scope: per-shard-per-faction, not per-guild.
- [x] Faction verification: organizational (guild declares once, role-gated submission), not
      cryptographic — nothing to verify against.
- [x] Anti-poison: source-guild tag on every report, per-guild kill switch, confirm/deny voting.
- [x] Retention: wipe per shard on `War.warId` change; no default export/archive.
- [x] Moderation: slur/hate-speech wordlist, reject-with-message, not a profanity filter.

## 2. Reporting UX

- [x] Ruled out plain-message parsing (no `MESSAGE_CONTENT` intent; architectural mismatch).
- [x] Settled: hex-autocomplete command → modal/component, **plus** a report button on existing
      map-render messages (skips hex selection via message context).
- [ ] custom_id scheme for the report button that survives a scheduled-report message being
      edited in place post-creation.

## 3. Location precision

- [x] Confirmed the in-game grid system (`G9k3`-style) and `Ctrl+LMB`-to-clipboard — real, existing
      player habit.
- [x] Decision: grid string via modal text field → parsed to normalized `(x, y)` → same
      `place_image_info` pipeline a `MapItem` already uses for rendering.
- [ ] **Blocking:** verify the grid's lettering/numbering range per hex (wiki has the concept;
      exact dimensions not yet pulled).
- [ ] **Blocking:** verify the exact string `Ctrl+LMB` copies to clipboard (bare token vs.
      something with extra text around it) — decides how forgiving the parser needs to be.
- [ ] Write the grid→normalized-coordinate conversion formula once both are known.
- [ ] Landmark-based fallback (autocomplete over cached `MapTextItems`) — proposed, not designed.

## 4. Nudging players to the grid string

- [x] Settled on three layers: teach-at-point-of-failure (primary), label/placeholder text,
      tolerant parsing (most important of the three). `/report-help` deferred unless needed.
- [ ] Nothing built yet — these are message-copy decisions, not implemented.

## 5. Not started at all

- [ ] Report schema: category list, free-text cap, per-category decay/expiry timers.
- [ ] Migration shape: shared report table, new guild-faction column.
- [ ] Decide one spec vs. several (command surface / data model / moderation / rendering).
- [ ] Write the actual spec(s) once the above lands.
