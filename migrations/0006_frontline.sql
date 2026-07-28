-- Per-guild opt-in for the frontline overlay.
--
-- Off by default, and not backfilled to anything else, for the same reason the
-- faction tint isn't: the line is a departure from the real map art, and a
-- guild that never asks for it keeps getting exactly what it got before. With
-- the column false the render is byte-identical to today's.
--
-- Both commands, unlike the tint. The tint answers "who holds this hex" at hex
-- resolution and is only worth the colour wash across 53 of them; a frontline
-- answers a sub-region question, and the hex a player cares about most is the
-- contested one — precisely where a single flat colour is least true. See
-- specs/active/frontline.md.
ALTER TABLE guilds
    ADD COLUMN frontline BOOLEAN NOT NULL DEFAULT FALSE;
