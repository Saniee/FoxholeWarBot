# Tasks: full-map renderer + scheduling gate

## Phase 1 — renderer (`specs/active/full-map-renderer.md`)
- [x] Add grid coordinates to `regions.rs` (53 entries, `col`/`row`), with a pointer to the spec
- [x] Canvas geometry constants in `RenderConfig`: column pitch, row pitch, odd-column offset,
      composite downscale factor (named, not literals)
- [x] `map_render::render_full_map` — bounded-concurrency fetch (`JoinSet` + `Semaphore(8)`),
      ETag-revalidated, reusing `conditional_get`/`resolve`
- [x] Composite all 53 hexes at their offsets in `spawn_blocking`; a failing region degrades to
      background-only, never aborts the map
- [x] Downscale the composite to ~2048 px long edge, encode PNG, check the encoded size against
      Discord's limit
- [x] `/full-map` command — ungated, guild-only, visibility-aware deferral
- [x] **Verify the geometry** — measured off the TGA alpha channel, not guessed: 0 uncovered
      pixels in the shared band, and a 53-hex composite with continuous coastlines. Still worth a
      glance at the *real* render (icons on, `image` crate's compositing) when you next build.
- [x] Builds and renders live (user, Able) — first render was correct apart from two hexes
- [x] Fix those two: lenient region matching, case-insensitive background retry, asset paths
      resolved through the table, and a region the API doesn't list drawn as bare terrain
- [ ] **Re-render and confirm Marban Hollow + Deadlands are back** — the fix is pushed, unverified
- [ ] Judge icon legibility at 0.2x; if poor, raise `icon_size_ratio` for the full map (do **not**
      touch the downscale)
- [ ] Delete the stale `MapMarbanHollow.TGA` and `MapClahstraHexMap.TGA` — older art, no longer
      reachable, still confusing
- [ ] Optional faction tint behind a `RenderConfig` flag, OFF by default
- [ ] Promote the spec out of `specs/active/`, reconcile it to shipped behavior

## Phase 2 — gate (`specs/active/premium-full-map.md`)
- [ ] `migrations/0002_*.sql` — `guilds.full_map_approved` + `full_map_approved_at`;
      `full_map_requests` table (incl. `member_count` snapshot); full-map marker on `cronjobs`
- [ ] `FullMapScheduling` trait + default impl (`guild.full_map_approved`, no size branch)
- [ ] `/schedule-report`: full-map target + unapproved ⇒ route into the form, don't hard-refuse
- [ ] `/request-full-map-schedule` — modal, writes a `pending` row
- [ ] Post the request to `REQUESTS_CHANNEL_ID` (embed + Approve/Deny buttons); unset or
      unreachable ⇒ command-only review + warn, never drop the request
- [ ] Owner/admin gate on the button interactions
- [ ] `/full-map-requests list|approve|deny` over the same rows and flag
- [ ] Tick-time re-check: revoked approval ⇒ job goes **dormant** with a one-time heads-up,
      not deleted; re-approval resumes it
- [ ] Make sure the "no payment involved" wording is prominent — with no free tier at all, an
      approval gate reads even more like a paywall than it did
- [ ] **Docs, in the same commit as the migration** — `tos.md`/`privacy.md` stored-data list,
      retention window (suggest 90 days for denied/withdrawn), the user-facing form section
      stating plainly that no payment is involved
- [ ] Promote the spec out of `specs/active/`, reconcile to shipped behavior
