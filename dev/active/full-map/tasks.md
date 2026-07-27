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
- [x] **Re-rendered: all 53 hexes present.** The world map is whole
- [x] Icon legibility at 0.2x — it was ~1 px on screen, i.e. invisible. Fixed with
      `full_map_icon_px` (12) + `RenderConfig::for_full_map`, which sizes source icons backwards
      from the finished PNG. Downscale untouched. **Unverified — needs a build**
- [x] Deleted the stale `MapMarbanHollow.TGA` and `MapClahstraHexMap.TGA`; `assets/Maps/` is now
      exactly one file per table region plus three non-conquest assets
- [x] Faction tint behind `RenderConfig::faction_tint`, off by default, opt-in per guild via
      `/set_guild_settings faction_tint:true`. Full map only; control = town + relic bases by
      majority; ties untinted. Strength 0.5 (user's pick). **Unverified — needs a build**
- [x] `scripts/update_assets.py` — pull art from a local warapi clone, renaming to the table's
      spelling; `--audit` cross-checks `assets/Maps/` against `regions.rs`
- [x] Clear the dev guild's commands on a global registration — stale guild-scoped registrations
      from `--local` outlive the process and hang as unhandled interactions
- [x] One shared HTTP client with timeouts (`utils::http`) — nothing had a request timeout
- [ ] **Diagnose the `/set_guild_settings` non-reply** — see "Open question" in `context.md`.
      Needs `docker compose logs bot`; the timeout fix may or may not cover it
- [ ] Promote the spec out of `specs/active/`, reconcile it to shipped behavior

## Phase 2 — gate (`specs/active/premium-full-map.md`)
- [ ] `migrations/0003_*.sql` — `guilds.full_map_approved` + `full_map_approved_at`;
      `full_map_requests` table (incl. `member_count` snapshot); full-map marker on `cronjobs`.
      **0002 is taken** by the faction tint column
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
