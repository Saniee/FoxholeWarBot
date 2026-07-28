# Tasks: frontline overlay

Spec: `specs/frontline.md`. Context: `context.md`.

Ordered so each step is verifiable before the next depends on it. §1–§5 are done; §6 is
next, and most of it needs a live war rather than more code.

## 1. Neighbours (no rendering involved) — **done**
- [x] `regions::neighbours(&Region) -> Vec<&'static Region>` from odd-q `(col, row)` arithmetic
- [x] Unit test: relation symmetric across all 53, degrees 2–6, Deadlands' full ring, and the
      two-neighbour corners. Compare regions by `api_name`: `REGIONS` is a `const`, so each use
      site can be a separate promoted copy and `ptr::eq` says two references to the same region
      differ. `cargo check` warns "never used" until §2 lands.

## 2. The field — `src/utils/frontline.rs`, no rendering in it
- [x] World-space points: `grid_offset` + normalized item coords, **every faction-held structure**,
      neutral ones excluded; distances in canvas pixels (`sources_in`)
- [x] `F(p) = Σ w/(d²+ε)` Colonial minus Warden, sampled on a coarse grid (`Field::sample`).
      Summed in `f64` — 2000 terms of ~1e-6 lose their tail in `f32`, and the tail *is* the far
      field that decides the quiet stretches. **No bilinear upsample**: the spec asked for one,
      but interpolation cannot invent a crossing inside a cell whose corners agree in sign, so
      smoothing the traced polyline reaches the same curve with one grid instead of two
- [x] Guard the degenerate case: `sample` returns `None` unless **both** sides have something.
      Covers "one faction holds it all" and "nothing here at all" in one check, and beats letting
      marching squares meet a cell that underflowed to exactly `0.0`
- [x] Full-map cost: **measured, not estimated.** On the real 10240 × 6216 canvas with 2000
      structures, release: 1/32 sampling = 62.9k cells = **255 ms**, contour + smoothing 0.26 ms.
      1/16 is 61 ms if that ever matters. No truncation, no spatial binning — the spec's fallback
      is not needed. Numbers are in the spec; note they are *release* numbers, and the Dockerfile
      builds release
- [x] Marching squares on `F = 0` → polylines (`contour`), then Chaikin (`smooth`). Measured on a
      bending fixture: worst turn 0.183 → 0.105 rad while total turning is unchanged, which is
      corner cutting doing its job rather than straightening the front
- [x] 11 unit tests. **The "lone outpost makes no island" criterion is met by the coarse grid, not
      by the field** — within ~√ε of an isolated structure `F` really does cross zero; the crossing
      just falls between samples. Recorded in `contour`'s doc comment, because it is the reason
      not to "improve" this by sampling finely

## 3. Drawing — **done for `/full-map`**; `/get-map` is wired in §4
- [x] `RenderConfig` gains `frontline`, `field_resolution_ratio`, `frontline_width_ratio`,
      `full_map_frontline_px`, `frontline_margin_ratio`, `influence_radius_ratio`,
      `frontline_color`, `frontline_halo`, `frontline_halo_ratio` — ratios, no literals.
      `influence_radius_ratio` replaces the spec's `influence_epsilon`: ε is px², a radius is a
      thing you can have an opinion about
- [x] Halo stroke first, then the line; after the tint, before the icons. **Halo is opaque** —
      segments overlap at every vertex, and a translucent one beads there
- [x] Sample past the hex bounds, mask by the background's alpha. Both covered by tests:
      nothing lands on a fully transparent canvas, and row 0 and row 63 are both painted by a
      line traced past them
- [x] `/full-map`: **decided — draw before the downscale**, as originally specced. The
      region-name precedent does not transfer: what the downscale destroys is *internal* detail
      (glyph counters, stroke gaps), and a stroke has none. Also settled by two constraints the
      alternative breaks — icons are drawn per tile, and the alpha mask stops existing once the
      tiles are composited. Reasoning is in the spec so it is not reopened a third time
- [x] Verified by a local `#[cfg(test)]` render of a synthetic contested world: line continuous
      across seams, reaching the map edge, legible on both pale and dark terrain, correctly
      separating the two sides' icons. **Scratch module deleted after looking at it**
- [x] **Acceptance criterion "off ⇒ byte-identical" verified** by rendering the same tiles twice
      and comparing the PNGs. Cost of the overlay is below the run-to-run variance of the 63.6 MP
      composite at 318 structures; the honest figure is §2's 255 ms at 2000
- [ ] Tune width and colour by eye against a **live** war — the synthetic render says the
      mechanism works, not that the values are right. `influence_radius_ratio` (50 px) is the one
      genuinely unvalidated number: it sets how close to a lone structure the field flips

## 4. Fetching — **done**
- [x] `/get-map` fetches neighbours' dynamic data concurrently (`region_frontline`); a failed
      neighbour warns naming the region and the line degrades at that edge only
- [x] `fetch_dynamic` + `save_dynamic_cache`: the dynamic half alone, which halves the fan-out.
      The independent-revalidation rule is what *permits* this — the halves already had separate
      ETags and separate cache files, so `save_map_cache` just became a call to both
- [x] Neighbour names resolved through `live_name` against the cached map list, so the API's
      spelling reaches the API and the table's reaches the assets. A neighbour not in this war is
      skipped
- [x] Verified with a local single-hex render: line edge to edge between the two sides,
      **13 silhouette-edge pixels painted at each of the two ends it exits by** — the "no gap"
      criterion, measured. Contour runs x 234..1072 and y 392..944 against a 1024 x 888 hex, so
      the oversampling genuinely reaches past the silhouette

