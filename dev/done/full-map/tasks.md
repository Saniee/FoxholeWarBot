# Tasks: full-map renderer + scheduling gate

## Phase 1 — renderer (`specs/full-map-renderer.md`)
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
- [x] **`/set_guild_settings` replies again.** Never diagnosed as such — it came back working
      after the HTTP timeout landed, which fits the "hung on a silent connection" candidate
- [x] Promote the spec out of `specs/active/`, reconcile it to shipped behavior

## Phase 2 — gate (`specs/premium-full-map.md`)
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
- [x] Reviewer gate on both surfaces: **exactly** `REVIEWER_IDS`, no implicit owner, not guild
      admins. Env var, not a table. Empty ⇒ nobody reviews, warned at startup
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
- [x] First build attempt — two errors, both `is_allowed`'s missing `Send` bound. Fixed
- [x] Builds; request → announce → approve seen working live
- [x] Withdraw (applicant) and Revoke (reviewer) buttons; both land on `withdrawn`
- [x] `0004_request_message.sql` + `review::refresh_post` — a decision made off the post corrects
      the post
- [x] Dormancy notice says **withdrawn**, not "isn't active" — it is the only message that ever
      reports a revocation
- [x] `/request-full-map-schedule` replies in public, not ephemerally
- [x] **Release test pass — walked, and it holds.** See the checklist below for what's left
- [x] Promote the spec out of `specs/active/`, reconcile to shipped behavior

## Release checklist — full-map + gate
**Walked by the user and it holds.** Boot and migration `0004`, the renderer and the tint
tiebreak, applying, withdrawing, deciding, and scheduling + dormancy all behave as designed.
`/set_guild_settings` replies again too.

What that pass did *not* cover — the only things standing between here and a tag:

- [ ] **90-day purge** — can't be exercised without waiting or backdating rows. Either backdate a
      denied/withdrawn row's `reviewed_at` past the window and watch the 03:30 job take it, or
      accept it on inspection. `docs/privacy.md` states the window, so something has to make it
      true; the query is `purge_stale_full_map_requests`
- [ ] **Peak memory during a full-map render** — never measured. 63.6 MP canvas, ~254 MB RGBA
      before downscale, and the container ceiling is not large
- [ ] `cargo clippy` clean — the build is green (user), but clippy was never run from here
- [x] `scripts/update_assets.py --audit` exits 0 — 53 regions, one file each, names exact
- [x] Both specs promoted out of `specs/active/` and reconciled to shipped behavior

Untested and low-stakes, worth knowing rather than doing: `REQUESTS_CHANNEL_ID` unset (should
degrade to command-only review), and a two-reviewer race on one request (should answer "already
decided by someone else").

## Next up (`specs/schedule-input.md`) — moved out, and now shipped
Ran as its own task: `dev/done/schedule-input/`. It took migration `0005`, so the next free
number is **0006**.
