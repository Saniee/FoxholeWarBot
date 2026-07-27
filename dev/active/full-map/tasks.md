# Tasks: full-map renderer + scheduling gate

## Phase 1 — renderer (`specs/active/full-map-renderer.md`)
- [ ] Add grid coordinates to `regions.rs` (53 entries, `col`/`row`), with a pointer to the spec
- [ ] Canvas geometry constants in `RenderConfig`: column pitch, row pitch, odd-column offset,
      composite downscale factor (named, not literals)
- [ ] `map_render::render_full_map` — bounded-concurrency fetch (`buffer_unordered(8)`),
      ETag-revalidated, reusing `conditional_get`/`resolve`
- [ ] Composite all 53 hexes at their offsets in `spawn_blocking`; a failing region degrades to
      background-only, never aborts the map
- [ ] Downscale the composite to ~2048 px long edge, encode PNG, check the encoded size against
      Discord's limit
- [ ] `/full-map` command — ungated, guild-only, visibility-aware deferral
- [ ] **Verify the seams on a real render** before building on the geometry (pitch 768 is derived)
- [ ] Optional faction tint behind a `RenderConfig` flag, OFF by default
- [ ] Promote the spec out of `specs/active/`, reconcile it to shipped behavior

## Phase 2 — gate (`specs/active/premium-full-map.md`)
- [ ] `migrations/0002_*.sql` — `guilds.member_count`, `full_map_approved`,
      `full_map_approved_at`; `full_map_requests` table; full-map marker on `cronjobs`
- [ ] Cache `member_count` on setup + refresh on `guild_create`/update **only if a row exists**
- [ ] `FullMapScheduling` trait + default impl (small guild ⇒ allowed; large ⇒ approval flag)
- [ ] `/schedule-report`: full-map target + large guild + unapproved ⇒ route into the form,
      don't hard-refuse
- [ ] `/request-full-map-schedule` — modal, writes a `pending` row
- [ ] Post the request to `REQUESTS_CHANNEL_ID` (embed + Approve/Deny buttons); unset or
      unreachable ⇒ command-only review + warn, never drop the request
- [ ] Owner/admin gate on the button interactions
- [ ] `/full-map-requests list|approve|deny` over the same rows and flag
- [ ] Tick-time re-check: revoked approval ⇒ job goes **dormant** with a one-time heads-up,
      not deleted; re-approval resumes it
- [ ] **Docs, in the same commit as the migration** — `tos.md`/`privacy.md` stored-data list,
      retention window (suggest 90 days for denied/withdrawn), the user-facing form section
      stating plainly that no payment is involved
- [ ] Promote the spec out of `specs/active/`, reconcile to shipped behavior
