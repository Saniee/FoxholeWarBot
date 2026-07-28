# Tasks: frontline overlay

Spec: `specs/active/frontline.md`. Context: `dev/active/frontline/context.md`.

Ordered so each step is verifiable before the next depends on it. Nothing below is started.

## 1. Neighbours (no rendering involved)
- [ ] `regions::neighbours(&Region) -> Vec<&'static Region>` from odd-q `(col, row)` arithmetic
- [ ] Unit test: relation symmetric across all 53, degrees 2–6, and the four hand-checked
      adjacencies from the spec

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
- [ ] Halo stroke first, then black line; after the tint, before the icons
- [ ] **Clip to the background's alpha** so nothing lands in the transparent hex corners
- [ ] `/full-map`: draw on the full-res composite, width sized backwards from the finished image

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
