-- The approval gate for *scheduled* full-map reports.
-- See specs/active/premium-full-map.md.
--
-- Rendering the full map on demand stays free and ungated for everyone. What is
-- gated is putting one on a timer, because that cost recurs forever, and it
-- recurs identically for a 5-member guild and a 500-member one. There is no size
-- exemption and no paid tier: this is an approval queue, not a paywall.

-- Approval state. Deliberately just a flag and its timestamp — nothing here is
-- an entitlement *level*, and there is no expiry column, because no paid or
-- donation tier ships (see the spec's ToS findings).
ALTER TABLE guilds
    ADD COLUMN full_map_approved    BOOLEAN NOT NULL DEFAULT FALSE,
    ADD COLUMN full_map_approved_at TIMESTAMPTZ;

-- Which kind of report a schedule is: a NULL `map_name` is the full-map job.
--
-- No separate `is_full_map` flag, deliberately. A boolean beside a nullable
-- name is two facts that can disagree, and the one thing worse than a job with
-- no region is a job that claims a region it hasn't got. NULL says it once, and
-- it arrives in Rust as an `Option` the compiler makes every call site answer.
--
-- The alternative was keeping the column NOT NULL with a sentinel like
-- 'full-map' in it — a value sitting in a field the rest of the code treats as a
-- region identifier, one missed branch away from asking the API for a region by
-- that name.
--
-- `dormant_notified` remembers that we have already told this job's channel its
-- approval was withdrawn. A revoked full-map schedule is not deleted — it stops
-- rendering and resumes if approval comes back — but "stopped without saying so"
-- looks exactly like the bot being broken, and saying so on every tick is worse
-- than saying nothing. Cleared whenever the guild is approved again.
ALTER TABLE cronjobs
    ALTER COLUMN map_name DROP NOT NULL,
    ADD COLUMN dormant_notified BOOLEAN NOT NULL DEFAULT FALSE;

-- The application queue.
--
-- `guild` is the surrogate `guilds.id`, like `cronjobs.guild`, so a guild must
-- have run /set-guild-settings before it can apply. That is not an extra hurdle:
-- scheduling anything at all already requires those settings, and this form
-- exists only to reach a schedule.
CREATE TABLE full_map_requests (
    id           BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    guild        BIGINT NOT NULL REFERENCES guilds(id) ON DELETE CASCADE,
    requested_by BIGINT NOT NULL,                        -- Discord user snowflake
    -- Review context only. Nothing reads this to decide anything, which is why
    -- it is a snapshot taken when the form is filed rather than a cached column
    -- on `guilds` that would have to be kept fresh.
    member_count INTEGER,
    cadence      TEXT   NOT NULL,                        -- the requested schedule, as typed
    channel_id   BIGINT NOT NULL,                        -- where the report would post
    use_case     TEXT,
    audience     TEXT,
    contact      TEXT,
    status       TEXT   NOT NULL DEFAULT 'pending',
    reviewed_by  BIGINT,                                 -- reviewer's Discord snowflake
    reviewed_at  TIMESTAMPTZ,
    created_at   TIMESTAMPTZ NOT NULL DEFAULT now(),
    CONSTRAINT full_map_requests_status
        CHECK (status IN ('pending', 'approved', 'denied', 'withdrawn'))
);

CREATE INDEX full_map_requests_guild_idx ON full_map_requests (guild);

-- The reviewer's queue is "everything still pending, oldest first", and it is
-- the only read that happens on a human's timescale.
CREATE INDEX full_map_requests_pending_idx
    ON full_map_requests (created_at)
    WHERE status = 'pending';

-- One open application per guild. Without this, a guild that runs the command
-- twice puts two rows in front of the reviewer, and approving one leaves the
-- other pending forever.
CREATE UNIQUE INDEX full_map_requests_one_pending_idx
    ON full_map_requests (guild)
    WHERE status = 'pending';
