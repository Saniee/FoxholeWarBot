# Full-map renderer (ACTIVE)

Status: **proposed** — layout derived from a reference screenshot (foxholestats.com, WC137) and
cross-checked 1:1 against `assets/Maps/`. Depends on `specs/active/rendering-placement.md`
(the per-region placement primitive) landing first.

## Summary
Render the **entire world conquest map** as a single image: all 53 hex regions composited onto
their hex-grid positions, each drawn with the existing per-region pipeline (background TGA +
dynamic icons + optional labels).

**Scope: the hex grid only.** No header/war banner, no casualty tables, no event log, no
per-region stat readouts, no player counts. Those exist on the reference site but are explicitly
**out of scope** — the deliverable is the rendered hexes.

## Grid geometry

The region assets are **flat-top hexagons**: 1024 × 888, and 1024/888 = 1.153 ≈ 2/√3 = 1.1547,
the exact aspect of a regular flat-top hex. They tile as a standard flat-top hex grid:

| Quantity | Value | Derivation |
|---|---|---|
| Hex width `W` | **1024** | asset width |
| Hex height `H` | **888** | asset height |
| Column pitch | **768** | `3/4 × W` (flat-top horizontal spacing) |
| Row pitch (within a column) | **888** | `H` |
| Odd-column vertical offset | **444** | `H / 2` |

Placement for a region at grid `(col, row)`:
```
x = col * 768
y = row * 888 + (if col is odd { 444 } else { 0 })
```
Adjacent columns' bounding boxes overlap by `W/4 = 256 px`; the assets are 32-bit RGBA with
transparent hex corners, so alpha compositing produces clean seams.

**Canvas:** columns 0–12 ⇒ width `12 × 768 + 1024 = 10240`. Tallest even column is col 6
(rows 0–6) ⇒ `6 × 888 + 888 = 6216`. Full canvas **10240 × 6216** (~63.6 MP).

> ⚠️ **Verify before trusting:** the 3/4-width packing is derived from the reference screenshot
> plus the assets' hex aspect ratio, **not** measured from the TGAs' transparent margins. On the
> first real render, confirm the seams align; if the art carries extra padding, adjust the column
> pitch and re-derive the canvas. This is the one number most likely to need a nudge.

## Region layout (53 regions)

Grid coordinates, `col` left→right, `row` top→bottom within a column (odd columns sit half a hex
lower). API/asset name is the `*Hex` identifier; the display name is what the game/UI shows.

| Col | Row | Display name | API / asset name (`Map…​.TGA`) |
|---|---|---|---|
| 0 | 2 | Olavis Wake | `OlavisWakeHex` |
| 1 | 1 | Pari Peak | `PariPeakHex` |
| 1 | 2 | Palantine Berm | `PalantineBermHex` |
| 1 | 3 | Oarbreaker Isles | `OarbreakerHex` |
| 2 | 1 | Kuura Strand | `KuuraStrandHex` |
| 2 | 2 | The Gutter | `GutterHex` |
| 2 | 3 | Fishermans Row | `FishermansRowHex` |
| 2 | 4 | Stema Landing | `StemaLandingHex` |
| 3 | 1 | Nevish Line | `NevishLineHex` |
| 3 | 2 | Farranac Coast | `FarranacCoastHex` |
| 3 | 3 | Westgate | `WestgateHex` |
| 3 | 4 | Origin | `OriginHex` |
| 4 | 1 | Callums Cape | `CallumsCapeHex` |
| 4 | 2 | Stonecradle | `StonecradleHex` |
| 4 | 3 | Kings Cage | `KingsCageHex` |
| 4 | 4 | Sableport | `SableportHex` |
| 4 | 5 | Ash Fields | `AshFieldsHex` |
| 5 | 0 | Speaking Woods | `SpeakingWoodsHex` |
| 5 | 1 | The Moors | `MooringCountyHex` |
| 5 | 2 | The Linn of Mercy | `LinnMercyHex` |
| 5 | 3 | Loch Mór | `LochMorHex` |
| 5 | 4 | The Heartlands | `HeartlandsHex` |
| 5 | 5 | Red River | `RedRiverHex` |
| 6 | 0 | Basin Sionnach | `BasinSionnachHex` |
| 6 | 1 | Reaching Trail | `ReachingTrailHex` |
| 6 | 2 | Callahans Passage | `CallahansPassageHex` |
| 6 | 3 | Deadlands | `DeadLandsHex` |
| 6 | 4 | Umbral Wildwood | `UmbralWildwoodHex` |
| 6 | 5 | Great March | `GreatMarchHex` |
| 6 | 6 | Kalokai | `KalokaiHex` |
| 7 | 0 | Howl County | `HowlCountyHex` |
| 7 | 1 | Viper Pit | `ViperPitHex` |
| 7 | 2 | Marban Hollow | `MarbanHollowHex` |
| 7 | 3 | The Drowned Vale | `DrownedValeHex` |
| 7 | 4 | Shackled Chasm | `ShackledChasmHex` |
| 7 | 5 | Acrithia | `AcrithiaHex` |
| 8 | 1 | Clanshead Valley | `ClansheadValleyHex` |
| 8 | 2 | Weathered Expanse | `WeatheredExpanseHex` |
| 8 | 3 | The Clahstra | `ClahstraHex` |
| 8 | 4 | Allods Bight | `AllodsBightHex` |
| 8 | 5 | Terminus | `TerminusHex` |
| 9 | 1 | Morgens Crossing | `MorgensCrossingHex` |
| 9 | 2 | Stlican Shelf | `StlicanShelfHex` |
| 9 | 3 | Endless Shore | `EndlessShoreHex` |
| 9 | 4 | Reavers Pass | `ReaversPassHex` |
| 10 | 2 | Godcrofts | `GodcroftsHex` |
| 10 | 3 | Tempest Island | `TempestIslandHex` |
| 10 | 4 | Wresta | `WrestaHex` |
| 10 | 5 | Onyx | `OnyxHex` |
| 11 | 2 | Lykos Isle | `LykosIsleHex` |
| 11 | 3 | The Fingers | `TheFingersHex` |
| 11 | 4 | Tyrant Foothills | `TyrantFoothillsHex` |
| 12 | 4 | Pipers Enclave | `PipersEnclaveHex` |

