# Tasks: frontline overlay

Spec: `specs/active/frontline.md`. Context: `dev/active/frontline/context.md`.

Ordered so each step is verifiable before the next depends on it. §1 is done; §2 is next.

## 1. Neighbours (no rendering involved) — **done**
- [x] `regions::neighbours(&Region) -> Vec<&'static Region>` from odd-q `(col, row)` arithmetic
- [x] Unit test: relation symmetric across all 53, degrees 2–6, Deadlands' full ring, and the
      two-neighbour corners. Compare regions by `api_name`: `REGIONS` is a `const`, so each use
      site can be a separate promoted copy and `ptr::eq` says two references to the same region
      differ. `cargo check` warns "never used" until §2 lands.

## 2. The field
- [ ] World-space points: `grid_offset` + normalized item coords, **every faction-held structure**,
      neutral ones excluded; distances in canvas pixels
- [ ] `F(p) = Σ w/(d²+ε)` Colonial minus Warden, sampled on a coarse grid, bilinear upsample
- [ ] Guard the degenerate case: no structures ⇒ no contour, not whatever marching squares does
- [ ] Full-map cost: sample at ~1/32; if still slow, truncate influence + spatially bin the points
- [ ] Marching squares on `F = 0` → polylines, then Chaikin smoothing (target: ≤1 px/px slope
      change, matching the reference)

## 3. Drawing
- [ ] `RenderConfig` gains `frontline`, `field_resolution_ratio`, `frontline_width_ratio`,
      `full_map_frontline_px`, `frontline_color`, `frontline_halo`, `influence_epsilon` — ratios,
      no literals
- [ ] Halo stroke first, then the line; after the tint, before the icons
- [ ] **Sample past the hex bounds, then mask by the background's alpha** — reaches the silhouette
      with no gap, and nothing lands in the transparent corners
- [ ] Tune width and colour by eye on a live render; the spec's values are starting points
- [ ] `/full-map`: **decide draw-before or draw-after the downscale first** — the region-name work
      changed what the cheaper option is, see context.md. Before ⇒ width sized backwards via
      `full_map_frontline_px`; after ⇒ the contour is scaled and stroked at final width, and
      `for_full_map` needs nothing
- [ ] Verify by rendering locally, the way the labels were: a `#[cfg(test)]` render to
      `scratchpad/`, no API and no Discord. `composite_full_map` takes `Vec<Tile>` with `None`
      data, and `place_image_info` takes hand-built `DynamicMapData` — synthetic structures on
      two sides are exactly what a contour test wants

## 4. Fetching
- [ ] `/get-map` fetches neighbours' dynamic data; a failed neighbour warns and does not fail
      the render
- [ ] Confirm the dynamic/static independent-revalidation rule still holds (only dynamic is
      needed here)

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
