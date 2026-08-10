-- Fresh Postgres schema for FoxholeWarBot.
-- See specs/postgres.md. No data is carried over from the old
-- SQLite `database.db`; guilds re-run /set-guild-settings and re-create schedules.

CREATE TABLE guilds (
    id                  BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    guild_id            BIGINT  NOT NULL UNIQUE,           -- Discord snowflake; UNIQUE fixes QA C-10
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
    job_id      TEXT,                                      -- scheduler UUID
    UNIQUE (guild, job_name)                               -- per-guild names; fixes QA B-2
);

CREATE INDEX cronjobs_guild_idx ON cronjobs (guild);
