# Project: FoxholeWarBot

A Discord bot (Rust / Serenity) that surfaces live [Foxhole](https://www.foxholegame.com/)
world-conquest data — rendered hex maps, war reports, global war state — and posts
recurring scheduled map reports via webhooks.

## Stack
- Rust 2021 (`cargo`)
- [poise](https://crates.io/crates/poise) `0.6` — slash-command framework (rewrite target;
  see `specs/architecture.md` → Command framework), built on
  [serenity](https://crates.io/crates/serenity) `0.12` — Discord gateway
- [sqlx](https://crates.io/crates/sqlx) `0.8` (SQLite) — guild settings + scheduled jobs
- [tokio-cron-scheduler](https://crates.io/crates/tokio-cron-scheduler) `0.13` — scheduled reports
- [image](https://crates.io/crates/image) / [imageproc](https://crates.io/crates/imageproc) / [ab_glyph](https://crates.io/crates/ab_glyph) — map rendering
- [reqwest](https://crates.io/crates/reqwest) `0.12` — Foxhole War API client

## Layout
- `src/main.rs` — client bootstrap, `EventHandler`, DB table creation
- `src/args.rs` — CLI args (`--local`, `--clear-commands`)
- `src/commands/` — one module per slash command (`run`, `autocomplete`, `register`)
- `src/utils/db.rs` — SQLite access + `Shard` enum (shard → API URL)
- `src/utils/cache.rs` — on-disk JSON cache under `./cache/`
- `src/utils/cron.rs` — `CronHandler`, scheduled report jobs
- `src/utils/request_processing.rs` — map image compositing
- `src/utils/api_definitions/foxhole.rs` — Foxhole War API response types
- `assets/Maps/` — per-hex background TGA images; `assets/MapIcons/` — icon PNGs
- `specs/` — feature specifications (see below)

## Commands
- Build: `cargo build`
- Check: `cargo check`
- Lint: `cargo clippy`
- Run (global commands): `cargo run`
- Run (guild-scoped commands, fast iteration): `cargo run -- --local`
- Clear all registered commands: `cargo run -- --clear-commands`

## Runtime config (`.env`)
- `TOKEN` — Discord bot token (required)
- `APP_ID` — Discord application id (required for `--clear-commands`)
- `GUILD_ID` — dev guild id, used by `--local` and `--clear-commands`

## Conventions
- Each command module exports a `#[poise::command(slash_command)]` async fn (typed params for
  options, `#[autocomplete = "..."]` for autocomplete); all are collected into the framework's
  `commands` list. Shared state (`db`, `cron`, `local`) is reached via `ctx.data()`.
- Guild-scoped settings (shard + output visibility) live in the `guilds` table; a guild
  with no row is treated as "not set up" and prompts the user to run `/set-guild-settings`.
- Foxhole API responses are cached to disk and revalidated with `If-None-Match` ETags
  (the API's `version` field); a `304 Not Modified` serves the cached copy.

## Don't
- Don't commit `.env`, `database.db`, or the `cache/` directory.
- Don't hardcode secrets; read them via `dotenv::var`.

## Specs
Feature specs live in `specs/`. `specs/` documents current behavior (1:1 with the code);
`specs/active/` holds specs for work that is planned or being reworked. See `specs/README.md`.
