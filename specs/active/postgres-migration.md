# SQLite → Postgres migration (ACTIVE)

Status: **proposed** — rewrite architecture decision.

## Motivation
The current store is a single local `database.db` (SQLite). An accidental delete or a redeploy
that doesn't preserve the file wipes all guild settings and scheduled reports — this has already
happened multiple times. Moving to a self-hosted **Postgres** instance (with a persistent named
volume) makes the data durable and decoupled from the bot container's filesystem.

## Decisions (locked)
- **Fresh start** — no data is carried over from the SQLite file. Guilds re-run
  `/set-guild-settings`; schedules are re-created. The legacy `foxholewarbot` migration path
  (`db.migrate()`) is **removed** entirely.
- **Self-hosted via docker-compose** — a `db` (Postgres) service with a named volume, and the
  bot service pointing `DATABASE_URL` at it.

## Dependency & connection changes
- `Cargo.toml`: swap the `sqlx` feature `sqlite` → `postgres`; keep `runtime-tokio`.
  ```toml
  sqlx = { version = "0.8", features = ["runtime-tokio", "postgres"] }
  ```
- `Database::connect()` reads `DATABASE_URL` (a Postgres URL) instead of a filename:
  ```rust
  let url = dotenv::var("DATABASE_URL").expect("DATABASE_URL not set");
  let conn = sqlx::postgres::PgPoolOptions::new()
      .max_connections(5)
      .connect(&url).await?;
  ```
- `Pool<Sqlite>` → `Pool<Postgres>` (`PgPool`) throughout `db.rs` and `cron.rs`.

## Dialect changes (must-do)
- **Placeholders:** every query moves from SQLite `?1, ?2` to Postgres `$1, $2`.
- **Snowflakes need 64-bit:** SQLite `INTEGER` is 64-bit, but Postgres `INTEGER` is 32-bit and
  **cannot hold a Discord ID**. All id/guild columns become `BIGINT`. (Rust side stays `i64`.)
- **Auto-increment:** `INTEGER PRIMARY KEY AUTOINCREMENT` → `BIGINT GENERATED ALWAYS AS IDENTITY`
  (or `BIGSERIAL`).
- **Booleans:** the `0/1 CHECK` columns (`show_command_output`, `draw_text`) become native
  `BOOLEAN`. Rust fields change `i32` → `bool` (drops the `== 1` comparisons and the
  `show == 1 ? defer : defer_ephemeral` branching reads as `if show { .. } else { .. }`).
- **Upsert:** `create_guild` becomes an idempotent upsert
  (`INSERT ... ON CONFLICT (guild_id) DO NOTHING`/`DO UPDATE`) — see schema constraints below.

## Schema (Postgres, fresh)
Managed with `sqlx::migrate!()` from a committed `migrations/` directory (embedded at compile
time — no DB needed to build; applied at startup), replacing the inline `CREATE TABLE IF NOT
EXISTS` statements in `main.rs`.

```sql
-- migrations/0001_init.sql
CREATE TABLE guilds (
    id                  BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    guild_id            BIGINT  NOT NULL UNIQUE,           -- fixes QA C-10
    shard               TEXT    NOT NULL,                  -- resolved API base URL
    shard_name          TEXT    NOT NULL,                  -- "Able" | "Baker" | "Charlie"
    show_command_output BOOLEAN NOT NULL DEFAULT FALSE
);

CREATE TABLE cronjobs (
    id          BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    guild       BIGINT  NOT NULL REFERENCES guilds(id) ON DELETE CASCADE,
    job_name    TEXT    NOT NULL,
    schedule    TEXT    NOT NULL,
    webhook_url TEXT    NOT NULL,
    map_name    TEXT    NOT NULL,
    draw_text   BOOLEAN NOT NULL DEFAULT FALSE,
    job_id      TEXT,                                      -- scheduler UUID, set post-insert
    UNIQUE (guild, job_name)                               -- per-guild names; fixes QA B-2
);
```

Constraint changes vs current SQLite schema:
- `guilds.guild_id` gains `NOT NULL UNIQUE` → kills duplicate-row lookups (QA **C-10**) and
  enables the upsert.
- `cronjobs` uniqueness moves from global `job_name` to composite `(guild, job_name)` (QA **B-2**).
- FK gains `ON DELETE CASCADE` so `delete_guild` cleans up a guild's jobs automatically.
- `cronjobs` gets its own surrogate `id`; `job_id` no longer needs to be UNIQUE (it's set after
  scheduling and only ever looked up by row).

## Removed / retired
- `db.migrate()` and the whole legacy `foxholewarbot` copy-and-drop step.
- The inline table-creation SQL in `main.rs` (superseded by `migrations/`).

## Deployment (docker-compose)
```yaml
# compose.yaml (sketch — Dockerfile for the bot added in the rewrite)
services:
  db:
    image: postgres:16
    restart: unless-stopped
    environment:
      POSTGRES_USER: foxhole
      POSTGRES_PASSWORD: ${POSTGRES_PASSWORD}
      POSTGRES_DB: foxholewarbot
    volumes:
      - fwb_db:/var/lib/postgresql/data          # durable, survives container removal
    healthcheck:
      test: ["CMD-SHELL", "pg_isready -U foxhole -d foxholewarbot"]
      interval: 5s
      timeout: 3s
      retries: 10

  bot:
    build: .
    restart: unless-stopped
    depends_on:
      db:
        condition: service_healthy
    environment:
      DATABASE_URL: postgres://foxhole:${POSTGRES_PASSWORD}@db:5432/foxholewarbot
      TOKEN: ${TOKEN}
      APP_ID: ${APP_ID}
      GUILD_ID: ${GUILD_ID}

volumes:
  fwb_db:
```

- The named volume `fwb_db` is what makes the data durable — `docker compose down` (without
  `-v`) keeps it. Document: **never** `down -v` unless you intend to wipe.
- Recommend a periodic `pg_dump` backup (cron on the host or a small sidecar) as a second line
  of defense.

## Env var changes
- **Add** `DATABASE_URL` (and, for compose, `POSTGRES_PASSWORD`).
- **Remove** the implicit `database.db` file dependency.
- Update `.env.example` accordingly.

## Cache note
The on-disk JSON cache under `./cache/` is unaffected — it's a rendering/API cache, not durable
state, and can stay on the container filesystem (or a small volume if you want to avoid cold
re-fetches on restart).

## Acceptance criteria
- Bot boots against a Postgres `DATABASE_URL`, runs migrations on startup, and creates no local
  `database.db`.
- `/set-guild-settings` twice for the same guild updates one row (no duplicates); two different
  guilds can share a schedule name; deleting a guild removes its cronjobs.
- `docker compose down` then `up` preserves all guild settings and schedules.
- No Discord ID is ever truncated (columns are `BIGINT`).
- Building the bot image requires no live database (migrations embedded at compile time).
