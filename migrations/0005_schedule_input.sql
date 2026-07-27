-- Structured schedule input and timezones.
-- See specs/active/schedule-input.md.
--
-- Two reported problems with one root cause: the bot asked users to express a
-- time in a language it never taught them (a free-text box handed straight to a
-- cron parser), and then interpreted it in a timezone they never chose. The
-- columns here are the storage half of the fix; the command surface stops
-- accepting free text at all.

-- The server's default timezone, as an IANA name (`Europe/Bratislava`), never a
-- fixed offset — an offset stored in summer is wrong in winter.
ALTER TABLE guilds ADD COLUMN timezone TEXT NOT NULL DEFAULT 'UTC';

-- Resolved when the schedule is created and stored on the row, rather than read
-- through to the guild's default at tick time. Changing the server default must
-- not silently move a report that already exists and that people have arranged
-- their day around.
ALTER TABLE cronjobs ADD COLUMN timezone TEXT NOT NULL DEFAULT 'UTC';

-- The cadence in words ("every 6 hours — 03:30, 09:30, 15:30, 21:30"), for the
-- report embed and for listings. `schedule` keeps its meaning — a string the
-- scheduler accepts — so nothing about existing rows changes; it just holds a
-- generated cron expression for new ones instead of whatever the user typed.
--
-- NULL for every row created before this migration, which display falls back
-- from by showing `schedule` as it always did.
ALTER TABLE cronjobs ADD COLUMN schedule_label TEXT;

-- Both timezone columns default to 'UTC', which is exactly what every existing
-- schedule already assumed. Nobody's report moves because this shipped.
