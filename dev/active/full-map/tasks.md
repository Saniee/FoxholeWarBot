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
- [ ] **Release test pass — see "Release checklist" below.** Everything after the first live run
      is uncompiled
- [ ] Promote the spec out of `specs/active/`, reconcile to shipped behavior

## Release checklist — full-map + gate
Ordered so each step sets up the next. Anything marked **never observed** has no runtime evidence
behind it at all.

### 0. Boot
- [ ] `docker compose up -d --build` applies `0004` cleanly on the **existing** database (0001–0003
      already ran there; this is the first migration landing on live rows)
- [ ] Startup log: `N full-map reviewer(s) configured`. Empty `REVIEWER_IDS` warns instead
- [ ] Commands register; no duplicates in the dev guild

### 1. Renderer (Phase 1 leftovers)
- [ ] `/full-map` — all 53 hexes, icons legible, no missing tiles
- [ ] Faction tint on: **every** contested hex tinted. The even-split tiebreak is **never
      observed** — a genuine dead heat still renders plain, which is correct, not a fault
- [ ] Peak container memory during a full-map render (**never measured**; the canvas is 63.6 MP,
      ~254 MB RGBA before downscale)
- [ ] `/set_guild_settings shard:… show_messages:… faction_tint:…` **replies**. Old bug, never
      diagnosed — if it hangs again, grab `docker compose logs bot` around it

### 2. Applying
- [ ] `/request-full-map-schedule` without `/set-guild-settings` first ⇒ the needs-setup reply
- [ ] Filing works; reply is **public**, carries **Withdraw request**
- [ ] The post appears in `REQUESTS_CHANNEL_ID` with Approve/Deny
- [ ] Filing a second time ⇒ "already open", also with Withdraw (not a duplicate row)
- [ ] Modal dismissed without submitting ⇒ nothing recorded, no error

### 3. Withdrawing (new, uncompiled)
- [ ] Applicant presses Withdraw ⇒ their message becomes "withdrawn", buttons gone
- [ ] **The review post updates too** — grey, `withdrawn`, "Closed by", no buttons. This is what
      `0004` bought; if it doesn't happen, check `message_id` on the row
- [ ] A **different member** presses Withdraw ⇒ "That isn't your request to withdraw", nothing
      changes. The reply is public, so this is reachable
- [ ] After withdrawing, a new request can be filed (the one-pending index released)

### 4. Deciding
- [ ] Approve from the post ⇒ green embed, Revoke button, requester told in their channel
- [ ] Approve from `/full-map-requests approve` ⇒ **the post updates as well**
- [ ] Deny ⇒ red, no buttons, requester told
- [ ] Two reviewers race one request ⇒ second gets "already decided by someone else"
- [ ] `/full-map-requests list` from a non-reviewer ⇒ refused. **Check it refuses a server admin
      who isn't in `REVIEWER_IDS`** — the whole point of dropping the admin path
- [ ] `REQUESTS_CHANNEL_ID` unset ⇒ requests still file, `/full-map-requests` still reviews

### 5. Scheduling and dormancy — **never observed end to end**
- [ ] Unapproved `/schedule-report map-name:full-map` ⇒ the short pointer at the form, no schedule
- [ ] Approved ⇒ schedule created; a full map actually posts at its first tick
- [ ] Revoke (button **and** `/full-map-requests revoke`) ⇒ next tick posts
      **Full-Map Approval Withdrawn**, renders nothing, deletes nothing
- [ ] **The notice appears exactly once** across several ticks (`dormant_notified`). This flag has
      no observation behind it whatsoever
- [ ] Re-approve ⇒ the schedule resumes on its own, and a later revoke notifies again
- [ ] A single-region schedule keeps running untouched through all of the above
- [ ] Restart the bot mid-way ⇒ `restore_jobs` brings both kinds back

### 6. Before tagging
- [ ] `cargo clippy` clean
- [ ] `scripts/update_assets.py --audit` exits 0
- [ ] `docs/` matches the schema after `0004` (message id, withdrawn retention) — already written,
      worth re-reading against the built behavior
- [ ] Both `specs/active/` specs promoted and reconciled

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
