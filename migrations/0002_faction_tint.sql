-- Per-guild opt-in for the full map's faction control tint.
--
-- Off by default, and deliberately not backfilled to anything else: the tint is
-- a visual departure from the real map art (specs/active/full-map-renderer.md),
-- so a guild that never asks for it keeps getting exactly what it got before.
--
-- Full map only. /get-map renders a single hex at a scale where the terrain is
-- the point, and a colour wash over it hurts more than it says.
ALTER TABLE guilds
    ADD COLUMN full_map_faction_tint BOOLEAN NOT NULL DEFAULT FALSE;
