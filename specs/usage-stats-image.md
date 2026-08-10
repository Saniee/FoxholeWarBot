# Spec: Image-Based Usage Statistics

## Goal
Replace the `/usage-stats` embed tables with a readable PNG report that presents
all requested daily and weekly statistics without Discord embed field limits.

## Constraints
- Reuse the existing `image`, `imageproc`, `ab_glyph`, and bundled Inter font.
- Preserve reviewer gating, ephemeral delivery, database queries, date ranges,
  columns, and privacy wording.
- Use a dark Foxhole-styled visual language with military-style panels and
  restrained faction colors.
- Support the existing maximums of 14 daily rows and 8 weekly rows.
- Do not change database fields or usage-tracking behavior.

## Approach
Add a dedicated renderer that produces a dynamically sized PNG with a title,
UTC timestamp, summary KPI panels, complete daily and weekly tables, zero-use
rows, and a privacy footer. Keep aggregation separate from drawing, then send
the image as an ephemeral attachment with a minimal embed.

## Files Touched
- `src/commands/usage_stats.rs` — render and send the PNG.
- `src/utils/usage_stats_render.rs` — image layout, typography, panels, tables,
  and PNG encoding.
- `src/utils/mod.rs` — register the renderer module.
- `specs/usage-stats.md` — document image output and layout behavior.

## Acceptance Criteria
- [ ] Authorized reviewers receive an ephemeral PNG report.
- [ ] Every requested daily and weekly bucket appears, including zero-use rows.
- [ ] The maximum 14-day and 8-week ranges remain readable without Discord
  truncation.
- [ ] Summary metrics and table values match the database-backed output.
- [ ] The report uses the Foxhole styling and bundled font.
- [ ] Unauthorized users retain the existing denial behavior.
- [ ] Existing aggregation tests and renderer tests pass.
- [ ] Formatting, check, tests, and Clippy pass.

## Verification
```text
cargo fmt --check
cargo check
cargo test
cargo clippy --all-targets --all-features -- -D warnings
```
