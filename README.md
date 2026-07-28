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
- `/set-guild-settings` — set the shard, reply visibility and timezone (Administrator).
- `/schedule-report` — post a map to a channel on a recurring schedule, in your own timezone
  (Manage Webhooks).
- `/remove-report` — delete a schedule and its webhook (Manage Webhooks).
- `/schedule-help` — how schedules are timed.

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

### Logs

> **In Docker, the log files are not in `./logs`.** They are written inside the container, at
> `/app/logs`, which is the `fwb_logs` **named volume** — Docker keeps it under
> `/var/lib/docker/volumes/`, not next to this README. A `./logs` directory in the repository is
> what a *local* `cargo run` writes to, and it stays empty while the bot runs in Docker. The bot
> prints the absolute directory it is using at startup; that line is the answer to "where are
> they".
>
> To get them beside the compose file instead, copy the override in and bring the bot back up:
>
> ```sh
> cp compose.override.example.yaml compose.override.yaml
> docker compose up -d
> ```
>
> `compose.override.yaml` is loaded automatically, needs no flags, and is untracked — it swaps the
> named volume for a `./logs` bind mount and changes nothing else.

The bot writes two files a day into `LOG_DIR` (`/app/logs` in the container, on the `fwb_logs`
volume):

| File | Holds |
|---|---|
| `foxholewarbot.<date>.log` | what the console shows: the bot's own messages, plus any warning |
| `foxholewarbot-verbose.<date>.log` | the same, plus everything serenity, poise, reqwest and sqlx say — every Discord HTTP request, every SQL statement, every connection |

Serenity's **gateway and HTTP** targets are the exception: they're muted even in the verbose file.
Their instrumentation puts whole structures on one line — a gateway event is the entire guild, and
an HTTP request is its body as a list of individual bytes, so posting a map PNG writes a log line
several megabytes long. Measured: 65 MB in two hours from the gateway, then 55.7 MB from 711 HTTP
lines once the gateway was muted. Reconnects, failures and rate limits still appear; they're
warnings, and warnings survive the mute. To get the firehose back for an evening:
`LOG_VERBOSE_FILTER=debug,tracing::span=off`.

The console deliberately shows only the first. The dependencies' chatter is worth keeping and
worth not reading: it buries a dozen useful lines a day under thousands, and it is wanted exactly
when something has already gone wrong.

```sh
docker compose logs -f bot                      # the quiet stream
docker compose exec bot sh -c 'ls /app/logs'     # both files
docker compose exec bot sh -c 'tail -f /app/logs/foxholewarbot-verbose.*.log'
```

Files older than `LOG_RETENTION_DAYS` (default 14) are deleted at startup and nightly. Raise the
console's verbosity temporarily with `RUST_LOG=info` — see `specs/architecture.md` → Logging for
the filters and the other two overrides.

### Updating map and icon art

Artwork tracks [`clapfoot/warapi`](https://github.com/clapfoot/warapi). With a clone of it on
disk:

```sh
scripts/update_assets.py --warapi ../warapi --dry-run   # see what would change
scripts/update_assets.py --warapi ../warapi
scripts/update_assets.py --audit                        # check what's on disk, copy nothing
```

It renames as it copies, because upstream and the renderer disagree about names in ways that fail
silently: upstream ships `MapDeadlandsHex.TGA` where the region table says `DeadLandsHex`, and
icons are descriptive TGA upstream (`MapIconTownBaseTier1Colonial.TGA`) against the
`{iconType}{Team}.png` the renderer looks up. Copying by hand once left a region with no
background and a clean `git status`.

`--audit` cross-checks `assets/Maps/` against the 53-region table and exits non-zero on a missing,
misnamed, or leftover file. Worth running after any art drop.

Some icons aren't in warapi at all and are sourced by hand — the script lists those as expected
and never touches them. If it reports an **UNEXPECTED** icon type instead, upstream renamed the
file: add the new name to `ICON_SOURCES` rather than renaming anything by hand.

---

Foxhole is a registered trademark of Siege Camp. This is an unofficial, free, fan-made tool, not
affiliated with or endorsed by Siege Camp. Map data comes from the public Foxhole War API; map
and icon artwork are the property of Siege Camp.
