//! The 53 world-conquest regions: their API identifiers, the names the game
//! actually displays, and their position on the hex grid.
//!
//! The old autocomplete derived the label with `name.replace("Hex", "")`, which
//! is wrong for a good third of the map: `MooringCountyHex` is "The Moors",
//! `DeadLandsHex` is "Deadlands", `OarbreakerHex` is "Oarbreaker Isles". An
//! explicit table is the only thing that gets these right.
//!
//! The grid coordinates live here rather than beside the renderer so that a
//! region Siege Camp ships is **one** edit, not two. They are a code constant on
//! purpose: a new region needs new art in `assets/Maps/` anyway, so the layout
//! can never change without a rebuild. See `specs/full-map-renderer.md`
//! for the derivation.

/// One region of the world-conquest map.
pub struct Region {
    /// The `*Hex` identifier the API uses, and the `Map{name}.TGA` asset stem.
    pub api_name: &'static str,
    /// What the game shows players. Frequently unrelated to `api_name`.
    pub display_name: &'static str,
    /// Hex-grid column, left to right. Odd columns sit half a hex lower.
    pub col: u32,
    /// Hex-grid row within the column, top to bottom.
    pub row: u32,
}

const fn region(
    api_name: &'static str,
    display_name: &'static str,
    col: u32,
    row: u32,
) -> Region {
    Region {
        api_name,
        display_name,
        col,
        row,
    }
}

/// All 53 world-conquest regions, in column-major grid order.
pub const REGIONS: &[Region] = &[
    region("OlavisWakeHex", "Olavis Wake", 0, 2),
    region("PariPeakHex", "Pari Peak", 1, 1),
    region("PalantineBermHex", "Palantine Berm", 1, 2),
    region("OarbreakerHex", "Oarbreaker Isles", 1, 3),
    region("KuuraStrandHex", "Kuura Strand", 2, 1),
    region("GutterHex", "The Gutter", 2, 2),
    region("FishermansRowHex", "Fishermans Row", 2, 3),
    region("StemaLandingHex", "Stema Landing", 2, 4),
    region("NevishLineHex", "Nevish Line", 3, 1),
    region("FarranacCoastHex", "Farranac Coast", 3, 2),
    region("WestgateHex", "Westgate", 3, 3),
    region("OriginHex", "Origin", 3, 4),
    region("CallumsCapeHex", "Callums Cape", 4, 1),
    region("StonecradleHex", "Stonecradle", 4, 2),
    region("KingsCageHex", "Kings Cage", 4, 3),
    region("SableportHex", "Sableport", 4, 4),
    region("AshFieldsHex", "Ash Fields", 4, 5),
    region("SpeakingWoodsHex", "Speaking Woods", 5, 0),
    region("MooringCountyHex", "The Moors", 5, 1),
    region("LinnMercyHex", "The Linn of Mercy", 5, 2),
    region("LochMorHex", "Loch Mór", 5, 3),
    region("HeartlandsHex", "The Heartlands", 5, 4),
    region("RedRiverHex", "Red River", 5, 5),
    region("BasinSionnachHex", "Basin Sionnach", 6, 0),
    region("ReachingTrailHex", "Reaching Trail", 6, 1),
    region("CallahansPassageHex", "Callahans Passage", 6, 2),
    region("DeadLandsHex", "Deadlands", 6, 3),
    region("UmbralWildwoodHex", "Umbral Wildwood", 6, 4),
    region("GreatMarchHex", "Great March", 6, 5),
    region("KalokaiHex", "Kalokai", 6, 6),
    region("HowlCountyHex", "Howl County", 7, 0),
    region("ViperPitHex", "Viper Pit", 7, 1),
    region("MarbanHollowHex", "Marban Hollow", 7, 2),
    region("DrownedValeHex", "The Drowned Vale", 7, 3),
    region("ShackledChasmHex", "Shackled Chasm", 7, 4),
    region("AcrithiaHex", "Acrithia", 7, 5),
    region("ClansheadValleyHex", "Clanshead Valley", 8, 1),
    region("WeatheredExpanseHex", "Weathered Expanse", 8, 2),
    region("ClahstraHex", "The Clahstra", 8, 3),
    region("AllodsBightHex", "Allods Bight", 8, 4),
    region("TerminusHex", "Terminus", 8, 5),
    region("MorgensCrossingHex", "Morgens Crossing", 9, 1),
    region("StlicanShelfHex", "Stlican Shelf", 9, 2),
    region("EndlessShoreHex", "Endless Shore", 9, 3),
    region("ReaversPassHex", "Reavers Pass", 9, 4),
    region("GodcroftsHex", "Godcrofts", 10, 2),
    region("TempestIslandHex", "Tempest Island", 10, 3),
    region("WrestaHex", "Wresta", 10, 4),
    region("OnyxHex", "Onyx", 10, 5),
    region("LykosIsleHex", "Lykos Isle", 11, 2),
    region("TheFingersHex", "The Fingers", 11, 3),
    region("TyrantFoothillsHex", "Tyrant Foothills", 11, 4),
    region("PipersEnclaveHex", "Pipers Enclave", 12, 4),
];

/// The six grid steps to a hex's neighbours, as `(col, row)` deltas.
///
/// The grid is flat-top **odd-q offset**: odd columns sit half a hex lower than
/// even ones (see `Region::col`), so the two diagonal pairs shift by one row
/// depending on the column's parity. The vertical pair never does.
const fn neighbour_deltas(col: u32) -> [(i64, i64); 6] {
    if col % 2 == 1 {
        [(0, -1), (0, 1), (-1, 0), (-1, 1), (1, 0), (1, 1)]
    } else {
        [(0, -1), (0, 1), (-1, -1), (-1, 0), (1, -1), (1, 0)]
    }
}

