# Tasks: frontline overlay

Spec: `specs/active/frontline.md`. Context: `dev/active/frontline/context.md`.

Ordered so each step is verifiable before the next depends on it. Nothing below is started.

## 1. Neighbours (no rendering involved)
- [ ] `regions::neighbours(&Region) -> Vec<&'static Region>` from odd-q `(col, row)` arithmetic
- [ ] Unit test: relation symmetric across all 53, degrees 2–6, and the four hand-checked
      adjacencies from the spec

## 2. The field
- [ ] World-space control points: `grid_offset` + normalized item coords, `CONTROL_ICON_TYPES`
      only, **no** structure tiebreak
- [ ] Label grid (Colonial / Warden / Neutral) with a claim radius, distances in canvas pixels
- [ ] Two-pass chamfer distance transform for the full-map path; naive is fine for one hex
- [ ] Marching squares → polylines, Colonial-vs-Warden edges only

## 3. Drawing
- [ ] `RenderConfig` gains `frontline`, `claim_radius_ratio`, `field_resolution_ratio`,
      `frontline_width_ratio`, `frontline_color`, `frontline_outline` — all ratios, no literals
- [ ] Stroke with an outline underneath, after the tint and before the icons
- [ ] `/full-map`: draw on the full-res composite so the downscale antialiases it

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
- [ ] Validate against a live war — the line's quality is entirely the control-point set, and
      `claim_radius_ratio` is the first thing to tune
- [ ] Check the acceptance criteria in the spec, especially "off ⇒ byte-identical output"
- [ ] `cargo clippy` clean (the two pre-existing warnings excepted)
- [ ] Promote `specs/active/frontline.md` → `specs/`, update `specs/README.md`, move
      `dev/active/frontline/` → `dev/done/`
