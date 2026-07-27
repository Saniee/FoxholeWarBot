# FoxholeWarBot

A Discord bot that surfaces live [Foxhole](https://www.foxholegame.com/) world-conquest data —
rendered hex maps, war reports, global war state — and posts recurring scheduled map reports
through webhooks.

[Invite the bot](https://discord.com/oauth2/authorize?client_id=886994381259833374) ·
[Docs, ToS & Privacy](https://saniee.github.io/FoxholeWarBot/) ·
[Support server](https://discord.gg/9wzppSgXdQ)

## Commands

- `/get-map` — render one region, with optional text labels.
- `/full-map` — render the whole world map: all 53 regions on one hex grid.
- `/war-report` — casualties, enlistments and day of war for one region.
- `/war-state` — the global war state for the server's shard.
- `/set-guild-settings` — set the shard and reply visibility (Administrator).
- `/schedule-report` — post a region's map to a channel on a recurring schedule (Manage Webhooks).
- `/remove-report` — delete a schedule and its webhook (Manage Webhooks).
- `/schedule-help` — the accepted schedule phrases.

## Self-hosting

The bot is Rust (poise + serenity) and stores its settings in **Postgres**; `compose.yaml` brings
up the bot and a database on a named volume, and migrations are embedded and applied at startup.
See `specs/postgres.md` for the schema and `specs/architecture.md` for how the pieces fit.

```sh
cp .env.example .env      # fill in TOKEN, APP_ID, DATABASE_URL, POSTGRES_PASSWORD
docker compose up -d --build
```

For local development:

```sh
cargo run -- --local      # register commands to GUILD_ID only; fast iteration
cargo run                 # register global commands
cargo run -- --clear-commands
```

Note: `docker compose down -v` removes the database volume. Use `docker compose down`.

---

Foxhole is a registered trademark of Siege Camp. This is an unofficial, free, fan-made tool, not
affiliated with or endorsed by Siege Camp. Map data comes from the public Foxhole War API; map
and icon artwork are the property of Siege Camp.
