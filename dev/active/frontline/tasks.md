# Tasks: frontline overlay

Spec: `specs/active/frontline.md`. Context: `dev/active/frontline/context.md`.

Ordered so each step is verifiable before the next depends on it. §1 is done; §2 is next.

## 1. Neighbours (no rendering involved) — **done**
- [x] `regions::neighbours(&Region) -> Vec<&'static Region>` from odd-q `(col, row)` arithmetic
- [x] Unit test: relation symmetric across all 53, degrees 2–6, Deadlands' full ring, and the
      two-neighbour corners. Compare regions by `api_name`: `REGIONS` is a `const`, so each use
      site can be a separate promoted copy and `ptr::eq` says two references to the same region
      differ. `cargo check` warns "never used" until §2 lands.

## 2. The field — `src/utils/frontline.rs`, no rendering in it
- [x] World-space points: `grid_offset` + normalized item coords, **every faction-held structure**,
      neutral ones excluded; distances in canvas pixels (`sources_in`)
- [x] `F(p) = Σ w/(d²+ε)` Colonial minus Warden, sampled on a coarse grid (`Field::sample`),
      bilinear upsample (`Field::value_at`). Summed in `f64` — 2000 terms of ~1e-6 lose their tail
      in `f32`, and the tail *is* the far field that decides the quiet stretches
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

## 5. The toggle — one commit, all of it
- [ ] `migrations/` — `guilds.frontline`
- [ ] `db.rs` — the column, and the "leave it alone" `Option<bool>` shape the tint already uses
- [ ] `/set-guild-settings` — the option, and reporting what the guild actually has
- [ ] **`docs/tos.md` + `docs/privacy.md` in the same commit** (repo rule; `specs/docs-site.md`)

## 6. Finishing
- [ ] Validate against a live war — the line's quality is entirely the point set. If it sits
      wrong, try `CONTROL_ICON_TYPES` only, or per-type weights, **before** touching resolution or
      smoothing; those cannot fix a bad input
- [ ] Check the acceptance criteria in the spec, especially "off ⇒ byte-identical output"
- [ ] `cargo clippy` clean (the two pre-existing warnings excepted)
- [ ] Promote `specs/active/frontline.md` → `specs/`, update `specs/README.md`, move
      `dev/active/frontline/` → `dev/done/`