## 5. The toggle — **done**, one commit
- [x] `migrations/0006_frontline.sql` — `guilds.frontline BOOLEAN NOT NULL DEFAULT FALSE`
- [x] `db.rs` — the column, and the "leave it alone" `Option<bool>` shape the tint already uses
- [x] `/set-guild-settings` — the option, reporting the stored value rather than "unchanged"
- [x] **`docs/tos.md` + `docs/privacy.md` in the same commit** (repo rule; `specs/docs-site.md`)
- [x] Honoured by all four render sites: `/get-map`, `/full-map`, and both branches of the
      scheduled tick. Easy to wire three and forget the fourth — the cron tick builds its own
      `RenderConfig` twice, once per branch
- [x] `specs/set-guild-settings.md` and `specs/postgres.md` updated

## 6. Finishing — **live-war feedback, round 1 handled; round 2 incoming**
- [x] Round 1: the line leaned onto the Colonial side on both `/full-map` and a Callahans Passage
      hex — about a quarter of the way across instead of half. **Cause was the exponent, not the
      point set**, which is what the triage list had sent me to first. `Σ w/(d²+ε)` lets `n`
      clustered structures hold ground `√n` times as far out as a lone base, so the contour tracked
      *density* rather than territory. Now `w/(d²+ε)²` via `influence_falloff`, making it `n^(1/4)`;
      `influence_radius_ratio` doubled to 100 px alongside it because `k` and `ε` are coupled.
      Measured 150 px → 82 px off centre on the new `a_crowd_does_not_buy_ground` fixture
- [x] Correct the spec claim this falsified — it asserted a structure deep in friendly territory
      "contributes nothing to where the boundary sits", which is not true of any inverse-power
      field. Same commit, per the 1:1 rule
- [x] Reorder context.md's triage list: "the line isn't centred" now points at `influence_falloff`
      first and the point set second
- [x] **Round 2 — "not sure if it's better": better centred, but visibly angular.** That is the
      exponent's known cost, and it proved round 1 had the wrong lever. Added `frontline::footings`,
      one point per cluster of same-side structures, and put `influence_falloff` back to 1.
      Measured on a 3:1-density fixture: every icon at `k=1` is +138 px off centre / 158° of
      turning, at `k=2` +68 px / 90°, **one point per footing at `k=1` is +33 px / 42°** — better
      on both axes than either. Free 16x speedup as well, 334 ms → 21 ms, since the field is
      `O(cells × points)` and 1995 points become 128
- [x] Clustering runs across the whole collected point set, never per region — a town on a hex
      boundary is split across two API responses
- [x] **Round 3: "quicker, and looks passable."** Centring accepted on both `/full-map` and the
      Callahans Passage hex, which settles §6 and unblocks §7. If it ever comes back,
      `footing_cluster_ratio` (100 px) is still the knob, **not** `influence_falloff` — that has
      been the wrong answer once. If it looks like it ignores a fortified town,
      `weight = sqrt(count)` in `footings`, not a return to counting icons
- [x] Acceptance criteria measured so far: off ⇒ byte-identical; the line reaches the silhouette
      with no gap (13 edge pixels painted at each end it exits by); nothing lands in the
      transparent corners. The rest need real data
- [x] `cargo clippy` clean (the two pre-existing warnings excepted) — keep it that way
## 7. Saying which side is which — **done**

- [x] **Two-tone halo**, `colonial_tint` one flank, `warden_tint` the other. **Replaces** the black
      halo rather than sitting outside it, so the line keeps the weight it had. Reuses the
      per-segment rasteriser — the split is the sign of `dx·(py−y0) − dy·(px−x0)` against a flag,
      one comparison per painted pixel — and sizes backwards through `for_full_map` as a multiple
      of the stroke
- [x] **The side comes from the field, never from the polyline.** `frontline::flanks` steps off
      each segment midpoint along its normal *both* ways and compares `influence`; the answer rides
      along in `Edge::colonial_side`, one flag per segment. Two-sided rather than one-sided because
      Chaikin moves the line off `F = 0` by up to a cell, and a single probe can land back across
      the contour. `translate` is a pure shift so it cannot mirror the flags
- [x] `frontline_halo_ratio` 2.0 → 3.0: two thirds of the halo is now the only thing carrying the
      answer, and at 2.0 the visible band is half a line width — sub-pixel after the full-map
      downscale
- [x] **Checked against a real war**, from a cache the user supplied: full map plus three hexes,
      flanks correct on all of them. Full-map caveat recorded in `context.md` — under
      `faction_tint` a band sits on ground already half its own colour, so on the dark hexes the
      Colonial one all but disappears and the white core carries the line
- [x] Pinned by tests: the two flanks get the right colours, and **the same front walked backwards
      renders identically** — the one that would have caught the winding bug. Eyeballed on a
      synthetic wavy front before the scratch render was deleted
- [ ] **B: text — not queued.** Kept written up only so the fallback is costed, not because it is
      scheduled. Build it only if the user asks again after seeing A. Prefer the horizontal `COLONIAL`/`WARDEN` pair offset
      either side of the normal at intervals over glyphs rotated along the tangent — nothing in the
      crate rotates text today, and at 2048 a hex is ~205 px across, where curved text is a few
      pixels tall against a 19 px region-label floor. Hex-only if it ships
- [ ] One setting, not two. A guild that opted into a frontline should not also have to opt into
      being told what it means

## 8. Promotion
- [x] Promote `specs/active/frontline.md` → `specs/`, update `specs/README.md`, move
      `dev/active/frontline/` → `dev/done/`