/// The regions sharing an edge with this one.
///
/// Derived from `(col, row)` rather than tabulated, for the same reason the grid
/// coordinates live in this file at all: a region Siege Camp ships should be one
/// edit. A neighbour column would be a second, and the kind that fails quietly —
/// a stale entry renders a map that is merely slightly wrong.
///
/// The world is not a filled rectangle, so degree runs 2–6: the coastal and
/// corner regions simply have fewer grid slots occupied around them.
pub fn neighbours(region: &Region) -> Vec<&'static Region> {
    neighbour_deltas(region.col)
        .iter()
        .filter_map(|(d_col, d_row)| {
            let col = u32::try_from(i64::from(region.col) + d_col).ok()?;
            let row = u32::try_from(i64::from(region.row) + d_row).ok()?;

            REGIONS
                .iter()
                .find(|candidate| candidate.col == col && candidate.row == row)
        })
        .collect()
}

/// Looks a region up by its API identifier.
pub fn find(api_name: &str) -> Option<&'static Region> {
    REGIONS.iter().find(|region| same_region(region.api_name, api_name))
}

/// Do two identifiers name the same region?
///
/// Deliberately lenient, because the API's own spelling is not stable: it
/// serves Marban Hollow as `MarbanHollow` while the assets and this table say
/// `MarbanHollowHex`, and `DeadLandsHex` has been seen written `DeadlandsHex`.
/// An exact `==` turns a spelling drift into a region that silently doesn't
/// exist — a hole in the full map, and a `/get-map` that can't find its own
/// background file.
///
/// So: case-insensitive, and the `Hex` suffix is optional. No two regions
/// collide under that rule.
pub fn same_region(a: &str, b: &str) -> bool {
    strip_hex(a).eq_ignore_ascii_case(strip_hex(b))
}

fn strip_hex(name: &str) -> &str {
    let name = name.trim();

    match name.len().checked_sub(3) {
        Some(cut) if name.get(cut..).is_some_and(|s| s.eq_ignore_ascii_case("hex")) => &name[..cut],
        _ => name,
    }
}

/// The name this region's assets are filed under, for an identifier that may be
/// spelled however the API felt like spelling it.
pub fn asset_name(api_name: &str) -> &str {
    match find(api_name) {
        Some(region) => region.api_name,
        None => api_name,
    }
}

/// The display name for an API region identifier.
///
/// Unknown identifiers — a region Siege Camp ships before we update the table —
/// fall back to de-camel-casing, which is wrong-ish but readable, and never
/// hides the region from users.
pub fn display_name(api_name: &str) -> String {
    if let Some(region) = find(api_name) {
        return region.display_name.to_string();
    }

    split_camel_case(api_name.strip_suffix("Hex").unwrap_or(api_name))
}

fn split_camel_case(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 4);

    for (i, ch) in s.char_indices() {
        if i > 0 && ch.is_uppercase() {
            out.push(' ');
        }
        out.push(ch);
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names_of(region: &Region) -> Vec<&'static str> {
        let mut names: Vec<_> = neighbours(region)
            .iter()
            .map(|region| region.display_name)
            .collect();
        names.sort_unstable();
        names
    }

    /// Regions are compared by `api_name`, not by address: `REGIONS` is a
    /// `const`, so every use site may get its own promoted copy and `ptr::eq`
    /// reports two references to the same region as different.
    fn is(a: &Region, b: &Region) -> bool {
        a.api_name == b.api_name
    }

    /// If A borders B then B borders A. This is what actually catches a wrong
    /// parity rule: get the odd/even split backwards and the diagonals stop
    /// agreeing with each other, everywhere at once.
    #[test]
    fn adjacency_is_symmetric() {
        for region in REGIONS {
            for neighbour in neighbours(region) {
                assert!(
                    neighbours(neighbour).iter().any(|back| is(back, region)),
                    "{} lists {} as a neighbour, but not the reverse",
                    region.display_name,
                    neighbour.display_name,
                );
            }
        }
    }

    /// No region is isolated, none exceeds six, and none is its own neighbour.
    #[test]
    fn degrees_are_plausible() {
        for region in REGIONS {
            let found = neighbours(region);

            assert!(
                (2..=6).contains(&found.len()),
                "{} has {} neighbours, expected 2-6",
                region.display_name,
                found.len(),
            );
            assert!(
                !found.iter().any(|other| is(other, region)),
                "{} is its own neighbour",
                region.display_name,
            );
        }
    }

    /// Hand-checked against the in-game map. Deadlands is the useful case: it is
    /// interior, so all six slots are filled, and its ring is well known.
    #[test]
    fn matches_the_real_map() {
        let deadlands = find("DeadLandsHex").expect("Deadlands is in the table");

        assert_eq!(
            names_of(deadlands),
            [
                "Callahans Passage",
                "Loch Mór",
                "Marban Hollow",
                "The Drowned Vale",
                "The Linn of Mercy",
                "Umbral Wildwood",
            ],
        );
    }

    /// The edges of the world, where the grid runs out rather than the map.
    #[test]
    fn edges_of_the_map_have_fewer() {
        let enclave = find("PipersEnclaveHex").expect("Pipers Enclave is in the table");
        assert_eq!(names_of(enclave), ["The Fingers", "Tyrant Foothills"]);

        let olavis = find("OlavisWakeHex").expect("Olavis Wake is in the table");
        assert_eq!(names_of(olavis), ["Palantine Berm", "Pari Peak"]);
    }
}
