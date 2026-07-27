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
      majority. Strength 0.5 (user's pick). **Seen working at full-map scale**
- [x] Tint tiebreak — one hex came back untinted because its bases were split evenly. A tie on
      bases now falls through to all other faction-held structures. **Unverified — needs a build**
- [x] `scripts/update_assets.py` — pull art from a local warapi clone, renaming to the table's
      spelling; `--audit` cross-checks `assets/Maps/` against `regions.rs`
- [x] Clear the dev guild's commands on a global registration — stale guild-scoped registrations
      from `--local` outlive the process and hang as unhandled interactions
- [x] One shared HTTP client with timeouts (`utils::http`) — nothing had a request timeout
- [ ] **Diagnose the `/set_guild_settings` non-reply** — see "Open question" in `context.md`.
      Needs `docker compose logs bot`; the timeout fix may or may not cover it
- [ ] Promote the spec out of `specs/active/`, reconcile it to shipped behavior

## Phase 2 — gate (`specs/active/premium-full-map.md`)
- [x] `migrations/0003_full_map_gate.sql` — `guilds.full_map_approved` + `full_map_approved_at`;
      `full_map_requests` table (incl. `member_count` snapshot); `cronjobs.dormant_notified`;
      `cronjobs.map_name` nullable, NULL = the full map (no `is_full_map` flag beside it)
- [x] `db.rs` access layer — `FullMapRequest`/`NewFullMapRequest`/`RequestStatus`, the queue
      reads, `review_full_map_request` (one transaction, `WHERE status = 'pending'`),
      `set_full_map_approved`, `purge_stale_full_map_requests`
- [x] `FullMapScheduling` trait + default impl (`guild.full_map_approved`, no size branch)
- [x] `/schedule-report`: full-map target via a `full-map` sentinel in the existing `map-name`
      option; unapproved ⇒ points at the form and names the free path, never a bare refusal
- [x] `/request-full-map-schedule` — modal, writes a `pending` row
- [x] Post the request to `REQUESTS_CHANNEL_ID` (embed + Approve/Deny buttons); unset or
      unreachable ⇒ command-only review + warn, never drop the request
- [x] Owner/team gate on both surfaces — **not** guild admins: a reviewer reads other servers'
      free-text answers, so the gate is who owns the bot, not who owns a server that added it
- [x] `/full-map-requests list|approve|deny|revoke` over the same rows and flag
- [x] Tick-time re-check: revoked approval ⇒ job goes **dormant** with a one-time heads-up,
      not deleted; re-approval resumes it (`go_dormant`, `dormant_notified`)
- [x] 90-day purge of denied/withdrawn requests, as a daily job — `docs/` states the window, so
      it has to be enforced somewhere
- [x] "No payment involved" wording — in the submit reply, the unapproved-target reply,
      `docs/tos.md`, `docs/privacy.md` and `docs/faq.md`
- [x] **Docs, in the same commit as the migration** — `tos.md`/`privacy.md` stored-data list,
      90-day retention for denied/withdrawn, the user-facing form section stating plainly that no
      payment is involved
- [ ] **Build and exercise the whole flow** — none of Phase 2 has been compiled or run
- [ ] Promote the spec out of `specs/active/`, reconcile to shipped behavior

## Next up (`specs/active/schedule-input.md`) — specced, nothing built
Raised by the user this session: people can't get a phrase past the free-text `schedule` box, and
nothing captures a timezone, so "18:00" means 18:00 UTC. Shape is settled (guild timezone default
+ per-schedule override; choice list + `Custom…`; existing schedules untouched). Two open
questions left: weekly in v1, and the minimum interval for full-map schedules.
- [ ] `migrations/0004_*.sql` — `guilds.timezone`, `cronjobs.timezone`, `cronjobs.schedule_label`
- [ ] Structured `frequency` + `at_time` + autocompleted `timezone`; bot generates the cron
- [ ] `Custom…` escape hatch — never in the way, and **must show the next three fire times before
      anything is saved**. Show that preview on the choice-list path too
- [ ] **Nightly job rebuild** — `Job::new_async_tz` snapshots a fixed offset at construction, so
      DST is otherwise only applied on restart
- [ ] Embed shows `<t:epoch:F>`/`<t:epoch:R>` so each reader sees their own timezone
