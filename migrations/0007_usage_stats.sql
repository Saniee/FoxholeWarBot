-- Counters for how much each command is actually used.
-- See specs/usage-stats.md.
--
-- The question this answers is the operator's: is the schedule system carrying
-- its weight, and is anyone running the one-shot commands. Nothing here is
-- per-user and nothing here is an event log — a row is a *tally*, one per
-- (day, command, server), incremented in place.
--
-- That shape is the privacy decision, not an optimisation. A row per invocation
-- would be an activity log of what each server did and when, which is a
-- different thing to store and a different sentence in docs/privacy.md. A
-- counter can say "this server used /full-map 40 times on Tuesday" and can never
-- say when, in what order, or by whom.
CREATE TABLE usage_daily (
    -- UTC, always. The scheduler ticks in UTC (specs/scheduling.md) and a
    -- deployment's local midnight is not a fact worth having in the data.
    day      DATE    NOT NULL,
    -- The slash command's qualified name ("get-map", "full-map-requests list"),
    -- or one of the synthetic names in `utils::usage` for a scheduled delivery.
    -- Free text rather than an enum: a command added later must not need a
    -- migration before it can be counted, and a name that stops being used just
    -- stops appearing.
    command  TEXT    NOT NULL,
    -- The Discord snowflake, deliberately **not** a reference to `guilds.id`.
    --
    -- A command can be invoked by a server that has never run
    -- /set-guild-settings — it gets the setup prompt — and those attempts are
    -- some of the most interesting ones there are: people reaching for the bot
    -- before it is configured. An FK would have nothing to point at, so they
    -- would either be dropped or force a nullable key column.
    --
    -- The cleanup an FK would have given is done explicitly in
    -- `Database::delete_guild`, in the same transaction as the settings, so
    -- removing the bot still takes a server's counts with it.
    guild_id BIGINT  NOT NULL,
    uses     INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (day, command, guild_id)
);

-- No second index, deliberately. Every read is "the last N days" and so is the
-- retention purge, and the primary key's own btree leads with `day` — a
-- `usage_daily (day)` index alongside it would be a prefix of one that already
-- exists, paid for on every increment.
