# Project: FoxholeWarBot

A Discord bot (Rust / poise + Serenity) that surfaces live [Foxhole](https://www.foxholegame.com/)
world-conquest data — rendered hex maps, war reports, global war state — and posts
recurring scheduled map reports via webhooks.

## Stack
- Rust 2021 (`cargo`)
- [poise](https://crates.io/crates/poise) `0.6` — slash-command framework
  (see `specs/architecture.md` → Command framework), built on
  [serenity](https://crates.io/crates/serenity) `0.12` — Discord gateway
- [sqlx](https://crates.io/crates/sqlx) `0.8` — guild settings + scheduled jobs, on self-hosted
  **Postgres** via docker-compose (see `specs/postgres.md`)
- [tokio-cron-scheduler](https://crates.io/crates/tokio-cron-scheduler) `0.13` — scheduled reports
- [image](https://crates.io/crates/image) / [imageproc](https://crates.io/crates/imageproc) / [ab_glyph](https://crates.io/crates/ab_glyph) — map rendering
- [reqwest](https://crates.io/crates/reqwest) `0.12` — Foxhole War API client

## Layout
- `src/main.rs` — `Data`/`Context`/`Error` types, poise framework bootstrap, run-once `setup`,
  gateway event handler, `on_error`
- `src/args.rs` — CLI args (`--local`, `--clear-commands`)
- `src/commands/` — one module per slash command; `mod.rs::all()` is the registration list
- `src/commands/common.rs` — guild lookup, visibility-aware deferral, map autocomplete
- `src/utils/db.rs` — Postgres access + `Shard` enum (shard → API URL)
- `src/utils/cache.rs` — on-disk JSON cache under `./cache/`
- `src/utils/map_render.rs` — fetch + ETag revalidate + render one region (shared by `/get-map`
  and the scheduled tick)
- `src/utils/cron.rs` — `CronHandler`, scheduled report jobs, the nightly DST rebuild
- `src/utils/schedule.rs` — frequency choices → cron, IANA timezones, fire-time previews
- `src/utils/request_processing.rs` — `RenderConfig` + per-region compositing
- `src/utils/regions.rs` — API region id → display name table
- `src/utils/api_definitions/foxhole.rs` — Foxhole War API response types
- `migrations/` — schema, embedded and applied at startup via `sqlx::migrate!`
- `assets/Maps/` — per-hex background TGA images; `assets/MapIcons/` — icon PNGs
- `scripts/update_assets.py` — refreshes both from a local `clapfoot/warapi` clone, and audits
  `assets/Maps/` against the region table
- `Dockerfile` / `compose.yaml` — self-hosted deployment (bot + Postgres on a named volume)
- `docs/` — the GitHub Pages site (ToS, Privacy, FAQ), deployed by `.github/workflows/pages.yml`
- `specs/` — feature specifications (see below)

## Commands
- Build: `cargo build`
- Check: `cargo check`
- Lint: `cargo clippy`
- Run (global commands): `cargo run`
- Run (guild-scoped commands, fast iteration): `cargo run -- --local`
- Clear all registered commands: `cargo run -- --clear-commands`
- Deploy: `docker compose up -d --build` (never `docker compose down -v` — it wipes the DB)
- Update art: `scripts/update_assets.py --warapi <path-to-warapi-clone>` (`--dry-run` first)
- Audit art only: `scripts/update_assets.py --audit` — exits non-zero if any region's background
  is missing or misnamed

## Runtime config (`.env`)
- `TOKEN` — Discord bot token (required)
- `APP_ID` — Discord application id (required for `--clear-commands`)
- `GUILD_ID` — dev guild id, used by `--local` and `--clear-commands`
- `DATABASE_URL` — Postgres connection string (required)
- `POSTGRES_PASSWORD` — password for the docker-compose `db` service
- `REQUESTS_CHANNEL_ID` — support-server channel for full-map schedule requests (optional;
  see `specs/premium-full-map.md`)
- `REVIEWER_IDS` — Discord user ids allowed to approve/deny those requests. One id, or several
  comma-separated. **The complete list — the app owner is not implicit and must list themselves.**
  Empty means nobody can review; the bot warns at startup

## Conventions
- Each command module exports a `#[poise::command(slash_command)]` async fn (typed params for
  options, `#[autocomplete = "..."]` for autocomplete); all are collected into the framework's
  `commands` list. Shared state (`db`, `cron`, `local`) is reached via `ctx.data()`.
- Guild-scoped settings (shard, output visibility, full-map faction tint) live in the `guilds`
  table; a guild with no row is treated as "not set up" and prompts the user to run
  `/set-guild-settings`.
- Foxhole API responses are cached to disk and revalidated with `If-None-Match` ETags
  (the API's `version` field); a `304 Not Modified` serves the cached copy. The dynamic and
  static halves revalidate independently — never assume they agree.
- Placement/sizing constants belong in `RenderConfig` as ratios of the region footprint, not as
  literals in the compositing code.
- Region display names come from `utils::regions::display_name`, never from string surgery on
  the API id.
- Art comes in through `scripts/update_assets.py`, not by hand. Upstream and this repo disagree
  about names on purpose (upstream ships `MapDeadlandsHex.TGA`, the table says `DeadLandsHex`;
  icons are descriptive TGA upstream and `{iconType}{Team}.png` here), and a hand copy that gets
  the case wrong renders as a missing hex with a clean `git status`.
- The stored-data list in `docs/tos.md` and `docs/privacy.md` is the database schema in prose:
  a change under `migrations/` is also a `docs/` change, in the same commit
  (see `specs/docs-site.md`).

## Don't
- Don't commit `.env` or the `cache/` directory.
- Don't hardcode secrets; read them via `dotenv::var`.

## Specs
Feature specs live in `specs/`. `specs/` documents current behavior (1:1 with the code);
`specs/active/` holds specs for work that is planned or being reworked, and is empty as of 2.0.
See `specs/README.md`.