**Verified:** this table's 53 entries match the 53 `Map*Hex.TGA` assets exactly — no asset
unplaced, no entry without an asset.

### Storage
A static table in code (`&[(&str, u8, u8)]` or a `const` map), **not** config — the layout only
changes when Siege Camp ships/removes a region, which is a code change anyway (new art asset
needed regardless). Keep it beside the renderer with a comment pointing at this spec.

### Display-name mapping (gotcha)
Several API names differ from displayed names — the renderer must not assume they match:

| Display | API name |
|---|---|
| The Moors | `MooringCountyHex` ⚠️ wholly different |
| Deadlands | `DeadLandsHex` ⚠️ capital **L** |
| Loch Mór | `LochMorHex` (no accent) |
| Oarbreaker Isles | `OarbreakerHex` (no "Isles") |
| The Linn of Mercy | `LinnMercyHex` |
| The Gutter / The Heartlands / The Drowned Vale / The Clahstra | `GutterHex` / `HeartlandsHex` / `DrownedValeHex` / `ClahstraHex` (no "The") |
| The Fingers | `TheFingersHex` ⚠️ **keeps** "The" — inconsistent with the above |

The existing autocomplete's "strip `Hex`" display transform is therefore **wrong** for these; the
rewrite should use an explicit display-name table (this one) rather than string surgery.

## Non-conquest assets (not part of the grid)
- `BGOneWorldMap.TGA` (1920 × 1080, 24-bit, no alpha) — background/decorative art, **not** the
  render target and not a grid tile (confirms the earlier correction).
- `MapHomeRegionC.TGA` / `MapHomeRegionW.TGA` — Colonial/Warden home regions; not part of the
  world-conquest grid, excluded.
- `MapMarbanHollow.TGA`, `MapClahstraHexMap.TGA` — duplicates of their `*Hex` counterparts;
  ignore (candidates for deletion).

## `OriginHex` — un-exclude it
Current code skips `OriginHex` in every autocomplete (`get_map.rs:128`, `war_report.rs:105`,
`schedule_report.rs:109`). The reference screenshot shows **Origin as a live region** (col 3,
row 4) with real activity. The exclusion appears to be legacy (Origin was once a dev/placeholder
region). The rewrite should include it in both the full map and autocomplete — **flagging for
confirmation**, since if the API still returns it as a non-playable stub the exclusion should stay.

## Rendering pipeline
1. Fetch dynamic (+ static if labels are on) data for **all 53 regions** — see Performance.
2. Allocate the 10240 × 6216 RGBA canvas.
3. For each region: run the existing per-region pipeline
   (`specs/active/rendering-placement.md` — background + icons at region-relative sizes), then
   `overlay` the result at its `(x, y)` grid offset. Region-relative sizing means a hex looks
   identical standalone or tiled; only the offset differs.
4. Downscale the finished composite (see below) and encode PNG.

### Downscaling (required)
10240 × 6216 is far past Discord's attachment limit as a PNG. Scale the **composite** uniformly —
never the per-region ratios — with a target long-edge of **~2048 px** (0.2× ⇒ 2048 × 1243), tuned
against the real encoded size. Make the factor a named constant in `RenderConfig`, not a literal.
If icons become illegible at that scale, prefer bumping `icon_size_ratio` for full-map renders
over changing the downscale.

### Performance
This is 53× the work of a single hex — the reason scheduled full-map renders are gated
(`specs/active/premium-full-map.md`):
- Fetch the 53 regions **concurrently** (bounded, e.g. `buffer_unordered(8)`) and **respect the
  API's cache headers/ETags** — the one explicit rule in the War API terms. The existing on-disk
  cache makes most ticks 304s.
- Run the CPU-bound compositing in `spawn_blocking` so the async runtime isn't stalled
  (QA **L-8**, which matters far more at this scale).
- A partial failure (one region 500s or its asset is missing) should render that hex as
  background-only or skip it, **never** abort the whole map.
- Consider caching the finished composite keyed by shard + newest `last_updated`, so concurrent
  requests share one render.

## Open decisions
- **Faction control tint.** The reference tints each hex blue/green by controlling faction (with
  "darker = more recent change"). Our renderer composites the real map art + icons and currently
  has no tint. Add one? It carries a lot of at-a-glance information but is a visual departure
  from the per-hex renders. *Recommend: optional flag, off by default for v1.*
- **Region name labels** on the full map — the reference labels every hex. Our `draw_text` draws
  in-region POI labels, which at 0.2× would be unreadable. A separate "region name per hex" label
  pass may be wanted instead. *Recommend: defer to v2.*
- Command surface: `/full-map` vs an option on `/get-map` (also open in the gating spec).

## Acceptance criteria
- All 53 regions render at their correct grid positions with clean seams (no gaps/overlap
  artifacts).
- A single region's appearance is identical to its standalone `/get-map` render, modulo the
  composite downscale.
- The output attaches successfully to a Discord message (within upload limits).
- One failing region degrades that hex only; the rest of the map still renders.
- Region fetches are concurrent and ETag-revalidated; compositing does not block the runtime.
- No stats/banner/log furniture is drawn — hexes only.
